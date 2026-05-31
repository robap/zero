# Plan: make `zero test` faster

## Summary

`zero test` pays per-file overhead 41 times over on `zero_demo` (259 tests /
~5.4s). The dominant removable waste is redundant work in the module loader:
every test file gets its own Boa context with its own `ZeroModuleLoader`, so any
`src/` module imported by N test files is read from disk and run through SWC N
separate times, and the constant runtime source strings (`runtime_module()` etc.)
are rebuilt per loader. This plan stays **single-threaded** and attacks that
redundancy: (1) add per-run timing instrumentation and capture a baseline
breakdown; (2) introduce a process-lifetime, in-memory transpile/read cache plus
a shared `RunCache` holding the constant runtime strings; (3) thread that
`RunCache` from `cmd/test.rs` through the harness into the loader, consulting the
cache on the non-coverage resolve path; (4) re-measure, document before/after,
and verify no change to results. Parallelism and on-disk caching are out of scope
(see spec). Target: ~5.4s → ~2.5s on `zero_demo`.

## Prerequisites

None. The spec's open questions are resolved within the plan:
- **Cache key:** canonical path string (within one short-lived run, a path's
  content is stable), scoped to the **non-coverage** resolve path only. The
  entry test file (`prepare_source`) is never cached — it is unique per file and
  carries a per-file source map that must stay intact.
- **Where the cache lives:** an `Rc<RunCache>` built once in `cmd/test.rs::run`
  and threaded explicitly into each loader. `mutate.rs` is left untouched (passes
  no `RunCache`), so its behavior is byte-identical to today.
- **More than transpile?** Step 4 gates on the Step 1 breakdown; if context-build
  / shim-eval dominates instead of transpile, the contingency in Risks applies.

## Steps

- [x] **Step 1: Per-run timing instrumentation + baseline measurement**
- [ ] ~~**Step 2: `TranspileCache` type**~~ — **dropped** (see Outcome below)
- [ ] ~~**Step 3: `RunCache` + thread shared strings and cache into the loader**~~ — **dropped**
- [ ] ~~**Step 4: Re-measure, document before/after, verify no semantics change**~~ — **dropped**

> **Outcome (2026-05-31):** Step 1 shipped the `ZERO_TEST_TIMING` instrumentation
> and the baseline measurement (`measurements.md`). That baseline **invalidated
> the premise of Steps 2–4**: SWC transpile is only **1.5%** of the run (84.9 ms
> of 5.5 s), while `test-exec` (70%), `dom-shim` (16%), and `runtime-eval` (12%)
> dominate — all irreducible per-context/per-test interpreter work under the
> file-isolation contract. A follow-up investigation confirmed `test-exec` has no
> hidden hotspot (no `run_jobs` busy-wait). The transpile cache + string hoisting
> would have moved ~1–2%, nowhere near the ~2.5 s target. Per the spec's own
> open question and the plan's Risks contingency, the optimization was **descoped
> here** and re-homed to a fresh, parallelism-based item:
> [`test-parallel`](../test-parallel/spec.md). Steps 2–4 are intentionally not
> implemented.

---

## Step Details

### Step 1: Per-run timing instrumentation + baseline measurement

**Goal:** Make the cost breakdown reproducible before changing anything, so the
optimization targets real bottlenecks and the before/after win is documented.
This is the spec's "measure first" deliverable. It ships behind the
`ZERO_TEST_TIMING` env var (off by default) — a documented debug knob.

**Files:**
- `crates/zero-test-runner/src/timing.rs` (new)
- `crates/zero-test-runner/src/lib.rs` (add `pub mod timing;`)
- `crates/zero-test-runner/src/harness.rs` (instrument phases)
- `crates/zero/src/cmd/test.rs` (reset at start, instrument discovery, print at end)
- `docs/config-and-cli.md` (document the `ZERO_TEST_TIMING` env var)
- `issues/test-perf/measurements.md` (new — record the baseline)

