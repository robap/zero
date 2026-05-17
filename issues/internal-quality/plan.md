# Plan: Phase 10 — Internal Quality

## Summary

Phase 10 is structural cleanup with no behavioral change. Work proceeds in four
phases that mirror the spec's requirement ordering: (R1) install
`cargo-llvm-cov`, capture a baseline, and add unit tests to genuinely-uncovered
modules so the safety net exists before any restructuring; (R2) convert the
single `zero` crate into a Cargo workspace of eight members (five Tier-1 leaf
crates, three Tier-2 engine crates, and one Tier-3 binary), with
`[workspace.dependencies]` driving version pinning so `boa_engine`,
`swc_core`, and `axum`/`hyper`/`tower` stop transitively rebuilding each
other; (R3) shrink every ≥80-line function listed in the spec, working inside
the new crate boundaries; (R4) wire coverage and function-size checks as
local recipes documented in `CLAUDE.md` (no CI host was located in the repo —
the checks become CI gates the moment CI exists).

## Prerequisites

The spec lists six **Open Questions**. The plan resolves them as follows so
execution can proceed without further input:

1. **`lib.rs` strategy → migrate tests, not re-export shim.** A repo-wide
   grep finds only three integration tests touching `zero::*` symbols:
   `tests/config_load.rs` (twice on `zero::config::Config::load_from_cwd`)
   and `tests/dev_serves_runtime.rs` + `tests/runtime_evaluates.rs` (on
   `zero::runtime::runtime_module`). Migrating these three call sites to
   `zero_config::Config` / `zero_runtime::runtime_module` is smaller than
   maintaining a shim and yields a more honest binary-crate surface
   (Step 16).
2. **Critical-module list for the 85 % coverage floor — locked to the
   spec's initial candidates plus none added:** `transpile`, `build/bundler`,
   `test_runner/harness`, `test_runner/coverage`, `dev/proxy`, `scaffold`.
   `test_runner/loader` and `test_runner/mutate` are not added — they are
   heavy but already extensively exercised through `harness` and the
   `cmd::mutate` integration tests, so the 70 % floor is sufficient.
3. **`CLAUDE.md` updates — yes, three additions** under "Commands": the new
   `cargo test --workspace`, `cargo install --path crates/zero` (replacing
   `cargo install --path .`), and a coverage recipe
   (`cargo llvm-cov --workspace --html`). The `node --test runtime/*.test.js`
   line stays unchanged — see Open Question 5 below.
4. **CI host — none located.** Coverage and function-size gates are
   implemented as `scripts/` shell entries with a `make`-less invocation
   documented in `CLAUDE.md`. A `TODO(ci)` comment in each script flags the
   wiring work for when CI lands. The spec explicitly permits this
   deferral.
5. **Workspace `Cargo.lock` — single root lockfile, no version churn
   expected.** Reason: every external crate currently in the root
   `Cargo.toml` is hoisted into `[workspace.dependencies]` with its
   existing version requirement, then inherited by members via
   `dep.workspace = true`. No version is loosened, tightened, or
   re-resolved. Also: `runtime/` stays at the workspace root (not moved
   into `crates/zero-runtime/`) so `node --test runtime/*.test.js` keeps
   working unchanged — `zero-runtime`'s `build.rs` references the
   directory via `../../runtime/` and emits
   `cargo:rerun-if-changed=../../runtime/<file>` for each input. Cargo
   accepts parent-relative paths in `rerun-if-changed`; this is a
   widely-used pattern for workspace-shared assets.
6. **Function-size CI script → simple `awk`-based heuristic.** The
   ~10-line shell script reuses the spec's own discovery heuristic. It is
   not perfectly accurate (it counts `}`-balanced blocks following a
   `^[\t ]*(pub )?(async )?fn ` line) but is good-enough as a guardrail.
   Investing in a `syn`-based walker is out of scope.

Two additional **decoupling decisions** the executor must apply without
re-deriving them:

- `src/toml_writer.rs` currently takes `&Answers` (defined in
  `src/prompts.rs`). The spec puts `toml_writer` in `zero-config` and
  `prompts` in the binary crate, which would force `zero-config` to
  depend on `prompts`. Resolution: `zero-config::render_toml` is changed
  to take a public `TomlInput { root: String, port: u16, proxy:
  Option<String>, out: String }` struct (Step 10). `cmd::init`
  constructs a `TomlInput` from `Answers` before calling it.
- `src/scaffold/` (the template tree consumed by `include_str!`) moves
  with `scaffold.rs` to `crates/zero-scaffold/src/scaffold/`. The
  `include_str!` argument strings stay byte-identical (they are relative
  to the `.rs` file, not to the manifest). No other path moves.

## Steps

