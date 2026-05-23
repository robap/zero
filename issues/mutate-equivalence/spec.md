# Spec: `zero mutate` — Static equivalence detection, skipped subdivisions, and a sane `--threads` default

## Problem Statement

Four open `zero mutate` items in `~/Documents/code/zero_demo/FRAMEWORK_NOTES.md`
(entries #46, #47, #48, #50) describe a single underlying story: the tool's
classification of mutants is too coarse in two places, and its parallelism
default is too conservative.

1. **#48 (🟡, load-bearing) — Equivalent mutants land in `survived`.** A
   `zero mutate` run on the demo's `src/stores/parts.ts` produces 5
   "survived" mutants that are not real survivals: the four members of
   `PART_STATUSES = ["out", "critical", "needs-reorder", "in-stock"] as const`
   (each → `""`) and the `"loading"` property of
   `partsSignal = signal<PartsState>({ kind: "loading" })`. The first group
   is a TS-type carrier — no runtime code reads the array's string contents,
   so mutating "out" to "" has no observable effect. The second is always
   overwritten by `load()` before any read of `partsSignal.kind`. Both pad
   `survived` and the mutation score downward (37.5% on this file when the
   real number is 100% of *reachable, observable* mutants).

2. **#46 (🟡, collapses into #48) — Originally reported as "errors the test
   run."** Re-diagnosed on the current build: the literals do **not** error;
   they survive. The friction-log entry misnamed the bucket. The fix is the
   same as #48 — detect these literals statically and reclassify them out of
   `survived`. #46 is folded into #48 below; no separate work item.

3. **#47 (🟢) — Skipped sub-buckets need to name *why*.** The prior
   `mutate-operators` spec split `Skipped` into `unreachable` (coverage
   filter) and `equivalent` (post-apply byte-identical). That two-way split
   stands, but heuristic (1)/(2) above introduces a *third* form of skip:
   "the static analyzer proved no runtime effect." Folding it into the
   existing `equivalent` bucket loses the signal that distinguishes a tuned
   heuristic going stale from the unchanging byte-equality case. Subdivide
   `equivalent` so the diagnostic value of each sub-tally is preserved.

4. **#50 (🟡) — `--threads` defaults to 1.** Measured 2.5× speedup at
   `--threads 8` on a 12-core box (6s vs ~15s). Sequential is a bad default
   on any modern machine. Default to `min(num_cpus, 8)`; explicit
   `--threads N` always wins.

All four sit in the same code paths — `crates/zero-test-runner/src/mutate.rs`
(visitor) and `crates/zero/src/cmd/mutate.rs` (CLI / summary / json). One
spec, one PR.

## Background

### Diagnosis evidence (already collected)

Running `zero mutate src/stores/parts.ts --threads 8 --quiet` against
`~/Documents/code/zero_demo` produces:

```
Generated: 8 mutants across 1 files
  Killed:    3  (37.5%)
  Survived:  5  (62.5%)
  Errored:   0  (0.0%)
  Skipped:   4  [unreachable: 4, equivalent: 0]
```

`web/mutation/mutation.json` confirms the five "survived" mutants are:

| Site | Source | Bucket today | Spec target bucket |
|---|---|---|---|
| L3 col 31 | `"out"` in `PART_STATUSES = [..., ...] as const` | survived | equivalent-static |
| L3 col 38 | `"critical"` in same array | survived | equivalent-static |
| L3 col 50 | `"needs-reorder"` in same array | survived | equivalent-static |
| L3 col 67 | `"in-stock"` in same array | survived | equivalent-static |
| L56 col 55 | `"loading"` in `signal<PartsState>({ kind: "loading" })` | survived | equivalent-static |

The "killed" 3 (L59, L69, L73) are `partsSignal.set({ kind: "..." })` calls
whose values *are* read by tests. The "unreachable" 4 are coverage drops.
`Errored: 0`. This evidence is load-bearing for the spec — #46 as worded
does not reproduce.

### Current mutate code shape

- **Visitor:** `crates/zero-test-runner/src/mutate.rs` walks the AST. In
  Collect mode it tallies per-operator `matched` + `unreachable` counts and
  returns a `Vec<MutationSite>`. The visitor decides whether a candidate
  becomes a site via `check()` (operator filter + coverage filter).
- **Apply:** `crates/zero/src/cmd/mutate.rs::pre_apply_to_queue` runs each
  site through `apply` (re-walks the AST and emits mutated source). If the
  emitted JS byte-matches the baseline, the site is counted as
  `skipped_equivalent` (the *existing* equivalent bucket). Otherwise it's
  enqueued for a worker.
- **Dispatch:** workers run the test suite against the mutated build;
  results come back as Killed/Survived/Errored. `consume_mutant_results`
  tallies them.
- **Summary:** `MutationSummary` holds globals (`killed`, `survived`,
  `errored`, `skipped_equivalent`, etc.) plus a `per_operator` block
  (`matched`, `unreachable`, `equivalent`, `killed`, `survived`, `errored`
  per operator). `write_terminal_summary` emits the headline + per-operator
  block; `write_mutation_json` emits `mutation.json` with
  `schema_version: 1`.

### Where the heuristic fits

The cleanest insertion point is **the visitor's Collect-mode check, before
`covered_lines` filtering**. If the visitor recognizes a literal as a
static-equivalent shape (rules R1.a and R1.b below), the site is dropped
from `self.sites` and tallied into a new `skipped_static_equivalent`
per-operator counter — exactly parallel to how `unreachable` is tallied
today. The Apply path is untouched; the existing `skipped_equivalent`
(byte-equality) bucket is renamed conceptually to `equivalent-byte` but
keeps its existing tally semantics.

This placement means:
- Worker dispatch is unaffected (the mutants never enter the queue).
- `mutation.json` `mutants[]` array doesn't grow phantom entries.
- The new bucket appears in both the global skipped row and per-operator
  rows.

### Why heuristic (a) and not full flow analysis

Heuristic-(a) — "conservative AST patterns, two specific shapes" — was
picked over a general "no observable read" analysis (e.g. reaching
definitions across modules) because:
- The two shapes account for both demo failure cases verbatim.
- The visitor can decide each in O(local AST nodes), no cross-file
  graph build.
- False positives are bounded by syntax shape, not by the precision of an
  analyzer. A regression shows up cleanly in `equivalent-static` count.
- A full analysis is a research project and out of scope.

### `--threads` default today

`crates/zero/src/main.rs:53` — `#[arg(long, default_value_t = 1)]`.
Plumbed into `cmd::mutate::run` → `run_inner` → `dispatch_parallel`. The
sequential path (`threads <= 1`) takes a different branch in `run_inner`
and is also the only path that works with `InProcess` isolation. The
default change must not break the in-process path.

### Adjacent surfaces touched

- `crates/zero-test-runner/src/mutate.rs` — visitor: new heuristic check,
  new per-operator tally for static-equivalents, new field on the
  generate-result struct.
- `crates/zero/src/cmd/mutate.rs` — `MutationSummary` gains a global
  `skipped_static_equivalent` and a per-operator `equivalent_static`
  field; existing `skipped_equivalent` is renamed to
  `skipped_equivalent_byte` (in-source identifier only; JSON name handled
  in R5).
- `crates/zero/src/main.rs` — `--threads` default expression.
- `crates/zero/src/cmd/mutate.rs` tests — new tests for heuristic
  detection (Branch A) and the threads default.
- `docs/config-and-cli.md` — `zero mutate` subcommand reference: new
  `--threads` default, the two new `Skipped` sub-labels, the per-operator
  row gaining a column.

### Design context (decided in scoping)

- **#46 collapses into #48.** Confirmed by fresh repro: the literals
  survive, they don't error. No separate work item for #46. The
  friction-log entry stays open with a `PARTIAL` annotation that points
  to this spec for the real fix.
- **Heuristic (a) only, not (b).** Two specific AST shapes — see R1.
- **Subdivide `equivalent` into `equivalent-byte` and `equivalent-static`
  (Option 2 from refine).** Both terminal summary and JSON show both
  sub-totals.
- **`--threads` defaults to `min(num_cpus, 8)`.** Cap at 8 keeps headroom
  on bigger boxes for IDE/build noise; `min` keeps it ≥1 on tiny VMs.
- **Diagnose-then-fix on the heuristic.** Mirror the prior spec. Step 1 of
  the plan: write a failing visitor fixture for both AST shapes and
  confirm sites are produced under today's code. Step 2: implement
  detection; the fixture's sites should now route into the new
  `equivalent-static` bucket without entering the worker queue.

## Requirements

### R1 — Visitor recognises two static-equivalent shapes

In `crates/zero-test-runner/src/mutate.rs`, `MutateVisitor` gains a
detection pass that runs **before** the coverage filter inside `check()`
(or in a sibling pre-check called from each `visit_mut_lit` /
`visit_mut_*` path that can hit one of these shapes). A site that
matches either rule is dropped from `self.sites` and tallied as
"static-equivalent" instead of becoming a generated mutant.

**R1.a — Member of an `as const` array used only for type derivation.**
A `Lit::Str` (or `Lit::Num` / `Lit::Bool`) whose enclosing chain is
`<literal> ∈ <ArrayLit> as const`, and whose `as const` is the
right-hand side of a *module-level* `const <Name> = [...] as const`
declaration, is a static-equivalent site **if** the module contains no
runtime read of `<Name>` other than:
- a `typeof <Name>` expression (TS type position — stripped at runtime),
- an indexed type access like `(typeof <Name>)[number]` (likewise
  stripped),
- no other references at all.

The check is intentionally local to the file: a runtime read in the same
module (e.g. `for (const s of PART_STATUSES) ...`, or `PART_STATUSES[0]`,
or `PART_STATUSES.includes(x)`) disqualifies the literal from being
considered static-equivalent — even though such a read might be unreached
by tests, the rule must not infer beyond syntactic evidence.

**R1.b — Property literal in a module-level signal/state initializer
overwritten before read.** A `Lit::Str` (or `Lit::Num` / `Lit::Bool`)
that appears as a property value in an object literal passed to a
**module-level** call recognisable as a state-cell initializer
(`signal(...)`, `computed(...)` — see implementation note below), and
whose immediately enclosing binding is `const <Name> = signal({...})`,
is a static-equivalent site **if** every runtime read of `<Name>` (in
the same module) is dominated by at least one `<Name>.set({...})` call
that overwrites the same property. The check looks for:
- the binding's `<Name>.set({ ... <property>: ... })` calls (any number),
- and verifies that every other reference to `<Name>` in the module is
  ordered *after* at least one `.set()` in source order — a coarse
  source-order approximation of dominance that is safe for module-level
  code.

The full read-flow analysis is intentionally not attempted. This coarse
check accepts the demo's `partsSignal` (every read site is in a function
that always calls `load()` before reading, and `load()` calls `.set()`)
when the read is in the same module *after* a `.set()` in source order.
If a read appears in source order before any `.set()`, the literal is
**not** considered static-equivalent — the rule errs on the side of
generating the mutant.

