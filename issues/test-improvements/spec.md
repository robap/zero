# Spec: Test runner improvements (Phase 11)

## Problem Statement

The Boa-backed test runner shipped with Phases 5 and 7 is functional but
threadbare. Four gaps consistently push test authors and reviewers into
worse workflows than they should have to accept:

1. **Failure messages have no location.** A failing `expect(1).toBe(2)`
   prints the matcher message and nothing else. There is no file, no
   line number, no caret, no source snippet — even though `harness.rs`
   already source-maps `.ts:LINE:COL` positions back through SWC's
   source map. Developers have to grep their own code by error string
   to find the failing assertion. AI agents reviewing test output have
   to guess.
2. **No coverage data.** `--coverage` is listed in the framework spec
   (§1, §8) but unimplemented. A reviewer cannot ask "what percentage
   of `src/` is exercised by the test suite," and an agent asked to
   "find untested code" has to read every file by hand.
3. **No mutation testing.** Coverage alone tells you which lines run,
   not whether tests actually assert anything meaningful about them. A
   `100%`-covered file whose tests only call functions and never check
   results is indistinguishable from a thoroughly-tested one. Mutation
   testing closes the loop by introducing small code changes and
   verifying that some test catches each one.
4. **No watch mode.** `--watch` is listed in the framework spec but
   unimplemented. Every change requires a manual re-invocation of
   `zero test`. The dev-server watcher infrastructure
   (`src/dev/watch.rs`) already exists and is reusable.

This slice closes all four gaps. The combined surface stays inside the
single CLI binary, requires no new npm dependencies, and reuses the
SWC transpiler and Boa harness that already ship.

## Background

### Where the relevant code lives

- `src/test_runner/harness.rs` — boots a Boa `Context` per file, evaluates
  the test file as an ES module, walks the describe tree, runs each
  `it` body, captures failures. Already calls `transpile_typescript`
  with `emit_source_map: true` and has a `remap_positions` helper that
  rewrites `file.ts:LINE:COL` substrings back to original positions.
  The failure message goes through `remap_positions` but the underlying
  `JsError::to_string()` is the only thing captured — no separate
  stack frames, no location parsed out of the matcher's message.
- `src/test_runner/reporter.rs` — formats `FileResult`s to stdout.
  Today: `PASS <name>   (<path>)` / `FAIL <name>   (<path>)` with a
  single indented message line and an optional `stack` field that is
  always `None` in the current code path.
- `src/test_runner/result.rs` — `Failure { message, stack }`,
  `TestOutcome { name_chain, status, duration_ms, failure }`,
  `FileResult { path, outcomes, load_error }`. The `Failure.stack`
  field exists but is never populated; that slot is where structured
  source location will land.
- `src/test_runner/discovery.rs` — file walker that returns absolute
  test-file paths. Knows how to descend into `.zero/components/` but
  skip every other `.zero/` subtree.
- `src/test_runner/loader.rs` — Boa module loader. Resolves
  `import … from "zero/test"` to the embedded `runtime/test.js`,
  resolves relative imports against `register_path`. Coverage
  instrumentation needs to hook here so user code is transformed on
  load but `runtime/*.js` is not.
- `src/cmd/test.rs` — current `zero test [target]` entry. Loads
  config, runs discovery, iterates files, prints via `Reporter`.
  Watch mode and coverage flags land here; mutation testing gets a
  parallel `src/cmd/mutate.rs`.
- `src/dev/watch.rs` — `notify`-based debounced file watcher already
  used by `zero dev` for SSE-driven full-page reloads. Reusable.
- `src/transpile.rs` — wraps SWC. Already returns an optional source
  map. Coverage instrumentation extends this with an AST-visiting
  pass that injects counter increments; mutation testing extends it
  with a pass that produces *N* mutant variants of a source file.
- `runtime/test.js` — JS-side test API. The `expect()` matchers
  throw plain `Error`s today; the source location is lost in the
  throw site. To surface the assertion location, errors thrown by
  matchers need to capture `new Error().stack` and pass it through
  so `harness.rs` can parse the top frame.

