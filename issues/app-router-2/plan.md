# Plan: App + Router, Part 2 — Pipeline Features

## Summary

Extend `App` and the router with the full navigation pipeline: global middleware
(`app.use`), per-route guards, side-effectful `load()` for state hydration,
nested routes via `children` arrays rendered through a reactive `outlet`
prop, a 150ms-debounced loading UI, an error UI with `retry`, supersede-on-new-nav
via a monotonic nav token, and scope sharing across navigations driven by a
divergence point so chrome above the divergence point stays mounted. Layout is
committed once at `run()` and subsequent navigations swap a reactive root slot
instead of clearing the mount. All work lands in `runtime/app.js` and
`runtime/router.js` plus their existing test files — no new files.

## Prerequisites

None.

## Steps

- [x] **Step 1: Nested-route flattening with `chain` field**
- [x] **Step 2: Reactive root slot (commit-once layout) and `outlet` rename**
- [x] **Step 3: State proxy threaded into route components**
- [x] **Step 4: `app.use()` middleware + nav-token supersede + `redirect`**
- [x] **Step 5: Extended `route()` opts (`guard`, `load`, `meta`) + leaf pipeline**
- [x] **Step 6: `app.loading()` + 150ms loading timer**
- [x] **Step 7: `app.error()` + try/catch + `retry`**
- [x] **Step 8: Nested chain walk + outlets + scope sharing/divergence**
- [x] **Step 9: Per-route loading/error overrides + meta merge across chain**
- [x] **Step 10: Guard-returning-false URL rollback**

---

## Step Details

### Step 1: Nested-route flattening with `chain` field

**Goal:** Let `app.route(path, loader, { children: [...] })` register nested
routes. Each child path concatenates onto its parent's normalized path and is
flattened into `_routes` as a sibling entry that knows its full ancestor chain.
The matcher stays unchanged — it iterates `_routes` and returns the leaf entry,
which now carries `chain: [rootEntryDescriptor, ..., leafEntryDescriptor]`.
Pipeline still consumes only the leaf in this step.

**Files:**
- `runtime/router.js`
- `runtime/app.js`
- `runtime/router.test.js`

**Changes:**
- `runtime/router.js`: add and export `_joinPaths(parent, child)`. Rules:
  - `child === '/'` → result is `parent` (exact-parent-match).
  - `parent === '/'` → result is `child` (avoid leading double-slash).
  - Else → `parent + child` (children always begin with `/`).
  - Apply `_normalizePath` to the result.
- `runtime/app.js`:
  - Change `route(pattern, loaderOrComponent, opts = {})`. Validate
    `loaderOrComponent` as a function (unchanged) and `opts.children` (if
    present) as an array.
  - Introduce an internal `_buildEntryDescriptor({ pattern, normalized, loaderOrLoad, opts })`
    helper returning an object `{ pattern, normalized, loaderOrLoad, opts }`
    (the descriptor; not yet a compiled route). Descriptors are the building
    blocks of the chain.
  - Recursively flatten:
    - For the top-level call, build the parent descriptor from the positional
      `loaderOrComponent` and `opts` (minus `children`).
    - If `opts.children` is absent → push one entry into `_routes`:
      `{ pattern: parent.normalized, normalized, compiled, loader: loaderOrComponent,
      opts: parentOpts, resolvedComponent: null, chain: [parentDescriptor] }`.
    - If `opts.children` is present → for each child `{ path, load, guard?,
      loading?, error?, meta?, children? }`: build the child descriptor with
      `loaderOrLoad = child.load`, compute the joined normalized path via
      `_joinPaths(parent.normalized, child.path)`, and recurse with chain
      `[parent, child]` appended. Each child must declare `load` as a function
      (the loader-or-component, same eager/lazy detection as the top-level
      positional arg).
  - The parent descriptor is the **same object** across every flattened entry
    descended from it. This is what makes divergence-by-reference work in
    Step 8. Achieve this by capturing the parent descriptor once and passing
    it down through the recursion.
  - On each flattened `_routes` entry, store `chain` root-first, leaf-last.
    `chain[chain.length - 1]` is the entry's own descriptor.
- `runtime/router.test.js`: import `_joinPaths`.

