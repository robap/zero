# Spec: `zero mutate` — fix coarse coverage granularity that misclassifies reachable mutation sites as unreachable

## Problem Statement

Open friction-log entry #61 in `~/Documents/code/zero_demo/FRAMEWORK_NOTES.md`
(🔴): `src/lib/transactions.ts` has a sibling `transactions.test.ts` that
exercises `transactionsQueryString` end to end. `zero test --coverage`
reports the file at **100%** (5/5 lines, 2/2 fns), yet
`zero mutate web/src/lib/transactions.ts` reports **all 19 mutation sites as
unreachable** (`Generated: 0`, `Skipped: 19 unreachable`), producing a
vacuous `Mutation score: 100.0%`. Coverage and mutate disagree about the
same source path.

The friction log's hypothesis — "the lib file re-exports types from a peer
store module, which may confuse mutate's test→source linkage" — is
**incorrect** (this spec re-diagnoses it, in the same vein as #46 and #54
where the friction-log description was off). The real cause is a
**granularity mismatch** between how coverage is instrumented and how
mutate decides a site is "reachable." It is **not** specific to re-exports,
and it silently inflates mutation scores across the whole codebase, not
just this one file.

A second friction-log entry, #64 (🟡, `2026-05-29`), is the **same bug**
in a different shape: `src/components/StatCard.ts` has no sibling test and
is exercised only transitively through `home.test.ts` render assertions,
yet its one `cond_neg` site reports `unreachable` and prints
`Mutation score: 100.0%`. #64 blames a *different* mechanism — "mutate's
mutant→test linkage only credits a file's own sibling test" — but that
hypothesis is also wrong: coverage credits `StatCard.ts` at 100%
*transitively* (see Background), and the dropped site is simply on a
continuation line of a multi-line statement. #61 and #64 are one fix; this
spec covers both. (Two further `2026-05-29` entries are **not** in scope:
#63, a Boa GC teardown panic under instrumentation, is a distinct runtime
fault treated as a dependency/risk below; #65, a `zero dev` proxy
query-string bug, is unrelated.)

This matters now because mutation testing is the framework's headline
"is your test suite actually checking anything" tool (see
`docs/agentic-coding.md` — "tests can look thorough while passing
vacuously; mutation testing is how you detect that"). A reachability
filter that drops genuinely-tested code makes the tool itself pass
vacuously: it reports a perfect score while never running a single mutant.
It also exposes a second, latent bug — `zero test --coverage`'s line metric
is itself dishonest (see Background) — which this fix resolves in passing.

## Background

### Reproduction (confirmed against the current installed CLI)

From the demo root (`~/Documents/code/zero_demo`, whose `zero.toml` sets
`project.root = "web"`):

```
$ zero test --coverage        # (excerpt)
  src/lib/theme.ts            16 / 16    7 / 7    100.0%
  src/lib/transactions.ts      5 /  5    2 / 2    100.0%

$ zero mutate --quiet web/src/lib/transactions.ts
  Generated: 0 mutants across 0 files
  Skipped:   19  [unreachable: 19, equivalent-byte: 0, equivalent-static: 0]
  ... lit_str: matched 8, executed 0, unreachable 8 ...
  Mutation score: 100.0%

$ zero mutate --quiet web/src/lib/theme.ts
  Generated: 7 mutants ... Killed: 7 (100.0%) ... Skipped: 11 [unreachable: 11, ...]
```

`theme.ts` mutates normally; `transactions.ts` does not, despite both
being at 100% line coverage with sibling test files.

### Root cause: coarse line instrumentation vs. exact-line reachability

Two facts combine into the bug.

1. **Coverage instrumentation is coarse** (`crates/zero-test-runner/src/coverage.rs`,
   `InstrumenterVisitor`). It injects a *line* counter only for:
   - each **top-level module item**'s own line (`visit_mut_module`), and
   - each function body's **first statement** line (`instrument_body`),
     plus a per-function counter in the separate `fns` map.

   Nested blocks (`if` / `try` / `for` bodies) get **no** per-statement
   line counters — `visit_mut_block_stmt` only recurses. So the recorded
   `lines` universe for `transactions.ts` is roughly `{13, 14, 21, 23, 24}`
   (decl lines + function-entry lines), not the body lines.

2. **Mutate's reachability filter is exact-line**
   (`crates/zero-test-runner/src/mutate.rs`, `MutateVisitor::check`):
   a site is kept only if `covered_lines.contains(&site.line)`; a miss
   increments `skipped_unreachable`. The `covered_lines` set comes from
   `crates/zero/src/cmd/mutate.rs::run_baseline` →
   `merge_covered_lines`, which reads **only the `lines` half** of the
   `__zero_coverage__` snapshot (the `fns` half is discarded).

`transactionsQueryString`'s mutable tokens live on lines **15–18** (inside
the `if` statements), below the function's first body statement (line 14).
None are on an instrumented line, so every site fails
`covered_lines.contains(line)` and is filed "unreachable."