### What "better file & line number message on error" means concretely

User choice: **file:line + source snippet at the failing site**. The
runner already source-maps positions inside error strings; what it
does *not* do is:

- Capture the *stack* of each thrown error (not just its message).
- Identify the topmost frame that points into the user's test file
  (skipping internal frames inside `runtime/test.js` and the assertion
  helpers).
- Source-map that frame back to the original `.ts` line.
- Open the source file and pull out the offending line plus a few
  lines of surrounding context.
- Render a caret (`^`) under the column.

This requires:

- Boa exposes thrown-error stacks via `JsError::stack()` /
  the error object's `stack` property; the harness needs to actually
  read it instead of only `e.to_string()`.
- A small stack-frame parser to extract `(file, line, col)` tuples
  from each frame.
- A "user-frame" filter: skip frames whose path is `runtime/test.js`,
  `runtime/template.js`, `runtime/reactivity.js`, etc. (anything from
  the embedded shim is internal), and skip frames whose file is the
  test file but whose function name is inside `expect()` machinery.
- A source-map lookup using `sourcemap::SourceMap` already in use.
- A source-snippet renderer in the reporter.

### What's already on disk for coverage and mutation testing

Nothing. No counter table, no AST visitor for either purpose, no
report writer. Both are net-new.

### Why mutation testing in this slice

The user picked "all four items in one spec." Mutation testing is
large (operator definitions, mutant generator, per-mutant test runs,
scoring, reporter) but its dependencies are the same SWC and Boa
plumbing the other three items extend. Doing it alongside coverage in
particular shares one AST-visitor scaffold and one re-entry point into
the runner.

### Why `zero mutate` is a separate subcommand

User choice. A mutation run is N × the cost of a single test run
(one re-run per mutant); conflating it with `zero test --mutate` puts
a 30-second test run and a 30-minute mutation run under the same
command name. The separate subcommand also leaves room for
mutation-specific flags (`--operators`, `--files`, `--max-mutants`)
without polluting `zero test`'s flag space.

### Why dependency-aware watch mode

User choice. Re-running the whole suite on every save is the simplest
implementation but the slowest feedback loop; per-changed-file rerun
is fast but misleading (a change to a shared utility won't trigger
the tests that depend on it). The middle path — track each test
file's transitive import set — is what every mature watch mode does
(Jest, Vitest, node:test with `--watch`). The loader already knows
how to resolve imports; the watcher already knows when files change;
the spec just wires them together.

## Requirements

### 1. Failure location and source snippets

#### 1.1 Capture stack traces

Extend `Failure` in `src/test_runner/result.rs`:

```rust
pub struct Failure {
    pub message: String,
    pub stack: Option<String>,        // raw, source-mapped stack text
    pub location: Option<SourceLoc>,  // top user frame, source-mapped
}

pub struct SourceLoc {
    pub file: PathBuf,    // absolute path to the original .ts/.js file
    pub line: u32,        // 1-based
    pub column: u32,      // 1-based
}
```

In `harness.rs::js_err_to_failure` and `js_val_to_failure`, read the
error's `.stack` property when it is a JS object; pass it through
`remap_positions` so every frame's `file.ts:L:C` is source-mapped.
Parse the remapped stack to find the topmost frame that:

- Is not inside any embedded `runtime/*.js` path.
- Is not inside the matcher implementation (frames whose function
  name starts with one of the matcher names — `toBe`, `toEqual`,
  `toThrow`, `toHaveBeenCalled*`, etc. — or the inner `expect`
  closure).

That frame's `(file, line, column)` becomes `Failure.location`.
If no user frame can be identified, `location` stays `None` and the
reporter falls back to today's behavior (matcher message only).

#### 1.2 Surface JS-side stacks

Matchers in `runtime/test.js` throw plain `Error` objects. Their
`.stack` property in Boa contains JS frames but the top frame is
inside `expect()`'s closure. Two changes:

