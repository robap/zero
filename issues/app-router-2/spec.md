# Spec: App + Router, Part 2 — Pipeline Features

## Problem Statement

The first half of Phase 3 shipped the `App` class, basic path matching,
`<a>`-click interception, `popstate` handling, and a reactive `route()`.
That gives a developer enough to render different components at different
URLs, but the navigation pipeline is currently a single bullet:
*match → render*. Real apps need to **gate** navigations (is the user
allowed here?), **fetch** data before the component renders, **show
feedback** while that fetch is in flight, **recover** when something goes
wrong, and **share chrome** across families of routes without unmounting
it on every sub-nav.

This spec covers the remaining Phase 3 checklist:

- `load()` data-hydration per route — a side-effectful step that
  populates `app.state` slices before the route component renders.
  Components consume data through `state`, not through a returned
  `data` prop. This is a deliberate departure from the framework
  spec's draft `data`-prop shape; see **Data loading model** below
  for the rationale.
- Global middleware chain (`app.use`).
- Per-route guards.
- Nested routes via a `children` array; matched child renders into
  the parent component's `outlet` prop.
- Loading and error UI (`app.loading`, `app.error`) with a delay-based
  loading display and a `retry` mechanism for errors.
- Route components receive `state` as a prop alongside `params`,
  `query`, and `outlet`. `inject` is retained for *non-route*
  components that aren't called by the router.

After this spec, Phase 3 in the framework spec is feature-complete except
for route transitions, `meta` (including `meta.title` document-title
sync), `app.on()` machine-to-machine wiring (depends on Phase 4), and
`app.testMiddleware()` — those move to subsequent specs.

## Background

### What already exists

- `runtime/app.js` exports the `App` class plus `inject` and the
  `_getCurrentApp` / `_setCurrentApp` holder. The current
  `_navigateTo(input)` method does: dispose-old-scope → match → update
  reactive signals → fresh route scope → call component (or await lazy
  loader) → optional layout wrap → clear mount → commit → apply active
  link attributes.
- `runtime/router.js` exports `navigate`, `back`, `forward`, reactive
  `route()`, plus internal `_compileRoutePattern`, `_matchRoutes`,
  `_normalizePath`, `_parsePathAndQuery`, `_parseQuery`.
- `runtime/reactivity.js` provides `_createScope()` with nested-scope
  disposal — child scopes dispose with their parent. This spec leans on
  that to give each nesting level of routes its own scope.
- `runtime/template.js` `commit(templateResult, container)` clones the
  cached template fragment and wires up parts inside whatever scope is
  active.

### Data loading model

Where does fetched data live, and how do components read it?

**The framework spec's draft shape:** `load()` returns a value, the
router passes it as a `data` prop, the component reads `props.data`.
Simple for one-shot route data. **Breaks down on shared data**: if
two route components (or a route component and a non-route
sub-component like a Header) both need the current user, each
route's `load()` would have to fetch it independently and the
components have no way to share. Frameworks address this with a
query cache / resource primitive, which is a substantial design
surface unto itself.

**This spec's model:** drop the `data` prop entirely. Shared and
route-scoped data both live in `app.state` slices. `load()` is a
**side-effectful** hydration step whose only job is to ensure the
slices the route needs are populated by the time the component
renders.

```js
// src/app.ts
app.state('user', signal(null))
app.state('teams', signal(null))

// src/routes/dashboard.ts
export async function load({ state, fetch }) {
  if (state.user.val == null) {
    state.user.set(await (await fetch('/api/me')).json())
  }
  if (state.teams.val == null) {
    state.teams.set(await (await fetch('/api/teams')).json())
  }
}

export default function Dashboard({ state, outlet }) {
  return html`
    <h1>Welcome ${() => state.user.val.name}</h1>
    <nav>${() => state.teams.val.map(t => html`<a href=${`/t/${t.id}`}>${t.name}</a>`)}</nav>
    ${outlet}
  `
}
```

Properties of this model:

1. **Dedup is structural.** Two components reading
   `state.user.val` share the same signal. Whichever route's `load()`
   populated it first wins; subsequent loads are skipped by the
   guard check.
2. **Components are functions of `{ params, query, state, outlet? }`**.
   No `data` prop. No hidden `inject` globals required in route
   components (though `inject` is retained for *non-route*
   sub-components like Header/Sidebar that aren't directly invoked
   by the router).
3. **Trivially testable.** Pass a stub `state` object to the route
   component and to `load()`. No `_setCurrentApp` dance is required
   for route-component testing.
4. **`load()` returns `void`.** The router awaits it for ordering
   (to drive the 150ms loading-UI timer and to surface errors), but
   does not consume the return value. If user code returns
   something, the runtime ignores it.

Route-scoped data — e.g., the user record on `/users/:id`, which
genuinely lives for one route mount — fits this model in one of
three ways:

- **Recommended:** a state slice that holds a map keyed by id:
  `app.state('users', signal({}))`. `load()` writes
  `state.users.update(m => ({ ...m, [params.id]: fetched }))`. The
  component reads `state.users.val[params.id]`. Dedup across
  revisits is automatic.
