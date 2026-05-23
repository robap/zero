# Plan: `zero mutate` — Static equivalence detection, skipped subdivisions, and a sane `--threads` default

## Summary

Six steps. Step 1 is mechanical plumbing — rename `skipped_equivalent` →
`skipped_equivalent_byte`, split per-operator `equivalent` into `_byte`
and `_static`, bump `mutation.json` to `schema_version: 2`, update every
test that reads the old field name or snapshots the summary output. Zero
behavior change after step 1; the new `equivalent_static` columns are
all zero. Steps 2 and 3 add the two AST-shape heuristics in the visitor
(R1.a `as const` array members, R1.b signal-init properties), each
behind a module-level pre-pass that runs before `visit_mut_module`.
Step 4 changes the `--threads` default to `min(available_parallelism, 8)`.
Step 5 is the end-to-end test against a synthetic project shaped like the
demo. Step 6 is docs + friction-log annotation.

Heuristic placement is in the visitor's Collect-mode pre-pass + the
existing literal handlers — not in the apply path. Static-equivalents
never enter the worker queue and never produce `mutation.json` entries.

## Prerequisites

The spec's four open questions are resolved here:

- **R1.b dominance approximation.** Commit to "every reference to the
  binding (other than `.set()` calls themselves) appears in source order
  after at least one `.set()`." If a fixture later shows this is too
  coarse to be useful, fall back to the stricter "every reference is
  inside a function whose body source-textually contains a prior
  `.set()`." Both are conservative.
- **`equivalent-byte` retention.** Keep it. Out of scope to collapse.
- **`num_cpus` vs `available_parallelism`.** Use
  `std::thread::available_parallelism()` (stable since 1.59,
  cgroup-aware). Verified `crates/zero` MSRV is compatible (workspace
  uses a 2024-class toolchain).
- **Visitor unit-test ergonomics.** The crate already parses TS via the
  same SWC pipeline used by `generate`; the existing unit tests (e.g.
  `lit_str_swaps_empty_and_nonempty`) parse multi-line TS sources. The
  `as const` and `signal({})` shapes parse identically. No new fixture
  loader needed.

No other issues block.

## Steps

- [x] **Step 1: Plumb new buckets through types, summary, terminal output, and JSON (no behavior change)**
- [x] **Step 2: Visitor pre-pass + R1.a `as const` array member detection**
- [x] **Step 3: Visitor R1.b signal-initializer property detection**
- [x] **Step 4: `--threads` default → `min(available_parallelism, 8).max(1)`**
- [x] **Step 5: End-to-end demo-shape test**
- [x] **Step 6: Docs and friction-log annotation**

---

## Step Details

### Step 1: Plumb new buckets through types, summary, terminal output, and JSON

**Goal:** Add every type, field, and output column the heuristic will
populate later, but keep the new counters at 0 so behavior is unchanged.
Every test that touches the renamed or restructured fields gets updated
in the same step so the codebase stays green. After this step the
codebase compiles, `cargo test --workspace` passes, and a `zero mutate`
run on any project produces output that is identical except the
`Skipped` row reads
`[unreachable: N, equivalent-byte: M, equivalent-static: 0]` and
per-operator rows show `equivalent-byte … equivalent-static 0`.

**Files:**
- `crates/zero-test-runner/src/mutate.rs` — visitor result types.
- `crates/zero/src/cmd/mutate.rs` — `MutationSummary`,
  `PerOperatorSummary`, `write_terminal_summary`,
  `write_per_operator_row`, `write_mutation_json`,
  `pre_apply_to_queue`, `generate_all_sites` (the visitor-result fold).
- All test modules in both files that touch `skipped_equivalent` or
  snapshot the summary output. `grep -rn "skipped_equivalent\|equivalent:"`
  under `crates/` to enumerate before editing.

**Changes:**