- Each matcher captures its location at the call site. The simplest
  mechanism: before throwing, the matcher calls a helper that does
  `new Error()` and walks `.stack` to find the first frame outside
  `runtime/test.js`, then attaches that as a property
  (`err._userFrame = "file.ts:42:7"`) on the thrown error.
- `harness.rs` reads `_userFrame` if present and prefers it over the
  parsed stack walk.

This makes the failing-line attribution robust: matcher-thrown errors
always carry the right frame even if Boa's stack format changes.

#### 1.3 Reporter rendering

Extend `Reporter::record_file` in `src/test_runner/reporter.rs` to
render failures as:

```
FAIL  Counter > increments on click   (src/components/Counter.test.ts)
        expect(0).toBe(1): values are not strictly equal
        at src/components/Counter.test.ts:14:7

          12 |     const el = render(Counter())
          13 |     fire(find(el, "button"), "click")
        > 14 |     expect(text(el, "p")).toBe("Count: 1")
             |       ^
          15 |   })
```

Details:

- The `at <file>:<line>:<col>` line uses `Failure.location` when
  present; falls back to no location line when `None`.
- The source snippet renders three lines of context (line − 2,
  line − 1, line, line + 1, line + 2 — clamped to file bounds).
- Line numbers are right-aligned, separator is ` | `, the failing
  line is prefixed with `> ` instead of `  `.
- The caret line uses spaces to pad to the column, then `^`. Column
  is taken from the source-mapped position.
- If reading the source file fails (file deleted between
  transpile and report), the snippet is omitted; the `at …` line
  still prints.
- File-load errors (current `ERROR loading …` path) gain the same
  `at file:line:col` line when a `location` is recoverable from the
  parse/runtime error.

#### 1.4 Existing remap helper stays

`remap_positions` remains the source-map gateway. The new code calls
it on whole stack strings (already supported by the regex it uses).
No changes to its signature.

### 2. Coverage (`zero test --coverage`)

#### 2.1 CLI surface

Add `--coverage` to `zero test`. When set:

- Source files matched by the coverage scope (see §2.2) are
  instrumented at transpile time before being handed to Boa.
- After all tests run, the runner aggregates per-file counters and
  prints a terminal table and writes `coverage/coverage.json`.
- Exit code semantics are unchanged (non-zero iff tests failed); a
  low coverage number does not, on its own, cause failure. A future
  `--coverage-threshold` flag is reserved but not implemented here.

#### 2.2 Coverage scope

Instrument exactly:

- Files under the project's `src/` directory (recursive).
- Both `.ts` and `.js` files.

Do **not** instrument:

- Test files (`*.test.{ts,js}`, `*.spec.{ts,js}`) anywhere on disk.
- Anything under `.zero/` (framework runtime *and* shipped components).
- Anything under `node_modules/`.
- Anything under the build output directory.

#### 2.3 Granularity

Track **line + function** coverage:

- **Line coverage:** every executable statement (the SWC AST visitor
  emits a counter increment as the first statement of each block, and
  for each top-level statement). A line is "covered" iff at least one
  statement on it ran.
- **Function coverage:** every function declaration, function
  expression, arrow function, method, and class constructor gets a
  counter increment in its prologue.

Explicitly out of scope: branch coverage (per-arm tracking for
`if`/ternary/short-circuit). The spec leaves room — the AST visitor
could grow it later without changing the report shape.

#### 2.4 Counter mechanism

A module-scoped global named `__zero_coverage__` (an object keyed by
absolute file path) is populated by Boa during the test run.
Instrumentation transforms each source file's prologue from:

```ts
// src/foo.ts (original)
export function add(a: number, b: number) { return a + b }
```

to roughly:

```ts
// src/foo.ts (instrumented)
const __c = (globalThis.__zero_coverage__ ||= {})["/abs/path/src/foo.ts"] ||= {
  lines: { 1: 0, 2: 0 /* ... */ },
  fns:   { add: 0 /* ... */ }
}
export function add(a: number, b: number) { __c.fns.add++; __c.lines[1]++; return a + b }
```

