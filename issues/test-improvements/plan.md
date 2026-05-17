# Plan: Test runner improvements (Phase 11)

## Summary

Close the four gaps in `zero test` identified in the spec: failure
location + source snippets, line/function coverage, mutation testing as
`zero mutate`, and dependency-aware watch mode. All work happens inside
the existing Rust binary; no new npm or Rust dependencies. The four
features share three pieces of plumbing — the SWC AST pass, the Boa
module loader, and the file walker — and are ordered so each step lands
in a compilable, test-passing state. The first three steps unblock
failure messages immediately (the highest-value piece). Coverage and
the watcher follow because mutation testing depends on coverage (for
the unreachable-mutant skip) and the watcher depends on the loader's
import-map (added in the coverage step).

## Prerequisites

The spec's "Open Questions" are answered as follows by this plan; if
any answer turns out to be wrong, the affected step replans, not the
whole slice:

- **Boa stack format** — verified in Step 2 via a smoke test before
  the parser is written. If the parser proves fragile, `_userFrame`
  (Step 2) is load-bearing and the harness-side parser becomes
  best-effort fallback. The plan assumes V8-style `at fn (file:L:C)`
  frames.
- **Counter map representation** — object `{ "14": 0 }`. Sparse,
  JSON-natural, plenty fast for this scale.
- **Source-snippet width** — hard-coded at two lines above and two
  lines below the failing line (so five lines total).
- **Mutation memory model** — sequential, one fresh Boa `Context` per
  mutant×test-file. Reuse not attempted in this slice.
- **`.toEqual` deep-diff** — deferred. Out of scope here.
- **Watch-mode SCSS handling** — `.scss` / `.css` changes are
  ignored. Only `.ts` / `.js` triggers a cycle.
- **Watch-mode first-cycle conservatism** — accepted; the map
  self-heals on cycle 2.
- **Mutation operator emit format** — SWC printer per mutant.
- **`zero mutate` progress output** — print `[N/M] killed: …` per
  mutant by default; `--quiet` collapses to summary only.
- **`coverage/` and `mutation/` in `.gitignore`** — Step 9 adds both
  to the scaffold's `.gitignore`.
- **Coverage hit-counts** — counts tracked internally, exposed in
  JSON; terminal table shows binary covered/uncovered.

No external issues block this slice. `issues/test-helpers/`'s spy
matchers continue to work without change (their thrown errors flow
through the same matcher path Step 2 instruments).

## Steps

- [x] **Step 1: Extend `Failure` with `SourceLoc`**
- [x] **Step 2: Capture stack + populate `Failure.location` from harness and matchers**
- [x] **Step 3: Reporter renders `at file:line:col` + source snippet**
- [x] **Step 4: SWC coverage instrumenter (`src/test_runner/coverage.rs`)**
- [x] **Step 5: `zero test --coverage` CLI wiring + JSON + terminal table**
- [x] **Step 6: SWC mutation generator (`src/test_runner/mutate.rs`)**
- [x] **Step 7: `zero mutate` subcommand orchestrator**
- [ ] **Step 8: Cancelled - Dependency-aware watch mode (`zero test --watch`)**
- [x] **Step 9: Scaffold + example `.gitignore` (narrowed: only the `coverage/`/`mutation/` lines; spec-doc amendments dropped because they referenced cancelled Step 8 work)**

---

## Step Details

### Step 1: Extend `Failure` with `SourceLoc`

**Goal:** Establish the data type the next two steps populate and
render, in isolation, so the rest of the codebase compiles unchanged.

**Files:**
- `src/test_runner/result.rs` (modify)
- `src/test_runner/harness.rs` (modify — only at construction sites)

**Changes:**

- In `result.rs`, add a new public struct:

  ```rust
  #[derive(Debug, Clone)]
  pub struct SourceLoc {
      pub file: PathBuf,
      pub line: u32,
      pub column: u32,
  }
  ```

  Add `pub location: Option<SourceLoc>` to `Failure`. Keep `message`
  and `stack` as-is.

- In `harness.rs`, every `Failure { message, stack: None }` literal
  becomes `Failure { message, stack: None, location: None }`. There
  are seven such sites today (build-context error, DOM shim error,
  file-read error, transpile error, parse error, load_error path,
  and `js_err_to_failure` / `js_val_to_failure`). None of these
  surface a location yet; that's Step 2.

- In `reporter.rs` tests' `failed_outcome` helper and `make_file`
  helper, the `Failure` literal also gains `location: None`.

**Tests:**

- Existing `result.rs` has no unit tests; add one constructing a
  `Failure` with a `Some(SourceLoc { ... })` and asserting the
  fields round-trip. This pins the public shape.
- All existing harness and reporter tests continue to compile and
  pass (no semantic change). `cargo test` is the verification.

---

### Step 2: Capture stack + populate `Failure.location` from harness and matchers

