import { signal, _createScope } from "./reactivity.js";
import { commit, html } from "./template.js";
import {
  _compileRoutePattern,
  _joinPaths,
  _matchRoutes,
  _normalizePath,
  _parsePathAndQuery,
  _parseQuery,
} from "./router.js";

/** @type {App | null} */
let _currentApp = null;

/**
 * Compose two `AbortSignal`s into a single signal that aborts when either
 * input aborts. Prefers `AbortSignal.any` when available; otherwise wires up
 * a fresh controller listening to both inputs.
 * @internal
 * @param {AbortSignal} a
 * @param {AbortSignal} b
 * @returns {AbortSignal}
 */
function _composeSignals(a, b) {
  if (typeof AbortSignal !== "undefined" && typeof AbortSignal.any === "function") {
    return AbortSignal.any([a, b]);
  }
  const ctrl = new AbortController();
  const onAbort = (src) => () => { ctrl.abort(src.reason); };
  if (a.aborted) ctrl.abort(a.reason);
  else a.addEventListener("abort", onAbort(a));
  if (b.aborted) ctrl.abort(b.reason);
  else b.addEventListener("abort", onAbort(b));
  return ctrl.signal;
}

/**
 * Build a route-scoped `fetch` wrapper that threads `navSignal` into every
 * request. Caller-supplied signals are composed with the nav signal so an
 * abort on either aborts the request.
 * @internal
 * @param {AbortSignal} navSignal
 * @returns {(input: RequestInfo | URL, init?: RequestInit) => Promise<Response>}
 */
