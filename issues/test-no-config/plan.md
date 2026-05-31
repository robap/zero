# Plan: `zero test` without a `zero.toml` (and intuitive file paths)

## Summary

Make `zero test` run in a bare directory of `.ts`/`.js` test files (no
`zero.toml`) by falling back to a built-in default — discovery root = cwd,
skipping `dist/` and `build/` — while leaving in-project behavior byte-for-byte
unchanged. In the same pass, fix the file argument so it resolves **cwd-first
with project-root fallback**, so `zero test web/src/app.test.ts` (the path as a
dev/agent sees it) works alongside the legacy `zero test src/app.test.ts`.

The work splits into four compilable, test-green steps:
1. Add `Config::load_from_cwd_optional()` to `zero-config` (absent → `None`,
   present-valid → `Some`, present-invalid → `Err`).
2. Extend `DiscoveryOpts` with `cwd` (for cwd-first file resolution) and
   `extra_skip_dirs` (additional dirs to skip), and implement both in
   `discovery.rs`. Mechanically update every construction site; behavior
   unchanged.
3. Wire the no-config fallback into `cmd/test.rs` using the new optional loader
   and default root/out/skip set.
4. Add a slow integration test that spawns the CLI in a config-less temp dir.

## Prerequisites

None. Both spec open questions are resolved (see Summary): the harness needs
only `root` to run a file (`zero/components` is the lone project-file import and
is opt-in), and the no-config defaults are constructed in `cmd/test.rs` backed
by a thin absence-detecting loader in `zero-config`.

## Steps

- [x] **Step 1: Add `Config::load_from_cwd_optional()` to `zero-config`**
- [x] **Step 2: Extend discovery — cwd-first file resolution + extra skip dirs**
- [x] **Step 3: Wire no-config fallback into `cmd/test.rs`**
- [x] **Step 4: Slow integration test — `zero test` in a config-less dir**

---

## Step Details

### Step 1: Add `Config::load_from_cwd_optional()` to `zero-config`
**Goal:** Give `cmd/test.rs` a way to distinguish "no `zero.toml`" (→ use
defaults) from "`zero.toml` present but invalid" (→ propagate the error). The
file read + `NotFound` handling already live here, so this is the right home;
existing `load_from_cwd()` stays untouched so other callers (`build`, `preview`,
`lint`, `mutate`, `update`) are unaffected.

**Files:**
- `crates/zero-config/src/config.rs` (modify)

**Changes:**
- Add a public method on `impl Config`:
  ```rust
  /// Like [`load_from_cwd`] but returns `Ok(None)` when `zero.toml` is
  /// absent, instead of erroring. A present-but-invalid file still returns
  /// `Err`. Used by `zero test`, which falls back to built-in defaults when
  /// no project config exists.
  pub fn load_from_cwd_optional() -> anyhow::Result<Option<Config>> {
      let cwd = std::env::current_dir()?;
      let path = cwd.join("zero.toml");
      match std::fs::read_to_string(&path) {
          Ok(text) => Ok(Some(Config::from_toml_str(&text)?)),
          Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
          Err(e) => Err(anyhow::anyhow!("failed to read {}: {e}", path.display())),
      }
  }
  ```
  Keep `load_from_cwd()` exactly as-is. (Optional internal tidy: have
  `load_from_cwd` express its missing-file branch in terms of the same read —
  not required; leave it if it risks churn.)

**Tests:** Add `#[test]`s in the existing `config.rs` test module. They need to
control CWD; reuse the same `std::env::set_current_dir` + tempdir pattern used in
`cmd/test.rs` tests (guard with a mutex if these run alongside other
CWD-mutating tests in this crate — check whether `zero-config` has a CWD lock;
if not, a `serial`-style single test that exercises all three cases in one
tempdir sequence avoids cross-test CWD races):
- absent `zero.toml` → `Ok(None)`.
- valid `zero.toml` → `Ok(Some(cfg))` with expected `project.root`.
- present-but-invalid (`deny_unknown_fields` violation, e.g. `[server]`) →
  `Err`.

### Step 2: Extend discovery — cwd-first file resolution + extra skip dirs
**Goal:** Implement the file-argument fix and the additional skip dirs in the
shared discovery code, updating all callers so the workspace still compiles and
all existing tests pass. No `cmd/test.rs` behavior change yet — this step is
purely additive plumbing plus the resolution logic.

**Files:**
- `crates/zero-test-runner/src/discovery.rs` (modify)
- `crates/zero/src/cmd/test.rs` (modify — construction site only)
- `crates/zero/src/cmd/mutate.rs` (modify — two construction sites)

