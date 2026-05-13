## Problem Statement

Phases 1 and 2 produced reactive primitives and a DOM template system. By themselves they render a single component into a container. To build real applications a developer needs an **application object** that owns app-level state, a **router** that maps URLs to components, and a **boot sequence** that mounts everything to the DOM and keeps it in sync with browser history.

This spec covers the **first half of Phase 3** — the `App` class plus a minimal-but-useful router. It is the smallest surface that lets a developer write a multi-page app: register some state, register some routes, call `app.run("#app")`, and have working in-app navigation. Subsequent work (middleware, route guards, nested routes, `load()`, loading/error UI, transitions, machine wiring) lands in a follow-up spec once this foundation is in place.

## Background

### What already exists

- `runtime/reactivity.js` exports `signal`, `computed`, `effect`, and `_createScope` (internal). Scopes nest, dispose recursively, and clean up effects registered while the scope is active via `scope.run(fn)`.
- `runtime/template.js` exports `html`, `commit`, `each`, `ref`. `commit(templateResult, container)` clones the cached template fragment and wires up dynamic parts, creating effects under whatever scope is active at call time. `commit` itself does **not** create a scope — callers do.
- `runtime/dom-shim.js` provides a minimal DOM in Node so tests run with `node:test` and zero npm deps. It side-effect-installs `globalThis.document` when no real DOM is present.

### How the App fits

Per the framework spec, the developer writes `index.html` and `src/app.ts`. The HTML's inline `<script type="module">` imports `app` and calls `app.run("#app")`. Constructing `new App()` is side-effect-free; only `run()` mounts to the DOM and starts listening to history. The app object is configured by chaining methods that register state and routes.

App-level state registered via `app.state(key, value)` is accessible from any component via `inject(key)`. This bridges the App instance to component code without prop drilling or a global singleton. `inject` is the only thing in this spec that requires an App-resolution mechanism reachable from any component code.

### How the Router fits

The router is **explicit**, not file-system-based. The developer registers routes via `app.route(path, loaderOrComponent)`. First match wins. The matched component is rendered, wrapped in the app's `layout` if one is registered. Mountpoint contents are owned by a **route scope** — when the route changes, the route scope is disposed (cleaning up all effects and event listeners committed during that route's render) and a new scope is created for the incoming route.

Browser history integration uses the standard `history.pushState`/`popstate` APIs. Same-origin `<a>` clicks are intercepted at the document level so plain HTML links navigate in-app instead of triggering a full page load.

### What is intentionally deferred

Phase 3 in the framework spec includes middleware, guards, nested routes with `children`, route groups, `load()` data fetching, loading/error UI, route transitions, and machine-to-machine wiring (`app.on()`). All of those are deferred to a second spec. They build on the surface this spec establishes and are large enough to deserve their own design pass.

## Requirements

### `App` class

Exported from the runtime (resolved via `from "z"` in user code; physically lives in the new file added by this spec, see **File layout**).

```js
new App()                     // construct — no side effects
app.state(key, value)         // register app-level state; returns app (chainable)
app.layout(component)         // set the layout component; returns app
app.route(path, loaderOrCmp)  // register a route; returns app
app.run(selector)             // mount and start (the only side-effecting call)
app.match(path)               // test helper: return { component?, params, query } | null
```

- `new App()` initializes empty internal collections (state map, route table, layout = null) and does nothing else. No DOM access, no history access, no global registration.
- All builder methods return `app` so `new App().state(...).route(...).run("#app")` works.
- Calling any builder method **after** `run()` is a programmer error — throw with a clear message.
- `run(selector)` is idempotent against double-call: throw if already running. The selector is resolved via `document.querySelector(selector)`. If the element is not found, throw.

### `app.state(key, value)`

- Stores `value` under `key` in an internal map on the App instance.
- `value` is stored as-is — no wrapping, no conversion. Signals stay signals, plain objects stay plain objects, future-machine instances will stay machine instances.
- Registering the same key twice throws — state registration is one-shot per key.

### `inject(key)`

- A module-level function exported from `"z"`.
- Returns the value previously registered under `key` on the **currently running App**.
- The "currently running App" is set when `app.run()` begins committing route content and cleared when no App is mounted. With one App per page (the supported case for this spec), this is unambiguous.
- If no App is running, or `key` is not registered, throw. (No silent `undefined` return — surfaces typos and ordering bugs.)
- Calling `inject` outside of an App-rendered code path (e.g., from a top-level module body before `run()`) throws.

### `app.layout(component)`

- Registers a single layout component. `component` is a plain function returning a `TemplateResult` (per the component model in the framework spec).
- When a route renders, the layout is called with `{ children }` where `children` is the matched route component's `TemplateResult`. The layout decides where to place `children` inside its own template.
- If no layout is registered, the matched route component is rendered directly into the mount element.
- Only one layout per App. Calling `layout()` twice throws.

### `app.route(path, loaderOrComponent)`