Implementation note on "recognisable as a state-cell initializer": match
by *callee name* (`signal`, `computed`) without trying to resolve
imports. False positives from a user-defined `signal()` shadowing
`zero`'s are accepted as out-of-scope; they would still be valid
candidates for "no observable effect" detection in practice.

Operators in scope for R1: `lit_str`, `lit_num`, `lit_bool`. (The demo
cases are all `lit_str`; `lit_num`/`lit_bool` get the same treatment for
consistency — e.g. a numeric tag in a `kind: 1` initializer.)

### R2 — Visitor tally for static-equivalent

`MutateVisitor`'s collect-mode result gains a per-operator
`equivalent_static: [usize; 8]` counter alongside the existing `matched`
and `unreachable` per-operator counters. The visitor's
`GenerateResult` (or equivalent struct from the prior spec) carries it
out to the caller. `equivalent_static` is incremented exactly when a
site is dropped by an R1 rule. `matched` is still incremented for the
site (the operator did *match* an AST node; we're filtering after match,
just like coverage).

### R3 — `MutationSummary` carries the new sub-bucket

`MutationSummary` in `crates/zero/src/cmd/mutate.rs` adds:

- **Global**: `skipped_equivalent_static: usize` (new).
- The existing `skipped_equivalent` field is renamed in-source to
  `skipped_equivalent_byte` for clarity; the rename is internal — the
  semantics are unchanged.