**Changes:**
- Extend `DiscoveryOpts<'a>` with two fields:
  ```rust
  pub struct DiscoveryOpts<'a> {
      pub root: &'a std::path::Path,
      pub out_dir: &'a std::path::Path,
      /// Additional directories to skip during the walk, beyond `out_dir`
      /// (e.g. `build/` in no-config mode). May be empty.
      pub extra_skip_dirs: &'a [std::path::PathBuf],
      pub target: Option<&'a str>,
      /// Base for cwd-first resolution of an explicit-file `target`. In an
      /// in-project run this is the real CWD (distinct from `root`); in
      /// no-config mode it equals `root`.
      pub cwd: &'a std::path::Path,
  }
  ```
- **File-arg resolution** (replace the `root.join(t)` bypass at lines ~30–38):
  try each base in order, returning the first that names a regular file:
  ```rust
  if let Some(t) = opts.target {
      for base in [opts.cwd, opts.root] {
          let candidate = base.join(t);
          if candidate.is_file() {
              let abs = candidate.canonicalize().unwrap_or(candidate);
              return Ok(DiscoveryResult { files: vec![abs] });
          }
      }
  }
  ```
  `is_file()` keeps directories falling through to the substring filter, as
  today. When `cwd == root` (no-config) the first base wins; harmless dup.
- **Skip set**: in `discover`, build one merged skip list and thread it through
  the walk, replacing the single `out_dir` param:
  ```rust
  let mut skip_dirs: Vec<PathBuf> = vec![opts.out_dir.to_path_buf()];
  skip_dirs.extend(opts.extra_skip_dirs.iter().cloned());
  walk_dir(root, &skip_dirs, &mut files)?;
  ```
  Change `walk_dir` and `walk_dot_zero` signatures from `out_dir: &Path` to
  `skip_dirs: &[PathBuf]`, and change the skip check from
  `path.starts_with(out_dir)` to `skip_dirs.iter().any(|d| path.starts_with(d))`.
  The substring-filter branch and the TS/JS collision guard are untouched.
- Update the `opts()` test helper in `discovery.rs` to default the two new
  fields so the ~20 existing call sites stay byte-identical:
  ```rust
  fn opts<'a>(root: &'a Path, out_dir: &'a Path, target: Option<&'a str>)
      -> DiscoveryOpts<'a>
  {
      DiscoveryOpts { root, out_dir, extra_skip_dirs: &[], target, cwd: root }
  }
  ```
- Update the three direct `DiscoveryOpts { .. }` constructions outside the
  helper:
  - `cmd/test.rs:27` — add `extra_skip_dirs: &[]` and `cwd: &cwd` (the binding
    at `cmd/test.rs:23` already exists). Leave `Config::load_from_cwd()` in
    place for now.
  - `cmd/mutate.rs:494` and `:1666` — add `extra_skip_dirs: &[]` and
    `cwd: root` (mutate always passes `target: None`, so `cwd` is inert there).

**Tests:** Add `#[test]`s to the `discovery.rs` test module (construct
`DiscoveryOpts` directly to exercise the new fields):
- *cwd-first wins:* create `cwd/a.test.ts` and `root/a.test.ts` (distinct
  dirs); `target = "a.test.ts"`, `cwd != root` → resolves to the cwd copy.
- *root fallback:* file exists only under `root`, `target` names it relative to
  `root`, cwd copy absent → resolves to the root copy.
- *neither is a file → substring filter still applies:* a `target` that matches
  no file but is a path substring still filters the walk (guard the fallthrough).
- *extra_skip_dirs:* a `build/foo.test.ts` under root is skipped when
  `extra_skip_dirs = [root/build]`, while a sibling `foo.test.ts` is found.
- Existing tests (collision guard, hidden-dir skip, `.zero/components`,
  `skips_out_dir`, substring) must remain green unchanged.

### Step 3: Wire no-config fallback into `cmd/test.rs`
**Goal:** Replace the hard `Config::load_from_cwd()?` gate with the optional
loader, selecting real config values when present and built-in defaults when
absent, then pass the right `root`/`out`/`extra_skip_dirs`/`cwd` to `discover`.

**Files:**
- `crates/zero/src/cmd/test.rs` (modify)

**Changes:**
- At the top of `run`, replace lines ~22–25 with:
  ```rust
  let cwd = std::env::current_dir()?;
  let (root, out, extra_skip_dirs) = match Config::load_from_cwd_optional()? {
      Some(config) => (
          cwd.join(&config.project.root),
          cwd.join(&config.build.out),
          Vec::new(),
      ),
      None => (
          cwd.clone(),
          cwd.join("dist"),
          vec![cwd.join("build")],
      ),
  };
  ```
  (In the no-config branch `out = cwd/dist` and `extra = [cwd/build]` together
  skip both `dist/` and `build/`.)
- Update the `discover(DiscoveryOpts { .. })` call to pass `root: &root`,
  `out_dir: &out`, `extra_skip_dirs: &extra_skip_dirs`, `target: target.as_deref()`,
  `cwd: &cwd`.
- Everything downstream already keys off `root`/`out`: `Reporter::new_with_root`,
  `CoverageScope::new(root, out)`, `run_file_with_coverage(&root, …)`,
  `agg.write_terminal(.., &root)`, `agg.write_json(&root)` all work with the
  no-config values (root = cwd, out = cwd/dist), so coverage writes
  `cwd/coverage/coverage.json`, mirroring in-project layout. No silent-notice
  output is added.

