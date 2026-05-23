# Spec: `AGENTS.md` refreshed by `zero update`

## Problem Statement

`AGENTS.md` is the in-project reference an agent or developer consults to
write code against the current zero API. Its contents track the framework's
shipped surface — lint rules, component library, JSDoc conventions, the list
of available imports, the design-system layout — and that surface is exactly
the kind of thing `zero update` exists to refresh in existing projects.

Today, `AGENTS.md` is written once by `zero init` (via `write_user_files`)
and treated as user-owned, one-shot. The original `issues/update/spec.md`
explicitly excluded it from `zero update` ("`AGENTS.md` stays user-owned").
The result: every shipped change to the agent reference — new components,
new lint rules, renamed tokens, removed sections — is invisible to projects
scaffolded against an older CLI. Existing users are stuck reading whichever
snapshot of `AGENTS.md` they scaffolded against, even after they upgrade the
binary and run `zero update`.

This issue moves `AGENTS.md` into the framework-owned manifest so
`zero update` refreshes it the same way it refreshes the type definitions,
the design-system partials, and the component library.

## Background

### Where `AGENTS.md` lives today

- Template: `crates/zero-scaffold/src/scaffold/AGENTS.md`
- Embedded as `TPL_AGENTS_MD` in `crates/zero-scaffold/src/lib.rs:30`.
- Written at the project root by `write_user_files` in the same file at
  line 242, alongside `index.html`, `tsconfig.json`, `src/*`, `.gitignore`,
  and `styles/app.scss`.
- It is **not** listed in `framework_manifest()` (lines 118–186), so
  `zero update` ignores it.

### The boundary `zero update` currently enforces

`crates/zero/src/cmd/update.rs::apply` (line 292) refuses to write any path
that is not a descendant of `<root>/.zero/`:

```rust
let dot_zero = root.join(".zero");
if !abs.starts_with(&dot_zero) {
    anyhow::bail!("zero update: refusing to touch path outside .zero/: ...");
}
```

This invariant was explicit in `issues/update/spec.md` (constraints section)
and is exercised by a unit test in `crates/zero/src/cmd/update.rs:594`. It is
the protection that lets users trust `zero update` to leave their `src/`,
`styles/app.scss`, `tsconfig.json`, `index.html`, and `.gitignore` alone.

### Why `AGENTS.md` cannot simply move into `.zero/`

`AGENTS.md` is a filename convention: Claude Code, Cursor, and other agent
tooling read the file at the **project root**. Moving it under `.zero/`
breaks discovery — the file would no longer be found by the tools it exists
to serve. So this issue keeps `AGENTS.md` at the root and introduces a
single, named exception to the `.zero/`-only boundary rather than moving the
file.

### Compute-plan side

`crates/zero/src/cmd/update.rs::compute_plan` (line 161) iterates the
manifest first to detect Add/Update operations, then walks `<root>/.zero/`
to detect Remove operations. Because the Remove walk is scoped to
`.zero/`, adding a single root-level entry to the manifest produces Add /
Update behavior for that path **without** the walker ever considering other
root-level files for removal. The asymmetry is intentional and load-bearing:
manifest membership decides what gets written; `.zero/` membership decides
what gets removed.

### Interactive accept/reject is already supported

Per `issues/update/spec.md` requirement 17, `zero update` already supports
`Y` / `n` / `i` at the top-level prompt, with `i` entering interactive mode
where every operation gets a per-file `y` / `n` prompt. A user who has hand-
edited their `AGENTS.md` and does not want it clobbered already has the
declination mechanism — no new flag needed.

## Requirements

### Manifest membership

1. **`AGENTS.md` joins `framework_manifest()`.** Add an entry
   `("AGENTS.md", TPL_AGENTS_MD)` to the vector returned by
   `framework_manifest()` in `crates/zero-scaffold/src/lib.rs`. Place it
   first (above the `.zero/zero.d.ts` entry) so the plan output groups it
   visibly at the top.

