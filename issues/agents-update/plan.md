# Plan: `AGENTS.md` refreshed by `zero update`

## Summary

Move `AGENTS.md` from the user-owned, one-shot scaffold set into the
framework-owned manifest so `zero update` keeps it current. `AGENTS.md`
stays at the project root (Claude Code / Cursor convention) by adding a
single named exception to the `<root>/.zero/`-only write boundary in
`apply`. The work lands in three ordered steps: (1) wire the manifest +
boundary change + retrofit existing tests so everything stays green; (2)
add new unit + integration tests that pin the new behavior; (3) update
the scaffold `AGENTS.md` and prepend a supersession note to the original
`issues/update/spec.md`. Each step leaves the codebase compilable with
the full test suite passing.

## Prerequisites

None — the spec's open questions are resolved inline below.

Resolutions:

- **Error message wording** — exactly:
  `"zero update: refusing to touch path outside .zero/ (other than AGENTS.md): <path>"`.
- **Manifest position** — first entry of `framework_manifest()`, above
  `.zero/zero.d.ts`. Plan output groups it visibly at the top of the
  framework list.
- **Boundary check shape** — exact equality (`abs != agents_md`).
  `AGENTS.md` is a single file at the root; no `starts_with` semantics
  apply. `abs` is built via `root.join(rel)`, which preserves the
  separator scheme and produces a byte-identical match against
  `root.join("AGENTS.md")`.
- **`render_plan` output** — confirmed: `crates/zero/src/cmd/update.rs`
  line 261 prints `"    {}"` via `p.display()` with no `.zero/`-prefix
  assumption, so an op on the bare `AGENTS.md` path renders cleanly.
- **`zero-framework-spec.md` update (spec req 21)** — dropped from this
  plan. The file does not exist in the repo. The scaffold `AGENTS.md`
  edit in Step 3 covers the user-visible documentation; no separate
  framework-spec file needs touching.
- **First-time-after-upgrade UX** — no code change. A user upgrading from
  a pre-this-issue version sees an `update:` row for `AGENTS.md` on the
  next `zero update` if the template has drifted; they can accept or
  decline in `i` mode. Worth a changelog line for the shipping version;
  not authored here.

## Steps

- [x] **Step 1: Move `AGENTS.md` into `framework_manifest()`, relax the `update` boundary check, and retrofit existing tests**
- [x] **Step 2: Add new tests for the AGENTS.md refresh behavior**
- [x] **Step 3: Update the scaffold `AGENTS.md` and add a supersession note to `issues/update/spec.md`**

---

## Step Details

### Step 1: Move `AGENTS.md` into `framework_manifest()`, relax the `update` boundary check, and retrofit existing tests

**Goal:** Land the structural change atomically. After this step,
`AGENTS.md` is a framework-owned file: `zero init` still writes it (via
`write_framework_files`), `zero update` will refresh it when it drifts,
and `apply` permits writing the one named root-level exception. All
existing tests are updated so the suite stays green; no new tests are
introduced (Step 2 covers those). This step is single because the
changes are interdependent — any subset leaves the test suite red.

**Files:**

- `crates/zero-scaffold/src/lib.rs` — manifest entry, drop AGENTS.md
  write from `write_user_files`, fix three tests.
- `crates/zero/src/cmd/update.rs` — relax `apply`'s boundary check; fix
  the `update_with_empty_dot_zero_dir_proposes_only_adds` test.
- `crates/zero/src/cmd/init.rs` — drop `AGENTS.md` from the hardcoded
  user-files list in `render_init_plan` (it now renders via the
  framework-manifest loop).

**Changes:**

1. **Manifest entry.** In `crates/zero-scaffold/src/lib.rs::framework_manifest()`
   (line 118), insert `("AGENTS.md", TPL_AGENTS_MD)` as the first entry of
   the returned `vec![...]`, above `(".zero/zero.d.ts", ...)`. No other
   entries reorder.