**Goal:** Every failed test carries the original `.ts` file, line,
and column of the failing assertion. Two cooperating paths
(matcher-attached `_userFrame`; harness-parsed stack) produce the
same `SourceLoc` so the result is robust to Boa stack-format drift.

**Files:**
- `runtime/test.js` (modify — every matcher in `expect()`)
- `runtime/test.test.js` (modify — assert `_userFrame` attached)
- `src/test_runner/harness.rs` (modify — read `.stack` + `_userFrame`,
  parse, source-map, choose top user frame)

**Changes:**

- `runtime/test.js` gets a new internal helper:

  ```js
  /**
   * Walk a fresh stack and return the first frame outside runtime/*.js.
   * @internal
   * @returns {string|null} formatted as "file:line:column" or null
   */
  function _captureUserFrame() {
    const stack = new Error().stack || "";
    for (const line of stack.split("\n")) {
      // Match "at <anything> (file:L:C)" or "at file:L:C" or "fn@file:L:C"
      const m = line.match(/(?:\(|@| )([^\s()]+):(\d+):(\d+)\)?$/);
      if (!m) continue;
      const path = m[1];
      if (path.includes("/runtime/") || path.endsWith("/test.js")) continue;
      return `${path}:${m[2]}:${m[3]}`;
    }
    return null;
  }
  ```

  Every matcher in `expect(actual)` wraps its `throw new Error(...)`
  so that, just before throwing, it does
  `err._userFrame = _captureUserFrame(); throw err`. The simplest
  spelling is a `_fail(msg)` helper used by every matcher:

  ```js
  function _fail(msg) {
    const err = new Error(msg);
    err._userFrame = _captureUserFrame();
    throw err;
  }
  ```

  All thirteen matcher bodies replace `throw new Error(...)` with
  `_fail(...)`.

- `src/test_runner/harness.rs`:
  - `call_and_drain` returns `Result<(), CapturedError>` instead of
    `Result<(), String>`, where `CapturedError { message: String,
    stack: Option<String>, user_frame: Option<String> }`.
    - The `Ok(val)` Promise-rejected branch also returns
      `CapturedError`; same for the `Err(e)` JsError branch.
    - For `JsError`: convert to a `JsValue` via `JsError::to_opaque`
      and use the same object-reading path as the JsValue branch.
    - For `JsValue` errors: read `.stack` (string) and `._userFrame`
      (string) if present.
  - A new helper `build_failure(captured: CapturedError, sm: Option<&SourceMap>) -> Failure`:
    1. `let message = remap_positions(&captured.message, sm)`.
    2. `let stack = captured.stack.as_deref().map(|s| remap_positions(s, sm))`.
    3. Compute `location`:
       - Prefer `captured.user_frame` (already a `path:L:C` string);
         remap with `remap_positions` then parse to `SourceLoc`.
       - Otherwise, walk the remapped `stack` line by line; for each
         frame, parse `(file, line, col)` with the existing regex
         and skip frames whose `file` contains `runtime/` or is the
         embedded `zero/test` synthetic source.
       - First surviving frame becomes the `SourceLoc`.
       - Test files (`.test.ts`, `.spec.ts`) are *not* filtered out —
         a failing assertion in a test file is exactly where we want
         the caret.
    4. Return `Failure { message, stack, location }`.
  - `walk_describe` calls `build_failure(captured, sm)` instead of
    building the `Failure` inline.
  - `js_err_to_failure` and `js_val_to_failure` are reworked to go
    through the same `CapturedError`-then-`build_failure` path so
    load errors also get `location` when a frame is present.

- Confirm Boa's stack format in one of two ways before writing the
  parser: a short throwaway test that prints `e.stack`, or a
  read-only browse through Boa's `JsError`/`ErrorObject` source.
  The regex above accommodates V8 (`at fn (path:L:C)`),
  SpiderMonkey (`fn@path:L:C`), and plain `path:L:C` lines.

**Tests:**

- `runtime/test.test.js`: new node:test case — call `expect(1).toBe(2)`
  inside a try/catch, assert that the caught error has a `_userFrame`
  matching the file the test runs in.
- `src/test_runner/harness.rs` tests:
  - `failing_assertion_carries_location` — re-running the existing
    `failing_assertion_produces_failed_outcome` fixture, assert
    `outcomes[0].failure.location.is_some()` and the file ends with
    the temp filename.
  - `load_error_carries_location_when_throwable_has_stack` — throw a
    constructed `Error` at top level, assert load_error has a
    location.
  - `location_skips_runtime_frames` — fixture that calls
    `expect(...).toBe(...)` from a function declared in the test
    file; assert location points at the call site, not the matcher.

---

### Step 3: Reporter renders `at file:line:col` + source snippet

**Goal:** Make the structured location visible. After this step, a
failing test produces the exact rendering in spec §1.3.

**Files:**
- `src/test_runner/reporter.rs` (modify)
- `src/test_runner/reporter.rs` tests (extend)

