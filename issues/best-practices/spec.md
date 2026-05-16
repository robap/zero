# Spec: Best Practices & Example Applications

## Problem Statement

Zero is usable but has no canonical guidance for organizing a real
application. Phase 11 (Decorators) was the planned vehicle for the
DX wins that motivated it — terser route registration, type-inferred
state access, co-located metadata, conventional structure — but JS/TS
decorators only attach to classes and class members. Forcing the
framework off its function-first model to gain `@Route` is the wrong
trade. Most of those wins survive as patterns and a strong opinion
about file structure. Phase 12 delivers (a) three buildable example
applications spanning a complexity curve and (b) docs that point at
specific files in those examples as canonical patterns.

## Background

### What exists

- The scaffold (`src/scaffold/`) emits a minimal counter (`src/app.ts`
  + `src/routes/home.ts` + one test). It is the only example today.
- `showcase/` exists as a sibling zero project for the component
  library and is exercised by `tests/showcase_build.rs` /
  `tests/showcase_dev.rs`. Same hosting model applies to examples.
- Component code today reads state as
  `inject<Signal<number>>("count")` — stringly-keyed and manually
  generic. §11 of the framework spec lists `inject(key)` with no
  registry mechanism.
- §5 of the framework spec defers state machines and recommends
  `signal({ status, ...data })` as the replacement. That guidance has
  no working demonstration in the repo.
- §6 of the framework spec specifies that route `load()` receives a
  `fetch` parameter for testability. The router today injects
  `window.fetch` verbatim — no `AbortSignal` is attached, so in-flight
  requests outlive route navigations and can land stale responses
  after the route scope has been disposed.
- `AGENTS.md` exists and is the conventional location for
  agent-facing repo guidance. The framework spec is the capability
  reference; it is not the place for choice-style guidance.

### Why now

After Phases 7–9, the framework's authoring surface is stable
(`.zero/` regenerable, design system, component library). Phase 11
was the next planned DX play and is blocked by the decorator
restriction. Phase 12 reaches the same goals through patterns: a
typed key registry replaces the manual generic on `inject`; module
stores co-locate state with the functions that mutate it; a canonical
file layout removes the "where does this go" tax; example apps give
adopters a concrete reference. Without this work, every adopter
invents their own conventions, the showcase remains the only example,
and the deferred-machines recommendation in §5 stays abstract.

## Requirements

### Example applications

Three buildable example projects under `examples/`, each an
independent zero project (its own `zero.toml`, `index.html`, `src/`,
test suite).

- **`examples/counter/`** — minimal: one route, one state key, one
  component. Demonstrates `signal`, `app.state`, `inject`,
  `app.route`, `html`. Target: ~50 lines of TS.
- **`examples/todos/`** — mid-size: list with add / edit / delete /
  filter, one form, localStorage persistence. Introduces `each()`
  keyed rendering, the module-store pattern, a structured single
  signal (`signal({ items, filter })`), and a typed key registry
  (`src/state.ts`).
- **`examples/tracker/`** — full: an issue tracker. Auth (login →
  protected dashboard), issue list with query-param-driven filters,
  detail view via `/issues/:id`, comment thread, nested layout for
  the dashboard shell. Exercises route guards, `load()` data
  fetching against a static JSON fixture, status-tagged auth signal,
  and the full state-organization stack.

Every example builds successfully under `zero build` and passes
`zero test`. Each example ships unit tests — one `*.test.ts` per
route and per non-trivial component — demonstrating the recommended
testing patterns from §8 of the framework spec.

A new integration test `tests/examples_build.rs` builds all three
examples and asserts clean exits and non-empty `dist/` output,
mirroring `tests/showcase_build.rs`. A second integration test
`tests/examples_tests.rs` runs each example's `zero test` suite,
mirroring `tests/component_library.rs`.

### Canonical file structure

The full example codifies the recommended layout. Smaller examples
adopt as much as they need; nothing in a smaller example contradicts
the full example's structure.

```
src/
  app.ts             # app construction, state registration, route table
  state.ts           # typed key registry (see below)
  routes/            # one file per route; nested folders for nested routes
    home.ts
    login.ts
    issues/
      index.ts       # /issues
      [id].ts        # /issues/:id
  components/        # presentational components only
  stores/            # one module per logical store: signal + mutators
    auth.ts
    issues.ts
  lib/               # non-UI helpers (formatters, validators)
styles/
  app.scss
```

Docs explicitly call out: `routes/` holds route components and their
co-located `load()` and `meta` exports; `stores/` owns state mutation;
`components/` is for presentational code only and does not call
`signal.set()` on store signals; `state.ts` is the single source of
truth for `inject` keys.

### Typed key registry

`examples/todos/` and `examples/tracker/` ship a `src/state.ts` that
declares both the key constants and the value types:

```ts
import type { Signal } from "zero"
import type { AuthState } from "./stores/auth"
import type { IssuesState } from "./stores/issues"

export const Keys = {
  Auth:   "auth"   as const,
  Issues: "issues" as const,
} as const

declare module "zero" {
  interface StateTypes {
    [Keys.Auth]:   Signal<AuthState>
    [Keys.Issues]: Signal<IssuesState>
  }
}
```

Component code reads state via `inject(Keys.Auth)` with no generic
argument; the return type is inferred from `StateTypes`. `zero.d.ts`
declares an empty `interface StateTypes {}` and an overload
`inject<K extends keyof StateTypes>(key: K): StateTypes[K]` so user
projects extend the registry by module-augmentation without any
runtime change.

### Status-tagged signals

`examples/tracker/src/stores/auth.ts` demonstrates the §5 pattern:

```ts
export type AuthState =
  | { status: "loggedOut" }
  | { status: "loading" }
  | { status: "loggedIn"; user: User }
```

The login flow transitions through `loading`; components branch on
`.val.status`. This is the working demonstration the deferred-machines
guidance currently lacks.

### Module-scoped stores

Every store under `src/stores/` exports the signal plus the functions
that legally mutate it. Component code never calls `signal.set()` on
a store signal — only via exported mutators. The docs frame this as
the framework's answer to Redux/Pinia-style stores: a plain TS module,
no library. `examples/tracker/src/stores/issues.ts` is the canonical
reference (`listIssues()`, `addIssue()`, `updateStatus()`,
`addComment()`).

### Single signal vs many

Docs include a short decision rule and a worked example pair:

- One structured signal when fields update together or constrain each
  other. Auth is the canonical case: `user` is only meaningful when
  `status === "loggedIn"`.
- Separate signals when fields update independently. Theme and
  sidebar-open are the canonical case.

`examples/todos/` shows one structured signal (`{ items, filter }`);
`examples/tracker/` shows both shapes in different stores.

### Co-located route metadata

Each route module in `examples/tracker/` and (where applicable)
`examples/todos/` exports `meta` next to the route component:

```ts
export const meta = {
  protected: true,
  title: (data) => `Issue #${data.issue.id}`,
}

export async function load({ params, fetch }) { ... }

export default function IssuePage({ data }) { ... }
```

Docs frame this as the answer to the original Phase 11 "co-locate
config with the function" goal — the binding lives next to the code,
without decorators.

### Documentation

- **`BEST_PRACTICES.md`** at the repo root holds the prose guidance.
  Sections: Project structure, State organization, Stores, Status-
  tagged signals, Routes, Testing, Performance. Each section is short
  (a few paragraphs) and ends with `→ See examples/tracker/src/...`
  pointing at the canonical file.
- **`AGENTS.md`** gains a `## Best practices` section that summarizes
  the document and binds agent behavior to those patterns.
- **Framework spec** §5 (status-tagged signals) and §11 (`inject`)
  gain one-line forward-pointers to `BEST_PRACTICES.md`. The
  capability reference stays separate from the choice-style guidance.
- **Phase 11 entry** in §12 is rewritten to a short "deferred
  indefinitely" note that names the blocker (decorators are class-
  only in JS/TS) and points forward to Phase 12 — same shape as the
  state-machines deferral in §5.

### Route-scoped `fetch` (framework change)

The router is updated so the `fetch` it injects into `load()` carries
an `AbortSignal` bound to the route scope. Concretely:

- Each route render creates an `AbortController` owned by the route
  scope. The route scope's disposal hook calls
  `controller.abort()` so navigating away aborts in-flight requests
  automatically.
- The injected `fetch` is a thin wrapper around `window.fetch` that
  threads `controller.signal` into the request. If the caller passes
  its own `AbortSignal` in `init.signal`, the wrapper composes the
  two (an abort on either signal aborts the request).
- Behavior outside `load()` is unchanged — `window.fetch` is not
  replaced globally.
- Aborts surface as the standard `AbortError` from `fetch`. The
  router catches `AbortError` thrown during a `load()` call that
  belongs to a disposed scope and silently drops the result —
  `app.error()` is not invoked for navigation-driven aborts.

This is the only behavioral change to the framework runtime in
Phase 12 (the other being the `StateTypes` type-surface extension
point). It lives in the router under §6 of the framework spec; the
spec text gains a short subsection in §6 describing the contract.

`tests/router.test.js` gains coverage that asserts (a) navigating
mid-`load()` aborts the pending fetch, (b) a caller-supplied signal
still aborts the request, (c) the post-navigation route scope's
fetch carries a fresh, non-aborted signal.

### `"zero/http"` module (framework)