2. **Drop the AGENTS.md write from `write_user_files`.** Delete line 242:

   ```rust
   fs::write(root_dir.join("AGENTS.md"), TPL_AGENTS_MD)?;
   ```

   No other changes to `write_user_files`. `write_initial_project`
   continues to call `write_user_files` then `write_framework_files`; the
   second call now writes `AGENTS.md` via the manifest loop.

3. **Retrofit `framework_manifest_matches_expected_path_set`.** In the
   `expected` set at `crates/zero-scaffold/src/lib.rs:915`, add
   `"AGENTS.md"` (as the first entry, mirroring manifest position). The
   `assert_eq!(manifest.len(), expected.len(), ...)` line below it (line
   985–989) needs no change because both sides grew by one.

4. **Retrofit `write_initial_project_emits_user_files`.** Remove the
   AGENTS.md assertion block at lines 479–480:

   ```rust
   let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();
   assert!(!agents.is_empty());
   ```

5. **Retrofit `write_initial_project_emits_framework_files`.** Append an
   `AGENTS.md` assertion to the existing block (after the last existing
   file-read in that test, before the binary-asset loop at line 532):

   ```rust
   let agents = fs::read_to_string(root.join("AGENTS.md")).unwrap();
   assert!(!agents.is_empty(), "AGENTS.md is empty");
   ```

6. **Retrofit `write_framework_files_writes_only_dot_zero`.** Rename the
   test to `write_framework_files_writes_only_dot_zero_and_agents_md`.
   Replace the existing single-entry assertion block (lines 1094–1101)
   with a two-entry one:

   ```rust
   let mut entries: Vec<String> = fs::read_dir(&root)
       .unwrap()
       .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
       .collect();
   entries.sort();
   assert_eq!(
       entries,
       vec![".zero".to_string(), "AGENTS.md".to_string()],
       "write_framework_files wrote unexpected root-level entries: {entries:?}"
   );
   ```

   Also add an assertion that `AGENTS.md` was actually written to the
   root in the for-loop at line 1081 (manifest-iteration block already
   covers this since `AGENTS.md` is now in the manifest — verify by
   inspection that the existing `root.join(rel).exists()` assertion
   inside the loop succeeds for `rel == "AGENTS.md"`; no extra code
   needed).

7. **Relax the boundary check in `apply`.** In
   `crates/zero/src/cmd/update.rs::apply` (line 292), replace the
   `dot_zero` block. Current code:

   ```rust
   let dot_zero = root.join(".zero");
   for op in ops {
       let rel = match op { ... };
       let abs = root.join(rel);
       if !abs.starts_with(&dot_zero) {
           anyhow::bail!(
               "zero update: refusing to touch path outside .zero/: {}",
               abs.display()
           );
       }
       ...
   }
   ```

   New code:

   ```rust
   let dot_zero = root.join(".zero");
   let agents_md = root.join("AGENTS.md");
   for op in ops {
       let rel = match op { ... };
       let abs = root.join(rel);
       if !abs.starts_with(&dot_zero) && abs != agents_md {
           anyhow::bail!(
               "zero update: refusing to touch path outside .zero/ (other than AGENTS.md): {}",
               abs.display()
           );
       }
       ...
   }
   ```

   Move the `let agents_md` declaration up alongside `let dot_zero` so it
   is computed once outside the loop. No other change to `apply`.

8. **Retrofit the existing boundary-check test.** In
   `crates/zero/src/cmd/update.rs::tests::apply_refuses_path_outside_dot_zero`
   (line 588), update the substring assertion to match the new error
   wording:

   ```rust
   assert!(
       err.to_string().contains("outside .zero/ (other than AGENTS.md)"),
       "unexpected error: {err}"
   );
   ```

9. **Retrofit `update_with_empty_dot_zero_dir_proposes_only_adds`.** The
   test (around line 658) deletes everything under `<root>/.zero/` then
   asserts `plan.len() == framework_manifest().len() + binary_manifest().len()`.
   `AGENTS.md` is at the root, not under `.zero/`, so it is not touched
   by the test setup and `compute_plan` will report no op for it.
   Update the length assertion to filter for `.zero/`-prefixed entries:

   ```rust
   let expected_adds = framework_manifest()
       .iter()
       .filter(|(p, _)| p.starts_with(".zero/"))
       .count()
       + zero_scaffold::binary_manifest().len();
   assert_eq!(
       plan.len(),
       expected_adds,
       "expected one Add per .zero/ manifest entry, got {plan:?}"
   );
   ```

   The inner per-op `matches!(op, Operation::Add(_))` loop below is fine
   as written.

