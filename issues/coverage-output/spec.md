# Spec: Opt-in `coverage.json` output (`--coverage-output`)

## Problem Statement

`zero test --coverage` does two things at once: it prints a coverage table to
the terminal **and** writes `<root>/coverage/coverage.json` to the developer's
working tree as an unconditional side effect. Writing a file into someone's
project should be an explicit choice, not a side effect of asking to *see*
coverage. The current behavior:

- Pollutes the working tree with an artifact nobody asked for (which then has to
  be `.gitignore`d, or risks being committed).
- Conflates two genuinely separate concerns — viewing coverage interactively
  while iterating, vs. emitting a machine-readable artifact for CI / external
  tooling.
- Was never a deliberate design decision: it shipped this way in the
  `test-improvements` slice and was surfaced during review of `test-no-config`
  (the "no-config coverage" note in `issues/test-no-config/review.md`).

The goal: `--coverage` shows coverage and writes nothing; a separate, explicit
`--coverage-output <path>` is the only thing that writes a file.

## Background

Confirmed by reading the code — these are the call sites the plan must touch:

- **CLI flag.** `crates/zero/src/main.rs:42-48` defines the `Test` subcommand
  with a single boolean `coverage` flag, dispatched as
  `cmd::test::run(target, coverage).await` (`main.rs:117`).
- **Coverage branch.** `crates/zero/src/cmd/test.rs` — when `coverage` is true it
  builds a `CoverageAggregator`, runs each file with a `CoverageScope`, then
  after the run calls **both** `agg.write_terminal(&mut stdout, &root)` (the
  table) **and** `agg.write_json(&root)` (the file). These are the two behaviors
  to split.
- **The writer.** `crates/zero-test-runner/src/coverage.rs:801`
  `CoverageAggregator::write_json(&self, project_root: &Path)` does
  `create_dir_all(project_root.join("coverage"))` then writes `coverage.json`.
  The JSON value comes from `to_json_value(project_root)` (`coverage.rs:812`),
  whose relative-path keys are computed against `project_root` (the discovery
  root) — independent of where the output file lands.
- **`root` selection.** `cmd/test.rs` already computes `root` (= `cwd/<project
  root>` with a `zero.toml`, or `cwd` in no-config mode). The terminal table and
  JSON keying both use this `root`; that stays.
- **Docs.** `docs/config-and-cli.md:156` and `docs/testing.md:21` both describe
  `--coverage` as "write coverage data to `coverage/`" — these must be updated.
- **Sibling, not in scope.** `zero mutate` writes `mutation/mutation.json` via a
  separate command and code path; untouched here.

## Requirements

### Flag behavior

- `--coverage` (boolean, unchanged spelling) enables coverage instrumentation
  and prints the terminal table to stdout. **It writes no file.**
- New `--coverage-output <path>` flag that takes a value. When present:
  - It **implies coverage is on** — instrumentation runs and the terminal table
    prints — even if `--coverage` is not also passed.
  - It writes the coverage JSON to `<path>`.
  - `<path>` is resolved **relative to the current working directory** (the path
    as the developer types it, consistent with the cwd-first file-argument
    resolution shipped in `test-no-config`). An absolute path is honored as-is.
  - Any missing **parent directories** of `<path>` are created before writing
    (mirrors today's `create_dir_all`).
- Passing both `--coverage` and `--coverage-output <path>` is valid and
  non-conflicting: table prints, file written to `<path>`.
- Passing neither flag: no instrumentation, no table, no file (unchanged from
  today's non-coverage path).

### Output

- The terminal table is **byte-for-byte unchanged** from today (same
  `write_terminal`, sorted the same way, keyed off `root`).
- The JSON **content/shape is unchanged** from today (same `to_json_value`); only
  *where* and *whether* it is written changes. Its relative-path keys remain
  computed against the discovery `root`, regardless of the output file location.
- `zero test --coverage-output coverage/coverage.json`, run from the project
  root, reproduces exactly the file that `--coverage` writes today.

### Docs

- `docs/config-and-cli.md` `zero test` flag table: `--coverage` description
  changes to "Print a coverage table (no file written)"; add a
  `--coverage-output <path>` row ("Write coverage JSON to `<path>`; implies
  `--coverage`").
- `docs/testing.md` "Running tests" examples updated to show `--coverage`
  (table only) and `--coverage-output <path>` (file).

### Tests

- Rewrite the existing `coverage_true_writes_coverage_json` test in
  `cmd/test.rs`: with `--coverage` alone, assert the run is `Ok` and that **no**
  `coverage/coverage.json` is written. Add a companion test that passes a
  coverage-output path and asserts the JSON file exists at the cwd-relative path
  and parses. (This also subsumes the "no-config coverage" gap noted in
  `test-no-config`'s review — exercise it in a no-config tempdir.)
- A unit/integration test that `--coverage-output sub/dir/cov.json` creates the
  missing `sub/dir/` parents and writes the file there.
- A test that `--coverage-output <path>` without `--coverage` still enables
  coverage (file is written and is non-empty / well-formed).

## Constraints

- **Localized change.** Touch only: the clap definition in `main.rs`, the
  coverage branch + `run()` signature in `cmd/test.rs`, the writer in
  `coverage.rs`, and the two docs files. No new config format, no `zero.toml`
  `[test]` knob.
- **`run()` signature.** It currently takes `coverage: bool`. It must carry the
  optional output path too (recommendation: keep `coverage: bool` and add
  `coverage_output: Option<String>` / `Option<PathBuf>`; coverage is on when
  `coverage || coverage_output.is_some()`). Update all call sites and tests.
- **Writer API.** Prefer adding a path-taking method (e.g. `write_json_to(&self,
  file: &Path, root: &Path)`) or repurposing `write_json` to take an explicit
  file path, rather than leaving a second method that hardcodes
  `<root>/coverage/`. The plan picks; the hardcoded-location write must not
  remain reachable from the default `--coverage` path.
- **No behavior change to the terminal table or JSON schema.**
- Rust functions stay under ~80 lines (`CLAUDE.md`).
- **Deliberate breaking change.** This removes the auto-write that `--coverage`
  performed. There is no deprecation shim or alias; the project is pre-1.0 and
  the change is intentional. Call it out in the docs update so anyone relying on
  the old path in CI knows to add `--coverage-output coverage/coverage.json`.

## Out of Scope

- Changing the terminal table format or the coverage JSON schema.
- `zero mutate`'s `mutation/mutation.json` (separate command, separate writer).
- A `zero.toml` `[test]` section / config-file default for coverage output.
- Additional coverage formats (lcov, Cobertura, HTML, etc.).
- A deprecation/back-compat alias for the old auto-write behavior.
- `--watch` / `--update-snapshots` and any other deferred `zero test` flags.

## Open Questions

- **Existing-path handling.** If `<path>` names an existing **file**, overwrite
  it (recommended). If it names an existing **directory**, error clearly rather
  than writing `dir/coverage.json` implicitly. Plan confirms the exact rule and
  message.
- **`run()` parameter shape.** `coverage: bool` + `coverage_output:
  Option<PathBuf>` vs. a small struct/enum collapsing both. Recommendation: the
  two-field form for minimal churn; plan may prefer an enum if it reads cleaner.
