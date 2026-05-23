# Plan: `zero mutate` — Operator Error Message & "Generated: 0" Diagnostics

## Summary

Fix two friction-log entries against `zero mutate`. (1) Replace the
Debug-cased operator list in `parse_operators`' error with the
snake_case ids the CLI actually accepts. (2) Surface per-operator tallies
(matched / unreachable / equivalent / killed / survived / errored) in
both the terminal summary and `mutation.json` so a `--operators arith`
run that prints `Generated: 0` shows *why* (no AST matches vs. all
matches dropped by the coverage filter vs. all matches equivalent),
rather than being mistaken for an unimplemented operator. The visitor
already counts per-operator sites internally for apply-mode; the work is
to extend collect-mode to report those counts to the caller and wire the
breakdown through `MutationSummary` → terminal → JSON. R6 of the spec
asks for a diagnosis step first — implemented here as a fixture test
that mirrors the demo's expressions and asserts the generator produces
sites for them; if it passes (expected outcome, Branch A), the
generator is fine and only the reporter changes; if it fails (Branch B),
the executor replans with a generator-fix step inserted.

## Prerequisites

Three spec open questions resolved here so execution does not stall:

1. **Visitor result shape — named struct.** Replace `generate()`'s
   `(Vec<MutationSite>, usize)` return with a named
   `GenerateResult { sites, skipped_unreachable, per_operator }`. There
   are two real call sites (`generate_all_sites` in `zero` and
   `locate_index` in `zero-test-runner`), plus tests. The
   refactor surface is small and the struct is the right end-state per
   the spec's recommendation.
2. **Default (no-filter) per-operator block rule.** Print the
   per-operator block for an operator iff `matched > 0 && executed ==
   0` — i.e. the operator hit AST nodes but every one was filtered out.
   `matched == 0 && executed == 0` (no signal at all) is silent in the
   default-run output. With `--operators` set, every selected operator
   prints regardless.
3. **`mutation.json` schema version.** Add `"schema_version": 1` at the
   top level. Cheap insurance against future shape changes, and
   downstream tooling can branch on it.

## Steps

- [x] **Step 1: Fix `parse_operators` error message (R1)**
- [x] **Step 2: Add demo-mirroring fixture test for arith/boundary (R6)**
- [x] **Step 3: Visitor reports per-operator matched + unreachable (R2)**
- [x] **Step 4: `MutationSummary` carries per-operator breakdown (R3)**
- [x] **Step 5: Terminal summary surfaces per-operator block (R4)**
- [x] **Step 6: `mutation.json` per-operator object + schema_version (R5)**
- [x] **Step 7: Docs update for `zero mutate` (R8)**

---

## Step Details

### Step 1: Fix `parse_operators` error message (R1)

**Goal:** Smallest standalone fix — replace the Debug-cased list in the
`unknown operator id` error with the snake_case ids the CLI accepts.
Lands first because it has no dependency on the rest of the work and
immediately reduces user friction.

**Files:**
- `crates/zero-test-runner/src/mutate.rs` — add `Operator::list_ids()`.
- `crates/zero/src/cmd/mutate.rs` — use the new helper in
  `parse_operators`.

**Changes:**

#### 1a. New helper on `Operator`

In `crates/zero-test-runner/src/mutate.rs`, inside the existing
`impl Operator` block (after `parse`, before `index`):

```rust
/// Comma-separated list of every accepted operator id, in declaration
/// order. The exact string returned is parseable token-by-token by
/// [`Operator::parse`] — split on `, ` and feed each piece back in.
pub fn list_ids() -> String {
    Operator::ALL
        .iter()
        .map(|op| op.id())
        .collect::<Vec<_>>()
        .join(", ")
}
```

#### 1b. Use the helper in the error

In `crates/zero/src/cmd/mutate.rs:101-106`, replace:

```rust
let op = Operator::parse(id).ok_or_else(|| {
    anyhow::anyhow!(
        "unknown operator id: {id:?}; expected one of {:?}",
        Operator::ALL
    )
})?;
```

with:

```rust
let op = Operator::parse(id).ok_or_else(|| {
    anyhow::anyhow!(
        "unknown operator id: {id:?}; expected one of {}",
        Operator::list_ids()
    )
})?;
```

The error for `--operators help` now reads:

```
unknown operator id: "help"; expected one of arith, cmp, bool, cond_neg, boundary, lit_bool, lit_num, lit_str
```

**Tests:**

In `crates/zero/src/cmd/mutate.rs` (alongside the existing
`parse_operators_rejects_unknown`):

```rust
#[test]
fn parse_operators_error_lists_accepted_ids() {
    let err = parse_operators(Some("help")).unwrap_err();
    let msg = format!("{err}");
    // Every accepted id appears in snake_case.
    for id in ["arith", "cmp", "bool", "cond_neg", "boundary",
               "lit_bool", "lit_num", "lit_str"] {
        assert!(msg.contains(id), "missing id {id} in: {msg}");
    }
    // No Debug-cased variant name leaks through.
    for variant in ["Arith", "Cmp", "Bool", "CondNeg", "Boundary",
                    "LitBool", "LitNum", "LitStr"] {
        assert!(!msg.contains(variant),
            "leaked Debug name {variant} in: {msg}");
    }
}
```