10. **Drop `AGENTS.md` from the `render_init_plan` user-files list.** In
    `crates/zero/src/cmd/init.rs::render_init_plan` (line 87–96), remove
    the `"AGENTS.md",` entry from the hardcoded `for path in [...]`
    array. `AGENTS.md` now renders via the
    `for (path, _) in framework_manifest()` loop above it.

    The existing test `init_plan_lists_framework_and_user_groups` (line
    148) does not need changes — its assertions check for the section
    headings, `.zero/styles/_tokens.scss`, `styles/app.scss`, and
    `.gitignore`; none of those are affected. But its coverage is
    incomplete for the new behavior (it does not assert `AGENTS.md`
    placement). Step 2 adds an assertion that pins this.

**Tests:** No new tests added in this step. Verify with:

```
cargo test --workspace
```

Expected: all existing tests pass after the retrofits above. The
integration test `tests/update.rs::update_restores_modified_recreates_deleted_removes_stray`
already asserts `AGENTS.md` bytes are equal pre- and post-update —
because the test does not mutate `AGENTS.md`, the assertion still holds
even though `AGENTS.md` is now framework-owned.

---

### Step 2: Add new tests for the AGENTS.md refresh behavior

**Goal:** Pin the new behavior so a future regression (e.g. someone
moving `AGENTS.md` back to `write_user_files`, or tightening the
boundary check) breaks something explicit. Adds three unit tests in
`update.rs`, one in `init.rs`, and one integration test in
`tests/update.rs`.

**Files:**

- `crates/zero/src/cmd/update.rs` — three new unit tests in `mod tests`.
- `crates/zero/src/cmd/init.rs` — one new unit test in `mod tests`.
- `crates/zero/tests/update.rs` — one new integration test.
- `crates/zero-scaffold/src/lib.rs` — one new unit test for the manifest
  entry (positions `AGENTS.md` correctly).

**Changes:**

1. **`crates/zero-scaffold/src/lib.rs::tests`** — add:

   ```rust
   #[test]
   fn framework_manifest_includes_agents_md_first() {
       let manifest = framework_manifest();
       let first = manifest.first().expect("manifest is non-empty");
       assert_eq!(first.0, "AGENTS.md", "AGENTS.md must be first entry");
       assert!(!first.1.is_empty(), "AGENTS.md template is empty");
   }
   ```

