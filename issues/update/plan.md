# Plan: `zero update` and the `.zero/` framework directory

## Summary

Introduce a hidden, framework-owned `.zero/` directory that holds regenerable
framework assets (type definitions and the four design-system SCSS partials plus
a new framework aggregate `zero.scss`), and a new `zero update` subcommand that
rewrites `.zero/` from the embedded binary. Both `zero init` and `zero update`
gain a pre-flight plan + confirmation step; `zero update` adds per-operation
accept/reject. The work proceeds in seven ordered steps: (1) move templates and
flip paths so `init` writes the new layout, (2) refactor `scaffold.rs` to expose
a framework manifest and split-purpose write functions, (3) add `--yes` and
pre-flight confirmation to `init`, (4) implement the `update` command logic
with stubbable confirmer, (5) wire `clap` and add the integration test, (6)
update the scaffold `AGENTS.md`, (7) update the top-level framework spec and
add a supersession note to the design-system spec. Each step ends with the
codebase compiling and the test suite passing.

## Prerequisites

None — the spec's open questions are resolved inline in the steps below. The
resolutions are:

- **`tsconfig.json` `"include"`** — keep an `include`-based approach. New value:
  `["src", ".zero/zero.d.ts", ".zero/zero-test.d.ts"]`. No `"types"` array
  introduced. (Spec §"Open Questions" first bullet; spec requirement 4 mentions
  `"types"`, but the existing tsconfig has only `"include"` — preserve that
  shape and update the paths.)
- **`.gitignore` contents** — minimal. Two lines: `.zero/` and `dist/`. No
  editor or `node_modules` boilerplate.
- **`zero update` exit code on declined operations** — always 0. CI strictness
  is achieved with `--yes`.
- **Interactive per-operation prompt format** — `y`/`n` only. No `q`, no `a`.
- **`init` ordering** — prompts → plan → confirm → write. Final gate is the
  confirmation.
- **`update` diffs in interactive mode** — operation type + path only. No diff
  rendering in v1.
- **`.zero/` exists but empty** — handled naturally by the diff logic (all
  paths report `Add`); a unit test asserts this case.
- **Section-sentinel test list** — append `## The .zero/ directory`. Final
  list has fourteen entries.
- **Re-running `init` post-init** — unchanged: still refuses on non-empty root
  before the prompt. A test asserts this.

## Steps

- [x] **Step 1: Move scaffold to `.zero/` layout and update existing tests**
- [x] **Step 2: Refactor `scaffold.rs` — manifest + split write functions**
- [x] **Step 3: Add `--yes` flag and pre-flight confirmation to `zero init`**
- [x] **Step 4: Implement `update` command logic with a stubbable `Confirmer`**
- [x] **Step 5: Wire `zero update` into `clap` and add the integration test**
- [x] **Step 6: Update scaffold `AGENTS.md` for the new layout**
- [x] **Step 7: Update `zero-framework-spec.md` and add supersession note to `issues/design-system/spec.md`**

---

## Step Details

### Step 1: Move scaffold to `.zero/` layout and update existing tests

**Goal:** Flip the on-disk layout produced by `zero init` to the target shape
described in spec requirement 1, without touching the command surface. After
this step, `zero init` writes `.zero/zero.d.ts`, `.zero/zero-test.d.ts`,
`.zero/styles/_tokens.scss`, `.zero/styles/_base.scss`,
`.zero/styles/_layout.scss`, `.zero/styles/_utilities.scss`,
`.zero/styles/zero.scss`, `styles/app.scss` (rewritten), `tsconfig.json`
(updated paths), and `.gitignore` (new). No `update` command yet; no
confirmation prompt yet.

**Files:**

- `src/scaffold.rs` — rewrite `write_to` for new paths.
- `src/scaffold/styles/app.scss` — rewrite contents (user entry).
- `src/scaffold/.zero/styles/zero.scss` (new) — framework SCSS aggregate.
- `src/scaffold/.zero/styles/_tokens.scss` (new — moved from
  `src/scaffold/styles/_tokens.scss`).
- `src/scaffold/.zero/styles/_base.scss` (new — moved from
  `src/scaffold/styles/_base.scss`).
- `src/scaffold/.zero/styles/_layout.scss` (new — moved from
  `src/scaffold/styles/_layout.scss`).
- `src/scaffold/.zero/styles/_utilities.scss` (new — moved from
  `src/scaffold/styles/_utilities.scss`).
- `src/scaffold/.gitignore` (new) — embedded `.gitignore` template.
- `src/scaffold/tsconfig.json` — update `"include"`.
- `src/scaffold/styles/_tokens.scss`, `_base.scss`, `_layout.scss`,
  `_utilities.scss` — deleted (their contents now live under
  `src/scaffold/.zero/styles/`).
- `src/scaffold/.zero/zero.d.ts` and `src/scaffold/.zero/zero-test.d.ts` —
  **not** introduced as new embedded templates; the runtime constants
  `ZERO_TYPES_BODY` and `ZERO_TEST_TYPES_BODY` in `src/runtime.rs` remain the
  source of truth and `write_to` writes them to the new `.zero/` paths.
- Tests in `src/scaffold.rs::tests` updated per spec requirements 26–31.
- Integration tests under `tests/` whose assertions read the moved paths.

**Changes:**

1. **Move four SCSS partials.** Use `git mv` (or equivalent file move) from
   `src/scaffold/styles/{_tokens,_base,_layout,_utilities}.scss` to
   `src/scaffold/.zero/styles/`. The contents are unchanged at this step
   except for `_tokens.scss`'s leading comment, which is updated:

   Old (lines 7–10 of `_tokens.scss`):
   ```
   // This file is user-owned after `zero init`. Edit, delete, or replace it
   // freely. The future `zero` component library assumes the tokens declared
   // here exist; removing `_tokens.scss` will break components that read
   // `--color-primary`, `--space-md`, etc.
   ```
   New:
   ```
   // This file lives under `.zero/` and is framework-owned — it is rewritten
   // by `zero update`. Do not edit. To override a token, re-declare the
   // custom property in your `styles/app.scss` after the `@use` line.
   ```