1. In `crates/zero-test-runner/src/mutate.rs`:
   - Extend `PerOperatorTally` with `equivalent_static: [usize; 8]`.
   - Extend `OperatorCounts` lookup struct with
     `equivalent_static: usize`.
   - Extend `GenerateResult` with
     `skipped_equivalent_static: usize` (global tally across operators).
   - Extend `MutateVisitor` with
     `equivalent_static_per_op: [usize; 8]` and
     `skipped_equivalent_static: usize` fields, initialized to 0 in
     both `new_collect` and `new_apply`.
   - Update the `generate()` closure return so the visitor's new
     counters flow into `GenerateResult`.

2. In `crates/zero/src/cmd/mutate.rs`:
   - Rename `MutationSummary.skipped_equivalent` →
     `skipped_equivalent_byte` (single rename + cascade through the
     module + tests).
   - Add `MutationSummary.skipped_equivalent_static: usize`.
   - In `PerOperatorSummary`, replace `equivalent: [usize; 8]` with
     two fields:
     ```rust
     pub equivalent_byte: [usize; 8],
     pub equivalent_static: [usize; 8],
     ```
   - Update the fold inside `generate_all_sites` (the function that
     copies visitor counts into `summary.per_operator`) to populate
     `equivalent_static` from `GenerateResult.per_operator.equivalent_static`
     and `skipped_equivalent_static` from
     `GenerateResult.skipped_equivalent_static`.
   - Update `pre_apply_to_queue` to write to `skipped_equivalent_byte`
     and `per_operator.equivalent_byte` (just a rename of existing
     writes).
   - Update `write_terminal_summary` `Skipped` row from
     `[unreachable: N, equivalent: M]`
     to
     `[unreachable: N, equivalent-byte: M, equivalent-static: K]`.
     Update `total_skipped` to sum all three.
   - Update `write_per_operator_row` to print
     `equivalent-byte X, equivalent-static Y` in place of
     `equivalent X`.
   - Update `write_mutation_json`:
     - Bump `schema_version` from `1` to `2`.
     - Per-operator object: replace `"equivalent": M` with two keys
       `"equivalent_byte": M, "equivalent_static": K`.
     - `totals` object: keep `skipped` as the umbrella; add
       `"skipped_unreachable"`, `"skipped_equivalent_byte"`,
       `"skipped_equivalent_static"` for callers that want the split
       (the spec's R5 text said "remove" but there was no global
       `skipped_equivalent` to remove — this is an additive expansion
       of `totals`).

3. Update every existing test:
   - Tests that read `summary.skipped_equivalent` → rename to
     `summary.skipped_equivalent_byte`.
   - Tests that read `summary.per_operator.equivalent[...]` → either
     read `equivalent_byte` or sum the two, whichever matches intent.
   - Tests that assert on terminal-summary substrings — update the
     expected substring (`equivalent:` → `equivalent-byte:` and add the
     new `equivalent-static:` column).
   - Tests that parse `mutation.json` — update key names; assert
     `schema_version == 2`.
   - The `terminal_summary_default_run_quiet_on_clean_operators` test
     (mutate.rs:1157) is a likely snapshot test; check its assertions.

**Tests:**
- All pre-existing tests, updated. `cargo test --workspace` must pass.
- Add `crates/zero/src/cmd/mutate.rs::tests::schema_version_is_2`:
  build a default `MutationSummary`, call `write_mutation_json` against
  a tempdir, parse the resulting JSON, assert
  `schema_version == 2` and the new per-operator and totals keys exist
  with zero values.

---

### Step 2: Visitor pre-pass + R1.a `as const` array member detection

**Goal:** Implement the first heuristic. Module-level
`const Name = [...] as const` declarations whose only references are
type-position (`typeof Name`, `(typeof Name)[number]`, an
`(typeof Name)[K]` chain, or no references at all) are marked, and
every literal inside the array body is tallied into
`equivalent_static` instead of becoming a `MutationSite`.

**Files:**
- `crates/zero-test-runner/src/mutate.rs` — pre-pass logic, new helper
  module/function, visitor wiring, tests.

**Changes:**

