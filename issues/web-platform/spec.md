# Spec: Web Platform surface audit and shim expansion

## Problem Statement

The test environment (`zero test`, powered by Boa + `runtime/dom-shim.js`) has
been bitten repeatedly by missing browser globals. Phase 13 ("DOM shim
expansion" in `zero-framework-spec.md`) drew its fence around the **DOM** —
Document, Element, Event constructors, classList, dataset, style, web storage,
observers, matchMedia, navigator, crypto, getComputedStyle, timers — and sized
the implementation correctly *for that fence*. The problem is that the fence
was drawn around the wrong thing. Real apps and the framework's own modules
reach for the **Web Platform** more broadly: Fetch, URL, encoders, Blob,
structured cloning, micro-task scheduling. Each missing API surfaces as a
`ReferenceError` mid-test, the user has no way to know whether the gap is a
bug or an intended boundary, and every shim iteration leaves another batch of
gaps to discover by failure.

The immediate trigger is that `runtime/http.js` (the public `"zero/http"`
module) does `new Headers()` and `new Request()` internally. Any user test
that imports `createHttp` from `zero/http` throws
`ReferenceError: Headers is not defined` before the test body runs. The
runtime-tests conversion (`issues/runtime-tests/plan.md` Step 3) is blocked
on the same gap.

The structural fix is to redraw the fence around **Web Platform** (a closed
enumerated list) instead of **DOM**, complete the implementation in one pass,
and make the boundary explicit in documentation so future gaps surface as
contract changes (with a spec) rather than as bug reports against test runs.

## Background

### What exists today

- `runtime/dom-shim.js` (~1500 lines) installs DOM globals onto `globalThis`
  as a side effect of importing `"zero"` or `"zero/test"`. Phase 13 in
  `zero-framework-spec.md` is the current contract.
- The shim is loaded automatically by the Boa-based test harness via the
  in-memory `"zero"` / `"zero/test"` modules (`crates/zero-runtime/src/lib.rs`).
- `runtime/http.js` is shipped as the public `"zero/http"` module. It uses
  `Headers` and `Request` constructors and `fetch` (configurable per call).
- `runtime/http.test.js` already documents the user-facing pattern: stub
  `globalThis.fetch` per test, build `Response` / `Headers` for the stub's
  return value.
- Boa implements ECMAScript; it provides **nothing** from the Web Platform.
  Every browser global must come from the shim.
- `cleanup()` (in `runtime/test.js`) already handles per-test reset of
  storage, timers, focus, and title. Pattern: shim exposes
  `__clearAllTimers__` etc. on `globalThis`; `cleanup()` calls them.

### What's missing today

A full audit lives in the Requirements section. The known-missing set at
spec-writing time:

- **Fetch API**: `Headers`, `Request`, `Response`, `fetch`,
  `AbortController`, `AbortSignal` (including `AbortSignal.any([...])` for
  the route-scoped fetch composition described in `zero-framework-spec.md` §6).
- **URLs**: `URL`, `URLSearchParams`. The router already parses query strings
  by hand; `URL` would replace ad-hoc parsing in user code.
- **Encoding**: `TextEncoder`, `TextDecoder`. Common in any code that touches
  bytes, hashing, or web-crypto.
- **Binary data**: `Blob`, `File`, `FormData`. Standard for upload-shaped UI.
- **Cloning / scheduling**: `structuredClone`, `queueMicrotask`,
  `Promise.withResolvers` (newer; ES2024 — may already be in Boa, verify).

### Why not jsdom / happy-dom / engine swap

Considered and rejected upstream of this spec:

- **jsdom / happy-dom** are JS DOM implementations. Bundling them as vendored
  bytes preserves "no npm dep for users" but requires a JS engine spec-
  compliant enough to run them. Boa is not (heavy `Proxy` / `Reflect` /
  WeakRef usage in those libraries trips Boa today). Using them implies a
  Boa → QuickJS or Boa → V8 swap, which is a months-long reshape, adds
  1–50MB to the binary, and contradicts the "single Rust binary, no
  external runtime" stance.
- **Rust-native DOM crates** (html5ever, kuchikiki, deno_dom) are HTML
  parsers, not DOMs. No events, no `window`, no storage. Using them means
  writing dom-shim on top of a different parser — same project, different
  base layer.
- **Embedded headless browser** (Servo, CEF, WebKit) adds tens to hundreds
  of MB and breaks the framework's whole shape.

The accepted path is: keep Boa, keep the hand-written shim, widen its scope
from "DOM" to "Web Platform," draw the boundary explicitly, and adopt a
"clear error message at the boundary" discipline so future users hit the
edge with a sentence they can act on instead of a `ReferenceError`.

### The "clear error" idea, scoped

A blanket "trap every undefined global and throw a friendly message" is
infeasible — the Web Platform's negative space is unbounded, and a Proxy on
`globalThis` would slow every property access. The discipline this spec
adopts is narrower: any API the shim **intentionally stubs** (i.e., it
installs a global, but the global doesn't do real work — `fetch` is the
canonical example) must throw a clear actionable message rather than silently
return a useless value. Unimplemented globals not in the audited surface get
the standard `ReferenceError` from Boa, which is already a clear signal that
the user is outside the supported test surface.

## Requirements

### 1. The audit (informs implementation scope)

Produce a closed enumerated list of every Web Platform global that must be
present in `zero test`. The list is built from three sources, all of which
must be exercised during the audit step:

1. **Framework runtime** — every Web Platform identifier referenced
   (transitively) by `runtime/*.js` excluding `runtime/*.test.js`. Today:
   `Headers`, `Request`, `Response`, `AbortController`, `AbortSignal`,
   `fetch` (from `runtime/http.js`); any others surfaced by grep.
2. **Shipped example projects** — every Web Platform identifier referenced
   (transitively) by `examples/counter`, `examples/todos`,
   `examples/tracker`, and `showcase/`. These are the dogfood surface; if a
   shipped example reaches for it, the shim must provide it.
3. **Documented user surface** — APIs not used by (1) or (2) but reasonable
   to expect a user to reach for: `URL`, `URLSearchParams`, `TextEncoder`,
   `TextDecoder`, `Blob`, `File`, `FormData`, `structuredClone`,
   `queueMicrotask`. Anything outside this third set is explicitly *not*
   covered and must be stubbed by the user.

The audit output is a single canonical list, kept in `zero-framework-spec.md`
(see Requirement 5). The audit itself is a one-time activity; future
additions go through a spec.

### 2. Implementation

Every API in the audited list is implemented in pure JS, hand-written,
following the existing `runtime/dom-shim.js` style. Implementation lives in
`runtime/dom-shim.js` or in sibling JS modules (`runtime/web-platform.js`,
`runtime/fetch-shim.js`, etc. — split is an Open Question, see below) that
the existing shim load path imports. The shim continues to install globals
onto `globalThis` as a side-effect of importing `"zero"` or `"zero/test"`.

The behavioral contract per API is the minimum that satisfies (a) every
caller inside the framework, (b) every test currently in `runtime/*.test.js`
post-conversion, and (c) the shape users would write a stub against. It is
**not** a full Web Platform implementation. Concrete shapes:

- **`Headers`** — Map-shaped with `get` / `set` / `has` / `delete` / `append`
  / `entries` / `keys` / `values` / iterator. Case-insensitive keys per spec.
- **`Request`** — constructor with `(input, init?)`. Exposes `url`, `method`,
  `headers`, `body`, `signal`. Does not implement `clone()` or stream bodies.
- **`Response`** — constructor with `(body?, init?)`. Exposes `ok`, `status`,
  `statusText`, `headers`, `json()`, `text()`. Does not implement
  `arrayBuffer()` / `blob()` beyond returning the raw body (or rejecting
  with a clear message).
- **`AbortController`** / **`AbortSignal`** — full standard shape including
  `signal.aborted`, `signal.reason`, `signal.throwIfAborted()`,
  `addEventListener('abort', ...)`, and **`AbortSignal.any([...])`** for the
  route-scoped fetch composition described in `zero-framework-spec.md` §6.
- **`fetch`** — default implementation **rejects** with a clear actionable
  error: `"zero test: globalThis.fetch is not implemented. Stub it in your
  test's beforeEach (or pass init.fetch to the call) — see
  runtime/http.test.js for the pattern."` The user's test setup overrides
  `globalThis.fetch`; `cleanup()` restores the default.
- **`URL`** / **`URLSearchParams`** — full standard surface as it appears in
  modern browsers. These are pure-JS implementations and are small.
- **`TextEncoder`** / **`TextDecoder`** — UTF-8 only. No `encodeInto` /
  `decode(stream)` complexity unless an audited caller needs it.
- **`Blob`** / **`File`** — constructors with `parts`, `type`. Methods:
  `text()`, `arrayBuffer()`, `slice(start, end, type)`, `size`, `type`.
- **`FormData`** — Map-shaped with `append` / `set` / `get` / `getAll` /
  `has` / `delete` / `entries` / iterator. Supports string and Blob values.
- **`structuredClone`** — deep clone covering plain objects, arrays, Map,
  Set, Date, RegExp, ArrayBuffer, typed arrays. Throws on functions /
  DOM nodes (spec-correct).
- **`queueMicrotask`** — schedules through Boa's job queue, same path the
  shim's timer host uses.
- **`Promise.withResolvers`** — verify Boa 0.21 already ships this in
  ES2024 mode. If yes, no shim needed; if no, polyfill at shim-load time.

Any audited API not listed above (surfaced by the framework / examples /
showcase grep) gets the same treatment: minimum behavioral contract,
hand-written.

### 3. The "intentional stub" error contract

Any API the shim installs but does not implement (today: `fetch`; future:
maybe others) must throw an error matching this shape:

```
zero test: <API> is not implemented. <one-sentence action the user can take>.
```

This is the discipline that makes future gaps actionable. `fetch`'s default
rejection (Requirement 2) is the first instance.

For APIs **outside** the audited list (everything else in the Web Platform),
no special handling: Boa's `ReferenceError: <name> is not defined` is the
boundary signal. Document this in the framework spec (Requirement 5) so
users know what to expect when they reach beyond the surface.

### 4. Per-test reset

Any per-test mutable state introduced by the new APIs is drainable via
`cleanup()`. Concretely:

- `globalThis.fetch` is restored to the default-rejecting stub on each
  `cleanup()`. Override is per-test, never leaks across tests.
- Any `AbortController` instances pinned in a "global registry" pattern (if
  the implementation uses one for `AbortSignal.any([...])`) are released.
- The drain hook follows the existing pattern: shim installs
  `globalThis.__resetFetch__` (or similar) and `runtime/test.js::cleanup`
  calls it alongside `__clearAllTimers__` and storage clearing.

### 5. Documentation

The Web Platform surface is documented in two canonical places:

- **`zero-framework-spec.md`** — rename §8's "No Browser Required" subsection
  or add a new §8.x titled "Web Platform surface in zero test." Lists every
  audited API with a one-line behavioral note. States explicitly that APIs
  not on this list are outside the test environment's scope and surface as
  `ReferenceError` — users stub them in their own test setup. The list
  produced by Requirement 1's audit lands here verbatim.
- **`BEST_PRACTICES.md`** — append a short "Testing against the Web
  Platform" section: how to stub `globalThis.fetch` per test, where the
  boundary sits, the standard pattern (mirror `runtime/http.test.js`'s
  `makeStubFetch` helper).

Phase 13's entry in `zero-framework-spec.md` is left intact (historical),
but a new Phase 14 (or §8.x non-phased entry) records this expansion.

### 6. Unblock downstream work

After this spec lands:

- `runtime/http.js` runs end-to-end under `zero test`. A user test of the
  form
  ```js
  globalThis.fetch = makeStubFetch(routes);
  const client = createHttp();
  expect(await client.get('/x')).toEqual(...);
  ```
  passes without `ReferenceError`.
- `issues/runtime-tests/plan.md` Step 3 (convert `runtime/http.test.js`)
  unblocks. The step itself stays in that plan; this spec just removes its
  blocker.

### 7. No engine / loader / build changes

- No engine swap. Boa stays.
- No npm dependency, no vendored JS DOM library (jsdom / happy-dom).
- No changes to `ZERO_RUNTIME_EXPORTS` / `ZERO_TEST_EXPORTS`.
- No changes to `crates/zero-test-runner/src/loader.rs` (the loader resolves
  bare specifiers; this spec adds no new specifiers).
- The build script (`crates/zero-runtime/build.rs`, if it concatenates shim
  files) may need to include additional `runtime/*.js` files if the
  implementation splits across multiple files — that's a minor structural
  change, not an API change.

## Constraints

- **Pure JS, hand-written.** No regex-via-eval, no `Function` constructor,
  no transpiled-from-TS source-of-truth — `.js` is the canonical extension
  for the shim (matches the rest of `runtime/`). Fully JSDoc-annotated per
  CLAUDE.md.
- **Boa compatibility.** Avoid the GC bug pattern documented in
  `boa-maplock-finalizer`: prefer separate functions per branch over
  inline if/else when adding new code paths to existing functions. Verify
  against `cargo test -p zero --test examples_tests tracker_tests_pass`
  after non-trivial additions — the failure mode is process-exit panic,
  not test failure.
- **Function size ≤80 lines** (CLAUDE.md). Split aggressively.
- **No new Rust APIs.** The harness, loader, and runtime exports surface
  do not change. This is a JS-side expansion.
- **Bundle growth budget.** Today the shim is ~1500 lines. Full Web Platform
  surface is plausibly +1500–2500 lines (rough estimate: Fetch ~600, URL
  ~400, encoding ~200, Blob/File/FormData ~400, structuredClone ~300,
  small utilities the rest). Acceptable; keeps the runtime well under any
  meaningful binary-size threshold.
- **Per-file isolation invariant holds.** Boa loads a fresh context per
  test file (`crates/zero-test-runner/src/harness.rs`). Within-file state
  drains via `cleanup()`. The new APIs must not break either property.
- **Audit is exhaustive, not sampled.** The grep / AST walk over framework
  and example code must surface every Web Platform identifier — missing
  one leaves a future gap. The Open Questions section addresses method.

## Out of Scope

- **Full Fetch network implementation.** `fetch` rejects by default; users
  stub it. No real network in `zero test`. (E2E testing is separately
  out of zero's scope per spec §8 — Playwright / Cypress own that.)
- **Streaming APIs.** `ReadableStream`, `WritableStream`, `TransformStream`.
  Reach for them when a framework module or example uses them; until then,
  out.
- **WebSocket / EventSource / Server-Sent Events.** Same — pull-when-used.
- **Web Workers / SharedWorkers / MessageChannel.** Threading semantics
  under Boa is an open question for another spec.
- **IndexedDB.** `localStorage` / `sessionStorage` already cover the common
  case. IDB is a separate, much larger surface.
- **CSS Object Model beyond what's already shimmed.** No real layout
  engine, no real `getBoundingClientRect` values, no CSSStyleSheet API.
- **WebGL / Canvas 2D.** Canvas is currently no-op in the shim; stays that
  way. Real implementations require an actual renderer.
- **Web Crypto beyond `crypto.randomUUID` / `crypto.getRandomValues`.**
  Already in the shim. `SubtleCrypto.digest` etc. — out unless a framework
  module needs it.
- **Notifications, Geolocation, MediaDevices, WebRTC, Bluetooth,
  Permissions, Clipboard, Battery, etc.** Long tail of browser APIs that
  no framework module touches and no shipped example reaches for. Out.
- **Engine swap to QuickJS / V8.** Decided against upstream of this spec.
- **Vendoring jsdom / happy-dom.** Decided against upstream of this spec.
- **Updating `issues/runtime-tests/plan.md`.** That plan's Risks section
  should mention this spec as the prerequisite to Step 3 — that edit
  happens when execution of Step 3 resumes, not in this spec.

## Open Questions

1. **File layout.** Single growing `runtime/dom-shim.js` (would push past
   ~3000 lines), or split: `dom-shim.js` (DOM proper), `fetch-shim.js`
   (Fetch + AbortController), `url-shim.js` (URL + URLSearchParams),
   `encoding-shim.js` (TextEncoder / TextDecoder), `binary-shim.js`
   (Blob / File / FormData), and a small `web-platform.js` that imports
   and re-exports them? Split keeps file sizes bounded but requires
   updating `crates/zero-runtime/build.rs` (which currently concatenates a
   fixed set of files into `ZERO_DOM_SHIM_BODY`). Plan author's call;
   default recommendation is split.

2. **Audit method.** Three options:
   - **By-hand grep.** Walk every identifier with `\b<CapitalizedName>\b`
     pattern across `runtime/`, `examples/`, `showcase/`. Cross-reference
     against the MDN list of Web Platform globals. Fast (an afternoon),
     but easy to miss things written in unusual forms (e.g., `globalThis.X`,
     destructuring from `window`, etc.).
   - **AST walk.** Use the existing transpiler crate (`crates/zero-transpile`)
     to walk every JS / TS file and collect undefined-binding references.
     More reliable. Requires writing a small tool.
   - **Runtime tracing.** Install a Proxy on `globalThis` in the shim, log
     every property access during the existing test suite + example builds,
     diff against the implemented set. Most reliable but requires
     instrumentation that itself may trip Boa.

   Recommendation: by-hand grep first to bootstrap the list, then runtime
   tracing as a verification pass against the example builds. AST walk if
   the runtime trace turns up too many false positives.

3. **`fetch` default behavior.** Three options:
   - Return a rejected Promise with a clear message (the spec's current
     stance).
   - Throw synchronously on call.
   - Return a 200 OK Response with an empty body.

   Rejected Promise is closest to "what happens if a real browser is
   offline" and matches `runtime/http.test.js`'s existing pattern. Plan
   author should confirm but the spec assumes rejection.

4. **`AbortSignal.any([...])` semantics.** The route-scoped fetch in §6
   relies on composing two signals — abort on either fires the composite.
   The standard spec is precise but small; spec author should confirm
   `AbortSignal.any` (not `AbortSignal.aborted`) is the right name and
   that Boa doesn't already ship it.

5. **`Promise.withResolvers` Boa support.** ES2024; Boa 0.21 may or may
   not have it. Plan author verifies and polyfills if missing.

6. **`structuredClone` scope.** Cover (a) plain objects + arrays only,
   (b) plus Date / RegExp / Map / Set, (c) plus typed arrays + ArrayBuffer,
   (d) plus transferable handling. Recommendation: (c) — covers everything
   except transferables, which require host-engine integration.

7. **Documentation phasing.** This work is "Phase 14" in
   `zero-framework-spec.md`, or a non-phased §8.x expansion? Phase numbers
   are running implementation-order markers; a documentation-shaped
   expansion that closes a contract may fit better as a §8 subsection.
   Plan author's call.

8. **Smoke test coverage.** Beyond `runtime/http.test.js` passing, what
   else verifies the audit is complete? Plausible additions: a small
   `runtime/web-platform.test.js` that exercises each audited API at
   least once, so regressions surface here instead of in downstream
   user tests.
