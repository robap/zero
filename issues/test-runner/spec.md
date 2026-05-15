# Spec: `zero test` — built-in test runner

## Problem Statement

The framework spec lists `zero test` as a first-class CLI subcommand and
describes a complete test API surface (`describe`/`it`/`expect`, DOM
helpers, snapshots, coverage). Nothing of it exists yet. The scaffold
already ships a `*.test.js` file that imports from `"zero/test"`, so a
developer running the canonical scaffold today gets a confusing missing-
module error if they try to run the test the way the scaffold implies.

`node --test` works for the framework's own `runtime/*.test.js` suite,
but pushing Node onto every downstream zero app breaks the "single
binary, batteries included, zero npm deps" pitch. The framework needs
its own test runner so the scaffold runs out of the box, and so the
test API surface (which the spec advertises) actually exists.

This slice ships the MVP: discovery, the test-structure API, the DOM
helpers, and a default reporter. `--watch`, `--coverage`, and snapshot
testing are deferred to follow-up slices.

## Background

### What exists today

- `runtime/reactivity.js`, `runtime/template.js`, `runtime/router.js`,
  `runtime/app.js` — the Phase 1–3 runtime. All ES modules.
- `runtime/dom-shim.js` — a ~345-line lightweight DOM implementation
  that the runtime's own `node:test` suite uses. Provides
  `document.createElement`, querySelector by tag or `#id`, event
  dispatch, `window.history` with `pushState`/`replaceState`/popstate,
  `window.location`. Installs itself onto `globalThis` on first import.
  Already adequate for everything the runtime uses today.
- `src/runtime.rs` exports `ZERO_RUNTIME_BODY` (the imports-stripped /
  exports-stripped concatenation of the runtime JS files, emitted by
  `build.rs` at compile time) and `ZERO_RUNTIME_EXPORTS` (the public
  name list). `runtime_module()` composes them into a complete ES
  module string for `/zero.js` and the bundler.
- `src/build/bundler.rs` — a CommonJS-style module-graph walker that
  pulls in the runtime + user modules and emits a single bundle.
- The scaffold's `src/routes/home.test.js` already imports `describe`,
  `it`, `expect`, `render`, `find`, `text`, `fire`, `cleanup`,
  `afterEach` from `"zero/test"` and `signal` from `"zero"`. **This
  file is the canonical acceptance target** — once this slice ships,
  `zero test` must run that file green.
- `Cargo.toml` already pulls in `clap`, `tokio`, `axum`, `regex`,
  `notify-debouncer-mini`, etc.; no JS engine yet.

### Why an embedded JS engine

Shelling out to Node / Deno / Bun would force every downstream zero
project to install a foreign runtime just to run their tests, which
contradicts the framework's "single binary, no external runtime"
positioning. Instead, `zero test` runs JS in-process via an embedded
engine. The decision was made up-front to use **Boa** — a pure-Rust
ECMAScript engine. Boa supports ES2022, ES modules, async/await, and
top-level await, which is enough for everything the spec advertises.

Trade-offs accepted:

- **Speed:** Boa is meaningfully slower than V8. For test suites at
  the scale a zero app realistically grows to (tens to a few hundred
  tests), this is fine; speed becomes a real problem only at much
  larger scales, and at that point a different engine choice can be
  revisited without changing the test API surface.
- **Spec coverage:** Boa doesn't implement 100% of the spec. The
  runtime and the scaffolded tests already work in `node:test`; if any
  feature they rely on isn't supported by Boa, we either rewrite the
  runtime to avoid it or file a Boa bug and work around it. The plan
  must include a smoke pass that runs the existing
  `runtime/*.test.js` files (re-imported into the new harness) under
  Boa to surface any incompatibilities before they hit a user.

### Why a Rust-orchestrated harness, not a pure-JS bootstrap

We could write the entire test runner in JS and have Rust just embed
+ invoke it. We're not doing that. Reasons:

1. File discovery (walking `<root>/**/*.test.js`) is trivial in Rust
   and avoids needing to expose a filesystem API to Boa.
2. CLI parsing and exit codes live in Rust already; adding the test
   subcommand there is natural.
