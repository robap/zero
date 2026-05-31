# QuickJS vs Boa — `zero test` timing comparison

The decision input for the Step 5 cutover gate (per `plan.md` / `spec.md`): the
bar is **no test-outcome regression AND a measurable single-threaded wall-clock
speedup on `zero_demo`**, with the user making the final call.

## Methodology

- **Binary:** single release build with `--features engine-quickjs`
  (`cargo build -p zero --release --features engine-quickjs`). The engine is
  selected at runtime via `ZERO_ENGINE` — unset = Boa (default dispatch),
  `quickjs` = rquickjs. Using one binary holds everything else constant; the
  Boa code path executed is identical to a default build.
- **Workload:** `../zero_demo` (`zero.toml` project), 41 test files / 259 `it()`
  bodies. Identical transpiled input for both engines.
- **Mode:** single-threaded (the runner is single-threaded; `test-parallel` is a
  separate item).
- **Timing:** `ZERO_TEST_TIMING=1` for the per-phase breakdown; the reporter's
  own `in X.XXXs` wall clock for the totals. Median of 3 runs.
- **Date:** 2026-05-31.

## Outcome parity (hard gate — must be zero regression)

| Engine   | passed | failed | skipped |
|----------|--------|--------|---------|
| Boa      | 259    | 0      | 0       |
| QuickJS  | 259    | 0      | 0       |

`--coverage` output is **byte-for-byte identical**: the terminal table totals
match (`640/665` lines, `163/227` fns, `90.0%`) and `web/coverage/coverage.json`
is `diff`-identical between the two engines.

## Wall-clock (median of 3)

| Engine   | run 1   | run 2   | run 3   | median  |
|----------|---------|---------|---------|---------|
| Boa      | 5.607s  | 5.612s  | 5.606s  | **5.607s** |
| QuickJS  | 1.468s  | 1.516s  | 1.463s  | **1.468s** |

**QuickJS is ~3.8× faster** on the single-threaded `zero_demo` run
(5.607s → 1.468s; −74%).

## Per-phase breakdown (`ZERO_TEST_TIMING`, representative run)

| Phase          | Boa       | QuickJS  | calls | notes |
|----------------|-----------|----------|-------|-------|
| discovery      | 0.6 ms    | 0.5 ms   | 1     | engine-independent |
| context-build  | 36.6 ms   | 9.6 ms   | 41    | per-file runtime/context |
| dom-shim       | 889.3 ms  | 182.5 ms | 41    | shim eval, ~4.9× |
| runtime-eval   | 691.2 ms  | 251.1 ms | 41    | entry + runtime modules, ~2.8× |
| transpile      | 84.0 ms   | 79.4 ms  | ~420  | SWC, engine-independent (as expected) |
| test-exec      | 4021.2 ms | 945.2 ms | 41    | `walk_describe` + `it` bodies, ~4.3× |

The win is concentrated exactly where `test-perf` predicted the cost was: the
interpreter-bound phases (test-exec, dom-shim, runtime-eval). Transpile (SWC) is
unchanged, confirming the engine — not the toolchain — was the bottleneck.

## Conclusion

Both gate conditions are met: **zero outcome/coverage regression** and a **large,
consistent single-threaded speedup (~3.8×)**. This is the evidence for the
user's Step 5 cutover decision.