- **Per-operator**: the existing `per_operator.equivalent: [usize; 8]`
  field is split into `per_operator.equivalent_byte` and
  `per_operator.equivalent_static`. The total equivalent count for an
  operator is the sum.

`MutationSummary` aggregation: `consume_mutant_results` is unchanged for
verdicts. The visitor-side counts are folded into the summary in the
existing `generate_all_sites` (or equivalent) function the same way
`unreachable` is folded today.

### R4 — Terminal summary subdivides skipped

`write_terminal_summary` in `crates/zero/src/cmd/mutate.rs` updates the
`Skipped` line:

Before:
```
  Skipped:   4  [unreachable: 4, equivalent: 0]
```

After:
```
  Skipped:   9  [unreachable: 4, equivalent-byte: 0, equivalent-static: 5]
```

(The 5 comes from the demo case under this spec.) Exact label spacing
and order are a planner choice; the requirement is that both sub-buckets
appear, with their distinct names, in the headline row.

**Per-operator row** (from the prior `mutate-operators` spec) gains the
same subdivision. The existing per-operator print:

```
  lit_str: matched 8, executed 3 (killed 3, survived 0, errored 0), unreachable 0, equivalent 5
```

becomes:

```
  lit_str: matched 8, executed 3 (killed 3, survived 0, errored 0), unreachable 0, equivalent-byte 0, equivalent-static 5
```