**Changes:**

- New private helper functions in `reporter.rs`, each kept under 80
  lines:
  - `write_location(w, loc, project_root) -> io::Result<()>` —
    prints `        at <relpath>:<line>:<col>` (or absolute path if
    not under project root).
  - `write_snippet(w, loc) -> io::Result<()>` — reads `loc.file`,
    splits into lines, picks `loc.line ± 2` (clamped to file
    bounds), right-aligns the line numbers, prefixes the failing
    line with `> `, draws a caret line consisting of spaces to
    column `loc.column` followed by `^`. Silently returns `Ok(())`
    if the file cannot be read.
- `Reporter` gains a project-root field set in `new_with_root`; the
  existing `new(writer)` keeps backward compatibility and stores
  `PathBuf::new()` (renders absolute paths). The CLI passes the
  actual root via `new_with_root`.
- In `record_file`, the `Status::Failed` arm prints, in this order:
  1. `FAIL  …` header (unchanged).
  2. `        <message>` (unchanged — already source-mapped).
  3. `write_location(...)` if `failure.location.is_some()`.
  4. Blank line then `write_snippet(...)` if location is some.
- The `ERROR loading …` arm gets the same `write_location` +
  `write_snippet` calls when a load-error location is available.

**Tests:**

- `reporter_renders_at_line_when_location_present` — synthesize a
  `Failure` with a `SourceLoc` pointing at a tempfile, assert
  output contains `at `, the relative path, `:14:7`, and the
  expected snippet lines.
- `reporter_omits_snippet_when_source_missing` — `SourceLoc.file`
  points at a non-existent path, assert the `at` line still prints
  and no panic/IO error reaches the test.
- `reporter_clamps_snippet_at_file_bounds` — failing line is the
  first line of the file; snippet renders only line 1 and 2.
- `reporter_renders_caret_at_correct_column` — column = 7, assert
  the caret line has exactly 6 spaces (the gutter) padding then `^`.
- Existing reporter tests continue to pass; only new lines are
  added to their expected output.

---

### Step 4: SWC coverage instrumenter (`src/test_runner/coverage.rs`)

**Goal:** A self-contained transformer that takes original TS/JS
source and emits instrumented JS plus a per-file counter universe.
This is the AST-pass scaffold both Step 5 and Step 6 use.

**Files:**
- `src/test_runner/coverage.rs` (new)
- `src/test_runner/mod.rs` (add `pub mod coverage;`)
- `src/test_runner/coverage.rs` tests (new, in-module)

**Changes:**

- New module exposes:

  ```rust
  pub struct CoverageMap {
      pub file: PathBuf,            // absolute, normalized
      pub lines: Vec<u32>,          // sorted, unique
      pub fns: Vec<String>,         // function identifiers, in source order
  }

  pub struct InstrumentOutput {
      pub code: String,
      pub source_map: Option<String>,
      pub map: CoverageMap,
  }

  pub fn instrument(
      source: &str,
      opts: &crate::transpile::TranspileOptions<'_>,
  ) -> Result<InstrumentOutput, crate::transpile::TranspileError>;
  ```

- Implementation reuses the existing `transpile_typescript` pipeline:
  parse → resolver → strip → instrumenter visitor → hygiene → fixer
  → emit. The new visitor is an
  `swc_core::ecma::visit::VisitMut` that:
  - On entering a function (declaration, expression, arrow,
    method, constructor) records the function's identifier (or a
    synthetic `anon@line` for anonymous functions) and prepends a
    `__c.fns["name"]++;` statement to the body.
  - On each top-level `Stmt` and the first statement of each
    `BlockStmt`, prepends `__c.lines[N]++;` where `N` is the
    source-position line lookup via SWC's `SourceMap`.
  - Maintains a `lines: BTreeSet<u32>` and `fns: Vec<String>`
    populated as it walks.
- After traversal, the visitor prepends a prologue Stmt:

  ```js
  const __c = (globalThis.__zero_coverage__ ||= {})["/abs/path/src/foo.ts"] ||=
    { lines: { /* all known lines zero-initialized */ },
      fns:   { /* all known fns zero-initialized */ } };
  ```

  The line/fn key set is the universe captured during the walk so
  the report knows total counts before any execution.
- The source map produced by the emitter still maps generated
  positions back to original `.ts` positions; the inserted counter
  statements get the line of the statement they precede so
  source-map lookups remain valid for Step 1's location attribution.

**Tests:**

- `instruments_top_level_statement_increments_line_counter` — input
  `export const x = 1;`, run through Boa with a stub
  `globalThis.__zero_coverage__`, assert `__zero_coverage__[path].lines[1] === 1`.
- `instruments_function_prologue` — input
  `export function f(){ return 1 }`, call `f()`, assert
  `__zero_coverage__[path].fns.f === 1` and `lines` contains the
  function body line.