### Proof it is granularity, not the re-export

`theme.ts`'s 7 *killed* sites are all on lines `3`, `4`, and `42`
(`zero mutate web/src/lib/theme.ts`, non-quiet):

- lines 3, 4 — top-level `const STORAGE_KEY` / `const ATTR` (top-level item
  lines → instrumented), and
- line 42 — the **first** body statement of `toggleTheme`
  (`const next = getTheme() === "dark" ? ...` → function-entry line →
  instrumented).

They pass the filter purely by sitting on instrumented lines. Meanwhile
`theme.ts`'s own `getTheme` (line 33: `return current === "light" ? ...`)
and `read` (line 9) literals are executed by the tests (`toggleTheme`
calls `getTheme`; `initTheme` calls `read`) but sit on *non-first* body
lines — so they land in `theme.ts`'s **11 "unreachable"** bucket. The bug
is therefore pervasive: every multi-statement function body silently drops
mutants below its first line. `transactions.ts` is just the extreme case
where *zero* sites happen to align with an instrumented line, collapsing
the score to a fully-vacuous 100%.

### Second manifestation: transitively-tested component (#64)

`src/components/StatCard.ts` is a single multi-line `return html\`…\``
statement (starts line 9, spans through ~line 16). Its `cond_neg` site is
the ternary condition `props.sub ?` on **line 12** — a continuation line of
that one statement. It has no sibling `StatCard.test.ts`; it is rendered
(both the sub-present and sub-absent branches) by `home.test.ts`.
Reproduction:

```
$ zero mutate --quiet web/src/components/StatCard.ts
  Generated: 0 mutants ...
  Skipped:   1  [unreachable: 1, ...]      cond_neg: matched 1, unreachable 1
```

Two findings confirm this is the same root cause, not a linkage gap:

1. **Transitive coverage is credited.** `coverage.json` records
   `StatCard.ts` at `fns 1/1, lines 2/2` (100%) purely from `home.test.ts`
   — `run_baseline` runs *every* test file with coverage and
   `merge_covered_lines` accumulates across all of them, so a file needs no
   sibling test to be credited. The friction-log claim that mutate "only
   credits a file's own sibling test" is therefore false; `src_to_tests`
   likewise records `home.test.ts` as a loader of `StatCard.ts`.
2. **The dropped site is a continuation-line site.** Line 12 is not the
   `return` statement's start line (9), so it is absent from the
   (already coarse) covered-line set and filed `unreachable` — exactly the
   multi-line-statement case that R3's enclosing-statement attribution
   targets. StatCard is the concrete reason R3 is required, not optional.

### Secondary bug this also fixes: `--coverage` is dishonest

Because only function-entry and top-level lines carry counters,
`zero test --coverage` reports `transactions.ts` as "5/5 lines = 100%"
when most executable lines were never individually counted. The line
metric overstates real coverage. Making instrumentation per-statement
(R2) makes that number mean what users assume it means; this is a feature,
not collateral damage.