2. **`AGENTS.md` is removed from `write_user_files`.** Delete the
   `fs::write(root_dir.join("AGENTS.md"), TPL_AGENTS_MD)?;` line in
   `write_user_files`. After this change, `write_framework_files` is the
   only path that writes `AGENTS.md` — and `write_initial_project` still
   produces the same on-disk result because it calls
   `write_framework_files` after `write_user_files`.

3. **No new template file.** The existing
   `crates/zero-scaffold/src/scaffold/AGENTS.md` is the same content,
   just re-categorized.

### Boundary check

4. **`apply` allows `<root>/AGENTS.md` in addition to `<root>/.zero/`.**
   Update the boundary check in `crates/zero/src/cmd/update.rs::apply`:

   ```rust
   let dot_zero = root.join(".zero");
   let agents_md = root.join("AGENTS.md");
   if !abs.starts_with(&dot_zero) && abs != agents_md {
       anyhow::bail!(
           "zero update: refusing to touch path outside .zero/ (other than AGENTS.md): {}",
           abs.display()
       );
   }
   ```

   The error message updates to reflect the new rule. No other paths are
   permitted outside `.zero/`.

5. **The boundary check is the only relaxation.** Every other invariant
   from `issues/update/spec.md` holds: `update` still refuses to write any
   path that is neither under `.zero/` nor exactly `AGENTS.md`; `compute_plan`
   still walks only `.zero/` for removals; the manifest is still the single
   source of truth.

### Init behavior

6. **`zero init` continues to write `AGENTS.md` once.** Because
   `write_initial_project` calls `write_framework_files` after
   `write_user_files`, and `AGENTS.md` now lives in the framework manifest,
   `init` still produces a project root with `AGENTS.md` populated. The
   visible behavior of `zero init` does not change.

7. **The init pre-flight plan groups `AGENTS.md` under "framework files".**
   In `render_init_plan()` (called by `zero init` before any write), the
   `AGENTS.md` line moves from the "user files" group to the "framework
   files (regenerable)" group. Update the helper accordingly.

### Update behavior

8. **`zero update` on a fresh post-init project is still a no-op.** Since
   the byte content of `AGENTS.md` on disk matches `TPL_AGENTS_MD` after
   init, `compute_plan` reports no drift and `update` exits with
   `"zero update: .zero/ is already up to date."`.

   The wording of that no-op message is not in scope for this issue — it
   continues to mention `.zero/` even though `AGENTS.md` is also tracked.
   Adjust only if it reads as actively wrong; otherwise leave alone.

9. **When `AGENTS.md` is mutated, `update` proposes an `Update`.** A user
   who has edited their `AGENTS.md` sees an `update:` line for `AGENTS.md`
   in the plan output. Accepting overwrites; declining (top-level `n`, or
   per-file `n` in interactive mode) leaves the user's content intact.
   Re-running `update` re-offers it.

10. **When `AGENTS.md` is missing, `update` proposes an `Add`.** If the
    user deleted their `AGENTS.md`, `update` recreates it from the
    embedded template.

11. **`update` never proposes a `Remove` for `AGENTS.md`.** The remove
    walker is scoped to `<root>/.zero/`; root-level files are never in
    that set. This is structurally guaranteed, not a special case.

### Scaffold `AGENTS.md` content

12. **The `## The .zero/ directory` section gains a short note about
    `AGENTS.md`.** The current section describes `.zero/` as the framework-
    owned, regenerable boundary. Append a sentence (or short paragraph)
    along the lines of: "Although `AGENTS.md` lives at the project root
    (so tools like Claude Code and Cursor find it), it is framework-owned
    just like the files under `.zero/` and is refreshed by `zero update`.
    Project-specific agent guidance should live in a separate file you
    maintain, not in `AGENTS.md`."

13. **No other scaffold content changes.** This issue is structural — it
    moves `AGENTS.md` between two categories. Content edits to surface new
    framework features ride in their own issues.

### Tests

14. **`framework_manifest_matches_expected_path_set` updated.** The
    expected `BTreeSet` in `crates/zero-scaffold/src/lib.rs:912` gains a
    `"AGENTS.md"` entry. The path-count assertion below it updates if it
    is a literal (currently `assert_eq!(manifest.len(), expected.len())`,
    so adding to both sides keeps it consistent).

