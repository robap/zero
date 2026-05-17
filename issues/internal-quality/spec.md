# Spec: Phase 10 — Internal Quality

## Problem Statement

The `zero` CLI has grown to ~12.4k lines of Rust in a single crate, with eleven
top-level functions over the spec's ~80-line ceiling, several heavy compile-time
dependencies (`swc_core`, `boa_engine`, `axum`/`hyper`/`tower`) that all rebuild
on every change, and no coverage tooling. Phase 10 addresses the structural
debt before the next round of feature work (`zero check`, `zero fmt`, `zero
lint`, `zero gen`, `zero preview`) lands on top of it.

The goal is structural — the user-visible behavior of every subcommand stays
identical. Success is faster incremental compiles, enforced module boundaries,
smaller functions, and a coverage safety net that prevents regressions.

## Background

### Current layout

One crate (`zero`), modules under `src/`:

```
src/
├── build/      bundler, css, index_html, manifest, resolver
├── cmd/        build, dev, init, mutate, test, update
├── dev/        server, files, transpile, proxy, watch, sass, sse, inject,
│               local, headers
├── test_runner/coverage, discovery, harness, loader, mutate, reporter, result
├── config.rs       toml parsing + validation
├── prompts.rs      dialoguer wrapper, only used by cmd::init
├── runtime.rs      embedded JS runtime strings (include_str!)
├── sass.rs         grass wrapper
├── scaffold.rs     embedded scaffold templates + manifest
├── toml_writer.rs  toml emission (69 lines)
├── transpile.rs    swc TypeScript→JS
├── lib.rs          pub mod re-exports for integration tests
└── main.rs         clap entry point
```

`lib.rs` re-exports every top-level module so integration tests in `tests/`
(32 files, using `zero::*`) can poke at internals.

### Oversized functions (≥80 lines)

Identified via top-level `fn` scan:

| File | Function | Lines |
|---|---|---|
| `src/test_runner/harness.rs` | `run_with_loader` | 236 |
| `src/cmd/mutate.rs` | `run_inner` | 215 |
| `src/build/bundler.rs` | `rewrite_module` | 207 |
| `src/dev/server.rs` | `serve` | 169 |
| `src/transpile.rs` | `transpile_typescript` | 150 |
| `src/dev/proxy.rs` | `proxy_request` | 130 |
| `src/test_runner/coverage.rs` | `instrument` | 121 |
| `src/build/bundler.rs` | `bundle` | 118 |
| `src/test_runner/coverage.rs` | `build_prologue` | 117 |
| `src/build/css.rs` | `process_css` | 117 |
| `src/test_runner/harness.rs` | `walk_describe` | 103 |

### Test landscape

- 26 source files contain unit-test modules (~238 `#[test]` cases).
- 32 integration tests in `tests/` exercise the CLI end-to-end.
- Source files **without** unit tests (excluding `mod.rs` shims, `main.rs`,
  `lib.rs`): `src/dev/transpile.rs`, `src/dev/proxy.rs`, `src/dev/server.rs`,
  `src/dev/local.rs`, `src/dev/sass.rs`, `src/dev/headers.rs`,
  `src/cmd/build.rs`, `src/cmd/test.rs`, `src/cmd/dev.rs`.
- No coverage tooling installed.

### Dependency weight

The two biggest compile-time costs are `swc_core` (transpile + bundler) and
`boa_engine` (test runner). They are independent of each other; one change to
`dev/server.rs` currently rebuilds both transitively. Splitting them into
sibling crates inside a workspace removes the cross-rebuild.

## Requirements

### R1 — Coverage baseline and uplift (first)

- `cargo-llvm-cov` is installed as a development tool (no Cargo.toml change
  required — installed via `cargo install cargo-llvm-cov` documented in
  `CLAUDE.md` or a `justfile`/Makefile target).
- A baseline coverage report is captured before any restructuring and
  committed to `issues/internal-quality/baseline-coverage.txt` (or similar)
  for later comparison.
- Unit tests are added to the genuinely-uncovered modules, prioritized:
  - `src/dev/server.rs` — route table, listener bind, shutdown plumbing
  - `src/dev/proxy.rs` — `proxy_request` happy path, header rewriting,
    upstream failure, abort propagation
  - `src/dev/local.rs` — local index resolution
  - `src/dev/transpile.rs` — request-time transpile + sourcemap inline
  - `src/dev/sass.rs`, `src/dev/headers.rs` — small, but should be covered
  - `src/cmd/build.rs`, `src/cmd/test.rs`, `src/cmd/dev.rs` — thin orchestrators;
    cover the argument-parsing and error-mapping seams (integration tests
    already cover the happy path)
- Per-module floor: **70 % line coverage** for every non-`mod.rs`/`main.rs`
  source file; **85 %** for modules deemed critical (the planner sets the
  final critical-module list; initial candidates: `transpile.rs`,
  `build/bundler.rs`, `test_runner/harness.rs`, `test_runner/coverage.rs`,
  `dev/proxy.rs`, `scaffold.rs`).
- All existing tests continue to pass.

### R2 — Workspace crate split (second)

The single crate is converted to a Cargo workspace with the following members:

**Tier 1 — leaf utility crates** (no `zero-*` deps):

- `zero-transpile` ← `src/transpile.rs`
- `zero-sass` ← `src/sass.rs`
- `zero-config` ← `src/config.rs` + `src/toml_writer.rs` (absorbed)
- `zero-runtime` ← `src/runtime.rs` (embedded JS strings)
- `zero-scaffold` ← `src/scaffold.rs` (embedded templates + manifest)

**Tier 2 — engine crates** (depend on tier 1):

