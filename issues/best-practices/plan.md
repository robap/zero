# Plan: Best Practices & Example Applications

## Summary

Phase 12 delivers (a) three buildable example projects under `examples/`,
(b) `BEST_PRACTICES.md` plus targeted updates to `AGENTS.md` and the
framework spec, and (c) three framework changes that close the gaps the
examples expose: a typed-key registry for `inject`, route-scoped abort
signals on the `fetch` injected into `load()`, and a new `"zero/http"`
module shipped on equal footing with `"zero/components"` and
`"zero/test"`.

Sequencing: framework changes land first (each in a self-contained step
that keeps the existing tests green), then `examples/counter` validates
the example/integration-test shape, then `examples/todos` extends it.
Before the tracker work begins, a polish-and-discipline pass retrofits
`counter` and `todos` with a per-app `Header` (logo + theme toggle) —
this both ships visible polish and establishes the canonical "build your
own component from layout classes when the library doesn't ship one"
pattern that the tracker will reuse. The tracker then exercises every
pattern end-to-end. Docs land last so they can point at concrete file
paths in the shipped examples. The full example stays under the 800-LOC
TS budget by reusing the shipped `"zero/components"` library and the
design-system SCSS layer.

Two discipline rules are loud throughout the example and doc steps,
because the first pass of execution drifted on both: (1) use
`zero/components` for every interactive primitive — drop to raw HTML
only with a documented reason or when you are building a new
presentational component; (2) when you do build your own component,
wrap shipped primitives rather than re-implement them.

## Prerequisites

Resolved open-question decisions (referenced where they matter):

| Spec OQ | Decision |
| --- | --- |
| Typed `inject` signature | Two overloads, typed first: `inject<K extends keyof StateTypes>(key: K): StateTypes[K]` then `inject<T = unknown>(key: string): T`. Empty-by-default `interface StateTypes {}`. The fallback signature is preserved verbatim so projects that don't augment the registry compile unchanged. |
| `tracker` data source | Static JSON fixture at `examples/tracker/public/data.json`, fetched via `load()` through `zero/http` threading the injected route-scoped fetch. Dev server already serves `/public/*` (see `src/dev/server.rs:127`). |
| Per-call fetch override | Option (a): `client.get(url, { fetch })`. `init.fetch` lives on the standard `init` bag — non-standard but contained, avoids a second positional argument. The http client extracts `init.fetch`, falls back to its constructor-time `fetch`, and never forwards `init.fetch` to the underlying call. |
| Module-scoped vs route-scoped clients | Confirmed: apps construct one client per backend at module scope (`stores/` or `lib/`). Route-specific middleware needs are met by constructing a second client. Not a missing capability. |
| Abort vs. error in `load()` | The router swallows `AbortError` **only when** the controller it owns for the navigation has fired. Any other `AbortError` (e.g. caller-supplied signal aborted by the developer) flows through to `app.error()`. Implementation pin: tag the controller-issued abort by checking `controller.signal.aborted` at the catch site rather than matching error name alone. |
| Routes filename for dynamic segments | `routes/issues/index.ts` for `/issues`, `routes/issues/issue.ts` (singular) for `/issues/:id`. Plain names, no brackets; the singular/plural pair reads naturally. Spec example showing `[id].ts` is superseded. |
| `AGENTS.md` vs `BEST_PRACTICES.md` boundary | Confirmed: `BEST_PRACTICES.md` (repo root) is the long-form reference; `src/scaffold/AGENTS.md` (which lands in every scaffolded project) gains a short `## Best practices` section — ≤ 40 lines, no code blocks longer than ~10 lines — that names the patterns and forward-points to `BEST_PRACTICES.md` for the prose. |
| Test-runner integration for examples | One Cargo test per example in `tests/examples_build.rs` and `tests/examples_tests.rs`. Each test uses a `prepare_example(name)` helper in `tests/common/mod.rs` that mirrors `prepare_showcase`: copy the example into a tempdir, create empty `.zero/`, run `zero update --yes`, return the tempdir. |
| Component usage discipline | Examples — and the patterns the docs prescribe — use `zero/components` for every interactive primitive (`Button`, `Input`, `Checkbox`, `Toggle`, `Select`, `TextArea`, `Radio`, `Dialog`, `Tabs`, `Card`, `Avatar`, `Badge`, `Spinner`, `Toast`). Raw `<button>` / `<input>` / `<select>` are allowed in exactly two situations: (a) the shipped component genuinely cannot express the required behavior (the canonical case is `examples/todos/src/components/FilterBar.ts`, where the button variant must track a signal reactively and `Button` takes no reactive `variant` prop) — when deviating, a `//` comment names the missing capability; (b) the markup belongs to a **new presentational component the app itself is building** (the canonical case is the per-example `Header` — there is no shipped `Header` component, so each app builds one from layout classes and design tokens). Plain DOM containers (`<main>`, `<section>`, `<header>`, `<nav>`, `<ul>`, `<li>`, `<form>`, `<label>`, `<span>`, `<a>`, `<svg>`) are not "components" in this rule and are used freely. |
| Header polish & theme switching | Every example ships a per-app `src/components/Header.ts` built from design-system layout classes (`cluster`, `stack`, `pad-*`, `gap-*`) and the SCSS token surface — **not** a pre-built component. The header renders a per-example inline-SVG `Logo` component and a `ThemeToggle` component that wraps the shipped `Toggle` (light/dark only — `Toggle` is binary, so the "auto" follows-system mode is the unset state at startup and is exited as soon as the user touches the toggle). Theme state lives in `src/stores/theme.ts` as `signal<"light" \| "dark" \| null>(null)`; an `effect()` in `src/app.ts` writes `document.documentElement.dataset.theme` (or removes the attribute when `null`). The pattern is intentionally **not** lifted into `BEST_PRACTICES.md` — it is example polish and serves as a worked demonstration of building an app-specific component when the library doesn't ship one. |

Additional decisions baked into this plan that the spec does not pin
explicitly:

- **Route `load`/`meta` co-location pattern.** The spec example shows
  `load`/`meta` as named exports of the route module. Without further
  runtime change, `app.route(..., { load, meta })` is the read path —
  it consumes them from the third argument. The canonical pattern this
  plan documents: route modules **export** `load` / `meta` / `default`
  side-by-side; `src/app.ts` does
  `import Issue, { load, meta } from "./routes/issues/issue.ts"`
  and passes them through to `app.route("/issues/:id", Issue, { load, meta })`.
  Co-location is preserved (all route concerns live in the route file);
  the registration site does the wiring. No runtime extension needed.
- **HTTP module placement.** `"zero/http"` follows the `"zero/test"`
  shape rather than the `"zero/components"` shape: implementation
  ships as an embedded module body served from `/zero-http.js` (dev)
  and inlined by the bundler. Only `runtime/zero-http.d.ts` lands in
  the user's `.zero/` via the scaffold manifest. Rationale: http.js is
  framework runtime code with no need for user-visible authoring
  surface, unlike the component sources.
- **HTTP module imports.** `runtime/http.js` is self-contained — no
  imports from `"zero"`. Aborts and fetch injection are passed in by
  the caller; nothing needs framework internals.

## Steps

- [x] **Step 1: Add `StateTypes` interface + typed `inject` overload to `runtime/zero.d.ts`.**
- [x] **Step 2: Route-scoped `fetch` — add per-nav `AbortController` in `runtime/app.js` and tie its abort to nav-scope disposal.**
- [x] **Step 3: Implement `runtime/http.js` + `runtime/http.test.js` + `runtime/zero-http.d.ts`.**
- [x] **Step 4: Wire `"zero/http"` through build/dev/test pipelines and the scaffold framework manifest.**
- [x] **Step 5: Add `examples/counter/` (minimal example) and its `home.test.ts`.**
- [x] **Step 6: Add `tests/examples_build.rs`, `tests/examples_tests.rs`, and `prepare_example()` helper — exercising counter only.**
- [x] **Step 7: Add `examples/todos/` (list + filter + localStorage, typed key registry, single structured signal) and extend the integration tests.**
- [x] **Step 8: Add per-example header polish (`Header` + `Logo` + `ThemeToggle` + theme store) to `examples/counter/` and `examples/todos/`.**
- [x] **Step 9: `examples/tracker/` — scaffolding, layout, auth store, login route, route guard middleware, header from the start.**
- [x] **Step 10: `examples/tracker/` — issues store using `zero/http`, list / detail / comment routes, `public/data.json` fixture, route tests, and extend the integration tests.**
- [x] **Step 11: Write `BEST_PRACTICES.md` at the repo root.**
- [x] **Step 12: Add `## Best practices` section to `src/scaffold/AGENTS.md` and extend the section-sentinel test.**
- [x] **Step 13: Update the framework spec — §5 + §11 forward pointers, §6 route-scoped fetch + §11 `"zero/http"` API surface, §12 Phase 11 deferral note and Phase 12 close-out.**

### Revisit (init/update gap)

After Steps 1–13 shipped, two follow-up gaps surfaced:

1. The examples can't be run by a fresh clone — they ship without `.zero/`
   (correctly gitignored, materialized by `prepare_example` in tests), and
   neither `zero init` (refuses when `zero.toml` already exists) nor
   `zero update` (bails when `.zero/` is missing) will bootstrap one.
2. Every example's `zero.toml` uses `root = "."`, which the `zero init`
   wizard does not produce (it defaults to `root = "web"` and validates
   against leading-`.` segments). The examples therefore advertise a
   shape the canonical bootstrap flow never emits, contradicting the
   "best practice" framing.

Resolution: (a) make `zero update` self-bootstrap a missing `.zero/`
directory so any valid `zero.toml` project gets a one-command path to a
runnable framework tree; (b) restructure each example to the canonical
subdir layout (`examples/<name>/zero.toml` + `examples/<name>/web/...`)
so the on-disk shape mirrors `zero init`'s output; (c) document the
bootstrap-via-update flow and the subdir layout in `BEST_PRACTICES.md`
and `src/scaffold/AGENTS.md`. The `showcase/` project also uses
`root = "."` but is intentionally **out of scope** for this revisit —
the user's call-out was the examples; showcase stays put and is noted
in Risks.

- [x] **Step 14: `zero update` self-bootstraps a missing `.zero/` directory.**
- [x] **Step 15: Restructure `examples/counter|todos|tracker/` to the canonical `<example>/web/` subdir layout (`root = "web"`).**
- [x] **Step 16: Simplify `prepare_example` helper to drop the manual `.zero/` mkdir, and re-green `tests/examples_build.rs` + `tests/examples_tests.rs` against the new layout.**
- [x] **Step 17: Document the bootstrap-via-update flow and the subdir layout in `BEST_PRACTICES.md` and `src/scaffold/AGENTS.md`; add a one-line note to the framework spec.**
- [x] **Step 18: Redefine the `.split` layout primitive as a flex with `justify-content: space-between`, simplify each example's `Header`, and update the layout-primitive docs.**
- [x] **Step 19: Move the tracker's HTTP client construction to `lib/api.ts` and its middleware registration to `app.ts`; update `BEST_PRACTICES.md` §6 and the scaffold AGENTS.md note.**

---

## Step Details

### Step 1: Add `StateTypes` interface + typed `inject` overload

**Goal:** Give projects a zero-runtime, module-augmentation–driven way to
type `inject()`. Land this first because it is purely additive to the
type surface and every other step that touches `inject()` benefits from
it. After this step the existing scaffold continues to compile and run;
new projects can opt in via `declare module "zero"`.

**Files:**

- `runtime/zero.d.ts` — module surface.
- `src/runtime.rs` — no code change, but the existing
  `zero_types_body_declares_every_public_runtime_export` test must
  still pass.
- (Existing scaffold tests in `src/scaffold.rs` already assert
  `declare module "zero"` is present; verify still true.)

**Changes:**

In `runtime/zero.d.ts`, inside the existing `declare module "zero"`
block, replace the single `inject` signature with:

```ts
export interface StateTypes {}

export function inject<K extends keyof StateTypes>(key: K): StateTypes[K];
export function inject<T = unknown>(key: string): T;
```

The typed overload is listed **first** so TypeScript picks it for keys
that exist in an augmented `StateTypes`. The fallback overload preserves
the existing behavior for callers passing arbitrary strings (the
showcase's `inject<Signal<ThemeMode>>("theme")` keeps compiling unchanged
because TS resolves it against the second signature when no
augmentation exists for `"theme"`).

**Tests:**

- `runtime/dom-shim.test.js` and existing app tests must continue to
  pass — `inject()` runtime is unchanged.