### Why per-statement instrumentation alone is not sufficient

A counter at each statement's **start** line fixes the multi-statement
*function body* case (each `if` is its own statement → its start line is
counted, and the site sits on that same line). It does **not** cover a
mutation site on a *continuation* line of a single multi-line statement —
e.g. `page: 1` on its own line inside

```ts
export const DEFAULT_TRANSACTIONS_QUERY = {   // statement starts here (counted)
  type: null,
  page: 1,                                    // lit_num site here — NOT the start line
};
```

or the `Intl.DateTimeFormat(undefined, { month: "short", ... })`
literals on lines 29–32 of `transactions.ts`. The robust fix is to check
reachability against the site's **enclosing executed statement**, keyed by
that statement's start line (which per-statement instrumentation now
records), rather than the site's own line. See R3.

### Approach chosen (and the one rejected)

- **Chosen — fix the source of the coarseness (Option B, done fully):**
  per-statement line instrumentation in `coverage.rs` (R2) **plus**
  enclosing-statement attribution in mutate's reachability check (R3).
  Coverage and mutate then agree on "what executed" by construction; no
  long-lived coupling between two visitors' naming schemes.
- **Rejected — function-granular reachability (Option A):** mutate maps
  each site to its enclosing *function* and consults the `fns` half of the
  snapshot. It is a smaller diff and leaves `--coverage` numbers untouched,
  but (a) it preserves the dishonest line metric, (b) it papers over the
  coarse signal for the one consumer instead of curing it, (c) it leaves a
  residual gap on multi-line statements, and (d) it requires the coverage
  and mutation visitors to forever agree on `anon@<line>` function naming.
  Decided against in scoping.
- **Rejected — mutate-internal fine instrumentation only (hybrid):** run a
  second, finer instrumentation pass used solely for mutate's reachability
  while leaving `--coverage` coarse. Buys nothing once honest coverage
  numbers are accepted as desirable, and doubles the instrumentation
  surface to maintain.

### Intentional precision boundary

The fix targets *line/statement*-level reachability, not *branch*-level. If
a ternary or short-circuit on a line executes at all, that line (its
enclosing statement) reads covered even when one branch never ran — so a
mutant on the un-taken branch is classified reachable, runs, and (correctly)
**survives**, lowering the score. This is the right behavior: the
`unreachable` bucket exists only to skip code no test executes; the test
verdict decides killed vs. survived for everything that runs. Branch- or
column-level coverage is explicitly out of scope (see Out of Scope).

### Code map (files this touches)

- `crates/zero-test-runner/src/coverage.rs` — `InstrumenterVisitor`:
  per-statement line counters; reconcile with the existing first-stmt /
  top-level counters so lines are not double-injected. Existing
  instrumenter + aggregator unit tests update to the finer line universe.
- `crates/zero-test-runner/src/mutate.rs` — `MutateVisitor`: track the
  enclosing-statement start line while walking; `check` consults it.
  `GenerateOptions.covered_lines` semantics unchanged (still a
  `HashSet<u32>`), but it is now complete.
- `crates/zero/src/cmd/mutate.rs` — no logic change required to
  `merge_covered_lines` (it already ingests `lines`); add/extend tests for
  the multi-statement-body and multi-line-statement shapes end to end.
- `docs/config-and-cli.md`, `docs/testing.md` — coverage granularity and
  the `zero mutate` "Reading `Generated: 0`" / unreachable wording.
- `~/Documents/code/zero_demo/FRAMEWORK_NOTES.md` — flip #61 **and #64** to
  fixed with correction notes (both hypotheses — re-export and
  sibling-only linkage — were wrong).

### Reference: prior, adjacent work