The instrumenter emits the counter map shape (which line numbers
exist, which function names exist) at transpile time and writes that
map to an in-process sidecar so the reporter knows the universe of
lines per file before any execution.

#### 2.5 Output

**Terminal table** (always written when `--coverage` is on):

```
Coverage:
  File                              Lines       Fns
  src/components/Counter.ts        12 /15    2 /  2    80.0%
  src/components/Toggle.ts          7 /12    1 /  3    58.3%
  src/routes/home.ts                4 / 4    1 /  1   100.0%
  ----------------------------------------------------------
  Total                            23 /31    4 /  6    74.2%
```

Sort by ascending coverage percentage (lowest first). Width-adapt the
file column to the longest path.

**JSON file** at `coverage/coverage.json` (always written when
`--coverage` is on; the directory is created if missing):

```json
{
  "totals": { "lines": { "covered": 23, "total": 31 }, "fns": { "covered": 4, "total": 6 } },
  "files": {
    "src/components/Counter.ts": {
      "lines": { "covered": 12, "total": 15, "uncovered": [3, 7, 14] },
      "fns":   { "covered": 2,  "total": 2,  "uncovered": [] }
    }
  }
}
```

Paths are relative to the project root, forward-slash normalized.
`uncovered` arrays let an agent jump straight to gaps without
parsing percentages.

### 3. Mutation testing (`zero mutate`)

#### 3.1 CLI surface

Add a new subcommand:

```
zero mutate [target]            Run mutation testing
  --operators <list>            Restrict to a comma-separated subset
  --max-mutants <n>             Cap total mutants generated
  -q, --quiet                   Suppress per-mutant lines, print summary only
```

`target` is an optional file path (single file) or substring filter
applied to the file scope (same rule as `zero test [target]`).

`zero mutate` reuses `src/test_runner/discovery.rs` to find test
files and runs the test suite once with no mutations (the "baseline")
to confirm it's green. If baseline fails, `zero mutate` aborts before
generating any mutants.

#### 3.2 Mutant scope

Generate mutants from files under `src/` only (same scope as
coverage). Test files, `.zero/`, `node_modules`, and the build output
are never mutated.

#### 3.3 Operator set

Eight operator families (the "standard core set"):

| Family | Mutation | Example |
| --- | --- | --- |
| Arithmetic | swap binary arithmetic | `a + b` → `a - b`; `*` → `/`; `/` → `*`; `%` → `*` |
| Comparison | swap relational | `<` → `<=`; `>=` → `>`; `==` → `!=`; `===` → `!==` |
| Boolean | swap logical | `&&` → `\|\|` and vice versa |
| Conditional negation | wrap test in `!` | `if (x)` → `if (!x)`; ternary tests likewise |
| Boundary | tighten/loosen | `<` → `<=`; `>` → `>=` |
| Literal boolean | flip booleans | `true` → `false` |
| Literal number | replace small ints | `0` → `1`; `1` → `0` |
| Empty string | flip empty/non-empty | `""` → `"zero"`; `"abc"` → `""` |

Each operator is identified by a short ID (`arith`, `cmp`, `bool`,
`cond_neg`, `boundary`, `lit_bool`, `lit_num`, `lit_str`). `--operators`
takes those IDs.

Mutations are generated at the AST level by an SWC visitor and
emitted back to source for transpile-and-execute. Each mutant has:

- `file` — absolute path
- `operator` — one of the IDs above
- `line`, `column` — original position
- `original` — short text of the original expression
- `replacement` — short text of the mutated expression

#### 3.4 Execution model

For each mutant:

1. Re-transpile that file with the mutation applied; leave all other
   files unmutated.
2. Run the same test discovery and execution loop. The test loader
   serves the mutated source for this file; everything else
   resolves normally.