- Add a type-check smoke that lives in the `examples/todos/` step (a
  `.ts` file that uses `Keys.Items` and reads back a `Signal<TodosState>`
  must transpile without a generic argument). No explicit `tsc`
  invocation in this step — the swc transpile pipeline does not
  type-check, so the assertion is a manual review point now, validated
  by usage in later steps.
- The Rust-side `zero_types_body_declares_every_public_runtime_export`
  test in `src/runtime.rs` continues to find `inject` in the body — no
  change.

---

### Step 2: Route-scoped `fetch` in the router

**Goal:** When `load()` runs and the user navigates away mid-load, the
fetch the load is awaiting aborts automatically, and the router silently
drops the resulting `AbortError`. Independent of any example — it
changes the semantics of every existing `load()` consumer, so it lands
before examples that rely on it.

**Files:**

- `runtime/app.js` — wrap `globalThis.fetch` in `_navigateTo`.
- `runtime/router.test.js` — new tests covering abort behavior.
- `runtime/app.test.js` — keep existing nav tests green.

**Changes:**

1. In `runtime/app.js::_navigateTo`, immediately after
   `this._navScope = _createScope();`, create an `AbortController`:

   ```js
   const navController = new AbortController();
   app._navScope.run(() => {
     // Tie abort to scope disposal. _createScope's onCleanup hook
     // (or equivalent) — confirm name when implementing.
     onCleanup(() => navController.abort());
   });
   ```

   If the scope API does not yet expose an `onCleanup`, add a minimal
   `scope.onCleanup(fn)` helper to `runtime/reactivity.js`. The
   existing `dispose()` already iterates a teardown list; the new helper
   appends to it. Cover with a unit test in `runtime/reactivity.test.js`.

2. Build the route-scoped fetch wrapper:

   ```js
   const routeFetch = (input, init = {}) => {
     const callerSignal = init.signal;
     const signal = callerSignal
       ? _composeSignals(navController.signal, callerSignal)
       : navController.signal;
     return globalThis.fetch(input, { ...init, signal });
   };
   ```

   `_composeSignals` is a small helper added at the top of `app.js`
   that returns an `AbortSignal` aborted when either input aborts (use
   `AbortSignal.any([a, b])` where available; fall back to manual
   composition via a fresh `AbortController` listening to both inputs).

3. Pass `routeFetch` in place of `globalThis.fetch?.bind(globalThis)`
   on every `load()` invocation (currently one site,
   `runtime/app.js:430`).

