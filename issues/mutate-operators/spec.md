# Spec: `zero mutate` — Operator Error Message & "Generated: 0" Diagnostics

## Problem Statement

Two `zero mutate` issues surfaced while building a real app on top of zero
(see `~/Documents/code/zero_demo/FRAMEWORK_NOTES.md`):

1. **🔴 `--operators` error lists IDs in the wrong case.** When the user
   passes an unknown operator id (e.g. `--operators help` or any typo), the
   command errors with `expected one of [Arith, Cmp, Bool, CondNeg,
   Boundary, LitBool, LitNum, LitStr]`. The CLI only accepts the snake_case
   forms (`arith`, `cmp`, `bool`, `cond_neg`, `boundary`, `lit_bool`,
   `lit_num`, `lit_str`). A user following the error message lands straight
   in a second error. The cause is `parse_operators` formatting
   `Operator::ALL` with `{:?}`, which prints the Rust enum variants by
   their Debug name, not by their accepted id.

2. **🟡 `--operators arith` / `--operators boundary` report "Generated: 0
   mutants" on a codebase that obviously has arithmetic and boundary
   comparisons.** The demo's `src/` contains `Math.ceil(tc / PAGE_SIZE)`,
   `onHand / denom`, `SlotsUsed * 100 / SlotsTotal`, `onHand <=
   ReorderPoint`, `tc <= PAGE_SIZE`, range guards — yet running
   `zero mutate --operators arith` (or `boundary`) emits `Generated: 0
   mutants`. The user inferred "operator unimplemented" from the output.
   Reading the source, both operators *are* implemented and have passing
   unit tests; the most likely explanation is that every collected site
   lives on a line the baseline tests don't cover, so the coverage filter
   in `MutateVisitor::check` tallies them into `skipped_unreachable`
   instead of `summary.generated`. The friction-log inference was wrong,
   but the *report* genuinely doesn't tell the user what happened. Same
   shape as `survived` vs `equivalent` (see friction-log entry #48): the
   summary collapses meaningfully different states into one bucket.

Both bugs live in `zero mutate`'s CLI surface and reporter: one is a
formatting fix in `parse_operators`, the other is a diagnostics task in
the visitor + summary that splits "Generated: 0" into actionable
sub-states. Both share the same CLI surface and the same user
journey ("I picked an operator and got an unhelpful answer"), so they
fit one spec.

## Background

### Where the wrong-case error comes from

`crates/zero/src/cmd/mutate.rs:95-118`:

```rust
fn parse_operators(spec: Option<&str>) -> anyhow::Result<Vec<Operator>> {
    match spec {
        None => Ok(Operator::ALL.to_vec()),
        Some(s) => {
            let mut out = Vec::new();
            for id in s.split(',').map(|p| p.trim()).filter(|p| !p.is_empty()) {
                let op = Operator::parse(id).ok_or_else(|| {
                    anyhow::anyhow!(
                        "unknown operator id: {id:?}; expected one of {:?}",
                        Operator::ALL
                    )
                })?;
                ...
```

`Operator::ALL` is a `&[Operator]`, and `Operator` derives `Debug`. The
`{:?}` formatter therefore prints each variant by its Rust identifier
(`Arith`, `Cmp`, …). The CLI parses via `Operator::parse` (`mutate.rs:82`)
which calls `id()` — the snake_case form. The error message and the
acceptor disagree.

The existing test (`parse_operators_rejects_unknown` at `mutate.rs:899`)
only asserts the unknown id is echoed in the error; it doesn't constrain
the format of the listed valid ids.

### Where the "Generated: 0" ambiguity lives

`crates/zero-test-runner/src/mutate.rs:414-457` (`MutateVisitor::check`)
in Collect mode increments one of three counters per visited candidate:

- Filtered by `operators_filter` → silently dropped, no tally.
- Filtered by `covered_lines` → `skipped_unreachable += 1`, dropped from
  the returned `Vec`.
- Accepted → site pushed onto `self.sites`, then later either pre-applied
  to the queue or counted as `skipped_equivalent` (if the mutated JS
  byte-matches the baseline).

`summary.generated` is incremented only in two places (in
`crates/zero/src/cmd/mutate.rs`):

- `pre_apply_to_queue` (`mutate.rs:516-546`) when `apply` errors — counts
  the site as `errored` and bumps `generated`.
- `consume_mutant_results` (`mutate.rs:550-591`) once a worker returns a
  verdict — bumps `generated` and the matching killed/survived/errored.

So a run of `--operators arith` where every arith expression lives on an
uncovered line produces `generated = 0`. The summary's "Generated: 0
mutants across 0 files" reads identically to "this operator doesn't exist
in the codebase" — and identically to "this operator isn't implemented."

The terminal summary (`mutate.rs:212-284`) already has a `Skipped:
[unreachable: N, equivalent: M]` row, but it's a single global tally. It
doesn't break down per operator, and it doesn't appear *before* "Generated:
0" — by the time the reader sees the unreachable count, they've already
read "Generated: 0" and drawn the wrong conclusion.

### Existing operator coverage in the visitor

`MutateVisitor` already maintains a per-operator counter for the apply
pass (`counts: [usize; 8]` at `mutate.rs:353`). It's used to find the Nth
same-operator site in a re-walk. We can extend the visitor's collect-mode
output to also report per-operator match totals without changing the apply
contract.

### Adjacent surfaces touched

- **`crates/zero/src/cmd/mutate.rs`** — `parse_operators`, the
  `MutationSummary` struct, `consume_mutant_results`,
  `write_terminal_summary`, `write_mutation_json`, `run_inner`.
- **`crates/zero-test-runner/src/mutate.rs`** — the visitor's
  collect-mode return shape, the `Operator::ALL` formatting helper.
- **`crates/zero/src/cmd/mutate.rs` tests** — `parses_operator_filter_csv`
  and `parse_operators_rejects_unknown` already exist; add coverage for
  the new error format and the per-operator tally.
- **`docs/config-and-cli.md`** — the `zero mutate` subcommand reference;
  update if the summary block grows new lines so the docs match the
  CLI output.

### Design context (decided in scoping)

- **Diagnose before fixing the generator.** Existing visitor unit tests
  (`arith_operator_generates_swap`, `boundary_swaps_lt_to_lte`) pass.
  The first work-item is to confirm whether the demo's "Generated: 0"
  case is coverage-filter-only by running the generator with
  `covered_lines = None` over the demo's `src/`. If sites are produced,
  the fix is reporter-side (this spec). If sites are *not* produced —
  meaning the unit-test fixtures don't represent real-world TS shapes —
  the spec adds a fixture-based regression test mirroring the demo's
  expressions (e.g. `Math.ceil(tc / PAGE_SIZE)`) before fixing the
  generator. Both outcomes are in scope; the path is decided after
  diagnosis.
- **Per-operator sub-totals are the right reporting surface.** A single
  global `Skipped` row doesn't help when the user filters by one
  operator. Per-operator counts (matched / executed / unreachable /
  equivalent) appear in the summary when an `--operators` filter is
  active, and always appear in `mutation.json`.
- **Error message lists the accepted ids, not Rust Debug.** Use the
  snake_case `id()` strings comma-separated, matching exactly what the
  parser accepts. No "Did you mean?" inference — keep the change minimal.

## Requirements

### R1 — `parse_operators` error message uses accepted ids

`parse_operators` in `crates/zero/src/cmd/mutate.rs` formats the
`expected one of …` list as the comma-separated `Operator::id()` strings,
not the Rust Debug names. The error for `--operators help` reads
substantially:

```
unknown operator id: "help"; expected one of arith, cmp, bool, cond_neg, boundary, lit_bool, lit_num, lit_str
```

Bracketing / separator style is a planner choice as long as every listed
token is parseable by `Operator::parse` verbatim. (Optional: extract a
`Operator::list_ids() -> String` helper on the enum so the same string is
reused anywhere the canonical list appears — `--help` summary, error
message, docs generator.)

### R2 — Visitor reports per-operator match totals

`MutateVisitor` in `crates/zero-test-runner/src/mutate.rs` extends its
collect-mode result so callers can read, per `Operator` variant, the
counts of:

- **Matched**: AST nodes that the operator's swap accepted (pre-coverage,
  pre-equivalence). For `arith` this is "binary expressions whose op is
  `+`/`-`/`*`/`/`/`%`, excluding string concatenation." For `boundary`
  this is "binary expressions whose op is `<`/`<=`/`>`/`>=`."
