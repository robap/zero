# Plan: `zero test` — built-in test runner

## Summary

Ship a built-in `zero test` subcommand that runs `*.test.js` / `*.spec.js`
files in-process under an embedded Boa engine, so a freshly-scaffolded zero
app can run its tests with no external runtime. The plan builds bottom-up:
first the JS-side test API (`runtime/test.js`, validated under `node:test`),
then the Rust pieces (build-time embedding, discovery, Boa harness,
reporter), then the CLI wiring and the scaffold update so `zero init` emits
the test file that `zero test` is meant to run. Snapshots, watch, coverage,
and a parallel/per-test-timeout layer are deferred.

## Prerequisites

None. All spec open questions are either resolved by the recommendations in
the spec (DOM shim install via Option A, `text()` recursive concat,
`signal` deep-equality, console pass-through, in-binary Boa) or recorded as
local plan-time decisions below. The framework-spec amendment to §8/§12 is
a follow-up doc edit, not a blocker.

## Steps

- [x] **Step 1: Author `runtime/test.js` (test API surface) under `node:test`**
- [x] **Step 2: Embed `runtime/test.js` and `runtime/dom-shim.js` at build time**
- [x] **Step 3: Add Boa, write the `src/test_runner/` skeleton (loader + harness)**
- [x] **Step 4: Implement discovery (`src/test_runner/discovery.rs`)**
- [x] **Step 5: Implement the reporter (`src/test_runner/reporter.rs`)**
- [x] **Step 6: Wire the `zero test` subcommand into the CLI**
- [x] **Step 7: Emit `home.test.js` from `zero init`**
- [x] **Step 8: End-to-end integration tests + Boa smoke pass over `runtime/*.test.js`**

---

## Step Details

### Step 1: Author `runtime/test.js` (test API surface) under `node:test`

**Goal:** Get the entire user-facing test API working as plain JS, validated
under Node's existing `node --test` harness. This decouples API correctness
from the Boa wiring and gives the executor a working module before any Rust
code touches it. After this step the module is importable in node-land via
relative paths; the next step embeds it for use under `"zero/test"`.

**Files:**
- `runtime/test.js` (new)
- `runtime/test.test.js` (new)
- `runtime/reactivity.js` (modify — export `_createScope` under the
  test-internal alias the test module will import; see below)

**Changes:**

1. **Expose internals needed by `runtime/test.js`.** `runtime/test.js`
   needs `_setCurrentApp`, `_createScope`, and `commit` from the runtime.
   `commit` and `_setCurrentApp` already exist as named exports; only
   `_createScope` (currently aliased via `export { createScope as _createScope }`
   in `runtime/reactivity.js`) needs no change — the alias is already
   public-with-underscore. `_setCurrentApp` is exported from `app.js`.
   In Step 2 these names will be added to `ZERO_RUNTIME_EXPORTS` so they
   resolve through `"zero"`; in this step, `runtime/test.js` imports them
   from the relative paths (`./reactivity.js`, `./app.js`, `./template.js`)
   so `node:test` can run it directly. Step 2 swaps the import target to
   `"zero"` once the test module is loaded inside Boa via the runtime
   wrapper.

   **Resolution chosen:** Author `runtime/test.js` with relative imports
   (`./reactivity.js`, `./app.js`, `./template.js`). Build.rs (Step 2)
   will strip those imports identically to how it strips runtime imports
   today, so when the file is concatenated into the in-Boa runtime body
   the symbols are already in scope.

2. **`runtime/test.js` exports — full surface:**

   ```js
   // Test tree
   export function describe(name, fn)
   export function it(name, fn)
   export function beforeEach(fn)
   export function afterEach(fn)
   export function beforeAll(fn)
   export function afterAll(fn)

   // Assertions
   export function expect(actual)            // returns matcher object

   // DOM helpers
   export function render(tr, opts)          // returns first element child
   export function find(el, selector)
   export function findAll(el, selector)
   export function text(el, selector?)
   export function fire(el, eventType, data?)
   export function cleanup()

   // Harness hooks (internal — Rust side calls these)
   export function __getTestTree__()
   export function __resetTestTree__()
   ```

