# Plan: `zero mutate` — fix coarse coverage granularity that misclassifies reachable mutation sites as unreachable

## Summary

Five steps. The bug: coverage instrumentation only emits a line counter
for top-level module items and each function's *first* body statement, but
mutate's reachability filter keeps a site only if its exact line is in the
covered set — so any mutation site below a function's first body line (or
on a continuation line of a multi-line statement) is wrongly filed
`unreachable`, silently inflating the score to a vacuous 100%
(friction-log #61 `transactionsQueryString`, #64 `StatCard`).

The fix is taken in two independent, separately-green steps:

- **Step 1 (R2)** makes the coverage instrumenter emit a line counter
  before *every* statement, via a new `visit_mut_stmts` override in
  `coverage.rs`, reconciled with the existing first-statement and
  top-level counters so no line is double-injected. After Step 1 the
  covered-line set is complete and the multi-statement *function body*
  case (#61) is fixed, because each statement now sits on a counted line.
- **Step 2 (R3)** makes mutate's reachability filter consult the start
  line of a site's *enclosing statement / module item* (tracked via
  `visit_mut_stmt` + `visit_mut_module_item` overrides) instead of the
  site's own line. This closes the *continuation-line* case (#64
  `StatCard`, and multi-line top-level initializers like `page: 1`), where
  the mutable token is below its statement's start line. The site's own
  `line`/`column` (used for reporting and the static-equivalence key) is
  unchanged; only the coverage check changes.

Step 3 is the end-to-end acceptance contract (both demo shapes + a
transitively-tested, no-sibling component + a genuinely-unreached
regression guard). Step 4 is docs. Step 5 flips friction-log #61 and #64
with corrected diagnoses.

The change is **monotonic**: the covered-line set only grows (Step 1) and
the reach line is `≤` the site line (Step 2), so no site that mutates
correctly today can become unreachable.

## Prerequisites

The spec's open questions are resolved here so execution doesn't stall:

- **Exact statement node set for R2.** Instrument through one
  `visit_mut_stmts(&mut Vec<Stmt>)` override: a line counter before each
  `Stmt` in every `Vec<Stmt>` context (function bodies, block bodies,
  nested `if`/`else`/`try`/`catch`/`for`/`while` blocks, and `switch`
  `case` statement lists — all of which are `Vec<Stmt>`). Top-level module
  items keep their existing per-item counters from `visit_mut_module`
  (`Vec<ModuleItem>` is not a `Vec<Stmt>`, so it is not touched by the new
  override → no double count). **Brace-less single-statement bodies**
  (`if (x) doThing();` — the `cons` is a `Box<Stmt>`, not a `Vec<Stmt>`)
  get no own counter; for the common single-line form the brace-less
  statement shares its parent's counted line, and R3 attributes the site
  to that line, so it still resolves. A *multi-line* brace-less body is a
  documented, narrow residual (consistent with the spec's per-line, not
  per-branch, precision boundary).
- **Reconciling existing counters.** `visit_mut_stmts` becomes the single
  source of statement-line counters. Drop the first-body-statement line
  counter from `instrument_body`, `visit_mut_arrow_expr`, and
  `visit_mut_constructor` (keep their `fns` entry counters). The `lines`
  `BTreeSet` dedups, so idempotency (`is_idempotent_within_one_module`)
  holds.
- **Enclosing-statement tracking (R3).** Add `reach_line: u32` to
  `MutateVisitor`; override `visit_mut_module_item` and `visit_mut_stmt`
  to save/set/restore it to the node's start line; `check`'s coverage
  filter consults `self.reach_line` (falling back to the passed `line`
  when `reach_line == 0`, which cannot happen for a real site but is a
  safe sentinel). Because the enclosing unit is a *module item or
  statement*, this also fixes multi-line **top-level** initializers, not
  just function bodies.