3. The plan phase can decide how much of the harness (test
   collection, runner loop, reporter) lives in Rust vs. JS, but the
   spec's intent is: Rust drives, JS provides the test API surface
   that user code imports.

### The `zero/test` virtual module

User test files do `import { describe, it, expect, render, find,
text, fire, cleanup } from "zero/test"`. The runtime concatenation
already handles `import ... from "zero"` (by virtue of being the
bundled runtime). `zero/test` needs the same treatment: an embedded
JS module that lives only at test time, never shipped to the browser,
never reachable from `zero dev` / `zero build`.

The new module exports the test API: structure functions
(`describe`/`it`/etc.), assertion factory (`expect`), DOM helpers
(`render`/`find`/etc.), and a stub-app installer used by `render` to
satisfy `inject()` calls.

### How `render()` makes `inject()` work

`runtime/app.js` exposes `_setCurrentApp(app)` for tests. The test
runner's `render(templateResult, { state })` helper builds a minimal
stub app whose `_state` map is seeded from the `state` object passed
in, sets it as the current app, commits the template into a fresh
container in a fresh scope, returns the container's first element.
`cleanup()` disposes those scopes and clears `_currentApp` so tests
don't leak state into each other. This pattern is what
`<root>/src/routes/home.test.js` already assumes.

## Requirements

### CLI surface

- New subcommand: `zero test [target]`. Wired into the existing clap
  dispatch in `src/main.rs` next to `init`/`dev`/`build`.
- `target` is optional, single positional argument. Resolution rule:
  1. If `target` is the path to an existing regular file (resolved
     relative to the project root), run only that file. The file
     does not have to match `*.test.js` / `*.spec.js` naming — the
     developer is being explicit and the runner trusts them.
  2. Otherwise, treat `target` as a substring and include any
     discovered test file whose path (relative to project root)
     contains it.
  Examples: `zero test` (all), `zero test web/src/routes/home.test.js`
  (just that file), `zero test routes` (any file with "routes" in
  its path), `zero test home.test` (any file matching that substring).
  Multiple positional args, glob patterns, and test-name filters are
  out of scope.
- Exit code: `0` if all tests pass, non-zero (e.g. `1`) on any failure
  or harness error. Standard CLI convention.
- Help text consistent with the existing subcommands.
- No other flags in this slice (`--watch`, `--coverage`,
  `--update-snapshots` are out of scope).

### Boa integration

- Add `boa_engine` (and whatever sibling crates Boa requires for ES
  module loading) to `Cargo.toml`. The plan picks the exact crate set
  and version; the spec only requires that ES modules with async/await
  work.
- Boa context is owned by the Rust runner. Lifetime spans one
  `zero test` invocation; no daemon, no warm pool.
- The runner sets up a custom module loader that resolves:
  1. `"zero"` → the in-memory `runtime_module()` string. Same surface
     as `/zero.js`.
  2. `"zero/test"` → the embedded test-API module (see below).
  3. Relative specifiers (`"./home.js"`, `"../foo.js"`) → file at that
     path on disk, resolved relative to the importer.
  4. Bare specifiers other than `"zero"` and `"zero/test"` → error.
     No npm support, no node_modules walk.
- Before any user code runs, the runner must install the DOM shim
  globals (`document`, `window`) onto Boa's global object. The
  existing `runtime/dom-shim.js` is the source; the plan picks
  whether to evaluate it as a JS module that side-effects onto
  `globalThis`, or to expose the same surface from Rust. Either way,
  user code that reads `document.querySelector` must work without an
  explicit import.
- Top-level errors thrown during module load are reported as harness
  failures with the file path and stack trace, then the next file is
  tried. One bad file does not abort the run.

### The `zero/test` module

Embedded as another runtime string (alongside `ZERO_RUNTIME_BODY`),
emitted by `build.rs` from a source file at `runtime/test.js` (path
the plan may adjust). The module exports:

#### Structure

- `describe(name, fn)` — groups its body's `it` calls under `name`.
  Nested `describe` is supported. `fn` may be sync or async; an async
  `fn` is awaited during collection.
- `it(name, fn)` — registers a test. `fn` may be sync or async. A
  thrown value, rejected promise, or failed `expect` marks the test
  as failed.