In `crates/zero-test-runner/src/mutate.rs` (alongside
`operator_id_round_trip`):

```rust
#[test]
fn list_ids_round_trips_through_parse() {
    let s = Operator::list_ids();
    let parsed: Vec<Operator> = s
        .split(", ")
        .map(|t| Operator::parse(t).expect("listed id should parse"))
        .collect();
    assert_eq!(parsed, Operator::ALL.to_vec());
}
```

Run `cargo test -p zero -p zero-test-runner --tests`.

---

### Step 2: Add demo-mirroring fixture test for arith/boundary (R6)

**Goal:** R6 says diagnose before fixing the generator. The diagnosis is
codified as a regression test that mirrors the demo's expressions and
asserts the generator produces sites for them. If the test passes,
Branch A holds (generator is fine; subsequent steps are reporter-only).
If it fails, Branch B is in play — pause, replan with a generator-fix
step before Step 3.

This step is the explicit fork point. It also doubles as the regression
test the spec requires under either branch.

**Files:**
- `crates/zero-test-runner/src/mutate.rs` — add tests in the existing
  `#[cfg(test)] mod tests` block.

**Changes:**

Add the following tests. Each parses a TS snippet from the demo and
asserts the visitor produces ≥ 1 site of the named operator with
`covered_lines = None` (i.e., pre-coverage-filter).

```rust
#[test]
fn arith_matches_demo_division_in_math_ceil() {
    let src = "const pages = Math.ceil(tc / PAGE_SIZE);\n";
    let (sites, _) = generate(
        src, &PathBuf::from("/abs/demo.ts"),
        &opts(&[Operator::Arith])
    ).expect("g");
    assert!(
        sites.iter().any(|s| s.operator == Operator::Arith),
        "expected arith site for `tc / PAGE_SIZE`, got {sites:?}"
    );
}

#[test]
fn arith_matches_demo_mul_then_div() {
    let src = "const pct = SlotsUsed * 100 / SlotsTotal;\n";
    let (sites, _) = generate(
        src, &PathBuf::from("/abs/demo.ts"),
        &opts(&[Operator::Arith])
    ).expect("g");
    // 2 binary ops: `*` and `/`; both arith.
    let arith_count = sites.iter()
        .filter(|s| s.operator == Operator::Arith)
        .count();
    assert!(arith_count >= 2, "expected ≥ 2 arith sites, got {arith_count}");
}

#[test]
fn arith_matches_demo_simple_division() {
    let src = "function ratio(onHand: number, denom: number) { return onHand / denom; }\n";
    let (sites, _) = generate(
        src, &PathBuf::from("/abs/demo.ts"),
        &opts(&[Operator::Arith])
    ).expect("g");
    assert!(sites.iter().any(|s| s.operator == Operator::Arith));
}

#[test]
fn boundary_matches_demo_lte() {
    let src = "const low = onHand <= ReorderPoint;\n";
    let (sites, _) = generate(
        src, &PathBuf::from("/abs/demo.ts"),
        &opts(&[Operator::Boundary])
    ).expect("g");
    assert!(sites.iter().any(|s| s.operator == Operator::Boundary));
}

#[test]
fn boundary_matches_demo_lte_inside_pagination() {
    let src = "if (tc <= PAGE_SIZE) return 1;\n";
    let (sites, _) = generate(
        src, &PathBuf::from("/abs/demo.ts"),
        &opts(&[Operator::Boundary])
    ).expect("g");
    assert!(sites.iter().any(|s| s.operator == Operator::Boundary));
}
```

Note: these tests use the **old** `(Vec, usize)` tuple return shape from
`generate()`. They will be updated in Step 3 when the return type
becomes a struct.

**Diagnosis recording.** After running `cargo test -p zero-test-runner`,
the executor records one of the following at the bottom of this file
under a new `## Diagnosis log` heading:

- **Branch A (expected):** All five tests pass. Proceed to Step 3 as
  written. Note: "Confirmed: demo expressions produce sites
  pre-coverage-filter. Friction-log entry is reporter-side; the
  generator is fine."
- **Branch B:** One or more tests fail. Stop. Capture which operator on
  which expression fails. Replan with a new step inserted before Step 3
  that fixes the generator to handle the failing shape. The fixture
  tests themselves remain as the regression suite for the generator
  fix.

**Tests:** the five fixture tests above are themselves the deliverable
of this step. Pass them with `cargo test -p zero-test-runner`.

---

### Step 3: Visitor reports per-operator matched + unreachable (R2)

**Goal:** Extend `generate()` to return, per `Operator`, the count of
AST nodes the visitor matched (pre-coverage) and the count it filtered
as unreachable. Foundation for the subsequent reporter work — without
per-operator data flowing out of the visitor, `MutationSummary` has
nothing to aggregate.

**Files:**
- `crates/zero-test-runner/src/mutate.rs` — `GenerateResult` struct,
  visitor counters, `generate()` signature change, callers in tests.
- `crates/zero/src/cmd/mutate.rs` — `generate_all_sites` updated for
  the new return shape (just transitively; the per-operator data is
  threaded into `MutationSummary` in Step 4).

**Changes:**

#### 3a. New `GenerateResult` struct

In `crates/zero-test-runner/src/mutate.rs`, just before `generate()`:

```rust
/// Result of a [`generate`] pass.
#[derive(Debug)]
pub struct GenerateResult {
    /// Concrete mutation sites the caller will apply and execute.
    pub sites: Vec<MutationSite>,
    /// Total sites filtered out because `covered_lines` did not include
    /// their line. Aggregated across all operators.
    pub skipped_unreachable: usize,
    /// Per-operator tally captured during the collect walk. Indexed by
    /// `Operator::index()`.
    pub per_operator: PerOperatorTally,
}

/// Per-operator counts produced by the collect-mode visitor. Indexed
/// the same way as [`Operator::ALL`].
#[derive(Debug, Default, Clone, Copy)]
pub struct PerOperatorTally {
    /// AST nodes the operator's swap function accepted, before any
    /// filtering. For arith this includes the string-concat exclusion
    /// (matches `+` only when both sides are not string literals).
    pub matched: [usize; 8],
    /// Subset of `matched` that was filtered by `covered_lines` and not
    /// returned in `sites`. Equals `matched[i]` when
    /// `covered_lines = None` and no line covered (i.e., zero — they
    /// all flow through).
    pub unreachable: [usize; 8],
}

impl PerOperatorTally {
    /// Lookup helper.
    pub fn get(&self, op: Operator) -> OperatorCounts {
        OperatorCounts {
            matched: self.matched[op.index()],
            unreachable: self.unreachable[op.index()],
        }
    }
}

/// View of a single operator's collect-mode counts.
#[derive(Debug, Clone, Copy)]
pub struct OperatorCounts {
    pub matched: usize,
    pub unreachable: usize,
}
```

`Operator::index` is currently `fn index(self) -> usize` (private at
line 86). Promote it to `pub fn index(self) -> usize` so external
callers (the `zero` crate aggregator in Step 4) can index a parallel
array. No behavior change.

#### 3b. Visitor counter changes

In `MutateVisitor`:

```rust
struct MutateVisitor<'a> {
    cm: Lrc<SwcSourceMap>,
    file: PathBuf,
    mode: Mode,
    operators_filter: Option<&'a [Operator]>,
    covered_lines: Option<&'a HashSet<u32>>,
    sites: Vec<MutationSite>,
    counts: [usize; 8],                 // existing — used by apply-mode
    skipped_unreachable: usize,         // existing — global tally
    matched: [usize; 8],                // NEW — per-op AST matches
    unreachable_per_op: [usize; 8],     // NEW — per-op unreachable
}
```

Initialize both new arrays to `[0; 8]` in `new_collect` and `new_apply`
(apply mode never reads them; zeroing is enough).

In `check()` (mutate.rs:414-457), inside `Mode::Collect`:

```rust
fn check(
    &mut self,
    op: Operator,
    line: u32,
    column: u32,
    original: &str,
    replacement: &str,
) -> bool {
    let idx = op.index();
    match self.mode {
        Mode::Collect => {
            if !self.filter_allows(op) {
                return false;
            }
            // NEW: count every AST match that passes the operator
            // filter, regardless of coverage outcome.
            self.matched[idx] += 1;
            if let Some(cov) = self.covered_lines
                && !cov.contains(&line)
            {
                self.skipped_unreachable += 1;
                self.unreachable_per_op[idx] += 1;   // NEW
                return false;
            }
            self.counts[idx] += 1;
            self.sites.push(MutationSite { ... });
            false
        }
        Mode::Apply { .. } => { /* unchanged */ }
    }
}
```

**Important semantic note about the operator filter.** `matched` only
counts nodes the operator filter *accepted* (i.e. nodes whose operator
is in `operators_filter` when set). That matches the user's mental
model: if they pass `--operators arith`, `matched[arith]` is the count
of arith AST nodes seen, and other operators' rows are zero. If they
don't pass a filter, every operator gets counted.

#### 3c. `generate()` return shape

Replace the current return:

```rust
pub fn generate(
    source: &str,
    file: &Path,
    opts: &GenerateOptions<'_>,
) -> Result<(Vec<MutationSite>, usize), TranspileError>
```

with:

```rust
pub fn generate(
    source: &str,
    file: &Path,
    opts: &GenerateOptions<'_>,
) -> Result<GenerateResult, TranspileError>
```

The body returns:

```rust
Ok(GenerateResult {
    sites: limited,
    skipped_unreachable: skipped,
    per_operator: PerOperatorTally {
        matched: v.matched,
        unreachable: v.unreachable_per_op,
    },
})
```

(Where `v` is the existing `MutateVisitor` instance whose fields we now
read.)

#### 3d. Caller updates

`locate_index` (mutate.rs:249) currently destructures the tuple:

```rust
let (sites, _) = generate(source, file, &opts)?;
```

Becomes:

```rust
let result = generate(source, file, &opts)?;
let sites = result.sites;
```

`generate_all_sites` in `crates/zero/src/cmd/mutate.rs:470-511`
currently does:

```rust
let (sites, unreachable) = match generate(&raw, src, &gen_opts) {
    Ok(r) => r,
    Err(_) => continue,
};
summary.skipped_unreachable += unreachable;
for s in sites { ... }
```

Becomes:

```rust
let result = match generate(&raw, src, &gen_opts) {
    Ok(r) => r,
    Err(_) => continue,
};
summary.skipped_unreachable += result.skipped_unreachable;
// Per-operator aggregation lands here in Step 4.
for s in result.sites { ... }
```