- [x] **Step 1: Install `cargo-llvm-cov`, capture baseline coverage, document in `CLAUDE.md`**
- [x] **Step 2: Add unit tests to uncovered `src/dev/` modules**
- [x] **Step 3: Add unit tests to uncovered `src/cmd/` orchestrators**
- [x] **Step 4: Verify per-module floors and recapture coverage**
- [ ] **Step 5: Convert root `Cargo.toml` to a Cargo workspace skeleton**
- [ ] **Step 6: Extract `zero-runtime` (Tier 1, owns build.rs)**
- [ ] **Step 7: Extract `zero-transpile` (Tier 1)**
- [ ] **Step 8: Extract `zero-sass` (Tier 1)**
- [ ] **Step 9: Extract `zero-config` (Tier 1, absorbs `toml_writer`)**
- [ ] **Step 10: Extract `zero-scaffold` (Tier 1)**
- [ ] **Step 11: Extract `zero-test-runner` (Tier 2)**
- [ ] **Step 12: Extract `zero-bundler` (Tier 2)**
- [ ] **Step 13: Extract `zero-dev` (Tier 2)**
- [ ] **Step 14: Move the binary crate to `crates/zero/` and migrate integration tests**
- [ ] **Step 15: Verify workspace builds, all tests pass, capture rebuild-timing observation**
- [ ] **Step 16: Refactor `zero-test-runner` oversized functions**
- [ ] **Step 17: Refactor `zero-bundler` oversized functions**
- [ ] **Step 18: Refactor `zero-dev` oversized functions**
- [ ] **Step 19: Refactor `zero-transpile::transpile_typescript`**
- [ ] **Step 20: Refactor `zero` (binary) `cmd::mutate::run_inner`**
- [ ] **Step 21: Add `scripts/check-coverage.sh` enforcing per-module floors**
- [ ] **Step 22: Add `scripts/check-fn-size.sh` flagging ≥80-line functions**
- [ ] **Step 23: Update `CLAUDE.md` workspace commands and recapture final coverage**

---

## Step Details

### Step 1: Install `cargo-llvm-cov`, capture baseline coverage, document in `CLAUDE.md`

**Goal:** Establish a coverage safety net before any restructuring, so later
steps have a measurable regression target.

**Files:**
- `CLAUDE.md` — add a "Coverage" subsection under "Commands"
- `issues/internal-quality/baseline-coverage.txt` — new file

**Changes:**
- Run `cargo install cargo-llvm-cov --locked` (one-time install; not a
  Cargo.toml dep). The executor verifies installation with
  `cargo llvm-cov --version`.
- Add to `CLAUDE.md` under "Commands":
  ```
  # Generate HTML coverage report (Rust)
  cargo llvm-cov --html

  # Per-module summary table (Rust)
  cargo llvm-cov --summary-only
  ```
  Note: `--workspace` is not added yet; this step still runs against the
  single-crate layout.
- Capture baseline: `cargo llvm-cov --summary-only > issues/internal-quality/baseline-coverage.txt`.
  The file is committed verbatim. No interpretation, no editing.

**Tests:** None added. This step is observability scaffolding; it must not
modify any source file under `src/`. Existing `cargo test` continues to
pass.

### Step 2: Add unit tests to uncovered `src/dev/` modules

**Goal:** Bring every `src/dev/*.rs` source file to ≥70 % line coverage
(≥85 % for `dev/proxy.rs`, which is a critical module per Prerequisite 2).
This is the largest uncovered surface in the spec.

**Files:**
- `src/dev/server.rs` — add `#[cfg(test)] mod tests`
- `src/dev/proxy.rs` — add `#[cfg(test)] mod tests`
- `src/dev/local.rs` — add `#[cfg(test)] mod tests`
- `src/dev/transpile.rs` — add `#[cfg(test)] mod tests`
- `src/dev/sass.rs` — add `#[cfg(test)] mod tests`
- `src/dev/headers.rs` — add `#[cfg(test)] mod tests`

**Changes:** For each file, add unit tests covering the seams the spec
calls out:

- **`server.rs`** — table-driven test that constructs the `axum::Router`
  via the public `build_router` (or equivalent; widen `pub(crate)` to
  `pub(super)` if needed) and asserts each documented path
  (`/zero-runtime.js`, `/zero-http.js`, `/zero-test.js`, SSE endpoint,
  static fallback) resolves to a handler. Test for listener bind error
  on a port already in use. Test for graceful shutdown signal handling
  via a `tokio::sync::oneshot`.
- **`proxy.rs`** — happy-path test using a stub `reqwest::Client`
  pointed at a `axum::test_helpers::TestServer` (or a stub `hyper`
  service): asserts request headers are rewritten per
  `forward_headers`, response body is streamed through, upstream
  `502/504` is propagated as an `axum::http::Response`, and client
  abort cancels the upstream call. Use `tokio::time::timeout` to bound
  the abort test.
- **`local.rs`** — covers the small `serve_local_index` function: HTML
  with no `<head>` returns 404-like response; HTML with `<head>`
  receives the SSE script inject; non-HTML extensions pass through.
- **`transpile.rs`** — request-time transpile of a `.ts` body returns
  JS with inline sourcemap when `sourcemap=true`, no sourcemap when
  `false`; syntax error returns `TranspileError`.
- **`sass.rs`** — `compile_for_request` happy path returns CSS; missing
  partial returns error mapped to 5xx-shaped error type.
- **`headers.rs`** — `no_cache_layer` returns a `tower_http::SetResponseHeaderLayer`
  whose applied response has `Cache-Control: no-store` and
  `Pragma: no-cache`.

**Tests:** Each new `#[cfg(test)] mod tests` block is the test
content for this step. The integration tests under `tests/dev_*.rs`
continue to pass unchanged.

### Step 3: Add unit tests to uncovered `src/cmd/` orchestrators