2. **Create `src/scaffold/.zero/styles/zero.scss`** with exactly:
   ```scss
   @use 'tokens';
   @use 'base';
   @use 'layout';
   @use 'utilities';
   ```

3. **Rewrite `src/scaffold/styles/app.scss`** to:
   ```scss
   @use '../.zero/styles/zero';

   // Your styles below.
   ```

4. **Create `src/scaffold/.gitignore`** with exactly (final newline included):
   ```
   .zero/
   dist/
   ```

5. **Update `src/scaffold/tsconfig.json`** `"include"` array from
   `["src", "zero.d.ts", "zero-test.d.ts"]` to
   `["src", ".zero/zero.d.ts", ".zero/zero-test.d.ts"]`.

6. **Rewrite `src/scaffold.rs::write_to`** to write the new layout. New
   `include_str!` constants:
   - `TPL_GITIGNORE = include_str!("scaffold/.gitignore")`.
   - `TPL_ZERO_SCSS = include_str!("scaffold/.zero/styles/zero.scss")`.
   - Existing partial constants update their paths to point under
     `scaffold/.zero/styles/`.

   New body order (after `fs::create_dir_all`):
   ```rust
   fs::create_dir_all(root_dir.join(".zero").join("styles"))?;
   fs::create_dir_all(root_dir.join("src").join("routes"))?;
   fs::create_dir_all(root_dir.join("styles"))?;

   // User-owned, one-shot
   fs::write(root_dir.join("index.html"), TPL_INDEX_HTML.replace("{{title}}", &ctx.title))?;
   fs::write(root_dir.join("tsconfig.json"), TPL_TSCONFIG_JSON)?;
   fs::write(root_dir.join("src/app.ts"), TPL_APP_TS)?;
   fs::write(root_dir.join("src/routes/home.ts"), TPL_HOME_TS)?;
   fs::write(root_dir.join("src/routes/home.test.ts"), TPL_HOME_TEST_TS)?;
   fs::write(root_dir.join("styles/app.scss"), TPL_APP_SCSS)?;
   fs::write(root_dir.join("AGENTS.md"), TPL_AGENTS_MD)?;
   fs::write(root_dir.join(".gitignore"), TPL_GITIGNORE)?;

   // Framework-owned, regenerable
   fs::write(root_dir.join(".zero/zero.d.ts"), crate::runtime::ZERO_TYPES_BODY)?;
   fs::write(root_dir.join(".zero/zero-test.d.ts"), crate::runtime::ZERO_TEST_TYPES_BODY)?;
   fs::write(root_dir.join(".zero/styles/_tokens.scss"), TPL_TOKENS_SCSS)?;
   fs::write(root_dir.join(".zero/styles/_base.scss"), TPL_BASE_SCSS)?;
   fs::write(root_dir.join(".zero/styles/_layout.scss"), TPL_LAYOUT_SCSS)?;
   fs::write(root_dir.join(".zero/styles/_layout.scss"), TPL_LAYOUT_SCSS)?;
   fs::write(root_dir.join(".zero/styles/_utilities.scss"), TPL_UTILITIES_SCSS)?;
   fs::write(root_dir.join(".zero/styles/zero.scss"), TPL_ZERO_SCSS)?;
   ```
   Keep `write_to` as the single function for now — the manifest split happens
   in Step 2.

7. **Update existing tests in `src/scaffold.rs::tests`** per spec
   requirements 26–31:
   - Rename `write_to_emits_all_files` → split into:
     - `write_to_emits_user_files` — asserts `index.html`, `tsconfig.json`,
       `src/app.ts`, `src/routes/home.ts`, `src/routes/home.test.ts`,
       `styles/app.scss`, `AGENTS.md` exist with their current markers.
     - `write_to_emits_framework_files` — asserts `.zero/zero.d.ts`,
       `.zero/zero-test.d.ts`, `.zero/styles/_tokens.scss`,
       `.zero/styles/_base.scss`, `.zero/styles/_layout.scss`,
       `.zero/styles/_utilities.scss`, `.zero/styles/zero.scss` all exist and
       are non-empty (and where applicable, that the existing
       `declare module "zero"`, `--color-primary:` markers appear in the new
       paths).
     - `write_to_emits_gitignore_with_zero_dir` — asserts `.gitignore` exists
       and contains a `.zero/` line.
   - Update `write_to_app_ts_imports_zero` — no path change, unchanged.
   - Update `write_to_index_html_links_to_scss` — unchanged (link target is
     still `/styles/app.scss`).
   - Add `app_scss_imports_framework_aggregate` — asserts
     `styles/app.scss` contains the literal `@use '../.zero/styles/zero'`.
   - Add `zero_scss_contains_aggregate_uses` — asserts
     `.zero/styles/zero.scss` contains all four lines
     `@use 'tokens';`, `@use 'base';`, `@use 'layout';`, `@use 'utilities';`.
   - Add `tsconfig_include_points_at_dot_zero` — asserts
     `tsconfig.json`'s `"include"` array contains `.zero/zero.d.ts` and
     `.zero/zero-test.d.ts` and does **not** contain bare `zero.d.ts` /
     `zero-test.d.ts`.
   - Move `tokens_scss_declares_tokens_directly` to read from
     `.zero/styles/_tokens.scss`. Assertions unchanged.
   - `write_to_agents_md_has_section_sentinels` — sentinel list update
     deferred to Step 6 (AGENTS.md is rewritten there).