- `instruments_arrow_function` — same shape, anonymous arrow gets
  the `anon@<line>` key.
- `coverage_map_contains_all_known_lines_and_fns_zero_initialized` —
  after instrumenting (without executing), inspect the prologue
  and assert every executable line and every function appears with
  initial value 0.
- `preserves_source_map_back_to_original_ts` — instrument
  `const x: number = 1`, lookup the generated position of `x = 1`
  back through the returned source map, assert it lands on
  original line 1.
- `is_idempotent_within_one_module` — running `instrument` twice on
  the same source produces equal `CoverageMap.lines` (no double-
  counting from prologue insertion).

---

### Step 5: `zero test --coverage` CLI wiring + JSON + terminal table

**Goal:** Surface coverage end-to-end. `zero test --coverage` runs
the suite with instrumentation applied to `src/`, then prints a
table and writes `coverage/coverage.json`.

**Files:**
- `src/main.rs` (modify — add `coverage: bool` flag)
- `src/cmd/test.rs` (modify — accept and thread the flag)
- `src/test_runner/loader.rs` (modify — optional instrumenter)
- `src/test_runner/harness.rs` (modify — optional instrumenter +
  post-run coverage read)
- `src/test_runner/coverage.rs` (extend — `CoverageScope`,
  `Aggregator`, report writers)
- `src/test_runner/mod.rs` (re-export the new types)
- `src/test_runner/coverage.rs` tests + a CLI integration test

**Changes:**

- New types in `coverage.rs`:

  ```rust
  pub struct CoverageScope {
      pub project_root: PathBuf,
      pub src_dir: PathBuf,   // <project_root>/src
      pub out_dir: PathBuf,   // skip files under this
  }

  impl CoverageScope {
      pub fn covers(&self, file: &Path) -> bool { /* see rules */ }
  }

  pub struct CoverageAggregator { /* per-file maps + per-file hit counts */ }

  impl CoverageAggregator {
      pub fn new() -> Self;
      pub fn register(&mut self, map: CoverageMap);
      pub fn ingest_run(&mut self, run: &serde_json::Value); // reads __zero_coverage__
      pub fn write_terminal(&self, w: &mut impl Write) -> io::Result<()>;
      pub fn write_json(&self, root: &Path) -> io::Result<()>; // coverage/coverage.json
  }
  ```

  `CoverageScope::covers` returns true iff the file is under
  `src_dir`, has a `.ts` or `.js` extension, is **not** a test
  file (`.test.{ts,js}` or `.spec.{ts,js}`), and is **not** under
  `out_dir` or any `.zero/` / `node_modules/` segment. The rules
  mirror §2.2 of the spec.

- `ZeroModuleLoader` gains:

  ```rust
  pub struct ZeroModuleLoader {
      // existing fields …
      coverage: Option<Rc<RefCell<CoverageContext>>>,
  }

  pub struct CoverageContext {
      pub scope: CoverageScope,
      pub maps: Vec<CoverageMap>,
  }

  impl ZeroModuleLoader {
      pub fn new_with_coverage(root: &Path, ctx: Rc<RefCell<CoverageContext>>) -> Self;
      pub fn drain_coverage_maps(&self) -> Vec<CoverageMap>;
  }
  ```

  In `resolve_relative`, when `coverage` is `Some` and
  `scope.covers(&canonical)` is true, call
  `coverage::instrument` instead of `transpile_typescript`. The
  resulting `CoverageMap` is appended to `ctx.borrow_mut().maps`.

- `harness::run_file` gains an overload (or a new entry-point
  function `run_file_with_options`) that accepts an
  `Option<Rc<RefCell<CoverageContext>>>` and, after the test loop,
  reads `globalThis.__zero_coverage__` from the Boa context and
  returns it alongside `FileResult` as a new
  `pub struct RunOutcome { result: FileResult, coverage: Option<serde_json::Value> }`.
  The original `run_file` stays as `pub fn run_file(...) ->
  FileResult` for back-compat with existing tests; internally it
  calls the new function with `None`.

- `src/main.rs`: add `#[arg(long, default_value_t = false)] coverage: bool`
  to the `Test` subcommand. Pass through to `cmd::test::run`.

- `src/cmd/test.rs`: when `coverage` is true,
  - Build the `CoverageScope`.
  - Build a shared `CoverageAggregator`.
  - For each test file, build a fresh `CoverageContext` (so the
    loader knows the scope), run the file via
    `run_file_with_options`, register the loader's drained maps
    with the aggregator, and `ingest_run` the file's
    `__zero_coverage__` snapshot.
  - After all files, call `aggregator.write_terminal(&mut stdout)`
    and `aggregator.write_json(&project_root)`.
  - Exit code: unchanged (only test failures cause non-zero).

- `coverage/coverage.json` format matches spec §2.5. The directory
  is created (`fs::create_dir_all`) before write. Paths in JSON are
  forward-slash-relative to project root, sorted alphabetically.