3. **Test tree shape (module-level state in `test.js`):**

   ```js
   // Each Describe node:
   //   { name, parent, children: Array<Describe|It>,
   //     beforeAll: Fn[], afterAll: Fn[],
   //     beforeEach: Fn[], afterEach: Fn[] }
   // Each It node:
   //   { name, fn, parent }
   let _root   = makeDescribe("", null);   // unnamed top-level group
   let _current = _root;                   // describe-stack pointer
   ```

   - `describe(name, fn)` pushes a new Describe under `_current`, sets
     `_current` to it, awaits `fn()` (if it returns a Promise),
     restores `_current`.
   - `it(name, fn)` pushes an It under `_current`.
   - `beforeEach`/`afterEach`/`beforeAll`/`afterAll` append to the
     corresponding array on `_current`.
   - `__getTestTree__()` returns `_root`.
   - `__resetTestTree__()` reinitializes `_root` and `_current` (only used
     by the node:test self-tests; under Boa we get a fresh context per file).

4. **`render(tr, opts = {})`:**

   ```js
   import { commit } from "./template.js";
   import { _createScope } from "./reactivity.js";
   import { _setCurrentApp } from "./app.js";

   const _renderTracker = []; // Array<{ scope, container }>

   export function render(tr, opts = {}) {
     const stateMap = new Map(Object.entries(opts.state ?? {}));
     const stub = {
       _state: stateMap,
       _getState(key) {
         if (!stateMap.has(key))
           throw new Error(`inject: key "${key}" is not registered`);
         return stateMap.get(key);
       },
     };
     _setCurrentApp(stub);
     const scope = _createScope();
     const container = document.createElement("div");
     scope.run(() => commit(tr, container));
     _renderTracker.push({ scope, container });
     for (const child of container.childNodes) {
       if (child.nodeType === 1) return child;
     }
     return null; // template rendered no element root
   }
   ```

   `cleanup()` iterates `_renderTracker`, calls `scope.dispose()` on
   each, calls `_setCurrentApp(null)`, and truncates the tracker.

5. **`find` / `findAll` / `text` / `fire`:** straight pass-throughs over
   `el.querySelector` / `el.querySelectorAll`. `text(el, selector?)`:

   ```js
   export function text(el, selector) {
     const target = selector ? el.querySelector(selector) : el;
     if (selector && target == null)
       throw new Error(`text: selector "${selector}" matched nothing`);
     let out = "";
     (function walk(node) {
       if (node.nodeType === 3) out += node.nodeValue;
       if (node.childNodes) for (const c of node.childNodes) walk(c);
     })(target);
     return out;
   }

   export function fire(el, type, data = {}) {
     let prevented = false;
     el.dispatchEvent({
       type, ...data,
       preventDefault() { prevented = true; },
       stopPropagation() {},
       get defaultPrevented() { return prevented; },
     });
   }
   ```

6. **`expect(actual)` matcher object:** plain object literal, each matcher
   throws an `Error` with a descriptive message on mismatch.

   - `.toBe(expected)` — `actual === expected`.
   - `.toEqual(expected)` — hand-rolled deep equality:
     - same reference → pass
     - primitives → strict equality
     - both arrays → same length and `toEqual` per index
     - both plain objects (Object.getPrototypeOf === Object.prototype OR
       null) → same key set and `toEqual` per key
     - signal-shaped (`val` getter, `set` function): compare their `.val`s
       recursively
     - otherwise → strict equality
     A small `_pretty(v)` helper renders values for the error message
     (string-quotes, array brackets, object braces, signal as
     `signal(<val>)`; circular refs become `[Circular]`).
   - `.toBeTruthy()` / `.toBeFalsy()` — `Boolean(actual)`.
   - `.toBeNull()` — `actual === null`.
   - `.toContain(item)` — string substring or array `indexOf >= 0`.
   - `.toThrow(message?)` — `typeof actual === "function"`; call it; pass
     if it throws. If `message` is a string, the caught error's `message`
     must contain it.
   - `.toBeTemplateResult()` — inline check (no import needed):
     `actual != null && typeof actual === "object" && actual._template != null && Array.isArray(actual._values)`.
   - `.toMatchSnapshot()` — stub that throws
     `Error("toMatchSnapshot: snapshot testing is not in this slice yet")`.

7. **JSDoc:** every exported function gets `@param`/`@returns`/`@template`
   per the project's `CLAUDE.md` rule. Module-level state (`_root`,
   `_current`, `_renderTracker`) gets `@type`. Internal helpers
   (`__getTestTree__`, `__resetTestTree__`, `_pretty`) get `@internal`.

**Tests:** `runtime/test.test.js` — a `node:test` suite (mirroring the
pattern in `runtime/app.test.js`). At minimum:

- `describe` nests; `it` registers; `__getTestTree__` returns the
  expected shape.