15. **`write_framework_files_writes_only_dot_zero` updated.** The test at
    `crates/zero-scaffold/src/lib.rs:1074` asserts that only `.zero` lives
    at the root after `write_framework_files`. Rename and rewrite to allow
    `AGENTS.md` as well — the new assertion is that the root directory
    contains exactly two entries (`.zero` and `AGENTS.md`), nothing else.
    Suggested new name: `write_framework_files_writes_only_dot_zero_and_agents_md`.

16. **`write_initial_project_emits_user_files` no longer asserts
    `AGENTS.md`.** The `let agents = fs::read_to_string(...)` block at
    `crates/zero-scaffold/src/lib.rs:479` moves to
    `write_initial_project_emits_framework_files` (or to a new test
    `write_initial_project_emits_agents_md_as_framework_file`), keeping the
    `assert!(!agents.is_empty())` assertion intact.

17. **Boundary-check test updated.** The unit test in
    `crates/zero/src/cmd/update.rs` that exercises the
    `"outside .zero/"` error (around line 594) keeps its existing positive
    case (a path like `outside.txt` still errors) and gains a new negative
    case asserting that `AGENTS.md` does **not** trigger the error.

18. **New unit tests in `crates/zero/src/cmd/update.rs::tests`** (alongside
    the existing `update_with_*_proposes_*` tests):
    - `update_with_modified_agents_md_proposes_update` — scaffold a
      project; overwrite `AGENTS.md` with `"# mutated"`; call
      `compute_plan`; assert plan contains
      `Operation::Update("AGENTS.md".into())`.
    - `update_with_missing_agents_md_proposes_add` — scaffold; delete
      `AGENTS.md`; call `compute_plan`; assert plan contains
      `Operation::Add("AGENTS.md".into())`.
    - `update_does_not_propose_remove_for_root_files` — scaffold; create
      `<root>/README.md` (stray root-level file); call `compute_plan`;
      assert the plan contains **no** `Remove` for `README.md`. This
      pins the asymmetry: root-level files are not in the remove walk.

19. **Integration test updated.** The existing `tests/update.rs`
    integration test (per `issues/update/spec.md` §34) currently asserts
    that `AGENTS.md` bytes are unchanged after `zero update --yes`. Now
    that `AGENTS.md` is framework-owned, this assertion still holds when
    the user has not modified the file. Add a second integration test in
    the same file:

    ```rust
    #[test]
    fn update_restores_modified_agents_md() {
        let tmp = tempdir().unwrap();
        init_project(tmp.path());
        let original = fs::read(tmp.path().join("AGENTS.md")).unwrap();
        fs::write(tmp.path().join("AGENTS.md"), b"# mutated\n").unwrap();

        Command::cargo_bin("zero").unwrap()
            .arg("update").arg("--yes")
            .current_dir(tmp.path()).assert().success();

        assert_eq!(fs::read(tmp.path().join("AGENTS.md")).unwrap(), original);
    }
    ```

### Documentation

20. **`issues/update/spec.md` gets a supersession note.** Prepend (above
    the existing `# Spec:` heading) a short note: "Requirement that
    `AGENTS.md` remains user-owned is superseded by
    `issues/agents-update/spec.md`: `AGENTS.md` is now framework-owned and
    refreshed by `zero update`, kept at the project root via a single
    named exception to the `.zero/`-only write boundary." The body of the
    older spec is not rewritten — it captured the design at a point in
    time.

21. **`zero-framework-spec.md` `zero update` subsection updated.** The
    section that lists what `zero update` touches (added in the update
    issue) currently says or implies "files under `.zero/`". Change to
    "files under `.zero/`, plus `AGENTS.md` at the project root", with a
    one-sentence explanation that `AGENTS.md` is kept at the root for tool
    discovery but is framework-owned.

22. **Scaffold `AGENTS.md` updated per requirement 12.**

## Constraints

- **Single named exception.** The boundary relaxation is for the literal
  path `AGENTS.md` at the project root. No glob, no list, no env-var
  override. Future framework files that need to live at the root require
  their own spec adding their own named exception (or — preferred — go
  under `.zero/`).
