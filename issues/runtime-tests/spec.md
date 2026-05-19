# Spec: Convert framework-internal runtime tests to `zero test`

## Problem Statement

The framework's own JS tests under `runtime/*.test.js` still run on
`node:test` + `node:assert/strict`, while user apps and every other
in-repo test surface (`examples/`, `showcase/`, scaffolded crates)
run on `zero test`. This split has two costs that have not changed
since the original spec:

1. **Node remains a contributor prerequisite for no other reason.**
   The framework has zero npm dependencies and is otherwise built
   with `cargo`. Forcing Node onto contributors only so the runtime
   tests can run contradicts the project's "single binary, no
   dependencies" stance. `CLAUDE.md` and `README.md` both still
   document two test commands.

2. **The framework does not dogfood its own test runner.** Bugs in
   `zero test` (matcher edge cases, DOM-shim drift, source-mapping
   regressions, async handling) surface in user apps before they show
   up in the framework's own suite.

This re-spec exists because the Web Platform shim work landed in the
meantime (`issues/web-platform/`). That work changes the inventory
of files involved and removes the original spec's only structural
blocker. The objective is the same: drop `node:test` from `runtime/`
and run every JS test in the repo under `zero test`.

## Background

### What changed since the original spec

- **Web Platform shim has landed.** `runtime/fetch-shim.js`,
  `runtime/url-shim.js`, `runtime/encoding-shim.js`,
  `runtime/binary-shim.js`, and `runtime/clone-shim.js` were added
  alongside `runtime/dom-shim.js`; all six are concatenated into the
  `ZERO_DOM_SHIM_BODY` that the `"zero"` / `"zero/test"` modules
  install on import (`crates/zero-runtime/build.rs`,
  `crates/zero-runtime/src/lib.rs`). Boa now sees `Headers`,
  `Request`, `Response`, `AbortController`, `AbortSignal` (including
  `AbortSignal.any`), `URL`, `URLSearchParams`, `TextEncoder`,
  `TextDecoder`, `Blob`, `File`, `FormData`, `structuredClone`,
  `queueMicrotask`, and `Promise.withResolvers` at test time. The
  surface is documented in `zero-framework-spec.md` §8 ("Web
  Platform surface in `zero test`").
- **`runtime/http.test.js` no longer blocks on missing globals.** The
  original spec's Step 3 was held by `ReferenceError: Headers is not
  defined`; that's resolved.
- **Five new shim test files exist** —
  `runtime/{binary,clone,encoding,fetch,url}-shim.test.js`. Each one
  reads its shim's source text, evaluates it inside a fresh
  `node:vm` sandbox, and asserts against the sandbox's `globalThis`.
  They exist to bypass Node's native classes and exercise the shim
  itself. None of them are convertible to `zero test` while keeping
  that posture: under Boa there is no native to shadow, so the
  sandbox-vs-globals distinction collapses, and re-asserting against
  `globalThis` would be re-asserting against the very shim the test
  is supposed to validate.
- **`runtime/web-platform.test.js` exists** — a one-`describe` smoke
  file with one `it` per audited Web Platform API. Today it imports
  `node:test` + `node:assert/strict` plus `./http.js`; under Boa, it
  would exercise the same shim that ships with `zero test`.

### What hasn't changed

- The seven original framework test files
  (`app.test.js`, `router.test.js`, `template.test.js`,
  `reactivity.test.js`, `http.test.js`, `dom-shim.test.js`,
  `test.test.js`) still import `node:test` and reach into
  `_`-prefixed framework internals
  (`_normalizePath`, `_compileRoutePattern`, `_matchAgainst`,
  `_joinPaths`, `_parsePathAndQuery`, `_parseQuery`, `_createScope`,
  `_setCurrentApp`, `_getCurrentApp`) and shim-internal state
  (`document._listeners`, `window._listeners`,
  `window.history._entries`, `window.history._index`,
  `__getTestTree__`, `__resetTestTree__`).
- `zero test` runs each test file in a fresh Boa context. The loader
  (`crates/zero-test-runner/src/loader.rs`) resolves bare specifiers
  `"zero"` and `"zero/test"` from the concatenated runtime
  (`crates/zero-runtime/src/lib.rs::runtime_module()` and
  `test_module()`). Public exports are listed in
  `ZERO_RUNTIME_EXPORTS` / `ZERO_TEST_EXPORTS`.
- Discovery (`crates/zero-test-runner/src/discovery.rs`) walks the
  project root for `*.test.{js,ts}` / `*.spec.{js,ts}`. `runtime/` is
  on the include path. No repo-root `zero.toml` exists today
  (`showcase/zero.toml` is unrelated).
- The Rust harness (`crates/zero-test-runner/src/harness.rs`) has
  its own integration tests covering the Rust↔JS contract — tree
  walking, source-mapping, async handling, hook ordering, console
  capture, throw semantics. Anything that exists in
  `runtime/test.test.js` only to verify that contract is already
  double-covered there.
- The `zero/test` and `node:test` APIs are similar enough that most
  rewrites are mechanical:
  - `assert.equal(a, b)` → `expect(a).toBe(b)`
  - `assert.deepEqual(a, b)` → `expect(a).toEqual(b)`
  - `assert.ok(x)` → `expect(x).toBeTruthy()`
  - `assert.throws(fn, /msg/)` → `expect(fn).toThrow('msg')`
  - `assert.rejects(p, ...)` → `try { await p() } catch (e) { ... }`
    plus field assertions (no `.rejects` matcher in `zero/test`).
  - `assert.notEqual(a, b)` → `expect(a === b).toBeFalsy()` (no
    `.not` matcher in `zero/test`).
  - `assert.doesNotThrow(fn)` → just call `fn()` (any throw fails).
- The framework-internal basename filter in
  `crates/zero-test-runner/src/harness.rs` (`FRAMEWORK_INTERNAL_BASENAMES`)
  hides frames from `runtime/*.js` when picking the user-code frame
  for failure locations. It matches basenames (`test.js`,
  `router.js`, etc.), not `.test.js`, so converted test files at
  `runtime/router.test.js` are picked correctly as user frames.

## Requirements

1. **Seven original framework tests convert to `zero/test`.** Every
   test in
   `runtime/{app,router,template,reactivity,http,dom-shim,test}.test.js`
   is either rewritten to import only from `"zero"`, `"zero/test"`,
   and (for `http.test.js`) `"zero/http"`, or deleted. No relative
   imports into `runtime/*.js`. No `_`-prefixed internal exports
   referenced. No reaching into shim-internal state
   (`document._listeners`, `window._listeners`,
   `window.history._entries`, `window.history._index`, etc.).

2. **Trivial mirror tests are deleted.** Anything that asserts a
   property exists rather than that the shim does something —
   `createElement('div').tagName === 'DIV'`,
   setAttribute/getAttribute round-trip, `_joinPaths` mirror tests,
   `_template.fragment` shape introspection, parts-shape introspection
   — goes away. Coverage is provided transitively by every other test
   that renders DOM, navigates, or fetches.

3. **Substantive behaviors are rewritten end-to-end through public
   helpers.** Mapping:
   - DOM queries / selector grammar →
     `render(html\`...\`)` + `find` / `findAll` / `text`
   - Event dispatch (capture/target/bubble, modifiers) →
     `fire(el, type, data)` and observe handler effects
   - `classList` / `dataset` / `style` semantics → render templates
     that bind those props and assert through `find` + property
     reads
   - Timers (`setTimeout`, `setInterval`, `requestAnimationFrame`)
     → call globals directly; `cleanup()` cancels pending work
   - Storage (`localStorage` / `sessionStorage`) → call globals
     directly; `cleanup()` clears them
   - Routing internals (`_normalizePath`, `_compileRoutePattern`,
     `_matchAgainst`, `_parseQuery`, `_parsePathAndQuery`) →
     register routes with `app.route(...)`, drive via
     `navigate(...)`, capture invocation args with `spy()`, assert
     on `params` / `query` from `route()`
   - History (`back`, `forward`) → already public; drive directly
   - Reactivity → `signal` / `computed` / `effect` already public
   - Templates → `html` / `commit` / `each` / `ref` already public
   - HTTP → `createHttp` / `HttpError` already public; stub
     `globalThis.fetch` per test as today

4. **JS↔Rust ABI tests are deleted.** Direct assertions on
   `__getTestTree__` / `__resetTestTree__` go away. The contract is
   covered by `crates/zero-test-runner/src/harness.rs`. A short
   comment block in `runtime/test.js` near both exports documents
   that they are the JS-side ABI consumed by the Rust harness and
   that the Rust integration tests are the authoritative coverage.

5. **`runtime/test.test.js` retains matcher self-tests.** The
   recursive shape (the runner running tests of the runner) is fine
   — if a matcher is broken, the failure manifests as either an
   incorrect pass or a noisy assertion message, both obvious. The
   matcher self-tests construct thrown-error fixtures via
   `try { expect(1).toBe(2) } catch (e) { ... }` and assert on the
   captured error's fields. No `assert.throws`.

6. **Five new shim test files are deleted.**
   `runtime/binary-shim.test.js`, `runtime/clone-shim.test.js`,
   `runtime/encoding-shim.test.js`, `runtime/fetch-shim.test.js`,
   and `runtime/url-shim.test.js` are removed. They test the shim
   source against a Node sandbox so it can be compared to Node's
   native classes; under Boa the comparison collapses (no native to
   shadow). Coverage of each shim's behavior comes from (a) every
   converted framework test running through Boa with the shim
   installed, and (b) the converted `runtime/web-platform.test.js`
   smoke file (Requirement 7).

7. **`runtime/web-platform.test.js` is converted, kept as the
   surface smoke test.** One `describe`, one `it` per audited Web
   Platform API. Imports `describe`, `it`, `expect` from
   `"zero/test"` and nothing else (every API under test is a
   global). Acts as the canary if a shim regresses in a way no
   other behavior test catches. The eleven existing `it`s
   (Headers / Request / Response, AbortController, AbortSignal.any,
   URL / URLSearchParams, TextEncoder / TextDecoder, Blob, File,
   FormData, structuredClone, queueMicrotask,
   Promise.withResolvers) translate one-for-one.

8. **The `cleanup()` boundary is the per-test reset, not module
   reload.** The current `runtime/test.test.js` uses
   `__resetTestTree__()` in `beforeEach` to rebuild the test tree
   between top-level `it`s under `node:test`. After rewriting, the
   test tree is the file's own (built up by the calls to
   `describe` / `it` that drive `zero test`); there is no
   inner-tree to reset. Drop the reset entirely.

9. **`zero test` is runnable from the repo root.** Currently no
   `zero.toml` lives at the workspace root and `zero test` cannot
   be invoked there. The conversion must resolve this — either by
   adding a minimal `zero.toml` at the repo root (scoped so
   discovery walks only `runtime/`, not the nested zero projects
   under `examples/`, `showcase/`, and `crates/*/tests/`), or by
   confirming bare `zero test` works without one. Without a runnable
   command the conversion can't be verified.

10. **After conversion:**
    - The `node --test runtime/*.test.js` invocation is removed
      from `CLAUDE.md` and `README.md`. The replacement command is
      documented in its place. README's "Node.js required only for
      the runtime test suite" prereq line is removed.
    - No remaining file in the repo `import`s from `'node:test'` /
      `'node:assert*'` / `'node:vm'`.
    - All converted tests pass under `zero test`. Failure counts and
      diagnostics are reviewed before the conversion is declared
      done — a silent skip or a deleted test that should have caught
      a real bug is a regression.

11. **No new exports, no loader changes, no Rust changes.** No
    additions to `ZERO_RUNTIME_EXPORTS` / `ZERO_TEST_EXPORTS`. No
    new in-memory module. No widening of the loader. No
    `"zero/internal"` escape hatch. The conversion is entirely on
    the test-file side plus docs plus (possibly) one new
    `zero.toml`. Build-script changes are allowed only if a test
    file is added or removed, since `crates/zero-runtime/build.rs`
    enumerates the shim sources (not the test files).

12. **Tests stay at `runtime/*.test.js`.** No file moves, no `.ts`
    siblings, no rename of `test.test.js`. The conversion is about
    the runner, not the layout.

## Constraints

- **Per-file isolation.** `zero test` boots a fresh Boa context per
  file. Tests that share mutable state across `it`s within a file
  must call `cleanup()` in `afterEach` if they touch document,
  storage, or timers — pattern already established in the existing
  `runtime/test.test.js`. `cleanup()` does not drain
  `window.history`, so files that navigate must explicitly reset URL
  state with `window.history.pushState(null, '', '/')` in their
  test helper.

- **`expect` has no `.not` and no `.rejects` matcher.** Rewrites
  needing negation use `expect(x === y).toBeFalsy()` or
  `expect(spy.callCount).toBe(0)`. Async rejection assertions use
  `try { await p() } catch (e) { ... }` + field assertions. Adding
  to the matcher surface is out of scope.

- **`fire` is the public path for synthetic events.** No imports of
  `Event` / `CustomEvent` / `KeyboardEvent` / `MouseEvent` from the
  shim. The shim's MouseEvent permits assignment of `target` (the
  click-interception tests rely on this), but if a converted test
  needs to observe `preventDefault()` directly, drop the assertion
  — `fire` discards `dispatchEvent`'s return value. Behaviors
  paired with `preventDefault` (e.g., a same-origin click does not
  change `window.location`) are still observable through
  `window.location.pathname`.

- **Performance.** Boa is meaningfully slower than V8. The full
  runtime suite runs slower under `zero test` than under
  `node --test`. Accepted trade-off. Quantify the delta once the
  conversion lands; if the dev loop is materially hurt, file a
  follow-up against the test runner, not against the conversion.

- **`_userFrame` discovery.** Verify after conversion that an
  assertion failure in `runtime/router.test.js` produces a
  `_userFrame` pointing at `router.test.js`, not at `router.js` or
  `test.js`. The basename filter excludes `test.js` / `router.js` /
  etc. but not `*.test.js`, so it should work — but it's easy to
  regress and worth a smoke test as part of the verification step.

- **Path-conflict rule.** Discovery bails if `foo.test.js` and
  `foo.test.ts` both exist. Conversion stays on `.js`.

- **Boa GC compatibility.** New code paths added during conversion
  (helpers in test files, e.g. `freshMount`) should follow the
  branch-into-its-own-function pattern documented in
  `[[boa-maplock-finalizer]]` — keep keyed / code-path-variant
  branches in their own functions. The failure mode is a
  process-exit panic, not a test failure.

- **`runtime/web-platform.test.js` imports only from `zero/test`.**
  No imports from `./http.js` or any other `runtime/` module. The
  current file imports `createHttp` to exercise the
  `Headers/Request/Response` chain; the converted version
  constructs `new Response(...)` directly and asserts on it,
  keeping the test purely about Web Platform globals.

## Out of Scope

- **Adding `Event` / `CustomEvent` / `KeyboardEvent` / `MouseEvent`
  constructors to `"zero/test"` named exports.** `fire()` is the
  public path for synthetic events.
- **Adding internal router primitives (`_normalizePath`,
  `_compileRoutePattern`, etc.) to any public module.** They stay
  module-local.
- **A `"zero/internal"` escape-hatch module.** Considered and
  rejected upstream of the original spec; that rejection still
  holds.
- **Removing Node from the contributor environment entirely.** The
  goal is just to drop the `node --test` invocation against runtime
  tests. Node may still be used elsewhere by individual
  contributors (editor tooling, etc.); the framework doesn't care.
- **TypeScript conversion of test files.** `.js` stays `.js`.
- **Watch mode for `zero test`.** Already on the long-term deferred
  list (`zero-framework-spec.md`).
- **Closing the perf gap with `node:test`.** Separate concern;
  revisit after measurement.
- **Moving the runtime tests out of `runtime/`.** Separate cleanup
  if desired.
- **Coverage parity.** Deleting trivial mirrors and the five
  sandbox-based shim tests will reduce raw line counts and possibly
  mutation kills. Accept the drop; if a specific mutant survives
  that previously died, address it in a follow-up — not by adding
  back the deleted test.
- **Replacing the shim sandbox isolation with a different
  mechanism.** The five `*-shim.test.js` files are deleted, not
  ported to Boa-flavored sandboxing. Shim correctness is asserted
  by the converted `runtime/web-platform.test.js` smoke plus every
  behavior test that runs through the installed shim.

## Open Questions

1. **Repo-root `zero.toml` scope.** The original plan proposed
   `[project] root = "runtime"` so discovery walks only the
   framework's own JS tests. That still seems right — the nested
   zero projects under `examples/`, `showcase/`, and
   `crates/*/tests/` each own their own `zero.toml` and have their
   own integration coverage. Plan author confirms `zero test`
   without any other repo-root file (no `index.html`, no `src/`)
   does not error in `crates/zero/src/cmd/test.rs::run`. If
   future hardening adds such a requirement, a placeholder file is
   acceptable.

2. **Canonical replacement command in `CLAUDE.md` / `README.md`.**
   Candidates: `zero test runtime/` (scoped, requires CLI
   install), `zero test` (whole repo, requires CLI install),
   `cargo run -p zero -- test runtime/` (no install needed). Bias
   toward the `cargo run` form for first-time contributors with a
   note that `zero test` works once installed. Plan author picks
   one and documents it.

3. **Order of conversion.** The original plan converted one file
   per step, smallest first (reactivity → http → template →
   router → app → dom-shim → test). That ordering still works: each
   step leaves both runners in a known state (unconverted files
   still pass under `node --test`, converted files pass under
   `zero test`). The two new units of work — delete the five shim
   tests, convert `runtime/web-platform.test.js` — can ride at the
   end (e.g., as one cleanup step before the final docs/verification
   step) or be folded into Step 1 (since they have no dependency on
   the seven core conversions). Plan author's call.

4. **Discovery walks `runtime/` automatically.** Confirm that the
   discovery walker picks up `runtime/*.test.js` when invoked from
   the repo root with the new `zero.toml`. Spot-check with a `zero
   test` dry-run before starting the rewrite — if discovery skips
   `runtime/` for any reason, the conversion needs to adjust the
   layout or the discovery rules first.

5. **`runtime/test.test.js` rename.** Under `zero test`, the file
   is discovered and its tests run inside the same runner they
   describe. The recursion is fine technically but the name is now
   slightly funny. Optional rename to e.g. `test-api.test.js` for
   clarity; or leave it. Plan author's call. (Requirement 12
   prohibits *moves*; a rename within `runtime/` is a question of
   churn-vs-clarity, not layout.)
