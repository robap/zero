> **Note:** The decision to keep `AGENTS.md` user-owned (see this
> spec's Problem Statement and the constraint "`zero update` never
> writes outside `.zero/`") is superseded by
> `issues/agents-update/spec.md`. `AGENTS.md` is now framework-owned
> and refreshed by `zero update`. It remains at the project root via
> a single named exception to the `.zero/`-only write boundary.

# Spec: `zero update` and the `.zero/` framework directory

## Problem Statement

`zero init` writes a fixed set of files into a new project and never touches them again. Two problems flow from that:

1. **No upgrade path.** When a new CLI version ships additional framework assets (more CSS utilities, a new shared SCSS partial, an expanded `zero.d.ts`, future component-library files), existing projects have no way to pick them up. Each user is on whatever snapshot `zero init` wrote when their project was scaffolded. The framework can evolve but projects cannot follow.
2. **No boundary between framework files and user files.** Today the scaffold writes `zero.d.ts`, `_tokens.scss`, `_base.scss`, `_layout.scss`, `_utilities.scss`, and `app.scss` into directories that look identical to user code. An agent (or the user) can edit `_tokens.scss` thinking it is their own file. Nothing in the project layout signals "this is owned by the framework — don't touch."

This issue introduces:

- A hidden, framework-owned `.zero/` directory at the project root holding regenerable framework assets. It is added to `.gitignore` by `zero init`.
- A new CLI command, `zero update`, that rewrites `.zero/` from the embedded binary. It never touches user-owned files.
- A new pre-flight confirmation step on **both** `zero init` and `zero update`: the command prints the list of operations it intends to perform and waits for the user's approval. `zero update` additionally supports per-operation accept/reject.

The result: framework assets become a versioned, regenerable surface that lives off to the side of user code, and the user has an explicit upgrade lever. The previous design-system decision that "files are user-owned post-init" (`issues/design-system/spec.md`) is **revised** for the four design-system partials and the two `.d.ts` files — they now move to `.zero/` and are framework-owned. The entry `styles/app.scss`, scaffolded `src/` files, `index.html`, `tsconfig.json`, and `AGENTS.md` remain user-owned and one-shot, written by `zero init` and never touched again.

## Background

### What `zero init` writes today

From `src/scaffold.rs::write_to` and `src/cmd/init.rs::run`:

| Path | Today's owner | Becomes |
| --- | --- | --- |
| `index.html` | user | user (unchanged) |
| `tsconfig.json` | user | user (unchanged; `"types"` paths updated to point into `.zero/`) |
| `src/app.ts` | user | user (unchanged) |
| `src/routes/home.ts` | user | user (unchanged) |
| `src/routes/home.test.ts` | user | user (unchanged) |
| `styles/app.scss` | user | user (rewritten to a one-line `@use '../.zero/styles/zero';` + a comment-marked space for user rules) |
| `styles/_tokens.scss` | "user-owned post-init" | **moves to `.zero/styles/_tokens.scss`** |
| `styles/_base.scss` | "user-owned post-init" | **moves to `.zero/styles/_base.scss`** |
| `styles/_layout.scss` | "user-owned post-init" | **moves to `.zero/styles/_layout.scss`** |
| `styles/_utilities.scss` | "user-owned post-init" | **moves to `.zero/styles/_utilities.scss`** |
| `zero.d.ts` (project root) | "user-owned" | **moves to `.zero/zero.d.ts`** |
| `zero-test.d.ts` (project root) | "user-owned" | **moves to `.zero/zero-test.d.ts`** |
| `AGENTS.md` | user | user (unchanged) |
| **new:** `.zero/styles/zero.scss` | framework | aggregate that does `@use 'tokens'; @use 'base'; @use 'layout'; @use 'utilities';` (today's `styles/app.scss` body, moved) |
| **new:** `.gitignore` | user (one-shot) | created with a `.zero/` entry (init only writes if `.gitignore` does not already exist) |

### Existing `init` flow (`src/cmd/init.rs`)

`run()` today:
1. Reads or prompts for `zero.toml` config (via `prompt_user()` in `src/prompts.rs`).
2. Determines `root_dir` from config.
3. Refuses if `root_dir` exists and is non-empty.
4. Calls `scaffold::write_to(root_dir, ctx)` — which unconditionally writes every file.
5. Prints `"Scaffold written to ./<root>/ — run \`zero dev\` to start."`.

No confirmation step today. The new flow inserts a confirmation between step 3 and step 4.

### Existing scaffold mechanism (`src/scaffold.rs`)

Embedded templates are `include_str!` constants. `write_to` is one straight-line function that writes each constant to its target path with `fs::write`. Tests in `src/scaffold.rs` assert specific files land with specific markers. The new layout needs new constants for the `.zero/` files and a clear split between the "user-owned, one-shot" template set and the "framework-owned, regenerable" template set.

### Design-system spec interaction

`issues/design-system/spec.md` previously stated:

> The scaffold ships a built-in design-system layer in `styles/`: five files (`_tokens.scss`, `_base.scss`, `_layout.scss`, `_utilities.scss`, `app.scss`) that establish a stable foundation for the future component library. After `zero init`, the files are user-owned — editable, deletable, or wholesale replaceable. There is no upgrade path; the framework never patches scaffolded files in-place.

This spec **revises** that decision. Four of those five files (`_tokens.scss`, `_base.scss`, `_layout.scss`, `_utilities.scss`) move into `.zero/styles/` and become framework-owned and regenerable. The fifth (`app.scss`) stays in `styles/` as the user-owned entry and now contains the relative-path import of the framework aggregate. Users who want to override tokens re-declare CSS custom properties in their `app.scss` after the `@use` line — overriding by re-declaration is preserved, just no longer by editing the file that declares the tokens.

The framework-spec section 13 row for "Design system" should be updated to reflect that the partials are framework-owned and live under `.zero/`. The `zero-framework-spec.md` §7.1 paragraph stating the partials are user-owned is **superseded** by this spec.

### Decisions already made in the refine session

- The `.zero/` directory is the boundary between framework-owned and user-owned files. Framework files live in `.zero/`, user files everywhere else.
- `.zero/` is added to `.gitignore` by `zero init`. It is regenerable — agents and users should treat it as off-limits even though it is on disk.
- Type definitions and the four design-system SCSS partials are the v1 contents of `.zero/`. `AGENTS.md` stays user-owned (option 2 from the refine session).
- Two distinct commands. `zero init` keeps its current "refuse non-empty directory" invariant. `zero update` is a separate command that requires an existing project and only touches `.zero/`.
- Both commands print a plan and ask for confirmation before acting. `zero update` additionally supports per-operation accept/reject.
- The user's `styles/app.scss` imports the framework aggregate via a relative path: `@use '../.zero/styles/zero';`. No SCSS resolver changes are required.

## Requirements

### Directory layout

1. After `zero init`, the project root contains:
   ```
   <root>/
   ├── .gitignore                       # contains ".zero/"
   ├── .zero/
   │   ├── zero.d.ts
   │   ├── zero-test.d.ts
   │   └── styles/
   │       ├── _tokens.scss
   │       ├── _base.scss
   │       ├── _layout.scss
   │       ├── _utilities.scss
   │       └── zero.scss
   ├── AGENTS.md
   ├── index.html
   ├── tsconfig.json
   ├── src/
   │   ├── app.ts
   │   └── routes/
   │       ├── home.ts
   │       └── home.test.ts
   └── styles/
       └── app.scss
   ```
2. `.zero/styles/zero.scss` is the framework aggregate and contains exactly:
   ```scss
   @use 'tokens';
   @use 'base';
   @use 'layout';
   @use 'utilities';
   ```
3. `styles/app.scss` is the user's entry. Initial content:
   ```scss
   @use '../.zero/styles/zero';

   // Your styles below.
   ```
4. `tsconfig.json` is updated so the `"types"` array points at `./.zero/zero.d.ts` and `./.zero/zero-test.d.ts` rather than the previous root-level paths. The `"include"` array continues to include `"src"`; agents may need to add `".zero"` to `"include"` if type-resolution requires it (the plan should confirm the exact form during implementation).
5. `index.html` continues to have a single `<link rel="stylesheet" href="/styles/app.scss">`. No second link tag is added; the framework's CSS is reached via the SCSS import in the user's app.scss.
6. The `.gitignore` written by `zero init` contains at minimum a line `.zero/`. If a `.gitignore` already exists in the target directory at the time `init` runs (which can only happen if the user pre-staged one before invoking `init`), `init` refuses non-empty-directory the same way it does today — no special handling.

### `zero init` changes

7. `zero init` writes the directory layout described in requirement 1.
8. `zero init` prints a pre-flight plan before writing any files. The plan lists every file it intends to create, grouped into "framework files (`.zero/`)" and "user files". Example:
   ```
   zero init will create:

     framework files (regenerable, under .zero/)
       .zero/zero.d.ts
       .zero/zero-test.d.ts
       .zero/styles/_tokens.scss
       .zero/styles/_base.scss
       .zero/styles/_layout.scss
       .zero/styles/_utilities.scss
       .zero/styles/zero.scss

     user files
       index.html
       tsconfig.json
       AGENTS.md
       .gitignore
       src/app.ts
       src/routes/home.ts
       src/routes/home.test.ts
       styles/app.scss

   Proceed? [Y/n]
   ```
9. The default answer at the `Proceed?` prompt is `Y`. Pressing Enter accepts; entering `n` or `N` aborts with `"zero init: aborted by user"` and exit code 0 (not an error — explicit user choice). Any other input re-prompts.
10. `zero init` supports a `--yes` / `-y` flag that skips the confirmation prompt and proceeds as if the user answered `Y`. Intended for scripts and CI.
11. The existing prompt phase (`prompt_user()`) runs before the plan/confirm step. The plan reflects the choices the user already made during prompting (e.g. the `project.root` value). The confirmation is the final gate before any disk write.
12. The existing "refuse non-empty `<root>/`" invariant is unchanged. The plan and confirmation only run for a fresh, empty target.

### `zero update` command

13. A new subcommand `zero update`. CLI surface (in the help block in `zero-framework-spec.md` §1 and the `clap` definitions in `src/main.rs`):
    ```
    zero update             Refresh framework files in .zero/
    ```
14. `zero update` requires an existing project. Detection: the current working directory (or `--cwd <path>`, if introduced — out of scope for v1) must contain a `zero.toml` and a `.zero/` directory. If either is missing, `update` exits with a clear error:
    - `"zero update: no zero.toml found — run 'zero init' first"` if `zero.toml` is missing.
    - `"zero update: no .zero/ directory found — this project predates the .zero layout; re-run 'zero init' in a fresh directory or create .zero/ manually"` if `zero.toml` exists but `.zero/` does not. (Migration is not in scope for v1 — see Out of Scope.)
15. `zero update` computes three sets of operations by comparing the current contents of `.zero/` against the file set the binary would emit:
    - **Add** — paths the binary emits that do not exist under `.zero/`.
    - **Update** — paths that exist under `.zero/` but whose contents differ from the binary's emission (compared by exact byte content).
    - **Remove** — paths under `.zero/` that exist on disk but are not in the binary's emission set.
16. If all three sets are empty, `zero update` prints `"zero update: .zero/ is already up to date."` and exits with code 0.
17. Otherwise, `zero update` prints a plan grouped by operation:
    ```
    zero update will perform these operations in .zero/:

      add:
        .zero/styles/_animations.scss

      update:
        .zero/zero.d.ts
        .zero/styles/_tokens.scss

      remove:
        .zero/styles/_legacy.scss

    Apply all? [Y/n/i]
    ```
    - `Y` (default) — apply every operation.
    - `n` — abort, no changes made.
    - `i` — enter interactive mode: prompt once per operation with the operation type and the path, accepting `y` (apply) or `n` (skip). After all per-operation prompts, print a summary of what will be applied and re-confirm with a single `Apply? [Y/n]`.
18. `zero update` supports a `--yes` / `-y` flag that skips the top-level prompt and applies all operations.
19. `zero update` writes only into `.zero/`. It must not touch `styles/app.scss`, `tsconfig.json`, `index.html`, `AGENTS.md`, `.gitignore`, `src/`, or any path outside `.zero/`. The plan output and per-operation interactive prompts likewise list only paths under `.zero/`.
20. When the user declines a per-operation prompt in interactive mode, that path is left in its current state (existing files keep their content; planned-but-skipped adds are not created; planned-but-skipped removes leave the file intact). A subsequent `zero update` invocation sees those paths as still-divergent and offers them again.
21. After applying operations, `zero update` prints a one-line summary: `"zero update: applied N operations (A added, U updated, R removed)."` and exits with code 0. If the user aborted (top-level `n`), the message is `"zero update: no changes applied"`.

### Scaffold split (`src/scaffold.rs`)

22. The `include_str!` constant block is partitioned into two sets:
    - **One-shot user templates** — `index.html`, `tsconfig.json`, `src/app.ts`, `src/routes/home.ts`, `src/routes/home.test.ts`, `styles/app.scss`, `AGENTS.md`. Written only by `zero init`.
    - **Framework templates** — `.zero/zero.d.ts`, `.zero/zero-test.d.ts`, `.zero/styles/_tokens.scss`, `.zero/styles/_base.scss`, `.zero/styles/_layout.scss`, `.zero/styles/_utilities.scss`, `.zero/styles/zero.scss`. Written by both `init` (initially) and `update` (refresh).
23. `src/scaffold.rs` exposes two functions:
    - `write_initial_project(root_dir, ctx) -> Result<()>` — writes the full set (both user and framework templates) plus `.gitignore`. Used by `init`.
    - `write_framework_files(root_dir) -> Result<Vec<Operation>>` — writes only the framework template set, returning the set of operations performed (for `update`'s summary). The `Operation` enum captures `Add`, `Update`, `Remove` with the path.
    The single straight-line `write_to` function is replaced by these two; existing callers update.
24. The framework-template set is exposed as a queryable structure (e.g. `fn framework_manifest() -> &'static [(&'static str, &'static str)]` returning `(relative_path, content)` pairs). `update` reads this to compute the add/update/remove sets without reaching into private state.
25. The `.gitignore` written by `zero init` is a new embedded template (`scaffold/.gitignore`) that contains at minimum `.zero/`. Additional standard ignores (e.g. `dist/`, `node_modules/`, IDE folders) are at the plan phase's discretion; the only hard requirement is `.zero/`.

### Existing scaffold tests (`src/scaffold.rs::tests`)

26. The existing test `write_to_emits_all_files` is split / renamed:
    - `write_initial_project_emits_user_files` — asserts the seven user-owned paths exist with their expected markers (the existing assertions for `index.html`, `tsconfig.json`, `src/app.ts`, `src/routes/home.ts`, `src/routes/home.test.ts`, `styles/app.scss`, `AGENTS.md` move here).
    - `write_initial_project_emits_framework_files` — asserts the seven framework paths exist under `.zero/` with their expected markers (the existing token, base, layout, utilities, type-def assertions move here and update their path prefixes).
    - `write_initial_project_emits_gitignore_with_zero_dir` — new test. Asserts `<root>/.gitignore` exists and contains a `.zero/` line.
27. The existing test `write_to_index_html_links_to_scss` continues to assert the single `<link>` tag points at `/styles/app.scss`.
28. New test `app_scss_imports_framework_aggregate` — asserts `<root>/styles/app.scss` contains the literal substring `@use '../.zero/styles/zero'`.
29. New test `zero_scss_contains_aggregate_uses` — asserts `<root>/.zero/styles/zero.scss` contains all four `@use 'tokens'`, `@use 'base'`, `@use 'layout'`, `@use 'utilities'` lines.
30. New test `tsconfig_types_point_at_dot_zero` — asserts `<root>/tsconfig.json`'s `"types"` array references `./.zero/zero.d.ts` and `./.zero/zero-test.d.ts` (not the previous root-level paths).
31. The previously updated assertion `tokens_scss_declares_tokens_directly` is moved to its new path under `.zero/styles/_tokens.scss`.

### `zero update` tests

32. New unit-level tests in `src/cmd/update.rs::tests` (or a sibling test module — plan phase picks):
    - `update_with_no_drift_reports_up_to_date` — set up a project where `.zero/` matches the framework manifest byte-for-byte; assert `update` reports up-to-date and writes nothing.
    - `update_with_missing_file_proposes_add` — delete one file from `.zero/`; assert `update`'s computed plan contains an `Add` for that path.
    - `update_with_modified_file_proposes_update` — change one byte in a `.zero/` file; assert plan contains an `Update`.
    - `update_with_extra_file_proposes_remove` — add a stray file to `.zero/`; assert plan contains a `Remove`.
    - `update_refuses_when_no_zero_toml` — run in a directory without `zero.toml`; assert exit error message.
    - `update_refuses_when_no_dot_zero_dir` — run in a directory with `zero.toml` but no `.zero/`; assert exit error message.
    - `update_yes_flag_applies_all_operations` — with `--yes`, assert all add/update/remove operations execute without prompting.
33. The interactive prompt logic itself is plan-level (a function that takes a plan and a `Confirmer` trait that the tests can stub out). Tests inject a stub confirmer to drive the `Y/n/i` paths and per-operation answers without reading from stdin.

### Integration test

34. A new integration test under `tests/` (parallel to existing `tests/scss_*` integration tests, file name at plan's discretion — recommend `tests/update.rs`):
    - Scaffold a project with `zero init --yes`.
    - Modify `<root>/.zero/styles/_tokens.scss` (change one token value).
    - Delete `<root>/.zero/styles/_utilities.scss`.
    - Create a stray `<root>/.zero/styles/_extra.scss`.
    - Run `zero update --yes`.
    - Assert: `_tokens.scss` is restored byte-identical to the embedded template, `_utilities.scss` is recreated, `_extra.scss` is removed.
    - Assert: `styles/app.scss`, `index.html`, `src/app.ts`, `tsconfig.json`, `AGENTS.md` are unchanged (compare byte-for-byte against their post-init contents).

### CLI plumbing (`src/main.rs`, `src/cmd/mod.rs`)

35. A new module `src/cmd/update.rs` implements `pub async fn run(args: UpdateArgs) -> anyhow::Result<()>`. `UpdateArgs` carries the `--yes` flag.
36. `src/main.rs`'s `clap` definitions get an `Update` variant with the `--yes` flag. The `Init` variant also gains a `--yes` flag.
37. The help text in `zero-framework-spec.md` §1's CLI block is updated to include the `zero update` line, and `zero init`'s help now mentions the confirmation step.

### Documentation

38. **`zero-framework-spec.md` §1 (CLI Interface)**: add the `zero update` line to the commands block. Add a "Subcommand Details" sub-section for `zero update` describing flags (`--yes`), the plan/confirm/interactive flow, and the "no drift → up to date" no-op behavior. The `zero init` sub-section gets a sentence about the new pre-flight plan and the `--yes` flag.
39. **`zero-framework-spec.md` §7.1 (Design system)**: revise the "Distribution model" paragraph. The previous wording ("After `zero init`, the files are user-owned … no upgrade path") is replaced with the new model: tokens, base, layout, utilities, and the framework aggregate `zero.scss` live in `.zero/styles/`, are framework-owned, and refresh via `zero update`. The user's `styles/app.scss` imports the aggregate via the relative path; users override tokens by re-declaring custom properties in their own `app.scss`.
40. **`zero-framework-spec.md` §13 (Key Design Decisions Summary)**: update the "Design system" row's rationale text from "user-owned" to "framework-owned regenerable layer under .zero/". Add a new row:
    `| Framework-file boundary | Hidden .zero/ directory, regenerated by `zero update` | Prevents accidental edits to framework-shipped files; gives projects a versioned upgrade path |`.
41. **Scaffold `AGENTS.md` (`src/scaffold/AGENTS.md`)**: extend the existing `## Styles` section to document the new layout — `_tokens.scss`, `_base.scss`, `_layout.scss`, `_utilities.scss`, `zero.scss` live under `.zero/styles/`, are framework-owned, are refreshed via `zero update`, and **must not be edited**. The user's `styles/app.scss` is the place to add application styles and to override tokens by re-declaring custom properties. Add a new top-level section, `## The .zero/ directory`, describing:
    - What lives there and why.
    - That the directory is in `.gitignore` and treated as off-limits.
    - That `zero update` is the only command that writes there.
    - The list of files currently shipped under `.zero/` (with a one-line description each).
    The section sentinels test in `src/scaffold.rs::tests::write_to_agents_md_has_section_sentinels` gains the `## The .zero/ directory` sentinel.
42. **Issue cross-reference**: `issues/design-system/spec.md` is **not** rewritten — that spec captured the design at a point in time. Instead, a short note is added at its top stating that requirements 19–20 (the scaffold file paths) are superseded by `issues/update/spec.md`. The plan should produce the exact note text.

## Constraints

- **No third-party crates.** Confirmation prompting uses the existing `inquire` (already in `Cargo.toml` for `prompt_user()`) or `std::io` directly — no new dependencies. The plan phase picks based on what already runs in `src/prompts.rs`.
- **No file system access outside the project root.** Both commands operate strictly inside the resolved `<root>/` (from `config.project.root`). Symlinks pointing outside the root are followed only for reading the existing state; writes stay confined to the root.
- **No silent destructive operations.** Without `--yes`, every write or delete is preceded by a confirmation prompt the user can decline. `--yes` is the only way to skip prompts and is opt-in.
- **`zero update` never writes outside `.zero/`.** Strictly enforced — any code path that constructs a write target for `update` joins from `<root>/.zero/` and the function refuses paths that escape that prefix (canonical-path check).
- **The framework-template set is the single source of truth.** Both `init` (when writing the initial framework files) and `update` consult the same `framework_manifest()` function. They cannot drift.
- **Byte-exact comparison.** `update` uses byte-for-byte file comparison to decide "update needed." No normalization (no line-ending coercion, no whitespace trimming). This matches the embedded `include_str!` contents exactly.
- **Removal is by manifest absence, not by metadata.** `update` removes a file under `.zero/` only if the path is not in `framework_manifest()`. There is no marker file or hash database tracking what was previously emitted; the embedded manifest is the only authority.
- **No partial-write recovery semantics.** If `update` fails mid-apply (e.g. disk-full), `.zero/` is left in whatever state the OS produced. No transactional / rollback behavior in v1. The user can re-run `update` to converge.
- **Init's existing `--refuse-on-non-empty-root` behavior is preserved exactly.** The plan/confirm step does not soften it; init still bails before the prompt if the target is non-empty.
- **The `.gitignore` write is one-shot.** `init` writes `.gitignore` only when the target directory is empty (which is its existing precondition). `update` never writes or modifies `.gitignore`. If a user removes `.zero/` from their `.gitignore` post-init, that is their call.

## Out of Scope

- **Migrating existing projects to the `.zero/` layout.** Projects scaffolded by older `zero` versions have `_tokens.scss` etc. in `styles/`. This spec does not introduce a migration command. `update` explicitly refuses to run when `.zero/` is missing. Users on old layouts re-scaffold manually if they want the new boundary. The plan should not invent a migration; that is a separate issue.
- **Three-way merge for divergent files.** `update` does not attempt to merge user changes inside `.zero/` with framework updates. The directory is framework-owned; any local edit is by definition discardable. `update` shows the diff in the plan (or at least flags an "update" operation on that path) and the user accepts or declines wholesale.
- **Hash-database / lock-file tracking.** No `.zero/.lock` or `.zero/manifest.json` tracking what was emitted. The embedded template is the source of truth.
- **Versioning / compatibility checks.** `zero update` does not check whether the CLI version matches what the project was scaffolded with. It always rewrites `.zero/` from the currently running binary. Cross-version concerns are deferred.
- **Updating user-owned files.** `zero update` never touches `styles/app.scss`, `tsconfig.json`, `index.html`, `AGENTS.md`, `.gitignore`, or anything under `src/`. If a future framework change requires a `tsconfig.json` field, that is a separate spec.
- **Component library shipping under `.zero/`.** Components are a future issue. This spec lays the framework-files boundary; the future component-library spec decides whether components ride in `.zero/` or land somewhere else.
- **The `--cwd` flag for running update against a non-CWD directory.** Both `init` and `update` operate on the current working directory. Cross-directory operations are future work.
- **Watching `.zero/` for drift during `zero dev`.** If a user edits a `.zero/` file while `dev` is running, behavior is whatever SCSS recompilation produces. No "dev refuses to start if `.zero/` is dirty" check. Convention plus gitignore plus documentation is the v1 enforcement; technical enforcement is deferred.
- **Reverting an `update`.** No `zero update --revert`. Users who want to undo restore from VCS — but since `.zero/` is gitignored, they cannot. Acceptable in v1; the operation is regenerable from the binary, so "revert" means "re-run update against the prior CLI version", which is a versioning concern (out of scope above).
- **Adding `align-*`, `justify-*`, or other new utilities.** This spec ships only the redistribution mechanism. The next spec adds new utilities to the existing surface; once `.zero/` and `update` exist, that next spec gets the easy delivery path.

## Open Questions

- **`tsconfig.json` `"include"`.** Today the `include` array is `["src"]`. If TypeScript needs `.zero` in `include` for the `zero.d.ts` types to resolve, the plan should pick: add `.zero` to `include`, or rely solely on `"types"` resolution. The current zero.d.ts resolution mechanism (via `"types": ["zero/types"]`-style or `"types": ["./.zero/zero.d.ts"]`-style) needs verification against the existing `src/runtime.rs::ZERO_TYPES_BODY` content and how `zero check` reads the file.
- **Exact `.gitignore` contents.** Spec requires at minimum a `.zero/` line. The plan should propose the full default `.gitignore` content — likely also includes `dist/`, `target/` (if applicable to user projects), and editor folders, or stay minimal with just `.zero/`. Lean toward minimal: only what the framework requires.
- **`zero update` exit code on user-declined operations.** Spec says exit 0 for both "user accepted everything" and "user declined everything." Alternative: exit 1 (or a distinct code) when the user declined some operations, so CI can detect partial application. Recommend exit 0 in both cases; a CI script that wants strictness should use `--yes`.
- **Interactive-mode prompt format.** Spec describes `Y/n/i` for the top-level prompt and per-operation `y/n` for interactive. Whether the per-operation prompt should also offer `q` (quit immediately) or `a` (apply all remaining) is a UX call for the plan.
- **`zero init`'s prompt phase ordering vs. confirmation.** Spec orders it: prompts → plan → confirm → write. Alternative: confirm first ("do you want to create a project in `./<root>/`? [Y/n]") then prompts. The first ordering matches the requirement that the plan reflects the user's answers; recommended.
- **Whether `update` should show file-content diffs in interactive mode.** Spec currently shows only path + operation type. A future enhancement might show a short diff for `update` operations. v1 keeps it simple — operation type + path only. The plan should confirm this is sufficient.
- **Behavior when `.zero/` exists but is empty.** Treat as "all add operations" (the current logic produces this naturally). Plan should confirm tests cover this case.
- **Section-sentinel test updates.** The `write_to_agents_md_has_section_sentinels` test currently lists thirteen sentinels. Adding the `## The .zero/ directory` sentinel changes the list. Plan should produce the exact updated assertion.
- **Re-running `zero init` post-init.** Today, re-running `init` errors because the directory is non-empty. With the new `--yes` flag and confirmation gate, the behavior is unchanged — `init` still refuses non-empty. The plan should add a test asserting this is preserved.