`issues/mutate-equivalence/spec.md` and `issues/mutate-operators/spec.md`
established the `matched / unreachable / equivalent-byte / equivalent-static
/ killed / survived / errored` accounting, the per-operator breakdown, and
`mutation.json` schema v2. This spec does **not** change that accounting or
the JSON schema — it only changes which sites land in `unreachable`
vs. become real, executed mutants. Both prior specs followed a
diagnose-then-fix discipline (write a failing fixture first); R1 mirrors it.

## Requirements

### R1 — Diagnose-then-fix: failing fixture first

Before any instrumenter change, add a test (in `cmd/mutate.rs` tests,
alongside `mutate_reclassifies_static_equivalents_end_to_end`) that builds
a synthetic project mirroring the demo's `transactionsQueryString` shape:
a multi-statement exported function whose mutable tokens are on lines
*below* its first body statement, with a sibling test that calls it across
several inputs. Assert that **under today's code** the run yields
`generated == 0` and `skipped_unreachable > 0` (the bug), then — once R2/R3
land — that the same fixture yields executed mutants and a non-vacuous
result. The negative direction stays as a regression guard.

### R2 — Per-statement line instrumentation

`InstrumenterVisitor` (`coverage.rs`) records a line counter for **every
executable statement**, not only top-level items and function-entry first
statements. Concretely, the recorded `lines` universe for a file must
include the start line of each statement that executes, including
statements nested inside `if` / `else` / `try` / `catch` / `for` / `while`
/ block bodies.

- The existing function-entry first-statement counter and top-level item
  counters must be **reconciled**, not stacked: a given source line gets at
  most one counter, and the `lines` `BTreeSet` stays free of duplicates
  (the `is_idempotent_within_one_module` invariant holds).
- Single-statement (brace-less) bodies (`if (x) doThing();`), `switch`
  case statement lists, and loop bodies without a block must also be
  covered, so sites inside them are reachable. (See Open Questions for the
  exact node set.)
- The per-function `fns` map and `fn_counter` behavior are unchanged.
- JS and TS inputs both work; the pass must not panic on either, and must
  remain a no-op-preserving transform (instrumented code behaves
  identically modulo the counter side effects).

### R3 — Reachability keyed on the enclosing executed statement

`MutateVisitor` (`mutate.rs`) attributes each mutation site to the start
line of its **innermost enclosing statement** (tracked as the visitor
descends), and `check` tests reachability against that line:

```
reachable(site) = covered_lines.contains(enclosing_stmt_start_line(site))
```

- For a site that sits on its enclosing statement's own start line (the
  common function-body case), this is equivalent to today's `site.line`
  check against the now-complete covered set.
- For a site on a continuation line of a multi-line statement (multi-line
  object literal, multi-line call args, multi-line ternary), this maps it
  back to the counted start line, closing the residual gap.
- Top-level sites (no enclosing function) already resolve to a top-level
  statement whose start line is instrumented, so they need no special case.
- `skipped_unreachable` / per-operator `unreachable` are still incremented
  on a miss; the bucket's *meaning* tightens to "the enclosing statement
  never executed in any baseline test," which is the honest definition.

### R4 — Acceptance on the demo shapes

End-to-end tests in `cmd/mutate.rs` (in-memory tempdir, no external deps)
covering both shapes:

1. **Multi-statement function body** (the `transactionsQueryString`
   reproduction from R1): with a sibling test that exercises it, the run
   produces executed mutants (`killed + survived + errored > 0`), and the
   `lit_str` / `cmp` / `lit_num` sites that were previously `unreachable`
   are now reached. A weak test (calls the function but asserts nothing)
   must surface **survived** mutants — proving the tool now detects the
   vacuous-test case it currently hides.