Step 4 layers in the per-operator aggregation; for now the
`result.per_operator` field is read but discarded. This is fine — the
field exists and is correct, just not yet plumbed downstream.

All existing test files that destructure the tuple (every `let (sites,
_) = generate(...)` in `mutate.rs` tests, and the Step 2 fixture tests)
update mechanically:

```rust
let GenerateResult { sites, .. } = generate(src, ...).expect("g");
```

Or with destructuring at the let:

```rust
let result = generate(src, ...).expect("g");
let sites = result.sites;
```

Pick the more readable per-site (both work; the planner doesn't care).

**Tests:**

Add to `crates/zero-test-runner/src/mutate.rs` tests:

```rust
#[test]
fn visitor_reports_per_operator_match_counts() {
    let src = "const r = (a + b) < c;\nconst s = a <= b;\nconst t = a / b;\n";
    let r = generate(
        src, &PathBuf::from("/abs/a.ts"),
        &GenerateOptions {
            operators: Operator::ALL,
            max_mutants: None,
            covered_lines: None,
        },
    ).expect("g");
    let arith = r.per_operator.get(Operator::Arith);
    let cmp = r.per_operator.get(Operator::Cmp);
    let boundary = r.per_operator.get(Operator::Boundary);
    // `a + b`, `a / b` → 2 arith sites.
    assert_eq!(arith.matched, 2, "arith: {:?}", arith);
    // `(a + b) < c`, `a <= b` → 2 cmp sites.
    assert_eq!(cmp.matched, 2, "cmp: {:?}", cmp);
    // Same two relational operators → 2 boundary sites.
    assert_eq!(boundary.matched, 2, "boundary: {:?}", boundary);
    // Nothing was filtered.
    assert_eq!(arith.unreachable, 0);
    assert_eq!(cmp.unreachable, 0);
    assert_eq!(boundary.unreachable, 0);
}

#[test]
fn visitor_counts_unreachable_per_operator() {
    let src = "const r = a + b;\nconst s = c + d;\n";
    let mut covered: HashSet<u32> = HashSet::new();
    covered.insert(1); // only line 1
    let r = generate(
        src, &PathBuf::from("/abs/a.ts"),
        &GenerateOptions {
            operators: &[Operator::Arith],
            max_mutants: None,
            covered_lines: Some(&covered),
        },
    ).expect("g");
    let arith = r.per_operator.get(Operator::Arith);
    assert_eq!(arith.matched, 2);      // both `a+b` and `c+d`
    assert_eq!(arith.unreachable, 1);  // `c+d` on uncovered line 2
    assert_eq!(r.sites.len(), 1);
}

#[test]
fn visitor_per_operator_respects_filter() {
    // Source has arith and cmp. With filter=[Arith], only arith counts.
    let src = "const r = (a + b) < c;\n";
    let r = generate(
        src, &PathBuf::from("/abs/a.ts"),
        &GenerateOptions {
            operators: &[Operator::Arith],
            max_mutants: None,
            covered_lines: None,
        },
    ).expect("g");
    assert_eq!(r.per_operator.get(Operator::Arith).matched, 1);
    assert_eq!(r.per_operator.get(Operator::Cmp).matched, 0);
}
```

The Step 2 fixture tests are updated to the new return shape as part of
this step.

Run `cargo test -p zero-test-runner -p zero --tests`.

---

### Step 4: `MutationSummary` carries per-operator breakdown (R3)

**Goal:** Aggregate per-operator counts across all source files, plus
the per-mutant verdicts from dispatch, into `MutationSummary`. Once
done, all downstream consumers (terminal summary, JSON) can read the
breakdown without rerunning anything.

**Files:**
- `crates/zero/src/cmd/mutate.rs` — `MutationSummary` struct, all
  internal helpers that mutate it.

**Changes:**

#### 4a. New per-operator summary fields

Add to `MutationSummary`:

```rust
#[derive(Debug, Default)]
pub struct MutationSummary {
    pub generated: usize,
    pub killed: usize,
    pub survived: usize,
    pub errored: usize,
    pub skipped_unreachable: usize,
    pub skipped_equivalent: usize,
    pub baseline_passed: bool,
    pub outcomes: BTreeMap<PathBuf, Vec<(MutationSite, MutantStatus)>>,
    /// Per-operator breakdown. Each field is indexed by
    /// `Operator::index()`. `matched + unreachable` is the visitor's
    /// view; `executed = killed + survived + errored`; `equivalent` is
    /// the byte-compare skip count.
    pub per_operator: PerOperatorSummary,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct PerOperatorSummary {
    pub matched: [usize; 8],
    pub unreachable: [usize; 8],
    pub equivalent: [usize; 8],
    pub killed: [usize; 8],
    pub survived: [usize; 8],
    pub errored: [usize; 8],
}

impl PerOperatorSummary {
    pub fn executed(&self, op: Operator) -> usize {
        let i = op.index();
        self.killed[i] + self.survived[i] + self.errored[i]
    }
    pub fn generated(&self, op: Operator) -> usize {
        self.executed(op)
    }
}
```

Note: `generated` per operator and `executed` per operator are the same
quantity (matches the global `generated` definition). The method is a
named alias for clarity at call sites.

#### 4b. Aggregate visitor tallies in `generate_all_sites`

Where Step 3 leaves a TODO, fold:

```rust
for i in 0..8 {
    summary.per_operator.matched[i] += result.per_operator.matched[i];
    summary.per_operator.unreachable[i] += result.per_operator.unreachable[i];
}
```

#### 4c. Track per-operator equivalent / errored / killed / survived

`pre_apply_to_queue` (mutate.rs:516-546) — on the `equivalent` branch
(`mutated_js == *baseline_js`):

```rust
if mutated_js == *baseline_js {
    summary.skipped_equivalent += 1;
    summary.per_operator.equivalent[site.operator.index()] += 1;   // NEW
} else { ... }
```

On the `Err(_)` branch (apply error → errored):

```rust
Err(_) => {
    summary.generated += 1;
    summary.errored += 1;
    summary.per_operator.errored[site.operator.index()] += 1;      // NEW
    ...
}
```

`consume_mutant_results` (mutate.rs:550-591) — on each verdict:

```rust
match status {
    MutantStatus::Killed => {
        summary.killed += 1;
        summary.per_operator.killed[site.operator.index()] += 1;   // NEW
    }
    MutantStatus::Survived => {
        summary.survived += 1;
        summary.per_operator.survived[site.operator.index()] += 1; // NEW
    }
    MutantStatus::Errored => {
        summary.errored += 1;
        summary.per_operator.errored[site.operator.index()] += 1;  // NEW
    }
}
```

The `site` variable is already in scope at both locations.

#### 4d. No-op for unfiltered runs is fine

When `--operators` is unset, the visitor records every operator's
`matched`. Step 5 decides which to surface; Step 4 just makes the data
available.

**Tests:**

In `crates/zero/src/cmd/mutate.rs`:

```rust
#[test]
fn per_operator_summary_filtered_run_counts_matches() {
    // Strong test on `a + b`; with filter=[Arith], summary.per_operator
    // shows arith matched=1 executed=1 killed=1.
    let dir = make_project(
        "export function add(a: number, b: number) { return a + b }\n",
        r#"import { describe, it, expect } from "zero/test";
import { add } from "./src/foo.ts";
describe("g", () => {
  it("adds 1+2", () => expect(add(1, 2)).toBe(3));
  it("adds 5+7", () => expect(add(5, 7)).toBe(12));
});
"#,
    );
    let root = dir.path();
    let out = root.join("dist");
    let mut sink: Vec<u8> = Vec::new();
    let summary = run_inner(
        root, &out, root, None, &[Operator::Arith],
        None, true, Isolation::InProcess, 1, &mut sink,
    ).expect("ok");
    let arith = Operator::Arith.index();
    assert!(summary.per_operator.matched[arith] >= 1);
    assert_eq!(summary.per_operator.unreachable[arith], 0);
    assert_eq!(summary.per_operator.equivalent[arith], 0);
    assert!(summary.per_operator.killed[arith] >= 1);
    assert_eq!(summary.per_operator.survived[arith], 0);
    // Other operators should be all-zero under the filter.
    let cmp = Operator::Cmp.index();
    assert_eq!(summary.per_operator.matched[cmp], 0);
}

#[test]
fn per_operator_summary_unreachable_when_uncovered() {
    // Source has arith on a line no test exercises (lib has untested
    // helper). All arith sites should land in unreachable.
    let dir = make_project(
        // The exported function is never called by the test. Coverage
        // never visits its body line.
        "export function unused(a: number, b: number) { return a + b }\nexport const ok = 1;\n",
        r#"import { describe, it, expect } from "zero/test";
import { ok } from "./src/foo.ts";
describe("g", () => { it("ok", () => expect(ok).toBe(1)); });
"#,
    );
    let root = dir.path();
    let out = root.join("dist");
    let mut sink: Vec<u8> = Vec::new();
    let summary = run_inner(
        root, &out, root, None, &[Operator::Arith],
        None, true, Isolation::InProcess, 1, &mut sink,
    ).expect("ok");
    let arith = Operator::Arith.index();
    assert!(summary.per_operator.matched[arith] >= 1,
        "expected an arith match");
    assert_eq!(summary.per_operator.matched[arith],
               summary.per_operator.unreachable[arith],
        "all arith matches should be unreachable");
    assert_eq!(summary.per_operator.executed(Operator::Arith), 0);
}
```

Run `cargo test -p zero --tests`.

---

### Step 5: Terminal summary surfaces per-operator block (R4)

**Goal:** Make a `Generated: 0` run on a filter-selected operator legible
at a glance. With the data from Step 4, write a per-operator block in
`write_terminal_summary` that distinguishes the four "Generated: 0"
sub-cases.

**Files:**
- `crates/zero/src/cmd/mutate.rs` — `write_terminal_summary`, new
  `write_per_operator_block` helper, `run` updated to pass the
  operator filter down.

**Changes:**

#### 5a. Thread the operator filter into the summary writer

`write_terminal_summary` currently takes `summary`, `report_base`,
`quiet`. It needs to know which operators the user filtered on so it can
choose between "always print" (filter set) and "print only if matched
> 0 && executed == 0" (no filter). Add a parameter:

```rust
fn write_terminal_summary<W: Write>(
    w: &mut W,
    summary: &MutationSummary,
    report_base: &Path,
    quiet: bool,
    operator_filter: Option<&[Operator]>,
) -> std::io::Result<()>
```

`run` (mutate.rs:818) already parses `operators: Option<String>` into a
`Vec<Operator>`. Pass `Some(&ops)` when the user supplied `--operators`,
`None` when they didn't. Disambiguate by inspecting the raw
`operators.as_deref()` before consuming it:

```rust
let filter_was_set = operators.is_some();
let ops = parse_operators(operators.as_deref())?;
// ...
write_terminal_summary(
    &mut stdout, &summary, &cwd, quiet,
    if filter_was_set { Some(&ops) } else { None },
)?;
```

(`parse_operators` collapses `None` to `Operator::ALL`, so we can't
recover "was the user explicit" from `ops` alone — track it directly.)

#### 5b. The per-operator block

After the existing `Skipped:` line and before the `Survived mutants:`
detail block:

```rust
let ops_to_print: Vec<Operator> = match operator_filter {
    Some(filter) => filter.to_vec(),
    None => Operator::ALL.iter().copied()
        .filter(|op| {
            let i = op.index();
            let executed = summary.per_operator.killed[i]
                         + summary.per_operator.survived[i]
                         + summary.per_operator.errored[i];
            summary.per_operator.matched[i] > 0 && executed == 0
        })
        .collect(),
};

if !ops_to_print.is_empty() {
    writeln!(w)?;
    writeln!(w, "Per-operator breakdown:")?;
    for op in &ops_to_print {
        write_per_operator_row(w, &summary.per_operator, *op)?;
    }
}
```

`write_per_operator_row`:

```rust
fn write_per_operator_row<W: Write>(
    w: &mut W,
    per_op: &PerOperatorSummary,
    op: Operator,
) -> std::io::Result<()> {
    let i = op.index();
    let matched = per_op.matched[i];
    if matched == 0 {
        writeln!(w, "  {}: 0 matches in src/", op.id())?;
        return Ok(());
    }
    let unreachable = per_op.unreachable[i];
    let equivalent = per_op.equivalent[i];
    let killed = per_op.killed[i];
    let survived = per_op.survived[i];
    let errored = per_op.errored[i];
    let executed = killed + survived + errored;
    writeln!(
        w,
        "  {}: matched {}, executed {} (killed {}, survived {}, errored {}), unreachable {}, equivalent {}",
        op.id(), matched, executed, killed, survived, errored, unreachable
    )?;
    Ok(())
}
```

The first row variant ("0 matches in src/") fires when a filter-selected
operator never matched. The second variant always shows the full
breakdown.

#### 5c. Helper-extraction discipline

`write_terminal_summary` is already ~70 lines today (mutate.rs:212-284)
and adds ~15 lines with the block selector. To stay within the 80-line
guideline (CLAUDE.md), extract the per-operator block selection into a
small `select_operators_for_block(summary, filter) -> Vec<Operator>`
helper, leaving `write_terminal_summary` itself unchanged in
length-class.

#### 5d. Update `run_inner` call site in CLI

In `run` (mutate.rs:818-860), update the call to pass the filter
correctly:

```rust
write_terminal_summary(&mut stdout, &summary, &cwd, quiet,
    if filter_was_set { Some(&ops) } else { None })?;
```

Existing tests calling `write_terminal_summary` need a new arg — search
for callers and pass `None` unless the test exercises the filter case.

**Tests:**

In `crates/zero/src/cmd/mutate.rs`:

```rust
#[test]
fn terminal_summary_filtered_run_prints_per_operator_block() {
    // Build the same uncovered-arith project from Step 4.
    let dir = make_project(
        "export function unused(a: number, b: number) { return a + b }\nexport const ok = 1;\n",
        r#"import { describe, it, expect } from "zero/test";
import { ok } from "./src/foo.ts";
describe("g", () => { it("ok", () => expect(ok).toBe(1)); });
"#,
    );
    let root = dir.path();
    let out = root.join("dist");
    let mut sink: Vec<u8> = Vec::new();
    let summary = run_inner(
        root, &out, root, None, &[Operator::Arith],
        None, true, Isolation::InProcess, 1, &mut sink,
    ).expect("ok");

    let mut buf: Vec<u8> = Vec::new();
    write_terminal_summary(&mut buf, &summary, root, true, Some(&[Operator::Arith]))
        .expect("write");
    let s = String::from_utf8(buf).unwrap();
    assert!(s.contains("Per-operator breakdown:"), "got:\n{s}");
    assert!(s.contains("arith:"), "got:\n{s}");
    assert!(s.contains("unreachable"), "got:\n{s}");
}

#[test]
fn terminal_summary_filtered_run_zero_matches_says_so() {
    // Project has no arith expressions at all.
    let dir = make_project(
        "export const ok = true;\n",
        r#"import { describe, it, expect } from "zero/test";
import { ok } from "./src/foo.ts";
describe("g", () => { it("ok", () => expect(ok).toBe(true)); });
"#,
    );
    let root = dir.path();
    let out = root.join("dist");
    let mut sink: Vec<u8> = Vec::new();
    let summary = run_inner(
        root, &out, root, None, &[Operator::Arith],
        None, true, Isolation::InProcess, 1, &mut sink,
    ).expect("ok");
    let mut buf: Vec<u8> = Vec::new();
    write_terminal_summary(&mut buf, &summary, root, true, Some(&[Operator::Arith]))
        .expect("write");
    let s = String::from_utf8(buf).unwrap();
    assert!(s.contains("arith: 0 matches in src/"), "got:\n{s}");
}

#[test]
fn terminal_summary_default_run_quiet_on_clean_operators() {
    // Strong test on `a + b`; all arith mutants killed. Default run
    // (no filter) should NOT print a per-operator block because no
    // operator has matched > 0 && executed == 0.
    let dir = make_project(
        "export function add(a: number, b: number) { return a + b }\n",
        r#"import { describe, it, expect } from "zero/test";
import { add } from "./src/foo.ts";
describe("g", () => {
  it("adds 1+2", () => expect(add(1, 2)).toBe(3));
  it("adds 5+7", () => expect(add(5, 7)).toBe(12));
});
"#,
    );
    let root = dir.path();
    let out = root.join("dist");
    let mut sink: Vec<u8> = Vec::new();
    let summary = run_inner(
        root, &out, root, None, Operator::ALL,
        None, true, Isolation::InProcess, 1, &mut sink,
    ).expect("ok");
    let mut buf: Vec<u8> = Vec::new();
    write_terminal_summary(&mut buf, &summary, root, true, None).expect("write");
    let s = String::from_utf8(buf).unwrap();
    assert!(!s.contains("Per-operator breakdown:"),
        "default run should be quiet on clean operators; got:\n{s}");
}
```

Run `cargo test -p zero --tests`.

---

### Step 6: `mutation.json` per-operator object + schema_version (R5)

**Goal:** Same data as the terminal block, but machine-readable, plus a
schema version. Lets dashboards / CI gates consume the breakdown without
parsing the per-mutant `outcomes` list.

**Files:**
- `crates/zero/src/cmd/mutate.rs` — `write_mutation_json`.

**Changes:**

In `write_mutation_json` (mutate.rs:288-331), extend the emitted JSON:

```rust
let mut operators_obj = serde_json::Map::new();
for op in Operator::ALL {
    let i = op.index();
    let executed = summary.per_operator.killed[i]
                 + summary.per_operator.survived[i]
                 + summary.per_operator.errored[i];
    operators_obj.insert(op.id().to_string(), serde_json::json!({
        "matched":     summary.per_operator.matched[i],
        "unreachable": summary.per_operator.unreachable[i],
        "equivalent":  summary.per_operator.equivalent[i],
        "killed":      summary.per_operator.killed[i],
        "survived":    summary.per_operator.survived[i],
        "errored":     summary.per_operator.errored[i],
        "executed":    executed,
    }));
}

let value = serde_json::json!({
    "schema_version": 1,                  // NEW
    "totals": { /* unchanged */ },
    "operators": operators_obj,           // NEW
    "files": files,                       // unchanged
});
```

Every operator always appears in the `operators` map (eight keys), even
when its counts are all zero. Predictable shape for consumers.

**Tests:**

```rust
#[test]
fn mutation_json_includes_per_operator_and_schema_version() {
    let dir = make_project(
        "export function add(a: number, b: number) { return a + b }\n",
        r#"import { describe, it, expect } from "zero/test";
import { add } from "./src/foo.ts";
describe("g", () => {
  it("adds", () => expect(add(1, 2)).toBe(3));
});
"#,
    );
    let root = dir.path();
    let out = root.join("dist");
    let mut sink: Vec<u8> = Vec::new();
    let summary = run_inner(
        root, &out, root, None, &[Operator::Arith],
        None, true, Isolation::InProcess, 1, &mut sink,
    ).expect("ok");
    write_mutation_json(root, root, &summary).expect("write json");

    let s = fs::read_to_string(root.join("mutation/mutation.json")).unwrap();
    let v: serde_json::Value = serde_json::from_str(&s).unwrap();

    assert_eq!(v["schema_version"], 1);
    assert!(v["operators"].is_object());
    let ops = v["operators"].as_object().unwrap();
    assert_eq!(ops.len(), 8, "expected all 8 operators in json");
    for id in ["arith", "cmp", "bool", "cond_neg", "boundary",
               "lit_bool", "lit_num", "lit_str"] {
        assert!(ops.contains_key(id), "missing operator {id}");
    }
    let arith = &ops["arith"];
    assert!(arith["matched"].as_u64().unwrap() >= 1);
    assert_eq!(arith["killed"].as_u64().unwrap(),
               arith["executed"].as_u64().unwrap());
}
```

Run `cargo test -p zero --tests`.

---

### Step 7: Docs update for `zero mutate` (R8)

**Goal:** Tell users what the new output means so the next adopter
doesn't repeat the friction-log misinterpretation. Without this step the
entries persist for the next reader.

**Files:**
- `docs/config-and-cli.md` — `zero mutate` subcommand section
  (currently lines 162-176).

**Changes:**

Replace the existing `### zero mutate [pattern]` section with an
expanded version. Keep the existing flag table; add a "Reading the
output" subsection plus a confirmation that operator ids are
snake_case.

```markdown
### `zero mutate [pattern]`

Mutation testing. Runs the baseline test suite, then iterates
over mutation sites in `src/`, re-running the affected tests
with each mutation applied.

| Flag                          | Description                                            |
|-------------------------------|--------------------------------------------------------|
| `--operators <csv>`           | Restrict to operator families (e.g. `arith,bool`).     |
| `--max-mutants <n>`           | Cap total mutants generated.                           |
| `--threads <n>`               | Run mutants in parallel.                               |
| `-q, --quiet`                 | Suppress per-mutant lines; print summary only.         |

Operator ids accepted by `--operators`: `arith`, `cmp`, `bool`,
`cond_neg`, `boundary`, `lit_bool`, `lit_num`, `lit_str`.

#### Reading `Generated: 0`

`Generated: 0 mutants` on a `--operators` run can mean three things:

- **No matches in `src/`.** The operator is implemented, but no AST
  node in the codebase matched its swap rules.
- **All matches on uncovered lines.** Sites were found but no
  baseline test exercises those lines; the coverage filter drops them.
- **All matches equivalent.** Sites were found and reached, but the
  mutated JS was byte-identical to the baseline (rare).

The per-operator breakdown printed under the headline distinguishes
the three. A row like `arith: matched 12, executed 0 (...), unreachable
12, equivalent 0` says "12 arith sites exist, every one is on a line
no test reaches" — write a test that calls into that code.

Writes `mutation/mutation.json` (schema version 1) with structured
results, including a per-operator breakdown under `operators`. Exit
code is non-zero if any mutant survived or errored.
```

**Tests:** N/A — docs change. Verify by reading the page in a browser
or via `cargo run -p zero -- ...` if there's a docs serve command; per
CLAUDE.md the smoke check is "the change renders sensibly."

Run `cargo test --workspace` once more to confirm nothing broke.

---

## Risks and Assumptions

- **Branch A vs Branch B fork at Step 2.** The plan assumes Step 2's
  fixture tests pass (Branch A: generator is fine, work is reporter-
  side). High confidence — the visitor visits `BinExpr` nodes
  unconditionally and `arith_swap` / `boundary_swap` are exhaustive over
  the relevant `BinaryOp` variants. If a test fails, the executor stops
  and replans with a generator-fix step before Step 3. The fixture tests
  themselves are valuable under either branch.
- **`Operator::index` visibility change.** Step 3 promotes
  `Operator::index` from `fn` to `pub fn`. No behavior change; the
  method is already accurate. If a consumer outside the workspace ever
  imports it, the `pub` is the right end-state anyway.
- **80-line function guideline.** `write_terminal_summary` is already
  long; Step 5 extracts `write_per_operator_row` and
  `select_operators_for_block` to keep `write_terminal_summary` under
  80 lines. CLAUDE.md treats this as a signal, not a hard gate, so
  going slightly over is tolerable if the alternative damages
  readability.
- **`mutation.json` consumer compatibility.** Adding `schema_version`
  and `operators` is purely additive. Any consumer that ignores unknown
  fields is unaffected. No existing field is renamed or removed.
- **Operator filter detection in `run`.** Step 5 relies on observing
  `operators: Option<String>` *before* it's parsed to know whether the
  user passed a filter. Existing code shape supports this — the
  `is_some()` check happens immediately before `parse_operators` is
  called.
- **Apply-mode visitor untouched.** Step 3 extends collect-mode counters
  only. Apply mode is the hot path for actually applying mutations and
  must not change behavior. The new fields are zero in apply-mode and
  ignored by callers — verified by leaving `new_apply` to initialize
  them to `[0; 8]` and never reading them under `Mode::Apply`.
- **Per-operator counts vs `--max-mutants`.** `generate()` applies
  `max_mutants` truncation *after* counting. So `per_operator.matched`
  can exceed the number of sites that actually flow into the queue when
  the user passes `--max-mutants`. This is fine — `matched` is the
  visitor-level count, not the "sites we'll run" count. The summary
  still adds up correctly because `executed` is tallied from actual
  worker verdicts.
- **`run_one_mutant_subprocess` exit-code semantics.** `consume_mutant_results`
  doesn't see equivalent skips (those are tallied in `pre_apply_to_queue`
  before dispatch). Step 4 puts the equivalent-bump there, not in
  `consume_mutant_results`. Confirmed against mutate.rs:516-546.
- **Test fixture for `unreachable` aggregation.** The Step 4 test
  "per_operator_summary_unreachable_when_uncovered" relies on the
  coverage filter actually classifying an exported-but-unused function's
  body as uncovered. If the JS coverage runtime reports lines for
  imported-but-unexercised module bodies (it shouldn't — coverage is
  execution-driven), this test will need adjustment. Worth verifying
  during execution; small risk.

## Diagnosis log

**Branch A (confirmed, 2026-05-23).** All five Step 2 fixture tests pass on
the unchanged generator:

- `arith_matches_demo_division_in_math_ceil` (`Math.ceil(tc / PAGE_SIZE)`)
- `arith_matches_demo_mul_then_div` (`SlotsUsed * 100 / SlotsTotal`, 2 sites)
- `arith_matches_demo_simple_division` (`onHand / denom`)
- `boundary_matches_demo_lte` (`onHand <= ReorderPoint`)
- `boundary_matches_demo_lte_inside_pagination` (`tc <= PAGE_SIZE`)

The demo's expressions produce sites pre-coverage-filter. The friction-log
entry is reporter-side; no generator fix needed. Proceeding to Step 3 as
written.