**Goal:** Cover the argument-parsing and error-mapping seams in the thin
orchestrator commands. Integration tests already cover the happy path
end-to-end; this step covers the branches integration tests cannot easily
reach.

**Files:**
- `src/cmd/build.rs` — add `#[cfg(test)] mod tests`
- `src/cmd/test.rs` — add `#[cfg(test)] mod tests`
- `src/cmd/dev.rs` — add `#[cfg(test)] mod tests`

**Changes:**
- **`cmd/build.rs`** — tests for `run` with `override_flag=Some(true)`,
  `Some(false)`, `None`; uses `tempfile::tempdir()` plus a minimal
  `zero.toml` and `src/app.ts` fixture. Asserts the resulting
  `dist/` (or configured `out`) contains `index.html` and a bundled
  `.js`. Asserts the manifest is written. Failure cases: missing
  `zero.toml` returns `Config::load_from_cwd` error; bundler error
  bubbles via `?`.
- **`cmd/test.rs`** — tests for `run` with `target=None` (discovers all
  tests) and `target=Some("nonexistent")` (zero discoveries, exits 0
  cleanly); `coverage=true` produces `coverage/coverage.json`.
- **`cmd/dev.rs`** — extremely thin; test asserts `run` resolves
  `Config::load_from_cwd` and panics gracefully (returns `Err`) when
  the configured port is unavailable. Use a dropped listener trick to
  pre-bind the port.

**Tests:** As above. Existing `tests/e2e_init_*.rs` continue passing.

### Step 4: Verify per-module floors and recapture coverage

**Goal:** Prove the 70 %/85 % floors are met before any restructuring,
so R2 has a "must not drop below" target.

**Files:**
- `issues/internal-quality/baseline-coverage.txt` — overwritten with the
  post-uplift summary
- `issues/internal-quality/critical-modules.txt` — new file listing the
  six 85 % modules from Prerequisite 2, one per line, so later steps and
  the CI gate (Step 21) reference a single source of truth.

**Changes:**
- Run `cargo llvm-cov --summary-only`. For each non-`mod.rs`/`main.rs`
  source file: assert ≥70 %. For each line in
  `critical-modules.txt`: assert ≥85 %.
- If any module is under floor, add one more round of tests in the same
  step before proceeding. Do not move to Step 5 until every floor is met.
- Overwrite `baseline-coverage.txt` with the new summary.

**Tests:** `cargo test` continues passing. No new tests in this step
beyond any added to meet floors.

### Step 5: Convert root `Cargo.toml` to a Cargo workspace skeleton

**Goal:** Set up the workspace structure with no source movement yet, so
the binary still builds against the old layout. Subsequent steps move one
module at a time.

**Files:**
- `Cargo.toml` (root) — rewritten as a workspace manifest
- `crates/` — new directory (empty initially)

**Changes:**
- Replace the existing `[package]` and `[dependencies]` tables with:
  ```toml
  [workspace]
  members = ["crates/zero"]
  resolver = "2"

  [workspace.package]
  edition = "2024"
  version = "0.1.0"

  [workspace.dependencies]
  clap = { version = "4", features = ["derive"] }
  tokio = { version = "1", features = ["macros", "rt-multi-thread", "signal", "fs", "io-util", "net"] }
  axum = "0.7"
  hyper = { version = "1", features = ["full"] }
  reqwest = { version = "0.12", default-features = false, features = ["stream"] }
  tower = "0.5"
  tower-http = { version = "0.6", features = ["set-header"] }
  toml = "0.8"
  serde = { version = "1", features = ["derive"] }
  serde_json = "1"
  dialoguer = "0.11"
  sha2 = "0.10"
  anyhow = "1"
  url = "2"
  boa_engine = { version = "0.21", features = ["annex-b"] }
  tokio-stream = { version = "0.1", features = ["sync"] }
  futures-util = "0.3"
  notify-debouncer-mini = "0.4"
  regex = "1"
  swc_core = { version = "65", features = [
      "ecma_parser", "ecma_parser_typescript", "ecma_codegen",
      "ecma_transforms", "ecma_transforms_typescript", "ecma_visit",
      "ecma_ast", "common", "common_sourcemap",
  ] }
  base64 = "0.22"
  sourcemap = "9"
  grass = { version = "0.13", default-features = false, features = ["random"] }
  tempfile = "3"
  assert_cmd = "2"
  predicates = "3"
  bytes = "1"
  http-body-util = "0.1"
  ```
- Move the existing root `Cargo.toml` `[package]` and `[dependencies]`
  content to `crates/zero/Cargo.toml`, but rewrite each external dep as
  `name = { workspace = true }` (or `name.workspace = true`). The
  `[package]` table uses `edition.workspace = true` and
  `version.workspace = true`.
- Move `src/`, `tests/`, `build.rs` to `crates/zero/` so the binary
  builds in the new location. `runtime/` and `examples/`, `showcase/`,
  `issues/`, etc. stay at the workspace root.
- Adjust `build.rs` inside `crates/zero/`: change the
  `manifest_dir.join("runtime")` line to walk up to the workspace root,
  e.g. `manifest_dir.parent().unwrap().parent().unwrap().join("runtime")`.
