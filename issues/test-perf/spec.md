# Spec: make `zero test` faster

> **Outcome (2026-05-31) — shipped as measurement only.** The "measure first"
> deliverable landed: a `ZERO_TEST_TIMING` per-phase instrumentation knob and a
> recorded `zero_demo` baseline (`measurements.md`). The measurement **refuted
> the optimization hypothesis** in this spec: redundant SWC transpile is only
> **1.5%** of wall time (84.9 ms of 5.5 s). The cost is dominated by `test-exec`
> (70%, genuine interpreter execution), `dom-shim` (16%), and `runtime-eval`
> (12%) — all per-context/per-test work that the single-threaded constraints
> below make irreducible (Boa modules are context-bound; contexts can't be
> shared). The transpile cache + string hoisting (the bulk of this spec) would
> have yielded ~1–2%, far short of the ~2.5 s target, so they were **not
> implemented**. The actual speedup — **parallel file execution**, listed as Out
> of Scope here — is re-spec'd as a fresh item:
> [`test-parallel`](../test-parallel/spec.md). The sections below are retained as
> the record of the investigated approach.

## Problem Statement

`zero test` is slow enough to hurt the inner loop. On the `zero_demo` project a
full run reports **259 passed, 0 failed, 0 skipped in 5.399s** across 41 test
files. That is not cold start — it is per-file overhead paid 41 times over. The
goal of this item is to cut full-suite wall-clock time substantially without
changing a single test result or any observable test semantics.

The current spec stance was "measure first, then fix." That still holds as the
implementation's first step, but the cause is now well enough understood from a
code read to commit to a direction: **eliminate redundant per-file work in a
single-threaded runner.** Parallelism and on-disk caching are explicitly
deferred (see Out of Scope).

## Background

### How a run is structured today

`crates/zero/src/cmd/test.rs` discovers files and runs them **sequentially**:

```
for f in &files {
    let outcome = run_file_with_coverage(&root, f, cov_ctx.clone());
    ...
}
```

Each file is handled by `run_with_loader_inner` in
`crates/zero-test-runner/src/harness.rs`, which for **every file**:

1. Builds a fresh Boa `Context` (`build_js_context`).
2. Installs `console` and `__readWorkspaceFile__` globals.
3. Re-parses and re-evaluates the entire DOM/web-platform shim blob
   (`eval_dom_shim` over `ZERO_DOM_SHIM_BODY`).
4. Constructs a fresh `ZeroModuleLoader`, whose `new()` calls `runtime_module()`,
   `test_module()`, and `http_module()` to build the runtime source strings.
5. On first import of `"zero"` / `"zero/test"` / `"zero/http"`, **re-parses**
   each of those runtime module strings into the new context (Boa `Module`s are
   context-bound and cannot be shared across contexts).
6. Transpiles the test file and every `src/` module it imports via SWC
   (`transpile_typescript` / coverage `instrument`), then parses each into the
   context. The loader's `module_cache` lives only for this one context, so a
   `src/` module imported by N different test files is **re-transpiled and
   re-parsed N times** across the run.
7. After the test loop, **leaks** the context with `std::mem::forget` to avoid
   Boa 0.21's `MapLock::finalize` panic (`BorrowMutError` during GC of a still-
   locked `Map`; see FRAMEWORK_NOTES #63 and the [[boa_maplock_finalizer]]
   memory).

### Why the cost is per-file, not cold start

There is no warm pool and no daemon; the only one-time cost is process/engine
startup, which is paid once. Everything in steps 1–6 above is paid **per file**.
With 41 files, the dominant terms are (a) building + tearing-up 41 contexts,
(b) 41 re-evaluations of the shim blob, (c) 41 re-parses of the runtime module
strings, and (d) redundant SWC transpiles of shared `src/` modules. This is a
throughput problem, and it scales linearly with file count.

### Hard constraints the architecture imposes

- **Per-file context isolation is load-bearing.** The runtime uses module-level
  mutable state (`_currentApp`, `_observerStack`, `_activeScope`, …). Tests
  within a file share one context and rely on `cleanup()`; tests across files
  must not see each other's state. A fresh context per file is the isolation
  contract — sharing one context across files is **not** an option.
- **The context must be leaked, not dropped.** Dropping/`force_collect`-ing runs
  the buggy Boa finalizer and can abort the whole process. Any optimization must
  preserve the `std::mem::forget` (or an equivalent that never runs the Map
  finalizer). The ~12 MB/file leak is accepted.
- **Boa `Context` is `!Send`.** Cross-thread parallelism would require a
  worker-thread pool with thread-local contexts. That is deferred (see Out of
  Scope) — this slice stays single-threaded.

### Where the single-threaded wins are

Given the constraints, the redundant work that can be removed without touching
isolation or the leak:

- **SWC transpile of `src/` modules** is the same input producing the same
  output for every file that imports it. A process-wide, in-memory cache keyed
  on a content hash of the source (and the relevant transpile options) lets the
  loader skip SWC on repeat imports across files. The cache stores the
  transpiled JS string (and source map, where emitted), not a Boa `Module` —
  the string is context-independent and safe to reuse; the `Module` still has to
  be parsed per context.
- **Runtime source strings** (`runtime_module()`, `test_module()`,
  `http_module()`, and the DOM shim body) are recomputed in every
  `ZeroModuleLoader::new`. They are constant for a run and can be built once and
  shared by reference, removing 41× string concatenation. (The Boa `Module`
  parse of these still happens per context and cannot be cached across contexts;
  only the string construction is hoisted.)