8. **Update integration tests under `tests/`** that read the moved paths:
   - `tests/design_system.rs::scaffold_home_uses_design_system_classes` —
     no change needed (it reads `web/src/routes/home.ts`).
   - `tests/design_system.rs::build_emits_design_system_css` and
     `build_design_system_passes_contrast_smoke` — exercise `zero build` and
     read compiled CSS, not partial source — no change needed if SCSS
     resolution works through the new `@use '../.zero/styles/zero';`. Run
     these tests as part of Step 1's verification; if SCSS resolution fails,
     investigate `src/sass.rs` (load-paths / file-system access) in this
     step before declaring it done.
   - `tests/e2e_init_typescript.rs`, `tests/e2e_init_test.rs`,
     `tests/dev_writes_dts_on_start.rs`, and any other test that reads
     `zero.d.ts` / `zero-test.d.ts` at the project root — repath to
     `<root>/.zero/zero.d.ts` and `<root>/.zero/zero-test.d.ts`.

9. **Update `src/dev/server.rs` and `src/build/index_html.rs`** only if they
   read the type-definition file paths. If `zero dev` writes a `zero.d.ts`
   on start (per `dev_writes_dts_on_start.rs`), repath its write target to
   `<root>/.zero/zero.d.ts`. Grep for any string literal containing
   `"zero.d.ts"` or `"zero-test.d.ts"` outside of `src/runtime.rs` constants
   and `src/scaffold.rs` — repath each.

**Tests:** All tests listed in change #7 plus #8 plus the existing test suite
(`cargo test`). Must end Step 1 green.

---

### Step 2: Refactor `scaffold.rs` — manifest + split write functions

**Goal:** Expose the framework template set as a queryable structure so
`update` (Step 4) can compute add/update/remove. Split `write_to` into a
user+framework function (used by init) and a framework-only function (used
by update). No external behavior change.

**Files:**

- `src/scaffold.rs` — refactor.
- `src/cmd/init.rs` — call site update.

**Changes:**

1. **Add `Operation` enum** to `src/scaffold.rs`:
   ```rust
   #[derive(Debug, Clone, PartialEq, Eq)]
   pub enum Operation {
       Add(std::path::PathBuf),
       Update(std::path::PathBuf),
       Remove(std::path::PathBuf),
   }
   ```
   Paths in `Operation` are **relative to the project root** (e.g.
   `.zero/styles/_tokens.scss`), not absolute.

2. **Add `framework_manifest()`** to `src/scaffold.rs`:
   ```rust
   /// Returns the canonical list of framework template files. Each tuple is
   /// `(relative_path, content)`. Both `init` (initial write) and `update`
   /// (diff + refresh) consult this single source of truth.
   pub fn framework_manifest() -> Vec<(&'static str, &'static str)> {
       vec![
           (".zero/zero.d.ts", crate::runtime::ZERO_TYPES_BODY),
           (".zero/zero-test.d.ts", crate::runtime::ZERO_TEST_TYPES_BODY),
           (".zero/styles/_tokens.scss", TPL_TOKENS_SCSS),
           (".zero/styles/_base.scss", TPL_BASE_SCSS),
           (".zero/styles/_layout.scss", TPL_LAYOUT_SCSS),
           (".zero/styles/_utilities.scss", TPL_UTILITIES_SCSS),
           (".zero/styles/zero.scss", TPL_ZERO_SCSS),
       ]
   }
   ```
   Returns `Vec` (not `&'static [...]`) because `ZERO_TYPES_BODY` is a
   `const &str` whose lifetime is `'static` — `Vec<(&'static str, &'static str)>`
   works fine.

3. **Add `user_template_paths()`** internal helper (no public API needed —
   used only by `write_initial_project`):
   ```rust
   fn write_user_files(root_dir: &Path, ctx: &ScaffoldContext) -> anyhow::Result<()> { /* ... */ }
   ```
   This function writes the seven user files plus `.gitignore`. Its body is
   extracted verbatim from the current `write_to`.

4. **Add `write_framework_files(root_dir: &Path) -> anyhow::Result<()>`** (no
   `Operation` return value yet — that comes from the diff logic in Step 4):
   ```rust
   pub fn write_framework_files(root_dir: &Path) -> anyhow::Result<()> {
       fs::create_dir_all(root_dir.join(".zero").join("styles"))?;
       for (rel, content) in framework_manifest() {
           let abs = root_dir.join(rel);
           if let Some(parent) = abs.parent() {
               fs::create_dir_all(parent)?;
           }
           fs::write(&abs, content)?;
       }
       Ok(())
   }
   ```

5. **Add `write_initial_project(root_dir: &Path, ctx: &ScaffoldContext) -> anyhow::Result<()>`**:
   ```rust
   pub fn write_initial_project(root_dir: &Path, ctx: &ScaffoldContext) -> anyhow::Result<()> {
       fs::create_dir_all(root_dir)?;
       write_user_files(root_dir, ctx)?;
       write_framework_files(root_dir)?;
       Ok(())
   }
   ```

6. **Delete `write_to`** and update `src/cmd/init.rs::run` to call
   `write_initial_project(&root_dir, &ctx)` instead.

7. **Tests:**
   - Replace the test names from Step 1 (`write_to_*`) with
     `write_initial_project_*` (spec requirement 26 already calls them this).
   - Add `framework_manifest_lists_seven_files` — asserts the manifest length
     is 7 and contains every expected relative path.
   - Add `write_framework_files_writes_only_dot_zero` — invoke
     `write_framework_files(root)` on a fresh dir; assert every manifest
     path now exists and that nothing outside `<root>/.zero/` was written
     (walk `<root>/` and assert each non-`.zero/` entry is the `.zero/`
     directory itself).

**Tests:** All Step 1 tests rename and continue to pass. Two new tests
above. `cargo test` green.

---

### Step 3: Add `--yes` flag and pre-flight confirmation to `zero init`

**Goal:** Per spec requirements 8–11, print a plan and prompt for
confirmation between the existing prompt phase and any disk write, with a
`--yes` flag that skips the prompt.

**Files:**