- `beforeEach(fn)` / `afterEach(fn)` — run before/after each `it`
  within the enclosing `describe` (and its nested descendants).
  Multiple registrations stack in registration order.
- `beforeAll(fn)` / `afterAll(fn)` — run once before/after the
  enclosing `describe`. Multiple registrations stack.
- All hooks support sync or async functions.

#### Assertions

`expect(actual)` returns an object with at least:

- `.toBe(expected)` — strict equality (`===`).
- `.toEqual(expected)` — structural deep equality on plain objects,
  arrays, primitives. Plan defines the exact algorithm; deferring to
  a small hand-rolled one is fine.
- `.toBeTruthy()` / `.toBeFalsy()`
- `.toBeNull()`
- `.toContain(item)` — for strings (substring) and arrays
  (`indexOf >= 0`).
- `.toThrow(message?)` — `actual` must be a function; call it,
  succeed if it throws; if `message` is a string, the thrown error's
  message must contain it.
- `.toBeTemplateResult()` — passes if the value has `_template` and
  `_values` properties matching the runtime's template shape (see
  the `_isTemplateResult` check in `runtime/template.js`).

`.toMatchSnapshot()` is **out of scope for this slice** (snapshot
follow-up). A test that calls it must error clearly so users get a
useful message instead of `undefined is not a function`.

A failing assertion throws synchronously with a message that includes
the matcher name, actual value, and expected value (where applicable).
The runner catches and records.

#### DOM helpers

- `render(templateResult, opts?)` — installs a stub app (seeding
  `inject()` keys from `opts.state ?? {}`), creates a fresh ownership
  scope, commits `templateResult` into a fresh container element,
  registers the scope and container with the cleanup tracker, returns
  the **first element child** of the container. (Returning the
  container itself surfaces the comment-anchor children the template
  system inserts; returning the first element matches what
  `find`/`text` callers expect.)
- `find(el, selector)` — wrapper over `el.querySelector(selector)`.
  The dom-shim now supports compound selectors composed of tag, `#id`,
  `.class`, `[attr]`, and `[attr=value]` (quoted or unquoted) parts
  against a single element (e.g. `button.btn[type=submit]`). Combinators
  (descendant, child, sibling), pseudo-classes, and attribute operators
  beyond `=` are still deferred; see `issues/test-helpers/spec.md`.
- `findAll(el, selector)` — wrapper over `el.querySelectorAll`.
- `text(el, selector?)` — if `selector` is omitted, returns the
  concatenated text content of all `nodeType === 3` descendants of
  `el`. If `selector` is provided, first does `el.querySelector(selector)`
  and reads from that. Throws if the selector matches nothing (helps
  catch test bugs early — the alternative, returning `""`, hides
  them).
- `fire(el, event, data?)` — constructs a minimal event object
  (`{ type, ...data, preventDefault, stopPropagation }`) and calls
  `el.dispatchEvent(...)`. Synchronous.
- `cleanup()` — disposes every scope `render()` created since the
  last `cleanup()`, clears the current-app stub, drops references to
  rendered containers. Safe to call multiple times.

### Discovery

- Walk `<root>/` (the value of `[project] root` from `zero.toml`)
  recursively, collecting files whose basename matches `*.test.js` or
  `*.spec.js`.
- Skip hidden files / directories (anything whose path component
  starts with `.`).