1. Add a new struct/type inside `mutate.rs`:
   ```rust
   /// Per-binding result of the static-equivalence pre-pass. Maps a
   /// `(line, column)` location of a literal to the reason it should
   /// be tallied as static-equivalent.
   #[derive(Debug, Default)]
   struct StaticEquivalence {
       /// Set of `(line, column)` keys of literals that the pre-pass
       /// determined are static-equivalent. The literal's source
       /// position is the join key — same coordinates the visitor's
       /// `line_col` produces, so the lookup is O(1).
       sites: HashSet<(u32, u32)>,
   }

   impl StaticEquivalence {
       fn contains(&self, line: u32, column: u32) -> bool {
           self.sites.contains(&(line, column))
       }
   }
   ```
   Place it adjacent to `MutateVisitor`.

2. Add a free function
   `fn analyze_static_equivalence(module: &Module, cm: &Lrc<SwcSourceMap>) -> StaticEquivalence`
   that:
   - Walks `module.body` once, collecting module-level `const`
     declarations whose initializer matches the R1.a shape:
     a `TsAs` (or `TsConstAssertion`) wrapping an `ArrayLit`, with the
     assertion being `as const`. Capture the binding's `Ident` name and
     the array literal's element list with each element's
     `(line, column)`.
   - Walks the module body a second time using a small visitor that
     records every `Ident` reference. Classify each reference:
     - **Type position** if its parent chain is a `TsTypeRef` /
       `TsTypeQuery` (i.e. `typeof Name`) / `TsIndexedAccessType` /
       `TsImportType`, etc. — anything inside a `Ts*` type node.
       Type-position references can be detected via SWC's `Visit`
       trait by tracking a depth counter that increments on entering
       any `Ts*Type` node and decrements on leaving. Reference seen
       while depth > 0 = type-position.
     - **Static-equivalent candidate own reference** if the reference
       *is* the binding's own `BindingIdent`. Skip.
     - **Runtime read** otherwise (anything else).
   - For each candidate binding: if there is no runtime read, every
     element's `(line, column)` is inserted into the result's `sites`
     set.