- Hook arrays accumulate in registration order.
- `expect(1).toBe(1)` passes; `.toBe(2)` throws with a message containing
  the actual and expected values.
- `expect({a:1}).toEqual({a:1})` passes; `.toEqual({a:2})` throws.
- `expect([1,2,3]).toContain(2)` passes; `.toContain(4)` throws.
- `expect(() => { throw new Error("boom") }).toThrow("boom")` passes;
  `.toThrow("nope")` throws.
- `expect(signal(0)).toEqual(signal(0))` passes (deep-equal-on-`.val`).
- `expect(html``).toBeTemplateResult()` passes.
- `.toMatchSnapshot()` throws with the deferred-feature message.
- `render(html`<p>hi</p>`)` returns a `<p>` element; `text()` returns "hi".
- `render` honoring `opts.state`: `inject("k")` resolves to the registered
  value.
- `cleanup()` disposes scopes (verify by counting effect stops via a
  sentinel signal that the rendered template depends on, then confirming
  it doesn't re-render after a `.set()` post-cleanup).
- `fire(el, "click")` calls the registered handler.

This file is run by the existing `node --test runtime/*.test.js` command;
no Rust changes are required for it to be picked up.

---

### Step 2: Embed `runtime/test.js` and `runtime/dom-shim.js` at build time

**Goal:** Make the test API and the DOM shim available to the embedded
Boa engine as in-memory module strings produced by the existing
build-time stripping pipeline. Mirrors the way `ZERO_RUNTIME_BODY` is
already produced from `RUNTIME_FILES`. After this step the binary
contains the three strings (runtime, test API, dom-shim) and exposes
helpers to assemble each as a complete module.

**Files:**
- `build.rs` (modify)
- `src/runtime.rs` (modify)

**Changes:**

1. **`build.rs`:** factor the stripping pipeline into a function
   `clean_runtime_source(raw: &str) -> (String, String)` returning
   `(cleaned_body, alias_lines)`. Call it three times:
   - Concatenate the existing `RUNTIME_FILES` (reactivity, template,
     router, app) into `zero_runtime_body.js` exactly as today.
   - Process `runtime/dom-shim.js` into `zero_dom_shim_body.js`.
   - Process `runtime/test.js` into `zero_test_body.js`.

   Each `cargo:rerun-if-changed=runtime/<f>` entry must be added for the
   two new inputs. The function must continue to (a) strip top-level
   imports — including the new relative imports added by `runtime/test.js`
   — (b) convert `export X` to plain declarations, (c) flatten
   `export { x as y }` to `const y = x;`, and (d) drop bare
   `export { name }` re-export blocks.

2. **`src/runtime.rs`:** add two new constants and a `test_module()`
   helper.

   ```rust
   pub const ZERO_RUNTIME_BODY: &str =
       include_str!(concat!(env!("OUT_DIR"), "/zero_runtime_body.js"));

   pub const ZERO_DOM_SHIM_BODY: &str =
       include_str!(concat!(env!("OUT_DIR"), "/zero_dom_shim_body.js"));

   pub const ZERO_TEST_BODY: &str =
       include_str!(concat!(env!("OUT_DIR"), "/zero_test_body.js"));

   pub const ZERO_RUNTIME_EXPORTS: &[&str] = &[
       "signal", "computed", "effect", "html", "commit", "each", "ref",
       "App", "inject", "navigate", "back", "forward", "route",
       // Internals needed by the test API. Underscore-prefixed
       // signals "not part of the public API"; they are still real
       // exports so `runtime/test.js`'s post-strip references can
       // resolve when Boa's loader composes "zero".
       "_setCurrentApp", "_createScope", "_getCurrentApp",
   ];

   pub const ZERO_TEST_EXPORTS: &[&str] = &[
       "describe", "it", "beforeEach", "afterEach",
       "beforeAll", "afterAll",
       "expect",
       "render", "find", "findAll", "text", "fire", "cleanup",
       "__getTestTree__", "__resetTestTree__",
   ];

   pub fn runtime_module() -> String { /* unchanged in shape */ }

   /// Build the `zero/test` module string: the runtime body, then the
   /// test body (so the test body's stripped imports of `_createScope`
   /// etc. resolve against the runtime symbols), then a trailing
   /// `export { ... }` aggregating the test exports.
   ///
   /// # Returns
   /// A complete ES module string ready to register under "zero/test".
   pub fn test_module() -> String {
       let mut s = String::from(ZERO_RUNTIME_BODY);
       if !s.ends_with('\n') { s.push('\n'); }
       s.push_str(ZERO_TEST_BODY);
       if !s.ends_with('\n') { s.push('\n'); }
       s.push_str("export { ");
       s.push_str(&ZERO_TEST_EXPORTS.join(", "));
       s.push_str(" };\n");
       s
   }
   ```

   **Decision — why concat into one module instead of two:** the
   alternative is to keep "zero" and "zero/test" as two separate
   modules where "zero/test" imports from "zero". That works too, but
   it doubles the runtime body's execution per test file (once per
   module). Concatenating into one keeps the body single-eval'd, since
   in Boa each test file gets a fresh context anyway. The user-facing
   shape (importing `signal` from `"zero"` and `describe` from
   `"zero/test"`) is unaffected because both modules export the
   appropriate subset.

   *Trade-off:* `"zero"` (served at `/zero.js` in dev/build) and the
   `"zero"` half of `"zero/test"` ship the runtime body twice in
   different contexts, but they never coexist in the same JS context
   (browser sees only `/zero.js`; Boa sees only `test_module()`).