- **Skipped unreachable (per-operator)**: matched sites dropped by the
  coverage filter.

The shape (a `BTreeMap<Operator, OperatorTally>` on the visitor result, or
two `[usize; 8]` arrays alongside the existing `counts`, or similar) is a
planner choice. Existing visitor unit tests must keep working; the new
fields are additive.

`generate()` returns the per-operator tally alongside `(sites,
skipped_unreachable)`; the existing tuple shape gets a new field or is
replaced with a small named struct. Internal-only API (`@internal` in
the JS sense), so callers can be updated wholesale.

### R3 — `MutationSummary` carries per-operator tallies

`MutationSummary` in `crates/zero/src/cmd/mutate.rs` gains a per-operator
breakdown that aggregates the visitor tallies plus the verdict
distribution from `consume_mutant_results`. Per `Operator`:

- `matched`: total AST sites the operator hit across all source files.
- `executed`: of those, how many actually ran in a worker (killed +
  survived + errored).
- `unreachable`: matched but filtered by coverage.
- `equivalent`: matched, reached, but mutated JS byte-matched the
  baseline.
- `killed` / `survived` / `errored`: verdicts from `consume_mutant_results`.

`generated = killed + survived + errored` per operator. The global
`generated` field on `MutationSummary` stays the sum across operators —
the global tally is unchanged for backwards compatibility.

