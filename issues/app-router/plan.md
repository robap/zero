# Plan: App Class and Minimal Router

## Summary
Phase 3a introduces a routing layer on top of the existing reactivity (Phase 1)
and templating (Phase 2) primitives. The work splits across three files:
`runtime/dom-shim.js` is extended with the browser globals required to drive
navigation in Node tests; a new pure-logic `runtime/router.js` owns path/query
parsing and pattern compilation; a new `runtime/app.js` owns the `App` class,
the render lifecycle, `inject`, link interception, and the active-link
attributes. The navigation API (`navigate`, `back`, `forward`, reactive
`route()`) lands last in `router.js` once the App's `_navigateTo` and the
current-app holder are in place. A module-level `_currentApp` variable in
`app.js` (set in `run()`, never explicitly cleared) supports the single-mounted-
app constraint; cross-file access uses a `_getCurrentApp` getter exported from
`app.js`.

## Prerequisites
None. The spec's Open Questions are resolved as follows and baked into this
plan:

- **Click-interception scope** — walk up from `event.target` via `parentNode`
  to find the nearest `<a>` ancestor (standard web behavior; nested elements
  inside an anchor still navigate).
- **Active-link walk** — O(N) synchronous walk over `<a>` tags under the mount
  element after each commit. Adequate for the surfaces this spec targets.
- **Currently-running-App lookup** — module-level variable in `app.js`. Each
  fresh `new App().run()` overwrites the prior reference; tests get clean
  isolation by construction.
- **Lazy-loader Promise detection** — call `loaderOrComponent({ params, query })`
  on first match. If the return is thenable, treat as a lazy loader (await,
  cache `module.default`). Otherwise, treat the function itself as the eager
  component and **reuse the returned TR** for the first render so the eager
  component is not called twice. Eager components must be synchronous.
- **Route component arity** — `{ params, query }` only. `data` is deferred to
  the follow-up spec; no `data: undefined` placeholder.

## Steps

- [x] **Step 1: Extend dom-shim with browser globals**
- [x] **Step 2: Pure routing primitives in router.js**
- [x] **Step 3: App class scaffolding and inject in app.js**
- [x] **Step 4: run() and full render lifecycle in app.js**
- [x] **Step 5: Navigation API and reactive route() in router.js**

---

## Step Details

### Step 1: Extend dom-shim with browser globals
**Goal:** Provide the synthetic browser surface the router and App rely on in
Node tests — a tiny selector engine for mounting, document-level events for
link interception, and a `window`/`history`/`location` triad for navigation.
Doing this first means later steps can be written against the same
`globalThis.document` / `globalThis.window` they will see in production.

**Files:**
- `runtime/dom-shim.js`
- `runtime/dom-shim.test.js`

**Changes:**
- **Selector subset.** Implement a minimal CSS selector parser supporting
  exactly two forms: `#id` (matches an element with `getAttribute('id')` equal
  to the suffix) and a lowercase tag name (`a`, `div`, …). Anything else
  throws `Error('dom-shim: unsupported selector "<selector>"')`. Internal
  helper `_matchSelector(node, selector)` and `_walkDescendants(root, fn)`.
- Add to elements (and to the `document` object where it makes sense):
  - `querySelector(selector)` — first descendant match, or `null`.
  - `querySelectorAll(selector)` — array of all descendant matches in
    document order. (Live HTMLCollections are not needed.)
  - `closest(selector)` — walk `parentNode` chain starting with the element
    itself; return first match or `null`.