- Verify `cargo build` from workspace root produces a working `zero`
  binary at `target/debug/zero`. Verify `cargo test --workspace` runs
  the existing test suite to completion.

**Tests:** Existing unit + integration tests continue to pass against
the relocated binary.

### Step 6: Extract `zero-runtime` (Tier 1, owns build.rs)

**Goal:** Move the JS embedding (build.rs + runtime.rs) into a leaf
crate. This is done first so the rest of the workspace can depend on a
stable `zero_runtime` API.

**Files:**
- `crates/zero-runtime/Cargo.toml` — new
- `crates/zero-runtime/src/lib.rs` — new (moved from `crates/zero/src/runtime.rs`)
- `crates/zero-runtime/build.rs` — new (moved from `crates/zero/build.rs`)
- `crates/zero/Cargo.toml` — add `zero-runtime = { path = "../zero-runtime" }`
  to `[dependencies]`; drop the `regex` build-dep (it moves to the new crate)
- `crates/zero/build.rs` — deleted (logic moved)
- `crates/zero/src/runtime.rs` — deleted
- `crates/zero/src/lib.rs` — replace `pub mod runtime;` with
  `pub use zero_runtime as runtime;`
- `Cargo.toml` (workspace) — add `"crates/zero-runtime"` to `members`

**Changes:**
- `crates/zero-runtime/Cargo.toml`:
  ```toml
  [package]
  name = "zero-runtime"
  version.workspace = true
  edition.workspace = true
  publish = false

  [build-dependencies]
  regex = { workspace = true }
  ```
- `crates/zero-runtime/build.rs`: byte-identical to the old root
  `build.rs` except `runtime_dir` becomes
  `manifest_dir.parent().unwrap().parent().unwrap().join("runtime")` so
  it walks up from `crates/zero-runtime/` to the workspace root. Each
  `cargo:rerun-if-changed=<...>` line becomes the absolute path the
  rewritten `runtime_dir` produces. Verify the build script runs at
  least once by touching a runtime file and observing rebuild.
- `crates/zero-runtime/src/lib.rs`: identical to the old `runtime.rs`.
  No re-export shim added inside this crate.
- The intermediate `pub use zero_runtime as runtime;` in the binary
  crate's `lib.rs` keeps the existing `zero::runtime::*` paths in
  `cmd/*` and `dev/*` source compiling unchanged. (Later steps will
  rewrite these `use crate::runtime::*` lines to `use zero_runtime::*`
  as each consuming module migrates to its own crate.)

**Tests:** Run `cargo test --workspace`. All existing JS-runtime
integration tests (`runtime_evaluates.rs`, `dev_serves_runtime.rs`)
must continue passing.

### Step 7: Extract `zero-transpile` (Tier 1)

**Goal:** Move the swc-driven TypeScript transpiler into its own crate.
This is a critical structural win: `swc_core` will stop rebuilding when
unrelated code changes.

**Files:**
- `crates/zero-transpile/Cargo.toml` — new
- `crates/zero-transpile/src/lib.rs` — new (moved from `crates/zero/src/transpile.rs`)
- `crates/zero/Cargo.toml` — add `zero-transpile`, drop `swc_core`,
  `base64`, `sourcemap` from binary's `[dependencies]`
- `crates/zero/src/transpile.rs` — deleted
- `crates/zero/src/lib.rs` — `pub use zero_transpile as transpile;`
- `Cargo.toml` (workspace) — `members` gets `"crates/zero-transpile"`

**Changes:**
- `zero-transpile` depends on: `swc_core` (with the full feature list),
  `base64`, `sourcemap`, `anyhow`, `serde_json` (whatever the file
  currently uses). Verified by `cargo build -p zero-transpile`.
- Source file moves verbatim. Any `use crate::*` lines inside
  `transpile.rs` are reviewed; the file currently has none crossing its
  own module.

**Tests:** Existing unit tests in `transpile.rs` move with the file and
run as `cargo test -p zero-transpile`. `cargo test --workspace` passes.

### Step 8: Extract `zero-sass` (Tier 1)

**Goal:** Move the grass wrapper. Symmetric to Step 7.

**Files:**
- `crates/zero-sass/Cargo.toml` — new
- `crates/zero-sass/src/lib.rs` — new (moved from `crates/zero/src/sass.rs`)
- `crates/zero/Cargo.toml` — add `zero-sass`, drop `grass`
- `crates/zero/src/sass.rs` — deleted
- `crates/zero/src/lib.rs` — `pub use zero_sass as sass;`
- `Cargo.toml` (workspace) — `members` gets `"crates/zero-sass"`

**Changes:** Source file moves verbatim. Crate depends on `grass`,
`anyhow`.

**Tests:** Existing unit tests in `sass.rs` run as
`cargo test -p zero-sass`.

### Step 9: Extract `zero-config` (Tier 1, absorbs `toml_writer`)

**Goal:** Pull configuration parsing and TOML emission into one leaf
crate. Requires the `Answers` decoupling from Prerequisites.

**Files:**
- `crates/zero-config/Cargo.toml` — new
- `crates/zero-config/src/lib.rs` — new; combines `config.rs` and
  `toml_writer.rs` into `mod config;` and `mod toml_writer;` (or flat
  re-exports of the public surface)