- **No new CLI surface.** No new flag, no new subcommand, no new prompt.
  The `Y` / `n` / `i` flow inherited from the original `update` spec is
  the user's only knob.
- **No content migration.** Users on the old "AGENTS.md is user-owned"
  model who have edited their `AGENTS.md` see an `update:` operation on
  their first `zero update` post-upgrade and can decline. The framework
  does not attempt to detect, diff, merge, or warn about local edits
  beyond what the existing operation-type signal already shows.
- **No backwards-compatibility shim.** The change to `write_user_files`
  removes the `AGENTS.md` write outright; there is no flag, env var, or
  legacy code path. The two functions `write_user_files` and
  `write_framework_files` continue to compose cleanly via
  `write_initial_project`.
- **`compute_plan` is not generalized.** It continues to walk `.zero/`
  for removals only. The exception path for `AGENTS.md` is **add/update
  only**; removal is structurally impossible. Do not add a special-case
  walk over root-level files.
- **Stable plan ordering.** `AGENTS.md` is placed first in
  `framework_manifest()` so it appears at a predictable position in the
  plan output. If the manifest order is later sorted alphabetically, that
  is a separate concern.

## Out of Scope

- **Moving `AGENTS.md` under `.zero/`.** Rejected in the refine session:
  Claude Code, Cursor, and similar tools read root-level `AGENTS.md`;
  hiding it breaks discovery.
- **Adding other root-level framework files.** This spec admits exactly
  one root-level exception. `README.md`, `LICENSE`, editor-config files,
  CI scripts, etc. are out of scope.
- **Per-project agent notes.** A "project-specific" companion file (e.g.
  `AGENTS.local.md`) that the user maintains alongside the framework-
  owned `AGENTS.md` is a reasonable future addition. Not in this issue.
- **Diff rendering.** `update` continues to show only operation type +
  path. No content diff for `AGENTS.md` (or anything else) in v1 of this
  change. Pulled forward from the original update spec.
- **Detection of meaningful user edits.** No heuristics (signature
  comments, hash database, marker sections) to detect whether a user
  edited `AGENTS.md` "meaningfully" vs. cosmetically. Either the bytes
  match the manifest or they do not.
- **Re-introducing the user-owned distinction in plan output.** The
  init pre-flight plan groups `AGENTS.md` under "framework files" per
  requirement 7. No "framework files plus one root-level file" sub-
  grouping.
- **Editing the scaffold `AGENTS.md` to add new framework surface.**
  Content edits ride in their own issues (e.g., a future "document
  feature X" issue). This issue ships the redistribution mechanism only.

## Open Questions

- **Exact wording of the boundary-check error.** Suggested:
  `"zero update: refusing to touch path outside .zero/ (other than AGENTS.md): <path>"`.
  Plan phase may shorten if it reads awkwardly in the test fixtures.
- **Manifest position.** Spec places `AGENTS.md` first in
  `framework_manifest()`. If the existing manifest order has an implicit
  contract (e.g., type defs first because `compute_plan` writes adds in
  order), the plan phase should confirm "first entry" doesn't violate
  that.
- **Boundary check: equality vs. `starts_with`.** The spec uses
  `abs != agents_md` (exact equality) rather than
  `abs.starts_with(&agents_md)`. Equality is correct (`AGENTS.md` is a
  file, not a directory); confirm this matches how `abs` is canonicalized
  in `apply` before the check.
- **`render_plan` output.** When the plan contains an `Update` for
  `AGENTS.md`, the rendered output shows the literal path `AGENTS.md`
  (no `.zero/` prefix). Plan phase should confirm the existing
  `render_plan` does not assume a `.zero/` prefix when grouping or
  printing.
- **First-time-after-upgrade UX.** A user who upgrades the CLI from a
  pre-this-issue version and runs `zero update` will see an `update:`
  operation on `AGENTS.md` for the first time (because the embedded
  template has likely drifted between binary versions). This is the
  intended effect. Worth a sentence in the release notes for the version
  that ships this change; not a code change. Plan should flag this for
  the changelog entry but does not need to author it.