**Tests:**

- `coverage.rs`: `scope_covers_src_ts_and_js`,
  `scope_excludes_test_files`, `scope_excludes_dot_zero`,
  `scope_excludes_out_dir`, `scope_excludes_node_modules`.
- `coverage.rs`: `aggregator_terminal_table_sorted_by_pct_ascending`,
  `aggregator_json_paths_are_project_relative`,
  `aggregator_totals_sum_correctly`.
- `harness`: `run_file_with_options_returns_coverage_snapshot` —
  instrument a temp `src/foo.ts` with a function, import it from a
  test file, run with options, assert the returned coverage shows
  the function as hit.
- New integration test under `tests/` (or a `#[test]` in
  `cmd::test`) that wires the full path: tempdir with `zero.toml`,
  a `src/foo.ts`, a `tests/foo.test.ts`, run
  `cmd::test::run(None, /*coverage*/ true)`, assert
  `coverage/coverage.json` exists and contains the expected paths
  and counts.

---

### Step 6: SWC mutation generator (`src/test_runner/mutate.rs`)

**Goal:** A pure mutation-enumeration pass. Given a source file,
produces a list of mutants. No test execution yet — that's Step 7.

**Files:**
- `src/test_runner/mutate.rs` (new)
- `src/test_runner/mod.rs` (add `pub mod mutate;`)
- `src/test_runner/mutate.rs` tests (in-module)

**Changes:**