function _makeRouteFetch(navSignal) {
  return (input, init = {}) => {
    const callerSignal = init.signal;
    const signal = callerSignal ? _composeSignals(navSignal, callerSignal) : navSignal;
    return globalThis.fetch(input, { ...init, signal });
  };
}

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
    this._running = false;
    this._rootSlotSig = signal(null);
    this._rootScope = null;
    this._stateProxy = new Proxy({}, { get: (_, key) => this._state.get(key) });
    this._middleware = [];
    this._navToken = 0;
    this._loading = null;
    this._error = null;
    this._navScope = null;
    /** @type {Array<{ descriptor: object, scope: object, outletSig: object|null }>} */
    this._chain = [];
    /** @type {string|null} */
    this._lastCommittedUrl = null;
  }

  /**
   * Return the index of the first diverging entry between the committed chain and new descriptors.
   * @private
   * @param {Array<{ descriptor: object }>} oldChain
   * @param {object[]} newDescriptors
   * @returns {number}
   */
  _computeDivergence(oldChain, newDescriptors) {
    let i = 0;
    while (i < oldChain.length && i < newDescriptors.length && oldChain[i].descriptor === newDescriptors[i]) {
      i++;
    }
    return i;
  }

  /**
   * Resolve the loading component: first per-route override at/below `divergeAt`
   * wins; falls back to `this._loading`; `null` if neither is set.
   * @private
   * @param {object[]} chainDescriptors
   * @param {number} divergeAt
   * @returns {Function|null}
   */
  _resolveLoadingFor(chainDescriptors, divergeAt) {
    for (let i = divergeAt; i < chainDescriptors.length; i++) {
      if (chainDescriptors[i].opts.loading) return chainDescriptors[i].opts.loading;
    }
    return this._loading;
  }

  /**
   * Shallow-merge `meta` from all chain descriptors (root → leaf; child wins).
   * @private
   * @param {object[]} chainDescriptors
   * @returns {object}
   */
  _mergeMeta(chainDescriptors) {
    return chainDescriptors.reduce((m, d) => Object.assign({}, m, d.opts.meta || {}), {});
  }

  /**
   * Return the reactive slot for the given divergence index.
   * Index 0 = root slot; otherwise the outletSig of the ancestor entry.
   * @private
   * @param {number} divergeAt
   * @returns {{ set(v: unknown): void }}
   */
  _slotAt(divergeAt) {
    if (divergeAt === 0) return this._rootSlotSig;
    return this._chain[divergeAt - 1].outletSig;
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
   * Register a middleware function that runs once per navigation before any route
   * pipeline step. Multiple calls form an ordered chain. Returns `this`.
   * @param {(ctx: { route: object, state: object, redirect: Function }) => void | Promise<void>} mw
   * @returns {this}
   */
  use(mw) {
    this._assertNotRunning("use");
    if (typeof mw !== "function")
      throw new Error("App.use: middleware must be a function");
    this._middleware.push(mw);
    return this;
  }

  /**
   * Register a global loading UI component. Returns `this`. Throws after `run()`.
   * @param {() => object} component
   * @returns {this}
   */
  loading(component) {
    this._assertNotRunning("loading");
    if (this._loading != null) throw new Error("App.loading: loading already set");
    if (typeof component !== "function")
      throw new Error("App.loading: component must be a function");
    this._loading = component;
    return this;
  }

  /**
   * Register a global error UI component. Returns `this`. Throws after `run()`.
   * @param {(props: { error: unknown, retry: Function }) => object} component
   * @returns {this}
   */
  error(component) {
    this._assertNotRunning("error");
    if (this._error != null) throw new Error("App.error: error already set");
    if (typeof component !== "function")
      throw new Error("App.error: component must be a function");
    this._error = component;
    return this;
  }

  /**
   * Register a route. Registration order is match order — first match wins.
   * `loaderOrComponent` may be an eager component (returns a TemplateResult synchronously)
   * or a lazy loader (returns a Promise of a module whose `.default` is the component).
   * The resolved component is cached after the first load.
   * @param {string} pattern - Exact path, `:param` segments, or bare `*` wildcard.
   * @param {Function} loaderOrComponent
   * @param {{ children?: Array<object>, guard?: Function, load?: Function, meta?: object, loading?: Function, error?: Function }} [opts]
   * @returns {this}
   */
  route(pattern, loaderOrComponent, opts = {}) {
    this._assertNotRunning("route");
    if (typeof loaderOrComponent !== "function")
      throw new Error("App.route: handler must be a function");
    if (opts.children != null && !Array.isArray(opts.children))
      throw new Error("App.route: opts.children must be an array");
    if (opts.guard != null && typeof opts.guard !== "function")
      throw new Error("App.route: guard must be a function");
    if (opts.load != null && typeof opts.load !== "function")
      throw new Error("App.route: load must be a function");
    if (opts.meta != null && (typeof opts.meta !== "object" || Array.isArray(opts.meta)))
      throw new Error("App.route: meta must be an object");
    if (opts.loading != null && typeof opts.loading !== "function")
      throw new Error("App.route: loading must be a function");
    if (opts.error != null && typeof opts.error !== "function")
      throw new Error("App.route: error must be a function");
    const normalized = _normalizePath(pattern);
    const { children, ...parentOpts } = opts;
    const parentDescriptor = _buildEntryDescriptor({ pattern, normalized, loaderOrLoad: loaderOrComponent, opts: parentOpts });
    this._flattenRoutes(parentDescriptor, [parentDescriptor], children);
    return this;
  }

  /**
   * Recursively flatten a route tree into `_routes`.
   * @private
   * @param {object} parentDescriptor
   * @param {object[]} chain - root-first chain leading to this entry
   * @param {Array<object>|undefined} children
   */
  _flattenRoutes(parentDescriptor, chain, children) {
    if (!children || children.length === 0) {
      const { normalized } = parentDescriptor;
      this._routes.push({
        pattern: parentDescriptor.pattern,
        normalized,
        compiled: _compileRoutePattern(normalized),
        loader: parentDescriptor.loaderOrLoad,
        opts: parentDescriptor.opts,
        resolvedComponent: null,
        chain,
      });
      return;
    }
    for (const child of children) {
      if (typeof child.load !== "function")
        throw new Error("App.route: each child entry must have a load function");
      const { children: grandChildren, ...childOpts } = child;
      const joinedNormalized = _joinPaths(parentDescriptor.normalized, child.path);
      const childDescriptor = _buildEntryDescriptor({
        pattern: child.path,
        normalized: joinedNormalized,
        loaderOrLoad: child.load,
        opts: childOpts,
      });
      this._flattenRoutes(childDescriptor, [...chain, childDescriptor], grandChildren);
    }
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

    this._rootScope = _createScope();
    this._rootScope.run(() => {
      if (this._layout) {
        const layoutTR = this._layout({ outlet: this._rootSlotSig });
        commit(layoutTR, this._mountEl);
      } else {
        const wrapperTR = html`${this._rootSlotSig}`;
        commit(wrapperTR, this._mountEl);
      }
    });

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
   * the document click interceptor. Runs the middleware chain, then resolves
   * and commits the matched route inside a fresh scope. Supersedes in-flight
   * navigations via a monotonic nav token.
   * @private
   * @param {string} input - Path-and-query string.
   */
  _navigateTo(input) {
    const token = ++this._navToken;

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
      this._rootSlotSig.set(null);
      return;
    }

    if (this._navScope) {
      this._navScope.dispose();
      this._navScope = null;
    }
    this._navScope = _createScope();

    const navController = new AbortController();
    this._navScope.onCleanup(() => navController.abort());

    const routeFetch = _makeRouteFetch(navController.signal);

    const app = this;
    (async () => {
      const state = app._stateProxy;
      const newChainDescriptors = m.route.chain;
      const newLen = newChainDescriptors.length;

      // Compute divergence — always rebuild at least the leaf.
      let divergeAt = app._computeDivergence(app._chain, newChainDescriptors);
      divergeAt = Math.min(divergeAt, newLen - 1);

      const parentSlot = app._slotAt(divergeAt);
      const mergedMeta = app._mergeMeta(newChainDescriptors);
      const routeCtx = { path: m.pathname, params: m.params, query: m.query, meta: mergedMeta };

      // Start 150ms loading timer at the divergence point.
      const resolvedLoading = app._resolveLoadingFor(newChainDescriptors, divergeAt);
      const loadingTimer = setTimeout(() => {
        if (token !== app._navToken) return;
        if (!resolvedLoading) return;
        app._navScope.run(() => {
          parentSlot.set(resolvedLoading());
        });
      }, 150);

      try {
        // Run global middleware chain (once per nav).
        for (const mw of app._middleware) {
          let didRedirect = false;
          const redirect = (path, opts = {}) => {
            didRedirect = true;
            app._navToken++;
            window.history.replaceState(null, "", path);
            app._navigateTo(path);
          };
          await mw({ route: routeCtx, state, redirect });
          if (token !== app._navToken) { clearTimeout(loadingTimer); return; }
          if (didRedirect) { clearTimeout(loadingTimer); return; }
        }

        // Walk chain from divergeAt → leaf: guard and load only.
        // Component detection and invocation happen in the build step (backward
        // pass), where outlet signals are known. This ensures each component is
        // called exactly once per navigation, avoiding double-call for parents.
        for (let i = divergeAt; i < newLen; i++) {
          const desc = newChainDescriptors[i];

          // Run guard.
          if (desc.opts.guard) {
            const redirect = (path, opts = {}) => {
              app._navToken++;
              window.history.replaceState(null, "", path);
              app._navigateTo(path);
            };
            const r = await desc.opts.guard({ params: m.params, query: m.query, state, route: routeCtx, redirect });
            if (token !== app._navToken) { clearTimeout(loadingTimer); return; }
            if (r === false) {
              clearTimeout(loadingTimer);
              if (app._lastCommittedUrl != null) {
                window.history.replaceState(null, "", app._lastCommittedUrl);
              }
              return;
            }
          }

          // Run load() (data hydration, separate from component).
          if (desc.opts.load) {
            await desc.opts.load({ params: m.params, query: m.query, state, fetch: routeFetch, route: routeCtx });
            if (token !== app._navToken) { clearTimeout(loadingTimer); return; }
          }
        }

        clearTimeout(loadingTimer);

        // Dispose old chain entries at/below divergeAt (leaf-first).
        for (let i = app._chain.length - 1; i >= divergeAt; i--) {
          app._chain[i].scope.dispose();
        }
        app._chain.length = divergeAt;

        // Build new chain entries leaf → divergeAt.
        // Component detection + invocation happens here so outlet is available.
        let childTR;
        const newEntries = [];
        for (let i = newLen - 1; i >= divergeAt; i--) {
          const desc = newChainDescriptors[i];
          const isLeaf = i === newLen - 1;
          const outletSig = !isLeaf ? signal(childTR) : null;
          const scope = _createScope();
          let tr;

          if (desc.resolvedComponent == null) {
            // First visit: detect lazy vs. eager, cache resolvedComponent.
            const ret = desc.loaderOrLoad({
              params: m.params, query: m.query, state,
              ...(outletSig != null ? { outlet: outletSig } : {}),
            });
            if (ret != null && typeof ret.then === "function") {
              const mod = await ret;
              if (token !== app._navToken) { clearTimeout(loadingTimer); return; }
              desc.resolvedComponent = mod.default;
              scope.run(() => {
                tr = desc.resolvedComponent({
                  params: m.params, query: m.query, state,
                  ...(outletSig != null ? { outlet: outletSig } : {}),
                });
              });
            } else {
              // Eager: the detection call IS the invocation. Reuse the TR.
              desc.resolvedComponent = desc.loaderOrLoad;
              scope.run(() => { tr = ret; });
            }
          } else {
            // Subsequent visit: call cached component with correct props.
            scope.run(() => {
              tr = desc.resolvedComponent({
                params: m.params, query: m.query, state,
                ...(outletSig != null ? { outlet: outletSig } : {}),
              });
            });
          }

          newEntries.unshift({ descriptor: desc, scope, outletSig });
          childTR = tr;
        }
        for (const entry of newEntries) app._chain.push(entry);

        // Swap the divergence point's parent slot to the new top TR.
        app._chain[divergeAt].scope.run(() => {
          parentSlot.set(childTR);
        });

        app._lastCommittedUrl = m.pathname + m.search;
        _applyActiveLinks(app._mountEl, m.pathname, m.search);
      } catch (err) {
        if (token !== app._navToken) return;
        // Silently drop AbortError when this navigation's controller fired it.
        // Caller-supplied aborts (where our controller is still live) flow through.
        if (err && err.name === 'AbortError' && navController.signal.aborted) {
          clearTimeout(loadingTimer);
          return;
        }
        clearTimeout(loadingTimer);
        if (app._error) {
          app._navScope.dispose();
          app._navScope = _createScope();
          const retry = () => app._navigateTo(input);
          app._navScope.run(() => {
            parentSlot.set(app._error({ error: err, retry }));
          });
          // Track errScope so next nav's tear-down picks it up.
          app._chain[divergeAt] = { descriptor: null, scope: app._navScope, outletSig: null };
          app._chain.length = divergeAt + 1;
        } else {
          console.error('navigation error', err);
        }
      }
    })();
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

/**
 * Build an entry descriptor object — the building block of the chain.
 * @param {{ pattern: string, normalized: string, loaderOrLoad: Function, opts: object }} params
 * @returns {{ pattern: string, normalized: string, loaderOrLoad: Function, opts: object, resolvedComponent: Function|null }}
 */
function _buildEntryDescriptor({ pattern, normalized, loaderOrLoad, opts }) {
  return { pattern, normalized, loaderOrLoad, opts, resolvedComponent: null };
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