- Add element conveniences used by router code:
  - `id` getter/setter proxying the `id` attribute.
  - `href` getter/setter proxying the `href` attribute.
  - `defaultPrevented` flag set when a synthetic event's `preventDefault()`
    is called (used by the click interceptor's short-circuit).
- **Document event surface.** Augment the existing `document` object with
  `_listeners: new Map()` plus `addEventListener(event, handler, options)`,
  `removeEventListener(event, handler)`, and `dispatchEvent(event)` — same
  shape as the element implementation. No bubbling; tests dispatch directly
  on `document` with `target` set to the inner-most element.
- **Window object.** Build a `window` object with the same event-target trio
  (`addEventListener`/`removeEventListener`/`dispatchEvent`). Attach:
  - `location` — a plain object with `origin: 'http://localhost'`, mutable
    `pathname`, `search`, `hash`, and a derived getter `href`. Add a hidden
    `_set(pathnameWithQueryAndHash)` helper that parses an input string and
    updates the three pieces atomically. This helper is the one
    `history.pushState` calls.
  - `history` — a stack-based implementation. Internal state:
    `_entries: [{ state, url }]` initialized with the current location,
    `_index: 0`. Methods:
    - `pushState(state, _title, url)` — truncate `_entries` past `_index`,
      append `{ state, url }`, advance `_index`, call `location._set(url)`.
    - `replaceState(state, _title, url)` — overwrite `_entries[_index]`,
      call `location._set(url)`.
    - `back()` — if `_index > 0`, decrement, sync location, dispatch a
      synthetic `popstate` event (`{ type: 'popstate', state: _entries[_index].state }`)
      on `window`. No-op if already at index 0.
    - `forward()` — symmetric.
  - `length` getter on history returns `_entries.length`.
- **Install globals.** Mirror the existing `globalThis.document` guard for
  `globalThis.window`: install only if not already present.
- Keep the existing `document` factories (`createElement`, etc.) intact;
  the additions are non-breaking.

**Tests (added to `runtime/dom-shim.test.js`):**
- `document.querySelector('#x')` finds a nested descendant with `id="x"`;
  returns `null` when absent.
- `el.querySelectorAll('a')` returns all anchors in document order across
  nesting depth.
- `el.closest('a')` matches the element itself, then ancestors; returns
  `null` if no ancestor matches.
- `document.addEventListener('click', fn) → document.dispatchEvent(...)`
  invokes the handler; `removeEventListener` un-registers it; `once: true`
  is honored.
- `window.history.pushState(null, '', '/about?x=1')` advances the index,
  appends an entry, and updates `window.location.pathname` and `.search`.
- `window.history.replaceState(...)` does not advance the index and rewrites
  the top entry.
- `window.history.back()` after two pushes dispatches a `popstate` event on
  `window` and rolls `location.pathname` back to the previous entry.
- `pushState` after `back()` truncates the forward history.

---

### Step 2: Pure routing primitives in router.js
**Goal:** Establish path matching and query parsing as pure functions with
zero DOM or App dependencies so they can be tested on their own and reused
by both `App.match()` and the render lifecycle.

**Files:**
- `runtime/router.js` (new)
- `runtime/router.test.js` (new)

**Changes (functions internal unless noted):**
- `_normalizePath(p)` — strip a single trailing `/` unless `p === '/'`.
  Return the result. Operates on the path portion only (caller separates
  query).
- `_parseQuery(search)` — accept either `''`, `'?'`, or `'?k=v&k2=v2'`.
  Decode keys and values via `decodeURIComponent`. Repeated keys: last wins.
  Return a plain object.
- `_parsePathAndQuery(input)` — split a `pathname[?query][#hash]` string
  into `{ pathname, search }`. Drop any hash. `search` retains its leading
  `?` (so it can be handed directly to `_parseQuery`).
- `_compileRoutePattern(pattern)` — accept a registration pattern, return
  `{ pattern, normalized, paramNames, regex, isWildcard }`.
  - `'*'` → `{ isWildcard: true, regex: /^.*$/, paramNames: [] }`.
  - Otherwise: normalize the pattern, split on `/`, escape literal
    characters, replace `:name` segments with `([^/]+)` and push `name` into
    `paramNames`. Anchor with `^` and `$`. `/` becomes `\/` after escape.
- `_matchAgainst(compiled, pathname)` — apply `compiled.regex` to
  `pathname`; on match build `{ [name]: decodeURIComponent(group) }` for
  each capture; return `{ params }` or `null`.
- `_matchRoutes(routeEntries, input)` — top-level matcher:
  1. `const { pathname, search } = _parsePathAndQuery(input);`
  2. `const normalizedPath = _normalizePath(pathname);`
  3. `const query = _parseQuery(search);`
  4. Iterate `routeEntries` in registration order; for each entry return
     `{ route, params, query, pathname: normalizedPath, search }` on first
     match. Return `null` when no route matches.
- Exports: `_normalizePath`, `_parseQuery`, `_parsePathAndQuery`,
  `_compileRoutePattern`, `_matchAgainst`, `_matchRoutes`. The navigation
  API (`navigate`, `back`, `forward`, `route`) is introduced in Step 5;
  this step adds no imports from `app.js`.

**Tests (in `runtime/router.test.js`):**
- `_normalizePath`: `/about/` → `/about`; `/` stays `/`; `/users/42/`
  → `/users/42`.
- `_parseQuery`: `''` → `{}`; `?a=1&b=2` → `{ a: '1', b: '2' }`;
  `?c=hello%20world` → `{ c: 'hello world' }`; duplicate keys: last wins;
  empty value → `''`.
- `_parsePathAndQuery`: `/about?x=1#y` → `{ pathname: '/about', search: '?x=1' }`.
- `_compileRoutePattern('/users/:id')` matches `/users/42` capturing
  `{ id: '42' }`; does not match `/users/42/posts`.
- Multi-param: `/users/:id/posts/:postId` captures both.
- `*` wildcard matches `/anything/here` with empty params.
- `_matchRoutes` honors registration order (specific route wins over `*`
  when registered first); wildcard catches the rest when registered last.
- Trailing-slash normalization: incoming `/about/` matches `/about`.
- Hash is dropped: `/about#section` matches `/about` and returns `search: ''`.
- Decoded params: `/users/%C3%A9` → `{ id: 'é' }`.

---

### Step 3: App class scaffolding and inject
**Goal:** Stand up the `App` instance and all its builders without yet
committing DOM. `inject` and the `_currentApp` holder become available so
later steps and tests can rely on the resolution mechanism.

**Files:**
- `runtime/app.js` (new)
- `runtime/app.test.js` (new)

**Changes:**
- **Module-level holder:**
  ```js
  let _currentApp = null;
  export function _getCurrentApp() { return _currentApp; }
  export function _setCurrentApp(app) { _currentApp = app; } // exported for tests + run()
  ```
- **Imports:**
  ```js
  import { signal, _createScope } from './reactivity.js';
  import { commit } from './template.js';
  import { _compileRoutePattern, _matchRoutes, _normalizePath } from './router.js';
  ```
  (`commit` and `_createScope` are unused until Step 4 but are wired now so
  the import list stops changing.)
- **`class App`:**
  - Constructor initializes:
    - `this._state = new Map();`
    - `this._routes = [];`
    - `this._layout = null;`
    - `this._pathSig = signal('');`
    - `this._paramsSig = signal({});`
    - `this._querySig = signal({});`
    - `this._mountEl = null;`
    - `this._routeScope = null;`
    - `this._running = false;`
  - Private guard `_assertNotRunning(method)`:
    `if (this._running) throw new Error(\`App.${method}() cannot be called after run()\`);`
  - `state(key, value)`:
    - `_assertNotRunning('state')`.
    - `if (this._state.has(key)) throw new Error(\`App.state: key "${key}" already registered\`);`
    - `this._state.set(key, value); return this;`
  - `layout(component)`:
    - `_assertNotRunning('layout')`.
    - `if (this._layout != null) throw new Error('App.layout: layout already set');`
    - `if (typeof component !== 'function') throw new Error('App.layout: component must be a function');`
    - `this._layout = component; return this;`
  - `route(pattern, loaderOrComponent)`:
    - `_assertNotRunning('route')`.
    - `if (typeof loaderOrComponent !== 'function') throw new Error('App.route: handler must be a function');`
    - `const normalized = _normalizePath(pattern);`
    - `this._routes.push({ pattern, normalized, compiled: _compileRoutePattern(normalized), loader: loaderOrComponent, resolvedComponent: null });`
    - `return this;`
  - `match(input)`:
    - Return `_matchRoutes(this._routes, input)` (or `null`).
  - `run(selector)` — stub in this step:
    `throw new Error('App.run not yet implemented (Step 4)')`. Reserves the
    method on the class so its shape is final.
  - `_getState(key)`:
    `if (!this._state.has(key)) throw new Error(\`inject: key "${key}" is not registered\`); return this._state.get(key);`
- **`export function inject(key)`:**
  ```js
  if (_currentApp == null) throw new Error('inject: no app is running');
  return _currentApp._getState(key);
  ```

**Tests (in `runtime/app.test.js`):**
- `new App()` does not throw and does not touch DOM/window globals (assert
  via no listener registration on `document`/`window`).
- Chaining returns `this`:
  `new App().state('a', 1).route('/', () => null)` returns an `App` instance.
- `state` duplicate-key throws; second `layout` call throws; `route` with a
  non-function throws.
- `app.match('/users/42')` against a registered `/users/:id` returns
  `{ params: { id: '42' }, query: {}, pathname: '/users/42', search: '', route }`.
- First-match wins under registration order; falls through to a trailing `*`.
- `inject` outside a running app throws `'no app is running'`.
- With `_setCurrentApp(app)` used as test-only injection, `inject('key')`
  returns the registered value; unknown key throws.

---

### Step 4: run() and full render lifecycle
**Goal:** Wire the App to the DOM. After this step, a developer (and tests)
can call `app.run('#app')` to mount, navigate via `popstate`, and have plain
`<a>` clicks intercepted and routed. The active-link attributes are also
applied here.

**Files:**
- `runtime/app.js`
- `runtime/app.test.js`

**Changes:**
- Replace the Step 3 stub `run(selector)` with:
  1. `if (this._running) throw new Error('App.run: already running');`
  2. `const el = document.querySelector(selector); if (!el) throw new Error(\`App.run: element not found for selector "${selector}"\`);`
  3. `this._mountEl = el; this._running = true; _setCurrentApp(this);`
  4. `const initialPath = window.location.pathname + window.location.search;`
  5. `this._navigateTo(initialPath);`
  6. Register `popstate` listener on `window`:
     `const onPopstate = () => this._navigateTo(window.location.pathname + window.location.search);`
     `window.addEventListener('popstate', onPopstate);`
     Store on `this._popstateListener` (used only for symmetry; no teardown
     in this spec).
  7. Register document-level click listener: `document.addEventListener('click', onClick)` where `onClick` is the bound `_onDocumentClick` described below. Store on `this._clickListener`.
- **`_navigateTo(input)`** — the navigation pipeline, used by initial render,
  `popstate`, and the future `navigate()`:
  1. `if (this._routeScope) { this._routeScope.dispose(); this._routeScope = null; }`
  2. `const m = _matchRoutes(this._routes, input);`
  3. Update reactive snapshot signals **regardless of match**:
     - If `m`: `this._pathSig.set(m.pathname); this._paramsSig.set(m.params); this._querySig.set(m.query);`
     - Else: parse `input` via `_parsePathAndQuery` + `_normalizePath` for
       `pathname`, `_parseQuery` for query, and set signals to those values
       with empty `params`.
  4. If `m == null`: clear the mount element's children synchronously and
     return. (No layout/no render.)
  5. `this._routeScope = _createScope();`
  6. Run the render inside the scope:
     ```js
     this._routeScope.run(async () => {
       const entry = m.route;
       let routeTR;
       if (entry.resolvedComponent != null) {
         routeTR = entry.resolvedComponent({ params: m.params, query: m.query });
       } else {
         const ret = entry.loader({ params: m.params, query: m.query });
         if (ret != null && typeof ret.then === 'function') {
           const mod = await ret;
           entry.resolvedComponent = mod.default;
           routeTR = entry.resolvedComponent({ params: m.params, query: m.query });
         } else {
           entry.resolvedComponent = entry.loader;
           routeTR = ret; // reuse first-call TR; do not double-invoke
         }
       }
       const mountTR = this._layout
         ? this._layout({ children: routeTR })
         : routeTR;
       _clearChildren(this._mountEl);
       commit(mountTR, this._mountEl);
       _applyActiveLinks(this._mountEl, m.pathname, m.search);
     });
     ```
  - Spec order honored: dispose old scope → match → update signals → clear +
    commit happens *after* any await, leaving prior content visible during a
    lazy load. The `_clearChildren` call is intentionally inside the scope
    callback (immediately before commit) per the spec note.
- **`_clearChildren(el)`** helper: while `el.childNodes.length > 0`,
  `el.removeChild(el.childNodes[0])`. (Effects/listeners were already
  detached by the prior scope's `dispose`; this just detaches the DOM nodes.)
- **`_applyActiveLinks(mountEl, currentPath, currentSearch)`:**
  - `const anchors = mountEl.querySelectorAll('a');`
  - For each anchor:
    - `const href = anchor.getAttribute('href');`
    - If `!href || href.startsWith('#')` → remove both attributes; continue.
    - Resolve to a same-origin `{ path, search }`:
      - If `href.startsWith('/')`: `path = href.split('?')[0]; search = '?' + (href.split('?')[1] || '')` (or `''` if no `?`).
      - Else if `href.startsWith(window.location.origin)`: strip origin, then split.
      - Else: not same-origin → remove both attributes; continue.
      - Normalize `path` with `_normalizePath`.
    - Compare:
      - Exact: `path === currentPath && search === currentSearch` →
        `setAttribute('data-active-exact', '')` + `setAttribute('data-active', '')`.
      - Prefix (segment-bounded): `currentPath === path || currentPath.startsWith(path + '/')` →
        `setAttribute('data-active', '')`, remove `data-active-exact`.
      - Otherwise: remove both attributes.
  - The walk is synchronous and re-runs on every navigation.
- **Document click interceptor** `_onDocumentClick(e)`:
  ```js
  function _onDocumentClick(e) {
    if (e.defaultPrevented) return;
    if (e.button != null && e.button !== 0) return;
    if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
    let anchor = e.target;
    while (anchor && anchor.tagName !== 'A') anchor = anchor.parentNode;
    if (!anchor) return;
    const target = anchor.getAttribute('target');
    if (target && target !== '_self') return;
    if (anchor.hasAttribute('download')) return;
    if (anchor.hasAttribute('data-external')) return;
    const href = anchor.getAttribute('href');
    if (!href) return;
    if (href.startsWith('#')) return;
    if (/^[a-z][a-z0-9+\-.]*:/i.test(href) && !href.startsWith(window.location.origin)) return;
    e.preventDefault();
    const stripped = href.startsWith(window.location.origin)
      ? href.slice(window.location.origin.length)
      : href;
    _navigateFromClick(stripped);
  }
  ```
- **`_navigateFromClick(input)`** — local helper that pushes state and
  triggers the pipeline against the current app:
  ```js
  function _navigateFromClick(input) {
    const app = _getCurrentApp();
    if (!app) return; // listener may fire post-teardown; defensive
    window.history.pushState(null, '', input);
    app._navigateTo(input);
  }
  ```
  (Step 5's exported `navigate` will use the same shape but accept opts.)
- **Lazy-loader race assumption** — if a second navigation begins while the
  first's import is still pending, the first scope is already disposed by
  step (1) of the new navigation, so the older commit lands into a disposed
  scope; its effects never observe updates. The render's commit may still
  write DOM into the mount before the newer commit replaces it; that DOM is
  cleared by the newer navigation's `_clearChildren`. Mentioned in **Risks**.

**Tests (in `runtime/app.test.js`):**
- Mount + initial render: `app.route('/', () => html\`<div>home</div>\`).run('#app')`
  writes `<div>home</div>` into `#app`. (Use a manually constructed mount
  element and stash its id; assert via `document.querySelector('#app')`.)
- Eager-component caching: register a route whose handler increments a
  counter and returns a TR. After two navigations to the same path, the
  counter has incremented exactly twice (once for detect+first-render
  reuse, once for second render). Note in the test that the detection
  reuses the first TR — no double call on first match.
- Lazy-loader caching: register `() => { loaderCalls++; return Promise.resolve({ default: Component }); }`.
  After first navigation (awaited), `loaderCalls === 1`. Navigate away then
  back; `loaderCalls === 1` (cache hit).
- Layout wrapping: with a layout set, the layout's `<main>` wraps the route's
  TR; without layout, the route's TR mounts directly.
- Route-change disposal: register a route component whose template contains
  a signal-bound attribute. Mount, change route, then update the signal —
  the abandoned DOM does not update (its scope was disposed).
- `popstate` reactivity: `window.history.pushState(null, '', '/about')`,
  then dispatch `popstate` on `window` — render lifecycle re-runs (with the
  current location) and renders the `/about` route.
- Click interception:
  - Plain `<a href="/about">` click navigates (history updated, mount shows
    the `/about` route).
  - Click on a `<span>` inside an anchor still navigates (ancestor walk).
  - `<a target="_blank">`, `<a download>`, `<a data-external>`, `<a href="https://example.com">`,
    and modified clicks (`metaKey: true`, `button: 1`) do **not** navigate
    and do not call `preventDefault`.
- `data-active` / `data-active-exact`:
  - Anchors matching the current path+query receive both attributes.
  - Anchors that are prefix matches (segment-bounded) receive only
    `data-active`.
  - Non-matching anchors have neither attribute (existing values removed on
    re-render).
- Double-run throws: `app.run('#x'); app.run('#x')` → error.
- Missing-selector throws: `app.run('#nope')` with no matching element →
  error.
- After `run()`, calling `state`/`layout`/`route` throws.

---

### Step 5: Navigation API and reactive route()
**Goal:** Complete the public surface — `navigate`, `back`, `forward`, and
the reactive `route()` snapshot — by hooking the router into the
currently running App via the `_getCurrentApp` getter.

**Files:**
- `runtime/router.js`
- `runtime/router.test.js`

**Changes (in `router.js`):**
- Add `import { _getCurrentApp } from './app.js';` at the top of the file.
  (Circular import is safe because the imported binding is only read inside
  functions called at runtime, not at module-init time.)
- `export function navigate(path, opts = {})`:
  ```js
  const app = _getCurrentApp();
  if (!app) throw new Error('navigate: no app is running');
  const state = opts.state ?? null;
  if (opts.replace) window.history.replaceState(state, '', path);
  else window.history.pushState(state, '', path);
  app._navigateTo(path);
  ```
- `export function back()`:
  ```js
  if (!_getCurrentApp()) throw new Error('back: no app is running');
  window.history.back();
  ```
  The `popstate` listener registered in Step 4 picks up the back navigation
  and drives the pipeline.
- `export function forward()` — symmetric to `back`.
- `export function route()`:
  ```js
  const app = _getCurrentApp();
  if (!app) throw new Error('route: no app is running');
  return {
    get path()   { return app._pathSig.val; },
    get params() { return app._paramsSig.val; },
    get query()  { return app._querySig.val; },
  };
  ```
  Each call returns a fresh proxy object; the underlying signals are
  shared, so reads from inside `effect`/`computed`/template reactive blocks
  subscribe correctly.

**Tests (in `runtime/router.test.js`):**
- Setup helper: a `freshApp()` function in the test file constructs an
  `App`, registers a `/` and `/about` route (each returning a tiny TR with
  the path text), mounts to a synthesized `#app` element, returns the app.
- `navigate('/about')` updates `window.history` (assert `length` and the
  top entry's URL) and triggers the render (`#app` text becomes `'about'`).
- `navigate('/about', { replace: true })` does not advance history length.
- `navigate('/about', { state: { from: 'x' } })` sets the history entry's
  state to the supplied object.
- `back()` after two pushes dispatches `popstate`; the App re-renders for
  the previous URL.
- `route()` outside a running app throws.
- `route()` is reactive: inside an `effect(() => last = route().path)`,
  after `navigate('/about')` the effect re-runs and `last === '/about'`.
- Two `route()` calls return distinct objects whose getters resolve to the
  same underlying values.

---

## Risks and Assumptions

- **Lazy-load races.** If a second navigation begins while the first's
  `await loader()` is still pending, the older scope is already disposed.
  Once the older promise resolves, its `commit` call runs inside a disposed
  scope: effects registered during commit never observe updates, but the
  DOM nodes may briefly land in the mount before the newer navigation's
  `_clearChildren` removes them. This spec does not require race
  cancellation. A future spec may add a per-navigation cancellation token;
  flagging now so reviewers can push back if they want it sooner.
- **dom-shim selector subset.** Only `#id` and lowercase tag selectors are
  supported. Real browsers handle the full grammar; production code is
  unaffected. If a future test reaches for `.class` or attribute selectors,
  the shim must be extended.
- **No bubbling in dom-shim.** Tests dispatch click events directly on
  `document` with `target` set to the inner-most element; the click
  interceptor walks up via `parentNode`. Production browsers bubble
  naturally — same logic applies.
- **`history` semantics.** The shim's stack-based history truncates the
  forward stack on `pushState` after `back()`. Tests assert this where
  relevant; production browsers behave the same.
- **Module-level `_currentApp`.** Each `new App().run()` overwrites the
  prior reference, so back-to-back tests do not interfere. If a future
  test asserts behavior *after* an app is "stopped", the spec does not
  define teardown — out of scope here.
- **Active-link walk is O(N).** Acceptable for pages with tens of links.
  Per-link reactive subscriptions are deferred to a follow-up if a hot
  spot emerges.
- **Eager component arity.** Components receive `{ params, query }`. The
  next spec adds `data`. Existing eager components written today remain
  shape-compatible because they accept a single props object.
- **Circular import** between `app.js` and `router.js`. Safe in practice:
  `app.js` imports only pure helpers from `router.js` (read at module
  load); `router.js` imports `_getCurrentApp` from `app.js` and reads it
  only inside function bodies. If a future refactor pulls those reads to
  top-level, TDZ errors may surface — call it out in the review.