- Public types:

  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum Operator { Arith, Cmp, Bool, CondNeg, Boundary, LitBool, LitNum, LitStr }

  impl Operator {
      pub fn id(self) -> &'static str; // "arith", "cmp", ...
      pub fn parse(id: &str) -> Option<Self>;
      pub const ALL: &'static [Operator] = &[ /* eight */ ];
  }

  #[derive(Debug, Clone)]
  pub struct MutationSite {
      pub file: PathBuf,
      pub operator: Operator,
      pub line: u32,
      pub column: u32,
      pub original: String,    // short source slice
      pub replacement: String,
  }

  pub struct GenerateOptions<'a> {
      pub operators: &'a [Operator],
      pub max_mutants: Option<usize>,
      /// Lines hit during the coverage baseline. Sites on uncovered
      /// lines are skipped and counted as "unreachable" by the caller.
      pub covered_lines: Option<&'a HashSet<u32>>,
  }

  pub fn generate(
      source: &str,
      file: &Path,
      opts: &GenerateOptions<'_>,
  ) -> Result<(Vec<MutationSite>, /* skipped_unreachable */ usize), TranspileError>;

  pub fn apply(
      source: &str,
      file: &Path,
      site: &MutationSite,
  ) -> Result<String, TranspileError>; // emit mutated JS via SWC printer
  ```

- One AST visitor implements both enumeration and application.
  Enumeration runs the visitor in "collect" mode, capturing every
  potential mutation site without modifying the AST. Application
  runs the visitor in "apply mode N" — keep a counter; only mutate
  the Nth site of matching operator.
- Operators (per spec §3.3):
  - **arith** — `BinExpr` with op `+ - * / %`. `+` → `-`, `-` → `+`,
    `*` → `/`, `/` → `*`, `%` → `*`. Skip `+` when both operands
    are strings (string concat — would change semantics in ways
    that are typically caught and uninteresting).
  - **cmp** — `BinExpr` with op `< <= > >= == != === !==`. Swap:
    `<` → `>=`, `<=` → `>`, `>` → `<=`, `>=` → `<`, `==` → `!=`,
    `!=` → `==`, `===` → `!==`, `!==` → `===`.
  - **bool** — `BinExpr` `&&` / `||` → the other.
  - **cond_neg** — wrap the test of `IfStmt`, `CondExpr`, `WhileStmt`,
    `DoWhileStmt`, and `ForStmt` (when test is `Some`) in a
    `UnaryExpr { op: "!", arg }`.
  - **boundary** — same node match as **cmp**'s `< > <= >=` arm,
    but only the four boundary swaps: `<` → `<=`, `<=` → `<`,
    `>` → `>=`, `>=` → `>`. Yes, this overlaps with **cmp**; they
    are reported as distinct mutants (matching most mutation-testing
    tools).
  - **lit_bool** — `Lit::Bool(true)` → `false`, and vice versa.
  - **lit_num** — `Lit::Num(0)` → `1`, `Lit::Num(1)` → `0`. (Other
    numerics ignored to keep the mutant count down.)
  - **lit_str** — `Lit::Str("")` → `"zero"`, non-empty string
    literal → `""`.
- `original` / `replacement` are short textual renderings emitted
  via SWC's printer with a 40-char cap. Used only for reports.
- `apply` re-parses, walks to the Nth site, mutates that node only,
  prints. Returns the JS code (already type-stripped because we
  walk the post-`strip` AST). It does *not* return a source map;
  the harness consumes the mutated JS directly without source-map
  back-attribution (mutation failures are reported by their
  pre-recorded `(line, column)`).

**Tests:**

- One test per operator family. Each writes a tiny fixture, calls
  `generate(...)`, asserts the expected set of `MutationSite`s,
  then for the first site calls `apply(...)` and asserts the
  resulting JS contains the mutated text and not the original.
- `respects_operator_filter` — `opts.operators = &[Operator::Arith]`,
  fixture mixes arith and cmp ops, only arith mutants are returned.
- `respects_max_mutants` — fixture has 10 candidate sites,
  `max_mutants = Some(3)`, returns 3.
- `skips_sites_on_uncovered_lines` — `covered_lines = Some(&{1})`,
  fixture has sites on lines 1 and 3, returns only the line-1 site
  and reports `skipped_unreachable = 1`.
- `string_plus_is_not_mutated_as_arith` — fixture `"a" + "b"`, no
  mutant generated for the `+`.
- `apply_emits_valid_js` — pick one site per operator, apply it,
  parse the result with `transpile_typescript`, assert it parses.

---

### Step 7: `zero mutate` subcommand orchestrator

**Goal:** End-to-end mutation run. Wire `zero mutate` to discovery,
baseline, coverage, generation, per-mutant execution, and report.

**Files:**
- `src/main.rs` (modify — add `Mutate` subcommand)
- `src/cmd/mod.rs` (modify — `pub mod mutate;`)
- `src/cmd/mutate.rs` (new)
- `src/test_runner/loader.rs` (modify — per-file overlay)
- `src/test_runner/harness.rs` (modify — accept loader overlay)
- `src/cmd/mutate.rs` tests + a small CLI integration test

**Changes:**

- `src/main.rs`: new variant:

  ```rust
  /// Run mutation testing
  Mutate {
      target: Option<String>,
      #[arg(long)] operators: Option<String>,
      #[arg(long)] max_mutants: Option<usize>,
      #[arg(long, short = 'q', default_value_t = false)] quiet: bool,
  },
  ```

- `src/cmd/mutate.rs::run(target, operators, max_mutants, quiet)`:
  1. Load config, compute root + out_dir.
  2. Run discovery (same as `zero test`); abort if no tests.
  3. **Baseline run.** Re-use `cmd::test`'s loop but with
     coverage instrumentation on. If any test fails, print
     `zero mutate: baseline test run failed; refusing to mutate`
     and exit non-zero. Save the per-file `covered_lines` set.
  4. **Generate mutants.** Walk `src/` (using the same scope
     rules as coverage), parse each `.ts` / `.js` file with
     `mutate::generate`, filter by `covered_lines`, cap by
     `max_mutants`. Build a flat `Vec<MutationSite>`.
  5. **Equivalence skip.** Before queueing a mutant, run
     `mutate::apply` and compare byte-for-byte with the
     unmutated baseline JS (already cached from the baseline run);
     drop mutants whose output is identical.
  6. **Per-mutant loop.** For each site, in source order:
     - Build a fresh `ZeroModuleLoader` with an overlay
       `HashMap<PathBuf, String>` that maps the mutated file's
       canonical path to its mutated JS body.
     - Run discovery again (cheap) and run each test file with
       this loader. **Stop as soon as one test fails** ⇒ status
       `Killed`. If a Boa load/transpile error occurs while
       running ⇒ status `Errored`. If all tests pass ⇒ status
       `Survived`.
     - Print `[N/M] <status>: <file>:<line>:<col> <op>` unless
       `--quiet`.
  7. **Report.** Print the summary block per spec §3.5 (counts,
     percentages, survived list). Write `mutation/mutation.json`
     (create dir, format per §3.5).
  8. Exit code: `0` iff `survived == 0 && errored == 0`. (Spec
     §3.5: "non-zero iff any mutant survived." Erroring mutants
     also fail the run since they indicate generated invalid JS
     escaped the equivalence skip.)

- `ZeroModuleLoader` gains:

  ```rust
  pub fn with_overlay(self, overlay: HashMap<PathBuf, String>) -> Self;
  ```

  In `resolve_relative`, before transpiling, check if `canonical`
  is in the overlay; if so, use the overlay's already-transpiled
  JS instead of re-running SWC. Cache key includes a "generation"
  counter set at loader construction so different mutants don't
  hit the same module cache from a previous loader (each mutant
  loop iteration builds a fresh loader anyway, so this is mostly
  defensive).

- `harness::run_file_with_options` (added in Step 5) gains an
  optional `loader: Option<Rc<ZeroModuleLoader>>` so the mutate
  command can pass in the prebuilt loader. The existing `run_file`
  still constructs its own.

**Tests:**

- `cmd::mutate` tests:
  - `baseline_failure_aborts_run` — tempdir whose baseline fails,
    `run()` returns the abort error.
  - `survived_mutant_produces_non_zero_exit` — handwritten src +
    test where one arith mutant survives; `run()` returns
    non-zero (assert via a returned summary or a refactored
    `run_inner` that returns `Result<MutationSummary>`).
  - `killed_mutant_summary_correct` — same fixture with strong
    tests that kill every mutant; summary reports `survived: 0`.
  - `respects_operator_filter` — pass `Some("arith".into())` and
    assert only arith mutants are exercised.
- One end-to-end smoke test that drives the whole pipeline on a
  tempdir to catch wiring regressions.

---

### Step 8: Dependency-aware watch mode (`zero test --watch`)

**Goal:** `zero test --watch` re-runs only the test files whose
transitive imports include the changed path. Cooperates with
`--coverage`.

**Files:**
- `src/main.rs` (modify — `watch: bool` flag)
- `src/cmd/test.rs` (modify — split run into `run_once` +
  `run_watch_loop`)
- `src/test_runner/loader.rs` (modify — record imports per
  resolution)
- `src/test_runner/harness.rs` (modify — surface per-file imports
  in the run outcome)
- `src/test_runner/watch.rs` (new — affected-set + watcher loop)
- `src/test_runner/mod.rs` (`pub mod watch;`)

**Changes:**

- `src/main.rs`: `#[arg(long, short = 'w', default_value_t = false)] watch: bool`
  on `Test`, threaded into `cmd::test::run(target, coverage, watch)`.