2. **`crates/zero/src/cmd/update.rs::tests`** — add three tests next to
   the existing `update_with_*` tests:

   ```rust
   #[test]
   fn update_with_modified_agents_md_proposes_update() {
       let (_dir, root) = scaffold();
       fs::write(root.join("AGENTS.md"), b"# mutated\n").unwrap();
       let plan = compute_plan(&root).unwrap();
       assert!(
           plan.contains(&Operation::Update(PathBuf::from("AGENTS.md"))),
           "plan missing Update for AGENTS.md: {plan:?}"
       );
   }

   #[test]
   fn update_with_missing_agents_md_proposes_add() {
       let (_dir, root) = scaffold();
       fs::remove_file(root.join("AGENTS.md")).unwrap();
       let plan = compute_plan(&root).unwrap();
       assert!(
           plan.contains(&Operation::Add(PathBuf::from("AGENTS.md"))),
           "plan missing Add for AGENTS.md: {plan:?}"
       );
   }

   #[test]
   fn update_does_not_propose_remove_for_root_files() {
       let (_dir, root) = scaffold();
       fs::write(root.join("README.md"), b"# stray root file\n").unwrap();
       let plan = compute_plan(&root).unwrap();
       for op in &plan {
           if let Operation::Remove(p) = op {
               assert!(
                   p.starts_with(".zero/"),
                   "compute_plan proposed Remove for a non-.zero/ path: {p:?}"
               );
           }
       }
       // And specifically: no Remove for README.md.
       assert!(
           !plan.contains(&Operation::Remove(PathBuf::from("README.md"))),
           "plan must not Remove root-level files: {plan:?}"
       );
   }
   ```

   Plus one boundary-check positive case (sibling of
   `apply_refuses_path_outside_dot_zero`):

   ```rust
   #[test]
   fn apply_permits_agents_md_at_root() {
       let (_dir, root) = scaffold();
       // Mutate AGENTS.md so the manifest write has visible effect.
       fs::write(root.join("AGENTS.md"), b"# mutated\n").unwrap();
       let ops = vec![Operation::Update(PathBuf::from("AGENTS.md"))];
       apply(&root, &ops).expect("apply must accept AGENTS.md at root");

       // Content was rewritten from the embedded template.
       let after = fs::read_to_string(root.join("AGENTS.md")).unwrap();
       assert!(
           !after.contains("# mutated"),
           "AGENTS.md not restored from manifest: {after}"
       );
       assert!(
           after.contains("# Zero — Agent & Developer Reference"),
           "AGENTS.md missing canonical heading after apply: {after}"
       );
   }
   ```

3. **`crates/zero/src/cmd/init.rs::tests`** — add:

   ```rust
   #[test]
   fn init_plan_lists_agents_md_under_framework_files() {
       let plan = render_init_plan();
       let framework_idx = plan
           .find("framework files (regenerable, under .zero/)")
           .expect("framework header present");
       let user_idx = plan.find("user files").expect("user header present");
       let agents_idx = plan
           .find("AGENTS.md")
           .expect("AGENTS.md present in plan");
       assert!(
           agents_idx > framework_idx && agents_idx < user_idx,
           "AGENTS.md must be under framework files, not user files: {plan}"
       );
       // And it must not appear a second time below the user-files heading.
       assert_eq!(
           plan.matches("AGENTS.md").count(),
           1,
           "AGENTS.md should appear exactly once: {plan}"
       );
   }
   ```

4. **`crates/zero/tests/update.rs`** — add (after the existing
   `update_on_clean_project_is_noop`):

   ```rust
   #[test]
   fn update_restores_modified_agents_md() {
       let tmp = tempdir().unwrap();
       let web = init_project(tmp.path());
       let original = fs::read(web.join("AGENTS.md")).unwrap();
       fs::write(web.join("AGENTS.md"), b"# mutated\n").unwrap();

       Command::cargo_bin("zero")
           .unwrap()
           .arg("update")
           .arg("--yes")
           .current_dir(tmp.path())
           .assert()
           .success();

       assert_eq!(
           fs::read(web.join("AGENTS.md")).unwrap(),
           original,
           "AGENTS.md not restored to embedded template"
       );
   }
   ```

**Tests:** All tests above. Run:

```
cargo test --workspace
```

Expected: every new test passes alongside the existing suite.

---

### Step 3: Update the scaffold `AGENTS.md` and add a supersession note to `issues/update/spec.md`

**Goal:** Documentation. Tell readers of `AGENTS.md` that the file is
framework-owned despite sitting at the project root, and record the
revised decision in the original `issues/update/spec.md`.

**Files:**

- `crates/zero-scaffold/src/scaffold/AGENTS.md` — append a paragraph to
  the `## The .zero/ directory` section.
- `issues/update/spec.md` — prepend a short supersession note above the
  existing `# Spec:` heading.
- `crates/zero-scaffold/src/lib.rs::tests` — add one assertion in the
  existing `write_initial_project_agents_md_has_section_sentinels` test
  (or a new dedicated test) that pins the new sentence.

**Changes:**