- Registers a route. The order of `route()` calls is the matching order — first match wins.
- `loaderOrComponent` may be:
  - A function returning a `TemplateResult` (eager component): treated as the route's component directly.
  - A function returning a Promise of a module (`() => import("./routes/home")`): treated as a lazy loader. On first match, await the import and read its `default` export as the route's component. The resolved component is **cached** on the route entry so subsequent matches do not re-import.
  - Distinguish "eager component" from "lazy loader" by calling the function and checking whether the return value is a thenable (`typeof val.then === 'function'`). Eager components return a `TemplateResult` synchronously; loaders return a Promise. (This means an eager component MUST return synchronously — it cannot be async. Async components belong in a `load()`, which is the second spec.)
- The route component is called with `{ params, query }`. `data` is reserved for the second spec (when `load()` lands) but is not passed in this spec.

### Path matching

Match rules, applied in registration order:

- **Exact paths**: `/`, `/about`. Match URL pathname literally after normalization.
- **Named params**: `/users/:id`, `/users/:id/posts/:postId`. Segment-bounded; `:name` consumes exactly one URL segment. Captured into `params` as decoded strings (`decodeURIComponent`).
- **Catch-all wildcard**: a bare `*` matches anything not previously matched. Multi-segment `*` patterns (e.g., `/files/*`) are out of scope — only the standalone `*` is supported.
- **Trailing-slash normalization**: incoming URL pathnames have a trailing `/` stripped before matching (except for the root `/`). Registered paths are likewise normalized at registration time. `/about` and `/about/` are equivalent.
- **Query**: the `?foo=bar&baz=1` portion is parsed into a `query` object with `decodeURIComponent` on both keys and values. Repeated keys overwrite (last wins). No nested-key parsing. Empty/missing query → `{}`.
- **Hash**: ignored by the router (the browser scrolls to `#id`; the router does not see it).

### `app.match(path)` — test helper

- Takes a full path-and-query string (e.g., `/users/42?tab=posts`).
- Returns `{ params, query, route }` on a match, or `null` if nothing matches.
- `route` is the internal route entry (path pattern, loader/component reference). For lazy loaders that have not yet resolved, the entry's resolved component will be `null` — that's fine for testing the matcher.
- Does **not** trigger loader resolution or mount anything. Pure, synchronous, side-effect-free.

### `app.run(selector)`

When called:

1. Resolve the mount element via `document.querySelector(selector)`. Throw if not found.
2. Register this App as the "currently running App" so `inject()` resolves correctly.
3. Match the current `window.location.pathname + window.location.search` against the route table.
4. Render the matched route (see **Render lifecycle**).
5. Attach a `popstate` listener on `window` so back/forward history navigation re-renders.
6. Attach a click listener on `document` for the `<a>`-tag interception (see **Link interception**).

### Render lifecycle

For each navigation (initial render or subsequent route change):

1. If there is an existing **route scope**, dispose it. Disposal cleans up all effects/listeners owned by the prior route render.
2. Create a fresh route scope via `_createScope()`.
3. Inside `routeScope.run(...)`:
   a. If the matched route is a lazy loader and has not resolved yet, await the import and store `module.default` as the component.
   b. Build the route component's `TemplateResult` by calling `Component({ params, query })`.
   c. If a layout is registered, build `Layout({ children: routeTemplateResult })`; the layout's `TemplateResult` is the one that gets mounted. Otherwise the route's `TemplateResult` is mounted directly.
   d. Clear the mount element's existing children, then `commit(templateResult, mountEl)`.
4. Update the reactive `route()` snapshot (see **Reactive `route()`**) with the new path/params/query.
5. Update `data-active` / `data-active-exact` attributes on all `<a>` tags inside the mount element (see **Active link attributes**).

Note: between **dispose old scope** and **commit new content**, lazy-loader awaiting may take arbitrary time. During the await, the mount element is left with the prior content visible until commit replaces it. Loading-UI handling lands in the second spec.

### `navigate(path, opts?)`, `back()`, `forward()`

Module-level functions exported from `"z"`:

- `navigate(path)` — `history.pushState(opts?.state ?? null, '', path)`; then run the navigation pipeline (match → render lifecycle) for the new path.
- `navigate(path, { replace: true })` — `history.replaceState(...)` instead of `pushState`.
- `navigate(path, { state })` — attaches arbitrary state to the history entry.
- `back()` — `history.back()` (popstate listener handles the resulting navigation).
- `forward()` — `history.forward()` (popstate listener handles it).

These functions operate on the **currently running App**. If no app is running, they throw.

### `<a>` tag interception (link interception)

A single click listener on `document` (registered in `app.run()`) intercepts left-button clicks on `<a>` elements:

- Skip if the click was modified (Ctrl, Cmd, Shift, Alt, middle button) — let the browser handle it.
- Skip if the anchor has `target="_blank"`, `download`, or any non-empty `target` other than `_self`.
- Skip if `href` is missing, starts with `#` (in-page anchor), or is a different origin.
- Skip if the anchor has `data-external` attribute (escape hatch).
- Otherwise: `event.preventDefault()`, extract `href`, call `navigate(href)`.

### `popstate` handling

Register a `popstate` listener on `window` during `run()`. On fire, re-match the current `location.pathname + location.search` and run the render lifecycle. Do **not** call `pushState` (the browser has already updated history).

