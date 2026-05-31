# Spec: `zero test` without a `zero.toml`

## Problem Statement

`zero test` currently assumes it is run from inside a scaffolded project — one
that has a `zero.toml` at the root. This couples the test runner to the full
project shape and blocks the cases where testing is most valuable on its own:

- A standalone library or a single module someone wants to verify before wiring
  it into a project.
- A scratch directory or a reduced-repro for a bug, where authoring a
  `zero.toml` just to run a test is pure friction.
- Adopters evaluating the runner in isolation ("does my code import and run
  under this harness?") before committing to the framework.

The goal is for `zero test` to **just work** in a directory of `.ts` / `.js`
test files, falling back to sensible defaults when no `zero.toml` is present,
while behaving exactly as it does today when one is.

## Background

Establish before implementing (do not assume — read the code):

- Where `zero test` loads config today. Likely `zero-config` resolving
  `zero.toml`, consumed by `zero-test-runner` (and/or the `zero` CLI command
  wiring). Find the exact call site and what it errors with when the file is
  absent.
- Which config values the test path actually reads vs. ignores. The runner
  needs test discovery roots, file extensions (`.ts`/`.js` per `CLAUDE.md`),
  and whatever transpile/runtime setup it shares with `zero dev`/`build`. Only
  the values on the test path matter here.
- How transpile and the DOM/web-platform shims are located today — are any of
  those paths config-derived, or are they all built into the binary
  (`zero-runtime/build.rs` concatenated blob)? Defaults must not depend on
  project files that won't exist.

## Proposed Behavior

- On `zero test`, resolve `zero.toml` if present (current behavior, unchanged).
- If absent, run with a built-in default config: discover `*.test.ts` /
  `*.test.js` (confirm the existing glob) under the current directory, use the
  same default transpile + runtime setup the scaffold would have produced.
- No new flags required for the common case; if an explicit override is useful,
  prefer reusing existing CLI flags over adding config-file-only knobs.

## Scope / Non-Goals

- In scope: `zero test` only. `zero dev` / `zero build` keep requiring a project
  unless trivially free to extend.
- Non-goal: inventing a new config format or a partial-`zero.toml` merge story
  beyond "present → use it, absent → defaults."

## Open Questions (resolve with user before plan)

- When some config keys are present but the file is partial, merge over
  defaults or treat as today? (Lean: merge over defaults.)
- Should `zero test` in a non-project dir print a one-line notice that it's
  using defaults, or stay silent? (Lean: silent unless `--verbose`.)

## Done When

- `zero test` and `zero test <file>` run green in a directory with test files
  and **no** `zero.toml`.
- Running inside a real scaffolded project is byte-for-byte unchanged.
- Covered by an integration test that runs the CLI in a config-less temp dir
  (mark `#[ignore = "slow"]` if it spawns the binary, per `CLAUDE.md`).
