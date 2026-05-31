# Spec: make `zero test` faster

## Problem Statement

`zero test` feels slow enough to be worth fixing, but the cause is not yet
established. This item is **measure first, then fix** — its first deliverable is
a breakdown of where wall-clock time goes, not a chosen optimization. Picking a
fix before profiling risks optimizing a path that isn't the bottleneck.

## Background

Candidate cost centers, to be confirmed by measurement, not assumed:

- **Runtime init (Boa).** Cold-starting the Boa engine and evaluating the
  concatenated DOM/web-platform shim blob
  (`zero-test-runner::harness::eval_dom_shim`) on every run, and possibly per
  test file. If the shim blob is re-parsed per file, that is a prime suspect.
- **Transpile.** swc transpiling each `.ts`/`.js` module on every run with no
  caching. Repeated runs re-doing identical work.
- **Discovery.** Filesystem walk to find test files — likely cheap, but confirm.
- **Per-file vs. per-run isolation.** How many Boa contexts are created — one
  per file (safe, slow) or shared (fast, risky)? The
  `boa_maplock_finalizer` memory constrains how teardown can be structured, so
  any "share the context" idea must respect that.

## Proposed Approach

1. **Instrument.** Add timing around discovery, transpile, runtime init, and
   test execution. Run on a representative file set (the framework's own runtime
   tests, and/or a synthetic N-file set) and produce a breakdown.
2. **Decide from data.** Only after the breakdown, pick targets. Likely
   candidates depending on results:
   - Build the shim blob / Boa context once and reuse across files (respecting
     the GC-teardown constraint).
   - Cache transpile output keyed on source hash across runs.
   - Parallelize per-file execution if isolation allows.
3. **Guard against regressions.** Capture a before/after number in the issue so
   the win is documented and a future change that re-slows it is visible.

## Scope / Non-Goals

- In scope: `zero test` startup and per-file overhead.
- Non-goal: rewriting the runtime engine, swapping Boa, or changing the JS
  semantics the harness exposes.

## Open Questions (resolve with user before plan)

- Is the pain mostly **cold start** (one `zero test app.test.js`) or
  **throughput** (full suite of many files)? Different fixes; confirm which
  hurts.
- Acceptable to add an on-disk transpile cache (where would it live — a
  `.zero/` dir, target/, OS cache dir)?

## Done When

- A documented before/after timing breakdown exists in this folder.
- A measured, non-trivial reduction on the agreed-upon scenario, with no change
  to test results or semantics.
