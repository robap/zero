# Review: `zero test` without a `zero.toml` (and intuitive file paths)

## Status
PASS WITH NOTES

## Checklist Completion
All steps complete: yes

- [x] Step 1: `Config::load_from_cwd_optional()` in `zero-config`
- [x] Step 2: Discovery — cwd-first file resolution + extra skip dirs
- [x] Step 3: Wire no-config fallback into `cmd/test.rs`
- [x] Step 4: Slow integration test in a config-less dir

## Test Results
All tests passing: yes

- `cargo test -p zero-config -p zero-test-runner -p zero` — green (135
  test-runner unit/integration tests + zero crate tests, 0 failed).
- `cargo test -p zero --test e2e_test_no_config -- --include-ignored` — both
  slow e2e cases pass (`runs_green_without_zero_toml`,
  `runs_cwd_relative_file_arg_without_zero_toml`).
- All 6 `cmd::test::tests` async cases pass (in-project + no-config branches).
- `cargo clippy` on the three touched crates — clean, no warnings.

## Requirements Coverage

| Requirement | Status | Notes |
|-------------|--------|-------|
| zero.toml present → behavior unchanged | Satisfied | `Some(config)` branch yields `cwd.join(root)` / `cwd.join(out)` / empty extra-skip, identical to prior behavior; in-project tests still green. |
| zero.toml absent → discovery root = cwd | Satisfied | `None` branch sets `root = cwd` (`cmd/test.rs:29`). |
| Skip dirs += `dist/` and `build/` (preserving node_modules/hidden/`.zero/components`) | Satisfied | `out = cwd/dist`, `extra_skip_dirs = [cwd/build]`; `walk_dir` skip check uses merged `skip_dirs` and keeps the hidden/`.zero` rules untouched. |
| Discovery globs unchanged | Satisfied | `is_test_file` and the TS/JS collision guard are untouched. |
| Fallback is silent (no notice) | Satisfied | No print in the `None` branch. |
| `--coverage` works in no-config mode, writes `coverage/coverage.json` under root | Satisfied | Coverage path keys off `root`/`out`; with root = cwd it writes `cwd/coverage/coverage.json`. Directly tested only in-project (see note 2). |
| File arg: cwd-first, root fallback, then substring filter | Satisfied | `discovery.rs:41-49` tries `[cwd, root]` for an existing file, else falls through to the unchanged substring filter. Covered by 3 unit tests + 1 e2e case. |
| Legacy + intuitive file paths both run the same file | Satisfied | cwd-first is additive; legacy `src/app.test.ts` still hits the root fallback. |

## Constraints and Scope
- **Localized:** changes confined to `cmd/test.rs`, `discovery.rs`, the new
  `load_from_cwd_optional()` loader in `zero-config`, and mechanical
  construction-site updates in `cmd/mutate.rs`. No new config format or
  partial-merge logic. ✓
- **No new CLI flags.** ✓
- **Functions < ~80 lines:** `run` ≈ 62 lines, `discover` ≈ 56 lines. ✓
- **Collision guard + substring semantics preserved** unchanged. ✓
- **Out of scope respected:** `dev`/`build` still call `load_from_cwd()`
  (hard-require config); `mutate` construction sites pass `extra_skip_dirs: &[]`,
  `cwd: root`, `target: None` — inert, behavior intact. ✓

## Code Quality Notes
- `discovery.rs:41-49` — the `for base in [opts.cwd, root]` cwd-first loop is
  clean and idiomatic; `canonicalize().unwrap_or(candidate)` is a sensible
  fallback when the path can't be canonicalized.
- `cmd/test.rs:23-30` — the `match` on the optional loader cleanly separates the
  config-present vs. default cases; downstream code keys entirely off
  `root`/`out`, so nothing else needed to change.
- `config.rs:163-171` — `load_from_cwd_optional` correctly distinguishes
  absent (`Ok(None)`) from present-but-invalid (`Err`); `load_from_cwd` left
  untouched so other callers are unaffected. Single combined CWD test avoids
  cross-test CWD races.
- Test quality is meaningful, not tautological: `no_config_skips_dist_and_build`
  plants *failing* tests in `dist/` and `build/` so a green run proves they were
  skipped; the e2e cwd-relative case asserts the named file ran and the sibling
  did not.

## Issues to Address
1. ~~(Minor / docs) `discover`'s `# Parameters` doc block duplicated the
   `DiscoveryOpts` fields and had drifted (missing `extra_skip_dirs`, `cwd`).~~
   **Resolved:** removed the redundant `# Parameters` block from `discover`
   (the doc now points at `DiscoveryOpts`), and documented all `DiscoveryOpts`
   fields so the struct is the single source of truth.
2. (Minor / optional test) No-config `--coverage` is exercised only indirectly:
   the explicit coverage test (`coverage_true_writes_coverage_json`) runs the
   in-project (`Some(config)`) branch. The no-config coverage path is the same
   code with `root = cwd`, so risk is low and the spec marks it "need not be
   elaborate". **Tracked separately:** folded into the `coverage-output` spec
   (`issues/coverage-output/spec.md`), which reworks the coverage tests and adds
   a no-config coverage assertion.
3. (Docs — gap in spec, plan, and this review) The feature changed two
   user-facing behaviors (`zero test` runs without a `zero.toml`; file args
   resolve cwd-first) but no `docs/` update was specified or done. The spec's
   Requirements/Done-When and the plan's four steps contained no docs step, and
   this review initially missed the omission. **Resolved:** documented both
   behaviors in `docs/testing.md` ("Without a `zero.toml`" and "File paths are
   resolved as you type them" subsections) and `docs/config-and-cli.md` (note
   under the `zero test` flag table). Process note for future specs: a
   user-facing behavior change should carry an explicit docs step.

Issues 1 and 3 are resolved. Issue 2 is non-blocking and tracked in a
follow-up.