3. **Tests in `src/runtime.rs`:**
   - Existing tests for `runtime_module()` keep passing — extend the
     export-block test to confirm the three new internal names appear.
   - New: `test_module_contains_describe_and_expect()`.
   - New: `test_module_ends_with_aggregate_export_block_for_test_exports()`.
   - New: `ZERO_DOM_SHIM_BODY` contains `globalThis.document = document`
     (the install side-effect) and `function createElement(`.

---

### Step 3: Add Boa, write the `src/test_runner/` skeleton (loader + harness)

**Goal:** Bring the Boa engine into the project and stand up the Rust
pieces that own the Boa context: a custom module loader resolving
`"zero"`, `"zero/test"`, and relative paths; a harness that boots the
DOM shim, imports the user test file, and walks the resulting test
tree, calling each hook and `it` body. No CLI wiring yet — this step
exposes a `run_file(path) -> FileResult` function the next step glues
to discovery.

**Files:**
- `Cargo.toml` (modify)
- `src/lib.rs` (modify — add `pub mod test_runner;`)
- `src/test_runner/mod.rs` (new)
- `src/test_runner/loader.rs` (new)
- `src/test_runner/harness.rs` (new)
- `src/test_runner/result.rs` (new — small struct types shared with
  reporter/CLI)

**Changes:**

1. **`Cargo.toml`:**

   ```toml
   boa_engine = { version = "0.20", features = ["annex-b"] }
   ```

   (Version pin verified at implementation time; the latest released
   `boa_engine` should be selected. `annex-b` enables a handful of legacy
   JS conveniences and is cheap.) Boa's module support lives in the
   `boa_engine::module` namespace and the `JsObject`/`Module`/`ModuleLoader`
   trait are public; no sibling crates are required for ES-module
   loading in current Boa. If a future minor version splits the loader
   into a separate crate, add it here.

2. **`src/test_runner/result.rs`:**

   ```rust
   #[derive(Debug)]
   pub struct TestOutcome {
       pub name_chain: Vec<String>,  // ["Home", "renders the initial count"]
       pub status: Status,
       pub duration_ms: u128,
       pub failure: Option<Failure>,
   }

   #[derive(Debug)]
   pub enum Status { Passed, Failed, Skipped(String) }

   #[derive(Debug)]
   pub struct Failure {
       pub message: String,
       pub stack: Option<String>,
   }

   #[derive(Debug)]
   pub struct FileResult {
       pub path: PathBuf,             // path relative to project root
       pub outcomes: Vec<TestOutcome>,
       pub load_error: Option<Failure>, // top-level/syntax error on the file
   }
   ```

3. **`src/test_runner/loader.rs`:** a struct implementing Boa's
   `ModuleLoader` (or, if Boa's current trait is async-only, a small
   wrapper that fulfills the contract synchronously since all our
   sources are in-memory or local-disk).

   - Owns: project root `PathBuf`, the precomputed
     `runtime_module()` string, the precomputed `test_module()` string.
   - On request for `"zero"`: parse and return a Boa `Module` from the
     runtime string.
   - On request for `"zero/test"`: parse and return a Boa `Module`
     from the test-module string.
   - On request for a relative specifier (`./` or `../`): canonicalize
     against the importer's directory; refuse anything that escapes
     the project root (mirroring `src/build/resolver.rs`); `fs::read`
     the file; parse and return.
   - Any other specifier → return a `JsError` with a clear message.
   - Cache parsed `Module` instances by absolute path / sentinel key so
     repeat imports inside one Boa context resolve to the same module
     identity.

   **Fallback noted (per spec open question):** if Boa's current
   `ModuleLoader` trait shape proves rough (e.g., requires the loader
   to call a continuation while holding an exclusive borrow on the
   `Context`), wrap the loader in `Rc<RefCell<...>>` and implement the
   trait on a thin shim. Only if that still doesn't fly do we drop to
   script-eval-with-transpiled-imports; this is recorded as a Risk
   below, not the default path.