- `ZeroModuleLoader` gains:

  ```rust
  /// Every resolved relative import is recorded here keyed by the
  /// referrer's canonical path.
  imports: RefCell<HashMap<PathBuf, HashSet<PathBuf>>>,
  ```

  Populated inside `resolve_relative` immediately after canonicalizing
  the resolved path — record (referrer_canonical → canonical_target).
  A new `pub fn drain_imports(&self) -> HashMap<PathBuf, HashSet<PathBuf>>`
  hands them off to the harness/watcher.

- `harness::RunOutcome` (from Step 5) gains
  `pub imports: HashMap<PathBuf, HashSet<PathBuf>>` populated from
  `loader.drain_imports()` after each file.

- `src/test_runner/watch.rs`:
  - `pub struct ImportGraph { /* edges: HashMap<PathBuf, HashSet<PathBuf>> */ }`
    with `add(referrer, target)`, `affected(changed) ->
    HashSet<PathBuf>` (reverse-walk: collect every key whose value
    set transitively contains `changed`).
  - `pub struct TestImportIndex { test_to_targets: HashMap<PathBuf, HashSet<PathBuf>> }`
    populated from `RunOutcome.imports` flattened per test entry.
    `affected_tests(changed_paths) -> HashSet<PathBuf>` returns the
    test files transitively depending on any changed path. If a
    test file has no recorded imports yet, it is treated as affected
    (conservative first-run fallback).
  - `pub fn run_watch(...) -> anyhow::Result<()>`:
    1. Set up a `notify_debouncer_mini` watcher rooted at the
       project root, 100ms debounce, mirroring `src/dev/watch.rs`.
    2. Channel `(tx, rx): mpsc::channel::<HashSet<PathBuf>>` for
       coalesced change sets.
    3. Spawn a watcher thread that filters events using
       `dev::watch::is_ignored` (already exists, takes
       `path, root, out_dir`) and additionally drops files whose
       extension is not `.ts` / `.js` (no SCSS/CSS triggers).
       `coverage/` and `mutation/` segments are also dropped (they
       would otherwise self-trigger on each run).
    4. Spawn a stdin reader thread that pushes `Enter` and `q`
       commands onto a separate channel.
    5. Run an initial cycle (always all discovered tests).
    6. Loop: `select!`-style polling — block on changes, recompute
       affected set, clear the terminal (`print!("\x1b[2J\x1b[H")`),
       run those tests via the existing reporter, refresh the index
       with the new `RunOutcome.imports`, print
       `> press Enter to re-run, q to quit`, loop.
- `cmd/test.rs`:
  - Refactor `run` into `run_once(target, coverage) -> RunSummary`
    and the public `run(target, coverage, watch)` which calls
    `run_once` once and then either exits or hands off to
    `watch::run_watch`.
  - `RunSummary { totals, index }` carries the import index so
    watch mode starts cycle 2 with knowledge from cycle 1.

**Tests:**

- `watch.rs`:
  - `affected_tests_returns_direct_importer` — index records
    `test_a.ts → src/foo.ts`; changing `src/foo.ts` returns
    `{ test_a.ts }`.
  - `affected_tests_transitive` — `test_a.ts → src/foo.ts →
    src/bar.ts`; changing `src/bar.ts` returns `{ test_a.ts }`.
  - `affected_tests_change_to_test_file_returns_itself` — changing
    `test_b.ts` (in the index) returns `{ test_b.ts }`.
  - `affected_tests_treats_unindexed_tests_as_affected` — index
    knows `test_a.ts` only; discovery returns
    `{test_a.ts, test_b.ts}`; `affected_tests` over discovery
    returns both (test_b.ts conservatively included).
  - `filter_drops_scss_and_css_paths`.
  - `filter_drops_coverage_and_mutation_segments`.