- **Blast-radius inventory.** Confirmed by grep: no framework test asserts
  instrumented line *counts* — `crates/zero/src/cmd/test.rs` only checks
  `coverage.json` exists, and `coverage.rs`'s `aggregator_totals_sum_correctly`
  uses hand-built `CoverageMap`s (unaffected by instrumentation density).
  Only `coverage.rs`'s instrumenter unit tests need attention, and they
  assert "line N is present / fn fired," which survives a larger line
  universe. Demo/`examples/` coverage numbers shift but are not asserted
  in this repo.
- **#63 (Boa GC teardown) interaction.** It did not reproduce here (the
  full instrumented demo suite ran `parts.test.ts` clean). Step 1 adds a
  density-mitigation lever (dedupe counters by line within a `Vec<Stmt>`)
  and Step 3's risk note requires a before/after run of the instrumented
  demo suite. #63 stays a separate issue; only its non-aggravation is in
  scope.

No other issues block.

## Steps

- [x] **Step 1: Per-statement coverage instrumentation (R2)**
- [x] **Step 2: Enclosing-statement reachability attribution in the mutate visitor (R3)**
- [x] **Step 3: End-to-end acceptance tests for both demo shapes (R1 contract, R4)**
- [x] **Step 4: Docs — `Generated: 0` / unreachable wording and `--coverage` granularity (R6)**
- [x] **Step 5: Friction-log annotations #61 + #64 (R7)**

---

## Step Details

### Step 1: Per-statement coverage instrumentation (R2)