4. **`src/test_runner/harness.rs`:**

   - `pub fn run_file(project_root: &Path, file_abs: &Path) -> FileResult`
   - Steps:
     1. Construct a fresh `boa_engine::Context`. Install the loader.
     2. Evaluate `ZERO_DOM_SHIM_BODY` as a *script* (not a module) so
        its top-level side effects (`globalThis.document = document`
        and `globalThis.window = window`) take hold before any user
        code runs. Wrap any error here as a `Failure` and return
        with `load_error = Some(...)`.
     3. Dynamically import the user file: build a tiny bootstrap module
        that does `await import("file:///<abs>")` (or whatever URL
        scheme Boa's loader uses for filesystem modules), driving the
        Boa event loop until the import resolves. Catch any error,
        record as `load_error`.
     4. Once the user file resolves, look up `"zero/test"`'s
        `__getTestTree__` export and call it to obtain the test tree.
     5. Walk the tree depth-first:
        ```
        async fn walk(desc):
          for hook in desc.beforeAll: await call(hook)         // record-and-skip-rest on throw
          for child in desc.children:
            if child is It:
              chain = ancestor.beforeEach[outermost..innermost]
              for hook in chain: await call(hook)              // skip on throw
              start = now()
              try: await call(child.fn)
              record_outcome(name_chain, status, now() - start)
              for hook in ancestors.afterEach[innermost..outermost]: await call(hook)
            else:
              walk(child)
          for hook in desc.afterAll: await call(hook)
        ```
        Implementation lives in Rust, but `await call(hook)` means
        "invoke the JS function, take the returned `JsValue`, if it's
        a promise call `boa_engine::context::HostHooks` / `run_jobs`
        loop until settled, then check for thrown errors".
     6. Failures inside `beforeAll`/`beforeEach` mark the remaining
        `it`s in the affected describe as
        `Status::Skipped("beforeAll failed: <msg>")` (or
        `beforeEach`). `afterEach`/`afterAll` failures are recorded
        against the most recent test outcome's failure or as a synthetic
        outcome named `"<describe> > afterAll"` if no current test.
     7. Drop the Context. Return `FileResult`.

   - **Source filename for stack traces:** when parsing the user
     file's Module, pass the absolute path as the source name (Boa's
     `Source::from_filepath` or equivalent) so thrown errors include
     it. Mirrors the spec's "preserve source filenames" guidance.

   - **`console.log` pass-through:** install a `globalThis.console`
     object on the context whose `log`/`warn`/`error` methods print to
     Rust's stdout/stderr. Matches the spec's MVP recommendation.

5. **`src/test_runner/mod.rs`:**

   ```rust
   pub mod discovery;
   pub mod harness;
   pub mod loader;
   pub mod reporter;
   pub mod result;

   pub use harness::run_file;
   pub use result::{FileResult, Status, TestOutcome};
   ```

   (`discovery` and `reporter` arrive in the next steps but the module
   tree can be stubbed out here so the layout lands once.)

**Tests:**

- `src/test_runner/loader.rs` unit tests:
  - `resolve("zero", ...)` produces a Module whose default-export-less
    body evaluates to having `signal` in scope (smoke-eval and call).
  - `resolve("zero/test", ...)` produces a Module that exposes
    `__getTestTree__`.
  - `resolve("./foo.js", importer_dir, root)` reads the file when it
    exists; errors when it escapes the root.
  - `resolve("lodash", ...)` errors with "unsupported".

- `src/test_runner/harness.rs` integration-style unit test:
  - Build an in-memory or `tempfile` test file:
    ```js
    import { describe, it, expect } from "zero/test";
    describe("g", () => { it("ok", () => expect(1).toBe(1)); });
    ```
    Call `run_file`; assert `outcomes.len() == 1`, `status == Passed`,
    `name_chain == ["g", "ok"]`.
  - Another file with `expect(1).toBe(2)`; assert `Status::Failed` and
    a message containing both `1` and `2`.
  - A file with a top-level `throw`; assert `load_error.is_some()` and
    `outcomes.is_empty()`.
  - A file with `beforeEach` that throws; assert subsequent `it`s are
    `Status::Skipped` with a reason mentioning `beforeEach`.

