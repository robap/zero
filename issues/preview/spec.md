# Spec: `zero preview` — serve the production build locally

## Problem Statement

`zero preview` is documented in three places — `docs/building-and-deploying.md:92-104`, `docs/config-and-cli.md:82` (Quick Start list), and `docs/config-and-cli.md:251-254` (subcommand reference) — but the subcommand does not exist. Running `zero preview` returns `unrecognized subcommand 'preview'`. The scaffold's `crates/zero-scaffold/src/scaffold/AGENTS.md` propagates the same claim into every new project, so agents and humans alike follow the doc and hit a wall.

This was caught in the demo's friction log (`~/Documents/code/zero_demo/FRAMEWORK_NOTES.md:51`, severity 🔴) and is the only open red entry. Anyone trying to smoke-test a `zero build` artifact follows the doc and is blocked.

Adjacent problem surfaced during scoping: `zero build` never removes stale output. `crates/zero/src/cmd/build.rs:25-26` only `create_dir_all`s `dist/assets/`; it never deletes prior files. So every build leaves the previous run's hashed assets in `dist/assets/`, and the directory accumulates across builds. Since `zero preview` will auto-invoke `zero build` (see R3), shipping `preview` without fixing this would amplify a real pre-existing pile-up problem. The two changes ship together.

## Background

### What the docs already promise

`docs/building-and-deploying.md:92-104` specifies the contract:

> Before deploying, you can run the production build locally:
>
> ```sh
> zero preview
> ```
>
> This serves `dist/` on `http://127.0.0.1:3000` with the same SPA fallback semantics as `zero dev` (unknown paths return `index.html`), but using the compiled bundle. It's how you sanity-check that the build outputs work end-to-end before pushing to prod.

`docs/config-and-cli.md:82` lists it in the Quick Start command table:

> `zero preview                Serve the production build locally`

`docs/config-and-cli.md:251-254` is the subcommand-reference stub that links back to `building-and-deploying.html#zero-preview`.

The implementation follows the docs verbatim — no surprising behavior, no new public surface beyond a subcommand entry.

### Where the dev server primitives live

`zero dev` already provides every primitive `preview` needs:

- `crates/zero-dev/src/server.rs:78` — `bind_listener(config.dev.port)` binds `127.0.0.1:<port>` with a friendly "port in use" message.
- `crates/zero-dev/src/local.rs` and `files.rs` — static-file serving with SPA fallback (unknown paths return `index.html`).
- `crates/zero-dev/src/headers.rs` — cache-defeating headers (`Cache-Control: no-store, no-cache, must-revalidate`, `Pragma: no-cache`, `Expires: 0`).

The dev server also injects HMR, SSE, transpile-on-request, and `.zero/` cache plumbing — none of which `preview` wants. The right factoring is a separate, smaller axum app that reuses `bind_listener` and `headers::no_cache_layer`, plus a static-file + SPA-fallback handler pointed at `config.out_dir_path()` (build output directory).

### Config and CLI shape

`zero dev` has no CLI flags — port comes from `[dev].port` in `zero.toml` (`crates/zero-config/src/config.rs:88`, default 3000). The `Commands::Dev` variant in `crates/zero/src/main.rs:29` is a bare unit variant. `zero preview` matches this shape: no flags, port from `[dev].port`. Decision rationale: one port knob beats two; the docs already say "3000," which is exactly the `[dev].port` default.

Trade-off accepted: a user can't run `zero dev` and `zero preview` simultaneously on the default port. They'd hit the existing "port in use" message and either stop one or change `[dev].port`. That's acceptable for a smoke-test workflow.

### Build's output directory is configurable

`crates/zero-config/src/config.rs:109` shows `[build].out` defaults to `"dist"` but can be overridden. `Config::out_dir_path()` (line 169) returns the resolved path. `preview` must use this — not a hardcoded `"dist"` — or it will silently serve the wrong directory for projects that override `out`.

### What lives in `dist/` today

`zero build` (`crates/zero/src/cmd/build.rs:25-91`) emits:

- `dist/assets/<hash>.<ext>` — bundled JS + CSS (hashed names)
- `dist/assets/<hash>.<ext>.map` — sourcemaps when enabled
- `dist/index.html` — rendered shell with hashed `<script>` / `<link>` tags inlined
- `dist/manifest.json` — manifest of the above
- `dist/public/**` — copied from `src/public/` (or whatever `[project].root/public` resolves to)
- `dist/.zero/fonts/**` — copied from `.zero/fonts/` when present

Nothing in `dist/` is user-curated. `assets/` is bundler-emitted. `public/` is a *copy* of source `public/`. `index.html` and `manifest.json` are regenerated. So nuking `dist/` at the start of each build is safe — no source-of-truth content lives there.

### Adjacent surfaces touched