The demo's headline `Survived: 5 (62.5%)` becomes `Survived: 0` and the
score moves from 37.5% to 100% (3 killed / 3 executed) — the spec's
acceptance condition.

### R5 — `mutation.json` schema bump

`write_mutation_json`:

- `schema_version` increments from `1` to `2`.
- `totals` gains `skipped_equivalent_static` and `skipped_equivalent_byte`;
  the existing `skipped_equivalent` field is **removed** (callers should
  use the two sub-fields). `skipped` (the umbrella) keeps its existing
  meaning: `unreachable + equivalent_byte + equivalent_static`.
- `operators.<id>` gains `equivalent_byte` and `equivalent_static`; the
  existing per-operator `equivalent` field is removed.
- `mutants[]` is unchanged (static-equivalents never become entries —
  they're skipped at visitor time).

This is a breaking change to the JSON schema, hence the `schema_version`
bump. Acceptable per `MutationSummary` being labeled "CLI-internal" in
the prior spec; downstream consumers branch on `schema_version`.

### R6 — `--threads` default changes to `min(num_cpus, 8)`

`crates/zero/src/main.rs` — the `Mutate` subcommand's `--threads` arg:

- Remove `default_value_t = 1`.
- Add a `default_value_t` (or `default_value`) computed at parse time as
  `std::cmp::min(num_cpus::get(), 8).max(1)`. Use `num_cpus` if already a
  dep; otherwise `std::thread::available_parallelism().map(|n|
  n.get()).unwrap_or(1)`.
- The help text on the arg is updated to say "defaults to min(cores, 8)"
  rather than "defaults to 1".

`run_inner` already short-circuits to the sequential dispatch path when
`threads <= 1 || isolation == Isolation::InProcess`; that branch keeps
working unchanged.

### R7 — Diagnose-then-fix on the heuristic

Mirror the prior spec's diagnose-then-fix pattern. Before any
implementation:

1. Add a visitor unit test that parses a fixture mirroring the demo's
   shapes (a module-level `PART_STATUSES = [...] as const` only consumed
   by `typeof X[number]`, plus a `signal({ kind: "loading" })` with a
   subsequent `.set()`). Without changes, the test should observe these
   as `matched` sites with no special handling — i.e. they'd flow into
   the apply phase and survive.
2. Add a second test that asserts the *desired* behavior: those sites
   end up in `equivalent_static`, never reaching the worker queue.
3. Implement R1 to make test 2 pass without breaking test 1's other
   assertions (it stays as a regression check that the heuristic
   doesn't over-fire on shapes that *should* generate mutants).

Negative-case fixtures must include:
- An `as const` array that IS read at runtime
  (`for (const s of FOO) console.log(s)`) — must still generate mutants.
- A `signal({ kind: "x" })` where some read happens before any `.set()`
  (source order) — must still generate mutants.
- A non-module-level `as const` (inside a function body) — out of scope
  for the rule; must still generate mutants.

### R8 — End-to-end test on the demo shape

A new test in `crates/zero/src/cmd/mutate.rs` builds a synthetic project
that contains both demo shapes (in-memory tempdir, no external deps),
runs `run_inner`, and asserts:

- `MutationSummary.survived == 0` for those sites.
- `MutationSummary.skipped_equivalent_static == <expected count>`.
- The per-operator row for `lit_str` shows
  `equivalent_static == <expected count>`.
- `mutation.json` reflects the same.

This test is the contract for "the demo now reads 100% on
`stores/parts.ts`".

### R9 — Tests for the threads default

`crates/zero/src/main.rs` (or a CLI parse test in the same crate):

- `threads_default_is_min_cores_8`: parse `zero mutate` with no
  `--threads` flag on a host with `available_parallelism() >= 8` and
  assert the parsed value is 8; on a host with `<= 8` assert it's the
  core count; on `1`-core hosts assert it's `1`.
- `threads_explicit_overrides_default`: `--threads 2` parses as `2`
  regardless of core count.

### R10 — Docs

`docs/config-and-cli.md`:

- `zero mutate --threads` default text updated to `min(cores, 8)` with a
  one-sentence rationale ("parallel by default; cap keeps headroom on
  bigger boxes").
- The `Skipped` line example in the output block updated to show both
  `equivalent-byte` and `equivalent-static`. Add a one-paragraph
  "Reading 'equivalent-static'" note: it's mutants whose AST shape is
  provably no-op (e.g. `as const` arrays only used for type derivation,
  state-cell initializers always overwritten before read), skipped at
  collect time so they don't pad `survived` or eat worker cycles.
- The per-operator row example updated to show the subdivided columns.
- `mutation.json` schema docs (if present) bumped to `schema_version: 2`
  with the new field names.

The friction-log entries #46, #47, #48, #50 in
`~/Documents/code/zero_demo/FRAMEWORK_NOTES.md` are marked `**FIXED
YYYY-MM-DD** (#PR / commit SHA): …` per the file's convention. #46
gets a fix annotation that *also* notes the friction-log description
was off (the literals survive, they don't error) — preserving the
discovered ground truth.

## Constraints

- No new npm dependencies. `num_cpus` is already in the Rust dep tree;
  if it isn't, fall back to `std::thread::available_parallelism()`.
- `MutationSummary` and `GenerateResult` are CLI-internal; additive
  changes are fine. `mutation.json` IS consumer-visible — break it
  cleanly with `schema_version: 2`, no silent rename.
- Visitor performance: R1 detection must run in the existing collect
  walk. Module-level scanning for "any other runtime reference to
  `<Name>`" must be O(file size) per pass, not O(n²).
- The 80-line per-function guideline (CLAUDE.md) applies. The R1
  detection logic will be substantial — factor it into helpers per
  rule (`is_static_equivalent_as_const_member`,
  `is_static_equivalent_signal_init_property`) rather than inlining
  into `check()`.
- Visitor must keep working for `.js` files, not just `.ts` — the
  `as const` shape is TS-only and won't appear, but the rule must not
  panic on JS inputs.
- The R1.b "source-order dominance" check is intentionally coarse. The
  rule must err on the side of *generating* the mutant — false negatives
  in the heuristic are acceptable, false positives are not.
- No change to the worker protocol. R1 prevents sites from being
  enqueued; the worker side stays identical.
- Existing tests in both crates must still pass without modification
  (except the JSON-shape tests that read `skipped_equivalent` — those
  are updated to read the new sub-field names).

## Out of Scope

- Cross-module flow analysis. R1 is intentionally per-file.
- A general "live variable / reaching definitions" analyser. Heuristic
  (b) from refine — explicitly rejected.
- New mutation operators.
- Changing default operator selection. `--operators` unset still runs
  `Operator::ALL`.
- "Did you mean" suggestions in any error path.
- Adding a verdict-level "equivalent" distinction (currently mutants
  that *run* and produce identical observable behavior are still
  classified as `survived` — only the skip path gains the new bucket).
  That deeper distinction would need worker-protocol changes and is a
  separate item.
- The friction-log's "468 unreachable" complaint (#47) about volume.
  This spec doesn't reduce unreachable counts; it only ensures the
  *equivalent* counts are named precisely. Reducing unreachable means
  adding tests in the user's project, which is not a framework concern.
- Concurrency tuning beyond changing the default. `--threads N` is
  already exposed; the cap stays at "whatever the user passes."

## Open Questions

- **R1.b dominance approximation.** "Source order with at least one
  prior `.set()`" is the proposed check. The plan phase must confirm
  it's safe on the demo (it accepts `partsSignal`) and rejects a clear
  counter-example fixture where a read precedes any `.set()`. If the
  approximation turns out to be too coarse to be useful, fall back to
  "reject all signal-init properties unless EVERY use is `.set()`-then-
  read in source order" — narrower but more defensible.
- **`equivalent-byte` retention.** The existing byte-equality bucket
  currently shows 0 on the demo. The plan should confirm whether it
  ever fires on real code or whether it's dead weight kept around for
  symmetry. If it never fires after R1 lands (because R1 catches the
  cases that would have produced byte-identical output), consider
  collapsing it back into a single `equivalent` bucket in a follow-up
  — but not in this spec.
- **`num_cpus` vs `available_parallelism`.** The latter is std-only and
  cgroup-aware (returns the container's effective cores, not the host's).
  Preferred. The plan should confirm MSRV allows it
  (`available_parallelism` is stable since 1.59).
- **Visitor unit test ergonomics.** R7 requires parsing TS source in a
  visitor test. The crate's existing visitor tests (e.g.
  `arith_operator_generates_swap`) use a parsing helper — the plan
  should confirm the same helper accepts `as const` syntax and
  module-level scanning works in the test harness. If not, add the
  fixture loader before R1 implementation.
- **`mutation.json` schema docs.** Confirm whether `docs/config-and-cli.md`
  actually documents the JSON schema today or just references its
  existence. R10's JSON docs update is conditional on the docs page
  having a schema section to update.