- `crates/zero/Cargo.toml` — add `zero-config`, drop `toml`, `serde`,
  `url` from binary `[dependencies]`
- `crates/zero/src/config.rs` — deleted
- `crates/zero/src/toml_writer.rs` — deleted
- `crates/zero/src/lib.rs` — `pub use zero_config as config;` and
  `pub use zero_config::toml_writer;` (or flat path) so existing
  `crate::config::*` / `crate::toml_writer::*` callers compile
- `crates/zero/src/cmd/init.rs` — convert `Answers` → `TomlInput`
  before calling `render_toml`; add `let toml_input = TomlInput { … }`
  near the call site

**Changes:**
- `crates/zero-config/src/lib.rs` exposes everything `Config` exposed
  plus:
  ```rust
  pub struct TomlInput {
      pub root: String,
      pub port: u16,
      pub proxy: Option<String>,
      pub out: String,
  }

  pub fn render_toml(input: &TomlInput) -> String { /* ... */ }
  ```
- Move the body of `render_toml` byte-for-byte; rename the parameter
  type from `&Answers` to `&TomlInput`. Field accesses are identical.
- `cmd/init.rs` builds `TomlInput { root: answers.root.clone(), … }`
  and passes it.
- Move the `#[cfg(test)] mod tests` block in `toml_writer.rs`: rewrite
  the `Answers { … }` fixture as a `TomlInput { … }` fixture. The
  round-trip assertion (`render_toml` → `Config::from_toml_str` →
  field equality) is preserved.

**Tests:** All existing config tests pass. The migrated toml_writer
test passes against the new `TomlInput` type. `cargo test --workspace`
green.

### Step 10: Extract `zero-scaffold` (Tier 1)

**Goal:** Move the scaffold templates and their materialization logic.

**Files:**
- `crates/zero-scaffold/Cargo.toml` — new
- `crates/zero-scaffold/src/lib.rs` — new (moved from `crates/zero/src/scaffold.rs`)
- `crates/zero-scaffold/src/scaffold/` — new (moved from `crates/zero/src/scaffold/`,
  contents byte-identical)
- `crates/zero/Cargo.toml` — add `zero-scaffold`
- `crates/zero/src/scaffold.rs` — deleted
- `crates/zero/src/scaffold/` — deleted (now in zero-scaffold)
- `crates/zero/src/lib.rs` — `pub use zero_scaffold as scaffold;`

**Changes:**
- `include_str!("scaffold/index.html")` literals stay byte-identical;
  they resolve relative to the new `lib.rs` location.
- Widen the one `pub(crate) fn write_user_files` at
  `scaffold.rs:178` to `pub` with `#[doc(hidden)]` so it can be called
  from the binary crate's `cmd/init.rs`.

**Tests:** Existing scaffold unit tests move with the file. The
`tests/e2e_init_*.rs` integration tests in the binary crate continue
passing because the public surface they invoke (via the CLI) is
unchanged.

### Step 11: Extract `zero-test-runner` (Tier 2)

**Goal:** Pull the Boa-based test runner into a sibling crate. Removes
`boa_engine` from the binary crate's direct dependency closure.

**Files:**
- `crates/zero-test-runner/Cargo.toml` — new; deps `zero-transpile`,
  `zero-runtime`, plus `boa_engine`, `anyhow`, `serde`, `serde_json`,
  `tokio` (whatever the modules use)
- `crates/zero-test-runner/src/lib.rs` — new
- `crates/zero-test-runner/src/{coverage,discovery,harness,loader,mutate,reporter,result}.rs`
  — moved from `crates/zero/src/test_runner/*.rs`
- `crates/zero/Cargo.toml` — add `zero-test-runner`, drop `boa_engine`
- `crates/zero/src/test_runner/` — deleted
- `crates/zero/src/lib.rs` — `pub use zero_test_runner as test_runner;`
- `crates/zero/src/cmd/test.rs`, `crates/zero/src/cmd/mutate.rs` —
  update `use crate::test_runner::*` to `use zero_test_runner::*`

**Changes:**
- Internal `use crate::transpile::*` lines in `coverage.rs`, `harness.rs`,
  `loader.rs`, `mutate.rs` → `use zero_transpile::*`.
- Internal `use crate::runtime::*` lines in `harness.rs`, `loader.rs` →
  `use zero_runtime::*`.
- Internal `use crate::test_runner::*` within the test runner files
  becomes `use crate::*` (now intra-crate).
- `lib.rs` of `zero-test-runner` mirrors the old `test_runner/mod.rs`:
  `pub mod coverage; pub mod discovery; …; pub use harness::run_file;
  pub use result::{FileResult, Status, TestOutcome};`

**Tests:** All unit tests in `test_runner/*.rs` move with the files
and run as `cargo test -p zero-test-runner`. Integration tests
(`test_runner_smoke.rs`) continue passing.

### Step 12: Extract `zero-bundler` (Tier 2)

**Goal:** Pull the build pipeline (bundler, css, index_html, manifest,
resolver) into a sibling crate.

**Files:**
- `crates/zero-bundler/Cargo.toml` — new; deps `zero-transpile`,
  `zero-runtime`, `zero-sass`, `zero-config`, plus `anyhow`, `serde`,
  `serde_json`, `sha2`, `regex`, `url`