- **Coverage instrumentation** has the same redundancy as transpile when
  `--coverage` is on, but its output must be tallied per run, not per file — the
  plan must be careful that caching instrumented output does not double-count or
  drop coverage maps. The simplest correct scope is: cache only the
  non-instrumented transpile path; leave the coverage path as-is unless the
  measurement shows it matters.

## Requirements

### Measure first (first deliverable)

- Instrument the run with timers around the major phases: discovery, per-file
  context build + global install, DOM-shim eval, runtime-module parse, per-file
  transpile (cumulative), and test execution. The breakdown may be surfaced
  behind a debug/verbose path (e.g. an env var or hidden flag) rather than
  default output — the plan decides — but it must be reproducible.
- Run it against `zero_demo` (41 files / 259 tests) and record a phase
  breakdown of the 5.4s baseline in this folder (e.g. `issues/test-perf/`),
  confirming where the time goes before any optimization lands.

### Single-threaded optimizations

- **In-memory transpile cache, per-run only.** Add a process-wide (single-run)
  cache so a `src/` module imported by multiple test files is transpiled by SWC
  at most once per run. Key on a content hash of the source plus the transpile
  options that affect output. Store the transpiled JS (and source map when
  emitted). No on-disk artifacts; the cache lives for the duration of one
  `zero test` invocation and is dropped at process exit.
  - Correctness: cached output must be byte-identical to what an uncached
    transpile would produce for the same input. Source maps must still map to
    the original `.ts` so failure locations remain correct.
  - The cache must not leak coverage-instrumented output into the
    non-instrumented path or vice versa (distinct cache keys, or coverage path
    excluded from the cache).
- **Hoist constant runtime-source construction.** Build the runtime module
  strings (and the DOM shim body reference) once per run and share them across
  every per-file loader/context, instead of rebuilding them in each
  `ZeroModuleLoader::new`.
- Any other redundant per-file work the measurement surfaces as material is
  fair game, provided it does not violate the isolation or leak constraints.

### Correctness / no-semantics-change (hard requirement)

- After the change, `zero test` on `zero_demo` must still report **259 passed,
  0 failed, 0 skipped** (modulo wall-clock time). Pass/fail/skip counts, test
  ordering within a file, failure messages, and source-mapped failure locations
  must be unchanged.
- `cargo test --workspace` and `cargo test --workspace -- --include-ignored`
  must stay green.
- `--coverage` output (table + `coverage/coverage.json`) must be unchanged when
  enabled.

### Target

- On the `zero_demo` benchmark, roughly **halve** full-suite wall-clock time:
  from ~5.4s to **~2.5s or better**, measured the same way before and after.
- Record the before/after numbers in this folder so the win is documented and a
  future regression is visible.

### Documentation

- This slice adds **no new CLI flags, no `zero.toml` config, and no change to
  observable behavior** (same results, same output, just faster). It is purely
  an internal performance change, so **no `docs/*.md` update is required.**
  This is called out explicitly so the plan phase records that docs were
  considered, not forgotten. If the plan ends up introducing a user-visible
  surface after all (e.g. a `--timing` flag promoted to public), it must add the
  corresponding entry to `docs/config-and-cli.html` (source `docs/config-and-cli.md`).

## Constraints

- **Single-threaded.** No worker pool, no `--threads`, no concurrent file
  execution in this slice.
- **In-memory only.** No on-disk transpile cache, no cache directory, no
  `.gitignore` entry, no cross-run persistence.
- **Per-file context isolation preserved.** A fresh Boa context per file; tests
  across files never share state.
- **The `std::mem::forget` leak stays.** No change that re-introduces the Boa
  `MapLock` finalizer panic. Memory growth per file remains acceptable.
- **No engine swap, no runtime rewrite.** Boa stays; the JS semantics the
  harness exposes do not change.
- **No change to discovery semantics, target-filter behavior, or reporter
  format** beyond optional debug timing output.

## Out of Scope

- **Parallel file execution** (worker-thread pool with thread-local Boa
  contexts, à la `zero mutate --threads`). This is the largest remaining lever
  and a natural follow-up, but it adds output-buffering and determinism
  concerns and is deferred to keep this slice focused.
- **On-disk / cross-run transpile cache.** Deferred; revisit if repeat-run
  latency (not full-suite latency) becomes the pain point.
- **Sharing a Boa context across files.** Ruled out by the isolation contract.
- **Reducing or fixing the per-file memory leak.** The real fix is upstream in
  Boa; not this slice's concern.
- **Reworking coverage or mutation testing performance.** `zero mutate` already
  has its own parallelism story; this item is about `zero test`.

## Open Questions

- **Transpile-cache key.** Content hash of source is the obvious key, but the
  plan must confirm which `TranspileOptions` fields affect output and fold them
  into the key (filename affects the emitted source-map `sources` entry, so the
  cache may need to key on path too, or store the map keyed separately). Resolve
  during planning.
- **Where the shared cache and hoisted runtime strings live.** Options: a
  per-run struct threaded from `cmd/test.rs` into each loader, or a process-wide
  `OnceLock`/thread-local. Plan picks; must respect that `cmd/mutate.rs` also
  constructs loaders and should not be silently changed in behavior.
- **Does the measurement justify more than transpile + string hoisting?** If the
  breakdown shows context-build or shim-eval dominates (not transpile), the
  ~2.5s target may need an additional lever within single-threaded scope. The
  plan should leave room to act on the breakdown rather than assuming transpile
  is the whole story.