---

### Step 4: Implement discovery (`src/test_runner/discovery.rs`)

**Goal:** Resolve the optional `target` CLI argument to a concrete list
of absolute file paths the harness should run, with the same
filtering/sort guarantees the spec calls out. Pure Rust, no Boa
involvement.

**Files:**
- `src/test_runner/discovery.rs` (new)

**Changes:**

```rust
pub struct DiscoveryOpts<'a> {
    pub root: &'a Path,         // project root (config.project.root resolved)
    pub out_dir: &'a Path,      // build.out resolved
    pub target: Option<&'a str>,
}

pub struct DiscoveryResult {
    pub files: Vec<PathBuf>,    // absolute paths, sorted
}

pub fn discover(opts: DiscoveryOpts<'_>) -> anyhow::Result<DiscoveryResult>
```

Algorithm:

1. If `target` is `Some(t)`:
   - Resolve `t` relative to the project root (mirror `bundler.rs`'s
     join + canonicalize approach). If the resolved path exists as a
     regular file, return `vec![that_path]` — discovery is bypassed
     even for non-`.test.js` filenames (spec rule).
   - Otherwise treat `t` as a substring filter (carry through to step 3).

2. Recursively walk `root`. For each entry:
   - Skip directories whose name component starts with `.` (hidden).
   - Skip a directory named `node_modules`.
   - Skip anything whose absolute path starts with `out_dir`.
   - Collect files whose basename matches `*.test.js` or `*.spec.js`.

3. Filter the collected list by the substring filter if present:
   keep candidates whose path **relative to project root, with
   forward-slash separators** contains `t` (case-sensitive).

4. Sort by path (`Vec::sort`). Return.

**Implementation note:** reuse the dev watcher's hidden/out-dir guard
where convenient (`src/dev/watch.rs::is_ignored` already implements
the dotfile + out-dir rule). Adding `node_modules` to that helper is
trivial but currently unused by the dev watcher; introduce a local
guard in `discovery.rs` rather than retrofitting `is_ignored` to add
a third concern.

**Tests** (in-file `#[cfg(test)] mod tests` using `tempfile`):
- Empty tree → empty result.
- Two `.test.js` files + one `.spec.js` + one `.js` → returns three,
  sorted.
- Substring filter `"routes"` matches only files whose relative path
  contains `routes`.
- `target` resolving to an existing non-`.test.js` file returns just
  that file.
- `.hidden/foo.test.js` is skipped.
- `node_modules/bar.test.js` is skipped.
- File under `out_dir` is skipped.
- Result is deterministic (sorted alphabetically).

---

### Step 5: Implement the reporter (`src/test_runner/reporter.rs`)

**Goal:** Turn a stream of `FileResult` into stdout text matching the
spec's contract: per-test pass/fail line, per-failure message + stack,
final summary line. Pure formatting + I/O — kept separate so the CLI
just hands it a writer.

**Files:**
- `src/test_runner/reporter.rs` (new)

**Changes:**

```rust
pub struct ReporterTotals { pub passed: usize, pub failed: usize, pub skipped: usize }

pub struct Reporter<'a, W: Write> {
    writer: &'a mut W,
    totals: ReporterTotals,
    started_at: Instant,
}

impl<'a, W: Write> Reporter<'a, W> {
    pub fn new(writer: &'a mut W) -> Self { ... }
    pub fn record_file(&mut self, file: &FileResult) -> io::Result<()>;
    pub fn finish(self) -> io::Result<ReporterTotals>;
}
```

Per-test line format (one-line, no color in MVP):

```
PASS  Home > renders the initial count   (web/src/routes/home.test.js)
FAIL  Home > increments the count        (web/src/routes/home.test.js)
        Error: expected "Count: 1" to be "Count: 2"
            at home.test.js:21:12
SKIP  Home > flaky one (beforeEach failed: oh no)
```

Final summary:

```
3 passed, 1 failed, 0 skipped in 0.142s
```

A top-of-file `load_error` is emitted as:

```
ERROR loading web/src/routes/broken.test.js
        SyntaxError: Unexpected token ...
```

…and counts toward `failed` (1).

**Tests:**
- Feed a `FileResult` with one passed + one failed outcome to
  `record_file`; capture into a `Vec<u8>` writer; assert the output
  contains `PASS`, `FAIL`, the failure message, and the file path.