- `crates/zero-bundler/src/lib.rs` — new
- `crates/zero-bundler/src/{bundler,css,index_html,manifest,resolver}.rs`
  — moved from `crates/zero/src/build/*.rs`
- `crates/zero/Cargo.toml` — add `zero-bundler`, drop `sha2` if no
  longer used directly
- `crates/zero/src/build/` — deleted
- `crates/zero/src/lib.rs` — `pub use zero_bundler as build;`
- `crates/zero/src/cmd/build.rs` — update
  `use crate::build::*` → `use zero_bundler::*`

**Changes:**
- Internal `use crate::runtime::*` in `bundler.rs` → `use zero_runtime::*`.
- Internal `use crate::transpile::*` in `bundler.rs` → `use zero_transpile::*`.
- Internal `use crate::config::Config` in `bundler.rs` → `use zero_config::Config`.
- `lib.rs` of `zero-bundler` mirrors the old `build/mod.rs`.

**Tests:** Build unit tests run as `cargo test -p zero-bundler`.
Integration tests `build_full.rs`, `build_smoke.rs`,
`build_sourcemap.rs` continue passing.

### Step 13: Extract `zero-dev` (Tier 2)

**Goal:** Pull the dev server into a sibling crate. Removes the heavy
`axum`/`hyper`/`tower` chain from the binary crate's direct closure.

**Files:**
- `crates/zero-dev/Cargo.toml` — new; deps `zero-transpile`,
  `zero-runtime`, `zero-sass`, `zero-config`, plus `axum`, `hyper`,
  `reqwest`, `tower`, `tower-http`, `tokio`, `tokio-stream`,
  `futures-util`, `notify-debouncer-mini`, `anyhow`, `url`, `regex`
- `crates/zero-dev/src/lib.rs` — new
- `crates/zero-dev/src/{files,headers,inject,local,proxy,sass,server,sse,transpile,watch}.rs`
  — moved from `crates/zero/src/dev/*.rs`
- `crates/zero/Cargo.toml` — add `zero-dev`, drop `axum`, `hyper`,
  `tower`, `tower-http`, `reqwest`, `notify-debouncer-mini`, `tokio-stream`,
  `futures-util` from `[dependencies]` (`tokio` stays — `main.rs` uses it)
- `crates/zero/src/dev/` — deleted
- `crates/zero/src/lib.rs` — `pub use zero_dev as dev;`
- `crates/zero/src/cmd/dev.rs` — update
  `use crate::dev::server::serve` → `use zero_dev::server::serve`

**Changes:**
- Internal `use crate::runtime::*` in `server.rs` → `use zero_runtime::*`.
- Internal `use crate::config::Config` in `server.rs` → `use zero_config::Config`.
- Internal `use crate::sass::*` in `dev/sass.rs` → `use zero_sass::*`.
- Internal `use crate::transpile::*` in `dev/transpile.rs` →
  `use zero_transpile::*`.
- The renamed `dev::sass` module (which wraps `zero_sass`) should be
  considered for rename to `dev::scss` if the naming clash bites. For
  now, keep names — `use zero_sass as sass_crate; use sass_crate::*;`
  inside the file disambiguates.

**Tests:** Dev unit tests run as `cargo test -p zero-dev`.
Integration tests `dev_*.rs` continue passing.

### Step 14: Move the binary crate to `crates/zero/` and migrate integration tests

**Goal:** Finalize the binary crate's surface. Remove the now-empty
`lib.rs` re-exports and rewrite the three lib-using integration tests to
depend on the sub-crates directly.

**Files:**
- `crates/zero/src/lib.rs` — deleted (no longer needed; only re-exports
  remained, and the only consumers are the three integration tests
  about to be migrated)
- `crates/zero/Cargo.toml` — remove `[lib]` entry if one was implicit
- `crates/zero/tests/config_load.rs` — `zero::config::Config::load_from_cwd`
  → `zero_config::Config::load_from_cwd` (two call sites)
- `crates/zero/tests/dev_serves_runtime.rs` — `zero::runtime::runtime_module`
  → `zero_runtime::runtime_module` (one call site)
- `crates/zero/tests/runtime_evaluates.rs` — same single replacement
- `crates/zero/Cargo.toml` `[dev-dependencies]` — add
  `zero-config = { path = "../zero-config" }` and
  `zero-runtime = { path = "../zero-runtime" }` so the tests link
  against them

**Changes:**
- Confirm no other test, example, or doc references `zero::*` (already
  verified — only the three above).
- After deletion, `cargo build -p zero` should produce only a binary,
  no `libzero` artifact.
- Per the spec's `pub(crate)` widening rule: by this point, the only
  cross-crate `pub(crate)` was `scaffold::write_user_files`, already
  widened in Step 10. Recheck with `rg 'pub\(crate\)' crates/` — any
  remaining instances that are read by another crate must be widened
  to `pub` + `#[doc(hidden)]`.

**Tests:** `cargo test --workspace` green. The three migrated tests
exercise the same code paths.

### Step 15: Verify workspace builds, all tests pass, capture rebuild-timing observation

**Goal:** Confirm the structural goals of R2 are met: incremental
rebuilds no longer pull in unrelated crate compilation.

**Files:**
- `issues/internal-quality/post-split-coverage.txt` — new file
- `issues/internal-quality/rebuild-observation.md` — new file