3. Record the run as **killed** if at least one test failed,
   **survived** if all tests passed, **errored** if the test runner
   itself crashed (the mutant produced invalid JS).
4. Stop the run as soon as the mutant is killed (don't waste time
   exercising more tests once the verdict is in).

Mutants are processed sequentially in the first slice. Parallelism
is reserved as a future optimization but not required.

Two performance optimizations are required, both cheap:

- **Coverage-guided skip.** Before generating mutants, run the test
  suite once with coverage instrumentation (the §2 pass). For each
  line, if no test exercises it, do not generate mutants on that
  line — they could not possibly be killed. The skipped mutants are
  reported as `unreachable` in the summary (separately from killed
  / survived counters).
- **Mutant-baseline equivalence skip.** Some mutants produce code
  byte-identical to the original (e.g. `+` → `-` on a string literal
  fast-path that's coincidentally rewritten the same way after
  reprinting). Skip those before running tests.

#### 3.5 Output

**Terminal summary:**

```
Mutation testing:
  Generated: 142 mutants across 12 files
  Killed:    118  (83.1%)
  Survived:   18  (12.7%)
  Errored:    2   (1.4%)
  Skipped:    4   (2.8%)  [unreachable: 4]

Survived mutants:
  src/components/Counter.ts:14:11  arith     `count + 1` → `count - 1`
  src/routes/home.ts:8:5           cond_neg  `if (user)` → `if (!user)`
  ...

Mutation score: 83.1%
```

**JSON file** at `mutation/mutation.json`:

```json
{
  "totals": { "generated": 142, "killed": 118, "survived": 18, "errored": 2, "skipped": 4, "score": 0.831 },
  "files": {
    "src/components/Counter.ts": {
      "mutants": [
        {
          "line": 14, "column": 11, "operator": "arith",
          "original": "count + 1", "replacement": "count - 1",
          "status": "survived"
        }
      ]
    }
  }
}
```

Exit code: `0` if every generated, runnable mutant was killed;
non-zero otherwise. `--quiet` suppresses the per-mutant survived list
but keeps the summary and JSON.

### 4. Watch mode (`zero test --watch`)

#### 4.1 CLI surface

Add `--watch` (alias `-w`) to `zero test`. When set, after the
initial test run the runner enters a loop:

- Watch the project root (excluding `.git`, `node_modules`, the
  build output directory, and `coverage/` / `mutation/`).
- On any `.ts` / `.js` / `.scss` / `.css` change, compute the set of
  affected test files (see §4.3) and re-run only those.
- Clear the terminal between cycles; print the same reporter output
  as a non-watch run.
- After every run, print `> press Enter to re-run, q to quit` on
  the last line.

The watcher uses the same `notify`-based debounced infrastructure as
`src/dev/watch.rs`. Debounce window: 100 ms (match the dev server).

#### 4.2 Compatibility with `--coverage`

`zero test --watch --coverage` is supported and re-emits the
coverage table and `coverage/coverage.json` on every cycle. Coverage
data is per-cycle, not cumulative.

#### 4.3 Dependency tracking

The harness already constructs a Boa `Context` per test file and the
loader records every relative `import`. Capture those resolutions
into a per-test-file `imports: Vec<PathBuf>` and persist them in an
in-memory `HashMap<PathBuf, Vec<PathBuf>>` keyed by test file path.

When watch mode detects a changed path `P`:

- If `P` is itself a test file: re-run `P`.
- Else: re-run every test file whose transitive import set contains
  `P`. Transitivity is computed by reverse-walking the import map.
- If the map has no entry for some test files yet (first run not yet
  complete), treat them as affected (conservative).

A change to a non-`src` file (e.g. a SCSS partial under `.zero/`)
that no test file imports does not trigger a re-run. A change to a
discovery-relevant directory (creating a new test file) triggers a
full rediscovery and runs the new file.

#### 4.4 Controls

Minimal control surface:

- **Enter** — re-run the affected set (or all, if no changes since
  the last cycle).