### Reactive `route()`

A module-level function exported from `"z"`:

- `route()` returns an object whose property reads (`r.path`, `r.params`, `r.query`) subscribe to internal signals, making them reactive inside `computed`/`effect` and template reactive blocks.
- Backed by three signals (`_pathSig`, `_paramsSig`, `_querySig`) on the App instance, updated in the render lifecycle step 4.
- The returned object's getters call `.val` on the signals. Two calls to `route()` return distinct objects but observe the same underlying signals.
- If no app is running, throw.

### Active link attributes

After each render lifecycle:

- Find all `<a>` tags inside the mount element. For each:
  - Compare its `href` (resolved to a same-origin pathname) against the current path:
    - **Exact** match (path + query equal) → set `data-active-exact`. Also set `data-active`.
    - **Prefix** match (current path starts with the link's path, segment-bounded) → set `data-active`.
    - No match → remove both attributes.
- Re-applied on every navigation. Implementation may walk the element tree synchronously after `commit` returns.

### File layout

```
runtime/
  reactivity.js        # Phase 1 — complete
  template.js          # Phase 2 — complete
  dom-shim.js          # Phase 2 — complete
  app.js               # this spec — App class, inject, run, render lifecycle, link interception
  router.js            # this spec — path matching, query parsing, navigate/back/forward, route(), match()
  app.test.js          # this spec
  router.test.js       # this spec
```

Splitting App and Router into two files keeps the router's pure matching/parsing logic independently testable. `app.js` imports from `router.js`. Cross-file private state (the "currently running App") lives in a small shared module-level holder — keep it in `app.js` and have `router.js` import a setter/getter pair.

## Constraints

- Plain JavaScript with JSDoc — no TypeScript syntax, no build step. Matches Phases 1 and 2.
- No external dependencies. The DOM shim covers test-time needs; production targets real browser DOM.
- `inject`, `navigate`, `back`, `forward`, `route` are module-level functions that resolve against the **currently running App**. One App per page is the supported case for this spec.
- All effects/listeners created during a route's render must be owned by the route scope so disposal cleans them up. The route scope is the only scope this spec creates; it is a child of no parent scope (top-level for the app).
- Mount-element children are owned by `commit()`. On route change, dispose the old scope **first**, then clear the mount element, then commit new content. Disposing first ensures cleanup callbacks run before nodes are detached.
- `popstate` and the document-level click listener must be removed when there is no running app — but since `run()` is one-shot per app and the page lifetime contains at most one app, formal teardown is not required. (A future `app.stop()` may be added in a later phase; out of scope here.)
- Same-origin check for `<a>` interception compares against `window.location.origin`. Anchors with `href="/about"`, `href="./about"`, `href="https://samehost/about"` all qualify; anchors to other origins do not.

## Out of Scope

- Middleware (`app.use(...)`).
- Route guards.
- Nested routes (`children`) — layout in this spec is one-deep only.
- Route groups (`group(...)`).
- `load()` data fetching and the `data` prop on route components.
- Loading and error UI (`app.loading()`, `app.error()`).
- Route transitions (CSS classes during enter/leave).
- Route `meta` (including `meta.title` document-title sync).
- Machine-to-machine wiring (`app.on(stateKey, stateName, handler)`) — depends on Phase 4 machines.
- `app.testMiddleware()` — depends on middleware.
- Multi-segment wildcards (e.g., `/files/*` capturing the rest of the path).
- Multiple App instances on one page. The architecture leaves room for it (App is a class, no module-level App-state beyond the "currently running" holder) but the module-level `inject`/`navigate`/`route` exports assume a single mounted App.
- HMR integration (Phase 6).

## Open Questions

- **Click-interception scope**: should the document-level click listener walk up from `event.target` to find the nearest `<a>` ancestor (handles clicks on nested elements like `<a><span>...</span></a>`), or only fire when `event.target` is the anchor itself? The plan should specify the walk-up behavior; ancestor-walk is the expected web behavior.
- **`active-link` walk**: after each commit, walking every `<a>` in the mount element is O(N) per navigation. For the surfaces this spec targets this is fine; if it becomes a hot path the plan may swap in a per-link reactive subscription. Decide in the plan.
- **`inject`'s "currently running App" lookup mechanism**: a simple module-level variable inside `app.js` (set at the start of `run()`, never cleared) works for the one-App-per-page case. Confirm in the plan that no test isolation issue arises from this (each test should create a fresh App and the previous module-level reference is overwritten).
- **Lazy-loader Promise detection**: the spec proposes calling the user-supplied function and inspecting its return value to distinguish eager component from lazy loader. This means eager components MUST be synchronous and return a `TemplateResult` directly. Confirm this is the intended ergonomics — the alternative is a separate API (`app.route(path).lazy(...)` or a `lazy()` helper).
- **Route component arity**: the framework spec shows route components receiving `({ data, params, query })`. This spec omits `data`. Confirm whether the component should still receive a `data: undefined` slot for forward-compat, or just `{ params, query }`. Recommendation: only pass what's defined now to keep the contract honest; the second spec adds `data`.