- `src/main.rs` — add `--yes` / `-y` to `Init`.
- `src/cmd/init.rs` — accept `yes: bool`, insert plan + confirm step.
- `src/prompts.rs` — add a thin confirmation helper (preferred; reuse
  `dialoguer::Confirm` which is already in scope via the `dialoguer = "0.11"`
  dep — no new crate needed per spec constraint #1).
- New unit test module in `src/cmd/init.rs` for the plan-rendering helper.

**Changes:**

1. **CLI definition** in `src/main.rs`:
   ```rust
   /// Scaffold a new zero app in the current directory
   Init {
       /// Skip the pre-flight confirmation prompt.
       #[arg(long, short = 'y', default_value_t = false)]
       yes: bool,
   },
   ```
   Dispatch: `Commands::Init { yes } => cmd::init::run(yes).await`.

2. **`cmd::init::run`** signature becomes
   `pub async fn run(yes: bool) -> anyhow::Result<()>`.

3. **Plan rendering helper** added to `src/cmd/init.rs`:
   ```rust
   /// Returns the multi-line plan string `zero init` prints before writing.
   /// Pure function — no I/O — for testability.
   fn render_init_plan() -> String { /* ... */ }
   ```
   The body returns exactly the text shown in spec requirement 8 (lines for
   framework files under `.zero/`, then user files), terminated with
   `"\nProceed? [Y/n]"`. The list is derived from
   `scaffold::framework_manifest()` (for the framework block) and a small
   hardcoded list of user files (the seven user paths + `.gitignore`) — keep
   the user list inline in `render_init_plan`; it does not need to be a
   separate manifest because user files are written only by init.

4. **Confirmation prompt** added to `src/prompts.rs`:
   ```rust
   /// Prompt the user with `prompt_text` followed by ` [Y/n] `. Defaults to
   /// Yes on empty input. Returns `Ok(true)` if the user accepts, `Ok(false)`
   /// if the user declines.
   pub fn confirm_default_yes(prompt_text: &str) -> anyhow::Result<bool> { /* uses dialoguer::Confirm with default=true */ }
   ```

5. **`run` body** (post-Step-2) becomes:
   ```rust
   pub async fn run(yes: bool) -> anyhow::Result<()> {
       let cwd = std::env::current_dir()?;
       let toml_path = cwd.join("zero.toml");

       let config = if toml_path.exists() {
           Config::load_from_cwd()?
       } else {
           println!("zero init — let's set up a project");
           let answers = prompt_user()?;
           write_toml_file(&toml_path, &render_toml(&answers))?;
           config_from_answers(&answers)?
       };

       let root_dir = cwd.join(&config.project.root);
       if root_dir.exists() && fs::read_dir(&root_dir)?.next().is_some() {
           anyhow::bail!(
               "zero init: ./{}/ is not empty; refusing to overwrite",
               config.project.root
           );
       }

       println!("{}", render_init_plan());
       if !yes && !crate::prompts::confirm_default_yes("Proceed?")? {
           println!("zero init: aborted by user");
           return Ok(());
       }

       let title = cwd
           .file_name()
           .and_then(|n| n.to_str())
           .map(|s| s.to_string())
           .unwrap_or_else(|| "My zero app".to_string());

       scaffold::write_initial_project(&root_dir, &ScaffoldContext { title })?;

       println!(
           "Scaffold written to ./{}/ — run `zero dev` to start.",
           config.project.root
       );
       Ok(())
   }
   ```

6. **Integration-test impact:** every existing `assert_cmd::Command::cargo_bin("zero").arg("init")`
   call needs `.arg("--yes")` appended, otherwise the test will block on
   stdin. Grep all of `tests/*.rs` for `.arg("init")` and add `--yes`.
   Specifically:
   - `tests/design_system.rs` (3 callers)
   - `tests/e2e_init_*.rs` (4 files)
   - `tests/init_existing_toml.rs`
   - `tests/scss_*.rs` (if they call `init`)
   - `tests/dev_*.rs` (if they call `init`)
   - `tests/build_*.rs` (if they call `init`)
   Grep + add the flag.

**Tests:**

- `init_plan_lists_framework_and_user_groups` — unit test in
  `src/cmd/init.rs::tests`. Calls `render_init_plan()`, asserts the output
  contains the substrings `"framework files (regenerable, under .zero/)"`,
  `".zero/styles/_tokens.scss"`, `"user files"`, `"styles/app.scss"`,
  `".gitignore"`, and `"Proceed? [Y/n]"`.
- `init_yes_skips_prompt_and_writes_files` — integration test (could go in
  a new `tests/init_confirm.rs`). Runs `zero init --yes` in a fresh tempdir
  with a pre-staged `zero.toml`; asserts files were written.
- `init_refuses_non_empty_root_before_prompt` — integration test asserting
  the existing non-empty-root failure is still raised even when `--yes` is
  passed (spec requirement 12, "Open Questions" last bullet). Test creates
  `web/foo.txt`, runs `zero init --yes`, expects failure with the
  `"is not empty; refusing to overwrite"` message.
- All existing integration tests pass thanks to the `--yes` retrofit
  (change #6).

---

### Step 4: Implement `update` command logic with a stubbable `Confirmer`

**Goal:** Compute the add/update/remove diff between disk and the framework
manifest, render the plan, prompt for the top-level decision, optionally
prompt per-operation, then apply. Driven by a `Confirmer` trait so unit
tests can stub the prompts.

**Files:**

- `src/cmd/update.rs` (new).
- `src/cmd/mod.rs` — register the new module.
- `src/scaffold.rs` — add a small helper if needed (see change #2 below).

**Changes:**

1. **Module `src/cmd/update.rs`** (no `pub fn run` yet — that's wired in Step 5):
   ```rust
   //! `zero update` — refresh framework files in `.zero/` from the embedded binary.

   use std::collections::BTreeSet;
   use std::fs;
   use std::path::{Path, PathBuf};

   use crate::scaffold::{framework_manifest, Operation};

   /// Decisions a user can make per operation in interactive mode.
   pub enum PerOpDecision { Apply, Skip }

   /// Top-level decisions at the initial `Apply all? [Y/n/i]` prompt.
   pub enum TopDecision { ApplyAll, Abort, Interactive }

   /// Trait that abstracts the interactive prompts so tests can stub them.
   pub trait Confirmer {
       fn top_level(&mut self, plan: &[Operation]) -> anyhow::Result<TopDecision>;
       fn per_operation(&mut self, op: &Operation) -> anyhow::Result<PerOpDecision>;
       fn final_apply(&mut self, plan: &[Operation]) -> anyhow::Result<bool>;
   }

   /// Compute the set of `Operation`s by comparing `<root>/.zero/` against
   /// the framework manifest.
   pub fn compute_plan(root: &Path) -> anyhow::Result<Vec<Operation>> {
       let manifest = framework_manifest();
       let manifest_paths: BTreeSet<PathBuf> = manifest.iter().map(|(p, _)| PathBuf::from(p)).collect();

       let mut ops = Vec::new();

       // Add + Update
       for (rel, content) in &manifest {
           let abs = root.join(rel);
           if !abs.exists() {
               ops.push(Operation::Add(PathBuf::from(rel)));
           } else {
               let on_disk = fs::read(&abs)?;
               if on_disk != content.as_bytes() {
                   ops.push(Operation::Update(PathBuf::from(rel)));
               }
           }
       }

       // Remove — walk <root>/.zero/ for files not in the manifest
       let dot_zero = root.join(".zero");
       if dot_zero.is_dir() {
           walk_files(&dot_zero, &mut |abs| {
               let rel = abs.strip_prefix(root).unwrap().to_path_buf();
               if !manifest_paths.contains(&rel) {
                   ops.push(Operation::Remove(rel));
               }
           })?;
       }
       Ok(ops)
   }

   fn walk_files(dir: &Path, f: &mut impl FnMut(&Path)) -> anyhow::Result<()> { /* recursive readdir, files only */ }

   /// Render the operation plan as a multi-line string, grouped by Add / Update /
   /// Remove, matching spec requirement 17. Pure function; no I/O.
   pub fn render_plan(ops: &[Operation]) -> String { /* ... */ }

   /// Apply a slice of operations to `<root>/.zero/`. Refuses to write to any
   /// path that does not have `<root>/.zero/` as its canonical prefix
   /// (spec constraint "`zero update` never writes outside `.zero/`").
   pub fn apply(root: &Path, ops: &[Operation]) -> anyhow::Result<()> {
       let manifest = framework_manifest();
       for op in ops {
           let rel = match op { Operation::Add(p) | Operation::Update(p) | Operation::Remove(p) => p };
           let abs = root.join(rel);
           // Boundary check: abs must start with root/.zero
           let dot_zero = root.join(".zero");
           if !abs.starts_with(&dot_zero) {
               anyhow::bail!("zero update: refusing to touch path outside .zero/: {}", abs.display());
           }
           match op {
               Operation::Add(_) | Operation::Update(_) => {
                   let content = manifest.iter().find(|(p, _)| Path::new(p) == rel.as_path())
                       .ok_or_else(|| anyhow::anyhow!("internal: no manifest entry for {}", rel.display()))?.1;
                   if let Some(parent) = abs.parent() { fs::create_dir_all(parent)?; }
                   fs::write(&abs, content)?;
               }
               Operation::Remove(_) => {
                   fs::remove_file(&abs)?;
               }
           }
       }
       Ok(())
   }

   /// Drive the full update flow: detect preconditions, compute plan, prompt,
   /// apply, print summary. `yes` short-circuits the top-level prompt.
   pub fn run_with(root: &Path, yes: bool, confirmer: &mut dyn Confirmer) -> anyhow::Result<()> { /* see #2 */ }
   ```

2. **`run_with` body** — explicit, since this is the meaty piece:
   ```rust
   pub fn run_with(root: &Path, yes: bool, confirmer: &mut dyn Confirmer) -> anyhow::Result<()> {
       // Precondition: zero.toml + .zero/ both exist (spec requirement 14).
       if !root.join("zero.toml").exists() {
           anyhow::bail!("zero update: no zero.toml found — run 'zero init' first");
       }
       if !root.join(".zero").is_dir() {
           anyhow::bail!("zero update: no .zero/ directory found — this project predates the .zero layout; re-run 'zero init' in a fresh directory or create .zero/ manually");
       }

       let plan = compute_plan(root)?;
       if plan.is_empty() {
           println!("zero update: .zero/ is already up to date.");
           return Ok(());
       }

       print!("{}", render_plan(&plan));

       let to_apply: Vec<Operation> = if yes {
           plan.clone()
       } else {
           match confirmer.top_level(&plan)? {
               TopDecision::ApplyAll => plan.clone(),
               TopDecision::Abort => {
                   println!("zero update: no changes applied");
                   return Ok(());
               }
               TopDecision::Interactive => {
                   let mut keep = Vec::new();
                   for op in &plan {
                       match confirmer.per_operation(op)? {
                           PerOpDecision::Apply => keep.push(op.clone()),
                           PerOpDecision::Skip => {}
                       }
                   }
                   if keep.is_empty() {
                       println!("zero update: no changes applied");
                       return Ok(());
                   }
                   // Re-confirm after per-op pass (spec requirement 17 bullet 3).
                   print!("{}", render_plan(&keep));
                   if !confirmer.final_apply(&keep)? {
                       println!("zero update: no changes applied");
                       return Ok(());
                   }
                   keep
               }
           }
       };

       apply(root, &to_apply)?;
       let (a, u, r) = count_kinds(&to_apply);
       println!("zero update: applied {} operations ({} added, {} updated, {} removed).", to_apply.len(), a, u, r);
       Ok(())
   }
   ```

3. **Default `StdinConfirmer`** struct in `src/cmd/update.rs` that
   implements `Confirmer` against stdin/stdout using `dialoguer`. The Step 5
   `pub async fn run` constructs one of these.

4. **Tests in `src/cmd/update.rs::tests`** (spec requirement 32). Set up a
   tempdir scaffolded via `scaffold::write_initial_project`. A
   `StubConfirmer { top: TopDecision::ApplyAll, per_op: vec![...], final_apply: true }`
   drives the prompts.
   - `update_with_no_drift_reports_up_to_date` — scaffold project, call
     `compute_plan(root)`, assert empty.
   - `update_with_missing_file_proposes_add` — scaffold, delete
     `.zero/styles/_utilities.scss`, call `compute_plan`, assert plan
     contains `Operation::Add(".zero/styles/_utilities.scss".into())`.
   - `update_with_modified_file_proposes_update` — scaffold, append `// x`
     to `.zero/zero.d.ts`, call `compute_plan`, assert plan contains
     `Operation::Update(".zero/zero.d.ts".into())`.
   - `update_with_extra_file_proposes_remove` — scaffold, write
     `.zero/styles/_extra.scss`, call `compute_plan`, assert plan contains
     `Operation::Remove(".zero/styles/_extra.scss".into())`.
   - `update_refuses_when_no_zero_toml` — empty tempdir, call `run_with`,
     assert error message contains `"no zero.toml found"`.
   - `update_refuses_when_no_dot_zero_dir` — tempdir with only `zero.toml`
     present, call `run_with`, assert error message contains
     `"no .zero/ directory found"`.
   - `update_yes_flag_applies_all_operations` — scaffold, mutate one file +
     create one extra + delete one file, call `run_with(root, yes=true, ...)`,
     assert all three converge to manifest state.
   - `apply_refuses_path_outside_dot_zero` — synthesize an
     `Operation::Add("outside.txt".into())` (no `.zero/` prefix), call
     `apply`, assert error.
   - `update_with_empty_dot_zero_dir_proposes_only_adds` — scaffold then
     `rm -rf .zero/*` (leave the dir itself), call `compute_plan`, assert
     plan is exactly the seven `Add` ops (open-question resolution).

5. **No `cmd/mod.rs` registration yet?** Actually do register it here:
   ```rust
   pub mod build;
   pub mod dev;
   pub mod init;
   pub mod test;
   pub mod update;
   ```
   so the test module compiles. The clap wiring lands in Step 5.

**Tests:** All tests above. `cargo test` green.

---

### Step 5: Wire `zero update` into `clap` and add the integration test

**Goal:** Make `zero update` invocable from the CLI and validate the full
end-to-end flow with a real binary.

**Files:**

- `src/main.rs` — add `Update` variant.
- `src/cmd/update.rs` — add `pub async fn run(yes: bool) -> anyhow::Result<()>`.
- `tests/update.rs` (new) — integration test per spec requirement 34.

**Changes:**

1. **`src/main.rs`** `Commands` enum gains:
   ```rust
   /// Refresh framework files in .zero/
   Update {
       /// Skip the pre-flight confirmation prompt.
       #[arg(long, short = 'y', default_value_t = false)]
       yes: bool,
   },
   ```
   Dispatch arm: `Commands::Update { yes } => cmd::update::run(yes).await`.

2. **`src/cmd/update.rs::run`**:
   ```rust
   pub async fn run(yes: bool) -> anyhow::Result<()> {
       let root = std::env::current_dir()?;
       let mut confirmer = StdinConfirmer::default();
       run_with(&root, yes, &mut confirmer)
   }
   ```

3. **`tests/update.rs`** — verbatim translation of spec requirement 34:
   ```rust
   use assert_cmd::Command;
   use std::fs;
   use tempfile::tempdir;

   fn init_project(tmp: &std::path::Path) {
       fs::write(tmp.join("zero.toml"), "[project]\nroot = \".\"\n\n[build]\nout = \"dist\"\n").unwrap();
       Command::cargo_bin("zero").unwrap()
           .arg("init").arg("--yes")
           .current_dir(tmp).assert().success();
   }

   #[test]
   fn update_restores_modified_recreates_deleted_removes_stray() {
       let tmp = tempdir().unwrap();
       init_project(tmp.path());

       // Snapshot post-init user-file bytes for the unchanged-assertions later.
       let app_scss = fs::read(tmp.path().join("styles/app.scss")).unwrap();
       let index_html = fs::read(tmp.path().join("index.html")).unwrap();
       let app_ts = fs::read(tmp.path().join("src/app.ts")).unwrap();
       let tsconfig = fs::read(tmp.path().join("tsconfig.json")).unwrap();
       let agents = fs::read(tmp.path().join("AGENTS.md")).unwrap();

       // Drift.
       fs::write(tmp.path().join(".zero/styles/_tokens.scss"), b"/* MUTATED */\n").unwrap();
       fs::remove_file(tmp.path().join(".zero/styles/_utilities.scss")).unwrap();
       fs::write(tmp.path().join(".zero/styles/_extra.scss"), b"// stray\n").unwrap();

       Command::cargo_bin("zero").unwrap()
           .arg("update").arg("--yes")
           .current_dir(tmp.path()).assert().success();

       // Tokens restored byte-identical to the embedded template.
       let tokens_after = fs::read(tmp.path().join(".zero/styles/_tokens.scss")).unwrap();
       assert!(std::str::from_utf8(&tokens_after).unwrap().contains("--color-primary:"));
       assert!(!std::str::from_utf8(&tokens_after).unwrap().contains("/* MUTATED */"));

       // Utilities recreated.
       assert!(tmp.path().join(".zero/styles/_utilities.scss").exists());

       // Stray removed.
       assert!(!tmp.path().join(".zero/styles/_extra.scss").exists());

       // User files unchanged.
       assert_eq!(fs::read(tmp.path().join("styles/app.scss")).unwrap(), app_scss);
       assert_eq!(fs::read(tmp.path().join("index.html")).unwrap(), index_html);
       assert_eq!(fs::read(tmp.path().join("src/app.ts")).unwrap(), app_ts);
       assert_eq!(fs::read(tmp.path().join("tsconfig.json")).unwrap(), tsconfig);
       assert_eq!(fs::read(tmp.path().join("AGENTS.md")).unwrap(), agents);
   }

   #[test]
   fn update_on_clean_project_is_noop() {
       let tmp = tempdir().unwrap();
       init_project(tmp.path());
       let assert = Command::cargo_bin("zero").unwrap()
           .arg("update").arg("--yes")
           .current_dir(tmp.path()).assert().success();
       let out = std::str::from_utf8(&assert.get_output().stdout).unwrap().to_string();
       assert!(out.contains("already up to date"), "expected up-to-date message, got: {out}");
   }
   ```
   (The integration test sets `root = "."` in `zero.toml` so the project is
   scaffolded into the tempdir root and `zero update` can run there
   without `--cwd` plumbing.)

**Tests:** New `tests/update.rs` (two tests). Plus existing suite still
green.

---

### Step 6: Update scaffold `AGENTS.md` for the new layout

**Goal:** Bring the user-facing `AGENTS.md` (written into every new
project) in line with the new layout, per spec requirement 41. Update the
section-sentinels test.

**Files:**

- `src/scaffold/AGENTS.md` — content edits.
- `src/scaffold.rs::tests::write_to_agents_md_has_section_sentinels` — add
  one sentinel.

**Changes:**

1. **Edit the project-layout block (lines 27–45 of current `AGENTS.md`)** to
   reflect the new layout:
   ```
   .
   ├── AGENTS.md                # this file
   ├── .gitignore               # ignores .zero/ and dist/
   ├── tsconfig.json            # editor-only TS config; the CLI ignores it
   ├── .zero/                   # framework-owned, refreshed by `zero update` — do not edit
   │   ├── zero.d.ts            # type surface for `"zero"`
   │   ├── zero-test.d.ts       # type surface for `"zero/test"`
   │   └── styles/
   │       ├── _tokens.scss     # design tokens + theme variants
   │       ├── _base.scss       # minimal reset, token-bound body
   │       ├── _layout.scss     # six layout primitives
   │       ├── _utilities.scss  # gap-*, pad-*, border-* utilities
   │       └── zero.scss        # aggregate that @use's the four partials
   ├── index.html               # entry HTML; <script> tags are injected automatically
   ├── src/
   │   ├── app.ts               # builds and starts the App
   │   └── routes/
   │       ├── home.ts          # default route component
   │       └── home.test.ts     # unit test for the home route
   └── styles/
       └── app.scss             # @use '../.zero/styles/zero'; — add your styles here
   ```

2. **Edit the `## Styles` section (lines 499–521)** to point at the new
   locations and convey framework-ownership:

   - Update the first bullet list to mention that `_tokens.scss`,
     `_base.scss`, `_layout.scss`, `_utilities.scss`, and `zero.scss` live
     under `.zero/styles/`, are framework-owned, and **must not be edited**.
   - Update the `### Design system` table caption from "five partials, all
     `@use`-d from `app.scss`" to "four partials plus an aggregate, all
     framework-owned in `.zero/styles/`, brought in by your
     `styles/app.scss` via `@use '../.zero/styles/zero';`".
   - Replace the "After `zero init`, the partials are normal project
     files…" paragraph with: "These partials live under `.zero/styles/`
     and are framework-owned — `zero update` refreshes them; do not edit
     them. To override a token, re-declare the CSS custom property in your
     `styles/app.scss` after the `@use` line. To add new utility classes,
     write them in `styles/app.scss` directly."

3. **Add a new top-level section `## The .zero/ directory`** (insert before
   `## Navigation` — line ~568):
   ```markdown
   ## The .zero/ directory

   `.zero/` is the framework's regenerable file boundary. It is hidden from
   git (added to `.gitignore` by `zero init`) and is owned by the `zero`
   CLI — `zero update` is the only command that writes there. Do not edit
   files under `.zero/`. To pick up new framework assets when you upgrade
   the CLI, run `zero update`.

   Files currently shipped under `.zero/`:

   | Path | What it is |
   | --- | --- |
   | `.zero/zero.d.ts` | TypeScript declarations for the `"zero"` import. |
   | `.zero/zero-test.d.ts` | TypeScript declarations for the `"zero/test"` import. |
   | `.zero/styles/_tokens.scss` | Design tokens and theme variants. |
   | `.zero/styles/_base.scss` | Minimal reset and token-bound `body` rule. |
   | `.zero/styles/_layout.scss` | Six layout primitives (`cluster`, `stack`, `frame`, `split`, `flank`, `grid`). |
   | `.zero/styles/_utilities.scss` | Gap, padding, and border utility classes. |
   | `.zero/styles/zero.scss` | Aggregate that `@use`'s the four partials above. |

   ### Updating

   ```bash
   zero update             # prints a plan, asks to confirm, refreshes .zero/
   zero update --yes       # apply without prompting (CI)
   ```

   In interactive mode (`i` at the top-level prompt) you can accept or
   reject each operation one at a time.
   ```

4. **Update the section-sentinels test** in `src/scaffold.rs::tests`:
   Append `"## The .zero/ directory"` to the array. New count: 14 sentinels.

5. **Edit the layout-block intro paragraph (line 26)** to mention the new
   directory: replace "The generated project layout:" — no change needed,
   but ensure the block in #1 above replaces lines 27–45 cleanly.

**Tests:**

- `write_to_agents_md_has_section_sentinels` — updated to include
  `"## The .zero/ directory"`.
- Optional add: `write_to_agents_md_mentions_zero_update` — asserts the
  string `"zero update"` appears in the rendered `AGENTS.md`.

---

### Step 7: Update `zero-framework-spec.md` and add supersession note to `issues/design-system/spec.md`

**Goal:** Bring the framework-level spec in line with the new layout, per
spec requirements 38–40 and 42.

**Files:**

- `zero-framework-spec.md` — §1 CLI block, §7.1 Design system, §13 summary.
- `issues/design-system/spec.md` — prepend a short supersession note.

**Changes:**

1. **`zero-framework-spec.md` §1 commands block (line ~27).** Add:
   ```
     zero update                 Refresh framework files in .zero/
   ```
   immediately after the `zero new` / `zero init` line. Add the matching
   "Subcommand Details" subsection covering:
   - `--yes` / `-y` flag.
   - The pre-flight plan + confirm flow.
   - The `Y` / `n` / `i` top-level prompt and the per-operation `y` / `n`
     prompt with final re-confirm.
   - Exit-0-on-no-drift behavior.

   In the existing `zero init` (or `zero new`) subcommand subsection, add a
   sentence: "Before writing files, `zero init` prints a plan of what it
   will create and waits for confirmation. Pass `--yes` to skip the
   prompt."

2. **`zero-framework-spec.md` §7.1 Design system (lines 828–853)** —
   rewrite the section-opening paragraph and the **Distribution model**
   paragraph:

   - Replace the opening paragraph (line 830) with: "The scaffold ships a
     built-in design-system layer in `.zero/styles/`: four partials
     (`_tokens.scss`, `_base.scss`, `_layout.scss`, `_utilities.scss`) plus
     an aggregate (`zero.scss`) that `@use`'s them. The user's
     `styles/app.scss` is a one-shot, user-owned entry that imports the
     aggregate via `@use '../.zero/styles/zero';`. The four partials are
     framework-owned — they live under the hidden, `.gitignore`-d `.zero/`
     directory and refresh via `zero update`."
   - Replace **Distribution model** (line 853) with: "Framework-owned and
     regenerable. `zero init` writes the partials into `.zero/styles/`;
     `zero update` refreshes them when the CLI ships new content. Users
     override tokens by re-declaring CSS custom properties in
     `styles/app.scss` after the framework `@use` line — overriding by
     re-declaration is preserved, just no longer by editing the file that
     declares the tokens."

3. **`zero-framework-spec.md` §13 (Key Design Decisions Summary)** at line
   ~1242:

   - In the "Design system" row, replace the rationale text "Common patterns
     shouldn't be hand-rolled per project; future component library has a
     stable foundation" with: "Built-in scaffold layer with tokens, themes,
     layout primitives. Framework-owned regenerable layer under `.zero/`,
     refreshed by `zero update`."
   - Insert a new row after the "Design system" row:
     `| Framework-file boundary | Hidden .zero/ directory, regenerated by zero update | Prevents accidental edits to framework-shipped files; gives projects a versioned upgrade path |`

4. **`issues/design-system/spec.md`** — prepend the following note above
   the existing first heading:
   ```
   > **Note:** This spec captured the design system as of the
   > design-system issue. Requirements 19–20 (scaffold file paths) are
   > superseded by `issues/update/spec.md`: `_tokens.scss`, `_base.scss`,
   > `_layout.scss`, `_utilities.scss`, and the aggregate `zero.scss` now
   > live under `.zero/styles/` and are framework-owned and regenerable
   > via `zero update`. The user's `styles/app.scss` remains user-owned
   > and is the place to add styles or override tokens.
   ```
   (Verify the exact requirement numbers in `issues/design-system/spec.md`
   that name the scaffold paths before committing the note. If they aren't
   numbered 19–20, adjust the citation accordingly.)

**Tests:** Documentation-only — no automated tests for the markdown.
Manually scan-read each edit. Run `cargo test` to confirm nothing
regressed.

---

## Risks and Assumptions

- **SCSS load-paths.** `styles/app.scss` now imports `../.zero/styles/zero`.
  This assumes `src/sass.rs` resolves relative paths from the file being
  compiled, not from a fixed search root. If it does not, the build
  pipeline breaks and Step 1's `tests/design_system.rs` integration tests
  fail. Mitigation: Step 1 explicitly runs the design-system integration
  tests; if they fail, the fix lives in `src/sass.rs` and is the first
  thing to address before declaring Step 1 done.
- **`zero dev` writing a `.d.ts`.** `tests/dev_writes_dts_on_start.rs`
  suggests `zero dev` writes type-definition files on start. If so, that
  write target needs to repath to `.zero/zero.d.ts`. Step 1 mentions this
  but the exact code location needs confirmation during execution.
- **`dialoguer::Confirm` in non-TTY tests.** `dialoguer` panics or blocks
  if stdin is not a TTY. All integration tests must use `--yes`; the unit
  tests for `update` use the `Confirmer` trait stub. This is already
  reflected in Step 3 (retrofit `--yes` to existing tests) and Step 4
  (trait-driven). Risk only materializes if a new test path is added that
  forgets `--yes`.
- **`Operation::Remove` for non-file entries under `.zero/`.** If the user
  creates a subdirectory under `.zero/`, the walk in `compute_plan`
  recurses into it; any files inside are emitted as `Remove`. Empty
  subdirectories are not in the manifest and are left in place. Acceptable
  — the spec only asserts file-level behavior.
- **`config.project.root = "."` in the integration test.** The
  `validate_path_segment` rejects `.` (leading dot is forbidden). The
  Step 5 integration test bypasses this by writing `zero.toml` directly,
  not by running the prompt. If `Config::from_toml_str` re-runs the
  segment validation, a different root value (e.g. `"web"`) must be used
  and the test paths adjusted to use `tmp.path().join("web")`. Verify
  during execution.
- **`framework_manifest()` returns `Vec` allocations on each call.** Used
  rarely (init, update); the allocation cost is negligible. A future
  optimization could memoize it, but not in v1.
- **Pre-existing `_tokens.scss` comment.** Step 1 rewords the leading
  comment from "user-owned" to "framework-owned." Future readers diffing
  against pre-Step-1 might be confused. The commit message should mention
  the move explicitly.