- **q** then **Enter** — exit cleanly.
- **Ctrl+C** — exit (same effect as q).

No filter mode, no per-test selection, no raw-mode TTY handling.
Reads come from stdin's line buffer.

### Spec text amendments

The framework spec (`zero-framework-spec.md`) requires four edits:

- §1 — `zero test` flag list: confirm `--watch` and `--coverage`
  match the behavior in this spec; add `zero mutate` to the
  subcommand list.
- §8 — drop the "E2E Tests" note's framing that
  positions zero as unit/integration only; explicitly mention
  watch mode, coverage, and mutation testing as in-runner
  capabilities.
- §11 — add `mutate` to the CLI command surface table; reference
  this spec.
- §12 — mark Phase 11 items shipped once execution is done; add
  a note that mutation testing is a `zero mutate` subcommand,
  not a `zero test` flag.

The `test-runner` spec (`issues/test-runner/spec.md`) currently lists
"Watch mode" and "Coverage reporting" under deferred work. Move both
to "delivered by `issues/test-improvements/`" and remove the deferral
language.

## Constraints

- **No new npm dependencies.** Coverage instrumentation, mutation
  generation, and the watcher all use existing Rust dependencies
  (SWC, `notify`, `sourcemap`) plus standard library.
- **No V8-style native coverage.** Boa has no built-in coverage
  hook; instrumentation is source-level via SWC. This is the only
  workable path.
- **No branch coverage in this slice.** Line + function only. The
  AST visitor scaffold leaves room but does not emit branch counters.
- **No parallel mutant execution in this slice.** Sequential only.
  Parallelism is a future optimization; the spec's surface does not
  depend on it.