Every realistic app needs HTTP with cancellation, middleware, and
typed error mapping. Asking each adopter to copy/paste a non-trivial
wrapper diverges conventions across projects and undermines the
framework's batteries-included posture. Phase 12 ships HTTP as a
framework module on equal footing with `"zero/components"`,
`"zero/test"`, and `"zero/wc"`.

**Module path.** Imported as `import { ... } from "zero/http"`.
Tree-shakeable — apps that don't fetch pay zero cost.

**Files.**

```
runtime/
  http.js              # implementation
  http.test.js         # tests
  zero-http.d.ts       # types (mirrors zero-test.d.ts shape)
```

`zero-http.d.ts` is wired into the scaffold so it lands in `.zero/`
alongside `zero.d.ts` and `zero-test.d.ts`, refreshable via
`zero update`.

**Capabilities.**

- **Factory.** A `createHttp(opts?)` factory returns a client
  instance. No singleton, no global state — apps construct one (or
  more) client per logical backend in their `src/stores/` or
  `src/lib/`.
- **Methods.** `get<T>(url, init?)`, `post<T>(url, body?, init?)`,
  `put`, `patch`, `delete`, plus a generic `request<T>(input, init?)`.
  `T` is the parsed-response type the caller asserts.
- **JSON I/O.** Request bodies that are plain objects are
  `JSON.stringify`'d with `Content-Type: application/json`. Responses
  with a JSON content type are parsed; otherwise the raw `Response`
  is returned (escape hatch for downloads / blobs).
- **Errors.** Non-2xx responses reject with `HttpError` (exported
  class) carrying `status`, `statusText`, and the parsed response
  body if available. Network failures reject with the underlying
  `TypeError`. Aborts reject with `AbortError`.
- **Middleware.** `client.use(mw)` registers a middleware. Signature:
  `(req: Request, next: (req: Request) => Promise<Response>) =>
  Promise<Response>`. Middlewares run outermost-first on the way
  down, innermost-first on the way back up — standard onion model.
  Middlewares may short-circuit (skip `next()`), wrap (transform the
  response), or rethrow. The canonical examples shipped with the
  module's tests: an auth-header injector and a `401 → navigate
  ("/login")` redirector.
- **Cancellation.** Each request accepts `init.signal` for caller-
  supplied aborts. When called from within a `load()`, the client
  composes the caller's signal with the route-scoped signal carried
  by the injected `fetch` (see "Route-scoped `fetch`" above). The
  client owns no `AbortController` of its own.
- **Fetch injection.** `createHttp({ fetch })` accepts an optional
  fetch reference, defaulting to `globalThis.fetch`. Test code passes
  a mock; production usually omits it. Inside `load()`, callers
  forward the injected route-scoped fetch via `client.request(..., {
  fetch })` so the route-scope abort attaches automatically. (Plan
  phase resolves whether the per-call `fetch` override sits in `init`
  or in a separate parameter.)

**Integration with `load()`.** `examples/tracker/src/stores/issues.ts`
constructs a module-scoped client and `examples/tracker/src/routes/
issues/[id].ts` calls it from `load()`, threading the injected
fetch:

```ts
// stores/issues.ts
import { createHttp } from "zero/http"
import { navigate } from "zero"
export const api = createHttp().use(async (req, next) => {
  const res = await next(req)
  if (res.status === 401) navigate("/login")
  return res
})

// routes/issues/[id].ts
import { api } from "../../stores/issues"
export async function load({ params, fetch }) {
  return { issue: await api.get(`/api/issues/${params.id}`, { fetch }) }
}
```

**Tests.** `runtime/http.test.js` covers: JSON request/response round-
trip, non-2xx → `HttpError`, abort via caller signal, middleware
ordering (onion), middleware short-circuit, error mapping inside
middleware. `examples/tracker` exercises end-to-end use through
`zero test`.

`examples/todos/` uses localStorage and does not import
`"zero/http"`. `examples/counter/` does not fetch at all.

### Performance / bundle-size guidance

A `## Performance` section in `BEST_PRACTICES.md` covers, each as a
short paragraph + one example pointer:

- Lazy-load every route except the entry route (`() => import(...)`).
- Split a store when its consumers diverge.
- Keep `computed()` bodies narrow — reading wide values causes wide
  re-runs.
- Prefer `each()` with a stable key over `.map()` for any list that
  reorders or churns.

## Constraints

- Phase 12's framework changes are bounded to: (a) the `StateTypes`
  interface + typed `inject` overload in `zero.d.ts` (type-surface
  only); (b) the route-scoped `fetch` behavior in the router (small
  additive runtime change); (c) the new `"zero/http"` module
  (`runtime/http.js`, `runtime/zero-http.d.ts`, `runtime/http.test.js`)
  with the scaffold integration to land `zero-http.d.ts` in `.zero/`.
  Any pattern in the docs that needs further capability goes in Open
  Questions, not Requirements.