- **Local signal in the component.** The route component function
  runs exactly once per mount; it can declare a local signal and
  trigger an effect to populate it. The 150ms loading-UI hook is
  *not* available for this path, since the router can't await
  in-component effects.
- **Future `resource(loader)` primitive.** A standalone spec will
  add this for the common case of "give me a reactive container
  whose loader runs at most once." It is **out of scope** here;
  the model above must work without it.

This is a deliberate divergence from the framework spec's §6 text
("`export async function load(...) { return { user: await ... } }`"
with a `data` prop). The framework spec text should be amended to
match this model when the broader framework spec is next revised.

### Pipeline shape this spec installs

The navigation pipeline becomes:

```
match
  ├─ short-circuit if no match (404 path; mount cleared)
  └─ run pipeline inside a fresh nav scope:
       1. start "show loading after delay" timer
       2. for each chain entry (parent → child for nested routes):
            a. run global middleware (one-shot per navigation)
            b. run entry's guard (if any)
            c. resolve entry's component (lazy import if needed)
            d. run entry's load() (if any)
          middleware/guard/load may all be async; any may redirect or throw
       3. cancel loading timer; remove loading UI if it was shown
       4. build the layout/parent → child TemplateResult chain
       5. clear mount, commit, apply active-link attributes
       6. on error during steps 1–5: commit the error UI in place
```

Global middleware runs **once per navigation**, not once per chain entry —
it observes the navigation as a whole. Guards are per-route (and per
nested entry); each entry can independently veto.

### Supersede-on-new-nav

Each `_navigateTo` call increments a nav-token counter and captures the
current token. After every `await` boundary inside the pipeline, the
captured token is compared against the current one; on mismatch the
pipeline aborts before any DOM mutation. This generalizes the
lazy-loader race note from Part 1 to cover middleware, guards, and
`load()` too. No `AbortSignal` is plumbed through to user code in this
spec.

### Loading delay

Showing the loading UI immediately would flicker on instant navigations
(cached lazy imports, synchronous loaders). A small delay debounces:
`app.loading()` content commits only if the pipeline hasn't resolved
within **150ms**. If it does fire and then the pipeline finishes (or
errors), the loading content is replaced by the final content. If the
nav is superseded mid-pipeline, the loading timer is cancelled.

### Error recovery

If middleware / guard / load throws (or a rejected Promise), the
pipeline catches, cancels the loading timer, and renders
`app.error({ error, retry })` into the mount. `retry()` invokes
`_navigateTo` again for the same URL, which re-runs the whole pipeline
from scratch. The error value is whatever was thrown; common patterns
are `throw { status: 404 }` from a `load()` for not-found,
`throw new Error(...)` for generic failures.

### Per-route overrides

`app.route(path, loader, { loading, error })` may override the global
loading/error components for that route. A nested-route entry may
override too; the closest enclosing override along the
parent → child chain wins. If nothing is set anywhere, the loading UI
is simply not rendered and a thrown error is re-thrown on the console
(this matches "Loading UI handling lands in the second spec" being
honored here while still keeping app-without-error-handler usable).

## Requirements

### `app.use(middleware)`

- Registers a middleware function. Returns `this` (chainable).
- Multiple `use()` calls form an **ordered chain**, executed in
  registration order on every navigation.
- After `run()`, `use()` throws ("cannot be called after run()") — same
  pattern as `state` / `route` / `layout`.