**Changes:**
- New `timing` module with a thread-local accumulator (the runner is
  single-threaded, so thread-local is sufficient and lock-free):
  ```rust
  #[derive(Clone, Copy, PartialEq, Eq, Hash)]
  pub enum Phase { Discovery, ContextBuild, DomShim, RuntimeEval, Transpile, TestExec }
  ```
  - `pub fn enabled() -> bool` — reads `ZERO_TEST_TIMING` once via a
    `OnceLock<bool>` (set when the var is present and non-empty).
  - `pub fn add(phase: Phase, d: Duration)` — no-op when `!enabled()`; otherwise
    adds to the thread-local total and bumps a per-phase call count.
  - `pub fn reset()` and `pub fn snapshot() -> Vec<(Phase, Duration, u64)>`.
  - `Phase::label(&self) -> &'static str` for printing.
- In `harness.rs::run_with_loader_inner`, wrap each phase with
  `Instant::now()` / `timing::add(...)`:
  - `ContextBuild` around `build_js_context` + the two `install_*` calls.
  - `DomShim` around `eval_dom_shim`.
  - `RuntimeEval` around `parse_test_module` + `evaluate_module`
    (covers the per-context parse of the entry module and the runtime modules it
    pulls in).
  - `TestExec` around the `walk_describe` call.
- In `harness.rs::prepare_source` and `loader.rs::resolve_relative`, wrap the
  `transpile_typescript` / `coverage::instrument` calls with `Transpile` timing.
  (The loader already depends on the crate, so it can call `crate::timing`.)
- In `cmd/test.rs::run`: call `timing::reset()` before discovery; wrap `discover`
  with `Discovery` timing; after `reporter.finish()`, if `timing::enabled()`,
  print the snapshot table (label, total ms, call count) to **stderr** so it
  never pollutes the reporter's stdout.
- **Docs:** add a short subsection under `zero test [pattern]` in
  `docs/config-and-cli.md` (the section at ~line 149) documenting that setting
  `ZERO_TEST_TIMING=1` prints a per-phase timing breakdown to stderr after the
  run — a diagnostic for investigating slow suites — and that it does not change
  test output or results. (No env vars are documented in the docs today; this is
  the first, so introduce it inline in that subsection rather than a new global
  "Environment variables" section.)

**Tests:**
- `timing.rs` unit tests: `add` accumulates per phase and counts calls;
  `snapshot` returns the totals; `reset` clears. Test the env parsing via a small
  pure helper `fn parse_enabled(v: Option<&str>) -> bool` (avoids mutating real
  env in tests).
- Manual: run `ZERO_TEST_TIMING=1 zero test` in `zero_demo`, paste the phase
  breakdown of the ~5.4s baseline into `issues/test-perf/measurements.md` with
  the total and the discovered file/test counts.
- `cargo test --workspace` stays green (instrumentation is additive, off by
  default).

### Step 2: `TranspileCache` type

**Goal:** A standalone, unit-tested, in-memory cache that maps a key to a shared
transpiled-source string, computing the value at most once. No wiring yet, so it
lands independently and compiles/tests clean.

**Files:**
- `crates/zero-test-runner/src/transpile_cache.rs` (new)
- `crates/zero-test-runner/src/lib.rs` (add `pub mod transpile_cache;`)

**Changes:**
- ```rust
  pub struct TranspileCache { map: RefCell<HashMap<String, Rc<str>>> }
  impl TranspileCache {
      pub fn new() -> Self
      /// Return the cached source for `key`, or compute it via `f`, store it,
      /// and return it. `f` runs at most once per key for the cache's lifetime.
      pub fn get_or_try_insert<F, E>(&self, key: &str, f: F) -> Result<Rc<str>, E>
      where F: FnOnce() -> Result<String, E>;
      pub fn len(&self) -> usize;       // for tests/metrics
      pub fn contains(&self, key: &str) -> bool;
  }
  ```
  - `get_or_try_insert`: borrow-check the map; on hit return the cloned `Rc<str>`
    (cheap); on miss run `f`, `Rc::from(String)`, insert, return. Do **not** hold
    the `RefCell` borrow across `f()` (drop the immutable borrow before calling
    `f`, take a mutable borrow only to insert) to avoid a borrow panic if `f`
    re-enters — it won't here, but keep the invariant clean.
