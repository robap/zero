# Plan: CLI Bootstrap — `zero init`, `zero dev`, `zero build`

## Summary

Build the Rust CLI binary that bootstraps a zero project (`zero init`),
serves it with a transforming reverse-proxy dev server (`zero dev`),
and produces a deployable bundle (`zero build`). The runtime is
embedded at compile time via a `build.rs`-driven concatenation of
`runtime/*.js`. The dev server uses `axum` + `tokio` + `reqwest`. The
build is a hand-rolled CommonJS-style module wrapper bundler with
regex-driven import/export rewriting — small enough to maintain
without a JS-bundler crate dependency, sufficient for the scaffold's
module shape.

Twelve incremental steps, each leaving the tree compilable and tests
green. Risk concentrates in two places: the runtime concatenation (a
circular import between `app.js` and `router.js` plus an `export ... as
... ` alias) and the build-time bundler (regex-driven ESM rewriting
that must handle the scaffold's import shapes). Both have explicit
fallbacks called out.

## Prerequisites

None of the spec's open questions block execution — they're recorded
recommendations the plan locks in below:

- **Bundler crate choice:** *no external bundler*. Hand-rolled module
  wrapper using regex rewriting. Sufficient for scaffold-shape
  imports; if user code grows complex, upgrade to `oxc_parser`-driven
  AST rewriting in a follow-up.
- **Runtime concat strategy:** regex rewrite at compile time. The
  runtime files use a small, fixed set of import shapes (verified by
  inspection: `import { x, y } from "./reactivity.js"` style only).
  Cycle between `app.js` and `router.js` resolves cleanly post-concat
  (top-level function declarations only).
- **HTML parser vs string injection:** case-insensitive find of
  `</head>` with fallbacks, no real HTML parser.
- **`manifest.json` key shape:** Vite-style source-relative
  (`"src/app.js"`, `"styles/app.css"`).
- **Port collision behavior:** exit non-zero with a clear error.
- **Prompt crate:** `dialoguer`. Mockable via splitting the prompt
  loop from the toml-write function.
- **Validation of prompt answers:** inline re-prompt on bad input.
- **Missing zero.toml on `dev` / `build`:** error message includes
  the hint "run `zero init` to create one".
- **Up-front check on `./<root>/`:** check after the folder name is
  known (after the first prompt or after reading toml). Don't
  pre-flight against `./web/`.

---

## Steps

- [x] **Step 1: Cargo dependencies and CLI skeleton**
- [x] **Step 2: `zero.toml` parsing and validation**
- [x] **Step 3: Runtime concatenation via `build.rs`**
- [x] **Step 4: `zero init` — scaffold templates, prompts, toml writer**
- [x] **Step 5: `zero dev` — server lifecycle and `/zero.js`**
- [x] **Step 6: `zero dev` — disk file serving**
- [x] **Step 7: `zero dev` — HTML injection (pure function)**
- [x] **Step 8: `zero dev` — local `index.html` fallback (no-proxy mode)**
- [x] **Step 9: `zero dev` — proxy mode**
- [x] **Step 10: `zero build` — bundler**
- [x] **Step 11: `zero build` — CSS hashing, `manifest.json`, static `index.html`**
- [x] **Step 12: End-to-end integration tests**

---

## Step Details

### Step 1: Cargo dependencies and CLI skeleton

**Goal:** Get a real CLI binary that recognizes `init`, `dev`, `build`
subcommands and emits help / version. Subcommands are stubs that
print "not implemented yet" and exit non-zero. This sets up the dep
graph so subsequent steps don't have to revisit `Cargo.toml`.

**Files:**
- `Cargo.toml` (modify)
- `src/main.rs` (rewrite)
- `src/cmd/mod.rs` (new)
- `src/cmd/init.rs` (new, stub)
- `src/cmd/dev.rs` (new, stub)
- `src/cmd/build.rs` (new, stub)

**Changes:**
- Add to `Cargo.toml` `[dependencies]`:
  - `clap = { version = "4", features = ["derive"] }`
  - `tokio = { version = "1", features = ["macros", "rt-multi-thread", "signal", "fs", "io-util", "net"] }`
  - `axum = "0.7"`
  - `hyper = { version = "1", features = ["full"] }`
  - `reqwest = { version = "0.12", default-features = false, features = ["stream"] }` (no compression — we send `Accept-Encoding: identity`)
  - `tower = "0.5"`
  - `tower-http = { version = "0.6", features = ["set-header"] }`
  - `toml = "0.8"`
  - `serde = { version = "1", features = ["derive"] }`
  - `serde_json = "1"`
  - `dialoguer = "0.11"`
  - `sha2 = "0.10"`
  - `anyhow = "1"`
  - `url = "2"`
  - `regex = "1"`
- Add to `[dev-dependencies]`:
  - `tempfile = "3"`
  - `assert_cmd = "2"` (for binary-invocation integration tests)
  - `predicates = "3"`
- `src/main.rs`: define a clap derive enum `Cli { Init, Dev, Build }`,
  parse args, dispatch to `cmd::{init,dev,build}::run(...)`. Each
  `run` returns `anyhow::Result<()>`. Print errors via `eprintln!`
  and `std::process::exit(1)` on failure.
- `src/cmd/{init,dev,build}.rs`: each defines `pub async fn run(...) -> anyhow::Result<()>` returning `Err(anyhow!("not implemented"))` for now (or `Ok(())` for `init` to keep CI happy — pick one and be consistent).
- The `main` function uses `#[tokio::main]` so dev / build can do async work later.
- Global `--version` from clap auto-derives from `Cargo.toml` version; `--help` is automatic.

**Tests:**
- `tests/cli_skeleton.rs` (new): use `assert_cmd` to run the binary
  with `--help` and assert output contains `init`, `dev`, `build`.
  Run with `--version` and assert it prints something matching
  `zero <semver>`.
- Run the binary with no args and an unknown subcommand; assert
  non-zero exit.

---

### Step 2: `zero.toml` parsing and validation

**Goal:** A standalone, well-tested config module. Subsequent steps
just call `Config::load_from_cwd()`.

**Files:**
- `src/config.rs` (new)
- `src/main.rs` or `src/lib.rs` (mod declaration)

**Changes:**
- Define `pub struct Config` with sub-structs:
  ```rust
  pub struct Config {
      pub project: ProjectConfig,
      pub dev: DevConfig,
      pub build: BuildConfig,
  }
  pub struct ProjectConfig { pub root: String }
  pub struct DevConfig { pub port: u16, pub proxy: Option<url::Url> }
  pub struct BuildConfig { pub out: String }
  ```
- Internal `RawConfig` with `serde(deny_unknown_fields)` on all
  structs (to reject typos). Parse with `toml::from_str`. Convert
  `RawConfig` → `Config` with validation:
  - `project.root` required, non-empty, no `..`, no leading `/`,
    no `\\` (Windows backslash), passes a single-segment-or-deeper
    path check (forbid `Path::new(s).has_root()` and any component
    that is `Component::ParentDir`).
  - `dev.port` defaults to 3000; must be 1–65535 (u16 max enforces
    upper bound automatically).
  - `dev.proxy` parsed via `url::Url::parse`; scheme must be `http`
    (reject `https`, `ws`, `wss`, etc.).
  - `build.out` defaults to `"dist"`; same path-escape rules as
    `project.root`.
- `pub fn load_from_cwd() -> anyhow::Result<Config>`: read
  `./zero.toml`, parse, validate. On `NotFound`, return a typed
  error with the message `"zero.toml not found at <cwd>; run \`zero init\` to create one"`.
- `pub fn project_root() -> PathBuf`: helper for `<root>` as a `PathBuf`
  joined to the CWD; canonicalize and verify it sits under the CWD
  (defense-in-depth against config edits between load and use).
- `pub fn out_dir() -> PathBuf`: same shape for `build.out`.

**Tests:**
- `src/config.rs` `#[cfg(test)] mod tests` (unit, no FS):
  - Happy-path TOML parses to expected `Config`.
  - Missing `[project] root` errors with a clear message.
  - `..` in `root` rejected.
  - Absolute path in `root` rejected.
  - Port `0` and port `65536`-style values rejected (the latter via
    serde's u16 parse).
  - `https://` proxy rejected.
  - Unknown top-level key (e.g. `[server]`) rejected.
  - Unknown key inside `[dev]` rejected.
  - Defaults: empty `[dev]` / `[build]` sections produce port=3000,
    out="dist".
- `tests/config_load.rs` (integration, uses `tempfile`): write a
  toml file, `chdir` into the temp dir, call `load_from_cwd`,
  assert. Then assert the not-found error message includes "run
  `zero init` to create one".

---

### Step 3: Runtime concatenation via `build.rs`

**Goal:** A compile-time-baked `pub const ZERO_RUNTIME: &str` that
holds the four runtime files merged into a single ES module exposing
the public surface. This unblocks `zero dev` (serves it) and
`zero build` (resolves `import "zero"` against it).

**Files:**
- `build.rs` (new, at repo root)
- `src/runtime.rs` (new)
- `src/main.rs` (`mod runtime;` declaration)

**Changes:**
- `build.rs`:
  - `cargo:rerun-if-changed=runtime/reactivity.js`
  - `cargo:rerun-if-changed=runtime/template.js`
  - `cargo:rerun-if-changed=runtime/router.js`
  - `cargo:rerun-if-changed=runtime/app.js`
  - For each file in dependency order (`reactivity.js`, `template.js`,
    `router.js`, `app.js`):
    1. Read file contents.
    2. **Strip imports.** Remove every line matching
       `^\s*import\s+.*\s+from\s+['"][^'"]+['"];?\s*$`. Multi-line
       imports also handled — match `^\s*import\s*\{[\s\S]*?\}\s*from\s+['"][^'"]+['"];?` with a multiline-aware regex. (Verified by inspection: runtime imports are short and single-line in practice; the multi-line pattern is defense.)
    3. **Strip export keywords on declarations.** `export function` →
       `function`, `export class` → `class`, `export const` → `const`,
       `export let` → `let`. Preserves the symbols at module top-level
       so they're visible to other concatenated bodies.
    4. **Handle `export { x as y }` aliases.** For
       `export { createScope as _createScope }` (the only alias in
       the runtime today), append a line `const _createScope = createScope;`
       at the end of that file's body. General rule: any matched
       `export\s*\{\s*([^}]+)\s*\}\s*;?` with `as` aliases gets
       converted to `const <new> = <old>;` lines.
    5. **Drop bare `export { name };` re-export blocks** (no body
       impact — symbol is already in scope post-concat).
  - Concatenate the four cleaned bodies in order. Wrap each file's
    body with a top comment `/* === <filename> === */` for debugging.
  - Append the public-surface re-export block:
    ```js
    export {
      signal, computed, effect,
      html, commit, each, ref,
      App, inject,
      navigate, back, forward, route,
    };
    ```
  - Write the result to `OUT_DIR/zero_runtime.js`.
- `src/runtime.rs`:
  ```rust
  pub const ZERO_RUNTIME: &str =
      include_str!(concat!(env!("OUT_DIR"), "/zero_runtime.js"));
  ```

**Tests:**
- `src/runtime.rs` `#[cfg(test)] mod tests`:
  - `ZERO_RUNTIME` non-empty.
  - Contains `function signal(`, `class App`, `function html(`,
    `function commit(`, `function navigate(`, `function route(`.
  - Ends with the public `export { ... }` block.
  - Does NOT contain a top-level `import ` statement (regex match
    `(?m)^\s*import\s`). Does NOT contain a bare `export { x }`
    re-export (only the final aggregate one).
  - Contains `const _createScope = createScope;` (the alias
    flattening).
- An integration test (`tests/runtime_evaluates.rs`) writes
  `ZERO_RUNTIME` to a temp `.mjs` file and runs
  `node --input-type=module --eval "await import('file://...');"`.
  Skip the test gracefully (`#[ignore]` or `eprintln!` skip with
  `Ok(())`) if `node` isn't on PATH; otherwise assert exit 0. This
  catches concat bugs at CI time without making `node` mandatory
  for `cargo test`.

---

### Step 4: `zero init` — scaffold templates, prompts, toml writer

**Goal:** `zero init` works end-to-end in both branches. Embedded
scaffold templates land in `./<root>/`. Interactive wizard
collects four answers and writes `./zero.toml` when the toml is
absent.

**Files:**
- `src/scaffold/index.html` (new)
- `src/scaffold/src/app.js` (new)
- `src/scaffold/src/routes/home.js` (new)
- `src/scaffold/styles/app.css` (new)
- `src/scaffold.rs` (new) — embedded constants + `write_to(dir, &context)`
- `src/prompts.rs` (new) — `Answers` struct + `prompt_user() -> Answers`
- `src/toml_writer.rs` (new) — `render_toml(&Answers) -> String`
- `src/cmd/init.rs` (replace stub)

**Changes:**

**Scaffold contents (intentionally minimal, runtime-correct):**

`src/scaffold/index.html`:
```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{{title}}</title>
  <link rel="stylesheet" href="/styles/app.css">
</head>
<body>
  <div id="app"></div>
</body>
</html>
```
No `<script>` tag — `zero dev` and `zero build` inject one.

`src/scaffold/src/app.js`:
```js
import { App } from "zero";
import Home from "./routes/home.js";

const app = new App();
app.route("/", Home);
export default app;
```
Note: top-level `app.run("#app")` is omitted because the spec says
the developer's HTML calls it. But the scaffold's `index.html`
doesn't have a `<script>` block to do that... so the scaffold needs
either (a) a tiny boot block at the bottom of `app.js` calling
`app.run("#app")` after import, or (b) the injected entry script
handles boot. **Decision (plan-level):** the entry script that
`zero dev` injects (`<script type="module" src="/src/app.js">`)
loads `src/app.js`; we add `app.run("#app")` at the bottom of
`src/app.js`. This matches "the developer owns the HTML" (the HTML
declares `<div id="app">`) while avoiding the developer needing to
write the boot line themselves in this slice. Update the `app.js`
template to:
```js
import { App } from "zero";
import Home from "./routes/home.js";

const app = new App();
app.route("/", Home);
app.run("#app");
```
(No default export needed.)

`src/scaffold/src/routes/home.js`:
```js
import { html } from "zero";

export default function Home() {
  return html`<h1>Hello from zero</h1>`;
}
```

`src/scaffold/styles/app.css`:
```css
body { font-family: system-ui, sans-serif; padding: 2rem; }
h1 { color: #3b82f6; }
```

**`src/scaffold.rs`:**
- `pub struct ScaffoldContext { pub title: String }`
- For each template: `const TPL_INDEX_HTML: &str = include_str!("scaffold/index.html");` etc.
- `pub fn write_to(root_dir: &Path, ctx: &ScaffoldContext) -> anyhow::Result<()>`:
  - Create `root_dir`, `root_dir/src/routes`, `root_dir/styles` (use `fs::create_dir_all`).
  - For each template, do `{{title}}` → `ctx.title` substitution and write to disk.
  - Return error if any write fails.

**`src/prompts.rs`:**
- `pub struct Answers { pub root: String, pub port: u16, pub proxy: Option<String>, pub out: String }`
- `pub fn prompt_user() -> anyhow::Result<Answers>`:
  - Use `dialoguer::Input` for each prompt with a default. Validation
    closure on each:
    - Root: non-empty, no `/`, no `\\`, no `..`, no leading `.` (avoid
      hidden dirs).
    - Port: u16 in 1–65535 (`Input<u16>` parses automatically).
    - Proxy: empty OK; if non-empty, must parse as `http://...` URL.
    - Out: same path rules as root.
  - On bad input, dialoguer re-prompts in place (its `validate_with`).
  - Return populated `Answers`.

**`src/toml_writer.rs`:**
- `pub fn render_toml(a: &Answers) -> String`. Always emit the
  `[project] root = "<root>"` line. For `[dev]`: emit `port = N`
  always; emit `proxy = "..."` only if `Some`. For `[build]`: emit
  `out = "<out>"` always. Defaults shown as commented-out helpful
  hints when the user accepted them, e.g.:
  ```toml
  [project]
  root = "web"

  [dev]
  port = 3000
  # proxy = "http://localhost:8080"

  [build]
  out = "dist"
  ```
  Comment-or-active rule: a key is active if its value differs from
  the default OR if it's required (only `project.root`). Otherwise
  it's commented. Plan permits the simpler "always active" variant
  if commenting proves fiddly; both round-trip the same way through
  the parser.

**`src/cmd/init.rs`:**
- `pub async fn run() -> anyhow::Result<()>`:
  1. CWD = `std::env::current_dir()?`.
  2. `let toml_path = cwd.join("zero.toml");`
  3. **If `toml_path` does not exist:**
     - Print a one-liner banner: "zero init — let's set up a project".
     - Call `prompts::prompt_user()` to get `Answers`.
     - `let toml_str = toml_writer::render_toml(&answers);`
     - Write `toml_str` to `toml_path` (refuse if it now exists due
       to race — `OpenOptions::new().write(true).create_new(true)`).
     - Build `Config` from `Answers` (or just re-load via
       `Config::load_from_cwd`).
  4. **Else (toml exists):** call `Config::load_from_cwd()`.
  5. `let root_dir = cwd.join(&config.project.root);`
  6. If `root_dir` exists and is non-empty (any entry inside via
     `fs::read_dir(&root_dir)?.next().is_some()`): error with
     `"zero init: ./<root>/ is not empty; refusing to overwrite"`.
  7. Compute scaffold title: basename of `cwd`. Fallback to
     `"My zero app"` if basename can't be determined.
  8. `scaffold::write_to(&root_dir, &ctx)?`.
  9. Print success summary (files written, next steps: `zero dev`).

**Tests:**
- `src/scaffold.rs` `#[cfg(test)] mod tests`: `write_to` into a
  tempdir, assert all four files exist with non-empty content,
  assert `index.html` contains the substituted title.
- `src/toml_writer.rs` `#[cfg(test)] mod tests`: build an `Answers`,
  render, parse the result back via the `config` module, assert
  the parsed `Config` matches the original `Answers`. Test
  no-proxy case: `Answers { proxy: None, .. }` produces a toml
  whose parse yields `dev.proxy = None`.
- `tests/init_existing_toml.rs` (integration): write a `zero.toml`
  in a tempdir, run the binary with `init` from that tempdir, assert
  the scaffold appears in `./<root>/`. Asserts no prompts are read.
- `tests/init_refuses_nonempty_root.rs`: write `zero.toml`, also
  write a file inside `./<root>/`, run `init`, assert non-zero exit
  and error message.
- The interactive prompt branch is **not** integration-tested in
  this slice (would need a PTY or scripted stdin via `dialoguer`'s
  `Term` mock). Unit tests on `prompt_user`'s validation closures
  via `dialoguer::FuzzSelect` mocking are deferred. The toml-writer
  unit test covers the toml-shape assertions; the existing-toml
  integration test covers the scaffold path.

---

### Step 5: `zero dev` — server lifecycle and `/zero.js`

**Goal:** A real `axum` server starts, binds the configured port,
serves `/zero.js` with no-cache headers, and shuts down cleanly on
Ctrl-C. Anything else 404s.

**Files:**
- `src/cmd/dev.rs` (replace stub)
- `src/dev/mod.rs` (new)
- `src/dev/server.rs` (new)
- `src/dev/headers.rs` (new — the `apply_no_cache` helper)
- `src/main.rs` (`mod dev;` if needed)

**Changes:**
- `src/dev/headers.rs`:
  ```rust
  pub fn no_cache_layer() -> tower_http::set_header::SetResponseHeaderLayer<...>
  ```
  Returns a layer (or three composed layers) that overrides
  `Cache-Control`, `Pragma`, `Expires` on every response. Use
  `SetResponseHeaderLayer::overriding(...)` so the values replace
  any prior settings. (Proxy responses get a separate strip step in
  Step 9 — this layer guarantees the *outgoing* values are correct
  for everything the server emits.)
- `src/dev/server.rs`:
  - `pub async fn serve(config: Config) -> anyhow::Result<()>`:
    - Build `Router` with one route: `.route("/zero.js", get(serve_runtime))`.
    - `.layer(no_cache_layer())`.
    - Bind `127.0.0.1:<port>` via `tokio::net::TcpListener`. On
      `AddrInUse`, return an error with `"port <N> is already in use; pick a different [dev].port in zero.toml"`.
    - Print `"zero dev — listening on http://127.0.0.1:<port>"`.
    - `axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()).await?`.
  - `serve_runtime`: returns
    `(StatusCode::OK, [(header::CONTENT_TYPE, "application/javascript; charset=utf-8")], crate::runtime::ZERO_RUNTIME)`.
  - `shutdown_signal`: `tokio::signal::ctrl_c().await.ok();`.
- `src/cmd/dev.rs` `pub async fn run() -> anyhow::Result<()>`:
  load config, call `dev::server::serve(config).await`.

**Tests:**
- `tests/dev_serves_runtime.rs` (integration): write a minimal
  `zero.toml` and scaffold (or just the toml — `/zero.js` doesn't
  need disk files) into a tempdir, spawn the binary as a child
  process via `assert_cmd` or `tokio::process::Command`, wait until
  the port is reachable (poll with backoff for a few hundred ms),
  GET `http://127.0.0.1:<port>/zero.js`, assert status 200,
  assert `content-type: application/javascript`, assert headers
  include `cache-control: no-store, no-cache, must-revalidate, max-age=0`,
  assert body equals `ZERO_RUNTIME`. Then SIGTERM the child.
- `tests/dev_port_in_use.rs`: bind a TcpListener to port `0` (get
  an OS-assigned port), then try to start `zero dev` on the same
  port; assert it exits non-zero with the expected error message.

---

### Step 6: `zero dev` — disk file serving

**Goal:** Serve `./<root>/src/**`, `./<root>/styles/**`, `./<root>/public/**`,
and well-known root files (`/favicon.ico`, `/robots.txt`) with
content-type derived from extension and path-traversal protection.

**Files:**
- `src/dev/files.rs` (new)
- `src/dev/server.rs` (extend the `Router` setup)

**Changes:**
- `src/dev/files.rs`:
  - `pub async fn serve_under(root: PathBuf, prefix: &'static str, uri_path: &str) -> Response`:
    - Strip `prefix` from `uri_path`; reject if it doesn't start with `prefix`.
    - Build `candidate = root.join(<stripped>)`.
    - Canonicalize both `root` and `candidate`. If
      `candidate` doesn't `.starts_with(&root_canonical)`, respond
      `403 Forbidden` (defense against path traversal).
    - If file doesn't exist or isn't a file, `404`.
    - Read the file, return as `(StatusCode::OK, [(content_type)], body)`.
  - `fn content_type_for(path: &Path) -> &'static str`: switch on
    extension. `.js` → `application/javascript; charset=utf-8`,
    `.css` → `text/css; charset=utf-8`, `.html` → `text/html; charset=utf-8`,
    `.json` → `application/json`, `.svg` → `image/svg+xml`,
    `.png` → `image/png`, `.jpg`/`.jpeg` → `image/jpeg`,
    `.ico` → `image/x-icon`, default → `application/octet-stream`.
- `src/dev/server.rs`: register routes (axum `.route("/src/*path", get(...))`,
  similarly `/styles/*path`, `/public/*path`, plus `/favicon.ico`
  and `/robots.txt` exact matches). Each handler captures the
  `<root>` path from `Config` and calls `files::serve_under`.
- The `<root>` path passed to handlers is `cwd.join(&config.project.root).canonicalize()?`,
  computed once at server startup. Fail to start if `<root>` doesn't
  exist (clear error: "configured `[project] root = <name>` not found at <path>").

**Tests:**
- `src/dev/files.rs` `#[cfg(test)] mod tests` (unit):
  - `content_type_for` returns expected MIME for each extension.
  - Path traversal: `serve_under("/tmp/x", "/src", "/src/../../../etc/passwd")` returns 403.
- `tests/dev_serves_files.rs` (integration): scaffold a tempdir,
  start dev, GET `/src/app.js`, assert content matches the file on
  disk, assert content-type `application/javascript`, assert no-cache
  headers. GET `/styles/app.css`, same. GET `/src/../../etc/passwd`,
  assert 403. GET `/src/nonexistent.js`, assert 404.

---

### Step 7: `zero dev` — HTML injection (pure function)

**Goal:** A unit-tested pure function that takes HTML bytes and
returns HTML bytes with the dev-mode script tags injected before
`</head>` (with documented fallbacks). Used by Steps 8 and 9.

**Files:**
- `src/dev/inject.rs` (new)

**Changes:**
- `pub const DEV_SCRIPTS: &str = r#"<script type="importmap">{"imports":{"zero":"/zero.js"}}</script>
<script type="module" src="/src/app.js"></script>"#;`
- `pub fn inject(body: &[u8]) -> Vec<u8>`:
  1. Convert to `&str` via `std::str::from_utf8`. If the body isn't
     valid UTF-8, return it unchanged (best-effort: backends emit
     unusual bytes). Log a `warn!`-style line via `eprintln!`.
  2. Find the case-insensitive index of `</head>` (use a manual
     loop or the `regex` crate's case-insensitive flag). Insert
     `DEV_SCRIPTS` immediately before that index.
  3. Fallback A: if no `</head>`, find case-insensitive `<body`
     (any case, partial match — body may have attrs); insert
     `DEV_SCRIPTS` before that index.
  4. Fallback B: if neither marker, prepend `DEV_SCRIPTS` to the
     body and emit a `eprintln!` warning ("zero dev: HTML response
     had no <head> or <body>; scripts prepended").
  5. Return resulting bytes.

**Tests:**
- `src/dev/inject.rs` `#[cfg(test)] mod tests`:
  - Standard `<head>...</head><body>` → inject before `</head>`.
  - Uppercase `</HEAD>` → inject before it (case-insensitive).
  - Missing `</head>`, has `<body class="...">` → inject before
    `<body`.
  - Missing both → inject at start; warning printed (test asserts
    just the result string).
  - Already-injected HTML (re-running inject): tolerate but still
    insert (de-duping is out of scope; document this).
  - Non-UTF-8 input: returns input unchanged.
  - HTML where the literal text `</head>` appears inside a comment
    BEFORE the real `</head>`: documents the known false-positive
    behavior with an explicit test that asserts the wrong-but-deterministic
    behavior. (Acceptable per spec's open question; any fix needs
    a real HTML parser.)

---

### Step 8: `zero dev` — local `index.html` fallback (no-proxy mode)

**Goal:** When `[dev].proxy` is unset, every URL not handled by Steps
5–6 returns the project's `./<root>/index.html` with scripts injected.

**Files:**
- `src/dev/server.rs` (extend with fallback handler)
- `src/dev/local.rs` (new — small helper)

**Changes:**
- `src/dev/local.rs` `pub async fn serve_local_index(root: PathBuf) -> Response`:
  - Read `root/index.html`. If missing, respond `500` with
    "zero dev: <root>/index.html not found". (Should not happen if
    `zero init` ran; defense.)
  - Inject via `inject::inject`.
  - Return `(StatusCode::OK, [(content_type, "text/html; charset=utf-8")], body)`.
- In `src/dev/server.rs`, add a `.fallback(...)` handler that
  branches on `config.dev.proxy`:
  - `None` → `serve_local_index(root)`.
  - `Some(proxy_url)` → call into the proxy handler (Step 9 will
    fill this in; for this step, leave a placeholder
    `(StatusCode::INTERNAL_SERVER_ERROR, "proxy not yet implemented")`).
- The fallback receives the full `Request`, so it has access to
  method / path / etc. for proxy use later.

**Tests:**
- `tests/dev_local_index.rs` (integration): scaffold a tempdir
  (no proxy in toml), start dev, GET `/`, assert HTML body
  contains both `<script type="importmap">{"imports":{"zero":"/zero.js"}}</script>`
  and `<script type="module" src="/src/app.js">`. GET `/anything-else`,
  same body. Verify no-cache headers.

---

### Step 9: `zero dev` — proxy mode

**Goal:** When `[dev].proxy` is set, fallback proxies the request to
the backend, strips its cache headers, applies the dev no-cache
headers, and injects scripts on `text/html` responses. Reject
WebSocket upgrades. Respond `502` on backend connection failure.

**Files:**
- `src/dev/proxy.rs` (new)
- `src/dev/server.rs` (wire fallback to call into proxy handler)

**Changes:**
- `src/dev/proxy.rs`:
  - Hold a `reqwest::Client` in shared `axum::State` (constructed
    once at server start with `redirect::Policy::none()`,
    `gzip(false)`, `brotli(false)`, `timeout(30s)`).
  - `pub async fn proxy_handler(State(client): State<Arc<Client>>, State(config): State<Arc<Config>>, req: Request) -> Response`:
    1. **WebSocket gate.** If the request has `Upgrade: websocket`
       header (case-insensitive), return `(StatusCode::NOT_IMPLEMENTED, "zero dev: WebSocket proxying is out of scope in this slice")`.
    2. Build the upstream URL by joining `config.dev.proxy` with
       the incoming request's path-and-query.
    3. Build the upstream request:
       - Same method.
       - Forward all headers except hop-by-hop (`Connection`,
         `Keep-Alive`, `Proxy-Authenticate`, `Proxy-Authorization`,
         `TE`, `Trailers`, `Transfer-Encoding`, `Upgrade`).
       - Override `Accept-Encoding` to `identity` (so the backend
         returns uncompressed HTML — we'd otherwise need to
         decompress to inject).
       - Forward the request body (use `reqwest::Body::wrap_stream`
         on the request body stream).
    4. Send via `client.execute(req).await`. On error, return
       `(StatusCode::BAD_GATEWAY, [(content_type, "text/html")], format!("<h1>zero dev</h1><p>Cannot reach backend at {}</p>", config.dev.proxy.as_ref().unwrap()))`.
    5. Read the upstream response:
       - Status forwarded as-is.
       - Headers forwarded EXCEPT: `Cache-Control`, `Pragma`,
         `Expires`, `ETag`, `Last-Modified`, `Content-Encoding`,
         hop-by-hop. The dev `no_cache_layer` re-adds the no-cache
         set on the way out.
       - If `Content-Type` starts with `text/html`:
         - Buffer the body (full read; HTML responses are small).
           **Concession:** spec mentions streaming pass-through, but
           injection requires the full body — for HTML, buffering is
           necessary. Other content types stream.
         - Run `inject::inject(&body_bytes)`.
         - Recompute `Content-Length`.
       - Else: stream the body through unchanged. Recompute
         `Content-Length` only if the upstream had one (otherwise
         use chunked).
- `src/dev/server.rs`: replace the proxy placeholder from Step 8
  with a real call into `proxy::proxy_handler`. Inject `Arc<Client>`
  and `Arc<Config>` via axum `State`.

**Tests:**
- `tests/dev_proxy.rs` (integration): use `axum::serve` to start
  a stub backend on a random port that responds:
  - `GET /` → `text/html` body `<html><head><title>X</title></head><body>hi</body></html>`
    plus `Cache-Control: max-age=3600` and `ETag: "abc"`.
  - `GET /api/data` → `application/json` body `{"x":1}`.
  - `GET /slow` → 5s sleep.
  - `GET /upgrade` → echoes `Upgrade` header status.
  Then start `zero dev` with `[dev] proxy = <stub-backend-url>`.
  Assertions:
  - GET `/` returns 200, body contains injected scripts before
    `</head>`, `Cache-Control` header from upstream is replaced
    with `no-store, no-cache, must-revalidate, max-age=0`, `ETag`
    header is absent.
  - GET `/api/data` returns 200, body is unchanged JSON, content-type
    `application/json`.
  - WebSocket upgrade request returns 501 with the expected message.
  - With the stub backend stopped: GET `/anything` returns 502 with
    the expected HTML body.

---

### Step 10: `zero build` — bundler

**Goal:** Walk the user-code module graph from `<root>/src/app.js`,
treat `"zero"` as the embedded runtime, produce a single ES module
output. Hash it, write to `<out>/assets/app.<hash>.js`.

**Files:**
- `src/build/mod.rs` (new)
- `src/build/bundler.rs` (new)
- `src/build/resolver.rs` (new)
- `src/cmd/build.rs` (replace stub)

**Changes:**

The bundler emits a CommonJS-style runtime preamble + per-module
factories. Output shape:

```js
const __zero_modules = {};
const __zero_cache = {};
function __zero_define(id, factory) { __zero_modules[id] = factory; }
function __zero_require(id) {
  if (__zero_cache[id]) return __zero_cache[id];
  const exports = {};
  __zero_cache[id] = exports;
  __zero_modules[id](exports, __zero_require);
  return exports;
}

__zero_define('zero', function (exports, __zero_require) {
  /* ---- ZERO_RUNTIME body, with `export { ... }` rewritten
     to assignments onto `exports` ---- */
});

__zero_define('./src/routes/home.js', function (exports, __zero_require) {
  const { html } = __zero_require('zero');
  exports.default = function Home() { return html`<h1>...</h1>`; };
});

__zero_define('./src/app.js', function (exports, __zero_require) {
  const { App } = __zero_require('zero');
  const Home = __zero_require('./src/routes/home.js').default;
  const app = new App();
  app.route('/', Home);
  app.run('#app');
});

__zero_require('./src/app.js');
```

**`src/build/resolver.rs`:**
- `pub fn resolve(specifier: &str, importer_dir: &Path, root: &Path) -> anyhow::Result<ModuleId>`:
  - If `specifier == "zero"`, return `ModuleId::Runtime`.
  - Else if specifier starts with `./` or `../`: join with
    `importer_dir`, normalize, ensure resulting path stays under
    `root` (forbid escape), check the file exists. Return
    `ModuleId::User(canonical_path_relative_to_root)`.
  - Else error: "unsupported import specifier '{}'; expected 'zero' or a relative path".
- `pub enum ModuleId { Runtime, User(PathBuf) }`. Implements `Hash`,
  `Eq` so it works as a `HashMap` key.

**`src/build/bundler.rs`:**
- `pub fn bundle(config: &Config) -> anyhow::Result<String>`:
  - Module graph walk (BFS/DFS) starting from
    `<root>/src/app.js` resolved as a `ModuleId::User`.
  - For each module, parse imports with regex (same import shapes
    as the runtime concat — `import { x, y } from "..."` and
    `import Default from "..."`):
    - `import { a, b as c } from "..."` →
      `const { a, b: c } = __zero_require("...");`
    - `import Default from "..."` →
      `const Default = __zero_require("...").default;`
    - `import * as Ns from "..."` →
      `const Ns = __zero_require("...");`
    - `import "..."` (side-effect only) →
      `__zero_require("...");`
  - Rewrite exports:
    - `export default <expr>;` → `exports.default = <expr>;`
    - `export default function Foo() {...}` →
      `function Foo() {...} exports.default = Foo;`
    - `export function foo() {...}` → `function foo() {...} exports.foo = foo;`
    - `export const foo = ...` → `const foo = ...; exports.foo = foo;`
    - `export { a, b as c };` →
      `exports.a = a; exports.c = b;`
  - For `ModuleId::Runtime`, take `crate::runtime::ZERO_RUNTIME`,
    apply the same export rewriting (the runtime's final
    `export { ... }` block was already a re-export — convert to
    `exports.foo = foo;` lines).
  - Also need to STRIP the runtime's existing `export { ... }`
    aggregate so it doesn't get re-rewritten incorrectly. Better:
    have the build.rs (Step 3) emit two strings: the cleaned body
    and the export name list. Expose both via `runtime.rs`:
    ```rust
    pub const ZERO_RUNTIME_BODY: &str = ...;        // no exports
    pub const ZERO_RUNTIME_EXPORTS: &[&str] = &[...]; // ["signal", "computed", ...]
    ```
    `bundler.rs` consumes both. The dev path (`/zero.js`) uses
    `ZERO_RUNTIME` (a third constant: body + final `export {...}`),
    or computes it at startup by appending the export list to
    the body. Pick the second; one canonical body, one canonical
    export list, derive everything from those.
  - **Update Step 3:** the build.rs must emit
    `ZERO_RUNTIME_BODY` (cleaned, no exports) and
    `ZERO_RUNTIME_EXPORTS` (list of public names).
    `runtime.rs` exposes both, and adds:
    ```rust
    pub fn runtime_module() -> String {
        let mut s = String::from(ZERO_RUNTIME_BODY);
        s.push_str("\nexport { ");
        s.push_str(&ZERO_RUNTIME_EXPORTS.join(", "));
        s.push_str(" };\n");
        s
    }
    ```
    Step 5's `/zero.js` handler returns `runtime_module()` (compute
    once at server start, store in `Arc<String>`).
- Concatenate all module factories in topological order (reverse
  post-order of the import graph — leaves first), then the
  bootstrap `__zero_require('./src/app.js');` line.
- Module IDs in the output keep the source-relative path string
  (e.g., `'./src/app.js'`) so a developer reading the bundle can
  trace what came from where.

**`src/cmd/build.rs`:**
- Load config. Compute `<root>` and `<out>` paths.
- If `<out>` exists, leave it alone (don't pre-clean — additive).
  Create `<out>/assets/`.
- Call `bundle(&config)?` → `String`.
- Compute SHA-256 of the bundle string; first 16 hex chars (8 bytes
  worth — collision-safe enough for asset hashing). `let hash = &hex_digest[..16];`. Wait — spec says first 8 chars. Use **8 hex chars** (4 bytes / 32 bits) per spec; matches Vite's typical 8-char hash.
- Write to `<out>/assets/app.<hash>.js`.
- Stash `("app.js", "assets/app.<hash>.js")` for the manifest (Step 11).

**Tests:**
- `src/build/resolver.rs` unit tests: `"zero"` → Runtime; relative
  → User with normalized path; absolute / bare specifier rejected;
  `"../../../etc"` rejected.
- `src/build/bundler.rs` unit tests: feed a tiny synthetic module
  graph via a trait that abstracts file reading; assert output
  contains expected `__zero_define` calls in topological order;
  assert imports rewritten correctly; assert exports rewritten
  correctly.
- `tests/build_smoke.rs` (integration): scaffold a tempdir, run
  `zero build`, assert `<out>/assets/app.<hash>.js` exists and is
  non-empty. Spawn `node --input-type=module --eval "global.document = { createElement: () => ({...}) }; const x = await import('file://.../app.<hash>.js'); console.log('ok');"` — but the bundle expects to mutate the DOM, which would fail in pure Node. Instead, just `node --check <bundle>` to verify it's syntactically valid (the bundle output is **non-module** since it uses CommonJS-style wrappers internally; check with `node --check` directly without `--input-type=module`).
  - Also assert the bundle string contains `function signal(`,
    `class App`, the user's `Home` function name.
  - Assert the bundle does NOT contain a top-level `import ` or
    `export ` keyword (all rewritten to runtime calls).

---

### Step 11: `zero build` — CSS hashing, `manifest.json`, static `index.html`

**Goal:** Round out the build output: hashed CSS files, manifest,
and a deploy-ready `<out>/index.html`.

**Files:**
- `src/build/css.rs` (new)
- `src/build/manifest.rs` (new)
- `src/build/index_html.rs` (new)
- `src/cmd/build.rs` (extend with the post-bundle steps)

**Changes:**
- `src/build/css.rs`:
  - `pub fn process_css(root: &Path, out: &Path) -> anyhow::Result<Vec<(String, String)>>`:
    - Walk `root/styles/*.css` (non-recursive — the scaffold is flat;
      recursion is a deferred enhancement).
    - For each file: read, hash (first 8 hex chars of sha256),
      compute output filename `<basename>.<hash>.css`, write to
      `out/assets/<output_filename>`.
    - Return `Vec<(source_relative, output_relative)>` like
      `("styles/app.css", "assets/app.5e8d9f01.css")`.
- `src/build/manifest.rs`:
  - `pub struct Manifest(BTreeMap<String, String>);` (BTreeMap for
    deterministic key order).
  - `pub fn write(out: &Path, entries: &[(String, String)]) -> anyhow::Result<()>`:
    serialize to JSON (`serde_json::to_string_pretty`), write to
    `out/manifest.json`. Always include `"app.js"` key first; CSS
    keys after.
- `src/build/index_html.rs`:
  - `pub fn render(root: &Path, out: &Path, manifest: &[(String, String)]) -> anyhow::Result<()>`:
    - Read `root/index.html`.
    - Build the script + link tags from manifest entries:
      `<script type="module" src="/<output_relative>"></script>` for
      `app.js`, `<link rel="stylesheet" href="/<output_relative>">`
      for each `.css`.
    - Inject before `</head>` (reuse `dev::inject::inject` style
      logic, but with the production tags — extract a shared helper
      `inject_before_head_close(html: &str, snippet: &str) -> String`).
    - Write to `out/index.html`.
- `src/cmd/build.rs`: after bundle (Step 10), call
  `process_css → manifest entries → manifest::write → index_html::render`,
  print summary (files written, byte sizes).

**Tests:**
- `src/build/css.rs` unit (with tempdir): two CSS files in `styles/`,
  process_css emits two hashed copies, manifest pairs are correct.
- `src/build/manifest.rs` unit: deterministic key order; round-trip
  parse equals input.
- `src/build/index_html.rs` unit: input HTML with `<head>` →
  output has `<script type="module" src="/assets/...">` and
  `<link rel="stylesheet" href="/assets/...">` before `</head>`,
  no import map (production mode).
- `tests/build_full.rs` (integration): scaffold tempdir, run
  `zero build`, assert all of: `<out>/assets/app.<hash>.js`,
  `<out>/assets/app.<hash>.css`, `<out>/manifest.json` parses to
  expected JSON, `<out>/index.html` contains script+link tags
  pointing at the hashed filenames.

---

### Step 12: End-to-end integration tests

**Goal:** Glue tests that exercise the full developer flow exactly
as a real user would. Most behavior is covered by per-step
integration tests; this step adds whole-flow smoke tests and
captures regressions across step boundaries.

**Files:**
- `tests/e2e_init_dev.rs` (new)
- `tests/e2e_init_build_node_eval.rs` (new, gated on `node` presence)

**Changes:**
- `e2e_init_dev.rs`:
  1. Tempdir.
  2. Write a `zero.toml` with `[project] root = "web"` (skips the
     prompt branch).
  3. Spawn `zero init` from the tempdir; assert success.
  4. Assert `web/index.html`, `web/src/app.js`, `web/src/routes/home.js`,
     `web/styles/app.css` exist.
  5. Spawn `zero dev` from the tempdir on a free port. Wait for it
     to listen.
  6. GET `/`, assert HTML contains both injected script tags
     and references the scaffold's `<title>`.
  7. GET `/zero.js`, assert it returns the runtime (length > 1000
     bytes, contains `function signal(`).
  8. GET `/src/app.js`, assert it equals the file on disk.
  9. GET `/styles/app.css`, assert content-type `text/css` and
     no-cache headers.
  10. SIGTERM the dev server.
- `e2e_init_build_node_eval.rs`:
  1. Same scaffold setup.
  2. Spawn `zero build`.
  3. Assert all output files exist.
  4. If `node` is on PATH: `node --check <out>/assets/app.<hash>.js`
     succeeds (syntactic validity of the bundle).
  5. If `node` is on PATH: parse `manifest.json` and assert `app.js`
     and `styles/app.css` keys map to existing files in `<out>`.
  6. Assert `<out>/index.html` parses (basic substring checks)
     and references the hashed filenames.
- Skip Node-dependent assertions if `which::which("node").is_err()`
  (use a `node_available()` helper); test still asserts the
  Rust-side outputs.

---

## Risks and Assumptions

- **Hand-rolled bundler edge cases.** The regex-driven import/export
  rewriting handles every shape used in the scaffold and the runtime,
  but real user code can throw curveballs: multi-line imports,
  `import { a as b }` aliases on default-imported names, dynamic
  imports (`import("...")`), `export *` re-exports. Mitigation: the
  Step 10 unit tests cover the supported shapes explicitly; the spec
  scopes user code to the scaffold for this slice. If a real
  developer hits an unsupported shape, the error message should
  point at the specific line; failing fast beats silent wrong output.
  Upgrade path is `oxc_parser`-driven AST rewriting in a follow-up.

- **Runtime cycle between `app.js` and `router.js`.** Current source
  has `app.js` importing `_matchRoutes` (and others) from
  `router.js`, while `router.js` imports `_getCurrentApp` from
  `app.js`. After concat, both are top-level function declarations
  in the same scope and the cycle vanishes. Build-time test in
  Step 3 (parsing the concat with Node, gated on Node presence)
  catches any silent breakage.

- **HTML injection is best-effort.** The find-`</head>`-and-insert
  approach has known false-positive cases (literal `</head>` inside
  a comment before the real one). Spec accepts this. If real backends
  start hitting it in practice, switch to `html5ever` in a follow-up.

- **`reqwest` buffering of HTML responses.** For HTML proxying we
  buffer the full body to inject scripts — fine for typical HTML
  (kilobytes), but a backend that streams a multi-megabyte HTML
  response over slow chunks would block the page. Acceptable for
  dev. Non-HTML responses stream.

- **No tests for the interactive prompt branch of `zero init`.**
  The toml-writer is unit-tested; the existing-toml branch is
  integration-tested. The wizard itself is exercised by manual
  testing only in this slice. Risk is low (four `Input` calls with
  validators); a PTY-based test can be added later if a regression
  bites.

- **`oxc` / `swc_bundler` revisit.** If the hand-rolled bundler
  accumulates too many supported shapes (i.e., we keep reaching
  for it), bite the bullet and adopt an AST-based bundler. The
  CommonJS-style preamble we emit can stay; only the
  parsing/rewriting changes.

- **Crate version ranges.** Pinning to majors (`axum = "0.7"`)
  assumes those families are still current. If a dep emits a
  semver-major and we can't upgrade smoothly during execute, pin
  to the older version explicitly. The dep set was chosen for
  long-term stability (tokio/hyper/axum/reqwest are the bedrock
  stack).

- **`zero init` prompt validation.** Inline re-prompt requires
  `dialoguer`'s `validate_with` closure; if it doesn't expose
  custom error messages cleanly, fall back to "validate after,
  print error, re-call `prompt_user` from scratch." Not a blocker.

- **Path canonicalization on Windows.** `<root>` and `<out>`
  validation uses `Path::components` (cross-platform), but
  canonicalization in the file-serving handler uses
  `fs::canonicalize` which on Windows produces UNC paths
  (`\\?\C:\...`). Cross-platform support isn't promised by this
  slice; `cargo test` on Linux is the bar. Note the limitation in
  a future cross-platform spec.