**Changes:**
- Run `cargo build --workspace` from clean (delete `target/`); measure
  wall time. Touch `crates/zero-dev/src/server.rs` (one char change);
  run `cargo build --workspace` again; measure incremental time.
  Repeat for `crates/zero-test-runner/src/harness.rs` and
  `crates/zero-transpile/src/lib.rs`. Record these numbers in
  `rebuild-observation.md` — three rows, before/after columns. (The
  "before" column is qualitative: "previously, any src/ change
  rebuilt swc + boa transitively." Numbers are not required for
  before because we don't want to revert the workspace just to
  measure.)
- Run `cargo llvm-cov --workspace --summary-only > issues/internal-quality/post-split-coverage.txt`.
  Diff against `baseline-coverage.txt` and confirm no module dropped
  below floor. (Coverage tool output paths now include the crate
  name; this is expected.)
- Run `cargo install --path crates/zero --locked --force` against a
  scratch `--root` and confirm the produced binary runs `zero --help`.
- Run the JS-runtime tests: `node --test runtime/*.test.js`. Confirm
  unchanged.

**Tests:** All existing tests pass. No source-code changes in this step
beyond test/coverage instrumentation.

### Step 16: Refactor `zero-test-runner` oversized functions

**Goal:** Reduce four oversized functions below 80 lines while keeping
behavior identical. Per spec, refactoring happens after the crate split
is done, never in place.

**Files:**
- `crates/zero-test-runner/src/harness.rs` — split
  `run_with_loader` (236) and `walk_describe` (103)
- `crates/zero-test-runner/src/coverage.rs` — split
  `instrument` (121) and `build_prologue` (117)

**Changes:**
- `run_with_loader`: extract named private fns for (a) loader
  initialization, (b) per-file evaluation, (c) per-test execution loop,
  (d) result aggregation. Each block becomes a `fn` returning the
  intermediate state it produces. The top-level fn becomes a
  top-to-bottom sequence of these calls.
- `walk_describe`: extract per-block setup, per-`it` execution, and
  hook-running into helpers.
- `instrument`: split the AST-walk into a `visit_*` helper per node
  kind (or a single helper per major branch); the top-level keeps the
  driver loop.
- `build_prologue`: factor the prologue into named string-building
  helpers (e.g. `prologue_for_module`, `prologue_for_globals`).
- No public API change. All existing unit tests pass.

**Tests:** Unit tests in `harness.rs` and `coverage.rs` continue
passing. `cargo test --workspace` green.

### Step 17: Refactor `zero-bundler` oversized functions

**Goal:** Reduce `rewrite_module` (207), `bundle` (118), `process_css`
(117) below 80 lines.

**Files:**
- `crates/zero-bundler/src/bundler.rs`
- `crates/zero-bundler/src/css.rs`

**Changes:**
- `rewrite_module`: extract the import-rewriting visitor, the
  export-collection step, and the per-specifier resolution into
  separate functions.
- `bundle`: extract dependency-graph walk, per-module rewrite call,
  and final concatenation into named steps.
- `process_css`: extract URL rewriting, asset copying, and sourcemap
  generation into helpers.

**Tests:** Bundler unit tests + integration tests (`build_*.rs`)
continue passing.

### Step 18: Refactor `zero-dev` oversized functions

**Goal:** Reduce `serve` (169) and `proxy_request` (130) below 80 lines.

**Files:**
- `crates/zero-dev/src/server.rs`
- `crates/zero-dev/src/proxy.rs`

**Changes:**
- `serve`: factor route registration into named helpers
  (`runtime_routes`, `local_routes`, `proxy_routes`), keep listener
  bind and shutdown in the top-level. Each helper takes the
  `axum::Router` and returns the augmented one.
- `proxy_request`: factor header forwarding, body streaming, and
  upstream error mapping into helpers.

**Tests:** Dev unit tests + integration tests (`dev_*.rs`) continue
passing.

### Step 19: Refactor `zero-transpile::transpile_typescript`

**Goal:** Reduce `transpile_typescript` (150) below 80 lines.

**Files:**
- `crates/zero-transpile/src/lib.rs`

**Changes:** Extract swc setup (compiler config, parser, source map),
the transform pipeline, and the emit step into named helpers. The
top-level becomes a five-line driver.

**Tests:** Transpile unit tests pass. Integration tests
(`build_sourcemap.rs`, `dev_serves_ts.rs`) pass.

### Step 20: Refactor `zero` (binary) `cmd::mutate::run_inner`

**Goal:** Reduce `run_inner` (215) below 80 lines. This is the
remaining oversized function and lives in the binary crate.

**Files:**
- `crates/zero/src/cmd/mutate.rs`