- `Rc<str>` (not `Arc`) because the runner is single-threaded and the value is
  immutable shared text.

**Tests:**
- Miss computes once: a closure with a `Cell<u32>` counter; two
  `get_or_try_insert` calls with the same key invoke the closure exactly once and
  return equal contents.
- Distinct keys are independent; `len()` reflects unique inserts.
- Error path: a closure returning `Err` does **not** populate the cache
  (`contains` is false afterward) and a subsequent successful call computes and
  caches.

### Step 3: `RunCache` + thread shared strings and cache into the loader

**Goal:** Build the constant runtime strings and the transpile cache **once per
run** and share them across every per-file loader, and consult the cache (and
skip the redundant `fs::read_to_string`) on the non-coverage resolve path. This
single piece of plumbing satisfies both the transpile-cache requirement and the
"hoist constant runtime-source construction" requirement — they share the same
shared object and are inseparable.

**Files:**
- `crates/zero-test-runner/src/loader.rs` (RunCache, loader fields/ctors, cache consult)
- `crates/zero-test-runner/src/harness.rs` (`run_file_with_cache` entry)
- `crates/zero/src/cmd/test.rs` (build one `RunCache`, pass into each run)

**Changes:**

1. **`RunCache` (in `loader.rs`):**
   ```rust
   pub struct RunCache {
       runtime_src: Rc<str>,
       test_src: Rc<str>,
       http_src: Rc<str>,
       transpile: Rc<TranspileCache>,
   }
   impl RunCache {
       pub fn new() -> Rc<Self> {
           Rc::new(Self {
               runtime_src: Rc::from(runtime_module()),
               test_src: Rc::from(test_module()),
               http_src: Rc::from(http_module()),
               transpile: Rc::new(TranspileCache::new()),
           })
       }
   }
   ```

2. **Loader fields:** change `runtime_src`/`test_src`/`http_src` from `String`
   to `Rc<str>` (deref to `str`, so existing `self.runtime_src.as_bytes()` call
   sites are unchanged). Add `transpile_cache: Option<Rc<TranspileCache>>`.
   - `ZeroModuleLoader::new(root)`: build the three strings fresh into `Rc<str>`
     (`Rc::from(runtime_module())`, …) and set `transpile_cache: None` — this
     keeps `mutate.rs` and the internal `run_file` path behaving exactly as today
     (own strings, no shared cache).
   - `new_with_coverage(root, ctx)`: unchanged behavior (delegates to `new`,
     sets coverage).
   - Add `new_with_run_cache(root, rc: &RunCache)`: clone `rc`'s three `Rc<str>`
     and set `transpile_cache: Some(rc.transpile.clone())`.
   - Add `new_with_run_cache_and_coverage(root, rc, cov)`: as above plus
     `coverage = Some(cov)`.