**Tests:** In the `cmd/test.rs` test module:
- **Rewrite** `missing_zero_toml_returns_error` (lines ~117–124) — this asserted
  the old hard error, which is the behavior being removed. Replace with
  `missing_zero_toml_runs_with_defaults`: in a bare tempdir with **no**
  `zero.toml` and one passing `*.test.js` (importing `zero/test`), assert
  `super::run(None, false).await.is_ok()` and exit-equivalent success (no panic;
  the function returns `Ok(())` rather than calling `process::exit`).
- Add `no_config_explicit_file_by_cwd_relative_path`: bare tempdir, a test file
  at `sub/a.test.js`, run `super::run(Some("sub/a.test.js".into()), false)` →
  `Ok(())` (cwd-relative file arg resolves with root = cwd).
- Add `no_config_skips_dist_and_build`: bare tempdir with `dist/x.test.js` and
  `build/y.test.js` plus a top-level `z.test.js`; assert the run is `Ok` and (if
  practically observable) only `z.test.js` ran — otherwise cover the skip
  precisely in the Step 2 discovery unit test and keep this as a smoke `Ok`.
- Keep `no_tests_found_returns_ok_quietly`, `target_filter_with_no_matches…`,
  and `coverage_true_writes_coverage_json` (in-project) green — they still use
  `write_minimal_project`, exercising the `Some(config)` branch unchanged.

### Step 4: Slow integration test — `zero test` in a config-less dir
**Goal:** End-to-end proof that the real `zero` binary runs green against a
directory with test files and no `zero.toml`, per the spec's Done-When.

**Files:**
- `crates/zero/tests/e2e_test_no_config.rs` (create)

**Changes:**
- New integration test using `assert_cmd::Command::cargo_bin("zero")`, mirroring
  the `test_runner_smoke.rs` style:
  - Create a `tempfile::tempdir()`. Do **not** write `zero.toml` and do **not**
    run `zero init`.
  - Write a passing test file at the temp root, e.g. `app.test.ts`:
    ```js
    import { it, expect } from 'zero/test';
    it('runs with no zero.toml', () => { expect(1 + 1).toBe(2); });
    ```
    (Confirm the exact `zero/test` assertion API from an existing in-tree
    `*.test.ts`/`*.test.js` before finalizing the snippet.)
  - Run `zero test` with `.current_dir(tmp.path())`, capture output, assert
    `.success()` (exit 0) and that stdout contains the reporter's pass marker
    (`"passed"`, matching the smoke tests' assertion).
  - Add a second case asserting the cwd-relative **file argument**: put the test
    at `sub/app.test.ts`, run `zero test sub/app.test.ts`, assert success and
    that exactly that file ran.
- Mark both `#[test] #[ignore = "slow"]` (spawns the built binary, per
  `CLAUDE.md`); they run under `cargo test --workspace -- --include-ignored`.

**Tests:** This step *is* the test; verify it passes via
`cargo test -p zero --test e2e_test_no_config -- --include-ignored`.

## Risks and Assumptions

- **`zero/test` API snippet.** The integration/unit test files assume the
  `import { it, expect } from 'zero/test'` surface. Mitigation: copy the exact
  pattern from an existing committed `*.test.*` before writing (Step 4 notes
  this). Low risk.
- **CWD-mutating tests racing.** `cmd/test.rs` tests already serialize via
  `CWD_LOCK`; new `zero-config` tests (Step 1) must not race other CWD-mutating
  tests in that crate. Mitigation: collapse Step 1's cases into one
  sequential test in a single tempdir, or reuse a crate-local lock if one
  exists. Low risk.
- **`extra_skip_dirs` empty-slice inference.** Passing `&[]` for
  `&'a [PathBuf]` at construction sites relies on type inference from the field;
  if inference complains, annotate as `&[] as &[std::path::PathBuf]`. Trivial.
- **Coverage in no-config mode.** Assumes `CoverageScope`/aggregator are happy
  with `root = cwd`, `out = cwd/dist` even when those dirs don't pre-exist. The
  in-project coverage test already creates `coverage/` under root; if writing to
  a non-existent `cwd/coverage` needs a `create_dir_all`, the aggregator likely
  already does this — verify during Step 3, add the dir-create only if a test
  surfaces a failure. Contained to `--coverage`, which the spec marks
  "need not be elaborate."
- **No behavior change for in-project runs.** The cwd-first resolution is
  backward-compatible (verified by reasoning: legacy `zero test src/app.test.ts`
  still hits the `root` fallback; previously-unfound `zero test web/src/...` now
  resolves via `cwd`). The merged skip set is a superset only when
  `extra_skip_dirs` is non-empty, which only happens in the no-config branch.
  If any existing in-project test regresses, that assumption is wrong and needs
  revisiting.