- Skip `node_modules` (defensive — zero apps shouldn't have it, but
  if a developer drops one in for editor support, don't crawl it).
- Skip the build output directory `<out>` if it sits under `<root>`
  (mirrors the dev-watch ignore).
- Target filter (if `target` was provided to the CLI):
  - If `target` resolved to a specific file, discovery is bypassed
    entirely and that single file is the run set.
  - Otherwise, the substring filter is applied to each candidate
    path (relative to project root).
- Discovery order: sort by path for deterministic output. Run order
  follows discovery order.
- One file at a time, in a fresh Boa context per file. This keeps
  module-level mutable state in the runtime (e.g. `_currentApp`,
  `_observerStack`) from leaking between files and gives clean
  module identity for the runtime/test-helper modules. Within a
  file, all `describe`/`it` blocks share one context — that's the
  unit of isolation users expect from other runners.

### Reporter

- One default reporter, human-readable, written to stdout. Format
  details are a plan-level decision; the spec requires:
  - Each `it` produces a single line indicating pass / fail, the
    test name (with parent `describe` names joined by ` > `), and
    the file path.
  - Failures print, after the per-test line, the assertion message
    and a stack trace if available.
  - A final summary line: `N passed, M failed, K skipped in <time>`.
- No TAP, no JSON, no JUnit XML in this slice.
- No colorization required in MVP; if Boa or the plan wants to use
  ANSI colors, fine, but it's not a requirement.

### Test lifecycle

For each discovered file:

1. Create a fresh Boa context.
2. Install DOM shim globals.
3. Load `"zero"` and `"zero/test"` modules into the context's loader
   cache (or arrange so the first import resolves them).
4. Dynamically import the test file. Top-level `describe(...)` calls
   register tests on a module-level test tree owned by `"zero/test"`.
5. After import resolves, walk the test tree:
   - For each `describe` (depth-first):
     - Run all `beforeAll` hooks.
     - For each `it`:
       - Run all `beforeEach` hooks from this `describe` and all
         ancestors (outermost first).
       - Run the test body.
       - Run all `afterEach` hooks (innermost first).
       - Record pass/fail.
     - Recurse into nested `describe`s.
     - Run all `afterAll` hooks.
6. Report per-file results, accumulate to the overall summary.
7. Drop the context.

Failures in `beforeAll`/`beforeEach`/`afterEach`/`afterAll` are
reported but don't abort the whole file; subsequent tests in the
same `describe` are marked as skipped (with a reason) so the run
can complete and the developer sees all the wreckage.

### Async details

- An async `it` body returns a promise; the runner awaits it.
- An async hook returns a promise; the runner awaits it.
- A test that hangs (promise never resolves) blocks the runner.
  Per-test timeout is **out of scope for this slice** (deferred);
  the developer can Ctrl-C. The plan should at least make Ctrl-C
  responsive (Boa's event loop integration must not swallow it).
- Microtasks scheduled inside a test must drain before the runner
  moves on. Boa's event loop handles this; the runner just `await`s
  the body.

### Module-level state safety

- Each file gets its own context, so the runtime concatenation
  (which uses module-level `let`s like `_observerStack`,
  `_activeScope`, `_currentApp`) is re-initialized per file. Within
  a file, multiple tests share that state — `cleanup()` is the
  contract that keeps it clean.
- `cleanup()` MUST:
  - Dispose every scope created via `render()`.
  - Call `_setCurrentApp(null)` (or the equivalent).
  - Empty any internal tracking arrays.
- The scaffolded `home.test.js` uses `afterEach(cleanup)` —
  this idiom must work and is the recommended pattern.

### Where things live in the repo

- `runtime/test.js` — the source of the `zero/test` module.
  Concatenated by `build.rs` into an embedded constant alongside
  `ZERO_RUNTIME_BODY`. JSDoc'd per the project's `CLAUDE.md` rule.
- `runtime/test.test.js` — `node:test` self-tests for the test API
  (run by `node --test runtime/*.test.js`, the same way the rest of
  the runtime is tested). This bootstrap matters: the test runner
  testing itself with itself is a chicken-and-egg problem we avoid
  by keeping a small `node:test` safety net for `runtime/test.js`
  during development. Once the runner is stable, the framework's own
  CI can move to using `zero test` for everything; for this slice,
  keep the `node:test` suite.
- `src/cmd/test.rs` — the subcommand entry point.
- `src/test_runner/` — the Boa-driver implementation. Module
  decomposition (loader, harness, reporter) is a plan-level decision.
- `src/runtime.rs` — extended with a second exported constant
  (`ZERO_TEST_BODY` or similar) and possibly a `test_module()`
  helper analogous to `runtime_module()`.
- `build.rs` — extended to also process `runtime/test.js` with the
  same imports-stripped / exports-stripped treatment.

### Tests (this slice's own tests)

- `cargo test` Rust unit tests:
  - Discovery walks the tree correctly, applies substring filter,
    bypasses discovery when `target` resolves to a file, ignores
    hidden / `node_modules` / `<out>`, sorts deterministically.
  - The Boa-driver code resolves `"zero"`, `"zero/test"`, and
    relative paths correctly; rejects bare specifiers it doesn't
    know.
  - Reporter formats pass/fail lines and the summary as expected.
- `cargo test` integration tests (à la `tests/e2e_init_dev.rs`):
  - Scaffold a temp project, run `zero test` on it, assert the
    scaffold's `home.test.js` passes (the canonical acceptance
    target).
  - A failing test produces non-zero exit and prints the failure.
  - A test file with a top-level syntax error is reported, the run
    continues, and the exit code is non-zero.
- `node --test runtime/test.test.js`:
  - Standalone tests of `describe`/`it` collection, `expect` matchers,
    DOM helper behavior, `cleanup()` semantics. These run under
    Node so they're not gated on the Boa integration.
- Existing `node --test runtime/*.test.js` continues to pass — this
  slice does not modify the rest of the runtime.

## Constraints

- **No Node, Deno, or Bun in `zero test`.** Tests run inside the
  embedded Boa engine. The single zero binary is the only thing the
  user needs installed.
- **One new Rust dependency family: Boa.** Whatever crates Boa
  needs to load ES modules and run async. No other engine choice in
  this slice.
- **No npm dependencies** — never, anywhere.
- **No external test framework** for downstream apps. `zero/test` is
  the only test API a zero developer imports.
- **No HMR / `--watch` / `--coverage` / `--update-snapshots` /
  `--reporters` flags.** All deferred.
- **No snapshot testing in this slice.** `.toMatchSnapshot()` must
  error clearly if called; full snapshot semantics land in a follow-up.
- **No browser, no jsdom.** The existing `runtime/dom-shim.js` is the
  DOM. Gaps in the shim are addressed only as encountered by real
  tests in this slice; broader DOM coverage is a separate concern.
- **Per-file isolation.** A new Boa context per file. Within a file,
  tests share state and rely on `cleanup()` (or `beforeEach`-reset
  patterns) for hygiene.
- **No npm-style bare-specifier resolution.** Only `"zero"` and
  `"zero/test"` are recognized as bare specifiers; everything else
  must be a relative path. (No `import "lodash"` etc.)
- **The test runner must not be reachable from `zero dev` or
  `zero build`.** The `zero/test` module is never served to the
  browser, never bundled into production output.
- **No per-test timeout.** A hung promise blocks the runner; the
  developer Ctrl-Cs. Configurable timeouts land later.
- **No test parallelism.** Files run sequentially. The Boa context
  is not thread-safe and parallelizing across files would mean
  spawning multiple contexts — possible but out of scope for this
  slice. Tests within a file are inherently sequential (shared
  mutable state).
- **Discovery is whole-`<root>/` recursive.** Not configurable via
  `zero.toml` in this slice. No `[test]` section. If real projects
  hit friction (e.g., wanting to separate unit and integration
  trees), a follow-up can add `[test] roots = [...]`.
- **No TypeScript.** `.test.js` and `.spec.js` only. TS support is a
  separate spec.

## Out of Scope

- `--watch` mode (re-run on file change). The dev-watch SSE
  infrastructure could be adapted, but it's a separate slice.
- `--coverage` reporting. Boa instrumentation hooks are non-trivial.
- Snapshot testing (`.toMatchSnapshot()`, `--update-snapshots`,
  snapshot file management).
- `settled()` async-quiescence helper. Useful for testing async
  navigations and route transitions; defer until a real test needs it.
- `--reporters` / alternate output formats (TAP, JSON, JUnit XML).
- Per-test timeouts and `--timeout` flag.
- Test parallelism (multiple Boa contexts running concurrently).
- Web Component (`z/wc`) test support — that whole module is an
  escape hatch and ships separately.
- Hot-reloading test files (re-running only changed files based on
  module-graph analysis).
- TypeScript test files (`.test.ts`, `.spec.ts`).
- A `--bail` flag to stop on first failure.
- A `--only` / `.only` mechanism. Easy to add later; the scaffold
  doesn't need it yet.
- A `--skip` / `.skip` mechanism. Same.
- Mocking utilities (module mocks, deep stubs). Spies ship in the `zero/test` selector-grammar + spy slice; see `issues/test-helpers/spec.md`.
- A `vi`-like global test API (Vitest-style globals). Imports only.
- IDE/editor protocol integration (running a single test from a
  VS Code lens, etc.).
- DOM coverage beyond what `runtime/dom-shim.js` already provides.

## Open Questions

- **Boa crate ergonomics for ES modules.** Boa's module loader API
  has changed across versions; the plan must verify against the
  current release that we can plug in a custom resolver that returns
  in-memory strings for `"zero"` and `"zero/test"` alongside on-disk
  files for relative paths. If module loading turns out to be
  rougher than expected, a fallback is to use Boa's script eval
  (not modules) and have the harness transpile `import`/`export`
  syntax. Strongly prefer ES modules — that's what user code is
  written in — but record the fallback so it isn't novel-thinking
  at implementation time.
- **DOM shim install path.** Option A: evaluate `runtime/dom-shim.js`
  as a side-effecting script in the Boa context (it already installs
  onto `globalThis` on import). Option B: skip the JS file and
  re-implement the shim's surface in Rust, exposing methods on Boa's
  global. Recommendation: Option A — reuse what works, no second
  source of truth.
- **Test tree storage.** Where does the running test tree live? Most
  natural: module-level state inside the `zero/test` module instance
  loaded into each Boa context. The Rust side queries the tree via
  a named export (`__getTestTree__` or similar) after the user file
  import resolves. Plan picks the exact shape.
- **What `text(el, selector?)` returns for nested element text.**
  The shim's text nodes are at `nodeType === 3`. Recommendation:
  recursively concatenate text descendants in source order, no
  whitespace collapse, no trimming. Matches what `Node.textContent`
  does in real browsers.
- **Failure message format.** Plan-level call. Recommendation:
  `expect(<actual>).toEqual(<expected>)` style with both values
  pretty-printed (small bespoke pretty-printer, not `JSON.stringify`,
  because `JSON.stringify` can't render functions / circular refs /
  signals well).
- **Console output during tests.** A `console.log` inside a test
  body should be captured (or at least not interleaved
  unintelligibly with the reporter). Recommendation for MVP:
  pass-through `console.log` straight to stdout; if it makes
  failure investigation worse rather than better in practice, add
  capture as a follow-up.
- **Error stack traces.** Boa's error stacks point at script
  coordinates, not original file paths if we eval'd via a string.
  The plan should preserve source filenames when loading modules
  so stacks are useful. If Boa drops source-map information, the
  fallback is to at least print the file path the failure originated
  from (which we already know).
- **`expect.toEqual` deep equality for signals.** Signals are
  objects with a getter `.val`; structural-equality across two
  signals would either recurse forever or compare object identity.
  Recommendation: `.toEqual` treats objects with the runtime's
  signal shape (a `val` getter) by comparing their `.val`s. Document
  this in the assertion's behavior section once the matcher is
  written.
- **Where the framework spec text needs amendment.** §8 currently
  mentions `--watch`, `--coverage`, `--update-snapshots`, and
  `.toMatchSnapshot()`. This slice ships none of those. Phase 5 in
  §12 should split into sub-items so post-merge it's clear which
  parts are done vs. deferred. The implementation PR should include
  the spec-text edits.
- **Bootstrap loop for `runtime/test.js`'s own tests.** Today the
  framework's `node --test` suite tests the runtime. After this
  slice, the test runner exists but `runtime/test.js` (the API
  module) should still be tested under `node:test` for
  unbreakable-by-design reasons (we can't trust the runner to
  validate the API the runner depends on). Plan: keep
  `runtime/test.test.js` under `node:test`; once the runner has
  shipped and been used a while, revisit whether to migrate.
- **Where to draw the binary-size line.** Boa is pure-Rust but not
  small. The plan should measure the resulting `zero` binary
  before/after and note it; if the jump is uncomfortable (say,
  >25–30MB), revisit whether the test runner ships in the same
  binary or a separately-built one. Recommendation: keep in the
  same binary unless the size is genuinely user-hostile; the
  "single binary" property is load-bearing.