- `finish` produces the summary line with the correct counts.
- An empty file (no outcomes, no load error) reports nothing for that
  file and contributes zero to the totals.

---

### Step 6: Wire the `zero test` subcommand into the CLI

**Goal:** Glue together discovery, the harness, and the reporter
behind a single `zero test [target]` invocation.

**Files:**
- `src/cmd/mod.rs` (modify — `pub mod test;`)
- `src/cmd/test.rs` (new)
- `src/main.rs` (modify — add the `Test { target: Option<String> }`
  variant to `Commands` and dispatch).

**Changes:**

1. **`src/main.rs`:** add the variant.

   ```rust
   #[derive(Subcommand)]
   enum Commands {
       Init,
       Dev,
       Build,
       /// Run *.test.js / *.spec.js under the embedded engine
       Test { target: Option<String> },
   }
   ```

   Dispatch arm:

   ```rust
   Commands::Test { target } => cmd::test::run(target).await,
   ```

2. **`src/cmd/test.rs`:**

   ```rust
   pub async fn run(target: Option<String>) -> anyhow::Result<()> {
       let config = Config::load_from_cwd()?;
       let cwd = std::env::current_dir()?;
       let root = cwd.join(&config.project.root);
       let out  = cwd.join(&config.build.out);

       let DiscoveryResult { files } = discover(DiscoveryOpts {
           root: &root, out_dir: &out, target: target.as_deref(),
       })?;

       if files.is_empty() {
           println!("zero test — no test files found");
           return Ok(());
       }

       let mut stdout = std::io::stdout().lock();
       let mut reporter = Reporter::new(&mut stdout);
       for f in &files {
           let result = test_runner::run_file(&root, f);
           reporter.record_file(&result)?;
       }
       let totals = reporter.finish()?;
       if totals.failed > 0 { std::process::exit(1); }
       Ok(())
   }
   ```

   Process-level `exit(1)` keeps the contract simple (consistent with
   the spec). The `anyhow::Result<()>` return remains for the harness
   errors that prevent any run at all (e.g., config missing).

3. **Help text:** the clap-derived `///` doc-comment on the variant
   becomes the `--help` blurb; the spec only asks for parity with the
   existing subcommands.

**Tests:** primarily covered by Step 8's integration tests; this
step's own unit-test coverage is light (clap parsing of `Commands::Test`
in `src/main.rs` is exercised by an end-to-end CLI test).

---

### Step 7: Emit `home.test.js` from `zero init`

**Goal:** Make the scaffold materialize the canonical `home.test.js`
so `zero init && zero test` works out of the box. The file already
exists at `src/scaffold/src/routes/home.test.js` but `scaffold.rs`
never writes it.

**Files:**
- `src/scaffold.rs` (modify)

**Changes:**

```rust
const TPL_HOME_TEST_JS: &str = include_str!("scaffold/src/routes/home.test.js");

// inside write_to(...):
fs::write(
    root_dir.join("src").join("routes").join("home.test.js"),
    TPL_HOME_TEST_JS,
)?;
```

**Tests:** extend `write_to_emits_all_four_files` (now "five files")
in `src/scaffold.rs` to assert `home.test.js` was written and
contains `import { describe, it, expect } from "zero/test";`. The
existing `tests/e2e_init_dev.rs` doesn't need changes — it doesn't
assert on test files — but if we choose to extend its `assert` block
that's a single-line addition.

---

### Step 8: End-to-end integration tests + Boa smoke pass over `runtime/*.test.js`

**Goal:** Lock the behavior at the CLI/process boundary, including the
spec's canonical acceptance target ("`zero test` runs the scaffolded
`home.test.js` green"). Also surface any Boa/spec-coverage gaps by
re-running the existing `runtime/*.test.js` suite under the new
harness; if any of them fail, that's a runtime-rewrite task we'd
rather find before users do.

**Files:**
- `tests/e2e_init_test.rs` (new)
- `tests/test_runner_smoke.rs` (new — Boa smoke pass)

**Changes:**