- **Mutation testing runs full test suite per mutant.** No
  test-impact analysis ("only run tests that touch the mutated
  file"). Coverage-guided mutant-skipping is the only avoidance
  optimization required.
- **Watch mode is line-buffered, not raw-mode.** No single-key
  shortcuts; user presses Enter after their key.
- **Coverage and mutation are mutually exclusive in one run.**
  `zero mutate` internally runs a coverage pass on the baseline,
  but it does not expose a `--coverage` flag of its own; combining
  them on `zero test` is also not supported.
- **No threshold flags.** `--coverage-threshold` and
  `--mutation-threshold` are reserved but not part of this slice.
  Exit code from `zero test --coverage` is unaffected by coverage
  percentage; exit code from `zero mutate` is non-zero iff any
  mutant survived (a stricter rule than thresholds).
- **Source snippets read from disk, not transpiled cache.** Snippets
  show the original `.ts` source the user wrote, not the JS the
  harness executed.
- **User-frame filter is a hardcoded path list.** Internal frames
  are identified by path prefix (`runtime/`) plus a fixed list of
  matcher names. Configurability is not exposed.
- **`spy()` matchers' user-frame attribution.** §1.2's
  `_userFrame` mechanism applies to every matcher in `runtime/test.js`,
  including the four spy matchers added by `issues/test-helpers/`.
  No change to the spy primitive is required.
- **Coverage instrumenter must preserve source maps.** The
  instrumented file's source map must still point back to the
  original `.ts` so §1's stack source-mapping continues to work
  during a `--coverage` run.
- **Mutation testing must respect the same module loader.** Mutants
  are produced as transformed source text and served through the
  existing `ZeroModuleLoader`'s caching path, with a per-mutant
  cache key so concurrent mutants don't poison each other.
- **Reporter output stays plain text.** No colors required in the
  first slice; the existing reporter is uncolored. (A `--color`
  flag is a future spec.)

## Out of Scope

- Branch coverage (per-arm `if`/ternary/`||`/`&&` tracking).
- HTML coverage report (`coverage/index.html`).
- lcov.info coverage output.
- Coverage thresholds (`--coverage-threshold 80`).
- Mutation testing thresholds (`--mutation-threshold 90`).
- Parallel mutant execution.
- Test-impact analysis (only run tests reaching the mutated file).
- Additional mutation operators beyond the eight in §3.3
  (statement deletion, return value mutation, regex mutation,
  array/object literal mutation).
- Watch mode raw-mode TTY shortcuts (`a` / `f` / `p`).
- Watch mode per-test name filter.
- Snapshot testing (still deferred per the test-runner spec).
- A `zero test --coverage --watch` combined output beyond
  re-emitting the table per cycle.
- Coverage of `.zero/components/` or any framework code.
- Integration with external coverage services (Codecov, etc.).
- Color or TTY-aware output in the reporter.
- Concurrent test-file execution (the harness still iterates files
  sequentially; that's an orthogonal performance spec).
- Coverage of code loaded dynamically via `import()` in user code
  (it works incidentally if Boa resolves through the same loader,
  but no special handling is required).

## Open Questions

- **Boa stack-frame format.** Boa's `Error#stack` is JS-engine
  specific. The plan should verify the exact format (V8-style
  `at fn (file:L:C)` vs. SpiderMonkey-style `fn@file:L:C` vs. a
  custom Boa shape) before committing to the parser. If the format
  is unstable across Boa versions, the `_userFrame` capture in §1.2
  becomes load-bearing and the harness-side parser becomes a
  best-effort fallback.
- **Coverage counter map representation.** Storing per-line counters
  as `{ 1: 0, 2: 0, 14: 0 }` (object) vs. `Uint32Array` of length
  `max_line`. Object is sparse and JSON-natural; array is faster to
  increment in JS. Plan picks; lean toward object for the first
  slice since perf is not the bottleneck.
- **Source-snippet width.** Hard-code three context lines, or make
  it configurable (`--snippet-context 5`)? Recommendation:
  hard-code at three (above + failing + below); revisit when a
  real test complains.
- **Mutation testing memory model.** Each mutant re-instantiates a
  Boa `Context` per test file (same as the test runner). For a
  142-mutant × 30-test-file suite that's 4,260 contexts. Verify the
  Boa Context teardown cost; if it dominates, consider reusing
  Contexts across mutants (requires module-cache invalidation per
  mutant).
- **`.toEqual` deep-diff in failure messages.** Not strictly part of
  Phase 11, but the source-snippet rendering surfaces how spartan
  matcher messages are. Plan should decide whether `.toEqual`
  failure messages grow a per-key diff in this slice or stay
  pretty-printed-as-today. Recommendation: defer to a follow-up;
  out of scope here unless trivial.
- **Watch-mode SCSS handling.** `.scss` changes don't affect test
  outcomes (tests don't import CSS). Recommendation: ignore `.scss`
  / `.css` changes in watch mode. Only `.ts` / `.js` triggers a
  cycle.
- **Watch-mode initial run cost.** First cycle has no import map
  yet, so a change in cycle 2 conservatively re-runs everything.
  Acceptable, or worth running discovery + dry-load on entry to
  pre-populate the map? Recommendation: accept the conservative
  first-cycle behavior; it self-heals.
- **Mutation operator emit format.** SWC's AST visitor can return
  a transformed AST or emit source text. Per-mutant code-gen via
  SWC's printer is the safest path (handles all edge cases of JS/TS
  syntax). Confirm in plan.
- **Should `zero mutate` show progress?** A long mutation run with
  no output looks hung. Recommendation: print `[N/M] killed: …`
  per mutant by default; `--quiet` suppresses to summary only.
- **JSON output paths.** `coverage/coverage.json` and
  `mutation/mutation.json` are written relative to the project
  root. Should these be added to `.gitignore`? Recommendation:
  add `coverage/` and `mutation/` to the scaffold's generated
  `.gitignore`; the spec doesn't enforce it but the plan should.
- **Should coverage hit-counts (vs. binary hit/miss) be reported?**
  Recommendation: track the count internally (cheap), report only
  binary covered/uncovered in the terminal table, expose counts
  in the JSON. That keeps the terminal output readable and the JSON
  useful for downstream analysis.