**Goal:** Make `coverage.rs` record a runtime line counter before every
statement so the covered-line set reflects each executed statement, not
just function entries and top-level items. After this step,
`zero test --coverage` line numbers grow (and become honest), and the
multi-statement *function body* case (#61 `transactionsQueryString`) is
fixed because each statement now sits on a counted line that mutate's
existing `site.line` filter accepts. The codebase stays green.

**Files:**
- `crates/zero-test-runner/src/coverage.rs` — `InstrumenterVisitor`:
  new `visit_mut_stmts` override; trim first-statement counters from
  `instrument_body`, `visit_mut_arrow_expr`, `visit_mut_constructor`;
  instrumenter unit tests.

**Changes:**

1. Add a `visit_mut_stmts` override to `impl VisitMut for InstrumenterVisitor`
   (place near `visit_mut_block_stmt`):
   ```rust
   fn visit_mut_stmts(&mut self, stmts: &mut Vec<Stmt>) {
       // Instrument nested statements first so inner blocks are counted.
       stmts.visit_mut_children_with(self);
       // Prepend a line counter before each statement. Dedupe by source
       // line within this Vec so multiple statements sharing a line add
       // at most one counter (keeps instrumentation density — and the
       // #63 GC-teardown surface — minimal).
       let mut out: Vec<Stmt> = Vec::with_capacity(stmts.len() * 2);
       let mut seen: BTreeSet<u32> = BTreeSet::new();
       for stmt in std::mem::take(stmts) {
           let line = self.line_of(&stmt);
           if seen.insert(line) {
               out.push(self.line_counter_stmt(line));
           }
           out.push(stmt);
       }
       *stmts = out;
   }
   ```
   `Stmt` is already imported in `coverage.rs`. `line_of` and
   `line_counter_stmt` already exist and `line_counter_stmt` inserts into
   `self.lines`, so the static universe and the runtime counters stay in
   sync.

2. Trim the now-redundant first-statement counters so no line is
   double-injected (the function body's statements are instrumented by
   `visit_mut_stmts` during the recursive visit):
   - `instrument_body`: keep the `fn_counter_stmt(name)` in `prefix`;
     **remove** the `body.stmts.first()` line-counter block. Still call
     `body.visit_mut_children_with(self)` then prepend `prefix`.
   - `visit_mut_arrow_expr`: keep `fn_counter_inline(&name)`; **remove**
     the `block.stmts.first()` line-counter block; recurse via
     `block.visit_mut_children_with(self)` then prepend the fn counter.
   - `visit_mut_constructor`: same — keep the constructor `fn` counter,
     drop the first-stmt line counter, recurse, prepend.
   - `visit_mut_module` (top-level per-item counters) and
     `visit_mut_block_stmt` (recurse-only) are unchanged.

3. No change to `build_prologue`, the `fns` map, or `CoverageMap`.

**Tests** (in `coverage.rs::tests`):

- `instruments_every_statement_in_a_body` (new):
  ```ts
  export function f(go) {
    const a = 1;
    if (go) { return a; }
    return 0;
  }
  ```
  Strip `export`, eval in boa, call `f(true)` and `f(false)`. Assert the
  recorded `out.map.lines` includes the line of `const a`, the `if`, the
  inner `return a`, and the trailing `return 0` — i.e. every statement
  line, not just the first — and that the inner/return counters fire when
  the branch runs. This is the executable proof that the coarse universe
  is gone.
- `multiple_statements_one_line_count_once` (new): `const a=1; const b=2;`
  on a single physical line → `out.map.lines` contains that line once
  (dedupe holds), counter fires.
- Re-verify the existing tests still pass unchanged:
  `instruments_top_level_statement_increments_line_counter`,
  `instruments_function_prologue`, `instruments_arrow_function`,
  `coverage_map_contains_all_known_lines_and_fns_zero_initialized`,
  `is_idempotent_within_one_module`. They assert "line N present / fn
  fired," which survives a larger line universe; adjust only if an
  assertion pins an exact `lines` length (none currently do).

Run `cargo test -p zero-test-runner --tests`.

---

### Step 2: Enclosing-statement reachability attribution in the mutate visitor (R3)

**Goal:** Make mutate decide reachability from the site's *enclosing
statement / module item* start line rather than the site's own line. This
closes the continuation-line case (#64 `StatCard`'s `cond_neg` on line 12,
attributed to its `return` statement on line 9) and multi-line top-level
initializers. The site's reported `line`/`column` and the
static-equivalence key are untouched. Green after this step.

**Files:**
- `crates/zero-test-runner/src/mutate.rs` — `MutateVisitor` field +
  constructors, two new visit overrides, `check` coverage-filter change,
  visitor tests.

**Changes:**

1. Add a field to `MutateVisitor`:
   ```rust
   /// Start line of the innermost enclosing statement / module item.
   /// Reachability is judged on this line (which carries a coverage
   /// counter) rather than the site's own line, so a mutable token on a
   /// continuation line of a multi-line statement is reached whenever the
   /// statement executed. 0 until the first statement/item is entered.
   reach_line: u32,
   ```
   Initialize `reach_line: 0` in both `new_collect` and `new_apply`.

2. Add two overrides to `impl VisitMut for MutateVisitor` (save / set /
   recurse / restore — same behavior as the default visit plus the line
   bookkeeping):
   ```rust
   fn visit_mut_module_item(&mut self, item: &mut ModuleItem) {
       let prev = self.reach_line;
       self.reach_line = self.line_col(item).0;
       item.visit_mut_children_with(self);
       self.reach_line = prev;
   }

   fn visit_mut_stmt(&mut self, stmt: &mut Stmt) {
       let prev = self.reach_line;
       self.reach_line = self.line_col(stmt).0;
       stmt.visit_mut_children_with(self);
       self.reach_line = prev;
   }
   ```
   Ensure `ModuleItem` and `Stmt` are imported in `mutate.rs` (add to the
   `swc_core::ecma::ast` import list if absent). `line_col` already
   accepts any `Spanned`.

3. Change only the coverage branch of `check` (the
   `Mode::Collect` arm). Leave the operator filter, the
   static-equivalence check (keyed on the literal's `line, column`), the
   `matched`/`unreachable` tallies, and the emitted `MutationSite`
   `line`/`column` exactly as they are:
   ```rust
   if let Some(cov) = self.covered_lines {
       let reach = if self.reach_line != 0 { self.reach_line } else { line };
       if !cov.contains(&reach) {
           self.skipped_unreachable += 1;
           self.unreachable_per_op[idx] += 1;
           return false;
       }
   }
   ```
   Add a one-line comment explaining `reach` is the enclosing
   statement/item line. `Mode::Apply` is unaffected (`covered_lines` is
   `None` there).

**Tests** (in `mutate.rs::tests`, using `generate` with an explicit
`covered_lines` set):

- `reach_attributes_continuation_line_site_to_statement_start` (new):
  ```ts
  function f(cond) {
    return cond
      ? "a"
      : "b";
  }
  ```
  `covered_lines = { line of the `return` }` only. Generate with
  `[Operator::LitStr]`. Assert both `"a"` and `"b"` sites are **produced**
  (reachable via `reach_line`), and `skipped_unreachable == 0`. Under
  today's code these would be skipped (their own lines aren't covered).
- `reach_top_level_multiline_initializer` (new):
  ```ts
  export const Q = {
    type: null,
    page: 1,
  };
  ```
  `covered_lines = { line of `export const Q` }`. Generate with
  `[Operator::LitNum]`. Assert the `1` site is produced (attributed to the
  `export const` line), `skipped_unreachable == 0`.
- `reach_unreached_statement_still_skipped` (new): two statements, the
  covered set includes only the first; the site in the second statement is
  `skipped_unreachable == 1`, 0 sites. Guards that the bucket does not
  collapse — genuinely unexecuted code is still skipped.
- `reach_covered_site_on_own_line_unaffected` (new / or assert within an
  existing test): a single-line statement whose site is on its own (and
  the statement's) line, with that line covered → site produced, matching
  today's behavior (monotonic non-regression).

Run `cargo test -p zero-test-runner --tests`.

---

### Step 3: End-to-end acceptance tests for both demo shapes (R1 contract, R4)

**Goal:** The executable contract that the demo's pathological cases are
fixed and not vacuous, and that the `unreachable` bucket still works.
These tests fail before Steps 1–2 and pass after; they are the
diagnose-then-fix endpoint (the diagnosis itself was confirmed by CLI
reproduction — see Diagnosis log).

**Files:**
- `crates/zero/src/cmd/mutate.rs` — new tests beside
  `mutate_reclassifies_static_equivalents_end_to_end`, reusing the
  existing `make_project` / `write_zero_toml` helpers.

**Changes:**

1. `mutate_reaches_sites_below_function_first_line` (the #61
   `transactionsQueryString` shape). Synthetic `src/foo.ts`:
   ```ts
   export function q(p: { type: string | null; page: number }): string {
     const parts: string[] = [];
     if (p.type) parts.push("type=" + p.type);
     if (p.page !== 1) parts.push("page=" + String(p.page));
     return parts.length ? "?" + parts.join("&") : "";
   }
   ```
   with a **strong** sibling test that asserts on `q(...)` across inputs
   (default, type-only, page-only, both). Run `run_inner` with
   `Isolation::InProcess`, `operators = Operator::ALL`, `threads = 1`.
   Assert:
   - `summary.killed + summary.survived + summary.errored > 0`
     (mutants actually executed — not all skipped),
   - at least the `lit_str` / `lit_num` / `cmp` sites that live on lines
     *below* line 1 are reached (e.g. `summary.per_operator.executed(...)`
     for those operators `> 0`),
   - the score is non-vacuous (killed-driven, not `Generated: 0`).

2. `mutate_weak_test_surfaces_survivor` (same source, **weak** sibling
   test that calls `q(...)` but asserts nothing meaningful). Assert
   `summary.survived > 0` — proving the tool now *detects* the
   vacuous-test case it currently hides behind `unreachable`.

3. `mutate_reaches_transitively_tested_no_sibling` (the #64 `StatCard`
   shape: continuation-line site, transitively covered, no sibling test).
   Layout:
   - `src/widget.ts` — exported function with a multi-line ternary whose
     condition / arms hold mutable tokens on continuation lines (mirrors
     `props.sub ? … : …`), **no** `widget.test.ts`.
   - `src/page.ts` — imports and calls `widget(...)` in both branches.
   - `page.test.ts` — tests `page.ts` (thereby exercising `widget`
     transitively).
   Run `run_inner` targeting `src/widget.ts`. Assert the `cond_neg` (and
   any `lit`) site is **executed** (not `unreachable`), and that
   `baseline.covered`/`src_to_tests` credit `widget.ts` via `page.test.ts`
   (i.e. transitive coverage is honored — guards against a future
   "sibling-only linkage" regression). Score is non-vacuous.

4. `mutate_genuinely_unreached_still_unreachable` (regression guard): a
   source file with an exported function **no test calls**; assert its
   sites land in `summary.skipped_unreachable` and `executed == 0` for
   that file's operators — the bucket must not collapse to zero.

**Tests:** the four tests above are the deliverable. Keep them in the
default suite (`Isolation::InProcess`, minimal projects). If any exceeds
~5s, mark `#[ignore = "slow"]` per the repo convention, but prefer the
default suite.

Run `cargo test -p zero --tests`, then `cargo test --workspace`.

---

### Step 4: Docs — `Generated: 0` / unreachable wording and `--coverage` granularity (R6)

**Goal:** Stop the next adopter from re-deriving the misdiagnosis, and
make the coverage metric's meaning explicit. No `mutation.json` schema
change.

**Files:**
- `docs/config-and-cli.md` — `zero mutate` "Reading `Generated: 0`"
  section and the `zero test --coverage` row.
- `docs/testing.md` — only if it characterizes coverage granularity.

**Changes:**

1. `docs/config-and-cli.md`, "Reading `Generated: 0`" (currently lines
   ~178–196): reword the **"All matches on uncovered lines"** bullet so
   `unreachable` means *the enclosing statement never executed in any
   baseline test* — not "the line lacks a counter." Remove the
   implication that fully-tested code can read as unreachable; that was
   the bug. Keep the other three bullets.
2. `docs/config-and-cli.md`, the `--coverage` flag row / surrounding prose
   (~line 156): note the line metric is **per executable statement** (each
   statement carries a counter), so the number reflects real line
   execution.
3. Add one sentence stating `mutation.json` is unchanged (still
   `schema_version: 2`) so a reader doesn't expect a bump.
4. `docs/testing.md`: if it describes coverage as function- or
   entry-grained, align it to per-statement. If it says nothing about
   granularity, leave it (note this in the step-completion summary).

**Tests:** none code-side; per CLAUDE.md the smoke check is "renders
sensibly." Re-read the edited sections for the new strings.

---

### Step 5: Friction-log annotations #61 + #64 (R7)

**Goal:** Close both entries in the demo's friction log with the
*corrected* diagnoses, preserving the discovered ground truth (the
convention used for #46).

**Files:**
- `~/Documents/code/zero_demo/FRAMEWORK_NOTES.md` — entries #61 and #64.

**Changes:**

1. #61: flip `- [ ]` → `- [x]`; append
   `**FIXED YYYY-MM-DD** (#PR / commit SHA):` noting the re-export was a
   red herring; the cause was coarse (function-entry-only) coverage
   instrumentation vs. an exact-line reachability filter, fixed by
   per-statement instrumentation plus enclosing-statement attribution
   (`issues/mutate-reachability/`).
2. #64: flip `- [ ]` → `- [x]`; append the same-style annotation noting
   the "only credits a file's own sibling test" hypothesis was wrong —
   transitive coverage *is* credited (`StatCard.ts` reads 100% from
   `home.test.ts`); it was the same continuation-line-of-a-multi-line-
   statement bug as #61, closed by the same fix.
3. Leave #63 and #65 untouched.

Use today's date for `YYYY-MM-DD`. Fill the PR/commit SHA at landing time.

**Tests:** none.

Run `cargo test --workspace -- --include-ignored` once more to confirm the
slow build/test/coverage integration tests pass with the shifted coverage
numbers.

---

## Risks and Assumptions

- **`visit_mut_stmts` double-instrumentation.** If Step 1 adds
  `visit_mut_stmts` but forgets to trim the first-statement counters in
  `instrument_body` / arrow / constructor, the first body statement gets
  two counters on one line — harmless at runtime (the `lines` set dedups,
  reachability is unaffected) but sloppy. The dedupe-by-line in
  `visit_mut_stmts` only guards within one `Vec`; the cross-path overlap
  is removed by the trims in change (2). The new
  `instruments_every_statement_in_a_body` test does not assert *counts*,
  so it won't catch a double-count; a quick eyeball of emitted code during
  execution is the check.
- **Brace-less multi-line bodies residual.** `if (cond)\n  doThing(x);`
  leaves the inner statement on a line with no counter and no enclosing
  `Vec<Stmt>`; a site in `x` attributes to the inner `ExprStmt` line
  (uncovered) and stays `unreachable`. Rare; accepted and consistent with
  the per-line precision boundary. If a real case appears, the follow-up
  is to normalize brace-less bodies to blocks before instrumenting — out
  of scope here.
- **Monotonicity.** The argument: Step 1 only enlarges the covered set;
  Step 2's `reach_line ≤ line`, and if a site executed then its enclosing
  statement executed (so its start line is covered under Step 1).
  Therefore no site reachable today becomes unreachable. The
  `reach_covered_site_on_own_line_unaffected` test guards the common case;
  `cargo test --workspace` guards the rest.
- **Coverage-number churn (`--coverage`).** Demo and `examples/` line
  totals rise. No framework test asserts those counts (grep-confirmed), so
  the workspace suite should stay green; the slow integration suite
  (`--include-ignored`) is the backstop in Step 5. If an unforeseen
  example test pins a number, update it as mechanical churn.
- **#63 (Boa GC teardown) aggravation.** Denser instrumentation could, in
  principle, raise the teardown-panic likelihood on large files. It did
  not reproduce here. Mitigation already in Step 1 (dedupe counters by
  line). Execution must run the full instrumented demo suite before/after
  Step 1 a few times; if R2 surfaces #63, pause and coordinate with that
  separate issue rather than expanding this one.
- **`Spanned` line of a statement after strip.** Step 2 reads
  `line_col(stmt)` on the post-`strip` AST; surviving nodes retain their
  original `BytePos`, so the start line matches the source (same
  assumption the static-equivalence pre-pass and the existing site lines
  already rely on). The
  `reach_attributes_continuation_line_site_to_statement_start` test fails
  loudly if this assumption is wrong.
- **Import additions.** Step 2 may need `ModuleItem` / `Stmt` added to
  `mutate.rs`'s `swc_core::ecma::ast` import list; Step 1 uses `Stmt` /
  `BTreeSet` already imported in `coverage.rs`. Compile errors surface
  these immediately.

## Diagnosis log

**Confirmed (CLI reproduction, pre-implementation).** Against the demo
(`~/Documents/code/zero_demo`, `project.root = web`) with the installed
`zero`:

- `src/lib/transactions.ts` (#61): `zero test --coverage` → `5/5 lines,
  2/2 fns = 100%`; `zero mutate --quiet web/src/lib/transactions.ts` →
  `Generated: 0`, `Skipped: 19 unreachable`, vacuous `100.0%`.
- `src/lib/theme.ts` (control, no re-export): 7 mutants killed; its 7
  killed sites all sit on lines 3, 4, 42 (top-level consts + the *first*
  body line of `toggleTheme`) — i.e. coarse-instrumented lines. Its
  executed `getTheme`/`read` literals (non-first body lines) fall in its
  11 `unreachable`, proving the bug is granularity, pervasive, not the
  re-export.
- `src/components/StatCard.ts` (#64): `coverage.json` credits it 100%
  transitively via `home.test.ts` (`fns 1/1, lines 2/2`); `zero mutate`
  reports its lone `cond_neg` site (line 12, a continuation line of the
  multi-line `return html\`…\`` starting line 9) as `unreachable`,
  printing vacuous `100.0%`. Disproves the "sibling-only linkage"
  hypothesis.

The mechanism is coarse coverage instrumentation (function-entry +
top-level only) vs. an exact-line reachability filter. No further
diagnosis needed before implementation; Steps 1–2 are the fix and Step 3
is the regression contract.