3. **Cache consult in `resolve_relative`:** today the method reads the file then
   branches `instrument | transpile-ts | raw-js` to compute `src`. Restructure:
   - Keep the existing `module_cache` (per-context Boa `Module`) check and the
     `overlay` short-circuit **first**, unchanged.
   - Compute `instrument_cov` as today. The cache is used **only when
     `instrument_cov.is_none()` and `self.transpile_cache.is_some()`**:
     ```rust
     let src: Rc<str> = match (&instrument_cov, &self.transpile_cache) {
         (None, Some(cache)) => cache.get_or_try_insert(&key, || {
             read_and_transpile_plain(&canonical) // fs::read + (ts→transpile | js→raw)
         })?,
         _ => Rc::from(/* existing inline path: read + instrument|transpile|raw */),
     };
     ```
   - Factor the plain read+transpile into `fn read_and_transpile_plain(canonical:
     &Path) -> JsResult<String>` (the `.ts` → `transpile_typescript` with
     `emit_source_map:false, inline_source_map:false`, `.js`/other → raw read).
     This is exactly the current non-coverage logic, just hoisted so the cache
     closure can own the `fs::read_to_string` (so a cache hit skips the read too).
   - The coverage/instrument branch keeps reading and instrumenting inline
     (uncached), recording its `CoverageMap` per `CoverageContext` as today.
   - After computing `src`, the rest is unchanged: `Module::parse(src.as_bytes())`,
     insert into `module_cache`, insert into `path_map` (so `loaded_paths()` and
     mutate test-impact are unaffected).
   - **Do not** touch `resolve_components_index` or the bare-specifier
     (`zero`/`zero/test`/`zero/http`) branches — those serve pre-built strings
     and are parse-only.

4. **Harness entry (`harness.rs`):** add a cache-aware entry and keep the old one
   as a thin delegate so `mutate.rs` and existing tests are untouched:
   ```rust
   pub fn run_file_with_cache(
       project_root: &Path, file_abs: &Path,
       coverage: Option<Rc<CoverageContext>>, run_cache: Option<Rc<RunCache>>,
   ) -> RunOutcome {
       let want_coverage = coverage.is_some();
       let loader = match (coverage, &run_cache) {
           (Some(c), Some(rc)) => Rc::new(ZeroModuleLoader::new_with_run_cache_and_coverage(project_root, rc, c)),
           (Some(c), None)     => Rc::new(ZeroModuleLoader::new_with_coverage(project_root, c)),
           (None, Some(rc))    => Rc::new(ZeroModuleLoader::new_with_run_cache(project_root, rc)),
           (None, None)        => Rc::new(ZeroModuleLoader::new(project_root)),
       };
       run_with_loader(project_root, file_abs, loader, want_coverage)
   }
   pub fn run_file_with_coverage(root, file, cov) -> RunOutcome {
       run_file_with_cache(root, file, cov, None)   // unchanged signature/behavior
   }
   ```
   Re-export `run_file_with_cache` and `RunCache` from `lib.rs` as needed.

5. **`cmd/test.rs::run`:** build `let run_cache = RunCache::new();` once before the
   loop; in the loop call
   `run_file_with_cache(&root, f, cov_ctx.clone(), Some(run_cache.clone()))`.
   Everything else (coverage aggregation, reporter, exit code) unchanged. The
   non-coverage benchmark path now shares one cache across all files.

**Tests:**
- `loader.rs` unit test — cross-file dedup: build one `RunCache`; create **two**
  separate contexts, each with a loader from `new_with_run_cache(root,
  &run_cache)` sharing `run_cache.transpile`; in each, resolve the same relative
  `./foo.ts`; assert `run_cache.transpile.len() == 1` and both evaluations
  succeed. This proves the same `src/` module is transpiled once across files.
- `loader.rs` unit test — no cache without RunCache: a loader from `new(root)` has
  `transpile_cache == None` and still resolves `./foo.ts` correctly (today's
  path), so `mutate`/internal callers are unaffected.
- `loader.rs` correctness test — cached output equals uncached: resolve a `.ts`
  file once through the cached path and once through a fresh uncached loader;
  assert the evaluated export value is identical (cache returns byte-identical
  transpile output).
- `harness.rs` test — `run_file_with_cache(root, file, None, Some(RunCache::new()))`
  produces the same outcomes as `run_file(root, file)` for a sample file
  importing a shared module (results unchanged with the cache on).
- Existing `cmd/test.rs` tests (`missing_zero_toml_runs_with_defaults`,
  `coverage_true_writes_coverage_json`, `no_config_skips_dist_and_build`, …) stay
  green — they exercise the wired path.
- `cargo test --workspace` green.

### Step 4: Re-measure, document before/after, verify no semantics change

