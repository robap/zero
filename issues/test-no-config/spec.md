# Spec: `zero test` without a `zero.toml` (and intuitive file paths)

## Problem Statement

Two friction points in `zero test`, addressed together because they share the
same resolution code:

1. **`zero test` requires a `zero.toml`.** It assumes it is run from inside a
   scaffolded project. This couples the test runner to the full project shape
   and blocks the cases where testing is most valuable on its own:
   - A standalone library or a single module someone wants to verify before
     wiring it into a project.
   - A scratch directory or a reduced-repro for a bug, where authoring a
     `zero.toml` just to run a test is pure friction.
   - Adopters evaluating the runner in isolation ("does my code import and run
     under this harness?") before committing to the framework.

2. **The file argument resolves against the wrong base.** `zero test <file>`
   resolves the path relative to the `[project] root` subdirectory (e.g.
   `web/`), *not* the developer's current directory. A dev sitting at the repo
   root who sees `web/src/app.test.ts` in their editor must instead type
   `zero test src/app.test.ts` — dropping the `web/` prefix. This is
   consistently confusing for both humans and coding agents, who naturally copy
   the path as it appears from where they are standing.

The goal: `zero test` should **just work** in a directory of `.ts` / `.js`
test files (falling back to sensible defaults when no `zero.toml` is present,
behaving exactly as today when one is), and the file argument should accept the
path as the developer actually sees it.

## Background

Confirmed by reading the code (do not re-derive — these are the call sites the
plan must touch):

- **Config load.** `crates/zero/src/cmd/test.rs:22` calls
  `Config::load_from_cwd()` (`crates/zero-config/src/config.rs:137`), which
  `bail!`s with a "zero.toml not found … run `zero init`" message when the file
  is absent. This is the gate to relax.
- **Roots.** `cmd/test.rs:24-25` computes `root = cwd.join(config.project.root)`
  (e.g. `cwd/web`) and `out = cwd.join(config.build.out)` (e.g. `cwd/dist`).
  These two paths are *all* the test path reads from config — discovery root,
  and the build-output dir to skip. Nothing else on the test path is
  config-derived.
- **File-arg resolution.** `crates/zero-test-runner/src/discovery.rs:30-38` —
  if `target` names an existing file it is resolved as `root.join(target)`
  (project-root-relative). Otherwise it falls through to a recursive walk of
  `root`, and `target` is applied as a **substring filter** after stripping the
  `root` prefix (`discovery.rs:67-74`).
- **No "partial" config state.** `[project] root` is a required field
  (`RawProject.root` is non-optional) and `[build] out` defaults to `dist`. So a
  `zero.toml` that parses always yields a complete `root` + `out`. There is no
  meaningful partial-merge case: the file is either present-and-complete or
  absent. The earlier open question about merging a partial file is dropped.
- **Transpile / shims are not config-derived.** The DOM / web-platform shims and
  transpile setup are built into the binary (e.g. `zero-runtime/build.rs`
  concatenated blob), so defaults do not depend on any project file existing.
  (Plan should confirm the harness in `zero-test-runner` needs only `root` to
  run a file, which `run_file_with_coverage(&root, f, …)` suggests.)

## Requirements

### No-config fallback

- When `zero.toml` is **present**, behavior is unchanged in every respect
  (discovery root, out-dir skip, coverage, exit codes, output).
- When `zero.toml` is **absent**, `zero test` runs against a built-in default
  instead of erroring:
  - **Discovery root = the current working directory.**
  - Skip dirs: the existing `node_modules` and hidden-dir rules (preserving the
    `.zero/components` exception), **plus** `dist/` and `build/` under the cwd,
    so a stray bundled copy of a test isn't discovered and run a second time.
  - Test discovery globs are unchanged (`*.test.ts` / `*.test.js` /
    `*.spec.ts` / `*.spec.js`, per `is_test_file` and the TS/JS collision check
    in `discovery.rs`).
- The fallback is **silent**: no notice is printed when defaults are used.
  Output for a passing/failing run looks the same as inside a project.
- `--coverage` continues to work in no-config mode, instrumenting `src/`
  relative to the default root (cwd). It need not be elaborate — just not crash
  and write `coverage/coverage.json` under the default root, mirroring the
  in-project path.

### File-argument resolution (applies in **both** config and no-config modes)

- `zero test <file>` resolves the file argument **cwd-first, with project-root
  as a fallback**:
  1. Try the path relative to the developer's cwd (`cwd.join(target)`).
  2. If that is not an existing file, try the current project-root-relative
     resolution (`root.join(target)`).
  3. If neither names a file, fall through to the existing substring-filter
     behavior (unchanged).
- Concretely, from the repo root with `[project] root = "web"`, all of these
  must run the same file:
  - `zero test web/src/app.test.ts` (cwd-relative — the new, intuitive form)
  - `zero test ./web/src/app.test.ts`
  - `zero test src/app.test.ts` (project-root-relative — the legacy form, still
    works)
- In no-config mode (root = cwd) the two resolution bases coincide, so the file
  arg is simply cwd-relative.

## Constraints

- Keep the change localized to the test path. The fix lives in `cmd/test.rs`
  (config-absent branch + root selection) and `zero-test-runner/discovery.rs`
  (file-arg resolution + the extra skip dirs). Do not introduce a new config
  format, a partial-`zero.toml` merge story, or new config-file-only knobs.
- No new CLI flags for the common case. (`--coverage` stays as-is. A `--verbose`
  notice is explicitly not required.)
- Rust functions stay under ~80 lines (`CLAUDE.md`); if the resolution logic
  grows, factor a helper in `discovery.rs`.
- The TS/JS collision guard and the substring-filter semantics must be preserved
  unchanged.

## Out of Scope

- `zero dev` and `zero build` keep requiring a `zero.toml` — they need a real
  project shape (index.html, out dir, etc.). Only extend them if it turns out to
  be trivially free, which it is not expected to be.
- `zero mutate` and other subcommands that share discovery — leave their config
  requirements as they are unless the shared `discover()` change touches them
  for free (in which case keep their existing behavior intact and note it).
- No new config format, no partial-merge logic (moot, see Background).
- No richer default-detection (monorepo root-finding, walking up for a
  `zero.toml`, etc.). Absent file ⇒ cwd is the root, full stop.

## Open Questions (resolve during plan)

- Confirm `zero-test-runner`'s harness (`run_file_with_coverage`) needs nothing
  from config beyond `root` to execute a file in no-config mode. If it reaches
  for anything else config-derived, surface it before coding.
- Decide where the no-config default `root`/`out` are constructed: a small
  defaults helper in `cmd/test.rs`, vs. a `Config::default_for_cwd()`-style
  constructor in `zero-config`. Lean: keep it in `cmd/test.rs` so `zero-config`
  stays strictly "parse a real file," but the planner may prefer the latter for
  reuse — pick one and justify.

## Done When

- `zero test` and `zero test <file>` run green in a directory with test files
  and **no** `zero.toml`, discovering from cwd and skipping `dist/`/`build/`.
- From a real scaffolded project, `zero test web/src/app.test.ts` (cwd-relative)
  and `zero test src/app.test.ts` (root-relative) both run that one file.
- Running inside a real scaffolded project is otherwise byte-for-byte unchanged
  (discovery, coverage, output, exit codes).
- Covered by tests:
  - A unit test in `discovery.rs` for cwd-first / root-fallback file-arg
    resolution and the new skip dirs.
  - An integration test that spawns the CLI in a config-less temp dir with test
    files and asserts a green run (mark `#[ignore = "slow"]` per `CLAUDE.md`).