4. In the existing `catch (err)` block of `_navigateTo`, before checking
   `app._error`, test:

   ```js
   if (err?.name === "AbortError" && navController.signal.aborted) {
     return;
   }
   ```

   The combined check ensures the swallow fires **only** for the
   controller this navigation owns. Caller-supplied aborts (where the
   caller's controller aborted but `navController` did not) propagate
   to `app._error`.

5. Behavior outside `load()` is unchanged. `globalThis.fetch` is not
   monkey-patched. Components that call `fetch` directly receive no
   route-scoped signal.

**Tests:**

- `runtime/router.test.js`:
  - `route_scoped_fetch_aborts_when_navigated_away` — a `load()` that
    awaits a never-resolving fetch; programmatic `navigate()` to a
    different route; the original fetch's signal must be `aborted`.
  - `route_scoped_fetch_composes_caller_signal` — caller passes an
    `AbortController.signal`; aborting the caller's controller aborts
    the request even though the route is still current; the
    `AbortError` reaches `app.error()` (not silently dropped).
  - `route_scoped_fetch_is_fresh_after_navigation` — after a successful
    nav, the next nav's injected fetch carries a new, non-aborted
    signal.
- `runtime/reactivity.test.js`: `scope_oncleanup_runs_on_dispose`.

---

### Step 3: Implement `runtime/http.js` + tests + types

**Goal:** Ship the runtime, tests, and type declarations for `"zero/http"`.
This step adds files but does not yet wire the module specifier into the
build, dev, or test pipelines (Step 4) — it stays an isolated unit-tested
addition that the rest of the codebase doesn't see yet.

**Files:**

- `runtime/http.js` — new module.
- `runtime/http.test.js` — new test file.
- `runtime/zero-http.d.ts` — new type declarations.

**Changes:**

1. **`runtime/http.js`** exports:

   ```js
   export class HttpError extends Error {
     constructor(status, statusText, body) { ... this.status = status; this.body = body; }
   }

   /**
    * @typedef {(req: Request, next: (req: Request) => Promise<Response>) => Promise<Response>} Middleware
    */

   export function createHttp(opts = {}) {
     const baseFetch = opts.fetch ?? globalThis.fetch;
     const middlewares = [];
     const client = {
       use(mw) { middlewares.push(mw); return client; },
       get:    (url, init) => request(client, "GET", url, undefined, init),
       post:   (url, body, init) => request(client, "POST", url, body, init),
       put:    (url, body, init) => request(client, "PUT", url, body, init),
       patch:  (url, body, init) => request(client, "PATCH", url, body, init),
       delete: (url, init) => request(client, "DELETE", url, undefined, init),
       request: (input, init) => requestFromInit(client, input, init),
       _baseFetch: () => baseFetch,
       _middlewares: () => middlewares,
     };
     return client;
   }
   ```

   Internal helpers:

   - `request(client, method, url, body, init)` builds a `Request`
     object (`init.fetch` is extracted before constructing it),
     serializes a plain-object `body` as JSON and sets
     `Content-Type: application/json`, and dispatches through the
     middleware chain.
   - `requestFromInit(client, input, init)` — the `request<T>` method,
     accepts a `Request | URL | string` and threads `init.fetch`.
   - `dispatch(req, mws, baseFetch)` — onion-walks the middlewares.
     Innermost layer calls `baseFetch(req)`.
   - Final post-middleware step: if the response is JSON
     (`Content-Type: application/json` or `+json`), parse and return
     the body as `T`; if non-2xx, reject with `HttpError`; otherwise
     return the raw `Response` (escape hatch).
   - Error mapping: network failures surface the underlying `TypeError`
     unwrapped; `AbortError` propagates unchanged so the route-scoped
     swallow in Step 2 catches it.

2. **`runtime/http.test.js`** — covers:

   - JSON request/response round-trip (plain object body → JSON; JSON
     response → parsed object).
   - Non-2xx → rejects with `HttpError` carrying status + parsed body.
   - Abort via caller signal → rejects with `AbortError`.
   - Middleware ordering: a chain of three middlewares logs entry/exit
     order; assertion is `["A in","B in","C in","C out","B out","A out"]`.
   - Middleware short-circuit: a middleware that returns a `Response`
     without calling `next()` skips the base fetch entirely.
   - Middleware-injected header: a middleware that adds
     `Authorization: Bearer x` is observed by a stub fetch.
   - Per-call fetch override: `client.get(url, { fetch: stub })`
     dispatches through `stub`, not the constructor-time fetch.
   - Test fetch stub: a `makeStubFetch(handler)` helper at the top of
     the test file accepts a function `(req) => Response | Promise<Response>`.

3. **`runtime/zero-http.d.ts`** — mirrors `runtime/zero-test.d.ts`:

   ```ts
   declare module "zero/http" {
     export class HttpError extends Error {
       readonly status: number;
       readonly statusText: string;
       readonly body: unknown;
     }
     export interface HttpClient {
       use(mw: Middleware): HttpClient;
       get<T = unknown>(url: string, init?: HttpInit): Promise<T>;
       post<T = unknown>(url: string, body?: unknown, init?: HttpInit): Promise<T>;
       put<T = unknown>(url: string, body?: unknown, init?: HttpInit): Promise<T>;
       patch<T = unknown>(url: string, body?: unknown, init?: HttpInit): Promise<T>;
       delete<T = unknown>(url: string, init?: HttpInit): Promise<T>;
       request<T = unknown>(input: Request | URL | string, init?: HttpInit): Promise<T>;
     }
     export interface HttpInit extends RequestInit {
       fetch?: typeof fetch;
     }
     export type Middleware = (req: Request, next: (req: Request) => Promise<Response>) => Promise<Response>;
     export function createHttp(opts?: { fetch?: typeof fetch }): HttpClient;
   }
   ```

**Tests:**

- `node --test runtime/http.test.js` — covered above.
- Existing tests stay green (no code-path change in the rest of the
  runtime).

---

### Step 4: Wire `"zero/http"` through build/dev/test pipelines and the scaffold

**Goal:** Make `import { createHttp } from "zero/http"` resolve in all
three module pipelines and add `zero-http.d.ts` to the scaffold's
framework manifest so user projects pick up the types.

**Files:**

- `build.rs` — emit `zero_http_body.js` and `zero_http_types_body.d.ts`.
- `src/runtime.rs` — embed the http body + types; add `http_module()`
  builder and `ZERO_HTTP_EXPORTS` list.
- `src/scaffold.rs` — add `.zero/zero-http.d.ts` to
  `framework_manifest()`; update expected-path set in
  `framework_manifest_matches_expected_path_set` and the tsconfig
  include test.
- `src/scaffold/tsconfig.json` — include `.zero/zero-http.d.ts`.
- `src/build/resolver.rs::resolve` — add the `"zero/http"` branch.
- `src/dev/inject.rs::dev_scripts` — extend importmap to include
  `"zero/http":"/zero-http.js"`.
- `src/dev/server.rs` — add a `/zero-http.js` route handler
  (mirroring `/zero.js`).
- `src/test_runner/loader.rs` — register `"zero/http"` against the
  embedded module body, mirroring the `"zero/test"` arm at
  `loader.rs:240`.
- `src/build/bundler.rs` — confirm the bundler picks up the embedded
  body; if `"zero"` is treated as a synthetic module that's inlined at
  bundle time, replicate for `"zero/http"`. The existing pattern for
  `"zero"` in `bundler.rs` is the template.

**Changes:**

1. `build.rs`:
   - Add `runtime/http.js` to `RUNTIME_FILES`? **No** — `RUNTIME_FILES`
     concatenates into a single `zero` module. http is a sibling
     module. Instead, add a parallel pipeline that reads
     `runtime/http.js` through `clean_runtime_source` and writes
     `zero_http_body.js`. Same for `runtime/zero-http.d.ts` →
     `zero_http_types_body.d.ts`. Add `cargo:rerun-if-changed` lines.

2. `src/runtime.rs`:
   - `pub const ZERO_HTTP_BODY: &str = include_str!(concat!(env!("OUT_DIR"), "/zero_http_body.js"));`
   - `pub const ZERO_HTTP_TYPES_BODY: &str = include_str!(concat!(env!("OUT_DIR"), "/zero_http_types_body.d.ts"));`
   - `pub const ZERO_HTTP_EXPORTS: &[&str] = &["createHttp", "HttpError"];`
   - `pub fn http_module() -> String { ... }` analogous to
     `runtime_module()`: body + trailing `export { createHttp, HttpError };`.
   - New tests in the `#[cfg(test)] mod tests` block:
     - `http_module_contains_create_http_factory`
     - `http_module_ends_with_aggregate_export_block`
     - `zero_http_types_body_declares_every_public_export`
     - `http_body_has_no_top_level_imports`

3. `src/scaffold.rs::framework_manifest`:
   - Add `(".zero/zero-http.d.ts", crate::runtime::ZERO_HTTP_TYPES_BODY)`.
   - Update `framework_manifest_matches_expected_path_set`'s expected
     set (add the new path).
   - Extend `tsconfig_include_contains_components_dts` or add a new
     test asserting `.zero/zero-http.d.ts` is in tsconfig include.

4. `src/scaffold/tsconfig.json` — add `.zero/zero-http.d.ts` to
   `include`.

5. `src/build/resolver.rs::resolve` — add before the relative branch:

   ```rust
   if specifier == "zero/http" {
       return Ok(ModuleId::Synthetic("zero/http"));
   }
   ```

   (If the existing `"zero"` resolution returns `Synthetic("zero")`,
   match that shape; if it returns a different variant, mirror that.
   Inspect resolver.rs's `ModuleId` enum during execution.)

   Add `zero_http_resolves_to_synthetic` test.

6. `src/dev/inject.rs::dev_scripts` — update importmap literal:

   ```rust
   r#"<script type="importmap">{"imports":{"zero":"/zero.js","zero/components":"/.zero/components/index.ts","zero/http":"/zero-http.js"}}</script>"#
   ```

   Add `dev_scripts_importmap_contains_zero_http`.

7. `src/dev/server.rs` — add before fallback:

   ```rust
   .route("/zero-http.js", get(serve_http_runtime))
   ```

   `serve_http_runtime` returns `crate::runtime::http_module()` with
   `Content-Type: application/javascript`.

8. `src/test_runner/loader.rs::resolve` — duplicate the
   `"zero/test"` arm (lines ~240–251) for `"zero/http"`, using a new
   `self.http_src` field on the loader populated in `new()` from
   `crate::runtime::http_module()`. Add `zero_http_resolves_in_loader`
   and `zero_http_cached_after_load` tests by extending the existing
   `zero_components_resolves` pattern.

9. `src/build/bundler.rs` — confirm wiring with an integration check
   in `tests/build_full.rs` or a new bundler test that a project
   importing `"zero/http"` produces a bundle containing
   `function createHttp(`. If the bundler currently inlines `"zero"` by
   matching a specific `ModuleId` variant in its emit loop, add the
   `zero/http` branch alongside.

**Tests:**

- All the per-step tests listed above.
- `tests/component_library.rs`-style smoke is unnecessary here; the
  examples that follow exercise the wiring end-to-end.

---

### Step 5: `examples/counter/`

**Goal:** The smallest possible self-contained example, primarily to (a)
validate the example/integration-test shape, (b) demonstrate the bare
minimum: `signal`, `app.state`, `inject`, `app.route`, `html`. Target
~50 lines of TS.

**Files (created):**

- `examples/counter/zero.toml`
- `examples/counter/index.html`
- `examples/counter/tsconfig.json` (copy of `src/scaffold/tsconfig.json`)
- `examples/counter/src/app.ts`
- `examples/counter/src/routes/home.ts`
- `examples/counter/src/routes/home.test.ts`
- `examples/counter/styles/app.scss`
- `examples/counter/.gitignore` (`.zero/`, `dist/`)

**Changes:**

- `examples/counter/zero.toml` — same shape as `showcase/zero.toml`
  but with a distinct dev port (`5180` for counter, `5181` for todos,
  `5182` for tracker — easy mental model when running multiple).
- `examples/counter/src/app.ts`:

  ```ts
  import { App, signal } from "zero";
  import Home from "./routes/home.ts";

  const app = new App();
  app.state("count", signal(0));
  app.route("/", Home);
  app.run("#app");
  ```

- `examples/counter/src/routes/home.ts` — keeps the same shape as the
  scaffold's `home.ts` (Counter component, increment button).
- `examples/counter/src/routes/home.test.ts` — mirrors the scaffold's
  `home.test.ts`.
- Do **not** ship `.zero/` in the repo — gitignored, materialized by
  `prepare_example` in Step 6.

**Tests:**

Run `zero build` and `zero test` against the counter example locally
during step verification. Automated coverage lands in Step 6.

---

### Step 6: Integration tests for examples (counter-only at this point)

**Goal:** Wire the build + test integration tests for examples,
including a reusable helper, so subsequent example steps only need to
add an entry rather than touch the test infrastructure.

**Files:**

- `tests/common/mod.rs` — add `prepare_example(name: &str)`.
- `tests/examples_build.rs` — new file.
- `tests/examples_tests.rs` — new file.

**Changes:**

1. `tests/common/mod.rs` — add:

   ```rust
   pub fn prepare_example(name: &str) -> tempfile::TempDir {
       let tmp = tempfile::tempdir().unwrap();
       let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
       let src = repo.join("examples").join(name);
       copy_dir_filtered(&src, tmp.path(), &[".zero", "dist", "node_modules"]);
       std::fs::create_dir_all(tmp.path().join(".zero")).unwrap();
       assert_cmd::Command::cargo_bin("zero")
           .unwrap()
           .arg("update")
           .arg("--yes")
           .current_dir(tmp.path())
           .assert()
           .success();
       tmp
   }
   ```

2. `tests/examples_build.rs`:

   ```rust
   mod common;
   #[test] fn counter_builds() { build_example("counter"); }
   // todos + tracker added in their steps
   fn build_example(name: &str) { ... }
   ```

   The `build_example` helper invokes `zero build` and asserts:
   - exit success;
   - `dist/index.html` exists;
   - `dist/assets/app.<hash>.js` exists and is non-empty.

3. `tests/examples_tests.rs`:

   ```rust
   mod common;
   #[test] fn counter_tests_pass() { run_example_tests("counter"); }
   fn run_example_tests(name: &str) { ... }
   ```

   `run_example_tests` invokes `zero test`, asserts success and
   `0 failed` in stdout.

**Tests:**

The two new files **are** the tests. They start out covering only
counter; Steps 7 and 9 add `#[test]` functions for `todos` and
`tracker`.

---

### Step 7: `examples/todos/`

**Goal:** Mid-size example. Introduces the canonical layout primitives
that the smallest example skips: `src/state.ts` typed-key registry, a
`src/stores/todos.ts` module store, `each()` keyed rendering, a
structured single signal (`{ items, filter }`), localStorage
persistence, one form.

**Files (created):**

- `examples/todos/zero.toml` (port 5181)
- `examples/todos/index.html`
- `examples/todos/tsconfig.json`
- `examples/todos/.gitignore`
- `examples/todos/styles/app.scss`
- `examples/todos/src/app.ts`
- `examples/todos/src/state.ts`
- `examples/todos/src/stores/todos.ts`
- `examples/todos/src/lib/storage.ts`
- `examples/todos/src/components/TodoItem.ts`
- `examples/todos/src/components/AddTodoForm.ts`
- `examples/todos/src/components/FilterBar.ts`
- `examples/todos/src/routes/home.ts`
- `examples/todos/src/routes/home.test.ts`
- `examples/todos/src/stores/todos.test.ts`
- `examples/todos/src/components/TodoItem.test.ts`
- `examples/todos/src/components/AddTodoForm.test.ts`

**Changes:**

- `src/state.ts` — the typed-key registry pattern as the spec lays out
  (`Keys.Todos` constant + `declare module "zero"` augmentation pinning
  `[Keys.Todos]: Signal<TodosState>`).
- `src/stores/todos.ts` — exports `todos: Signal<TodosState>` plus
  mutators `addTodo`, `toggleTodo`, `deleteTodo`, `editTodo`,
  `setFilter`. Internally calls `todos.set(...)` / `todos.update(...)`.
  Reads/writes localStorage via `src/lib/storage.ts`. On module load,
  hydrates from storage; an `effect()` registered in `app.ts` persists
  changes on every update.
- `src/lib/storage.ts` — thin wrapper around `localStorage.getItem` /
  `setItem` with JSON serialization and a quiet try/catch (storage
  errors are ignored — the state in-memory still works).
- Components import the **store** module's mutator functions; they
  never call `.set()` on the signal directly. This is the canonical
  rule documented in `BEST_PRACTICES.md`.
- `src/app.ts` registers `app.state(Keys.Todos, todos)` and reads
  `Keys.Todos` via `inject` in components.
- `home.ts` renders `<AddTodoForm />`, `<FilterBar />`, then the
  filtered list via `each()` with a stable key (todo `id`).

**Tests:**

- `home.test.ts` — renders the page with seeded state; asserts the
  initial list renders; types into the add form and presses Enter;
  asserts the new item appears.
- `stores/todos.test.ts` — direct exercise of mutators against an
  isolated `signal()` (the store module exposes a `_resetForTest()`
  hook or the test imports the mutators and constructs a fresh signal —
  decide during execution; prefer the fresh-signal-per-test pattern so
  test isolation is automatic).
- `TodoItem.test.ts` — toggling fires `toggleTodo`; the spy matcher
  documented in `zero-test.d.ts` confirms call shape.
- `AddTodoForm.test.ts` — empty-input submit is a no-op; non-empty
  fires `addTodo`.

**Integration test update:**

In `tests/examples_build.rs` and `tests/examples_tests.rs`, add:

```rust
#[test] fn todos_builds() { build_example("todos"); }
#[test] fn todos_tests_pass() { run_example_tests("todos"); }
```

---

### Step 8: Header + theme polish for counter & todos

**Goal:** Retrofit the two existing examples with a per-app header
(logo + theme toggle) before the tracker work begins. This step is
about polish *and* about establishing the canonical "build your own
component when the library doesn't ship one" pattern that the tracker
will reuse. Each example ends this step visibly branded, theme-
switchable, and a useful reference for the `Header` pattern.

**Files (counter):**

- `examples/counter/src/components/Header.ts` (new)
- `examples/counter/src/components/Logo.ts` (new — per-example SVG)
- `examples/counter/src/components/ThemeToggle.ts` (new)
- `examples/counter/src/components/Header.test.ts` (new)
- `examples/counter/src/components/ThemeToggle.test.ts` (new)
- `examples/counter/src/stores/theme.ts` (new)
- `examples/counter/src/app.ts` (modified — register theme state + effect, mount header)
- `examples/counter/src/routes/home.ts` (modified — drop top-level `<h1>` if duplicated by header; otherwise unchanged)
- `examples/counter/styles/app.scss` (modified — `@use` an `_app.scss` with header-specific tokens / a minimal `.app-header` rule if layout classes alone are insufficient)

**Files (todos):**

- `examples/todos/src/components/Header.ts` (new — distinct logo, same shape)
- `examples/todos/src/components/Logo.ts` (new)
- `examples/todos/src/components/ThemeToggle.ts` (new)
- `examples/todos/src/components/Header.test.ts` (new)
- `examples/todos/src/components/ThemeToggle.test.ts` (new)
- `examples/todos/src/stores/theme.ts` (new)
- `examples/todos/src/state.ts` (modified — add `Keys.Theme` and the augmentation entry)
- `examples/todos/src/app.ts` (modified — register theme state + effect, mount header)
- `examples/todos/src/routes/home.ts` (modified — drop the `<h1>Todos</h1>` because the header owns the brand)
- `examples/todos/styles/app.scss` (modified — same as counter)

**Changes:**

1. **`src/stores/theme.ts`** (identical shape per example, copied
   intentionally — examples are self-contained references):

   ```ts
   import { signal } from "zero";
   import type { Signal } from "zero";

   export type Theme = "light" | "dark" | null;

   /** `null` means "follow the system preference" (no `data-theme` attr). */
   export const theme: Signal<Theme> = signal<Theme>(null);

   export function setTheme(t: Theme): void { theme.set(t); }
   ```

2. **`src/components/Logo.ts`** (per-example SVG mark — the user
   decision pinned the marks as illustrative of the app's purpose):

   - `counter`: a stylized digit `0` glyph in a 24×24 box (the
     zero-framework "0" — counter's domain is a digit). Two-color
     SVG using `currentColor` for the stroke so the logo inherits
     text color from `data-theme`.
   - `todos`: a checkmark inside a rounded square (24×24, same
     `currentColor` rule).
   - `tracker` (in Step 9): a small ticket / bug glyph (24×24, same
     rule). Defined in Step 9 alongside the tracker header.

   Each `Logo` exports `default function Logo(): TemplateResult` and
   returns the inline `<svg>` literal. No props — the brand is fixed.

3. **`src/components/ThemeToggle.ts`**:

   ```ts
   import { html, signal, effect } from "zero";
   import type { TemplateResult } from "zero";
   import { Toggle } from "zero/components";
   import { theme, setTheme } from "../stores/theme.ts";

   /**
    * ThemeToggle — light/dark switch built on the shipped `Toggle`
    * component. Wraps a local `Signal<boolean>` (`true` = dark) and
    * mirrors changes into the app-level theme store, so the underlying
    * `Toggle` keeps its standard signal-binding contract. The startup
    * state ("follow system") is encoded as `theme.val === null`; the
    * toggle reads system preference once on mount to seed its initial
    * position.
    */
   export default function ThemeToggle(): TemplateResult {
     const initial = theme.val ?? (matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light");
     const dark = signal(initial === "dark");
     effect(() => { setTheme(dark.val ? "dark" : "light"); });
     return Toggle({ checked: dark, label: "Dark mode" });
   }
   ```

   Note on the rule: `ThemeToggle` itself is an **app-built
   component**, but the interactive primitive inside it is the shipped
   `Toggle`. This is the canonical "wrap, don't replace" pattern.

4. **`src/components/Header.ts`** (per-example; counter shown,
   todos differs only in the title text and the imported `Logo`):

   ```ts
   import { html } from "zero";
   import type { TemplateResult } from "zero";
   import Logo from "./Logo.ts";
   import ThemeToggle from "./ThemeToggle.ts";

   /**
    * App-level header — built from design-system layout classes
    * (cluster, pad-md, gap-md) and tokens, not from a shipped
    * component. The framework does not ship a `Header` because the
    * brand and contents are inherently app-specific; this is the
    * worked example of building your own.
    */
   export default function Header(): TemplateResult {
     return html`
       <header class="app-header cluster pad-md gap-md">
         <a class="app-header-brand cluster gap-sm" href="/">
           ${Logo()}
           <span class="app-header-title">counter</span>
         </a>
         <div class="app-header-spacer"></div>
         ${ThemeToggle()}
       </header>
     `;
   }
   ```

   `app-header`, `app-header-brand`, `app-header-spacer`, and
   `app-header-title` are example-local SCSS classes. They live in a
   sibling `styles/_header.scss` file `@use`d from `styles/app.scss`
   (or inlined into `app.scss` — file split is at executor's
   discretion as long as the tokens used are from
   `.zero/styles/_tokens.scss`, not hand-rolled hex). Each rule is a
   thin wrapper over tokens — `justify-content: space-between;`,
   `border-bottom: 1px solid var(--color-border);`,
   `background: var(--color-surface);` — to keep the
   "layout-classes-plus-tokens" demonstration honest.

5. **`src/app.ts`** changes (counter shown; todos is the same shape
   layered on its existing app.ts):

   ```ts
   import { App, signal, effect, html } from "zero";
   import Header from "./components/Header.ts";
   import { theme } from "./stores/theme.ts";
   import Home from "./routes/home.ts";

   const app = new App();
   app.state("count", signal(0));

   // Reflect theme into the document. `null` = follow system pref.
   effect(() => {
     const t = theme.val;
     if (t === null) document.documentElement.removeAttribute("data-theme");
     else document.documentElement.dataset.theme = t;
   });

   // Layout wraps every route with the persistent header.
   app.layout(({ children }) => html`${Header()}<main class="app-main">${children}</main>`);
   app.route("/", Home);
   app.run("#app");
   ```

   `examples/todos/src/app.ts` keeps its existing `persistTodos()`
   effect and registers `Keys.Theme` alongside `Keys.Todos` — the
   theme store augments the `StateTypes` registry per the canonical
   pattern.

   **Routes drop their duplicate `<h1>`.** With the header owning the
   brand, the per-route `<h1>` in `examples/todos/src/routes/home.ts`
   ("Todos") is removed. `examples/counter` previously rendered
   `<h1>Counter</h1>` inside home; remove it.

6. **Theme registration in `examples/todos/src/state.ts`**:

   ```ts
   import type { Signal } from "zero";
   import type { TodosState } from "./stores/todos.ts";
   import type { Theme } from "./stores/theme.ts";

   export const Keys = {
     Todos: "todos" as const,
     Theme: "theme" as const,
   } as const;

   declare module "zero" {
     interface StateTypes {
       [Keys.Todos]: Signal<TodosState>;
       [Keys.Theme]: Signal<Theme>;
     }
   }
   ```

   Counter does not have a typed registry (the spec / Step 5 keeps
   counter minimal and string-keyed); its theme store is imported
   directly, not via `inject(Keys.Theme)`. This is intentional —
   counter is the "before" picture; todos shows the registry payoff.

**Tests:**

- `Header.test.ts` (each example) — renders the header, asserts the
  brand link, the logo `<svg>`, and the `ThemeToggle` mount. Uses the
  shipped test helpers per `zero-test.d.ts`.
- `ThemeToggle.test.ts` (each example) — flipping the toggle updates
  `theme.val` from `"light"` ↔ `"dark"`. The first flip exits the
  null/system-preference mode unconditionally.
- Existing `home.test.ts` for both examples is updated only if the
  removed `<h1>` participates in an assertion (counter probably; todos
  asserts list rendering, not the heading — verify at execution time).
- Existing `tests/examples_build.rs` + `tests/examples_tests.rs`
  entries already cover counter and todos; no new Cargo tests in
  this step — the build + test sweep simply continues to pass.

**Constraints:**

- No new `zero/components` modules. The `Header`, `Logo`, and
  `ThemeToggle` live inside each example and demonstrate the
  app-built-component pattern.
- No theme-handling section in `BEST_PRACTICES.md` (Prereqs
  decision). This step's output speaks for itself in code.
- The `app-header*` SCSS rules in each example total ≤ ~25 lines.
  Any more and the demonstration starts looking like a missing
  framework feature rather than light app polish.

---

### Step 9: `examples/tracker/` — scaffolding, layout, auth, login

**Goal:** Land the tracker's skeleton: directory structure, design-
system styles, the auth store demonstrating the status-tagged signal,
the login route, the protected-route middleware, the dashboard
layout, and the per-app `Header` (logo + theme toggle) from the
start. The example builds and passes tests at the end of this step
with a working login → empty dashboard flow.

**Files (created):**

- `examples/tracker/zero.toml` (port 5182)
- `examples/tracker/index.html`
- `examples/tracker/tsconfig.json`
- `examples/tracker/.gitignore`
- `examples/tracker/styles/app.scss`
- `examples/tracker/styles/_header.scss` (optional split — see Step 8)
- `examples/tracker/src/app.ts`
- `examples/tracker/src/state.ts`
- `examples/tracker/src/stores/auth.ts`
- `examples/tracker/src/stores/auth.test.ts`
- `examples/tracker/src/stores/theme.ts`
- `examples/tracker/src/lib/guards.ts`
- `examples/tracker/src/components/DashboardLayout.ts`
- `examples/tracker/src/components/Header.ts`
- `examples/tracker/src/components/Header.test.ts`
- `examples/tracker/src/components/Logo.ts`
- `examples/tracker/src/components/ThemeToggle.ts`
- `examples/tracker/src/components/ThemeToggle.test.ts`
- `examples/tracker/src/routes/home.ts`
- `examples/tracker/src/routes/login.ts`
- `examples/tracker/src/routes/login.test.ts`

**Changes:**

- `src/state.ts` — declares `Keys.Auth` and `Keys.Theme`. The
  `Issues` augmentation references `IssuesState` from the store that
  Step 10 will create; since this step does not create `issues.ts` yet,
  either (a) include `Keys.Issues` and use a forward `// import type`
  reference (TypeScript module-augmentation tolerates this), or (b)
  add only `Keys.Auth` + `Keys.Theme` here and let Step 10 extend the
  registry. **Plan picks (b)** — fewer cross-step compilation worries.
  Step 10's diff to `state.ts` adds the `Issues` entry.
- `src/stores/theme.ts`, `src/components/Logo.ts`,
  `src/components/ThemeToggle.ts`, `src/components/Header.ts`,
  `styles/app.scss` (+ optional `styles/_header.scss`) — same shape as
  Step 8 (counter / todos), with a tracker-specific `Logo` (ticket /
  bug glyph per the user decision) and `app-header-title` text of
  `"tracker"`. The `Header` is mounted in the root `app.layout(...)`
  so it shows on `/`, `/login`, the dashboard, and every detail
  route. The header is intentionally outside the dashboard's
  `DashboardLayout` (which is a nested-route layout) — it owns global
  brand + theme; `DashboardLayout` owns the sidebar nav.
- `src/stores/auth.ts` — exports:

  ```ts
  export type User = { id: string; name: string };
  export type AuthState =
    | { status: "loggedOut" }
    | { status: "loading" }
    | { status: "loggedIn"; user: User };

  export const auth: Signal<AuthState> = signal({ status: "loggedOut" });
  export async function login(name: string): Promise<void> { ... }
  export function logout(): void { auth.set({ status: "loggedOut" }); }
  ```

  `login` transitions through `loading` (the documented §5 demo),
  resolves after a `setTimeout(..., 200)` simulating a network call,
  then sets `loggedIn`.
- `src/lib/guards.ts` — exports `requireAuth({ state, redirect })`
  that reads `state[Keys.Auth].val.status` and `redirect("/login")` if
  not `loggedIn`. Tracker uses this guard on the dashboard route(s).
- `src/components/DashboardLayout.ts` — wraps an `<aside>` (nav links)
  + `<section>${children}</section>`. The dashboard parent route uses
  it via the framework's nested-route layout mechanism (`children`
  prop).
- `src/routes/home.ts` — redirects to `/login` if not logged in
  (programmatic `navigate`), otherwise to `/issues`. (Or simpler: a
  static landing page that links to both.) Decide during execution;
  prefer the simpler landing-page form to keep `home.ts` testable
  without router setup.
- `src/routes/login.ts` — exports `default` (the page component) and
  no `load`/`meta`. The form submits `login(name)`; on the
  `loggedIn` transition, `navigate("/issues")`.
- `src/app.ts`:

  ```ts
  import { App, html, effect } from "zero";
  import { auth } from "./stores/auth.ts";
  import { theme } from "./stores/theme.ts";
  import { Keys } from "./state.ts";
  import Header from "./components/Header.ts";
  import DashboardLayout from "./components/DashboardLayout.ts";
  import Home from "./routes/home.ts";
  import Login from "./routes/login.ts";

  const app = new App();
  app.state(Keys.Auth, auth);
  app.state(Keys.Theme, theme);

  effect(() => {
    const t = theme.val;
    if (t === null) document.documentElement.removeAttribute("data-theme");
    else document.documentElement.dataset.theme = t;
  });

  app.layout(({ children }) => html`${Header()}<main class="app-main">${children}</main>`);
  app.route("/", Home);
  app.route("/login", Login);
  // Dashboard / issues registered in Step 10. DashboardLayout is wired
  // as a nested-route layout inside that step, *under* the root
  // layout — the global header stays mounted across login → dashboard
  // transitions.
  app.run("#app");
  ```

**Tests:**

- `stores/auth.test.ts` — direct exercise of `login` / `logout`;
  asserts the `loading` intermediate state is observed (use the
  test's microtask awareness: await a tick after `login()`, assert
  `.val.status === "loading"`, then await the full settle and assert
  `loggedIn`).
- `routes/login.test.ts` — renders the page, types a name, submits;
  asserts the auth signal transitions to `loggedIn`; uses a spy on
  `navigate` to assert redirect to `/issues`.
- `components/Header.test.ts` and `components/ThemeToggle.test.ts` —
  same shape as the counter / todos equivalents in Step 8.

**Integration test update:**

Not yet. Tracker integration tests land at the end of Step 10 so the
build + test sweep covers the full app.

---

### Step 10: `examples/tracker/` — issues, comments, fixture, tests

**Goal:** Complete the tracker: the issues store using `zero/http`,
list / detail / comment routes, the static JSON fixture, route tests,
and the integration-test entries. After this step the example
demonstrates every pattern called out in the spec.

**Files (created):**

- `examples/tracker/public/data.json`
- `examples/tracker/src/state.ts` (modified — adds `Keys.Issues`)
- `examples/tracker/src/stores/issues.ts`
- `examples/tracker/src/stores/issues.test.ts`
- `examples/tracker/src/routes/issues/index.ts`
- `examples/tracker/src/routes/issues/index.test.ts`
- `examples/tracker/src/routes/issues/issue.ts`
- `examples/tracker/src/routes/issues/issue.test.ts`
- `examples/tracker/src/components/IssueRow.ts`
- `examples/tracker/src/components/CommentThread.ts`
- `examples/tracker/src/components/IssueFilters.ts`
- `examples/tracker/src/lib/format.ts`
- `examples/tracker/src/lib/format.test.ts`

**Changes:**

1. `public/data.json` — a few dozen lines of fixture: ~10 issues, each
   with `id`, `title`, `status` (`open`/`closed`), `assignee`, and a
   `comments: [{author, body, createdAt}]` array. Status mix exercises
   the filter; comments exercise the thread component.

2. `src/stores/issues.ts`:

   ```ts
   import { createHttp } from "zero/http";
   import { signal } from "zero";
   import { navigate } from "zero";

   export type Issue = { id: string; title: string; status: "open" | "closed"; assignee: string; comments: Comment[] };
   export type Comment = { author: string; body: string; createdAt: string };
   export type IssuesState = { items: Issue[]; loaded: boolean };

   export const api = createHttp().use(async (req, next) => {
     const res = await next(req);
     if (res.status === 401) navigate("/login");
     return res;
   });

   export const issues = signal<IssuesState>({ items: [], loaded: false });

   export function setIssues(items: Issue[]): void { issues.set({ items, loaded: true }); }
   export function addComment(id: string, c: Comment): void { ... issues.update(...) }
   export function updateStatus(id: string, status: Issue["status"]): void { ... }
   ```

3. `src/state.ts` — extend with `Keys.Issues` and the
   `[Keys.Issues]: Signal<IssuesState>` line in `interface StateTypes`.

4. `src/routes/issues/index.ts` — exports `default` (the list page),
   `load`, and `meta`:

   ```ts
   export const meta = { protected: true };
   export async function load({ fetch }) {
     const data = await api.get<{ issues: Issue[] }>("/data.json", { fetch });
     setIssues(data.issues);
     return { issues: data.issues };
   }
   export default function IssuesIndex({ data, query }) { ... }
   ```

   Filters drive off `query.status` (`/issues?status=open`). The page
   wires `<IssueFilters />` which uses `navigate` to update the query
   string — no separate filter state.

5. `src/routes/issues/issue.ts` — detail view:

   ```ts
   export const meta = {
     protected: true,
     title: (data: { issue: Issue }) => `Issue #${data.issue.id}`,
   };
   export async function load({ params, fetch }) {
     const data = await api.get<{ issues: Issue[] }>("/data.json", { fetch });
     const issue = data.issues.find(i => i.id === params.id);
     if (!issue) throw { status: 404 };
     return { issue };
   }
   export default function IssuePage({ data }) { ... }
   ```

   Renders the issue header + `<CommentThread issue={data.issue} />` +
   a comment-add form using the `addComment` mutator.

6. `src/app.ts` — extend with:

   ```ts
   import IssuesIndex, { load as loadIssues, meta as issuesIndexMeta } from "./routes/issues/index.ts";
   import IssuePage, { load as loadIssue, meta as issueMeta } from "./routes/issues/issue.ts";
   import { issues } from "./stores/issues.ts";
   import { requireAuth } from "./lib/guards.ts";

   app.state(Keys.Issues, issues);
   app.route("/issues", IssuesIndex, { load: loadIssues, meta: issuesIndexMeta, guard: requireAuth });
   app.route("/issues/:id", IssuePage, { load: loadIssue, meta: issueMeta, guard: requireAuth });
   ```

7. Page-title side effect: the `meta.title` pattern is wired by a
   small `effect()` in `app.ts` that watches the route + reads the
   merged `meta` (the framework already merges chain meta into the
   route context). Implementation detail: since `meta.title` is a
   function, the app calls it with the resolved route `data`. The
   simplest approach is a layout-level computation; if reaching `data`
   from the layout proves awkward, push the title-setting into each
   route component (one extra line). Pick at execution time.

8. `src/components/IssueRow.ts`, `CommentThread.ts`, `IssueFilters.ts`
   — presentational only, no `.set()` calls on store signals.

9. `src/lib/format.ts` — `formatDate(iso: string): string` and similar
   helpers, exercised by `format.test.ts`. Demonstrates the `lib/`
   directory's purpose: pure functions, no UI, no state mutation.

**Tests:**

- `stores/issues.test.ts` — direct exercise of `setIssues`,
  `addComment`, `updateStatus` against a fresh signal.
- `routes/issues/index.test.ts` — renders with seeded `data.issues`,
  asserts the list renders; clicks a filter, asserts `navigate` was
  called with the right query string.
- `routes/issues/issue.test.ts` — renders with a seeded issue, asserts
  comment thread renders; types a comment, submits, asserts
  `addComment` is invoked.
- `lib/format.test.ts` — basic format tests.

**Integration test update:**

`tests/examples_build.rs` and `tests/examples_tests.rs`:

```rust
#[test] fn tracker_builds() { build_example("tracker"); }
#[test] fn tracker_tests_pass() { run_example_tests("tracker"); }
```

**LOC budget check:** at the end of this step, count TS LOC under
`examples/tracker/src/` excluding `*.test.ts`. If > 800, trim — the
likely candidates are presentational components (collapse helpers
into the route file) and `lib/format.ts` (drop if unused). The spec
forbids splitting into a second full example.

---

### Step 11: `BEST_PRACTICES.md` at repo root

**Goal:** The long-form prose reference. Each section ends with a
`→ See examples/...` pointer at a specific file in the examples just
shipped.

**Files:**

- `BEST_PRACTICES.md` — new file at repo root.

**Structure:**

1. **Project structure** — the canonical layout. Short prose; the file
   tree from the spec.  → `See examples/tracker/src/`.
2. **State organization** — `inject` + typed key registry, single
   signal vs many.  → `See examples/tracker/src/state.ts` and
   `examples/todos/src/state.ts`.
3. **Stores** — module-scoped stores; the "no `.set()` from
   components" rule.  → `See examples/tracker/src/stores/issues.ts`.
4. **Status-tagged signals** — the §5 working demo.
   → `See examples/tracker/src/stores/auth.ts`.
5. **Routes** — co-located `load`/`meta`/`default`, the
   `import Issue, { load, meta }` wiring pattern.
   → `See examples/tracker/src/routes/issues/issue.ts` and
   `examples/tracker/src/app.ts`.
6. **HTTP** — `createHttp()`, middleware patterns, route-scoped fetch
   threading.  → `See examples/tracker/src/stores/issues.ts`.
7. **Component usage** — *prefer `zero/components` over raw HTML for
   every interactive primitive.* The shipped library covers `Button`,
   `Input`, `Checkbox`, `Toggle`, `Select`, `Radio`, `TextArea`,
   `Dialog`, `Tabs`, `Card`, `Avatar`, `Badge`, `Spinner`, `Toast`.
   Reach for raw `<button>` / `<input>` / `<select>` only when (a) the
   shipped component cannot express the required behavior (file a
   comment naming the missing capability — the canonical example is
   `examples/todos/src/components/FilterBar.ts`, where the button
   variant must track a signal reactively), or (b) you are building a
   **new** presentational component the library doesn't ship (the
   canonical example is the per-app `Header` — there is no shipped
   `Header`, so each app builds one from layout classes and design
   tokens; see `examples/tracker/src/components/Header.ts`). Plain DOM
   containers (`<main>`, `<section>`, `<header>`, `<nav>`, `<ul>`,
   `<li>`, `<form>`, `<label>`, `<span>`, `<a>`, `<svg>`) are not
   "components" under this rule — use them freely. When you do build
   your own presentational component, wrap shipped primitives rather
   than re-implement them (the canonical example is
   `examples/tracker/src/components/ThemeToggle.ts`, which wraps the
   shipped `Toggle`).
   → `See examples/tracker/src/components/Header.ts`,
   `examples/tracker/src/components/ThemeToggle.ts`, and
   `examples/todos/src/components/FilterBar.ts`.
8. **Testing** — store tests, route tests, component tests.
   → `See examples/todos/src/stores/todos.test.ts` and friends.
9. **Performance** — the four bullets from the spec:
   - Lazy-load every route except the entry route.
   - Split a store when consumers diverge.
   - Keep `computed()` bodies narrow.
   - Prefer `each()` with a stable key over `.map()` for churning
     lists.

**Tests:** None. This step is prose. A future link-check is out of
scope; manually verify each `→ See` pointer references a real file in
the just-shipped examples.

---

### Step 12: `## Best practices` in `src/scaffold/AGENTS.md`

**Goal:** Bind agent behavior to the patterns. Short — ≤ 40 lines, code
blocks ≤ 10 lines — so it stays loaded in context without crowding the
rest of `AGENTS.md`.

**Files:**

- `src/scaffold/AGENTS.md` — append new section.
- `src/scaffold.rs` — extend
  `write_initial_project_agents_md_has_section_sentinels` with
  `"## Best practices"`.

**Section content:**

- One-paragraph framing: real apps want a state.ts, stores/, lib/,
  components/, routes/ layout.
- Bulleted directives:
  - **Use `zero/components` for every interactive primitive**
    (`Button`, `Input`, `Checkbox`, `Toggle`, `Select`, `Radio`,
    `TextArea`, `Dialog`, `Tabs`, `Card`, `Avatar`, `Badge`,
    `Spinner`, `Toast`). Drop to raw `<button>` / `<input>` /
    `<select>` only when the shipped component cannot express the
    behavior (leave a `//` comment) or when you are building a new
    presentational component the library does not ship (the per-app
    `Header` is the canonical case). Plain containers (`<main>`,
    `<section>`, `<form>`, `<ul>`, `<li>`, `<label>`, `<a>`,
    `<svg>`, …) are not "components" under this rule.
  - When building your own presentational component, **wrap shipped
    primitives** rather than re-implementing them.
  - Reach for `inject` via the `Keys` registry, not bare strings.
  - Mutate store signals only via the store's exported mutators.
  - Co-locate `load` / `meta` / `default` in the route file.
  - Use `zero/http` for HTTP, not `fetch` directly, in `load()` and
    elsewhere where middleware (auth headers, 401 redirect) applies.
- One ~5-line minimal example: a route file with `meta` + `load` +
  `default` and the matching `app.route(...)` registration.
- Closing line: `For longer rationale, see BEST_PRACTICES.md at the
  repo root.` (Repo root — the file isn't shipped into scaffolded
  projects, but the reference still points the reader at the
  framework repo.)

**Tests:**

- `src/scaffold.rs::write_initial_project_agents_md_has_section_sentinels`
  must include `"## Best practices"`.

---

### Step 13: Framework spec edits

**Goal:** Land the spec changes the new requirements imply. Spec stays
the capability reference; choice-style guidance stays in
`BEST_PRACTICES.md`.

**Files:**

- `zero-framework-spec.md`

**Changes:**

1. **§5 (State Machines, deferred)** — append one-line forward
   pointer: `> For the canonical signal({ status, ... }) pattern in
   working code, see `BEST_PRACTICES.md` and
   `examples/tracker/src/stores/auth.ts`.`
2. **§6 (Router)** — add a new subsection `### Route-scoped fetch`
   describing the contract from Step 2 (the `fetch` injected into
   `load()` carries an `AbortSignal` bound to the route scope;
   navigation aborts in-flight requests; `AbortError` from a
   navigation-driven abort is silently dropped; caller-supplied
   signals compose).
3. **§11 (Complete API Surface)** — replace the bare `inject(key)`
   line with the typed-registry pattern; add the `"zero/http"` API
   block (mirrors the `"zero/test"` block structure):
   ```ts
   // From "zero/http"
   createHttp(opts?)                    // factory → HttpClient
   client.use(mw)                       // register middleware
   client.get<T>(url, init?)            // and post / put / patch / delete
   client.request<T>(input, init?)      // generic
   HttpError                            // class — status, body, statusText
   ```
   Add the same forward-pointer at the bottom: `> For organization
   patterns, see `BEST_PRACTICES.md`.`
4. **§12 (Implementation Priority)**:
   - Rewrite the Phase 11 entry (current title: "Test Improvments
     [sic]") — actually, the existing Phase 11 entry is **already** a
     placeholder ("Test Improvments", currently a stub). The spec's
     "Phase 11 - Decorators" is described in the issue spec's
     Background — verify the latest spec.md state at execution time
     and rewrite whichever Phase 11 is the deferred-decorators slot to
     a one-paragraph deferral note that names the blocker (JS/TS
     decorators are class-only) and forward-points to Phase 12.
   - Update the Phase 12 entry from the current bullet list
     ("Add many more examples...", "Establish best practices...") to
     a checked-off list reflecting what this issue lands.
5. **§13 (Key Design Decisions Summary table)** — add one row:
   `| HTTP client | `"zero/http"` module with middleware | Every real
   app fetches; shipping one obvious wrapper avoids divergent
   conventions. |`

**Tests:** None — spec is prose. Verify section headers stay stable
so existing tooling (none currently parses the spec) is unaffected.

---

## Risks and Assumptions

- **Bundler module-resolution surface.** Step 4 assumes the bundler
  has a `ModuleId::Synthetic` variant or equivalent already used for
  `"zero"`. Inspect `src/build/bundler.rs` during execution; if the
  shape is different, mirror it. Worst case: `"zero/http"` needs a
  small additional emit branch alongside `"zero"`.
- **`AbortSignal.any` availability in Boa.** Step 2's
  `_composeSignals` may need a manual implementation if the Boa
  context driving the test runner doesn't expose `AbortSignal.any`.
  Pin during Step 2 — implement composition manually via a fresh
  `AbortController` if needed.
- **Co-located `load`/`meta` is a documentation convention, not a
  framework feature.** This plan documents the working pattern
  (`app.route` opts argument with imports from the route module).
  Adopters who expect the framework to auto-pick up named exports
  from the route module will be surprised. `BEST_PRACTICES.md` calls
  this out explicitly. Reconsider in a future phase if the friction is
  worth a runtime extension.
- **Tracker LOC budget.** 800 LOC of TS (excluding tests) is tight
  for an app that ships auth, layout, list, detail, comments, filters,
  HTTP middleware. If exceeded, trim presentational helpers and
  collapse `lib/format.ts` rather than splitting into a second full
  example (which the spec forbids).
- **Dev-port collisions.** Counter / todos / tracker use 5180 / 5181 /
  5182. Showcase uses 5174; scaffold default is whatever
  `src/cmd/init.rs` picks (verify it doesn't conflict). The choices
  are arbitrary; any free port works.
- **Test-discovery and `[id].ts`.** Decision in Prerequisites avoids
  the bracket-filename question entirely by using `issue.ts`. No
  discovery walker change needed.
- **Static-asset serving for `examples/tracker/public/data.json`.**
  Dev server `/public/*` route exists (`src/dev/server.rs:127`).
  Build pipeline copies `public/*` into `dist/` — verify in
  `src/build/`; if it doesn't, either land that capability in this
  issue or fall back to the in-memory fixture (the static-JSON
  decision is reversible at low cost). Plan currently assumes the
  build pipeline already copies `public/`; confirm at the start of
  Step 10.
- **Components-first rule is documentation-only.** Nothing in the
  build or test pipeline rejects a raw `<button>`. The discipline
  lives in `BEST_PRACTICES.md`, the AGENTS.md scaffold section, and
  the per-step reviewer eye. Code review of the tracker work
  (Steps 9–10) should flag any new raw-HTML primitive without a `//`
  comment naming the missing capability.
- **`matchMedia` in the test runtime.** `ThemeToggle` seeds its
  initial position from `matchMedia("(prefers-color-scheme: dark)")`.
  If the test DOM shim lacks `matchMedia`, fall back to a literal
  `"light"` default and add a `globalThis.matchMedia ?? (() => ({
  matches: false }))`-style guard in `ThemeToggle.ts`. Pin during
  Step 8.

---

## Revisit step details (Steps 14–17)

### Step 14: `zero update` self-bootstraps a missing `.zero/` directory

**Goal:** Make the one-command bootstrap path real. After this step,
`cd <project-with-zero.toml> && zero update --yes` materializes the
framework manifest whether or not `.zero/` exists. This unblocks every
example (and every future contributed project) from a fresh clone with
no manual mkdir dance.

**Files:**

- `src/cmd/update.rs` — drop the missing-`.zero/` bail; rely on
  `apply()`'s own `create_dir_all(parent)?` (already present at
  `src/cmd/update.rs:299–301`) to materialize the directory tree as the
  first Add operations land.
- `src/cmd/update.rs::tests` — replace
  `update_refuses_when_no_dot_zero_dir` with a positive test that
  asserts `.zero/` is created and populated when missing.
- `tests/common/mod.rs` — note only; the `prepare_showcase` and
  `prepare_example` helpers still pre-create `.zero/`, which is now
  redundant. Helper change lands in Step 16 (deliberately deferred so
  Step 14 changes only the CLI surface).

**Changes:**

1. In `src/cmd/update.rs::run_with`, remove the
   `if !project_root.join(".zero").is_dir()` bail block at
   `src/cmd/update.rs:97–102`. The downstream `compute_plan(&project_root)`
   already tolerates a missing `.zero/` — its `dot_zero.is_dir()`
   guard at `update.rs:180` skips the "extras" walk. The manifest
   enumeration produces `Add(...)` ops for every framework path, which
   `apply()` materializes with `fs::create_dir_all(parent)?`.

2. Update the user-facing summary line so the bootstrap case is
   distinguishable. Two reasonable shapes; plan picks (b):
   - (a) Same `applied N operations (...)` line either way.
   - (b) When `.zero/` was missing on entry, prefix with
     `bootstrapped .zero/ — applied N operations (...)`. Implementation:
     check `project_root.join(".zero").is_dir()` before
     `apply()` and stash the boolean for the final `println!`.

3. The `update_refuses_when_no_zero_toml` test (the only remaining
   "refuses" path) stays — `zero.toml` is still the entry contract.

**Tests:**

- `src/cmd/update.rs::tests::update_bootstraps_missing_dot_zero` —
  new. Build a tempdir with only `zero.toml` + the user-files scaffold
  (use `write_user_files` directly rather than `write_initial_project`
  so `.zero/` is genuinely absent on entry). Call
  `run_with(&root, true, &mut stub)`. Assert: (a) success exit,
  (b) `.zero/` now exists, (c) every manifest path under `.zero/` is
  present on disk, (d) the printed summary contains
  `"bootstrapped .zero/"`.
- Delete `update_refuses_when_no_dot_zero_dir` — its premise is gone.
- Existing tests
  (`update_with_no_drift_reports_up_to_date`,
  `update_with_missing_file_proposes_add`,
  `update_with_modified_file_proposes_update`,
  `update_with_extra_file_proposes_remove`,
  `update_yes_flag_applies_all_operations`,
  `apply_refuses_path_outside_dot_zero`,
  `update_with_empty_dot_zero_dir_proposes_only_adds`,
  `update_refuses_when_no_zero_toml`) stay green unchanged.

---

### Step 15: Restructure examples to the canonical subdir layout

**Goal:** Each example's on-disk shape mirrors the output of
`zero init` (with `root = "web"`). The user-facing files move under
`<example>/web/`; `zero.toml` and the dev-time `dist/` stay at
`<example>/` (sibling of `zero.toml`, matching `src/cmd/build.rs:25–26`'s
`out_dir = cwd.join(&config.build.out)` semantics). After this step
the example layout is what `zero init` would produce.

**Files (moved per example — counter, todos, tracker):**

For each `name` in `{counter, todos, tracker}`:

- `examples/<name>/zero.toml` — **modified**: `root = "web"` (was `"."`).
  Port and `out` lines unchanged.
- `examples/<name>/.gitignore` — **modified**: `web/.zero/` (was
  `.zero/`); `dist/` line unchanged.
- `examples/<name>/index.html` → `examples/<name>/web/index.html`
- `examples/<name>/tsconfig.json` → `examples/<name>/web/tsconfig.json`
- `examples/<name>/src/**` → `examples/<name>/web/src/**`
- `examples/<name>/styles/**` → `examples/<name>/web/styles/**`
- `examples/<name>/public/**` → `examples/<name>/web/public/**`
  (tracker only)

No file *contents* change in this step except `zero.toml` and
`.gitignore`. All TS/SCSS/HTML files move verbatim — the
`from ".../routes/home.ts"` import paths inside them are unaffected
because they are relative to the file's location, not the project root.

**Changes:**

1. For each example, run an in-tree `git mv` of `index.html`,
   `tsconfig.json`, `src/`, `styles/`, and (tracker only) `public/`
   into a new `web/` subdir. Verify with `git status` that the diff
   is a pure rename — content hashes unchanged.

2. Rewrite each `zero.toml`:

   ```toml
   [project]
   root = "web"

   [dev]
   port = <unchanged: 5180/5181/5182>

   [build]
   out = "dist"
   ```

3. Rewrite each `.gitignore` to:

   ```
   web/.zero/
   dist/
   ```

   (`dist/` stays at the top level because `build.out` is joined to
   the CWD where `zero.toml` lives, not to `[project] root`.)

4. **No source-file edits.** The runtime-relevant paths — index.html's
   `/styles/app.scss` link, the `src/app.ts` route imports, the
   `tsconfig.json`'s `include` lines — are all relative to the file's
   own directory and continue to resolve under the new `web/` root.

5. **Verification per example:** with the integration tests still
   pinned to the pre-Step-16 helper (which copies the whole example
   tree and runs `update`), run
   `cargo test --test examples_build counter_builds` (and the todos /
   tracker equivalents) to confirm the restructure didn't break the
   pipeline. The helper's pre-creation of `.zero/` at the **top** of
   the tempdir is now wrong (it should be created at `web/.zero/`,
   which is what `zero update` will do anyway after Step 14), so the
   helper change in Step 16 is the corollary. Step 15 alone, with the
   un-changed helper, fails the integration tests — that is expected
   and resolved by Step 16. **Sequencing note:** treat Step 15 + Step
   16 as a single landing unit; the codebase is broken between them.

**Tests:**

No new tests in this step. The integration tests in Step 16 are the
acceptance criterion.

---

### Step 16: Adjust `prepare_example` and re-green the integration tests

**Goal:** With Step 14 making `zero update` self-bootstrap and Step 15
moving the project root under `web/`, the `prepare_example` helper
needs to stop pre-creating `.zero/` at the wrong place. After this
step the example integration tests pass against the new layout.

**Files:**

- `tests/common/mod.rs` — modify `prepare_example`; optionally
  modify `prepare_showcase` (see below).
- `tests/examples_build.rs` — no change; the `dist/index.html` and
  `dist/assets/app.<hash>.js` paths are still relative to the tempdir
  root because `out = "dist"` is joined to the CWD where `zero.toml`
  lives, not to `[project] root`.
- `tests/examples_tests.rs` — no change; `zero test` is invoked from
  the tempdir, reads `zero.toml`, walks into `web/` for tests.

**Changes:**

1. In `tests/common/mod.rs::prepare_example`, remove the
   `std::fs::create_dir_all(tmp.path().join(".zero")).unwrap();`
   line at `tests/common/mod.rs:88`. The `zero update --yes` call
   that follows now self-bootstraps `web/.zero/` (Step 14).

2. The `copy_dir_filtered` call already skips `.zero` and `dist`
   only as **top-level** names. Under the new layout the example's
   `.zero/` (when locally built) lives at `examples/<name>/web/.zero/`,
   which is not a top-level skip target. Two options:
   - (a) Tighten the skip to also handle the nested path: extend
     `copy_dir_filtered` with a `skip_relative_paths` parameter for
     `web/.zero` / `web/dist`.
     **Not adopted** — needlessly broadens the helper.
   - (b) Rely on `.gitignore` plus the documented "don't commit
     `web/.zero/`" rule: the in-repo example tree never contains
     `web/.zero/`. The test helper copies verbatim, gets no `.zero/`,
     and `zero update` materializes it fresh.
     **Adopted.** Cleaner and matches `prepare_showcase`'s implicit
     assumption.

3. **`prepare_showcase`** uses the same pre-mkdir line at
   `tests/common/mod.rs:65`. Showcase still ships with `root = "."`
   (out of scope for restructure), so its `.zero/` would live at the
   tempdir root. After Step 14 the pre-mkdir is redundant for
   showcase too; drop the line there as well for consistency. The
   pre-existing `tests/showcase_build.rs` / `tests/showcase_dev.rs`
   continue to pass — they don't touch the directory shape.

4. Run the full sweep:

   ```bash
   cargo test --test examples_build
   cargo test --test examples_tests
   cargo test --test showcase_build
   cargo test --test showcase_dev
   cargo test --package zero --lib update::
   ```

   All must pass. If any test fails because it referenced the old
   top-level `index.html` / `src/` paths inside the tempdir (rather
   than going through `zero build` / `zero test`), update its path
   assertions.

**Tests:**

The existing `tests/examples_build.rs` and `tests/examples_tests.rs`
suites are the tests for this step. They cover counter, todos, and
tracker as `#[test]` functions already.

A new tiny smoke can be added if useful:
`tests/examples_layout.rs::counter_has_web_subdir` — asserts the
in-repo `examples/counter/web/src/app.ts` exists and that
`examples/counter/zero.toml` declares `root = "web"`. Cheap guard
against accidental future regressions. **Recommended but optional.**

---

### Step 17: Documentation — bootstrap flow, subdir layout, spec note

**Goal:** Surface both changes in the user-facing docs so the on-disk
shape and the bootstrap path are discoverable without spelunking the
issue tree.

**Files:**

- `BEST_PRACTICES.md` — add a §1.0 (or expand the opening of §1) on
  project lifecycle.
- `src/scaffold/AGENTS.md` — extend the existing `## Quick start`
  block (or add a small `### Refreshing the framework files`
  subsection) with the `zero update` bootstrap behavior.
- `src/scaffold.rs::tests` — extend
  `write_initial_project_agents_md_has_section_sentinels` if a new
  sentinel header is introduced (only if Step 17 adds a new `##`
  header — a subsection under an existing `##` does not).
- `zero-framework-spec.md` — append one sentence to the
  `zero update` description (wherever it lives in §10/§12) noting
  the auto-bootstrap behavior. No new section.

**Changes:**

1. **`BEST_PRACTICES.md` — new opening subsection in §1:**

   ```
   ### Project layout on disk

   A zero project is a directory containing a `zero.toml` and a
   project-root subdirectory (default `web/`). `zero init` writes
   this shape; `zero build` and `zero dev` read `zero.toml` from the
   working directory and walk into `<root>/`.

       my-app/
       ├── zero.toml          # [project] root = "web", [dev] port = 3000, …
       ├── dist/              # build output (gitignored)
       └── web/               # the project root
           ├── index.html
           ├── tsconfig.json
           ├── src/…
           ├── styles/…
           └── .zero/         # framework-owned (gitignored, refreshed by `zero update`)

   The shipped `examples/` (`counter`, `todos`, `tracker`) follow this
   shape exactly. The default `web` name is a convention, not a
   requirement — any non-leading-dot, non-escaping relative path is
   accepted by the `zero.toml` parser.

   ### Bootstrapping `.zero/`

   `zero update` materializes the framework files into `<root>/.zero/`.
   It auto-creates the directory if missing, so a fresh clone of any
   zero project becomes runnable with:

       zero update --yes
       zero dev

   `zero init` is for scaffolding a brand-new project; `zero update`
   is for keeping the framework files in sync with the installed CLI
   version.
   ```

2. **`src/scaffold/AGENTS.md` — extend `## Quick start` block:**

   Add one bullet after the existing `zero update`-adjacent line (or
   between `zero init` and `zero dev` in the existing block):

   > `zero update` refreshes the framework-owned files under
   > `<root>/.zero/`. If `.zero/` does not yet exist, `zero update`
   > creates it — fresh clones of an existing zero project are made
   > runnable with `zero update --yes` followed by `zero dev`.

   No new `##` section, so the section-sentinel test in
   `src/scaffold.rs` does not need extending. Verify at
   implementation time.

3. **`zero-framework-spec.md`:** locate the `zero update` paragraph
   (introduced in the earlier "test improvements" / `.zero/`
   ownership phase). Append:

   > `zero update` bootstraps a missing `.zero/` automatically — the
   > directory is created on demand when the first framework file is
   > written. The only hard precondition is that `zero.toml` exists.

**Tests:**

- `src/scaffold.rs::write_initial_project_agents_md_has_section_sentinels`
  — only update if a new `##` header lands. Plan currently does not
  introduce one; verify at implementation time.
- Manual review: confirm each `→ See …` cross-link in
  `BEST_PRACTICES.md` still resolves under the new `web/` paths
  (e.g. `examples/tracker/src/stores/issues.ts` becomes
  `examples/tracker/web/src/stores/issues.ts`). **Plan picks the
  shorter form** — rewrite every `→ See examples/<name>/src/...`
  reference in `BEST_PRACTICES.md` to `→ See
  examples/<name>/web/src/...`. Search-and-replace; verify each hit
  is the actual file pointer and not a code-block illustration.

---

## Revisit risks and assumptions

- **`showcase/` is out of scope.** Showcase also uses `root = "."`
  and would benefit from the same restructure for symmetry, but the
  user's call-out was the examples. Restructuring showcase would
  ripple through `tests/showcase_*.rs` and the design-system reload
  pipeline, which warrants its own decision. Listed here as
  follow-up. The Step 14 `zero update` bootstrap change is universal
  and benefits showcase regardless of layout.
- **Path-rename hygiene.** Step 15 is dominated by `git mv` of large
  subtrees. A misordered rename or a half-committed batch leaves the
  example unrunnable. Mitigation: complete Step 15 + Step 16 in one
  commit, run the example integration tests locally before pushing.
- **Path assumptions inside example files.** None observed — the
  scaffold model has always co-located `src/`, `styles/`, and
  `index.html` under a single root, and the example sources mirror
  that. `tsconfig.json` `include` patterns are relative to the
  tsconfig file's own directory and stay correct under `web/`. If a
  test (e.g. a `prepare_example` consumer) hard-codes the old
  top-level path, fix it locally; the integration tests in Step 16
  catch it.
- **Bootstrap fast path masks errors.** Auto-creating `.zero/` means
  a user who *intentionally* removed `.zero/` to start over silently
  gets it back. This is the desired behavior — `.zero/` is
  framework-owned. If we ever want a destructive "reset and choose"
  flow, it lives in a future `zero update --interactive` pass, not
  here.
- **Docs cross-references.** The `→ See examples/<name>/src/...`
  pointers in `BEST_PRACTICES.md` change to
  `→ See examples/<name>/web/src/...`. A future move of these examples
  would invalidate the links again; consider a follow-up that names
  the example directory once and uses relative links throughout.
  Outside this issue's scope.
- **No CLI flag added.** This revisit does not add `--bootstrap` to
  `zero update`. The behavior is unconditional. The rationale: every
  legitimate use of `zero update` against a project missing `.zero/`
  is the bootstrap case; the previous bail was defensive scaffolding
  rather than a real failure mode.

---

### Step 18: Redefine `.split`; simplify example headers; update docs

**Goal:** The `.split` primitive is currently defined as a two-column
grid (`grid-template-columns: 1fr 1fr;`) — semantics that nothing in
the framework or examples actually uses, and that conflicts with the
more useful "end-anchored groups with growing space between" pattern
the headers re-implement via `justify-content: space-between` plus an
explicit spacer `<div>`. Redefine `.split` as that flex pattern, then
delete the per-example workaround (the `app-header-spacer` element,
the `app-header { justify-content: space-between }` rule, and the
matching `.app-header-spacer` SCSS rule). Each `Header` then composes
`cluster split` to get the layout it already wanted — the canonical
"use the design system, don't hand-roll" demonstration.

**Files:**

- `src/scaffold/.zero/styles/_layout.scss` — replace `.split`
  definition.
- `runtime/zero.d.ts` — no change (layout primitives are CSS, not
  typed exports).
- `examples/counter/web/src/components/Header.ts` — add `split` to
  the class list; drop the `<div class="app-header-spacer"></div>`.
- `examples/todos/web/src/components/Header.ts` — same.
- `examples/tracker/web/src/components/Header.ts` — same, **and**
  add `cluster` to the class list (the tracker Header currently uses
  `class="app-header pad-md gap-md"` without `cluster`, unlike the
  other two — verified at `examples/tracker/web/src/components/Header.ts`).
  This brings tracker into line with counter/todos.
- `examples/counter/web/styles/app.scss` — drop the
  `justify-content: space-between;` line from `.app-header`; delete
  the entire `.app-header-spacer { flex: 1 1 auto; }` rule.
- `examples/todos/web/styles/app.scss` — same.
- `examples/tracker/web/styles/app.scss` — same.
- `src/scaffold/AGENTS.md` — update the `split` row in the
  layout-primitives table (currently at
  `src/scaffold/AGENTS.md:540`: "Two equal-width columns. Default
  `gap: var(--space-md)`. Does not wrap.").
- `zero-framework-spec.md` — the line at `zero-framework-spec.md:892`
  enumerates `cluster, stack, frame, split, flank, grid` but does not
  define each primitive's shape. **No change required** unless the
  spec carries a per-primitive table further down; check at
  implementation time and update the row if present.
- Tests in `examples/*/web/src/components/Header.test.ts` — verify
  no assertion targets the removed spacer (`.app-header-spacer`); if
  any does, drop it.

**Changes:**

1. **`src/scaffold/.zero/styles/_layout.scss` — new `.split`:**

   ```scss
   // Split — flex row with end-anchored groups; growing space distributes between them.
   .split {
       display: flex;
       justify-content: space-between;
       gap: var(--space-md); // default gap; override with gap-* utility
   }
   ```

   Drop the previous grid-based definition entirely. Rationale:
   nothing in the framework, examples, or showcase uses `.split`
   today (verified: `grep -rn 'class=".*split' --include='*.ts'
   --include='*.html' examples/ showcase/ src/` returns no hits
   outside the materialized `.zero/styles/_layout.scss`), so the
   redefinition has zero blast radius beyond the docs that describe
   it. The grid-based "two equal columns" use case is already covered
   by the more general `.grid` primitive with `--grid-min` set to
   roughly half the container width — it does not warrant a dedicated
   primitive.

   `.split` deliberately does **not** include `align-items: center`
   or `flex-wrap: wrap`. Composing with `.cluster` (which provides
   both) handles the header use case; consumers that want a bare
   split row apply `align-items` per-instance via the alignment
   utilities in `_alignment.scss`. This keeps each primitive
   single-responsibility — `.split` owns horizontal distribution
   only.

2. **`examples/<name>/web/src/components/Header.ts`** — counter and
   todos shown; tracker is the same with `cluster` added:

   ```ts
   export default function Header(): TemplateResult {
     return html`
       <header class="app-header cluster split pad-md gap-md">
         <a class="app-header-brand cluster gap-sm" href="/">
           ${Logo()}
           <span class="app-header-title">counter</span>
         </a>
         ${ThemeToggle()}
       </header>
     `;
   }
   ```

   Two edits per file: append `split` to the existing class list;
   remove the `<div class="app-header-spacer"></div>` line. The
   tracker variant additionally gains `cluster` at the front of its
   class list (it was missing — see Files note above).

   The combined `cluster split` works because both set
   `display: flex` and `gap: var(--space-md)`; `cluster` adds
   `flex-wrap: wrap` and `align-items: center`; `split` adds
   `justify-content: space-between`. None of these properties
   conflict.

3. **`examples/<name>/web/styles/app.scss`** — drop two things from
   the `.app-header` block, plus delete the spacer rule. After the
   edit, the counter header block reads:

   ```scss
   .app-header {
     background: var(--color-surface);
     border-bottom: var(--border-thin) solid var(--color-border);
   }

   .app-header-brand {
     color: var(--color-text);
     text-decoration: none;
     font-weight: var(--weight-bold);
   }

   .app-header-title {
     font-size: var(--font-lg);
   }
   ```

   The `.app-header-spacer { flex: 1 1 auto; }` rule is deleted
   outright. Todos and tracker mirror counter; tracker's
   `.app-main`, `.login-error`, `.dashboard-shell`, `.dashboard-nav`,
   `.dashboard-content`, `.issue-row*`, `.issue-detail-meta`,
   `.comment*` rules are unrelated to this step and are left alone.

4. **`src/scaffold/AGENTS.md`** — replace the `split` row in the
   primitives table at `src/scaffold/AGENTS.md:540`:

   ```
   | `split` | Horizontal flex with end-anchored groups; `justify-content: space-between` distributes growing space between children. Default `gap: var(--space-md)`. |
   ```

   The summary table at `src/scaffold/AGENTS.md:527` and the
   framework-files table at line 708 already list `split` as one of
   the six primitives without describing its shape — no change
   needed there.

5. **`zero-framework-spec.md`** — the one mention at line 892
   enumerates the primitive names without per-primitive descriptions.
   Re-read the surrounding paragraph during implementation; if there
   is a follow-up sentence or table that pins `.split`'s grid
   semantics, replace with the flex-with-space-between description.

**Tests:**

- `examples/<name>/web/src/components/Header.test.ts` — these
  currently assert `a.app-header-brand`, `svg.app-logo`,
  `.toggle`, and `.app-header-title` exist after rendering. None
  references `.app-header-spacer`. No test changes required;
  re-run all three after the SCSS / TS edits to confirm.
- `tests/examples_build.rs` — already covers each example; SCSS
  changes compile through `process_css` and a build failure would
  flag a syntax error in the rewritten primitives. No new tests.
- `tests/examples_tests.rs` — `zero test` for each example runs
  the Header tests under the new markup; must stay green.
- **Visual smoke (manual):** start `zero dev` in each example,
  load `/`, confirm the brand sits on the left, the theme toggle
  sits on the right, with the gap distributed between rather than
  collapsed against either edge. The redefined `.split` is what
  makes this correct without the spacer div.
- **Showcase regression check:** the showcase project also pulls
  `_layout.scss` from the framework manifest via `zero update`.
  No showcase markup references `.split` (verified above), so the
  redefinition does not affect showcase rendering. Re-run
  `cargo test --test showcase_build` after the change as a
  belt-and-suspenders check.

**Risks specific to this step:**

- **Breaking change for any external consumer.** If a project
  outside this repo already uses `.split` for two-equal-columns
  layout, this step changes the layout from grid to flex. The
  framework has no version-pinning surface in `.zero/` today, so
  such users would notice on their next `zero update`. Mitigation:
  call this out in the Step 17 / framework-spec note (one sentence:
  "Phase 12 redefines `.split` from a two-column grid to a flex
  row with `justify-content: space-between`."). This is the right
  trade: nothing in our tree uses the old semantics, the new
  semantics matches the name better, and the grid use case is
  served by `.grid` with `--grid-min`.
- **Cluster + split composition order.** If `_layout.scss` ever
  reorders these declarations such that `.split` is emitted before
  `.cluster`, the `display: flex` and `gap` properties cascade
  identically (both set the same values), but a future addition to
  either rule that *does* conflict (e.g. setting `flex-direction`)
  would silently flip behavior based on declaration order. The
  primitives' single-responsibility discipline (each owns exactly
  one axis of behavior) is the structural defense; document this
  in the primitive table's prose if a future maintainer is
  tempted to expand `.split`.

---

### Step 19: Move HTTP client construction and middleware out of the store

**Goal:** The tracker currently constructs its HTTP client **and**
registers a 401-redirect middleware at the top of
`examples/tracker/web/src/stores/issues.ts` (lines 31–43). Conflating
state-mutation concerns with application-wide policy (auth redirects)
inside a domain store is wrong on two counts: (a) middleware is a
cross-cutting concern owned by the app composition root, not by any
single feature store; (b) any future second store that wants to share
the client either duplicates the construction or implicitly imports
from `stores/issues.ts`, leaking a "primary" store designation that
shouldn't exist. Move the client construction to `src/lib/api.ts`
(empty middleware list at module scope) and shift the `.use()` calls
into `src/app.ts`, before `app.run()`. This is the placement convention
the docs should have endorsed in the first place.

**Sanity check on the `createHttp()` API.** Re-reading
`runtime/http.js:55–62`, `client.use(mw)` mutates the middleware array
in place and returns the client for chaining. This means a module that
exports an "empty" client can be augmented from elsewhere as long as
every augmentation lands **before** the first dispatched request. In
the proposed shape: `lib/api.ts` constructs the empty client at module
load; `app.ts` imports it and calls `.use()` synchronously; `app.run()`
starts the router, which fires route matching and `load()`. ES module
load order guarantees `app.ts`'s top-level statements run before
`app.run()` returns control, so every `.use()` call lands before any
HTTP request leaves the client. **The current API supports the
recommended pattern without modification** — the issue is placement,
not surface.

(One known sharp edge documented under Risks: scattered `.use()` calls
across files create implicit ordering. The recommendation is that all
middleware registration happens in `app.ts` in a single block, which
the convention enforces.)

**Files (tracker):**

- `examples/tracker/web/src/lib/api.ts` — **new**. Constructs and
  exports the empty client.
- `examples/tracker/web/src/stores/issues.ts` — **modified**. Drop
  the `createHttp` import, the `api` construction, and the
  `.use(...)` middleware. The store now owns only the signal and
  mutators. The `api` export is removed entirely; consumers import
  from `lib/api.ts`.
- `examples/tracker/web/src/app.ts` — **modified**. Import `api`
  from `lib/api.ts`; register the 401 middleware inline before
  `app.run()`.
- `examples/tracker/web/src/routes/issues/index.ts` — **modified**.
  Change `import { api, setIssues } from "../../stores/issues.ts"`
  to two imports: `api` from `../../lib/api.ts`, and
  `setIssues` from `../../stores/issues.ts`.
- `examples/tracker/web/src/routes/issues/issue.ts` — **modified**.
  Same split: `api` from `lib/api.ts`; `setIssues`, `addComment`,
  `updateStatus` from `stores/issues.ts`.
- `examples/tracker/web/src/stores/issues.test.ts` — **verify**. If
  the test imports `api`, drop the import. If it asserts middleware
  behavior, that assertion moves out of the store test entirely
  (middleware is no longer a store concern). Read the test at
  implementation time and adjust accordingly.

**Files (docs):**

- `BEST_PRACTICES.md` — rewrite §6 HTTP (lines 272–316) to teach
  the lib/app split.
- `src/scaffold/AGENTS.md` — locate the HTTP bullet (currently
  `src/scaffold/AGENTS.md:1026`'s "Use `zero/http` for HTTP, not
  raw `fetch`...") and add one sentence on placement: "Construct
  the client once in `src/lib/api.ts`; register middleware in
  `src/app.ts` before `app.run()`."
- `zero-framework-spec.md` — the `"zero/http"` API surface in §11
  is unchanged. No spec edit required. Verify no §6 / §11 prose
  endorses the old "construct in stores/" placement; if it does,
  rewrite the sentence.

**Changes:**

1. **`examples/tracker/web/src/lib/api.ts`** (new):

   ```ts
   // Single backend client for the tracker example. Middleware is
   // registered from `app.ts` (cross-cutting policy belongs to the
   // composition root, not a feature store). Stores and routes import
   // `api` from here and call its methods directly.

   import { createHttp } from "zero/http";
   import type { HttpClient } from "zero/http";

   export const api: HttpClient = createHttp();
   ```

   No middleware, no per-call defaults. Apps with multiple backends
   add a sibling file per backend (e.g. `lib/billing.ts`) — each is
   one empty `createHttp()` call at module scope.

2. **`examples/tracker/web/src/stores/issues.ts`** — remove lines
   5 (`navigate` import), 7–8 (`createHttp` / `HttpClient` imports),
   and 31–43 (the `api` construction block). The file collapses to
   types + signal + mutators:

   ```ts
   // stores/issues.ts — issues domain state and mutators. Pure store
   // semantics: nothing here knows about HTTP, redirects, or the wire
   // format. The list is hydrated by route `load()`s calling
   // `setIssues(...)`; mutators handle in-memory updates.

   import { signal } from "zero";
   import type { Signal } from "zero";

   export type IssueStatus = "open" | "closed";

   export interface Comment { ... }
   export interface Issue { ... }
   export interface IssuesState { items: Issue[]; loaded: boolean }

   export const issues: Signal<IssuesState> = signal<IssuesState>({
     items: [],
     loaded: false,
   });

   export function setIssues(items: Issue[]): void { ... }
   export function addComment(id: string, c: Comment): void { ... }
   export function updateStatus(id: string, status: IssueStatus): void { ... }
   ```

   The interface bodies and mutator bodies are unchanged — pure
   delete of the HTTP-related lines.

3. **`examples/tracker/web/src/app.ts`** — add two things between
   the existing route registrations and `app.run()`:

   ```ts
   import { api } from "./lib/api.ts";
   // … existing imports …

   const app = new App();
   // … existing app.state / effect / app.layout / app.route calls …

   // HTTP middleware: 401 redirects to /login. Registered here (the
   // composition root) rather than inside a store so the policy is
   // visible at the place that owns app-wide behavior.
   api.use(async (req, next) => {
     const res = await next(req);
     if (res.status === 401) navigate("/login");
     return res;
   });

   app.run("#app");
   ```

   `navigate` is already imported in `app.ts` (line 1). The
   middleware block sits right before `app.run("#app")` so the
   ordering is unambiguous to a reader: state → effects → layout
   → routes → middleware → run.

4. **`examples/tracker/web/src/routes/issues/index.ts`** — replace
   the single import line:

   ```ts
   // before:
   import { api, setIssues } from "../../stores/issues.ts";
   // after:
   import { api } from "../../lib/api.ts";
   import { setIssues } from "../../stores/issues.ts";
   import type { Issue } from "../../stores/issues.ts";  // unchanged
   ```

5. **`examples/tracker/web/src/routes/issues/issue.ts`** — same
   split:

   ```ts
   import { api } from "../../lib/api.ts";
   import { setIssues, addComment, updateStatus } from "../../stores/issues.ts";
   import type { Issue } from "../../stores/issues.ts";
   ```

6. **`BEST_PRACTICES.md` §6 rewrite** — replace the existing block
   (lines 272–316) with prose that teaches the placement:

   ```
   ## 6. HTTP

   `zero/http` ships a small, middleware-aware fetch wrapper.
   Construct one client per logical backend in `src/lib/`, with
   **no middleware at construction time**:

       // src/lib/api.ts
       import { createHttp } from "zero/http";
       export const api = createHttp();

   Register middleware in `src/app.ts`, before `app.run()`. The
   composition root is the right place for cross-cutting policy:
   auth headers, 401 redirects, retry, logging. Keeping middleware
   out of stores means a domain store like `stores/issues.ts` owns
   only state — HTTP transport is somebody else's problem.

       // src/app.ts
       import { api } from "./lib/api.ts";
       import { navigate } from "zero";

       api.use(async (req, next) => {
         const res = await next(req);
         if (res.status === 401) navigate("/login");
         return res;
       });
       app.run("#app");

   `client.use(mw)` mutates the middleware list in place; registering
   from `app.ts` before `app.run()` guarantees every middleware is in
   place before the first request fires. Apps with multiple backends
   declare one client per backend (e.g. `lib/billing.ts`,
   `lib/auth.ts`) and register their respective middleware in the
   same `app.ts` block.

   Middlewares run outermost-first on the way down, innermost-first
   on the way back up — the standard onion model. Canonical examples:

   - **Auth header injector** — `req.headers.set("Authorization", token)`
     before `next(req)`.
   - **401 → login redirect** — call `navigate("/login")` when the
     response status is 401; return the response unchanged so the
     caller still sees a rejection via `HttpError`.
   - **Short-circuit** — return a synthetic `Response` without
     calling `next()` to mock or cache.

   Inside a `load()`, thread the injected `fetch` through `init.fetch`:

       await api.get<T>("/public/data.json", { fetch: ctx.fetch });

   This routes the request through the route-scoped abort signal: a
   mid-load navigation cancels the in-flight fetch and the router
   swallows the resulting `AbortError`. See spec §6 for the full
   contract.

   Non-2xx responses reject with `HttpError` carrying `status`,
   `statusText`, and (if JSON) the parsed body. Network failures
   surface the underlying `TypeError`; aborts surface as
   `AbortError`.

   → See `examples/tracker/web/src/lib/api.ts` and
     `examples/tracker/web/src/app.ts`.
   ```

7. **`src/scaffold/AGENTS.md`** — extend the existing HTTP bullet
   at line ~1026 with one trailing sentence:

   > Construct the client once in `src/lib/api.ts` with no
   > middleware; register middleware in `src/app.ts` before
   > `app.run()` so cross-cutting policy lives at the composition
   > root rather than inside a domain store.

**Tests:**

- `examples/tracker/web/src/stores/issues.test.ts` — read at
  implementation time. If it imports or exercises `api`, drop that
  portion. The store now has no HTTP surface to test; the existing
  mutator tests (set/add/update) stay green unchanged.
- `tests/examples_build.rs::tracker_builds` and
  `tests/examples_tests.rs::tracker_tests_pass` — must stay green.
  The build verifies the new `lib/api.ts` is reachable through the
  bundler; the test sweep verifies the route loaders still hydrate
  the store through the lib-owned client.
- **No new dedicated test** for "middleware registered before
  `app.run()`". The existing route tests exercise the full
  composition; if middleware were misplaced (e.g. registered after
  `app.run()` and after a route already fired), the issues-index
  test would observe an unmiddlewared response. The existing
  http.test.js coverage of middleware ordering (`runtime/http.test.js`)
  is unchanged — the framework-side behavior is the same.

**Risks specific to this step:**

- **Scattered `.use()` discipline.** Nothing in the framework
  prevents a future contributor from adding `api.use(...)` inside
  a store. The convention is documentation-only. If/when this
  matters, a future revision could either freeze the middleware
  list after first dispatch (and throw on late `.use()`) or accept
  middleware at construction time only — both are out of scope
  here. The doc framing ("middleware is a composition-root
  concern") is the structural defense.
- **Test file imports.** The full set of route / component / store
  tests in `examples/tracker/web/src/` may import `api` from
  `stores/issues.ts`. A simple `grep -rn 'from "../../stores/issues' examples/tracker/web/src/`
  at the start of implementation surfaces every consumer; rewrite
  each import in lockstep. The integration sweep at the end of the
  step is the final guard.
- **Scaffold note placement.** `src/scaffold/AGENTS.md` does not
  currently have a dedicated HTTP section header — the HTTP
  guidance is a bullet inside the "Best practices" section. The
  added sentence rides on the existing bullet. If a future
  refactor splits AGENTS.md into per-topic subsections, this
  sentence can graduate into its own paragraph.
- **API change is deliberately not in scope.** The user asked
  whether `createHttp()` "is even going to work the way I want it
  to." The honest answer landed in the Goal: yes, the mutation-via-
  `.use()` model supports the lib/app split cleanly. If after
  living with the new pattern the user wants a stricter shape (e.g.
  `createHttp({ middleware: [...] })` so the list is fixed at
  construction), that's a future API addition; flag it as an open
  question in the issue tracker rather than in this step.