- No `node_modules`. Examples are pure TS + zero, like every other
  zero project.
- Examples must consume the framework as a user would: imports from
  `"zero"`, `"zero/components"`, `"zero/test"`. No direct imports of
  `runtime/` or `src/scaffold/.zero/`.
- Documentation lives in repo-root markdown files. No docs site, no
  MDX, no SSG.
- The full example must remain readable end-to-end in one sitting.
  Target: ≤ 800 LOC of TS excluding tests. If it exceeds that, cut
  scope rather than splitting into multiple full examples.
- File-structure recommendations are a strong opinion but not
  enforced by the CLI. Phase 12 is documentation + reference code,
  not a new lint surface.

## Out of Scope

- A `zero new --example=<name>` scaffold generator. Examples are
  read-only references; copy/paste or fork.
- A recommended API-mocking pattern. Each example ships its own
  fixtures — in-memory for `todos`, a static `data.json` for
  `tracker`. No MSW-style abstraction.
- Retry/backoff, response caching, request deduplication, or any
  higher-level data-fetching behaviors on top of `"zero/http"`. The
  module is a fetch wrapper, not a data-fetching library.
- GraphQL, gRPC, or other protocol-specific clients. `"zero/http"`
  is a thin layer over `fetch` — REST-shaped JSON is its happy path;
  other protocols ride on top via user code.
- Top-level re-exports of `http` from `"zero"`. Only `"zero/http"`
  exposes the module.
- Any other change to the `App` class, `signal`, `inject`'s runtime
  behavior, or framework primitives. The three surface changes are
  the `StateTypes` extension point, the route-scoped `fetch`, and
  the new `"zero/http"` module.
- Decorators (original Phase 11) as a feature — formally retired in
  favor of this work; the framework spec's Phase 11 entry is
  rewritten as a deferral note.
- E2E tests for examples. Unit only.
- A shared test-fixture library across examples.
- I18n, accessibility audits, animation guidance.

## Open Questions

- **Typed `inject` signature.** Spec proposes
  `inject<K extends keyof StateTypes>(key: K): StateTypes[K]` with
  `StateTypes` as an empty interface in `zero.d.ts`, augmented per
  project via `declare module "zero"`. The plan should confirm the
  overload preserves the existing fallback signature
  `inject<T = unknown>(key: string): T` for projects that opt out of
  the registry.
- **`tracker` data source.** Spec assumes a static JSON fixture
  fetched in `load()`. Confirm with the plan that the dev server and
  build pipeline serve static JSON correctly (likely fine —
  `src/dev/files.rs` already serves arbitrary assets); fall back to
  an in-memory store if not.
- **Per-call fetch override.** Spec requires the client to accept a
  per-call fetch (so `load()` can thread its route-scoped fetch in)
  but does not pin the placement. Two reasonable shapes:
  (a) `client.get(url, { fetch })` — overload of the standard `init`;
  (b) `client.get(url, init, { fetch })` — separate framework-options
  argument. Plan picks one; recommend (a) — `init.fetch` is non-
  standard but contained, and avoids a second positional argument.
- **Module-scoped vs route-scoped clients.** Spec assumes apps
  construct one client per backend at module scope (in `stores/` or
  `lib/`). Middleware is registered once at construction. If an app
  needs route-specific middleware (e.g., admin routes get extra
  auth headers), it constructs a second client. Confirm with the
  plan that route-scoped construction isn't a missing capability.
- **Abort vs. error in `load()`.** Spec says the router silently
  drops `AbortError` from a `load()` belonging to a disposed scope.
  Confirm with the plan that the swallow is scoped narrowly — it
  must not mask `AbortError` thrown for unrelated reasons (a caller-
  supplied signal that the developer aborted on purpose, where they
  expect their own catch to see the error).
- **Routes index.** The proposed full-example layout uses
  `routes/issues/index.ts` for the collection view and `[id].ts` for
  the detail view. Filename conventions for dynamic segments are a
  doc convention (the router takes the registered pattern, not the
  filename). The plan should confirm whether to canonize the
  `[id].ts` form (signals intent visually) or simply name the file
  `detail.ts`.
- **`AGENTS.md` vs. `BEST_PRACTICES.md` boundary.** Spec proposes
  both: `BEST_PRACTICES.md` is the long-form reference, `AGENTS.md`
  carries a summary plus the binding rules for agents. Plan should
  confirm this split is right and that the `AGENTS.md` section is
  short enough to keep loaded in context routinely.
- **Test-runner integration for examples.** `tests/component_library.rs`
  invokes the test runner against `src/scaffold/.zero/components/`.
  The plan should confirm the equivalent shape for examples (one
  Cargo test per example, invoking `zero test` from the example
  directory).