**Goal:** Confirm the target and pin the result. This closes the spec's "Done
When": a documented before/after on the benchmark, a non-trivial reduction, and
identical results.

**Files:**
- `issues/test-perf/measurements.md` (append after-numbers + phase breakdown)

**Changes:**
- Run `zero test` in `zero_demo` and confirm it still reports **259 passed, 0
  failed, 0 skipped**; record wall-clock before (5.399s) and after.
- Run `ZERO_TEST_TIMING=1 zero test` in `zero_demo` again; append the post-change
  phase breakdown next to the baseline so the transpile-phase drop is visible.
- State the speedup vs the ~2.5s target. If the target is met, done. If the
  breakdown shows the residual is dominated by `ContextBuild` / `DomShim` (per
  the Risks contingency), record the finding and the recommended follow-up rather
  than expanding scope here.

**Tests / gates:**
- `cargo test --workspace -- --include-ignored` green (full suite incl. the slow
  `e2e_*`, `examples_*`, `test_runner_smoke`, `component_library` integration
  tests that exercise `zero test` end-to-end).
- `cargo test -p zero-test-runner` and `cargo test -p zero` green.
- `--coverage` spot check: `zero test --coverage` in a project still writes
  `coverage/coverage.json` with unchanged numbers (the instrument path was left
  uncached).

## Documentation

One docs change: the `ZERO_TEST_TIMING` env var is documented under the
`zero test [pattern]` section of `docs/config-and-cli.md` (handled in Step 1,
where the var is introduced). It is a diagnostic debug knob, off by default, that
does not affect test output or results.

The optimizations themselves (transpile cache, shared runtime strings) add **no
CLI flag, no `zero.toml` config, and no change to observable behavior** — same
results, same reporter output, just faster — so they need no further docs. The
default `zero test` experience is unchanged.

## Risks and Assumptions

- **Transpile may not be the dominant cost.** The plan assumes a meaningful share
  of the 5.4s is redundant SWC transpile of shared `src/` modules. If Step 1's
  breakdown shows `ContextBuild` (Boa intrinsics init per file) or `DomShim`
  (re-eval per file) dominates, the cache alone may not reach ~2.5s. Those costs
  are inherent to the per-file-context isolation contract and cannot be removed
  single-threaded without sharing contexts (ruled out) — so the contingency is to
  document the finding in `measurements.md` and flag parallelism (the deferred
  lever) as the follow-up, rather than violating a constraint. This is the
  spec's acknowledged risk.
- **Path-keyed caching correctness.** Keying on canonical path is safe only
  because a `zero test` run is short-lived and files do not change mid-run. If a
  future feature edits sources during a run (e.g. a `--watch` mode), the cache
  must switch to content-hash keying or be invalidated per change. Noted for the
  deferred watch slice; not a concern now.
- **Entry file must stay uncached.** `prepare_source` transpiles the unique entry
  test file with a per-file source map used for failure locations. The plan
  deliberately does **not** cache it; caching it would risk cross-file source-map
  bleed. The cache is confined to `resolve_relative`'s non-coverage branch.
- **`Rc<str>` field-type change ripple.** Switching the loader's
  `runtime_src/test_src/http_src` to `Rc<str>` relies on `Deref<Target=str>` so
  existing `.as_bytes()` uses keep compiling. If any call site moved/owned the
  `String`, it needs a `.to_string()`/clone — caught at compile time.
- **Coverage path intentionally unoptimized.** `--coverage` runs see little
  benefit because instrumented sources are uncached by design (to avoid
  double-counting maps). Acceptable: the benchmark and common path is plain
  `zero test`; the spec scoped coverage performance out.
- **`mutate.rs` untouched.** The plan adds a delegating `run_file_with_coverage`
  and leaves mutate's `ZeroModuleLoader::new(...).with_overlay(...)` path exactly
  as-is, so no silent change to mutation testing. Verified by `mutate`'s existing
  tests in the full-suite gate.
