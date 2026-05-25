# Plan: `zero preview` — serve the production build locally

## Summary

Implement the missing `zero preview` subcommand and harden `zero build` so
`dist/` only ever contains the current build's artifacts. The approach: add a
small static-file + SPA-fallback axum app inside `zero-dev` that reuses
`bind_listener` and `no_cache_layer` (so the dev server and preview share their
network primitives), wire a new `cmd::preview` that auto-runs the build before
serving, and add an output-directory cleanup step to `cmd::build` so the auto-
build leaves no stragglers behind. The build's existing `run` is refactored to
delegate to a `build_inner(&Config, …)` so preview can drive the build with a
shared `Config` instance — meeting the spec's invariant that auto-build and
preview operate on the same config snapshot.

## Prerequisites

Open questions from the spec are resolved as follows; no external blockers:

- **Cache headers** — reuse `headers::no_cache_layer` (spec default).
- **Build invocation factoring** — extract `build_inner(config, override)` and
  call it from both `cmd::build::run` and `cmd::preview::run`. The extracted
  signature lets preview share a single `Config` instance with the build.
- **Notice wording** — preview prints `zero preview — running zero build…` to
  stdout on its own line, before invoking the build (no cross-talk inside
  `build`'s banner).
- **Symlink safety on `dist/`** — explicitly check `symlink_metadata` of
  `out_dir_path()` before `remove_dir_all`; bail with a helpful message if it
  is a symlink. `remove_dir_all` does not follow symlinks on Linux/macOS, but
  the explicit check is one line and surfaces a clearer error.
- **Scaffold AGENTS.md** — the file lists `zero preview     # serve the
  production build locally` (line 24). That description still matches behavior
  after this change (the auto-build is internal). No edit.

## Steps

- [x] **Step 1: Refactor `cmd::build::run` to delegate to `build_inner(&Config, Option<bool>)`**
- [x] **Step 2: Clear `out_dir` at the start of `build_inner`, with symlink safety**
- [x] **Step 3: Add `zero-dev::preview` module with `serve_preview` + router**
- [x] **Step 4: Add `cmd::preview` subcommand wiring auto-build → serve**
- [x] **Step 5: Update docs to describe the auto-build behavior**

---

## Step Details

### Step 1: Refactor `cmd::build::run` to delegate to `build_inner(&Config, Option<bool>)`

**Goal:** Make the build's core logic callable from `cmd::preview::run` with
a shared `Config` instance. No behavior change in this step — pure
refactor that leaves all existing tests passing as-is. Done first because
Step 2 modifies the same function, and a no-op refactor is easier to review
isolated from the new cleanup logic.

**Files:**

- `crates/zero/src/cmd/build.rs`

**Changes:**

- Move the body of `run` (everything after `let config = Config::load_from_cwd()?;`)
  into a new `pub(crate) async fn build_inner(config: &Config, sourcemap_override: Option<bool>) -> anyhow::Result<()>`.
- `pub async fn run(sourcemap_override: Option<bool>) -> anyhow::Result<()>`
  becomes:
  ```rust
  let config = Config::load_from_cwd()?;
  build_inner(&config, sourcemap_override).await
  ```
- Inside `build_inner`, replace `let cwd = std::env::current_dir()?;` + manual
  `cwd.join(...)` for the build output with `config.out_dir_path()`. Keep
  `cwd.join(&config.project.root)` for `root` (no `Config` helper exists for
  that yet and adding one is out of scope).
- Keep `copy_tree` private to the module exactly as today.

**Tests:**

- All existing `cmd::build` tests run unchanged and pass: `missing_zero_toml_returns_error`,
  `override_sourcemap_true_writes_external_map_file`, `override_sourcemap_false_omits_external_map_file`,
  `build_writes_manifest_and_index_html`, `override_none_falls_back_to_config_default`,
  `copy_tree_recurses_and_counts_files`, `build_copies_dot_zero_fonts_into_dist`.

---

### Step 2: Clear `out_dir` at the start of `build_inner`, with symlink safety

**Goal:** Guarantee `dist/` contains only the current build's outputs. This is
R4 from the spec. Lands before the preview command exists so the cleanup is
exercised by every existing build invocation and any new build-side tests.

**Files:**

- `crates/zero/src/cmd/build.rs`

**Changes:**

- At the top of `build_inner`, before `create_dir_all(&assets_dir)`, add:
  ```rust
  let out_dir = config.out_dir_path();
  if let Ok(meta) = std::fs::symlink_metadata(&out_dir) {
      if meta.file_type().is_symlink() {
          anyhow::bail!(
              "build.out `{}` is a symlink; refuse to delete through it. \
               Remove the symlink and run `zero build` again.",
              out_dir.display()
          );
      }
      std::fs::remove_dir_all(&out_dir)?;
  }
  ```
- Continue with the existing `assets_dir = out_dir.join("assets");
  std::fs::create_dir_all(&assets_dir)?;`.

**Tests:**

- Add `clears_out_dir_before_writing` in `cmd/build.rs`:
  - Write minimal project (reuse `write_minimal_project`).
  - Manually create `dist/assets/` and write `dist/assets/junk.txt`.
  - Call `super::run(None).await`.
  - Assert `dist/assets/junk.txt` no longer exists.
  - Assert `dist/index.html` and `dist/manifest.json` do exist.
- Add `clears_stale_sourcemap_on_disabled_rebuild`:
  - Run `super::run(Some(true)).await` once (writes a `.map` file).
  - Run `super::run(Some(false)).await` again.
  - Assert no `.map` files remain under `dist/assets/`. (Spec R6 second
    build-side test.)
- Add `errors_when_out_dir_is_symlink`:
  - Write minimal project.
  - Create a sibling directory `other_dist/` and `std::os::unix::fs::symlink`
    it as `dist`.
  - Call `super::run(None).await` and assert the error message contains
    `symlink`. (Gate with `#[cfg(unix)]`.)
- Existing `cmd::build` tests must still pass — none of them depend on a stale
  artifact surviving across builds.

---

### Step 3: Add `zero-dev::preview` module with `serve_preview` + router

**Goal:** Stand up the preview HTTP layer in `zero-dev`, sharing
`bind_listener` and `no_cache_layer` with `zero dev`. Self-contained and
testable before any CLI wiring exists.

**Files:**

- `crates/zero-dev/src/preview.rs` (new)
- `crates/zero-dev/src/lib.rs` (add `pub mod preview;`)
- `crates/zero-dev/src/server.rs` (change `bind_listener` from `async fn` to
  `pub(crate) async fn`)

**Changes:**

- `server.rs`: relax `bind_listener` visibility to `pub(crate)` so
  `preview.rs` can call it. No other change to `server.rs`.
- `preview.rs` — top-level shape:

  ```rust
  //! `zero preview` — static-file server for the production build.

  use std::path::PathBuf;
  use std::sync::Arc;

  use axum::Router;
  use axum::body::Body;
  use axum::extract::State;
  use axum::http::{Request, StatusCode, header};
  use axum::response::{IntoResponse, Response};

  use zero_config::Config;

  use crate::files::content_type_for;
  use crate::headers::no_cache_layer;
  use crate::server::bind_listener;

  #[derive(Clone)]
  struct PreviewState {
      out_dir: PathBuf,
  }

  /// Start the preview server and block until shutdown.
  pub async fn serve_preview(config: &Config) -> anyhow::Result<()> {
      let out_dir = config.out_dir_path();
      let listener = bind_listener(config.dev.port).await?;
      println!(
          "zero preview — listening on http://{}",
          listener.local_addr()?
      );
      let app = build_preview_app(out_dir);
      axum::serve(listener, app)
          .with_graceful_shutdown(async {
              let _ = tokio::signal::ctrl_c().await;
          })
          .await?;
      Ok(())
  }

  pub(crate) fn build_preview_app(out_dir: PathBuf) -> Router {
      let state = Arc::new(PreviewState { out_dir });
      Router::new()
          .fallback(handle_request)
          .layer(no_cache_layer())
          .with_state(state)
  }

  async fn handle_request(
      State(state): State<Arc<PreviewState>>,
      req: Request<Body>,
  ) -> Response { /* see "handler logic" below */ }
  ```

- Handler logic for `handle_request` (extracted into helpers so no single
  function exceeds ~80 lines):
  1. Reject any URI path containing a `..` segment → 403 (mirrors
     `files.rs::serve_under`).
  2. Strip the leading `/`; compute `candidate = state.out_dir.join(rel)`.
  3. If `rel` is empty (`GET /`) or `candidate` resolves to a directory,
     fall through to SPA fallback (serve `index.html`).
  4. Canonicalize both `state.out_dir` and `candidate`. If `candidate`
     canonicalizes successfully and starts with the canonicalized
     `out_dir`, serve the file with `content_type_for(&candidate)`.
  5. If `candidate` does not exist or is a directory, serve
     `out_dir/index.html` with `Content-Type: text/html; charset=utf-8`
     and status `200`. This is the SPA fallback.
  6. If `out_dir/index.html` itself is missing, return 500 with a message
     pointing at `zero build` (mirrors `local.rs`'s "run `zero init`
     first" pattern but pointing at `zero build`).
- Two private helpers keep the public `handle_request` under 80 lines:
  - `async fn serve_static(out_root: &Path, candidate: &Path) -> Option<Response>`
    — returns `Some(resp)` if a file under `out_root` is served, else
    `None` (caller does SPA fallback).
  - `async fn serve_spa_index(out_root: &Path) -> Response` — reads
    `out_root/index.html` and returns it, or 500.

**Tests** (in `crates/zero-dev/src/preview.rs`):

- `serves_index_for_unknown_path`:
  - Create temp `out_dir` with `index.html` containing `"<!doctype html>SPA"`.
  - Build router via `build_preview_app(out_dir)`.
  - `oneshot` GET `/some/client/route` → status 200, body contains `SPA`,
    `Content-Type` includes `text/html`.
- `serves_static_file_with_no_cache_headers`:
  - Write `out_dir/assets/app.abc123.js` with `"console.log(1)"`.
  - GET `/assets/app.abc123.js` → 200, body bytes match,
    `Content-Type: application/javascript; charset=utf-8`,
    `Cache-Control` contains `no-store`.
- `serves_root_returns_index`:
  - GET `/` → 200, body is `index.html` contents.
- `traversal_returns_403`:
  - GET `/../etc/passwd` → 403.
- `missing_index_returns_500_for_spa_fallback`:
  - `out_dir` exists but is empty.
  - GET `/anything` → 500.
- `serves_public_subtree_files`:
  - Write `out_dir/public/robots.txt`.
  - GET `/public/robots.txt` → 200, body matches.

Note: the port-in-use test from spec R6 is already exercised by
`bind_listener`'s shared use in `zero dev`, so it doesn't need re-asserting
here — but add `port_in_use_returns_friendly_error`:
  - Bind a `TcpListener` on `127.0.0.1:0`, capture the port.
  - Construct a `Config` directly with `dev.port = <that port>`, an
    `out_dir` pointing at a temp dir with a stub `index.html`.
  - Call `serve_preview(&config).await` → expect `Err` whose message
    contains `port` and `already in use`.

---

### Step 4: Add `cmd::preview` subcommand wiring auto-build → serve

**Goal:** Expose the new functionality via the CLI. Calls `build_inner` first
then `serve_preview`, both with the same `Config` instance (R7 invariant).

**Files:**

- `crates/zero/src/cmd/preview.rs` (new)
- `crates/zero/src/cmd/mod.rs` (add `pub mod preview;`)
- `crates/zero/src/main.rs` (add `Preview` variant + dispatch)

**Changes:**

- `crates/zero/src/cmd/preview.rs`:

  ```rust
  //! `zero preview` subcommand entry point.

  use zero_config::Config;
  use zero_dev::preview::serve_preview;

  use crate::cmd::build::build_inner;

  pub async fn run() -> anyhow::Result<()> {
      let config = Config::load_from_cwd()?;
      println!("zero preview — running zero build…");
      build_inner(&config, None).await?;
      serve_preview(&config).await
  }
  ```

- `crates/zero/src/cmd/mod.rs`: add `pub mod preview;`.
- `crates/zero/src/main.rs`:
  - Add `Preview` unit variant to `Commands` with doc
    `/// Build, then serve the production output locally`.
  - Add dispatch arm `Commands::Preview => cmd::preview::run().await,`.
- `cmd::build::build_inner` (Step 1) needs `pub(crate)` visibility — adjust
  if Step 1 left it private. It must be `pub(crate)` so `cmd::preview::run`
  can call it.

**Tests** (in `crates/zero/src/cmd/preview.rs`):

- `missing_zero_toml_returns_error` — mirrors `cmd/dev.rs`. Uses `CWD_LOCK`.
- `missing_project_root_returns_error` — mirrors `cmd/dev.rs`. Writes a
  `zero.toml` pointing at a non-existent `[project] root`, asserts error
  message contains `not found`.
- `auto_builds_before_serving_then_serves_index`:
  - Hold `CWD_LOCK`. Create a temp project (reuse the shape of `cmd/build.rs`'s
    `write_minimal_project`), with the addition of a `[dev] port = 0` line so
    a freshly-allocated port is used. (Side note: the config parser rejects
    `port = 0` via TOML; this test instead constructs `Config` directly,
    same pattern as `zero-dev/src/server.rs:455`'s
    `serve_returns_error_when_root_missing` — bypass the TOML loader.)
  - Define a thin local helper `run_with_config(config) -> Result<()>` that
    mirrors `cmd::preview::run` but takes a pre-built `Config`. Either
    expose this in `cmd::preview` (recommended: `pub(crate)`) or duplicate
    the body inline in the test.
  - Spawn `run_with_config(config)` via `tokio::spawn`.
  - In a retry loop (≤500 ms), `tokio::net::TcpStream::connect`-poll the
    chosen port until it accepts.
    Allocation strategy: bind a throwaway `TcpListener` on
    `127.0.0.1:0`, capture its port, drop it, and pass that port via
    the Config. Race tolerable for tests.
  - Issue `reqwest::get("http://127.0.0.1:<port>/")`; assert 200 and that
    the body contains the built `index.html`'s script-tag reference to
    `assets/app.` (matches `cmd::build`'s manifest output).
  - Assert `dist/index.html` exists on disk (proves auto-build ran).
  - Call `JoinHandle::abort()` and `.await.err()` to clean up.
- `build_failure_does_not_bind`:
  - Write a `zero.toml` whose `[project] root` exists but `src/app.ts` is
    invalid TS so `build_inner` errors. Choose a Config with port = 0 so
    *if* the listener somehow bound it wouldn't collide with anything.
  - Pick a port via the bind-throwaway trick. Assert the port is **not**
    accepting connections after the call returns Err. Concretely: assert
    `cmd::preview::run_with_config(config).await.is_err()` and then a
    follow-up TCP connect to that port refuses. (Belt-and-braces — the
    sequence `build_inner → bind` guarantees this; the test pins the
    invariant.)

**Exit-code semantics check:** `main.rs` already maps `Err` from any subcommand
to `eprintln!` + `process::exit(1)`. Bind failure inside `serve_preview`
propagates as `Err` — exits 1. Build failure propagates as `Err` before bind
— exits 1. Ctrl-C during serve drives `with_graceful_shutdown` to completion
and returns `Ok(())` — exits 0. This matches the spec's R-section constraint
on exit codes; no additional logic needed.

---

### Step 5: Update docs to describe the auto-build behavior

**Goal:** Bring the docs in line with implemented behavior (R7). Keep the
existing structure; small additions, no rewrites.

**Files:**

- `docs/building-and-deploying.md`
- `docs/config-and-cli.md`

**Changes:**

- `docs/building-and-deploying.md` — extend the existing `## zero preview`
  section by appending a paragraph after line 104:

  > `zero preview` first runs `zero build`, then serves the result. Because
  > the build clears the output directory before writing, `dist/` only ever
  > contains the most recent build's artifacts — no stragglers from prior
  > runs.

- `docs/config-and-cli.md` — replace the subcommand reference body (lines
  252–254) with:

  > Builds, then serves the production output locally. See
  > [Building and Deploying § zero preview](./building-and-deploying.html#zero-preview).

- `docs/index.md` — no change (line 51 already lists `zero preview` in the
  Building and Deploying chapter bullet).
- `crates/zero-scaffold/src/scaffold/AGENTS.md` — no change. Line 24's
  `serve the production build locally` description still matches behavior
  (the auto-build is an implementation detail; from the user's perspective
  the command "serves the production build").

**Tests:**

- None — docs only.

---

## Risks and Assumptions

- **Assumption:** `std::fs::remove_dir_all` does not follow symlinks on
  Linux/macOS (verified — it errors when given a path that is a symlink
  rather than a directory). Step 2 still does the `symlink_metadata` check
  for a clearer error and for forward-compat with platforms where this may
  differ.
- **Assumption:** Tests can construct `zero_config::Config` directly,
  bypassing `from_toml_str`'s port-0 validation. Verified at
  `crates/zero-dev/src/server.rs:450-463`.
- **Risk:** The integration-style `auto_builds_before_serving_then_serves_index`
  test races against the port allocation (bind throwaway → drop → bind in
  preview). On a quiet test machine this is reliable; on a heavily contended
  CI runner the port could be reused by another process between drop and
  rebind. Acceptable for a smoke-style test; if flakiness materializes,
  fall back to driving `build_preview_app` via `tower::ServiceExt::oneshot`
  for the GET assertion and keep the `dist/index.html` existence check as
  the auto-build proof.
- **Risk:** Aborting the spawned `serve_preview` task in the test leaves the
  listener socket in `TIME_WAIT`. This is fine for a single test run because
  each test picks a fresh port; serialization via `CWD_LOCK` already
  prevents two preview tests from racing on the same port.
- **Risk:** `bind_listener` visibility change to `pub(crate)` could be lifted
  to `pub` later if a third caller appears outside the crate; today
  `pub(crate)` is sufficient and minimizes surface.
- **Assumption:** No `cmd::build` test relies on stale artifacts surviving.
  Confirmed by reading `crates/zero/src/cmd/build.rs:113-258` — every test
  starts from a fresh `tempfile::tempdir()`.
- **Replan trigger:** If `build_inner` grows beyond ~80 lines after the
  cleanup step, split the bundle / css / manifest / index-html stages into
  helpers before adding more logic. The current `run` is ~70 lines; adding
  ~8 lines of cleanup keeps it within the CLAUDE.md guideline.
