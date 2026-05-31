# `zero test` performance measurements

Benchmark project: `zero_demo` (41 test files / 259 tests).
Binary: `cargo build -p zero --release`; run from the `zero_demo` root.
Timing: `ZERO_TEST_TIMING=1 zero test` (per-phase breakdown printed to stderr).

## Baseline (after Step 1 instrumentation, before any optimization)

Wall clock (reporter line): **259 passed, 0 failed, 0 skipped in 5.501s**
(spec recorded 5.399s on an earlier run; same order of magnitude).

Per-phase breakdown (cumulative across all 41 files):

| Phase          | Total      | Calls | Share  |
|----------------|-----------:|------:|-------:|
| discovery      |    0.7 ms  |     1 |  0.0%  |
| context-build  |   35.2 ms  |    41 |  0.6%  |
| dom-shim       |  869.3 ms  |    41 | 15.6%  |
| runtime-eval   |  675.4 ms  |    41 | 12.1%  |
| transpile      |   84.9 ms  |   418 |  1.5%  |
| test-exec      | 3898.4 ms  |    41 | 70.1%  |
| **sum**        | **5563.9 ms** |    |        |

Notes:
- `transpile` (418 calls) = 41 entry files + 377 `src/` module imports, run
  through SWC. **It totals only 84.9 ms — 1.5% of wall time.**
- `test-exec` is the actual execution of test bodies + hooks (`render`,
  `cleanup`, matcher work) in the Boa interpreter. It dominates at 70%.
- `dom-shim` (re-evaluating the shim blob per context) and `runtime-eval`
  (parse + evaluate of the entry module and the runtime modules it pulls in,
  per context) are the next-largest terms at 15.6% and 12.1%.

## Finding: the plan's target lever (transpile cache) addresses ~1.5% of cost

The plan assumed "redundant SWC transpile of shared `src/` modules" was the
"dominant removable waste" and targeted ~5.4s → ~2.5s via an in-memory
transpile cache. The measurement contradicts this:

- **Transpile is 84.9 ms total.** Even a perfect cross-file cache (deduping the
  377 `src/` imports to their unique modules) can only remove a fraction of
  85 ms — on the order of tens of milliseconds. Hoisting the constant runtime
  strings saves only string construction, which is negligible next to the
  per-context parse/eval those strings feed.
- The dominant costs — `test-exec` (70%), `dom-shim` (16%), `runtime-eval`
  (12%) — are all **per-context inherent work** under the file-isolation
  contract. Boa modules are context-bound, so the shim and runtime modules must
  be re-parsed/re-evaluated per file; that re-eval cannot be cached across
  contexts (spec §"Hard constraints"). `test-exec` is genuine interpreter time
  running the test bodies.

This is the exact contingency the spec flagged as an open question ("Does the
measurement justify more than transpile + string hoisting? If the breakdown
shows context-build or shim-eval dominates …") and the plan listed under Risks
("Transpile may not be the dominant cost"). Here the residual is dominated by
`test-exec` — even more fundamental than setup overhead.

**Implication:** the planned single-threaded optimizations (transpile cache +
string hoisting) are still worth doing as spec-required hygiene, but they
**cannot reach the ~2.5s target.** The only lever that addresses the dominant
costs is parallel file execution (thread-local Boa contexts), which the spec
explicitly places **out of scope** for this slice.

## Investigation: is there a cheaper win hiding inside `test-exec`?

To rule out a hidden hotspot (e.g. a `run_jobs` busy-wait on pending promises),
`test-exec` was temporarily split into its synchronous-call and job-draining
parts (throwaway instrumentation, since reverted):

| Sub-phase  | Total      | Calls | Meaning                                  |
|------------|-----------:|------:|------------------------------------------|
| call-body  | 3068.8 ms  |   559 | synchronous `fn.call()` of it-bodies + hooks |
| run-jobs   |  919.6 ms  |    63 | `ctx.run_jobs()` draining async microtasks |
| (sum)      | 3988.4 ms  |       | ≈ `test-exec` total                      |

Findings:
- The two sub-phases sum to the `test-exec` total — no unaccounted time.
- **`run_jobs` is not a busy-wait.** Only 63 calls total (≈9 per async test
  across the 7 `async` files; **0 files use `setTimeout`**). A pathological spin
  would show thousands of calls. The 920 ms is genuine async microtask
  execution, not idle looping.
- **`call-body` (3069 ms / 559 calls)** is genuine synchronous interpretation of
  test bodies and hooks (component `render`, store mutations, matcher work) in
  the Boa interpreter. Average ~5.5 ms/call.

Conclusion: `test-exec` is irreducible single-threaded interpreter work. There
is no cheap structural win hiding in it. Combined with `dom-shim` + `runtime-eval`
(per-context parse/eval, ~28%, constrained by the isolation contract), ~98% of
the run is inherent per-file/per-test cost. **Parallel file execution is the
only lever that meaningfully moves the number, and it is out of scope here.**