**Tests (router.test.js):**
- `_joinPaths`: `('/dashboard', '/')` → `/dashboard`; `('/dashboard', '/analytics')`
  → `/dashboard/analytics`; `('/', '/about')` → `/about`; `('/', '/')` → `/`;
  trailing-slash on parent normalised before join.
- Flatten one-level: `app.route('/dashboard', P, { children: [{ path: '/', load: O },
  { path: '/analytics', load: A }] })` produces two `_routes` entries with
  normalized paths `/dashboard` and `/dashboard/analytics`. Both have
  `chain.length === 2`, both `chain[0] === chain[0]` by reference, and
  `chain[1]` of the first uses `O`, of the second uses `A`.
- Sibling parent-descriptor reuse: the parent descriptor in both children's
  chains is identical by reference (`===`).
- Two-level nesting: `{ children: [{ path: '/foo', children: [{ path: '/bar',
  load: Leaf }] }] }` produces an entry at `/dashboard/foo/bar` with
  `chain.length === 3`.
- Plain top-level route (no `children`): entry has `chain: [self]`.

Existing tests continue to pass because `_matchRoutes` and the pipeline still
operate on `_routes` as a flat list and the leaf entry is what's returned.

---

### Step 2: Reactive root slot (commit-once layout) and `outlet` rename

**Goal:** Commit the layout (or a naked wrapper) once at `run()` time and
let navigations swap a reactive root-slot signal instead of clearing and
re-committing the mount. Rename the layout's prop from `children` to `outlet`
for consistency with nested-route parents.

**Files:**
- `runtime/app.js`
- `runtime/app.test.js`

**Changes:**
- Constructor adds:
  - `this._rootSlotSig = signal(null)` — the reactive container the router
    writes into.
  - `this._rootScope = null` — owns the layout/wrapper's effects.
