# Spec: parallel `zero test` execution

## Problem Statement

`zero test` is slow enough to hurt the inner loop: on `zero_demo` a full run is
**259 passed, 0 failed, 0 skipped in ~5.5s** across 41 test files. The
[`test-perf`](../test-perf/spec.md) slice instrumented the run
(`ZERO_TEST_TIMING`) and measured where the time goes
([`test-perf/measurements.md`](../test-perf/measurements.md)). The verdict:

| Phase          | Share | Removable single-threaded? |
|----------------|------:|----------------------------|
| test-exec      | 70%   | No — genuine interpreter execution of test bodies/hooks |
| dom-shim       | 16%   | No — per-context shim eval (Boa modules are context-bound) |
| runtime-eval   | 12%   | No — per-context parse/eval of runtime + entry modules |
| transpile      | 1.5%  | Partially — but only 85 ms total |
| context-build  | 0.6%  | No |

~98% of the run is **inherent per-file/per-test work** that cannot be removed
without sharing contexts (ruled out by the isolation contract). The one lever
that moves the dominant cost is running files **in parallel** across worker
threads, each with its own thread-local Boa context. This was explicitly
deferred by `test-perf`; this item picks it up.

## Background

- `crates/zero/src/cmd/test.rs::run` executes discovered files **sequentially**
  in a `for f in &files` loop, calling `run_file_with_coverage` per file.
- Each file already runs in a **fresh, isolated Boa `Context`** built inside
  `harness::run_with_loader_inner` and leaked via `std::mem::forget` at the end
  (Boa 0.21 `MapLock::finalize` panic; see `boa_maplock_finalizer` /
  FRAMEWORK_NOTES #63). Files therefore share **no** JS state today — the
  isolation a thread pool needs is already in place.
- **`Context` is `!Send`.** Parallelism must use a worker-thread pool where each
  thread owns its contexts and never moves a `Context` across threads — the same
  pattern `zero mutate --threads` already uses (`crates/zero/src/cmd/mutate.rs`).
- The reporter (`crates/zero-test-runner/src/reporter.rs`) currently streams
  per-file results to stdout in discovery order as they complete.

## Requirements

### Parallel execution
- Run discovered test files across a worker-thread pool, each worker holding a
  thread-local Boa context per file (files remain fully isolated). Default to a
  sane parallelism (e.g. `min(cores, N)`); allow `--threads <n>` with `1` forcing
  sequential, mirroring `zero mutate`'s flag and defaults.
- The `ZERO_TEST_TIMING` accumulator is currently a thread-local; under the pool
  it must aggregate across worker threads (sum per-phase totals/counts) before
  printing, or the breakdown will undercount.

### Correctness / no-semantics-change (hard requirement)
- `zero test` on `zero_demo` must still report **259 passed, 0 failed, 0
  skipped**. Pass/fail/skip counts, failure messages, and source-mapped failure
  locations unchanged.
- **Deterministic output.** Per-file results must be reported in a stable order
  (discovery order) regardless of completion order — buffer per-file output and
  emit in order, or sort before the final summary. No interleaved/garbled lines.
- `--coverage` output (table + `coverage/coverage.json`) must be unchanged: the
  per-file `CoverageContext` maps must be aggregated across threads without loss
  or double-counting.
- The `std::mem::forget` leak and per-file context isolation are preserved.
- `cargo test --workspace -- --include-ignored` stays green.

### Target
- On `zero_demo`, cut full-suite wall-clock substantially (the original ~2.5 s
  goal). Since ~98% of work is parallelizable test execution, wall time should
  approach `total / threads` plus a serial tail. Record before/after in this
  folder.

## Constraints
- No engine swap; Boa stays. No sharing a `Context` across files or threads.
- No change to discovery semantics, target-filter behavior, or (ordered)
  reporter format beyond what determinism requires.
- The per-file leak stays; do not re-introduce the Boa `MapLock` finalizer panic.

## Out of Scope
- On-disk / cross-run transpile caching.
- Sharing a Boa context across files.
- Reworking coverage or mutation-testing internals beyond cross-thread
  aggregation needed for correctness.

## Open Questions
- **Thread pool vs. `mutate` reuse.** Can the worker-pool plumbing from
  `cmd/mutate.rs` be factored into a shared helper, or is a separate, simpler
  pool warranted for `test`? Resolve during planning.
- **Output buffering granularity.** Stream-in-order with a reorder buffer, or
  collect all `FileResult`s and render at the end? The latter is simpler but
  loses incremental feedback on long runs. Plan picks.
- **Timing aggregation.** Move the `timing` accumulator from thread-local to a
  cross-thread merge (workers push their snapshot to a shared sink on finish),
  or keep thread-local and sum collected snapshots. Plan picks.
- **`--watch` interaction.** Does parallelism complicate the existing watch loop?
  Confirm the watch path still works (or is sequential) under the pool.