3. Wire the pre-pass into `generate()`:
   - After the resolver / strip pass produces `module`, call
     `analyze_static_equivalence(&module, &cm)` once.
   - Pass the result into `MutateVisitor::new_collect` via a new field
     `static_equivalence: StaticEquivalence`. Default to empty for
     `new_apply` (apply path doesn't need it — those sites are never
     reached because they didn't exist in the collect output).

4. Update `MutateVisitor::check` (or, if the location of literal
   classification differs, the `visit_mut_lit` arm). After computing
   `(line, col)` but before any other filter, check:
   ```rust
   if self.static_equivalence.contains(line, col) {
       self.equivalent_static_per_op[op.index()] += 1;
       self.skipped_equivalent_static += 1;
       return false; // don't produce a site
   }
   ```
   This check runs **after** the operator filter (so a filtered-out
   operator doesn't bump the tally) but **before** the coverage filter
   (so a static-equivalent literal is *not* double-counted as
   unreachable). Order matters; document with a comment.

5. **Important: type-stripping interaction.** The visitor runs after
   `strip()` removes TS type annotations. By that point, `as const`
   assertions are stripped too. The pre-pass MUST run BEFORE `strip`
   — verify whether `analyze_static_equivalence` should receive the
   pre-strip `Module` (parsed but not yet type-stripped). If yes, take
   the snapshot inside the same `GLOBALS.set` block before calling
   `strip`. Add a comment to that effect.

   If the pre-strip module isn't readily accessible from the current
   `generate()` flow, factor out a small refactor: run the pre-pass
   on the parsed `Module` before `program.mutate(strip(...))`, store
   the `StaticEquivalence` result, then strip and visit.

**Tests** (all in `mutate.rs::tests`):

- `static_equivalence_as_const_type_only`:
  ```ts
  const PART_STATUSES = ["out", "critical", "needs-reorder", "in-stock"] as const;
  type PartStatus = (typeof PART_STATUSES)[number];
  export function get(s: PartStatus): PartStatus { return s; }
  ```
  Run `generate` with `[Operator::LitStr]`. Assert:
  - `result.sites.is_empty()`
  - `result.per_operator.equivalent_static[Operator::LitStr.index()] == 4`
  - `result.skipped_equivalent_static == 4`

- `static_equivalence_as_const_runtime_read_disqualifies`:
  ```ts
  const TAGS = ["a", "b"] as const;
  for (const t of TAGS) console.log(t);
  ```
  Assert: 2 `LitStr` sites are produced, `equivalent_static == 0`.

- `static_equivalence_as_const_inside_function_not_eligible`:
  ```ts
  function f() {
    const TAGS = ["a", "b"] as const;
    type T = (typeof TAGS)[number];
    return TAGS;
  }
  ```
  Non-module-level binding. Assert: 2 `LitStr` sites are produced,
  `equivalent_static == 0`.

- `static_equivalence_as_const_indexed_access_only_is_type_only`:
  ```ts
  const X = ["a"] as const;
  type Y = typeof X;
  type Z = Y[number];
  ```
  Mixed type forms. Assert: 0 sites, 1 in `equivalent_static`.

---

### Step 3: Visitor R1.b signal-initializer property detection

**Goal:** Implement the second heuristic. Module-level
`const Name = signal({ ... })` (and `signal<T>({ ... })`) bindings
whose property values are all overwritten before any read (by the
source-order approximation) have those property literals tallied as
static-equivalent.

**Files:**
- `crates/zero-test-runner/src/mutate.rs` — extend pre-pass and tests.

**Changes:**

1. Extend `analyze_static_equivalence`'s first pass to also match:
   ```rust
   const <Name> = signal(<ObjectLit>)
   const <Name> = signal<...>(<ObjectLit>)
   const <Name> = computed(<ObjectLit>)  // see note
   ```
   The callee identifier is matched by *name* — `signal` or
   `computed`. Don't try to resolve the import; per spec, false
   positives from a shadowed local `signal` are accepted.

   Capture: binding name, the `ObjectLit`'s property list with each
   property's `(name, value_line, value_column)`. Only `Lit::Str`,
   `Lit::Num`, `Lit::Bool` value positions are interesting.

2. Extend the second pass to record, for each candidate binding, the
   source-order positions of:
   - **Set calls**: `Name.set(...)` — any call expression whose callee
     is a member expression `Name.set`. Record the call's source
     position.
   - **Other references**: any other use of `Name` (including
     `Name.<prop>` reads, `Name` passed as an argument, destructuring,
     etc. — basically every reference that isn't a `Name.set(...)`
     call or the binding's own `BindingIdent`).

3. Source-order dominance check: a binding qualifies under R1.b iff
   **every "other reference" position is greater than at least one
   `.set()` call position** (in source span comparison — `BytePos`).
   - If no `.set()` calls exist: disqualify (nothing overwrites the
     initial values).
   - If a reference precedes all `.set()` calls in source order:
     disqualify.
   - Otherwise: qualify. Every property value literal's
     `(line, column)` is inserted into the `sites` set.

4. The "other reference" classifier needs care:
   - `Name` appearing as the receiver of `Name.set(...)` — NOT counted
     as an other-reference (it's part of the set call).
   - `Name` appearing as the receiver of `Name.<other_prop>` — counted
     as an other-reference (it's a read).
   - `Name` appearing as a call argument or in destructuring — counted
     as an other-reference.
   - The binding itself (`const Name = ...`) — not a reference.

   Implement this in the same visitor as step 2's reference walker by
   pattern-matching the parent expression. SWC's visitor doesn't give
   parent context for free; track it via a small stack of "what kind
   of expression position are we in" inside the visitor.

5. Update the same `MutateVisitor::check` location from step 2: the
   `static_equivalence.contains(line, col)` check already covers both
   R1.a and R1.b sites because both write into the same `sites` set.
   No new check needed.

**Tests** (all in `mutate.rs::tests`):

- `static_equivalence_signal_init_overwritten_before_read`:
  ```ts
  import { signal } from "zero";
  type S = { kind: "loading" | "ok" };
  const s = signal<S>({ kind: "loading" });
  export function load() { s.set({ kind: "ok" }); }
  export function read() { return s.kind; }
  ```
  Source order: signal init → load() body → read() body. The set call
  in `load()` appears before the read in `read()`. Assert:
  - The `"loading"` literal site is in `equivalent_static`.
  - 0 sites for `LitStr` operator.

- `static_equivalence_signal_read_precedes_set`:
  ```ts
  const s = signal({ kind: "x" });
  console.log(s.kind);
  s.set({ kind: "y" });
  ```
  Read precedes the set in source order. Assert: 1 `LitStr` site
  (`"x"`), `equivalent_static == 0` for that operator.

- `static_equivalence_signal_no_set_call`:
  ```ts
  const s = signal({ kind: "loading" });
  export function read() { return s.kind; }
  ```
  No set anywhere. Assert: 1 site, `equivalent_static == 0`.

- `static_equivalence_signal_inside_function_not_eligible`:
  ```ts
  function make() {
    const s = signal({ kind: "x" });
    s.set({ kind: "y" });
    return s;
  }
  ```
  Non-module-level. Assert: 1 site (the `"x"` literal).

- `static_equivalence_signal_multiple_properties`:
  ```ts
  const s = signal({ kind: "loading", count: 0 });
  export function load() { s.set({ kind: "ok", count: 1 }); }
  export function read() { return s.kind; }
  ```
  Both `"loading"` and `0` should be tallied as static-equivalent.
  Assert: 0 sites for `LitStr`, 0 sites for `LitNum`,
  `equivalent_static` shows 1 in each operator's slot.

---

### Step 4: `--threads` default → `min(available_parallelism, 8).max(1)`

**Goal:** Replace the `default_value_t = 1` on `Mutate::threads` with a
parallel-by-default value capped at 8.

**Files:**
- `crates/zero/src/main.rs` — the `Mutate` subcommand arg definition.
- `crates/zero/src/cmd/mutate.rs` — comment on the `threads` parameter
  of `run_inner` (currently says "1 = sequential, current default")
  needs the wording corrected.

**Changes:**

1. In `main.rs`, replace:
   ```rust
   #[arg(long, default_value_t = 1)]
   threads: usize,
   ```
   with:
   ```rust
   #[arg(long, default_value_t = default_threads(),
         help = "Number of mutants to exercise in parallel. \
                 Defaults to min(cores, 8); pass 1 for sequential.")]
   threads: usize,
   ```
   and add a helper at module scope:
   ```rust
   fn default_threads() -> usize {
       std::thread::available_parallelism()
           .map(|n| n.get())
           .unwrap_or(1)
           .min(8)
           .max(1)
   }
   ```
   Note: clap's `default_value_t` evaluates the expression at clap-build
   time (per-parse). If clap requires a `const` here, fall back to
   `default_value_t = 0` and resolve `0 → default_threads()` inside the
   `match` arm in `main()` before passing to `cmd::mutate::run`. Try the
   expression form first; if clap rejects, take the resolve-on-zero
   path.

2. Update the doc comment on `run_inner`'s `threads` parameter in
   `cmd/mutate.rs:455` from "`1` = sequential, current default" to "`1`
   = sequential; the CLI defaults to `min(available_parallelism, 8)`."

**Tests:**
- `crates/zero/src/main.rs` (or a new `cli_args` test module):
  - `threads_default_uses_available_parallelism`: parse
    `vec!["zero", "mutate"]`. Assert the parsed `threads` value is
    `default_threads()`. (This is a tautological assertion against the
    helper, so it's just verifying clap wired the default through.)
  - `threads_explicit_overrides_default`: parse
    `vec!["zero", "mutate", "--threads", "2"]`. Assert `threads == 2`.
  - `threads_explicit_one_still_works`: parse
    `vec!["zero", "mutate", "--threads", "1"]`. Assert `threads == 1`.

  If clap parsing isn't easily testable as a unit, fall back to a
  smoke integration test in `crates/zero/tests/` (if such a directory
  exists; otherwise add one).

---

### Step 5: End-to-end demo-shape test

**Goal:** A single test in `crates/zero/src/cmd/mutate.rs` that builds
a synthetic project containing both R1.a and R1.b shapes, runs
`run_inner`, and asserts the demo's pathological case
(`Survived: 5`, `Score: 37.5%`) is now `Survived: 0`, `Score: 100%`,
with five mutants in `equivalent_static`.

**Files:**
- `crates/zero/src/cmd/mutate.rs` — new test
  `mutate_reclassifies_static_equivalents_end_to_end`.

**Changes:**

1. The existing test module already builds tempdirs with synthetic
   `src/` content. Reuse that helper (or factor a minimal version if
   none exists). The test:
   - Creates a tempdir with `zero.toml`, an `index.html`, a minimal
     `src/app.ts` entrypoint, and a `src/stores/parts.ts` whose
     contents mirror the demo:
     ```ts
     import { signal } from "zero";
     export const PART_STATUSES = ["out", "critical", "needs-reorder", "in-stock"] as const;
     export type PartStatus = (typeof PART_STATUSES)[number];
     type PartsState = { kind: "loading" } | { kind: "ok"; items: number[] } | { kind: "error" };
     export const partsSignal = signal<PartsState>({ kind: "loading" });
     export function load() {
       partsSignal.set({ kind: "ok", items: [] });
     }
     export function error() {
       partsSignal.set({ kind: "error" });
     }
     ```
   - Creates `src/stores/parts.test.ts` with a test that calls
     `load()` and asserts on `partsSignal`'s state. This gives the
     baseline coverage that makes the `.set({ kind: "ok" })` literals
     **killed** and the initial `"loading"` static-equivalent.
   - Runs `run_inner` with `Isolation::InProcess` (faster, no
     subprocess), `operators = [Operator::LitStr]`,
     `max_mutants = None`, `quiet = true`, `threads = 1`.
   - Asserts:
     - `summary.survived == 0`
     - `summary.skipped_equivalent_static == 5` (4 array members + 1
       signal-init property)
     - `summary.per_operator.equivalent_static[Operator::LitStr.index()] == 5`
     - `summary.score() == 1.0` (or the killed count divided by
       executed equals 1.0 within float epsilon)

2. Also assert on `mutation.json`: after `run_inner`, call
   `write_mutation_json` against the tempdir, parse the JSON, assert
   `schema_version == 2`, `totals.skipped_equivalent_static == 5`,
   `operators.lit_str.equivalent_static == 5`.

**Tests:**
- The single end-to-end test described above. Cleanup the tempdir
  on success.
- This test is the executable contract for the spec. It must fail
  before steps 2 and 3, and pass after.

---

### Step 6: Docs and friction-log annotation

**Goal:** Update the user-facing docs and mark the friction-log
entries.

**Files:**
- `docs/config-and-cli.md` — `zero mutate` reference.
- `~/Documents/code/zero_demo/FRAMEWORK_NOTES.md` — friction-log
  entries #46, #47, #48, #50.

**Changes:**

1. `docs/config-and-cli.md`:
   - Find the `zero mutate` section. Update the `--threads` default
     text from `1` to `min(cores, 8)`. Add a one-sentence rationale:
     "Parallel by default; the cap keeps headroom on bigger boxes
     for IDE / build processes."
   - Update the example output block (if present) to show the new
     `Skipped` row format
     `[unreachable: N, equivalent-byte: M, equivalent-static: K]` and
     the new per-operator row format
     (`… unreachable X, equivalent-byte Y, equivalent-static Z`).
   - Add a "Reading 'equivalent-static'" subsection (≤ 1 paragraph):
     explain that these are mutants the visitor proved no-op by AST
     shape (currently: `as const` arrays only referenced in type
     position; module-level signal initializer properties overwritten
     before any read). Skipped at collect time — they don't pad
     `survived` or run through the worker queue.
   - If `docs/config-and-cli.md` contains `mutation.json` schema
     documentation, bump the version reference to `2` and document the
     new field names. If the docs don't currently document the schema,
     skip — but flag this in the plan-completion summary.

2. `~/Documents/code/zero_demo/FRAMEWORK_NOTES.md`:
   - Flip `- [ ]` to `- [x]` on entries #47, #48, #50.
   - Each flipped entry gets the fix annotation per the file's
     convention:
     `**FIXED YYYY-MM-DD** (issues/mutate-equivalence/): one-sentence
     note on what changed.`
   - For #46: do NOT flip — instead append a `**PARTIAL YYYY-MM-DD:**`
     annotation noting that the friction-log description was off
     (literals survive, they don't error) and pointing to
     `issues/mutate-equivalence/` for the real fix. Per the file's own
     guidance: "If a fix is partial (e.g. the docs gap is closed but
     the underlying CLI behavior still surprises), leave the box
     unchecked and append a `**PARTIAL YYYY-MM-DD:** ...` note."
     Actually — the underlying behavior IS fixed (the survivors are
     reclassified), it was only the description that was wrong.
     **Decision: flip #46 to `[x]` with the fix annotation noting the
     description was inaccurate**, since the user's observed symptom
     (these mutants pad the score) is fully resolved. The `PARTIAL`
     guidance is for cases where the underlying behavior still
     surprises, which doesn't apply here.

**Tests:**
- None code-side. The docs page renders via the existing GH-Pages
  build; smoke check by opening `docs/config-and-cli.md` locally and
  scanning for the new strings.

---

## Risks and Assumptions

- **Type-strip ordering risk.** R1.a depends on detecting `as const`
  on the parsed AST. If the pre-pass accidentally runs after
  `strip()`, the `TsAs` / `TsConstAssertion` nodes are gone and the
  rule silently matches nothing. Step 2's "Important" note flags this;
  the test
  `static_equivalence_as_const_type_only` will fail loudly if the
  ordering is wrong. Mitigation: run pre-pass before `strip` and
  verify in the unit test.
- **Source-order dominance is coarse.** R1.b's check accepts any
  ordering where every reference is after at least one `.set()`. This
  passes some shapes that aren't truly safe (e.g. a top-level `if`
  branch that reads before any function-scoped set runs). Acceptable
  per spec — the rule is intentionally a heuristic and errs toward
  *generating* mutants by being conservative about what counts as
  "after". If the demo's `partsSignal` doesn't qualify (e.g. because
  `partsSignal.set({ kind: "loading" })` in `load()` is in a function
  body that precedes `partsSignal.kind` reads but doesn't *execute*
  before them), the unit test in step 3 will catch it and we adjust
  the rule before step 5.
- **Clap `default_value_t = expr()` may not compile.** If clap
  requires `const` for `default_value_t`, the fallback ("default 0,
  resolve to `default_threads()` in `main()`") adds one branch but is
  trivially testable. Step 4's risk is bounded.
- **Snapshot tests may have many call sites.** The rename in step 1
  may touch every test that exercises terminal-summary output. If
  the count is unexpectedly large, step 1 grows but stays focused —
  no behavioral correctness is at stake.
- **`available_parallelism` returns container limit.** Inside CI
  containers this may be 2 or 4; the new default still beats `1`.
  Inside a 1-core environment, the default reduces to `1` and parity
  with the old behavior is preserved.
- **Demo end-to-end test is slow.** Step 5's test spins up the runner
  on a real tempdir project. Mitigation: use `Isolation::InProcess`,
  filter operators to `LitStr` only, keep the synthetic project
  minimal. If this still runs > 5s, mark `#[ignore]` and run via a
  feature flag — but try to keep it in the default suite.
- **Friction-log entry #46 disposition.** Spec says it collapses into
  #48; plan step 6 flips it `[x]`. If you'd rather it stays open with
  a `PARTIAL` annotation, that's a one-line change to step 6 with no
  code impact.