- Middleware signature:
  ```js
  middleware({ route, state, redirect }) => void | Promise<void>
  ```
  - `route`: `{ path, params, query, meta }` — the navigation target.
    `meta` is the route entry's static `meta` object (see **Route meta**
    below) or `{}` if absent.
  - `state`: a read-only view of the App state map — `state.<key>`
    resolves to the value registered via `app.state(<key>, value)`. No
    setter; middleware mutates state by calling methods on the values
    themselves (e.g., a signal's `.set`).
  - `redirect(path, opts?)`: short-circuits the current navigation. The
    *current* pipeline is abandoned (its token is invalidated), then
    `navigate(path, opts)` is invoked. `opts` accepts `{ replace }`;
    redirects default to `replace: true` (the redirected-away-from URL
    does not get its own history entry).
  - Returning a Promise causes the chain to await it before continuing.
  - Throwing or rejecting routes to the error path (see **Error
    handling** below).
- Middleware runs **once per navigation** (not once per nested route
  entry). It sees the leaf-most matched route in its `route` argument
  but its `meta` is the *merged* meta from parent → child along the
  chain (last write wins per key).

### `app.route(path, loader, opts)` — extended opts

Adds three optional opts (the existing two-arg signature still works):

```js
app.route(path, loader, {
  guard,        // ({ params, query, state, route, redirect }) => boolean | void | Promise<...>
  load,         // ({ params, query, state, fetch, route }) => void | Promise<void>
  loading,      // () => TemplateResult — per-route loading override
  error,        // ({ error, retry }) => TemplateResult — per-route error override
  meta,         // arbitrary static object accessible from middleware/guards
  children,     // nested-route entries (see below)
})
```

- `guard` returning `false` aborts the navigation **without** redirecting
  or erroring — the previous content stays on screen, the URL is **not**
  rolled back automatically (this is the responsibility of the guard via
  `redirect`; a bare `return false` is a "stay where you were" intent
  that callers rarely use). Returning `true` or `undefined` proceeds.
  Throwing routes to error UI. `redirect(...)` works the same as in
  middleware.
- `load` is invoked **after** middleware and guard. It is a
  side-effectful function — it writes into `state` slices and its
  return value is **discarded by the router**. Components read the
  resulting state via their `state` prop. See **Data loading model**
  in Background for rationale and examples.
- `load` signature: `({ params, query, state, fetch, route }) => void | Promise<void>`.
- The `fetch` argument to `load` is `globalThis.fetch.bind(globalThis)`
  in production. The test runner can swap `globalThis.fetch` before
  invoking `_navigateTo` to stub responses; an explicit `fetch` opt on
  the App for global override is **out of scope** (defer to Phase 5).
- `loading` and `error` are plain components (no props for loading;
  `{ error, retry }` for error). Per-route overrides win over global.

### `app.loading(component)`

- Registers the global loading UI. Returns `this`. Throws after `run()`.
- Calling twice throws (`loading already set`).
- `component` is a plain function returning a `TemplateResult`. No
  arguments are passed.

### `app.error(component)`

- Registers the global error UI. Returns `this`. Throws after `run()`.
- Calling twice throws.
- `component` is `({ error, retry }) => TemplateResult`.
- `retry` re-runs `_navigateTo` with the URL at the time the error was
  rendered. The error UI is replaced with the new pipeline's loading UI
  (after the 150ms delay) or final content.

### Divergence point

The **divergence point** is the deepest chain entry shared between the
old and new navigation chains. Computed by walking root → leaf comparing
entry references; the divergence point is the first index at which the
entries differ. If there is no old chain (initial render), the
divergence point is the root mount itself. If the root entries already
differ (or there is no `app.layout()` and the old/new root entries are
different), the divergence point is again the root mount.

The divergence point is used for two things:

1. **Scope sharing across navs** — entries above the divergence point
   keep their scopes and their committed DOM untouched. Entries at and
   below the divergence point are torn down and rebuilt.
2. **Loading and error UI placement** — both render *into the parent
   slot at the divergence point*, not into the root mount. Preserved
   chrome above the divergence point stays on screen during loading and
   during errors.

### Outlet: the reactive router slot

The router-injected child slot on a parent route component is named
**`outlet`**, not `children`. The two names disambiguate:

- **`children`** (route registration) — the static array of nested
  route definitions passed to `app.route(..., { children: [...] })`.
  Defines the route *tree*.
- **`outlet`** (component prop) — the reactive slot the router writes
  into at render time, holding whichever child route's
  `TemplateResult` is currently matched. The parent component reads
  it via `${outlet}` in its template.

Regular component composition keeps the conventional `children` prop:
`Card({ title, children })`, `Layout({ outlet })` for a router-managed
layout slot, `Page({ header, sidebar, children })` for user-passed
content. The rule: **any slot the router fills is called `outlet`;
any slot the developer fills by hand is called `children`** (or a
custom prop name).

For loading/error UI to swap into the outlet without re-committing
the parent, each parent entry's `outlet` is backed by a reactive
container the router writes into during a navigation:

- The router holds a `signal` per chain entry, `entry._outletSig`.
  Its value is the `TemplateResult` currently rendered in that
  entry's outlet.
- The parent component receives `{ params, query, state, outlet }`
  where `outlet` reads from this signal — the template system's
  existing reactive-block handling already swaps node content in
  place when a signal value changes inside `${outlet}`.
- During a navigation, the router writes loading content / error
  content / final child content into the divergence point's parent's
  `_outletSig`. The parent component is **never re-invoked**; only
  the outlet content changes.
- The leaf entry has no `_outletSig` (it has no nested routes to
  render).
- For the root mount (no parent above the divergence point), the
  equivalent reactive slot lives on the App: `app._rootSlotSig`.
  Initial mount commits a single TemplateResult — `app.layout({
  outlet: rootSlot })` if a layout exists, otherwise just `rootSlot`
  — once. All subsequent navigations swap `_rootSlotSig` without
  re-committing the layout.

This is a small but important refinement to the Part 1 commit model:
Part 1 cleared and re-committed the mount on every nav. Part 2
commits the layout (or naked root slot) **once** at `run()` time and
mutates outlet signals thereafter. The active-link walk still
re-runs after each successful nav (it reads the live DOM, so it's
unaffected by where the swap happened).

**Note on the layout `children` → `outlet` rename:** Part 1 shipped
`app.layout(component)` where `component` received `{ children }`.
This spec changes the prop name to `outlet` for consistency with
nested routes. Existing Part 1 demos must rename. The change is
mechanical and the framework spec text (§2, §6) should be updated to
match.

### Loading-display timing

- The pipeline starts a `setTimeout(_commitLoading, 150)` immediately
  after match.
- If the pipeline (including all awaits) finishes before 150ms, the
  timer is cancelled and the loading UI never renders — the navigation
  is a clean swap.
- If the timer fires first, the loading content is committed at the
  **divergence point**:
  - Resolve the loading component by walking the new chain from the
    divergence point downward to the leaf: first per-route `loading`
    override wins; if none, fall back to global `app.loading()`; if
    none, do nothing (the prior outlet content stays on screen).
  - Build the loading `TemplateResult` and write it into the parent's
    `_childrenSig` (or `app._rootSlotSig` if the divergence point is
    the root). The slot swap happens reactively — the parent
    component is not re-invoked, preserved chrome stays mounted.
  - The pipeline continues; when the final commit lands it overwrites
    the same slot with the resolved chain content.
- A new navigation that supersedes an in-flight one cancels the old
  pipeline's timer before any loading content commits.
- The loading content is committed inside the **nav scope** (a fresh
  scope created at the start of `_navigateTo` and disposed when the
  pipeline finishes or is superseded), so any reactive bindings in
  the loading component are cleaned up when the navigation finishes
  or is superseded.

### Error handling

- Any throw / rejection inside middleware, guard, lazy-load import, or
  `load()` is caught by the pipeline runner.
- On catch:
  - Cancel the loading timer.
  - Compute the **divergence point** between the old chain and the
    chain in progress (entries already partially resolved up to the
    throwing step). Entries above the divergence point keep their
    scopes and committed DOM.
  - Resolve the error component by walking the new chain from the
    divergence point downward to the leaf: first per-route `error`
    override wins; if none, fall back to global `app.error()`; if
    none, log the error via `console.error` and leave the prior
    outlet content on screen (the navigation effectively fails
    open).
  - If an error component is found, build a `retry` closure that
    calls `app._navigateTo(originalInput)`, then commit the error
    `TemplateResult` into the divergence point's parent slot
    (`_childrenSig` or `app._rootSlotSig`) — the same swap mechanism
    as the loading UI. Preserved chrome above the divergence point
    stays on screen.
- The error UI is committed inside the **nav scope**, so a fresh
  navigation (or `retry` itself) disposes its effects when the slot
  is overwritten.
- Errors raised by global middleware (which runs before any chain
  entry is resolved) commit at the root mount — the divergence point
  is effectively the root since no new chain has been walked yet.

### Nested routes via `children`

`children` opt on `app.route` (and on each entry within `children`):

```js
// src/app.ts — registration
app.route('/dashboard', () => import('./routes/dashboard'), {
  children: [
    { path: '/',          load: () => import('./routes/dashboard/overview') },
    { path: '/analytics', load: () => import('./routes/dashboard/analytics') },
    { path: '/settings',  load: () => import('./routes/dashboard/settings') },
  ],
})
```

The parent route component receives `outlet` and renders it where
the matched child should appear. A leaf route does **not** receive
`outlet`. Both receive `state`. Per the data-loading model,
`load()` hydrates state slices; the component reads them via the
`state` prop:

```js
// src/app.ts — register the slices the dashboard family uses
app.state('analytics', signal(null))

// src/routes/dashboard.ts — parent (has nested children, so receives outlet)
export default function Dashboard({ outlet }) {
  return html`
    <div class="dashboard">
      <aside>
        <a href=${'/dashboard'}>Overview</a>
        <a href=${'/dashboard/analytics'}>Analytics</a>
        <a href=${'/dashboard/settings'}>Settings</a>
      </aside>
      <section>${outlet}</section>
    </div>
  `
}

// src/routes/dashboard/analytics.ts — leaf (no children, no outlet prop)
export async function load({ state, fetch }) {
  if (state.analytics.val != null) return  // idempotent: skip if already loaded
  const res = await fetch('/api/analytics')
  state.analytics.set(await res.json())
}

export default function Analytics({ state }) {
  return html`
    <h1>Analytics</h1>
    <p>${() => state.analytics.val.views} views this week</p>
  `
}
```

Two components reading `state.analytics.val` share the same signal —
no duplicate fetch when sibling routes (e.g. an analytics-summary
widget in the Dashboard sidebar) need the same data.

The relationship: `children: [...]` at registration declares *which*
nested routes exist; `${outlet}` in the parent's template declares
*where* the matched child renders. If `${outlet}` is omitted from a
parent that registered `children`, child routes match but render
nowhere — a class of bug worth surfacing in dev mode (see Open
Questions).

Notes on shape:

- Each child entry is `{ path, load, guard?, loading?, error?, meta?, children? }`.
  Yes, `children` recurses — arbitrary depth.
- The `load` field on a child entry doubles as the loader-or-component
  (same eager/lazy detection rules as the parent's positional argument
  in Part 1). The name `load` is used inside `children` to match the
  framework spec; for the top-level `app.route(path, loader, ...)` the
  positional second arg is still the loader-or-component, and there is
  no top-level `load` opt that competes with it. `guard`, `loading`,
  `error`, `meta` may also appear at any depth.
- Path concatenation: a child's `path` is joined onto the parent's
  normalized path. `/` as a child path means "exact parent match"
  (i.e., parent path with no extra segments). All resulting full paths
  are compiled and registered as siblings in the route table at
  registration time, preserving order. (Trees are flattened into the
  flat list `_routes` already used; each compiled entry carries a
  `chain: [...]` of the ancestor entries from root to leaf.)
- Match returns the leaf entry; the pipeline walks the leaf's `chain`
  from root to leaf, running each entry's `guard` and resolving each
  entry's component + `load`.
- The render is constructed inside-out: leaf → parent → grandparent.
  Each ancestor receives `{ params, query, state, outlet }` where
  `outlet` is a **reactive slot** (a signal `entry._outletSig` whose
  value is the next-deeper `TemplateResult`). The leaf receives
  `{ params, query, state }`, no `outlet`. Reading `${outlet}`
  inside the parent's template auto-subscribes to the signal — when
  the router swaps the slot value (loading → final, or loading →
  error, or final → next-leaf on a sibling sub-nav), only the
  outlet contents change; the parent component is not re-invoked.
- `state` at every level is the **same** state proxy — there is no
  per-level isolation. Each `load()` writes into the shared state
  map; the rendering chain reads from the same map. This is exactly
  what makes dedup structural: a parent's `load()` hydrating
  `state.user` is observed by every descendant's component.
- Each level gets its own **nested scope** (created via
  `_createScope()` inside the parent's scope's `run`). On navigation,
  the nav scope is disposed first; the new pipeline rebuilds the
  scope tree from scratch. Optimization: if the new path shares
  ancestor entries with the previous, the ancestor's scope is
  **preserved** and only the changed sub-tree is re-rendered. This
  matters for the dashboard-with-sub-pages case (sidebar stays
  mounted). See **Scope sharing across navs** below.
- `params` and `query` passed at every level are the full leaf
  match's `params` / `query`. Ancestor entries' guards/loads see the
  same `params` (segment-bounded named params still come from the
  leaf-resolved URL).
- `meta` merge: when reading `route.meta` from middleware, parent
  metas are shallow-merged left-to-right (child overrides parent on
  matching keys).
- The active link attributes (`data-active` / `data-active-exact`)
  continue to compare against the leaf `pathname` / `search`; the
  prefix rule already covers the parent-route case (`/dashboard`
  anchor gets `data-active` while on `/dashboard/analytics`).

### Scope sharing across navs

To avoid unmounting shared chrome on sub-route navigations:

- Each chain entry's runtime state is tracked on the App instance:
  `this._chain: Array<{ entry, scope, outletSig }>` (the current
  chain, root → leaf).
- On a new navigation, after match:
  - Compute the **divergence point** (see the "Divergence point"
    section) — the deepest index where old and new entries are
    identical by reference.
  - Entries above the divergence point keep their scope, their
    committed DOM, and their `outletSig`. Their components are not
    re-invoked.
  - Entries at and below the divergence point have their scopes
    disposed leaf-first, then are rebuilt by the new pipeline.
- When the divergence point is at the leaf (i.e., new nav is to the
  same leaf entry, only params/query changed), the leaf entry is
  **still rebuilt** because its own `guard` / `load()` may need to
  re-run with new params. The parent entries above the leaf are
  preserved (their scopes, their committed DOM). The leaf's parent
  has its `outletSig` swapped to the newly-built leaf
  `TemplateResult`.
- For the initial render, there is no prior chain to share; the
  divergence point is the root mount and the full chain is built.
- If the pipeline throws, the error UI commits at the divergence
  point per **Error handling**. Preserved chrome above stays mounted.

### Route meta

A static plain object attached to a route entry at registration:

```js
app.route('/admin', loader, { meta: { protected: true } })
```

- Available to middleware as `route.meta`.
- This spec **does not** read `meta.title` or update `document.title` —
  that lands when the broader `meta` story (titles, OG tags, head
  injection) is specified.
- Not reactive. Meta is read once per navigation.

### Updated `_navigateTo` order

Replaces the Part 1 pipeline. Pseudocode (errors caught into the error
path are omitted for clarity; see **Error handling**):

```
_navigateTo(input):
  token = ++this._navToken

  match = _matchRoutes(this._routes, input)
  if !match:
    teardown old chain scopes; swap root slot to empty/404 fallback
    update reactive route signals (best-effort parse of input)
    return

  // Compute divergence index against the existing chain
  divergeAt = computeDivergence(this._chain, match.entry.chain)
  parentSlot = slotAt(divergeAt)   // either an entry._childrenSig or app._rootSlotSig

  // Start loading-display timer (writes into parentSlot when it fires)
  loadingTimer = setTimeout(() => commitLoading(parentSlot, match, divergeAt), 150)

  // Build merged meta from the new chain
  mergedMeta = mergeMeta(match.entry.chain)

  // Run global middleware (once per nav)
  for mw in this._middleware:
    await mw({ route: { path, params, query, meta: mergedMeta }, state: stateProxy, redirect })
    if token != this._navToken: clearTimeout(loadingTimer); return  // superseded
    if a redirect was issued: clearTimeout(loadingTimer); return     // recursive _navigateTo

  // Walk the new chain from the divergence point downward, running
  // guard + resolve + load on entries at/below divergeAt only.
  // Entries above divergeAt are preserved as-is.
  for i from divergeAt to leaf:
    entry = match.entry.chain[i]
    if entry.opts?.guard:
      const r = await entry.opts.guard({ params, query, state, route, redirect })
      if token != this._navToken: clearTimeout(loadingTimer); return
      if r === false:
        clearTimeout(loadingTimer)
        // No commit; prior parentSlot content stays. URL handling: see Open Questions.
        return

    if entry.resolvedComponent == null:
      resolve component (eager TR cached / lazy import awaited & cached)
      if token != this._navToken: clearTimeout(loadingTimer); return

    if entry.opts?.load:
      // load() is side-effectful; its return value is ignored.
      await entry.opts.load({ params, query, state, fetch, route })
      if token != this._navToken: clearTimeout(loadingTimer); return

  // Cancel loading timer; loading UI either never showed or will be replaced
  clearTimeout(loadingTimer)

  // Tear down old chain entries at/below divergeAt (leaf-first)
  for i from oldChainLength-1 down to divergeAt:
    this._chain[i].scope.dispose()

  // Build new chain entries leaf → divergeAt, inside fresh scopes
  // parented by the entry at divergeAt-1's scope (or the nav scope at root)
  childTR = undefined
  for i from leaf down to divergeAt:
    entry = match.entry.chain[i]
    const scope = _createScope()  // parented to entry above, or app root scope
    let outletSig = i < leaf ? signal(childTR) : null
    scope.run(() => {
      const tr = entry.resolvedComponent({
        params: match.params,
        query: match.query,
        state: stateProxy,
        outlet: outletSig ? makeOutletSlot(outletSig) : undefined,
      })
      this._chain[i] = { entry, scope, outletSig }
      childTR = tr
    })

  // Swap the divergence point's parent slot to the new childTR
  parentSlot.set(childTR)

  // Update reactive route signals
  this._pathSig.set(match.pathname)
  this._paramsSig.set(match.params)
  this._querySig.set(match.query)

  // Apply active-link attributes by walking the live DOM under mountEl
  applyActiveLinks(mountEl, match.pathname, match.search)
```

Where:

- `slotAt(divergeAt)` returns `app._rootSlotSig` when `divergeAt === 0`
  (the root mount is the boundary), otherwise the parent entry's
  `_outletSig` (the entry at index `divergeAt - 1`).
- `makeOutletSlot(sig)` wraps the signal in whatever shape the
  template system needs for a reactive `${outlet}` read — likely
  just the signal itself, since the template system already
  auto-subscribes on `.val` reads.
- `commitLoading(parentSlot, match, divergeAt)` resolves the loading
  component (per-route on entries at/below divergeAt → global → none)
  and `parentSlot.set(loadingTR)`.

### Error UI commit point

When an exception escapes any pipeline step:

```
catch (err):
  if token != this._navToken: return     // a newer nav already won
  clearTimeout(loadingTimer)

  // divergeAt was computed at the start of the navigation;
  // parentSlot points at the divergence point's reactive slot.
  const errorComp = resolveErrorComponent(match?.entry?.chain, divergeAt)
  // resolveErrorComponent walks divergeAt → leaf for per-route error
  // overrides, then falls back to this._error (global), then null.

  // Tear down any new-chain scopes that were partially built at/below
  // divergeAt during this navigation; preserved chrome above stays.
  for i from oldChainLength-1 down to divergeAt:
    this._chain[i]?.scope.dispose()
    this._chain[i] = undefined

  if errorComp:
    const errScope = _createScope()
    errScope.run(() => {
      const tr = errorComp({ error: err, retry: () => this._navigateTo(input) })
      parentSlot.set(tr)
    })
    // Track errScope so the next nav can dispose it; store at
    // this._chain[divergeAt] = { entry: null, scope: errScope, outletSig: null }
    // so the regular tear-down loop picks it up.
  else:
    console.error('navigation error', err)
    // parentSlot keeps its prior value; preserved chrome and any
    // existing outlet content stay on screen.
```

### Updated route-component signature

```js
Component({ params, query, state, outlet? })
```

- `params`, `query`: unchanged from Part 1.
- `state`: the same state proxy passed to `load()`, `guard`, and
  middleware. `state.<key>` returns the value registered via
  `app.state(<key>, value)`. The component reads from state; it does
  not receive a separate `data` prop. See **Data loading model** in
  Background.
- `outlet`: for parent entries in a nested-route chain (or for the
  `app.layout()` component), the reactive slot the router writes the
  next-deeper `TemplateResult` into. `undefined` for the leaf.

Breaking changes to Part 1's component contract:

1. New `state` and `outlet` fields are added. Purely additive — Part
   1 components that ignore them still work.
2. The layout component's slot prop is renamed from `children` to
   `outlet`. Part 1 demos using `Layout({ children })` must rename
   to `Layout({ outlet })`.
3. The framework spec at §6 currently shows a `data` prop on route
   components and a `return { ... }` from `load()`. Both are
   superseded by the data-loading model in Background. The
   framework spec text must be updated to match when next revised.

### `app.layout` interaction

The single app-wide layout (Part 1) is now treated as a **zero-path
parent above the root chain entry** for slot/divergence purposes:

- At `run()`, if a layout is registered, the App builds it once with
  `app._rootSlotSig` as its `outlet` prop, and commits the layout
  TemplateResult into the mount. The layout is **never re-invoked**
  on subsequent navigations.
- If no layout is registered, the App still commits a small invisible
  wrapper (a single comment-anchor + reactive block reading
  `app._rootSlotSig`) into the mount at `run()`. Navigations swap the
  root slot reactively; nothing about the wrapper changes.
- The layout's `outlet` prop is the same reactive-slot shape as a
  nested-route parent's `outlet`: reading `${outlet}` subscribes to
  `_rootSlotSig`; the router writes to it on every nav.
- A layout failure (the layout function itself throwing during the
  initial `run()`) is fatal — there is no error UI to fall back to
  before the layout commits. This case is treated as a programmer
  error and re-thrown.
- Layout has no `load()` of its own (cannot hydrate state on its
  behalf). If layout-level slices need pre-population, do it in the
  root-route entry's `load()` or in middleware.

For a nav from `/dashboard/overview` to `/`, the divergence point is
the root slot (different root entries), so the loading/final/error
content swaps into `app._rootSlotSig` — Layout stays mounted, only
the slot underneath swaps. For a nav from `/dashboard/overview` to
`/dashboard/analytics`, the divergence point is the dashboard's child
slot — both Layout and the Dashboard component stay mounted; only
the deepest slot swaps.

### File layout

```
runtime/
  app.js          # extend: use(), loading(), error(), pipeline, chain rebuild, error path, loading timer
  router.js       # extend: nested-route flattening into _routes with chain field
  app.test.js     # add: middleware, guards, load(), loading/error UI, nested routes
  router.test.js  # add: nested-route flattening, chain construction
```

No new files. The `_matchRoutes` helper already returns the matched
route entry; adding a `chain: [rootEntry, ..., leafEntry]` field onto
each flattened route entry at registration time keeps the matcher
itself unchanged.

## Constraints

- Plain JavaScript + JSDoc. Match existing Phases 1, 2, 3a.
- No new external deps. The 150ms delay uses `setTimeout`.
- Middleware/guard/load may be async; the pipeline must handle the
  `superseded` case after every `await`. Single nav-token counter on
  the App instance suffices; do not introduce `AbortController` /
  `AbortSignal` plumbing in user-visible APIs.
- Redirects from middleware/guard default to `replace: true` so the
  source URL of the redirect does not get its own history entry. (User
  can pass `{ replace: false }` to keep it.)
- All effects/event listeners committed during a chain entry's render
  must be owned by that entry's scope so per-level dispose works
  correctly. The leaf-most entry owns the bulk of the route-specific
  effects; ancestor scopes own ancestor effects only.
- No retry-loop protection on `retry()` — user code is responsible for
  not creating infinite retry loops. (A "max retries" cap is a future
  spec.)
- Per-route `loading` and `error` opts apply only to their own entry
  and its descendants — they do **not** retroactively apply to
  ancestor failures. (Resolution walks the chain from the leaf
  upward, then falls back to global.)
- Trailing-slash normalization applies after path concatenation in
  nested routes. `'/dashboard' + '/'` → `'/dashboard'`,
  `'/dashboard' + '/analytics'` → `'/dashboard/analytics'`.

## Out of Scope

- Route transitions (CSS classes during enter/leave, `transition` opt
  on `app.route`).
- `meta.title` → `document.title` syncing (and any other head
  metadata injection).
- `app.on(stateKey, stateName, handler)` — machine-to-machine wiring;
  depends on Phase 4 machines.
- `app.testMiddleware()` test helper — small surface, but cleaner to
  ship in the test-runner phase (Phase 5).
- `group(opts, routes)` — defer; users can attach `guard` per route
  for now.
- A `resource(loader)` primitive for lazily-loaded reactive
  containers with built-in dedup, status tracking, revalidation
  policies, and mutation support. Userland can hand-roll the
  "if (state.x.val == null) state.x.set(await fetch...)" pattern
  in the meantime.
- An `ensureLoaded(slice, fetcher)` helper that wraps the
  idempotent-load guard for `load()` functions. Trivial to add
  later if the pattern proves repetitive in practice.
- A global `app.fetch` override or fetch interceptors. Tests stub via
  `globalThis.fetch`.
- `AbortSignal` propagation into user `load()` functions.
- Max-retry protection on `error.retry`.
- Multi-segment wildcards (still deferred from Part 1).

## Open Questions

- **Guard returning `false` with no redirect.** The spec says the
  prior content stays on screen and the URL is not rolled back. This
  leaves the URL out of sync with the rendered route until the next
  successful nav — is that acceptable, or should the router push the
  prior URL back via `history.replaceState` to keep address bar and
  view consistent? Recommendation: do the `replaceState`-to-prior in
  the plan, since silent URL/view drift confuses users.
- **Layout vs nested-route parent.** Both wrap children. Spec keeps
  them separate (app-wide `layout` wraps the root chain entry). An
  alternative is to drop `layout()` entirely and treat it as a
  zero-path root route with `children`. Recommendation in the plan:
  keep both; the friction of converting Part 1 demos to nested-route
  registration is not justified by the small surface savings.
- **Middleware short-circuit semantics.** Spec says `redirect()`
  abandons the current nav and starts a new one (recursion through
  `navigate`). Should `redirect` from inside middleware return a
  promise the caller can await, so subsequent middleware does not
  run? The recommendation here is "first redirect wins, subsequent
  middleware does not run" — but since async middleware may have
  awaits in flight, we rely on the token check to no-op them.
  Confirm in the plan that no middleware can observe state after a
  redirect has been issued.
- **`fetch` ergonomics.** The framework spec shows `fetch` passed as a
  prop to `load()`. In production it's `globalThis.fetch`. Tests
  monkey-patch `globalThis.fetch`. Is this the desired contract, or
  should `load()` also accept a `signal` (for `AbortController`) and
  / or a typed `fetch` wrapper that propagates the nav token? This
  spec defers both. Confirm.
- **`load()` running on params-only changes.** Spec says navigating to
  the same leaf with different params re-runs the leaf's `load()`.
  Under the state-hydration model, this means `load()` runs and
  (presumably) overwrites the relevant slice. For shared slices that
  should persist across param changes (e.g., the current user), the
  user's idempotency guard (`if (state.x.val == null) ...`) prevents
  re-fetch. For per-param slices keyed by id, the user fetches
  unconditionally for the new id. The model leaves this entirely
  in user code, which is the right place. Recommendation:
  unconditionally re-run `load()` on every nav; a future
  `revalidate` opt can refine if a uniform policy is wanted.
- **Stale state across logout / context switch.** With state living
  outside route mounts, navigating away does not clear it. If the
  user logs out and logs back in as a different user,
  `state.user.val` is still the old user until something writes to
  it. Application code must reset slices on auth state changes. This
  is the same shape as Redux / Zustand / signals-libraries — not a
  router concern — but call it out so reviewers don't mistake it for
  a bug. A "scope state to a slice that resets when X happens" tool
  is a state-machines concern (Phase 4) more than a router one.
- **Loading-delay constant.** 150ms is the recommendation; React
  Router, TanStack, and Remix sit between 100ms and 300ms. Plan
  should not make this configurable in this spec (avoid the foot-gun
  of per-app drift); revisit if real-world feedback says 150 is
  wrong.
- **Error component absence.** Spec falls back to `console.error` and
  leaves the prior content on screen. Alternative: render a built-in
  minimal error fallback ("Something went wrong"). Recommendation:
  keep the silent fallback — frameworks that render UI you didn't
  write are surprising; the console message is enough signal during
  development.
- **Reactive-slot feasibility.** This spec assumes the existing
  template system handles a signal whose `.val` is a `TemplateResult`
  by mounting that TR, and on `.set(newTR)` cleanly replacing the
  rendered content. The current `template.js` `_applyNodeValue` path
  treats `_isReactive(value)` and runs an `effect` that calls
  `_applyNodeValueLeaf(anchor, value.val, ...)` — which routes a TR
  through `_appendNodeItem` → `commit(value, frag)`. That should work
  out of the box. The plan must add a focused unit test for this
  exact pattern (signal-of-TR, swapped multiple times, asserting that
  old DOM is removed and old effects are disposed) before relying on
  it. If a gap is found, the plan extends `template.js` first.
- **Missing `${outlet}` in a parent template.** If a route registers
  `children: [...]` but the parent component's template omits
  `${outlet}`, child routes match but render nowhere — a silent
  footgun. Options: (a) do nothing, document the convention; (b) at
  registration time, parse the parent component's template strings
  to detect an `outlet` reference (brittle, the function may build
  its template conditionally); (c) at first render, check whether
  the parent's committed DOM subscribed to `_outletSig` and warn via
  `console.warn` if not. Recommendation in the plan: (c), but only
  in a development build flag — production stays silent.
- **Loading content disposal on supersede.** When a slow loader's
  loading UI is on screen and a new nav supersedes, the new nav
  begins by computing its own divergence point against the *current*
  chain (which may include a partially-built or loading-only state).
  The plan needs to define how mid-pipeline state is represented in
  `this._chain` so the new nav's divergence computation does not get
  confused. Recommendation: only mutate `this._chain` after a chain
  step has fully committed (or for the error/loading slot insert);
  partial-build state lives in local variables of the in-flight
  `_navigateTo` call.