- `zero-test-runner` ← `src/test_runner/` (pulls `boa_engine`); depends on
  `zero-transpile`, `zero-runtime`
- `zero-bundler` ← `src/build/`; depends on `zero-transpile`,
  `zero-runtime`, `zero-sass`, `zero-config`
- `zero-dev` ← `src/dev/`; depends on `zero-transpile`, `zero-runtime`,
  `zero-sass`, `zero-config` (pulls `axum`/`hyper`/`tower`)

**Tier 3 — binary:**

- `zero` ← `src/cmd/` + `src/main.rs` + `src/prompts.rs` (absorbed);
  depends on every Tier 1/2 crate

Constraints:

- `tomp_writer.rs` and `prompts.rs` are absorbed (too small to be standalone).
- The workspace uses `[workspace.dependencies]` so external crate versions
  are declared once and inherited by members.
- Each crate has its own `Cargo.toml` listing only the external deps it
  actually uses (e.g. `zero-test-runner` lists `boa_engine` but not `swc_core`).
- Integration tests in `tests/` continue to compile and pass against the
  binary crate. The binary crate exposes a thin `lib.rs` that re-exports the
  symbols the existing tests touch — the API surface stays the same; only
  the source-of-truth moves. (See **Open Questions** for the
  re-export-vs-migrate decision.)
- `pub(crate)` items that need to cross the new crate boundary are widened
  to `pub` with a `#[doc(hidden)]` annotation when they are not part of the
  intended public surface.
- All `cargo test` / `cargo build` / `cargo clippy` invocations work from the
  workspace root with no surprising flags.

### R3 — Function-size refactor (third)

- Every function ≥80 lines listed in **Background** is reduced below the
  bar, with named intermediate steps that read top-to-bottom.
- "Reduced below the bar" is judged structurally, not by mechanical line
  cutting. A 100-line function that is a flat sequence of well-named,
  obvious steps may stay if splitting it would harm readability — but this
  is the exception, not the rule, and must be called out in the refactor
  commit message.
- No behavioral changes. Test suite (unit + integration) passes unchanged.
- The refactor happens inside the new crate boundaries from R2; we do not
  refactor in place and then move.

### R4 — CI gates (last)

- Coverage report runs in CI; build fails if any module falls below its
  per-module floor.
- Optional: a lightweight `scripts/check-fn-size.sh` (or equivalent) flags
  any newly-introduced function ≥80 lines. Implemented as a CI step or a
  pre-commit hook — planner decides.
- Existing test suite continues to run; no test is deleted or skipped.

## Constraints

- **No behavioral change.** Every subcommand, flag, prompt, and exit code
  behaves identically before and after Phase 10. Integration tests are the
  proof.
- **Per `CLAUDE.md`:** Rust functions stay under ~80 lines (this is the
  driving rule).
- **Workspace stays one published binary.** End users still get a single
  `zero` binary; the crate split is internal-only.
- **No new runtime dependencies.** The split may not introduce new third-party
  crates; it only reshuffles existing ones across `Cargo.toml` files.
- **`cargo install --path .` (or the equivalent workspace incantation) still
  produces a working CLI.** Release ergonomics do not regress.

## Out of Scope

- Phase 6 TODOs: `zero check`, `zero fmt`, `zero lint`, `zero gen`, `zero
  preview`. Phase 10 is structural cleanup, not feature work.
- Watch mode for `zero test` (the one remaining Phase 11 placeholder item).
- Rust-side mutation testing (`cargo-mutants`). Reconsider later if
  Phase 10 reveals weak spots.
- HMR / module-state preservation for `zero dev` (Phase 6 placeholder).
- Publishing any of the new sub-crates to crates.io. The workspace is
  internal; sub-crates have `publish = false` until a separate decision.
- Renaming or restructuring the JS runtime (`runtime/`) or scaffold
  contents (`src/scaffold/`). The Rust embedding moves; the embedded
  contents do not.
- Migration of existing integration tests off the binary crate (decided
  under Open Questions if we go that route — otherwise skipped).

## Open Questions

1. **`lib.rs` strategy.** Two viable options after the split:
   - **Re-export shim.** The binary crate's `lib.rs` re-exports symbols
     from the sub-crates so existing `zero::scaffold::foo`,
     `zero::build::bundler::bundle`, etc. keep compiling. Minimal test
     churn; least clean.
   - **Migrate tests.** Update each integration test to depend on the
     sub-crate it actually uses (e.g. `zero_bundler::bundle`). More churn
     up front; results in a smaller, more honest public surface for the
     binary crate. The planner should choose one and apply it uniformly.
2. **Critical-module list for the 85 % coverage floor.** Initial candidates
   are listed in R1; the planner should review and lock the final list
   before coverage uplift begins.
3. **Test-running ergonomics.** With a workspace, `cargo test --workspace`
   becomes the canonical command. `CLAUDE.md` currently documents
   `node --test runtime/*.test.js` for the JS runtime tests; that command
   does not change. The planner should confirm whether any `CLAUDE.md`
   updates are needed for the Rust side.
4. **CI host.** No CI configuration was located in the repo. If CI exists
   elsewhere, the gate from R4 wires into it; if there is none, the gate
   is documented as a local `cargo` recipe and deferred until CI lands.
5. **Workspace `Cargo.lock`.** A single workspace lockfile at the root
   replaces the current one. The planner should confirm there are no
   dependency-version conflicts surfaced by the split (very unlikely
   since the source-of-truth versions are unchanged).
6. **Function-size CI script.** Reuse the `awk` heuristic from this spec's
   discovery pass, or invest in a real `syn`-based walker? The former is
   ~10 lines and good-enough; the latter is more reliable but is itself
   code to maintain.