- `run(selector)`:
  - After resolving `_mountEl`, create `this._rootScope = _createScope()`.
  - Inside `this._rootScope.run(() => { ... })`:
    - If `this._layout` is set, invoke `this._layout({ outlet: this._rootSlotSig })`
      and `commit(layoutTR, this._mountEl)`.
    - Else build a one-shot wrapper via ``html`${this._rootSlotSig}`\`\`
      and commit it. This is a template that just reads the signal and renders
      whatever TR it contains.
  - Then call `_navigateTo(initialPath)` as before.
- `_navigateTo`:
  - Stop calling `_clearChildren(this._mountEl)` on every nav.
  - When committing the new route TR, set `this._rootSlotSig.set(newTR)`
    instead. The signal-driven effect inside the layout/wrapper's outlet
    block clears prior content and commits the new TR automatically (see
    `template.js`: `_isReactive(value)` → `effect(() => _applyNodeValueLeaf(...))`).
  - For the no-match case, `_rootSlotSig.set(null)` — the outlet effect
    renders nothing.
- Update `app.layout()` JSDoc: prop is `outlet`, not `children`.

**Slot effect ownership.** When `_rootSlotSig.set(newTR)` re-runs the layout's
`${outlet}` effect, the new effects spawned by the inner `commit(newTR)`
register to whatever `_activeScope` is on the stack at the call site. To
keep ownership correct, every `_rootSlotSig.set(...)` from `_navigateTo`
must be wrapped in the new route's scope `run`. Step 5 onward already
constructs a per-nav scope and a per-entry scope; for Step 2 (still
single-level), wrap the set in a fresh nav scope.

**Tests (app.test.js):**
- Update existing test "layout wraps route content; without layout, route
  renders directly" to use `({ outlet }) => html`<main>${outlet}</main>`\`.
- Add: layout component is invoked exactly once across multiple navigations
  (call-count assertion).
- Add: with no layout, content still renders inside the mount across
  navigations.

Existing "double commit" eager-component test continues to pass because the
leaf component is still invoked on each nav; only the layout is now once.

---

### Step 3: State proxy threaded into route components

**Goal:** Pass `state` to route component invocations. `state.<key>` returns
the value registered via `app.state(<key>, value)`. This lets components read
state slices without `inject()`.

**Files:**
- `runtime/app.js`
- `runtime/app.test.js`

**Changes:**
- Constructor: lazily build `this._stateProxy` (or eagerly — both fine) as a
  `Proxy({}, { get: (_, key) => this._state.get(key) })`. The same proxy is
  later passed to middleware, guard, and load.
- In `_navigateTo`'s component-invocation site, change all
  `entry.loader({ params, query })` and `entry.resolvedComponent({ params, query })`
  calls to also pass `state: this._stateProxy`. Same for the eager-detection
  invocation (`entry.loader({ params, query, state })`).
- The layout component does **not** receive `state` (it already received
  `outlet` in Step 2). It's invoked only at `run()` and is not part of
  per-nav data.

**Tests:**
- A route component receives `{ params, query, state }`; reading
  `state.foo` returns the signal registered as `app.state('foo', signal(...))`.
- A component that doesn't destructure `state` still works (purely additive).

---

### Step 4: `app.use()` middleware + nav-token supersede + `redirect`

**Goal:** Register an ordered middleware chain that runs once per navigation,
before any chain entry is resolved. Introduce `this._navToken` and check
after every `await`. Provide `redirect(path, opts?)` to short-circuit a nav.

**Files:**
- `runtime/app.js`
- `runtime/app.test.js`

**Changes:**
- Constructor:
  - `this._middleware = []`
  - `this._navToken = 0`
- `use(mw)`:
  - `_assertNotRunning('use')`; validate function; push; return `this`.
- `_navigateTo(input)`:
  - Becomes `async`. First line: `const token = ++this._navToken;`.
  - After computing `match`, run the middleware chain:
    ```js
    for (const mw of this._middleware) {
      let didRedirect = false;
      const redirect = (path, opts = {}) => {
        didRedirect = true;
        this._navToken++; // invalidate this in-flight nav
        _routerNavigate(path, { replace: true, ...opts });
      };
      await mw({
        route: { path: match.pathname, params: match.params, query: match.query, meta: {} },
        state: this._stateProxy,
        redirect,
      });
      if (token !== this._navToken) return;
      if (didRedirect) return;
    }
    ```
  - `meta` is `{}` in Step 4; the chain-merged value lands in Step 9.
  - For every subsequent `await` in `_navigateTo` (component resolve, load,
    etc., already in code or coming in Step 5), insert
    `if (token !== this._navToken) return;` immediately after.
- Circular import: `app.js` needs `navigate` from `router.js`, which already
  imports from `app.js`. Use a top-of-file `import { navigate as _routerNavigate }
  from './router.js';`. ESM allows this for function bindings consumed at call
  time. If ordering breaks at runtime, fall back to inlining the
  `pushState/replaceState + _navigateTo` two-liner directly (no behavior
  difference).

**Tests:**
- `use()` after `run()` throws (`/cannot be called after run/`).
- Two `use()` calls execute in registration order on a navigation.
- Middleware can read and mutate state (e.g. set a signal); subsequent
  middleware sees the new value.
- Async middleware: await a deferred promise; the route content commits only
  after the middleware resolves.
- `redirect('/login')` from middleware: `window.location.pathname` is
  `/login`; the original target's component never runs (call-count assertion).
- Supersede: kick off a navigation with a slow middleware, then immediately
  call `navigate(/second)`. Only `/second`'s route content commits.

---

### Step 5: Extended `route()` opts + leaf guard / load / lazy import

**Goal:** Add `guard`, `load`, `meta` opts to `app.route`. Pipeline runs
the leaf entry's `guard` then resolves its component then runs its `load()`.
Ancestor chain entries are still ignored in this step — Step 8 generalizes
to the full chain walk. This step keeps the single-level pipeline complete
and tested.

**Files:**
- `runtime/app.js`
- `runtime/app.test.js`

**Changes:**
- `route(pattern, loaderOrComponent, opts = {})`: pass `opts` through to
  the descriptor built in Step 1. Validate types where present:
  `guard?: function`, `load?: function`, `meta?: object`.
- Pipeline (after middleware):
  - Build `mergedMeta` placeholder (`= {}` for single-level; Step 9 generalizes).
  - `const leaf = match.route.chain[match.route.chain.length - 1];`
  - If `leaf.opts.guard`:
    - `const redirect = (path, opts) => { this._navToken++; _routerNavigate(path, { replace: true, ...opts }); };`
    - `const r = await leaf.opts.guard({ params, query, state, route, redirect });`
    - `if (token !== this._navToken) return;`
    - If `r === false`, abort (no commit). (URL rollback lands in Step 10.)
  - Resolve the component (existing eager/lazy logic, unchanged). After the
    lazy `await`, supersede check.
  - If `leaf.opts.load`:
    - `await leaf.opts.load({ params, query, state, fetch: globalThis.fetch.bind(globalThis), route });`
    - Discard the return value. Supersede check after the await.
  - Commit the route TR (set the root slot) as in Step 2.

**Tests:**
- `route('/x', X, { guard: x })` with non-function `guard` throws.
- Guard returning `false` cancels nav: prior content unchanged, no commit.
- Guard returning `true` (or `undefined`) proceeds.
- Guard calling `redirect('/login')` → URL is `/login`; target component
  never invoked.
- `load()` side-effect: registered signal is `null` before load, then the
  registered value after; the rendered component reads the new value.
- `load()` returning a slow promise delays the commit until resolve.
- `load()`-stubbed `fetch`: monkey-patch `globalThis.fetch` for the test;
  the load reads it.
- `load()` whose return value is non-null: router ignores it (no `data`
  prop on component).

---

### Step 6: `app.loading()` + 150ms loading timer

**Goal:** Register a global loading UI; show it only if the pipeline hasn't
completed within 150ms; cancel on supersede; replace on success/error.

**Files:**
- `runtime/app.js`
- `runtime/app.test.js`

**Changes:**
- Constructor: `this._loading = null; this._navScope = null;`.
- `loading(comp)`:
  - `_assertNotRunning('loading')`.
  - Throw if `this._loading != null` (`'App.loading: loading already set'`).
  - Validate function. Store. Return `this`.
- `_navigateTo`:
  - At the top (after token bump, before match), dispose any prior in-flight
    nav scope: `if (this._navScope) { this._navScope.dispose(); this._navScope = null; }`.
    Then create a fresh one: `this._navScope = _createScope();`.
  - After match (before middleware), start the loading timer:
    ```js
    const loadingTimer = setTimeout(() => {
      if (token !== this._navToken) return;
      if (!this._loading) return;
      this._navScope.run(() => this._rootSlotSig.set(this._loading()));
    }, 150);
    ```
  - On every early return (supersede, redirect, guard-false, error) and on
    success, `clearTimeout(loadingTimer)`.
  - Wrap `_rootSlotSig.set(newTR)` on success in the nav scope so that
    transient loading effects are disposed when the next nav disposes the
    scope (Step 8 will introduce per-entry scopes; for now the nav scope is
    fine).

**Tests:**
- Fast nav (no load(), eager component): loading UI never appears. Assert
  the loading component's call count is 0 after `await Promise.resolve()`.
- Slow nav (load() awaits a 200ms promise): loading UI is rendered after
  150ms; then replaced by route content once load resolves. Use
  `await new Promise(r => setTimeout(r, 200))` style waits.
- Supersede cancels the loading timer of the prior nav.

---

### Step 7: `app.error()` + try/catch + `retry`

**Goal:** Catch any throw from middleware, guard, lazy-import, or `load()`.
Cancel the loading timer. Render the error component into the root slot with
`{ error, retry }`. If no error component is registered, log via
`console.error` and leave prior content on screen.

**Files:**
- `runtime/app.js`
- `runtime/app.test.js`

**Changes:**
- Constructor: `this._error = null;`.
- `error(comp)`:
  - `_assertNotRunning('error')`.
  - Throw if `this._error != null`.
  - Validate function. Store. Return `this`.
- `_navigateTo`: wrap the body from "after match" through commit in
  `try { ... } catch (err) { ... }`:
  ```js
  catch (err) {
    if (token !== this._navToken) return;
    clearTimeout(loadingTimer);
    if (this._error) {
      // Fresh scope so retry-created effects dispose cleanly on next nav.
      this._navScope.dispose();
      this._navScope = _createScope();
      const retry = () => this._navigateTo(input);
      this._navScope.run(() => {
        this._rootSlotSig.set(this._error({ error: err, retry }));
      });
    } else {
      console.error('navigation error', err);
    }
  }
  ```
- Make sure throwing `guard`, throwing `load`, throwing middleware, and
  throwing lazy-import (rejected promise from `entry.loader()`) all flow
  through this catch.

**Tests:**
- Throw in middleware → error UI rendered, `error` arg matches.
- Throw in guard → error UI rendered.
- Throw in load() (`throw new Error('boom')` or rejected promise) → error UI
  rendered.
- No error registered + a throw → `console.error` called once (spy via
  patching `console.error` before the test, restore after); prior content
  stays on screen.
- `retry()` reinvokes the pipeline. Use a counter: first call throws,
  second succeeds. Assert success content renders after `retry()` is
  invoked.
- A new nav started during an error-rendered state replaces the error UI
  with the new route's content.

---

### Step 8: Nested chain walk + outlets + scope sharing/divergence

**Goal:** Run guard/resolve/load for every chain entry; build the parent →
child TR chain with each parent receiving a reactive `outlet` signal whose
value is the next-deeper TR; compute the divergence point against the
previously committed chain so ancestor scopes above the divergence point are
preserved; place loading/error UI at the divergence point's parent slot
(not always the root).

**Files:**
- `runtime/app.js`
- `runtime/app.test.js`

**Changes:**
- App-instance state:
  - `this._chain = []` — current committed chain root → leaf: each item
    `{ descriptor, scope, outletSig }`. `outletSig` is `null` for the leaf.
  - Drop the per-nav `_navScope` from Step 6 once per-entry scopes own
    everything. The loading/error UI gets its own scope (a "transient slot
    scope") for the duration the slot holds that content.
- Helpers (private):
  - `_computeDivergence(oldChain, newChainDescriptors)`: walk `i = 0` upward
    while `i < min(oldLen, newLen) && oldChain[i].descriptor === newChainDescriptors[i]`;
    return `i`. If `oldChain` is empty, returns `0`.
  - `_slotAt(divergeAt)`: returns `this._rootSlotSig` when `divergeAt === 0`,
    otherwise `this._chain[divergeAt - 1].outletSig`.
- Pipeline (replaces Step 5/6/7's leaf-only walk and root-only commit):
  - After middleware, build `newChainDescriptors = match.route.chain;`
  - `const divergeAt = this._computeDivergence(this._chain, newChainDescriptors);`
  - `const parentSlot = this._slotAt(divergeAt);`
  - Loading timer now writes into `parentSlot` (not always the root):
    `parentSlot.set(loadingTR)`. The loading-component resolution falls back
    to `this._loading` for Step 8; per-route loading overrides land in Step 9.
  - **Walk root → leaf for entries at or below `divergeAt`**, running:
    - If `desc.opts.guard`, run guard (supersede check after each await,
      handle `false`, `redirect`, throw as before).
    - Resolve the descriptor's component (eager/lazy detection identical to
      Part 1, except the `loaderOrLoad` field is used instead of `loader`).
      Cache `desc.resolvedComponent` on the descriptor.
    - If `desc.opts.load`, await it. Supersede check.
  - Cancel the loading timer (it either never fired or will be overwritten).
  - **Dispose old chain entries at or below `divergeAt`, leaf-first:**
    ```js
    for (let i = this._chain.length - 1; i >= divergeAt; i--) {
      this._chain[i].scope.dispose();
    }
    this._chain.length = divergeAt;
    ```
  - **Build new chain entries leaf → divergeAt** (so each parent has its
    child TR in hand when its `outlet` signal is initialised):
    ```js
    let childTR = undefined;
    const newEntries = []; // collected leaf-first; reversed before append
    for (let i = newChainDescriptors.length - 1; i >= divergeAt; i--) {
      const desc = newChainDescriptors[i];
      const parentScope = i === divergeAt
        ? (divergeAt === 0 ? this._rootScope : this._chain[divergeAt - 1].scope)
        : null; // children parented to the entry above (filled below)
      const scope = _createScope(); // parented by current _activeScope
      // To parent to a specific scope, run inside that scope's run():
      let entryRecord;
      const ownerScope = parentScope ?? newEntries[0].scope; // newEntries[0] is the just-built child
      ownerScope.run(() => {
        const childScope = _createScope();
        const outletSig = i < newChainDescriptors.length - 1 ? signal(childTR) : null;
        childScope.run(() => {
          const tr = desc.resolvedComponent({
            params: match.params,
            query: match.query,
            state: this._stateProxy,
            outlet: outletSig ?? undefined,
          });
          entryRecord = { descriptor: desc, scope: childScope, outletSig };
          childTR = tr;
        });
      });
      newEntries.unshift(entryRecord);
    }
    // Append into _chain
    for (const rec of newEntries) this._chain.push(rec);
    ```
    Notes:
    - The scope-parenting recipe above relies on `_createScope()` capturing
      `_activeScope` at creation time, which `reactivity.js` already does
      (`createScope()` reads `_activeScope` into `_parentScope`).
    - The leaf has no `outletSig`. Parents pass their own `outletSig` as the
      `outlet` prop.
    - The component's reading of `${outlet}` in its template subscribes to
      the signal via the template system's existing reactive-block handling
      (no change needed in `template.js`).
  - **Swap the divergence point's parent slot to the new top-of-new-entries
    TR**:
    ```js
    const topEntry = this._chain[divergeAt]; // first new entry
    topEntry.scope.run(() => parentSlot.set(childTR));
    ```
    Wrapping in `topEntry.scope.run` ensures the parent's outlet-effect
    re-run (which calls `commit(childTR)`) registers its commit-spawned
    effects to the new top entry's scope. The deeper entries' effects were
    already attached to their own scopes during the component invocations
    above.
  - Update reactive route signals (`_pathSig`, `_paramsSig`, `_querySig`)
    and run `_applyActiveLinks(this._mountEl, match.pathname, match.search)`
    as before.
  - On error, the catch block computes `divergeAt` (it was computed at the
    start of the pipeline and is in scope) and writes the error UI into
    `_slotAt(divergeAt)` instead of always into `_rootSlotSig`. Track the
    error-UI scope on `this._chain[divergeAt] = { descriptor: null, scope:
    errScope, outletSig: null }` so the next nav's tear-down loop picks it up.

**Tests (app.test.js):**
- Two-level: `app.route('/dashboard', Parent, { children: [{ path: '/analytics',
  load: Analytics }] })`. Navigating to `/dashboard/analytics` renders the
  parent's DOM plus the child's DOM inside `${outlet}`.
- Sub-nav preserves parent: nav `/dashboard/overview` → `/dashboard/analytics`.
  Track a `parentRenderCount` and a `parentDomRef` (DOM element from first
  render); after sub-nav, `parentRenderCount` is still 1 and the parent's
  root DOM node is the same reference.
- Param-only nav (same leaf, different `:id`): leaf's load() re-runs;
  leaf's component re-invoked; ancestor scopes preserved.
- Three-level nesting works end-to-end.
- Each level's `load()` writes to `state`; descendants see the value.
- Guard `false` at a child entry: nav aborted, prior chain on screen,
  no DOM change.
- Loading UI at divergence point: with a slow leaf load(), the parent stays
  mounted and the loading TR appears inside the parent's outlet (assert by
  looking up the loading element via `parentDom.querySelector(...)`).
- Error UI at divergence point: leaf load() throws → error UI renders inside
  the parent's outlet, parent DOM is the same reference as before.

---

### Step 9: Per-route loading/error overrides + meta merge across chain

**Goal:** Per-entry `loading` / `error` opts beat the global ones for that
entry and its descendants (resolved by walking from `divergeAt` down to the
leaf). Middleware receives a `meta` shallow-merged across the chain (root →
leaf, child wins on collisions).

**Files:**
- `runtime/app.js`
- `runtime/app.test.js`

**Changes:**
- Validation in `route()`: when `opts.loading` / `opts.error` present, validate
  function.
- `_resolveLoadingFor(chainDescriptors, divergeAt)`: walk `i in [divergeAt, leaf]`,
  return the first `desc.opts.loading` if set; else `this._loading`; else `null`.
  Same shape for `_resolveErrorFor`.
- `_mergeMeta(chainDescriptors)`: `chainDescriptors.reduce((m, d) =>
  Object.assign({}, m, d.opts.meta || {}), {})`.
- Loading timer now uses `_resolveLoadingFor(newChainDescriptors, divergeAt)`
  to choose the component to render. If `null`, the timer fires but does
  nothing (no slot.set), matching the spec's "loading UI is simply not
  rendered".
- Error catch uses `_resolveErrorFor(newChainDescriptors, divergeAt)`. If
  `null`, fall back to `console.error` as in Step 7.
- Middleware sees `route.meta` set to `_mergeMeta(newChainDescriptors)`.

**Tests:**
- Per-route `loading` override: a slow load on a child whose parent registers
  `loading` renders the parent's loading TR (not the global one).
- Per-route `error` override behaves the same.
- Override applies to descendants: `loading` on the parent, child has none,
  child loads slowly → parent's loading shows.
- Meta merge: parent `{ a: 1, b: 2 }`, child `{ b: 3, c: 4 }` → middleware
  sees `{ a: 1, b: 3, c: 4 }`.
- Global `app.loading` is used as fallback when no entry in the chain
  overrides.

---

### Step 10: Guard-returning-false URL rollback

**Goal:** When a guard returns `false`, replace the address-bar URL with the
last successfully committed URL so the address bar and the rendered route
agree (spec Open Questions, recommendation).

**Files:**
- `runtime/app.js`
- `runtime/app.test.js`

**Changes:**
- Constructor: `this._lastCommittedUrl = null`.
- At the end of a successful nav (after `_applyActiveLinks`), set
  `this._lastCommittedUrl = match.pathname + match.search`.
- In the guard-false branch (any chain entry), before returning:
  `if (this._lastCommittedUrl != null) window.history.replaceState(null, '', this._lastCommittedUrl);`
- Initial-nav guard-false (no prior commit): leave the URL as-is.

**Tests:**
- Commit `/`, then `navigate('/admin')` where the admin guard returns false:
  `window.location.pathname` rolls back to `/`.
- Initial nav with a guard that returns false: `window.location.pathname`
  stays at the initial input (no rollback).

---

## Risks and Assumptions

- **Slot effect ownership.** The plan assumes `_activeScope` at the call
  site of `slot.set(...)` is the scope effects spawned by the resulting
  commit should belong to. This matches `reactivity.js`'s contract:
  `_run()` does not touch `_activeScope`, so effects created during a
  re-run register to whatever scope is on the stack at the trigger site.
  Every `slot.set(newTR)` is wrapped in the owning entry's `scope.run`.
  If a test reveals a leak (e.g. an effect fires after its scope was
  disposed), this is where to look.
- **Reactive-slot via signal-of-TemplateResult.** The template system's
  `_isReactive(value)` path handles `signal(TemplateResult)` by mounting
  the TR and replacing it via `_clearNodeContent` + `_appendNodeItem` on
  `.set`. The plan does not add new template.js code. If Step 2's tests
  reveal that swapping a TR through a signal leaks DOM or effects, that's
  a template.js gap that must be patched before proceeding; the plan
  treats this as a confirmed-by-construction assumption (Open Question in
  the spec).
- **Circular import `app.js` ↔ `router.js`.** `_navigateTo` imports
  `navigate` from `router.js`, which already imports from `app.js`. ESM
  handles this for function-binding consumers at call time. If the
  Node test runner errors at import time, fall back to inlining the
  two-line equivalent (`window.history.replaceState(...)` /
  `pushState(...)`, then `app._navigateTo(...)`) directly in `redirect`.
- **`globalThis.fetch` in tests.** Node 18+ ships `fetch`. Tests that
  stub it must restore the original after.
- **Timer cleanup paths.** The loading timer must be cancelled on every
  early return: supersede, guard-false, redirect, error, and success.
  Each path in `_navigateTo` must `clearTimeout(loadingTimer)`. A Step 6
  test covers the supersede case; Step 7 covers error.
- **Stale `_chain` during a race.** The plan only assigns to
  `this._chain` after the slot.set commits, keeping in-progress chain
  state in `_navigateTo` locals. If two navs race, the second sees the
  first's last fully-committed chain when computing divergence, not the
  first's partial state.
- **Layout failure during `run()` is fatal.** Spec calls this out — the
  initial layout commit isn't wrapped in the error catch. If the layout
  function throws, the throw escapes `run()`. No special handling in this
  plan.
- **`${outlet}` missing in a parent template.** Spec Open Question
  recommendation (c) — dev-mode `console.warn` — is **deferred** in this
  plan. If silent footguns become a problem in practice, add the warn
  in a follow-up. Not scoped here.
- **`AbortSignal` plumbing.** Not implemented. Loaders cannot cancel
  in-flight `fetch` on supersede; the new nav simply ignores the old
  result via the token check. Acceptable per spec.
- **`app.layout()` semantics unchanged.** The plan keeps `app.layout`
  as a separate concept (not converted to a zero-path route with
  children) per spec recommendation.