1. **`tests/e2e_init_test.rs`** — modeled on `tests/e2e_init_dev.rs`:

   - Scaffold a temp project (`zero init`), then run `zero test`
     against it via `assert_cmd::Command`.
   - Assert `.success()` (exit 0) and that stdout contains
     `2 passed, 0 failed` (the two `it`s in the canonical
     `home.test.js`).
   - Mutate `home.test.js` to break one assertion; rerun; assert
     `.failure()` and that stdout contains `FAIL`.
   - Add a `broken.test.js` containing top-level `throw new Error("nope")`;
     rerun; assert `.failure()`, that stdout contains
     `ERROR loading`, AND that the original tests still ran (the
     bad file doesn't abort the run).
   - Add a `target` invocation: `zero test routes/home.test.js` runs
     only the matched file (verify by ensuring the other files are
     absent from output).

2. **`tests/test_runner_smoke.rs`** — Boa smoke pass over the
   framework's own existing tests:

   - Copy `runtime/reactivity.test.js`, `runtime/template.test.js`,
     `runtime/router.test.js`, `runtime/app.test.js` into a temp
     project's `src/` (renaming relative imports as needed: replace
     `from './reactivity.js'` with `from 'zero'` and similar, since
     the existing files import internal modules directly that aren't
     part of the public surface). Or, simpler: scaffold a temp
     project with just one shim file per existing test that re-exports
     the test cases under the public API. (Implementation detail
     deferred to execute-time; the test exists to surface Boa
     incompatibilities before users hit them.)
   - Run `zero test` on the temp project. The pass bar is **soft**:
     this test exists to gate failures during development, but the
     CI for this slice runs it with `#[ignore]` so a single broken
     case doesn't block landing the runner. The execute-time agent
     should run it manually and either fix the runtime to be
     Boa-compatible, file a Boa bug + work around it, or document the
     gap.

**Tests:** Steps 1, 3, 4, 5, 6 already include their own unit tests.
This step is itself the integration-test layer.

---

## Risks and Assumptions

- **Boa's `ModuleLoader` trait shape.** The plan assumes Boa exposes a
  synchronous `ModuleLoader` we can implement against in-memory and
  on-disk sources. If the current release only offers an async loader
  bolted to a `JsFuture`, the loader code grows by a wrapper but the
  shape stays the same. If module support proves unusable, the fallback
  is the spec's documented one: eval each file as a script after
  transpiling `import`/`export` syntax to `__zero_require` calls
  (reusing the bundler's already-built rewriter from `src/build/bundler.rs`).
  This fallback ships nothing the user-facing API can detect — only the
  harness internals change.

- **Boa async/microtask semantics.** Awaiting async `it` bodies relies
  on Boa's job queue progressing each tick. The harness `run_jobs()`
  loop must drain the queue until both the user promise settles AND no
  microtasks are left, otherwise `cleanup()` from `afterEach(cleanup)`
  could race with deferred effects. If Boa lacks a clean
  "run-until-promise-settled" hook, we pump the queue with a small
  helper.

- **Boa spec coverage in real test files.** The Step 8 smoke pass is
  the early-warning. The known-shaky surfaces likely to bite are
  `Proxy` (used by `App._stateProxy`), tagged template literals (used
  pervasively by `html\`...\``), and the `WeakMap` used by
  `_templateCache`. Boa supports all three on paper; the smoke pass
  proves it in practice.

- **Stack-trace fidelity.** Boa's stack format may not match what users
  expect from Node. The plan opts to preserve filenames at parse time
  via `Source::from_filepath`; if Boa's printed stacks are still poor,
  we may need a small post-processor that maps Boa coordinates back to
  files. Not a blocker for this slice — minimum bar is "the file path
  appears in the failure output", which the reporter already prints.

- **Binary size.** Boa is pure-Rust but not small. The plan assumes the
  resulting `zero` binary is within an acceptable size envelope; the
  execute agent should `cargo build --release` before and after and
  record the delta. If the jump is genuinely user-hostile (>30MB), the
  fallback is to compile the test runner behind a feature flag or as a
  separate binary — out-of-scope work that would be a new slice.

- **Internal-export underscore convention.** Step 2 extends
  `ZERO_RUNTIME_EXPORTS` with `_setCurrentApp` / `_createScope` /
  `_getCurrentApp`. The underscore prefix communicates "not public", but
  technically any browser-served `import { _setCurrentApp } from "zero"`
  would now succeed. This is consistent with the runtime's existing
  convention (`_setCurrentApp` was already exported from `app.js`); no
  *new* visibility is introduced for the runtime author's perspective.
  If we later want hard enforcement, the bundler/runtime composer can
  drop underscore-prefixed names from the browser-facing export list
  while keeping them for the in-Boa loader — a one-line follow-up.

- **No per-test timeout.** A hung promise hangs the runner forever
  until Ctrl-C. Spec acknowledges this; the assumption is that real
  test suites don't have indefinite hangs and the Ctrl-C escape hatch
  is adequate for MVP. If this proves painful in practice, a
  `--timeout=Ns` flag is a small follow-up.