1. **`crates/zero-scaffold/src/scaffold/AGENTS.md`** — at the end of the
   `## The .zero/ directory` section (after the
   `"Update with `zero update` (interactive plan) or `zero update --yes` (CI)."`
   line at line 294, before the `---` separator at line 296), append:

   ```markdown

   `AGENTS.md` itself sits at the project root (so Claude Code, Cursor,
   and other tools that read a root-level `AGENTS.md` find it) but is
   framework-owned just like the files under `.zero/`. `zero update`
   refreshes it. Do not put project-specific agent guidance here — it
   will be overwritten on the next update. Keep your own notes in a
   separate file you maintain.
   ```

2. **`issues/update/spec.md`** — prepend, above the existing first line
   (`# Spec: \`zero update\` and the \`.zero/\` framework directory`):

   ```markdown
   > **Note:** The decision to keep `AGENTS.md` user-owned (see this
   > spec's Problem Statement and the constraint "`zero update` never
   > writes outside `.zero/`") is superseded by
   > `issues/agents-update/spec.md`. `AGENTS.md` is now framework-owned
   > and refreshed by `zero update`. It remains at the project root via
   > a single named exception to the `.zero/`-only write boundary.

   ```

   (Blank line after the blockquote so the existing `# Spec:` heading
   parses cleanly as a heading.)

3. **`crates/zero-scaffold/src/lib.rs::tests`** — extend the existing
   `write_initial_project_agents_md_has_section_sentinels` (line 578) by
   adding a new substring assertion at the bottom of the function:

   ```rust
   assert!(
       agents.contains("framework-owned just like the files under `.zero/`"),
       "AGENTS.md missing framework-ownership note in the .zero/ section: {agents}"
   );
   ```

   (Or, equivalently, drop a distinct one-test wrapper if the existing
   test is judged too noisy to extend — at executor's discretion.)

**Tests:**

- `cargo test -p zero-scaffold` — the extended sentinel assertion
  passes.
- Manual scan of the rendered `AGENTS.md` (open the file after a
  scaffolded init in a tempdir) to confirm the new paragraph reads
  cleanly. Optional but recommended.

---

## Risks and Assumptions

- **`render_init_plan` output regression.** Step 1 change #10 removes
  `AGENTS.md` from the hardcoded user-files array. If the executor
  forgets this and only adds AGENTS.md to the manifest, the file will
  list under both groups. Step 2's
  `init_plan_lists_agents_md_under_framework_files` test pins this; if
  it fails, that is the fix.
- **`abs != agents_md` equality semantics.** The `apply` boundary check
  uses path equality. Both `abs` and `agents_md` are constructed via
  `root.join(...)`. As long as `root` is canonicalized identically in
  both, the equality holds. If `apply` ever starts taking a
  non-canonical `root` (e.g. one with `..` components) and the rel path
  is `"AGENTS.md"`, equality still holds because both are built from
  the same `root`. No platform-specific separator issues (the path is a
  literal `"AGENTS.md"` with no separator).
- **`compute_plan`'s Remove walk.** Remove operations are still walked
  only under `<root>/.zero/`. Adding `AGENTS.md` to the manifest does
  not introduce remove walks at the root; the
  `update_does_not_propose_remove_for_root_files` test pins this. If a
  future change generalizes the walk, that test will fail loudly.
- **Existing-project upgrade UX.** Users on a pre-this-issue scaffold
  who have edited their `AGENTS.md` will see an `update:` op on the
  next `zero update`. They can decline (top-level `n` or per-file `n`
  in interactive mode). This is the intended behavior and matches the
  existing treatment of `.zero/` files. A changelog line for the
  shipping release would help; not authored in this plan.
- **`zero-framework-spec.md` absence.** Spec req 21 asked for an update
  to that file, but the file is not in the repo. Dropped from the plan;
  no replacement document is created because the scaffold `AGENTS.md`
  edit in Step 3 covers user-visible documentation.
- **Manifest ordering churn.** Placing `AGENTS.md` first in
  `framework_manifest()` changes the order in which `compute_plan`
  emits Adds/Updates. The plan output is grouped by op type before
  printing (see `render_plan` at line 244), so ordering within a group
  is the only visible effect. Tests assert membership, not position
  within a group, so no further updates are needed.