**Changes:** Extract test-discovery, mutation-generation, worker-pool
setup, and per-mutant orchestration into helpers. `worker_main` stays
unchanged (it's a separate function and already under the bar).

**Tests:** Mutate unit tests + `tests/test_runner_smoke.rs` continue
passing. `cargo mutate --quiet` against a fixture project completes
with the same exit-code as before.

### Step 21: Add `scripts/check-coverage.sh` enforcing per-module floors

**Goal:** Implement R4's coverage gate as a local recipe, ready to wire
into CI later.

**Files:**
- `scripts/check-coverage.sh` — new
- `issues/internal-quality/critical-modules.txt` — referenced by the script

**Changes:**
- Script logic: run `cargo llvm-cov --workspace --json --summary-only`,
  parse the JSON, for each file emit pass/fail against the 70 % floor;
  for each file listed in `critical-modules.txt` apply the 85 % floor
  instead. Non-zero exit if any file fails.
- Add a `TODO(ci)` comment in the script header: "Wire into CI when a
  CI host is selected (spec Open Question 4)."
- Document in `CLAUDE.md` under "Commands":
  ```
  # Enforce per-module coverage floors
  ./scripts/check-coverage.sh
  ```

**Tests:** Run the script locally; it must exit 0 against the
post-refactor codebase. Manually break a floor (delete a test) and
confirm non-zero exit, then restore.

### Step 22: Add `scripts/check-fn-size.sh` flagging ≥80-line functions

**Goal:** Implement R4's function-size gate as a local recipe.

**Files:**
- `scripts/check-fn-size.sh` — new

**Changes:**
- Script logic (per Prerequisite 6): for every `.rs` file under
  `crates/*/src/`, find each `^[\t ]*(pub )?(async )?fn ` line, count
  the lines until brace-balance returns to zero. Emit a warning for
  any function ≥80 lines.
- Default mode: warn only, exit 0. With `--strict` flag, exit non-zero
  on any flag.
- Document in `CLAUDE.md`:
  ```
  # Flag any function ≥80 lines (warning-only)
  ./scripts/check-fn-size.sh

  # Same, but exit non-zero on hits (use in CI)
  ./scripts/check-fn-size.sh --strict
  ```
- Add `TODO(ci)` comment.

**Tests:** After Steps 16–20, the script with `--strict` exits 0.
(If any function the spec listed in Background is still over the bar,
fix it; if a refactor justified leaving one over the bar, the script
gets a per-file `# allow: <reason>` comment list at the top — keep
this list short and link to the commit that justified it.)

### Step 23: Update `CLAUDE.md` workspace commands and recapture final coverage

**Goal:** Final documentation pass and final coverage snapshot.

**Files:**
- `CLAUDE.md` — replace the single-crate commands section
- `issues/internal-quality/final-coverage.txt` — new

**Changes:**
- `CLAUDE.md` "Commands" section becomes:
  ```bash
  # Run all Rust tests (workspace)
  cargo test --workspace

  # Run tests for a single crate
  cargo test -p zero-bundler

  # Run all JS runtime tests (unchanged)
  node --test runtime/*.test.js

  # Build / install the CLI
  cargo build --workspace --release
  cargo install --path crates/zero --locked

  # Coverage
  cargo llvm-cov --workspace --html
  ./scripts/check-coverage.sh
  ./scripts/check-fn-size.sh --strict
  ```
- Recapture: `cargo llvm-cov --workspace --summary-only > issues/internal-quality/final-coverage.txt`.
  Confirm no regression vs `post-split-coverage.txt`.

**Tests:** All four `check-*` invocations exit 0. Full
`cargo test --workspace` and `node --test runtime/*.test.js` green.

---

## Risks and Assumptions

- **`cargo:rerun-if-changed=../../runtime/...` portability.** Cargo's
  documented behavior accepts parent-relative paths in
  `rerun-if-changed`, but the docs hedge ("may not work in some
  cases"). If this proves flaky in practice during Step 6, the
  fallback is to move `runtime/` to `crates/zero-runtime/runtime/` and
  update `CLAUDE.md`'s `node --test runtime/*.test.js` line to
  `node --test crates/zero-runtime/runtime/*.test.js`. This is a
  contingency, not the plan.
- **`pub(crate)` widening surprises.** Only one `pub(crate)` was
  found in the current source, but private items used across modules
  in the same file are invisible to grep. If the crate split surfaces
  additional cross-crate calls, Step 14's recheck catches them; each
  one becomes a `pub` + `#[doc(hidden)]` per the spec.
- **`Answers` decoupling cascade.** Introducing `TomlInput` in Step 9
  is the only API-shape change in the entire plan. If `prompts.rs`
  also gains other consumers during execution, this remains correct:
  `zero-config` does not depend on `prompts`, only on the
  field-shaped `TomlInput`.
- **Critical-module list is wrong.** If post-uplift Step 4 shows a
  module other than the six on the locked list is harder to test
  than expected and lands at 71 % instead of 85 %, that's fine
  (the 70 % floor still applies). The 85 % list is a quality bar,
  not a discovery mechanism.
- **Function-size script false positives.** The `awk` heuristic
  can miscount in the presence of string literals containing `{`
  / `}`. If post-Step 20 the script reports false positives, the
  fix is a per-file allow-list at the top of the script. Investing
  in a `syn`-based walker remains explicitly out of scope.
- **No CI host means R4 is partially deferred.** The scripts ship
  and are documented as local recipes. The actual gating happens
  the moment a CI host exists. This is consistent with spec Open
  Question 4.
- **No behavioral change is the central constraint.** Every step
  ends with `cargo test --workspace` + `node --test runtime/*.test.js`
  passing. The integration test suite (32 files) is the proof. If at
  any step that suite regresses, stop and root-cause before
  proceeding — do not delete or skip a test.