### R4 — Terminal summary surfaces the per-operator breakdown

`write_terminal_summary` in `crates/zero/src/cmd/mutate.rs` is updated:

- When **`--operators` is set** (any subset of operators), the summary
  prints a per-operator block immediately after the headline counts. For
  each operator in the filter, one row showing `matched / executed /
  unreachable / equivalent / killed / survived / errored`.
- When **`--operators` is unset** (all operators), the per-operator block
  prints only for operators with `matched > 0 && executed == 0` (i.e. the
  operators most likely to confuse the reader). This avoids drowning the
  default-run output in eight rows when seven of them have no signal.
- When `matched == 0` for a filter-selected operator, the row reads
  `arith: 0 matches in src/` (or equivalent prose) so the user immediately
  sees "the operator is implemented but found nothing" rather than
  inferring "the operator is broken."

Exact rendering is a planner choice; the requirement is that running
`zero mutate --operators arith` on the demo produces an output where the
distinction between "no matches in code" / "matches all unreachable" /
"matches all equivalent" is unambiguous in the first screen.

### R5 — `mutation.json` carries the per-operator breakdown

`write_mutation_json` adds an `operators` object alongside `totals` and
`files`, keyed by operator id, with the fields from R3. Downstream
tooling (CI checks, dashboards) can consume the JSON without re-parsing
the per-mutant `outcomes` list.

### R6 — Diagnose-then-fix on arith / boundary

Before changing any generator code, run the existing visitor over the
demo's `src/` with `covered_lines = None` and `operators = [Arith]` (and
again with `[Boundary]`) and record the matched count. Two branches:

- **Branch A — sites are produced.** This is the expected outcome. R2–R5
  alone close the friction-log entry — the "Generated: 0" output now
  reads `arith: matched 12, unreachable 12 (no test reaches these
  lines)`. No generator change. Add a regression test capturing the demo
  expressions (`Math.ceil(a / b)`, `onHand <= reorderPoint`,
  `slotsUsed * 100 / slotsTotal`) and asserting they each produce at
  least one site of the expected operator.
- **Branch B — sites are *not* produced.** Fix the visitor to handle the
  shape the demo uses. Add the demo-mirroring fixture from Branch A as
  a failing test first; make it pass. Likely culprits if this branch
  fires: parenthesized binary expressions, type-strip dropping spans the
  visitor relies on, or a parse-mode mismatch (the visitor uses
  `tsx: false`; the demo has no TSX so this should be fine — verify).

The plan phase records the diagnosis outcome and proceeds down one
branch.

### R7 — Tests

`crates/zero/src/cmd/mutate.rs` tests:

- `parse_operators_error_lists_accepted_ids`: assert the error string
  contains every `Operator::id()` value and contains *no* Debug-cased
  variant name (`Arith`, `Cmp`, …). Specifically assert each of the 8
  snake_case ids is present.
- `summary_per_operator_matches_visitor`: end-to-end test that builds a
  project with both arith and boundary expressions on uncovered lines,
  runs `run_inner`, and asserts the per-operator tally shows
  `matched > 0` and `unreachable == matched` for both operators.
- `summary_per_operator_executed_on_covered_lines`: same project but with
  tests that cover both expressions; the tally now shows `executed > 0`.