- `loader`: `records_imports_during_resolve` — set up a tempdir
  graph, drive a load, assert `drain_imports` returns the expected
  edges.
- No end-to-end test for the interactive loop (stdin/notify are
  difficult to drive reliably); the unit tests above plus manual
  smoke-test in the PR description are sufficient.

---

### Step 9: Spec amendments + scaffold `.gitignore`

**Goal:** Bring written spec into agreement with the new
implementation. Pure documentation + scaffold-file edits.

**Files:**
- `zero-framework-spec.md` (edit §1, §8, §11, §12)
- `issues/test-runner/spec.md` (move watch + coverage out of
  deferred work)
- `src/scaffold/.gitignore` (modify — add `coverage/`, `mutation/`).
  If the scaffold has no `.gitignore` yet, add one mirroring
  whatever the scaffold currently produces in `dist/`.

**Changes:**

- `zero-framework-spec.md` §1 — confirm the `zero test --watch` /
  `--coverage` bullets describe this slice's behavior; add a
  `zero mutate` row to the subcommand list.
- `zero-framework-spec.md` §8 — replace the "E2E Tests" framing
  that positions zero as unit/integration only with a paragraph
  that names watch mode, coverage, and mutation testing as
  in-runner capabilities. Keep the existing E2E paragraph (or
  follow whatever guidance currently lives there).
- `zero-framework-spec.md` §11 — add `mutate` row to the CLI
  command surface table; cross-reference `issues/test-improvements/`.
- `zero-framework-spec.md` §12 — flip `[ ]` to `[x]` for the
  three Phase 11 items (`test coverage`, `mutation testing`,
  `watch mode`) and add a clarifying note that mutation testing
  ships as `zero mutate`, not `zero test --mutate`.
- `issues/test-runner/spec.md` — remove "Watch mode" and
  "Coverage reporting" from the deferred-work list; add a single
  line under "delivered later" pointing at
  `issues/test-improvements/`.
- Scaffold `.gitignore` — add `coverage/` and `mutation/` lines.

**Tests:**

- Add to the existing scaffold tests (search for
  `src/scaffold.rs` tests that check generated files) an
  assertion that the generated `.gitignore` contains
  `coverage/` and `mutation/`.
- No other automated tests for spec text; reviewer eyeball pass.

---

## Risks and Assumptions

- **Boa stack format is unknown until Step 2 starts.** Step 2's
  regex covers V8, SpiderMonkey, and plain forms, so any reasonable
  format works. If Boa produces something exotic (e.g., positions
  only, no file paths), `_userFrame` becomes the sole source of
  location attribution. Mitigation: the matcher-side capture in
  `runtime/test.js` is the more reliable path by design; the
  harness-side parser is a fallback for non-matcher throws.
- **Coverage instrumenter source-map preservation.** If the new
  visitor disrupts SWC's source map (inserted counter statements
  with synthetic spans), Step 1's location attribution silently
  becomes wrong under `--coverage`. Mitigation: the visitor reuses
  the wrapped statement's `Span` rather than `DUMMY_SP` for inserted
  counters, and Step 4 includes a test that walks the source map
  back to original positions.
- **Mutation testing runtime cost.** A 142-mutant × 30-file run
  with a fresh Boa `Context` per file = thousands of context
  builds. If the per-context overhead is large in practice, the
  whole `zero mutate` UX degrades. Mitigation: print
  `[N/M] killed: …` per mutant so progress is visible; if it
  proves intolerable in real use, a future slice can pool
  `Context`s. Out of scope here.
- **Watch mode platform variability.** `notify` behavior differs
  across macOS/Linux/Windows. The dev server already accepts these
  quirks; the watcher reuses the same library and ignore rules.
- **Stdin handling in watch mode.** A line-buffered read on stdin
  blocks; the implementation uses a dedicated thread so the watcher
  thread is not stalled. If stdin is not a TTY (CI, piped input),
  the `Enter`/`q` controls become inert but Ctrl+C still works.
- **Module cache poisoning across mutants.** Each per-mutant test
  run builds a fresh `ZeroModuleLoader`, so cache entries cannot
  leak. The "overlay generation" counter in Step 7 is defensive,
  not strictly required.
- **Scope of `src/` is project-conventional.** The plan hardcodes
  `<project_root>/src` as the coverage and mutation scope. If a
  user's project puts source elsewhere, both features will report
  nothing. Mitigation: that convention is already enforced by the
  scaffold and framework spec; if it ever changes, a single
  `CoverageScope::new()` change accommodates it.