- **`crates/zero/src/main.rs`** — add `Preview` to the `Commands` enum; route to a new `cmd::preview::run()`.
- **`crates/zero/src/cmd/preview.rs`** — new file; mirrors `cmd/dev.rs` shape (load config, delegate to a serve function).
- **`crates/zero/src/cmd/mod.rs`** — declare the new module.
- **`crates/zero-dev/src/`** — *probably* the right home for the preview server too (rename to `zero-serve`? or add a `preview_server` module here?). Planner decides crate boundary; either keep `zero-dev` and add a sibling module, or move shared bind/headers into a `zero-serve` crate. Recommended: add `crates/zero-dev/src/preview.rs` that reuses `bind_listener` and `headers::no_cache_layer` and exports a `serve_preview(config) -> Result<()>` function. Renaming the crate is out of scope.
- **`crates/zero/src/cmd/build.rs`** — add a `clean_out_dir` step at the top of `run` that nukes `config.out_dir_path()` before `create_dir_all`.
- **`docs/config-and-cli.md`** — the existing `zero preview` subcommand stub is fine; update the Quick Start row if the description changes; add a one-line note about the auto-build behavior.
- **`docs/building-and-deploying.md`** — extend the `zero preview` section with a one-paragraph note that it auto-runs `zero build` first and prints a notice.

## Requirements

### R1 — `zero preview` subcommand exists

`Commands::Preview` is added to the clap enum in `crates/zero/src/main.rs`. Like `Dev`, it's a unit variant with no flags. It dispatches to `cmd::preview::run().await`.

Running `zero preview --help` shows the subcommand in the top-level help; running `zero preview` no longer returns `unrecognized subcommand 'preview'`.

### R2 — Serves the production build with SPA fallback

`cmd::preview::run` loads `Config::load_from_cwd()` and invokes a `serve_preview(config)` function in `zero-dev` (or wherever the planner places it) that:

- Reads `config.out_dir_path()` to find the build output directory.
- Binds `127.0.0.1:<config.dev.port>` using `bind_listener` (so the "port in use" message is shared with `zero dev`).
- Prints `zero preview — listening on http://<addr>` on bind (matching `zero dev`'s startup line shape).
- Serves files from the output directory as static content.
- Returns `index.html` for any path that does not resolve to a file on disk (SPA fallback, matching `zero dev`'s behavior for client-routed paths).
- Applies `headers::no_cache_layer()` so repeated `zero build && zero preview` cycles aren't masked by browser caching.
- Does **not** inject HMR, SSE, transpile-on-request, or any dev-only middleware. It serves the compiled artifacts byte-for-byte.

The 404 / fallback shape must match `zero dev` so a user's local manual testing reflects what they'd see in production behind a static host with the equivalent fallback rule.

### R3 — Auto-builds before serving, with a user-visible notice

`cmd::preview::run` always invokes a build before binding the listener. Sequence:

1. Print a notice on stdout: `zero preview — running zero build…` (exact wording is a planner choice, but it must appear *before* the build's own output so a watcher knows why the build is running).
2. Invoke the build — either by calling `cmd::build::run()` directly or by extracting the build core into a function callable from both. The planner picks the factoring; either is fine as long as the build's existing output (`zero build — N bytes JS, …`) still prints.
3. After build success, bind the listener and serve.
4. If the build fails, exit with the build's error; do not bind.

No flag turns this off. Rationale: the docs sell `preview` as "sanity-check that the build outputs work end-to-end" — serving a stale build silently defeats the point.

### R4 — `zero build` clears the output directory before writing

`cmd::build::run` adds a step before `create_dir_all(&assets_dir)`:

- If `config.out_dir_path()` exists, recursively remove it.
- Then `create_dir_all` the assets directory as today.

This applies whether `build` was invoked directly or via `preview`. After this change, `dist/` contains exactly the files emitted by the most recent build — no stragglers from prior runs.

Edge case: a concurrent `zero preview` reading `dist/` while a `zero build` is mid-removal could 404 mid-request. This is acceptable — concurrent invocations are user-driven and the worst case is a refresh.

### R5 — `[build].out` is respected end-to-end

`preview` reads the directory from `config.out_dir_path()`, not a hardcoded `"dist"`. Build's cleanup (R4) targets the same path. A `zero.toml` with `[build] out = "public/build"` causes both subcommands to operate on `public/build/` without further configuration.

### R6 — Tests

`crates/zero/src/cmd/preview.rs` tests (mirror `cmd/dev.rs`'s test shape):

- `missing_zero_toml_returns_error` — running in a directory without `zero.toml` errors with a message mentioning `zero.toml`.
- `missing_project_root_returns_error` — `zero.toml` points at an absent `[project].root` → error mentioning "not found".
- `auto_builds_before_serving` — set up a tiny project, run preview against a free port (the existing test pattern in `zero-dev/src/server.rs:455` uses `port: 0`), assert `dist/index.html` exists after invocation even though the test never called `zero build` directly. Bound the test on a successful GET of `/` returning `200` with the built `index.html`.
- `sourcemap_pile_up_is_cleared` (or similar) — run build once with sourcemaps on, run build again with sourcemaps off, assert no `.map` files remain in `dist/assets/`. Covers R4 from the build side.

`crates/zero-dev/src/preview.rs` (or wherever the preview server lives) tests:

- `serves_index_for_unknown_path` — SPA fallback returns `index.html` body with `200` for `/some/client/route`.
- `serves_static_file_with_no_cache_headers` — `GET /assets/<hash>.js` returns the file bytes plus `Cache-Control: no-store`.
- `port_in_use_returns_friendly_error` — same shape as `zero dev`'s existing port-in-use test.

`crates/zero/src/cmd/build.rs` tests:

- Existing tests must still pass after the R4 cleanup step is added. If any test asserts on files that survive across builds (e.g. checks for a stale artifact), it's wrong and must be updated.
- Add `clears_out_dir_before_writing`: write a junk file into `dist/assets/junk.txt`, run `build`, assert `junk.txt` is gone but the new artifacts exist.

### R7 — Docs

- `docs/building-and-deploying.md` — extend the existing `zero preview` section with: "zero preview re-runs zero build first, then serves the result. The output directory is cleared at the start of every build, so dist/ only ever contains the most recent run's artifacts."
- `docs/config-and-cli.md` — the Quick Start row description is fine. The subcommand reference (`### zero preview`) gets a sentence noting the auto-build.
- `docs/index.md` — already lists Building and Deploying with "zero preview" in the bullet; no change needed.
- `crates/zero-scaffold/src/scaffold/AGENTS.md` — if it contains `zero preview`, ensure the description matches the implemented behavior (auto-builds, serves on `[dev].port`). If it just lists the command name, no edit needed.

## Constraints

- No npm dependencies; same workspace dependencies as the rest of `zero`.
- The 80-line per-function guideline (CLAUDE.md) applies; if `serve_preview` grows beyond that, split file-serving and SPA-fallback into helpers.
- `zero preview` must share `bind_listener` and `no_cache_layer` with `zero dev` — duplicating that logic guarantees skew over time.
- The build's R4 cleanup must not follow symlinks out of `out_dir`. Use `std::fs::remove_dir_all` on the resolved path; if `out_dir_path()` ever resolves to a symlink target, error rather than silently delete the target. (Today `out_dir_path()` returns `cwd.join(config.build.out)`; it doesn't resolve symlinks. Planner verifies during implementation.)
- The auto-build must use the same `Config` instance the preview server uses, so the dist path and any other build settings can't drift between the two.
- Exit code semantics: a successful build followed by a graceful Ctrl-C of the server returns 0; a build failure returns the build's exit code; a bind failure returns 1 (matching `zero dev`).

## Out of Scope

- A `zero clean` subcommand. R4 makes it unnecessary because every build cleans first. A standalone `clean` can be added later if a use case appears (e.g. CI artifact hygiene without a rebuild).
- A `--no-build` / `--skip-build` flag on `preview`. The docs don't promise it and the user expectation is "smoke-test the build." If a real workflow demands serving the existing `dist/` without rebuilding, it's a separate feature.
- A `--port` CLI override on either `dev` or `preview`. Today's config-only port flow is the convention; changing that is a separate decision.
- Running `dev` and `preview` simultaneously on the default port. Users hit the existing "port in use" error and either stop one or change `[dev].port`.
- Production-grade cache headers (`Cache-Control: public, max-age=31536000, immutable` for hashed assets and `no-cache` for `index.html`). Serving with `no-cache` everywhere is safer for a smoke-test loop; matching prod headers can be a follow-up if anyone needs to test caching behavior locally.
- Live reload in `preview`. Preview serves the compiled bundle; live reload belongs to `dev`.
- Moving dev/preview into a shared `zero-serve` crate. The recommendation is to land the new module inside `zero-dev` for now; restructuring the crate is a separate refactor.

## Open Questions

- **Cache headers.** Recommended: reuse `headers::no_cache_layer` so a `zero build && reload` cycle always shows fresh output. Alternative: prod-mirroring headers (immutable for hashed assets, no-cache for `index.html`) for a more honest smoke test. Plan-phase decision; spec defaults to no-cache.
- **Build invocation factoring.** R3 says "call `cmd::build::run()` directly or extract a core function." The planner picks: (a) call `run()` as-is — simplest, but couples preview to the build command's CLI-facing wrapping; (b) extract a `build_inner(config) -> Result<()>` and have both `cmd::build::run` and `cmd::preview::run` call it. Option (b) is the recommended end-state if `build`'s `run` does any CLI-only work (arg parsing, summary printing) that `preview` shouldn't re-do.
- **Notice wording.** The exact string for "running zero build…" is a small UX choice. Should it be one line before the build's own banner, or should the build's banner itself say "(via zero preview)" when invoked from preview? Spec recommends the former — simplest, no cross-talk between the two commands.
- **Symlink safety on `dist/`.** R4 says error if `out_dir_path()` resolves to a symlink target. Planner confirms whether this needs explicit handling or whether `remove_dir_all` on a symlink does the right thing on Linux/macOS by default. If the latter, no extra code; if the former, a one-line check.
- **Scaffold AGENTS.md sync.** Spec says "edit if needed." Planner reads `crates/zero-scaffold/src/scaffold/AGENTS.md` first; if `zero preview` is just listed without description, no edit. If there's a description that goes stale under the new behavior, update it.
