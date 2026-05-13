import { signal, _createScope } from "./reactivity.js";
import { commit } from "./template.js";
import {
  _compileRoutePattern,
  _matchRoutes,
  _normalizePath,
  _parsePathAndQuery,
  _parseQuery,
} from "./router.js";

/** @type {App | null} */
let _currentApp = null;

/**
 * @internal
 * @returns {App | null}
 */
export function _getCurrentApp() {
  return _currentApp;
}

/**
 * Overwrite the currently running app. Exported for tests — prefer `app.run()` in
 * production code.
 * @internal
 * @param {App | null} app
 */
export function _setCurrentApp(app) {
  _currentApp = app;
}

/**
 * Top-level application object. Owns app-level state, the route table, and the
 * render lifecycle. Construct once, configure with the builder methods, then
 * call `run()` to mount to the DOM.
 *
 * ```js
 * new App()
 *   .state('user', userSignal)
 *   .layout(RootLayout)
 *   .route('/', Home)
 *   .route('/about', About)
 *   .run('#app');
 * ```
 *
 * All builder methods return `this` and throw if called after `run()`.
 */
export class App {
  constructor() {
    this._state = new Map();
    this._routes = [];
    this._layout = null;
    this._pathSig = signal("");
    this._paramsSig = signal({});
    this._querySig = signal({});
    this._mountEl = null;
    this._routeScope = null;
    this._running = false;
  }

  /**
   * @private
   * @param {string} method
   */
  _assertNotRunning(method) {
    if (this._running)
      throw new Error(`App.${method}() cannot be called after run()`);
  }

  /**
   * Register a value under `key` for retrieval via `inject()`.
   * Stored as-is — signals, plain objects, and future machine instances are all valid.
   * Throws if the key is already registered or if the app is already running.
   * @param {string} key
   * @param {unknown} value
   * @returns {this}
   */
  state(key, value) {
    this._assertNotRunning("state");
    if (this._state.has(key))
      throw new Error(`App.state: key "${key}" already registered`);
    this._state.set(key, value);
    return this;
  }

  /**
   * Set a layout component that wraps every route's output.
   * Called with `{ children }` where `children` is the matched route's TemplateResult.
   * Only one layout per app; throws on a second call or if `component` is not a function.
   * @param {(props: { children: object }) => object} component
   * @returns {this}
   */
  layout(component) {
    this._assertNotRunning("layout");
    if (this._layout != null) throw new Error("App.layout: layout already set");
    if (typeof component !== "function")
      throw new Error("App.layout: component must be a function");
    this._layout = component;
    return this;
  }

  /**
   * Register a route. Registration order is match order — first match wins.
   * `loaderOrComponent` may be an eager component (returns a TemplateResult synchronously)
   * or a lazy loader (returns a Promise of a module whose `.default` is the component).
   * The resolved component is cached after the first load.
   * @param {string} pattern - Exact path, `:param` segments, or bare `*` wildcard.
   * @param {Function} loaderOrComponent
   * @returns {this}
   */
  route(pattern, loaderOrComponent) {
    this._assertNotRunning("route");
    if (typeof loaderOrComponent !== "function")
      throw new Error("App.route: handler must be a function");
    const normalized = _normalizePath(pattern);
    this._routes.push({
      pattern,
      normalized,
      compiled: _compileRoutePattern(normalized),
      loader: loaderOrComponent,
      resolvedComponent: null,
    });
    return this;
  }

  /**
   * Test-helper: match `input` against the registered routes without rendering anything.
   * @param {string} input - Path-and-query string, e.g. `/users/42?tab=posts`.
   * @returns {{ route: object, params: Record<string,string>, query: Record<string,string>, pathname: string, search: string } | null}
   */
  match(input) {
    return _matchRoutes(this._routes, input);
  }

  /**
   * Mount the app to the DOM element matched by `selector` and start the render lifecycle.
   * - Resolves the mount element via `document.querySelector(selector)`.
   * - Registers this app as the currently running app so `inject()` resolves correctly.
   * - Renders the route matching the current `window.location`.
   * - Attaches a `popstate` listener for back/forward navigation.
   * - Attaches a document-level click listener to intercept same-origin `<a>` clicks.
   * Throws if already running or if the selector matches nothing.
   * @param {string} selector
   */
  run(selector) {
    if (this._running) throw new Error("App.run: already running");
    const el = document.querySelector(selector);
    if (!el)
      throw new Error(`App.run: element not found for selector "${selector}"`);
    this._mountEl = el;
    this._running = true;
    _setCurrentApp(this);

    const initialPath = window.location.pathname + window.location.search;
    this._navigateTo(initialPath);

    const onPopstate = () =>
      this._navigateTo(window.location.pathname + window.location.search);
    this._popstateListener = onPopstate;
    window.addEventListener("popstate", onPopstate);

    const onClick = (e) => _onDocumentClick(e);
    this._clickListener = onClick;
    document.addEventListener("click", onClick);
  }