`crates/zero-test-runner/src/mutate.rs` tests:

- `visitor_reports_per_operator_match_counts`: parse a fixture with one
  arith and two boundary expressions, assert the visitor's tally has
  `arith: matched 1, boundary: matched 2`.
- The Branch A regression test from R6 (added regardless of which branch
  fires): the demo-mirroring fixture file containing `Math.ceil(a / b)`,
  `onHand <= reorderPoint`, etc., with assertions on the expected site
  counts.

### R8 — Docs

`docs/config-and-cli.md` (`zero mutate` reference):

- Update the example output block (if one exists) to show the new
  per-operator rows on a filtered run.
- Add a one-paragraph "Reading 'Generated: 0'" note: explain that an
  operator with zero generated mutants may mean the codebase has no
  matching AST nodes, or that all matches sit on lines no test reaches —
  the per-operator breakdown distinguishes the two.
- Confirm the operator-id list in the docs matches `Operator::ALL` in
  snake_case (it should already; verify during planning).

## Constraints

- No npm dependencies; same workspace dependencies as the rest of `zero`.
- `MutationSummary` is part of the `mutate` module's CLI-internal surface
  (`pub` but not stabilized); additive field changes are fine. `mutation.json`
  *is* a consumer-visible format — additions are backwards-compatible,
  but no existing field is renamed or removed.
- Performance: the new per-operator tally must not require a second AST
  pass. Hook it into the existing collect walk.
- The 80-line per-function guideline (CLAUDE.md) applies to any new
  function in this spec. `write_terminal_summary` is already long;
  factor the per-operator block into a helper if needed.
- Existing tests in both crates must still pass without modification
  (except where they directly assert on the visitor's return shape — those
  are updated to read the new struct/tuple).
- Error message format: the comma-separated list must be parseable by
  `Operator::parse` token-for-token. No fancy formatting (no "or" before
  the last token, no Oxford comma sneaking in a stray character).

## Out of Scope

- Distinguishing "equivalent" from "survived" verdicts on a per-mutant
  basis. That's friction-log entry #48 and a much larger change to the
  worker protocol — out of scope here. We only split *skipped*
  equivalence from *executed* survival, both of which are already
  tracked.
- "Did you mean?" suggestions in the parse error. Listing the accepted
  ids is enough; Levenshtein-style hints are a future polish.
- Adding new operator families (e.g. an `assignment` operator). The work
  is on the existing eight.
- Changing default operator selection. `--operators` unset still runs
  `Operator::ALL` exactly as today.
- Restructuring the global `Skipped` row. It stays; the per-operator
  block sits alongside it.
- Coverage-runtime changes. If the diagnosis under R6 reveals the
  coverage path-key mismatch hypothesis (covered_lines keyed by a path
  shape that doesn't match the walker's absolute paths), that's a
  separate bug worth filing — but fixing it is not in this spec's
  scope. R2–R5 still close the friction-log entry by making the
  failure mode legible.

## Open Questions

- **Visitor result shape.** The cleanest API replaces `generate()`'s
  `(Vec<MutationSite>, usize)` return with a named struct
  (`GenerateResult { sites, skipped_unreachable, per_operator }`) — but
  that touches every call site. The planner decides between (a) named
  struct, (b) adding a third tuple element, or (c) a side-channel
  populated through `GenerateOptions`. Option (a) is the recommended
  end-state.
- **Terminal output for default (no-filter) runs.** R4 says the
  per-operator block prints for operators with `matched > 0 &&
  executed == 0` when no filter is set. The planner confirms this rule
  doesn't double-print when a run has eight legitimately-empty operators
  (e.g. an empty `src/`). One edge to watch: `matched == 0` and
  `executed == 0` should not appear in the default block — the filter is
  "looks like signal got eaten," not "no signal at all."
- **`mutation.json` schema versioning.** No version field exists today.
  Additive changes are technically safe, but the planner should decide
  whether to introduce a `"schema_version": 1` field now for future-
  proofing.
- **R6 diagnosis evidence.** The plan phase must record the matched
  counts measured during the diagnosis step (per operator, on the demo's
  `src/`) in `issues/mutate-operators/plan.md` so reviewers can see why
  Branch A vs Branch B was chosen. Don't drop the evidence after the
  fork — it's the load-bearing reason for the chosen scope.