2. **Multi-line statement, transitively tested** (the `StatCard.ts` /
   entry-#64 shape): a `cond_neg` (or literal) site on a continuation line
   of an executed multi-line `return html\`…\`` statement, in a file with
   **no sibling test** that is rendered only through another file's test,
   is reached and executed — not skipped. The test must also assert that
   the file is credited via transitive coverage (no sibling `.test.ts`
   required), so a future change cannot "fix" this by narrowing linkage to
   siblings.
3. **Genuinely unreached code** (a function no test calls) still reports
   its sites as `unreachable` — the bucket must not collapse to zero.

The spec's headline acceptance condition: running mutate on a
`transactions.ts`-equivalent fixture no longer yields a vacuous
`Generated: 0 / score 100%`.

### R5 — `--coverage` reflects the finer granularity, tests updated

`zero test --coverage` line counts now reflect per-statement
instrumentation. Update the affected `coverage.rs` unit tests (e.g.
`coverage_map_contains_all_known_lines_and_fns_zero_initialized`, the
aggregator totals tests) and any in-repo fixtures/snapshots that assert
specific line numbers. The aggregator's JSON/terminal *shapes* are
unchanged; only the numbers move. No change to `coverage.json`'s schema.

### R6 — Docs

- `docs/config-and-cli.md`:
  - The `zero mutate` "Reading `Generated: 0`" section's second bullet
    ("All matches on uncovered lines … no baseline test exercises those
    lines") is reworded: after this fix, `unreachable` means the enclosing
    statement never executed in any baseline test — not "the line lacks a
    counter." Remove the implication that fully-tested code can read as
    unreachable.
  - The `zero test --coverage` row / any prose describing the line metric
    is updated to note it is per-executable-statement.
- `docs/testing.md`: if it characterizes coverage granularity, align it
  with per-statement instrumentation.
- No `mutation.json` schema/version change (still v2); call that out so a
  reader does not expect one.

### R7 — Friction-log correction

Flip **both #61 and #64** in
`~/Documents/code/zero_demo/FRAMEWORK_NOTES.md` from `- [ ]` to `- [x]` with
`**FIXED YYYY-MM-DD** (#PR / commit SHA):` annotations that **also record
the corrected diagnosis**:

- #61 — the re-export was a red herring; the cause was coarse
  (function-entry-only) coverage instrumentation versus an exact-line
  reachability filter, fixed by per-statement instrumentation plus
  enclosing-statement attribution.
- #64 — the "linkage only credits a file's own sibling test" hypothesis was
  wrong; transitive coverage *is* credited (`StatCard.ts` reads 100% from
  `home.test.ts`). It is the same continuation-line-of-a-multi-line-
  statement bug as #61, closed by the same fix.

Preserve the discovered ground truth, per the file's convention (mirrors
how #46 was annotated). Leave #63 and #65 untouched.

## Constraints

- **No new dependencies** (Rust or JS). The change is within the existing
  SWC instrumenter and the existing mutate visitor.
- **No `mutation.json` schema change**, no change to the
  `matched/unreachable/equivalent-*/killed/survived/errored` accounting
  model from `issues/mutate-equivalence`. Only the partition between
  `unreachable` and executed shifts.
- **Monotonic on reachability**: the change may only move sites from
  `unreachable` to executed (because the covered-line set only grows and
  the enclosing-statement line is ≤ the site line). No site that mutates
  correctly today may become unreachable.
- **80-line function guideline** (CLAUDE.md). The instrumenter's
  statement-walking logic and the visitor's enclosing-statement tracking
  should be factored into helpers rather than inlined into existing large
  methods.
- **Instrumented code must stay behavior-preserving** and idempotent within
  a module (no duplicate counters for one line); the existing
  `is_idempotent_within_one_module` test must still hold.
- **JS and TS** inputs both supported; no panic on either; TS type-only
  constructs (already stripped before the instrumenter runs) must not
  produce phantom covered lines.
- Coverage is a **signal, not a gate** (CLAUDE.md), so shifting numbers
  across the demo/examples is acceptable churn and breaks no CI threshold;
  still, update every in-repo assertion that names a specific count.
- The slow integration tests (`#[ignore = "slow"]`) that exercise
  build/test/coverage flows must pass under `--include-ignored` after the
  numbers move.
- **Do not regress the Boa GC teardown surface (friction-log #63).** R2
  increases instrumentation density (more `__c.lines[N]++` statements,
  larger instrumented AST), and #63 reports a deterministic
  `<Boa GC panic during teardown> BorrowMutError` on the demo's largest
  instrumented test file (`parts.test.ts`) under the full suite — which, in
  the author's environment, aborts the mutate baseline project-wide. It did
  **not** reproduce here (the full instrumented suite ran `parts.test.ts`
  clean), but the plan must confirm per-statement instrumentation does not
  make it appear or worsen, and must not assume the demo baseline always
  passes. #63 is a separate issue (different root cause); this spec only
  requires that its fix not aggravate it.

## Out of Scope

- **Branch- or column-level coverage.** Reachability stays
  line/statement-grained; un-taken branches on an executed line are
  reachable-and-survivable by design (see Background → precision boundary).
- **Function-granular reachability via the `fns` map** (the rejected
  Option A) and the **hybrid** dual-instrumentation approach.
- **New mutation operators**, changes to default operator selection, or any
  `--threads` / equivalence-detection behavior (owned by
  `issues/mutate-operators` and `issues/mutate-equivalence`).
- **`mutation.json` schema changes** or new summary buckets.
- **Reducing the *volume* of unreachable mutants** in genuinely untested
  files — that is a user-project test-coverage task, not a framework
  concern (same stance as the prior mutate specs).
- The other open friction-log entries: #58 `Intl`, #59 descendant
  combinator, #60 `http.get` content-type, #62 `documentElement`, **#63 the
  Boa GC teardown panic under instrumentation** (a distinct runtime fault —
  own issue; only its *non-aggravation* is a constraint here), and **#65 the
  `zero dev` proxy query-string drop** (unrelated subsystem).

## Open Questions

- **Exact statement node set for R2.** Which SWC `Stmt` variants get a
  counter, and how brace-less bodies (`IfStmt.cons`/`alt` as a non-block
  `Stmt`), `SwitchCase.cons` lists, labeled statements, and `for`/`while`
  bodies are handled. The plan should enumerate these against the demo +
  `examples/` to confirm no executable line is missed and none is
  double-counted. Arrow expression bodies (no block) already have a
  special path — confirm it composes.
- **Reconciling existing counters.** Whether to delete the current
  first-body-statement special case in `instrument_body` and the top-level
  per-item counter in `visit_mut_module` in favor of a single uniform
  per-statement pass, or to keep them and dedupe via the `BTreeSet`. The
  plan should pick whichever keeps the instrumenter simplest while
  preserving the `fns` counters and idempotency.
- **Enclosing-statement tracking in the mutation visitor (R3).** Confirm
  the cheapest mechanism — e.g. a `visit_mut_stmt` override that pushes the
  current statement's start line onto a stack the literal/operator checks
  read — and that it yields the correct start line for sites deep inside
  expression trees. If statement tracking proves heavy, fall back to
  per-statement instrumentation only (R2) and accept the multi-line-
  statement residual as a documented, narrower gap (this still fixes the
  reported `transactionsQueryString` bug); R4 case 2 would then move to a
  follow-up.
- **Blast radius of the coverage number changes.** The plan should run
  `zero test --coverage` across the demo and `examples/` to inventory which
  asserted numbers move, before editing, so the test/doc churn is a known,
  bounded list rather than discovered piecemeal.
- **Performance.** Per-statement counters enlarge the instrumented AST and
  add runtime increments. Confirm the baseline coverage run and the
  per-mutant worker runs don't regress materially on the demo; if they do,
  consider counting once per line rather than once per statement on a line.
- **Interaction with the Boa GC teardown panic (#63).** Whether the denser
  instrumentation measurably raises the teardown-panic likelihood on large
  files. The plan should run the full instrumented demo suite before/after
  R2 on a few iterations. If R2 does aggravate #63, sequence it behind that
  fix or coordinate the two; counting once-per-line (fewer counter
  statements than once-per-statement) is a mitigation lever.