  /**
   * Core navigation pipeline. Called by `run()`, the popstate listener, and
   * the document click interceptor. Disposes the previous route scope, updates
   * reactive signals, then commits the new route (and optional layout) inside a
   * fresh scope. For lazy loaders, the commit happens after the import resolves,
   * leaving prior content visible during the await.
   * @private
   * @param {string} input - Path-and-query string.
   */
  _navigateTo(input) {
    if (this._routeScope) {
      this._routeScope.dispose();
      this._routeScope = null;
    }

    const m = _matchRoutes(this._routes, input);

    if (m) {
      this._pathSig.set(m.pathname);
      this._paramsSig.set(m.params);
      this._querySig.set(m.query);
    } else {
      const { pathname, search } = _parsePathAndQuery(input);
      this._pathSig.set(_normalizePath(pathname));
      this._paramsSig.set({});
      this._querySig.set(_parseQuery(search));
    }

    if (m == null) {
      _clearChildren(this._mountEl);
      return;
    }

    this._routeScope = _createScope();
    const app = this;
    this._routeScope.run(async () => {
      const entry = m.route;
      let routeTR;
      if (entry.resolvedComponent != null) {
        routeTR = entry.resolvedComponent({ params: m.params, query: m.query });
      } else {
        const ret = entry.loader({ params: m.params, query: m.query });
        if (ret != null && typeof ret.then === "function") {
          // Lazy loader: await the import and cache the default export.
          const mod = await ret;
          entry.resolvedComponent = mod.default;
          routeTR = entry.resolvedComponent({
            params: m.params,
            query: m.query,
          });
        } else {
          // Eager component: reuse the TemplateResult from the detection call.
          entry.resolvedComponent = entry.loader;
          routeTR = ret;
        }
      }
      const mountTR = app._layout
        ? app._layout({ children: routeTR })
        : routeTR;
      _clearChildren(app._mountEl);
      commit(mountTR, app._mountEl);
      _applyActiveLinks(app._mountEl, m.pathname, m.search);
    });
  }

  /**
   * @private
   * @param {string} key
   * @returns {unknown}
   */
  _getState(key) {
    if (!this._state.has(key))
      throw new Error(`inject: key "${key}" is not registered`);
    return this._state.get(key);
  }
}

/** @param {Element} el */
function _clearChildren(el) {
  while (el.childNodes.length > 0) el.removeChild(el.childNodes[0]);
}

/**
 * Set `data-active` and `data-active-exact` on `<a>` tags inside `mountEl`
 * based on `currentPath` and `currentSearch`. Runs synchronously after each commit.
 * Only same-origin hrefs are considered; external and hash-only hrefs are skipped.
 * @param {Element} mountEl
 * @param {string} currentPath
 * @param {string} currentSearch
 */
function _applyActiveLinks(mountEl, currentPath, currentSearch) {
  const anchors = mountEl.querySelectorAll("a");
  for (const anchor of anchors) {
    const href = anchor.getAttribute("href");
    if (!href || href.startsWith("#")) {
      anchor.removeAttribute("data-active");
      anchor.removeAttribute("data-active-exact");
      continue;
    }
    let path, search;
    if (href.startsWith("/")) {
      const q = href.indexOf("?");
      if (q >= 0) {
        path = href.slice(0, q);
        search = href.slice(q);
      } else {
        path = href;
        search = "";
      }
    } else if (href.startsWith(window.location.origin)) {
      const stripped = href.slice(window.location.origin.length);
      const q = stripped.indexOf("?");
      if (q >= 0) {
        path = stripped.slice(0, q);
        search = stripped.slice(q);
      } else {
        path = stripped;
        search = "";
      }
    } else {
      anchor.removeAttribute("data-active");
      anchor.removeAttribute("data-active-exact");
      continue;
    }
    path = _normalizePath(path);
    const isExact = path === currentPath && search === currentSearch;
    const isPrefix = currentPath === path || currentPath.startsWith(path + "/");
    if (isExact) {
      anchor.setAttribute("data-active-exact", "");
      anchor.setAttribute("data-active", "");
    } else if (isPrefix) {
      anchor.setAttribute("data-active", "");
      anchor.removeAttribute("data-active-exact");
    } else {
      anchor.removeAttribute("data-active");
      anchor.removeAttribute("data-active-exact");
    }
  }
}

/** @param {string} input */
function _navigateFromClick(input) {
  const app = _getCurrentApp();
  if (!app) return;
  window.history.pushState(null, "", input);
  app._navigateTo(input);
}

/**
 * Document-level click handler that intercepts same-origin `<a>` clicks and
 * routes them through the app instead of triggering a full page load.
 * Walks up from `event.target` to find the nearest anchor ancestor.
 * Skips modified clicks, non-left-button clicks, external links, and anchors
 * with `target`, `download`, or `data-external` attributes.
 * @param {MouseEvent} e
 */
function _onDocumentClick(e) {
  if (e.defaultPrevented) return;
  if (e.button != null && e.button !== 0) return;
  if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
  let anchor = e.target;
  while (anchor && anchor.tagName !== "A") anchor = anchor.parentNode;
  if (!anchor) return;
  const target = anchor.getAttribute("target");
  if (target && target !== "_self") return;
  if (anchor.hasAttribute("download")) return;
  if (anchor.hasAttribute("data-external")) return;
  const href = anchor.getAttribute("href");
  if (!href) return;
  if (href.startsWith("#")) return;
  if (
    /^[a-z][a-z0-9+\-.]*:/i.test(href) &&
    !href.startsWith(window.location.origin)
  )
    return;
  e.preventDefault();
  const stripped = href.startsWith(window.location.origin)
    ? href.slice(window.location.origin.length)
    : href;
  _navigateFromClick(stripped);
}

/**
 * Retrieve a value registered with `app.state(key, value)` on the currently
 * running app. Throws if no app is running or if `key` was not registered.
 * @template T
 * @param {string} key
 * @returns {T}
 */
export function inject(key) {
  if (_currentApp == null) throw new Error("inject: no app is running");
  return _currentApp._getState(key);
}
