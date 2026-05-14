# Plan: TypeScript support

## Summary

Add first-class TypeScript support to the `zero` CLI without introducing any npm
dependency. The transpiler is `swc` pulled in as a Rust crate, invoked per file
to strip types and emit JS (plus optional source maps). `.ts` becomes the
canonical authoring path in the scaffold while `.js` continues to work
everywhere. The dev server transpiles `/src/**/*.ts` on the fly with inline
source maps, the bundler walks `.ts` and `.js` modules via the same
post-transpile JS rewriter it uses today, and the boa-based test runner
transpiles `.ts` files before parsing them as ES modules. Hand-written `.d.ts`
files for `"zero"` and `"zero/test"` are embedded into the CLI binary at
compile time and written into user projects on `zero init` / `zero dev`.

## Prerequisites

Open Questions are resolved up-front for this plan (decisions baked in below):

- **sourcemap toggle shape:** two independent toggles — `[dev] sourcemap`
  (default `true`) and `[build] sourcemap` (default `false`).
- **dev-script injection:** the injector probes for `<root>/src/app.ts` and
  picks the `.ts` script tag when present; falls back to `app.js`.
- **type-declaration layout:** two files — `<root>/zero.d.ts` and
  `<root>/zero-test.d.ts`, each a single `declare module "..."` block. Linked
  to tsconfig via `include`, no `paths` needed.
- **tsconfig contents:** see Step 10 below.
- **AGENTS.md scope:** minimal additions — keep the document's structure and
  example shapes, but flip Quick start, Imports, project layout, and the
  scaffold reference to `.ts`. A one-paragraph note documents that JS still
  works.
- **`extract_imports` on TS:** transpile first, then run the existing JS
  regex; never run the regex on raw TS source.
- **entry collision:** hard error when both `src/app.ts` and `src/app.js`
  exist.
- **`zero dev` startup `.d.ts` refresh:** unconditional overwrite. The files
  are documented as auto-managed.

No other issues block this work.

## Steps

- [x] **Step 1: swc transpile module**
- [x] **Step 2: `zero.toml` sourcemap configuration keys**
- [x] **Step 3: embedded `.d.ts` declarations**
- [x] **Step 4: resolver + bundler accept `.ts`**
- [x] **Step 5: `zero build --sourcemap` flag and `.map` emission**
- [x] **Step 6: dev server transpiles `/src/*.ts`**
- [x] **Step 7: dev-script injector probes for `.ts` entry**
- [x] **Step 8: `zero dev` startup writes `.d.ts` into project root**
- [x] **Step 9: test runner discovery and loader for `.ts`**
- [x] **Step 10: scaffold flips to TS, ships `tsconfig.json` + `.d.ts`**
- [x] **Step 11: end-to-end integration coverage and docs alignment**

---

## Step Details

### Step 1: swc transpile module

**Goal:** Land the transpiler primitive used by every other step. After this
step, the rest of the codebase is unchanged but `zero::transpile` exists.

**Files:**
- `Cargo.toml` — add `swc_core` with the features needed for parse + TS strip
  + codegen + source maps.
- `src/transpile.rs` (new) — the transpile function.
- `src/lib.rs` — declare `pub mod transpile;`.

**Changes:**

1. `Cargo.toml`: add `swc_core = { version = "...", features = [
   "ecma_parser", "ecma_parser_typescript", "ecma_codegen",
   "ecma_transforms_typescript", "ecma_visit", "ecma_ast", "common",
   "common_sourcemap" ] }`. Use whatever the latest 0.x is; pin the exact
   minor in the Cargo.lock that ends up checked in.
2. `src/transpile.rs`:
   ```rust
   pub struct TranspileOptions<'a> {
       /// Logical filename used for diagnostics and source-map source paths.
       pub filename: &'a str,
       /// Emit an inline `//# sourceMappingURL=data:...` at the end of the JS.
       pub inline_source_map: bool,
       /// If `true`, also return the raw source-map JSON string.
       pub emit_source_map: bool,
   }

   pub struct TranspileOutput {
       pub code: String,
       /// Present only when `opts.emit_source_map == true`. JSON text.
       pub source_map: Option<String>,
   }

   #[derive(Debug)]
   pub struct TranspileError {
       pub file: String,
       pub line: u32,
       pub column: u32,
       pub message: String,
   }

   impl std::error::Error for TranspileError {}
   impl std::fmt::Display for TranspileError { /* "file:line:col: message" */ }

   pub fn transpile_typescript(
       source: &str,
       opts: &TranspileOptions<'_>,
   ) -> Result<TranspileOutput, TranspileError>;
   ```
   Internally:
   - Build a `SourceMap`, register `source` under `opts.filename`.
   - Parse with `Parser::new_from(Lexer::new(Syntax::Typescript(TsConfig {
     decorators: false, tsx: false, ..Default::default() }), ...))`. JSX off,
     decorators off — accidental decorator syntax produces a parse error.
   - Apply `swc_ecma_transforms_typescript::strip` (or `strip_with_config`
     with `import_not_used_as_values: Preserve`) and any helper passes
     needed (e.g. `resolver` + `hygiene` only if required by `strip`).
   - Emit with `swc_ecma_codegen::Emitter` against a `JsWriter`, capturing
     source-map mappings.
   - When `inline_source_map` is set: serialize the source map to JSON,
     base64-encode, append `\n//# sourceMappingURL=data:application/json;base64,<...>`.
   - When `emit_source_map` is set (independent of inline): return the
     serialized map alongside the code.
   - Errors from the parser or transformer are converted to
     `TranspileError { file, line, column, message }` using the SourceMap's
     `lookup_char_pos` for the first error span.
3. `src/lib.rs`: add `pub mod transpile;`.

**Tests** (in `src/transpile.rs`):
- `strips_simple_type_annotations`: input `const x: number = 1;` produces
  `const x = 1;`.
- `strips_interface_and_type_alias`: `interface I {}` and `type T = number;`
  vanish from output.
- `default_export_function_is_preserved`: `export default function f() {}`
  passes through.
- `inline_source_map_appended_when_requested`: output ends with
  `//# sourceMappingURL=data:application/json;base64,`.
- `external_source_map_returned_when_requested`: `output.source_map` is
  `Some` and parses as JSON containing `"version":3`.
- `parse_error_returns_structured_error`: input with bad syntax returns
  `TranspileError` with non-zero line/column, message includes the parse
  diagnostic.
- `decorator_syntax_is_a_parse_error`: input `@foo class C {}` returns a
  `TranspileError` (decorators disabled in v1).

---

### Step 2: `zero.toml` sourcemap configuration keys

**Goal:** Carry the sourcemap defaults through `Config` so steps 5 and 6 can
read them. No behavior change yet — the keys are accepted and validated.

**Files:**
- `src/config.rs`

**Changes:**

1. Add a `sourcemap: bool` field to `DevConfig` and `BuildConfig`.
2. Add `sourcemap: Option<bool>` to `RawDev` and `RawBuild`.
3. Defaults: `dev.sourcemap` = `true`, `build.sourcemap` = `false`. Apply in
   `Config::from_toml_str`.
4. No other validation needed — `serde(deny_unknown_fields)` still rejects
   unknown keys.

**Tests** (in `src/config.rs`):
- `defaults_sourcemap_dev_true_build_false`: a minimal `[project]`-only TOML
  yields `dev.sourcemap == true` and `build.sourcemap == false`.
- `explicit_dev_sourcemap_false_is_honored`: `[dev] sourcemap = false`
  parses, `dev.sourcemap == false`.
- `explicit_build_sourcemap_true_is_honored`: `[build] sourcemap = true`
  parses, `build.sourcemap == true`.
- `non_boolean_sourcemap_is_rejected`: a string or number value produces a
  parse error.

---

### Step 3: embedded `.d.ts` declarations

**Goal:** Author the type surface for `"zero"` and `"zero/test"` once and
embed both files into the CLI binary.

**Files:**
- `runtime/zero.d.ts` (new) — hand-written `declare module "zero" { ... }`.
- `runtime/zero-test.d.ts` (new) — hand-written `declare module "zero/test"
  { ... }`.
- `build.rs` — read both, copy verbatim to `OUT_DIR/zero_types_body.d.ts` and
  `OUT_DIR/zero_test_types_body.d.ts`, and emit `cargo:rerun-if-changed`
  directives.
- `src/runtime.rs` — expose `pub const ZERO_TYPES_BODY: &str =
  include_str!(...)` and `pub const ZERO_TEST_TYPES_BODY: &str =
  include_str!(...)`.

**Changes:**

1. `runtime/zero.d.ts`: a single `declare module "zero" { ... }` block
   covering every public name in `ZERO_RUNTIME_EXPORTS` (i.e. excluding the
   three `_`-prefixed internals). Specifically:
   ```ts
   declare module "zero" {
     export interface Signal<T> {
       readonly val: T;
       set(value: T): void;
       update(fn: (current: T) => T): void;
     }
     export interface Computed<T> { readonly val: T; }
     export interface Ref<T = any> { el: T | null; }
     export interface TemplateResult { /* opaque */ }
     export interface RouteView { path: string; params: Record<string, string>; query: Record<string, string>; }

     export function signal<T>(initial: T): Signal<T>;
     export function computed<T>(fn: () => T): Computed<T>;
     export function effect(fn: () => void | (() => void)): () => void;
     export function inject<T = unknown>(key: string): T;

     export function html(strings: TemplateStringsArray, ...values: unknown[]): TemplateResult;
     export function each<T>(source: Signal<T[]> | Computed<T[]>, render: (item: T, index: number) => TemplateResult): TemplateResult;
     export function ref<T = any>(): Ref<T>;

     export function navigate(to: string, opts?: { replace?: boolean; state?: unknown }): void;
     export function back(): void;
     export function forward(): void;
     export function route(): RouteView;

     export class App {
       constructor();
       state(key: string, value: unknown): this;
       use(mw: (ctx: { route: RouteView; state: Record<string, unknown>; redirect: (path: string) => void }) => void | Promise<void>): this;
       route(pattern: string, loaderOrComponent: unknown, opts?: Record<string, unknown>): this;
       layout(component: (props: any) => TemplateResult): this;
       loading(component: () => TemplateResult): this;
       error(component: (props: { error: unknown; retry: () => void }) => TemplateResult): this;
       run(selector: string): void;
       match(input: string): { route: unknown; params: Record<string,string>; query: Record<string,string>; pathname: string; search: string } | null;
     }
   }
   ```
   (Field shapes mirror the runtime's actual surface; refine for accuracy
   against `runtime/app.js` and `runtime/router.js` while authoring.)
2. `runtime/zero-test.d.ts`: a `declare module "zero/test" { ... }` block
   covering every public name in `ZERO_TEST_EXPORTS` (excluding the two
   `__getTestTree__` / `__resetTestTree__` `_`-prefixed internals).
   Specifically: `describe`, `it`, `beforeEach`, `afterEach`, `beforeAll`,
   `afterAll`, `expect`, `render`, `find`, `findAll`, `text`, `fire`,
   `cleanup`. `expect` returns a chainable matcher interface (each method
   returns `void`, none return `this`, matching the current runtime).
3. `build.rs`: at the top, alongside the other generated bodies, read both
   `.d.ts` files into `OUT_DIR` outputs and emit `rerun-if-changed`. Reuse
   the existing pattern; don't strip imports/exports because `.d.ts` files
   have neither (declaration-only).
4. `src/runtime.rs`: add the two `pub const` strings via `include_str!`.

**Tests** (in `src/runtime.rs`):
- `zero_types_body_declares_every_public_runtime_export`: iterate over
  `ZERO_RUNTIME_EXPORTS`, skipping names starting with `_`, and assert each
  appears in `ZERO_TYPES_BODY` (substring check). This is the compile-time
  guard the spec calls for in the Constraints section.
- `zero_test_types_body_declares_every_public_test_export`: same for
  `ZERO_TEST_EXPORTS`, skipping `__`-prefixed names.
- `zero_types_body_contains_signal_app_html_route`: spot-check critical
  names so a regression is loud.

---

### Step 4: resolver + bundler accept `.ts`

**Goal:** The bundler can walk a graph that mixes `.ts` and `.js` modules,
with TS files transpiled before the existing CJS rewriter sees them.

**Files:**
- `src/build/resolver.rs`
- `src/build/bundler.rs`

**Changes:**

1. `resolver.rs::resolve`: no signature change. The existing relative-path
   logic already canonicalizes against disk, so it accepts `.ts` specifiers
   for free once `.ts` files exist on disk. Add a test (below) to lock this
   in. Confirm bare specifiers other than `"zero"` are still rejected.
2. `bundler.rs::bundle`: change entry detection.
   ```rust
   let entry_ts = root.join("src").join("app.ts");
   let entry_js = root.join("src").join("app.js");
   let (entry_path, entry_id) = match (entry_ts.exists(), entry_js.exists()) {
       (true, true) => anyhow::bail!("zero build: both src/app.ts and src/app.js exist; remove one"),
       (true, false) => (entry_ts, ModuleId::User(PathBuf::from("./src/app.ts"))),
       (false, true) => (entry_js, ModuleId::User(PathBuf::from("./src/app.js"))),
       (false, false) => anyhow::bail!("zero build: no entry point at src/app.ts or src/app.js"),
   };
   ```
3. `bundler.rs`: introduce a small helper that reads a user-module source
   and, if the path ends with `.ts`, transpiles it via
   `crate::transpile::transpile_typescript` with `inline_source_map: false`
   and `emit_source_map: false`. This is called both in the BFS walk (where
   `extract_imports` runs over the source) and in the second-pass `sources`
   population. Both passes must consume the same transpiled JS — extract
   transpiled text once and reuse.
4. Refactor the walk so each user module is read+transpiled once, cached in
   a `HashMap<ModuleId, String>` keyed by id, and the second `sources` loop
   becomes a clone of that cache. This eliminates the double read+transpile
   that would otherwise happen. (The current code does a double read; this
   step also fixes it.)
5. `extract_imports` and `rewrite_module` remain unchanged — they only ever
   see transpiled JS.

**Tests:**
- `resolver.rs`: `relative_ts_resolves_to_user` — write `src/home.ts`,
  `resolve("./home.ts", src_dir, root)` returns
  `ModuleId::User("./src/home.ts")`.
- `bundler.rs`: `bundle_with_ts_entry_strips_types_and_imports_zero` — fake
  a project with `src/app.ts` containing `import { signal } from "zero";
  const n: number = 1; signal(n);`. The resulting bundle string contains
  `__zero_require('zero')` and does NOT contain `: number`.
- `bundler.rs`: `bundle_errors_when_both_entries_present` — both `app.ts`
  and `app.js` exist, `bundle()` errors with a message mentioning the
  collision.
- `bundler.rs`: `bundle_mixed_ts_and_js_dependencies` — `app.ts` imports
  `./util.js`, `util.js` imports `./inner.ts`. Bundle succeeds; output
  contains `__zero_define('./src/app.ts'` etc. (module-id strings use the
  source extension verbatim).

---

### Step 5: `zero build --sourcemap` flag and `.map` emission

**Goal:** `zero build` produces an external source map when requested by flag
or by `[build] sourcemap = true` in `zero.toml`. The map reflects per-module
transpiled-JS → original-source positions for `.ts` modules, and identity
mappings for `.js` modules.

**Files:**
- `Cargo.toml` — add the `sourcemap` crate (a small, well-known
  source-map-v3 builder; pull-up cost is minor and the alternative is
  hand-rolling VLQ).
- `src/main.rs` — add `--sourcemap` and `--no-sourcemap` to the `Build`
  subcommand (Clap `clap::ArgAction::SetTrue` + a separate flag for
  override).
- `src/cmd/build.rs` — accept the flag, resolve effective value (flag
  overrides config), pass to bundler.
- `src/build/bundler.rs` — when source maps are enabled, emit a combined
  map alongside the bundle.

**Changes:**

1. `Cargo.toml`: add `sourcemap = "9"` (or current major).
2. `src/main.rs`:
   ```rust
   Build {
       #[arg(long, default_value_t = false)]
       sourcemap: bool,
       #[arg(long, default_value_t = false)]
       no_sourcemap: bool,
   }
   ```
   Resolve to `Option<bool>`: both false → `None` (use config default);
   `sourcemap` → `Some(true)`; `no_sourcemap` → `Some(false)`; both →
   error.
3. `src/cmd/build.rs::run` becomes `pub async fn run(sourcemap_override:
   Option<bool>) -> Result<()>`. The effective value is
   `sourcemap_override.unwrap_or(config.build.sourcemap)`.
4. `src/build/bundler.rs`: change `bundle` to
   `bundle(config: &Config, emit_sourcemap: bool) -> Result<BundleOutput>`
   where `BundleOutput { code: String, source_map: Option<String> }`.
   When `emit_sourcemap`:
   - During the walk, for `.ts` user modules, also collect the per-module
     swc source map (call `transpile_typescript` with
     `emit_source_map: true`).
   - For `.js` modules and the runtime body, no per-module map is needed —
     the combined map records identity mappings for their line ranges.
   - Track each module's starting line in the final bundle (count `\n` in
     the prefix + the `__zero_define(...)` wrapper line).
   - Build the final source map with the `sourcemap` crate: use
     `SourceMapBuilder` to emit, for each transpiled module, the mappings
     from its swc map shifted by the module's starting line; for
     non-transpiled modules, add identity mappings line-by-line.
5. `cmd/build.rs`: when `source_map` is `Some`, write
   `app.<hash>.js.map` next to the bundle and append
   `\n//# sourceMappingURL=app.<hash>.js.map\n` to the bundle file before
   writing. The hash is computed over the original bundle code (without
   the sourceMappingURL comment), so the filename is stable.

**Tests:**
- `bundler.rs::bundle_emits_no_source_map_by_default` —
  `bundle(&cfg, false)` returns `source_map == None`.
- `bundler.rs::bundle_emits_source_map_when_requested` — `bundle(&cfg,
  true)` returns a `Some(...)` whose JSON has `"version":3` and
  `"sources"` listing the user modules with their source extensions.
- `tests/build_sourcemap.rs` (new): end-to-end — `zero build --sourcemap`
  on the (post-step-10) TS scaffold produces `dist/assets/app.*.js.map`
  AND the bundle ends with `//# sourceMappingURL=`. Until step 10
  lands, this test can be authored against a TS fixture written inline.

---

### Step 6: dev server transpiles `/src/*.ts`

**Goal:** A `GET /src/x.ts` request returns transpiled JS with an inline
source map (configurable). Errors return HTTP 500 with the error body.

**Files:**
- `src/dev/transpile.rs` (new) — thin wrapper around
  `crate::transpile::transpile_typescript` that returns an axum
  `Response`.
- `src/dev/files.rs` — `serve_under` (or a new helper) detects `.ts` and
  routes to the transpile path.
- `src/dev/server.rs` — thread `sourcemap` flag from `Config` into
  `AppState`, pass through to the handler.

**Changes:**

1. `AppState` gains `dev_sourcemap: bool` (copied from
   `config.dev.sourcemap` at `serve()`).
2. `src/dev/transpile.rs`:
   ```rust
   pub async fn serve_typescript_file(
       abs_path: PathBuf,
       logical_path: String, // e.g. "/src/routes/home.ts"
       inline_source_map: bool,
   ) -> Response;
   ```
   - Reads the file off disk.
   - Calls `transpile_typescript(&source, &TranspileOptions { filename:
     &logical_path, inline_source_map, emit_source_map: false })`.
   - On `Ok`: 200 with `Content-Type: application/javascript;
     charset=utf-8` and the transpiled bytes.
   - On `Err(e)`: 500 with `Content-Type: text/plain; charset=utf-8` and
     body `"zero dev: transpile error\n  {file}:{line}:{col}\n  {message}"`.
3. `src/dev/files.rs`: introduce
   ```rust
   pub async fn serve_under_with_transpile(
       root: PathBuf,
       prefix: &'static str,
       uri_path: &str,
       inline_source_map: bool,
   ) -> Response;
   ```
   which is `serve_under` for non-`.ts` paths and dispatches to
   `serve_typescript_file` when the resolved path ends with `.ts`. Path
   traversal/escape checks stay identical. The original `serve_under`
   stays for `/styles` and `/public`.
4. `src/dev/server.rs`: replace the `/src/*path` handler with one that
   calls `serve_under_with_transpile(s.root.join("src"), "/src",
   &format!("/src/{p}"), s.dev_sourcemap)`. Other routes unchanged.

**Tests:**
- `src/dev/files.rs` (unit): `content_type_for(Path::new("a.ts"))` should
  not be `application/javascript` directly — TS handling is at a higher
  layer. Add a test asserting `.ts` returns `application/octet-stream` so
  the route layer is unambiguously the one responsible for transpiling.
  (Alternative: map `.ts` → JS in `content_type_for` for consistency.
  Picking the route-layer pattern keeps `serve_under` byte-pure.)
- `tests/dev_serves_ts.rs` (new):
  - Spawn `dev::server::serve` against a temp dir with `src/foo.ts`
    containing `const n: number = 1; export { n };`. `GET /src/foo.ts`
    returns 200, `Content-Type` is JS, body contains `const n = 1;`,
    does NOT contain `: number`, and contains
    `//# sourceMappingURL=data:application/json;base64,`.
  - With `[dev] sourcemap = false` in the config, the response omits the
    sourceMappingURL comment.
  - `GET /src/bad.ts` with bad syntax returns 500 and the body mentions
    the line/column.
  - `GET /src/plain.js` continues to return the raw file unchanged.

---

### Step 7: dev-script injector probes for `.ts` entry

**Goal:** The injected `<script type="module">` tag points at whichever
entry point actually exists in the project.

**Files:**
- `src/dev/inject.rs`
- `src/dev/local.rs`

**Changes:**

1. Replace `DEV_SCRIPTS: &str` with a function:
   ```rust
   pub fn dev_scripts(app_entry_href: &str) -> String;
   ```
   that returns the same triple of script tags but with the `app_entry_href`
   substituted into the `<script type="module" src="...">` tag. The
   importmap and reload-EventSource scripts are unchanged.
2. `inject(body, app_entry_href)` (signature gains a parameter). Existing
   callers updated.
3. `src/dev/local.rs::serve_local_index` becomes `serve_local_index(root,
   app_entry_href)`. It probes the root: `if root.join("src/app.ts").is_file()
   { "/src/app.ts" } else { "/src/app.js" }`. (The probe happens once per
   request — acceptable; the file system check is cheap and the dev server
   is single-user.)
4. `src/dev/server.rs`: the fallback handler invokes `serve_local_index`
   with the probed href.

**Tests:**
- `dev/inject.rs`:
  - `dev_scripts_uses_ts_entry_when_provided`: `dev_scripts("/src/app.ts")`
    contains `src="/src/app.ts"` and NOT `src="/src/app.js"`.
  - `dev_scripts_uses_js_entry_when_provided`: `dev_scripts("/src/app.js")`
    contains `src="/src/app.js"`.
  - Existing injection tests get updated to pass an explicit href.
- `tests/dev_local_index.rs` (extend): with `src/app.ts` present in the
  fixture, the served `index.html` references `/src/app.ts`; with only
  `src/app.js`, references `/src/app.js`.

---

### Step 8: `zero dev` startup writes `.d.ts` into project root

**Goal:** Every time `zero dev` starts, `<root>/zero.d.ts` and
`<root>/zero-test.d.ts` are unconditionally overwritten with the embedded
declarations, so a CLI upgrade keeps user types current.

**Files:**
- `src/dev/server.rs` (or a new `src/dev/types_refresh.rs`)
- `src/runtime.rs` — re-export the two consts as a convenience.

**Changes:**

1. Early in `serve()`, after `root` is canonicalized and before binding,
   write the two files:
   ```rust
   std::fs::write(root.join("zero.d.ts"), ZERO_TYPES_BODY)?;
   std::fs::write(root.join("zero-test.d.ts"), ZERO_TEST_TYPES_BODY)?;
   ```
2. If either write fails, log a warning to stderr but continue starting —
   write failures shouldn't break dev. (The bail-on-error path is
   reserved for the `index.html`/`port` problems.)

**Tests:**
- `tests/dev_writes_dts_on_start.rs` (new): start `serve()` in a
  background tokio task against a temp project with `index.html` and a
  basic `src/app.js`. Once the port is reachable, assert that both
  `zero.d.ts` and `zero-test.d.ts` exist under root and contain
  `declare module "zero"` / `declare module "zero/test"` respectively.
  Shutdown the server cleanly via the existing ctrl-c path (or by
  dropping the task; the test only needs file presence).

---

### Step 9: test runner discovery and loader for `.ts`

**Goal:** `zero test` discovers and runs `*.test.ts` / `*.spec.ts` files,
and TS-imported modules transpile before reaching boa.

**Files:**
- `src/test_runner/discovery.rs`
- `src/test_runner/loader.rs`

**Changes:**

1. `discovery.rs::is_test_file`: extend to
   ```rust
   name.ends_with(".test.js") || name.ends_with(".spec.js")
       || name.ends_with(".test.ts") || name.ends_with(".spec.ts")
   ```
2. `discovery.rs`: after `files.sort()` and before applying the substring
   filter, scan for collisions: for each `.test.ts`/`.spec.ts` path,
   compute the corresponding `.js` sibling (same dir, same stem, same
   `.test`/`.spec` infix) and bail if present:
   `anyhow::bail!("zero test: {} and {} both exist; remove one", ts_path,
   js_path)`.
3. `loader.rs::resolve_relative`: after computing `canonical`, if the
   canonical path ends with `.ts`, read the source and pass it through
   `crate::transpile::transpile_typescript` with `inline_source_map:
   false, emit_source_map: false, filename: logical-path`. The
   transpiled JS replaces `src` before the `Module::parse` call. Errors
   propagate as `JsError`s with a message that includes the
   `file:line:col`.
4. `harness.rs::run_file`: the entry file's source is also read by the
   harness directly (not through the loader). Add the same TS-detection
   logic: if `file_abs` ends with `.ts`, transpile the source before
   `Module::parse(Source::from_bytes(...))`. The `with_path(file_abs)`
   call stays so stack traces still reference the `.ts` file (a v1
   known limitation: line numbers refer to the stripped JS, not the
   original `.ts`).

**Tests:**
- `discovery.rs`:
  - `collects_test_ts_and_spec_ts`: write `a.test.ts`, `b.spec.ts`,
    `c.test.js`; discovery returns all three.
  - `collision_ts_and_js_for_same_logical_name_errors`: write both
    `home.test.ts` and `home.test.js`; `discover()` errors with both
    paths in the message.
- `loader.rs`:
  - `resolves_relative_ts_file`: write `foo.ts` with `export const x:
    number = 42;`. Entry module does `import { x } from './foo.ts';
    if (x !== 42) throw 'no';`. Promise fulfills.
  - `parse_error_in_ts_dependency_surfaces`: `foo.ts` contains a
    syntax error. Entry's `load_link_evaluate` rejects; the reason
    string mentions the line/column.
- `harness.rs::run_file_handles_ts_entry`: a `.test.ts` file with one
  passing `it()` runs and produces a `Passed` outcome.

---

### Step 10: scaffold flips to TS, ships `tsconfig.json` + `.d.ts`

**Goal:** New projects scaffolded by `zero init` are TypeScript by default.
JS-only existing projects continue to work end-to-end (covered by Step 11's
regression test).

**Files (deletions and additions in `src/scaffold/`):**
- Delete: `src/scaffold/src/app.js`, `src/scaffold/src/routes/home.js`,
  `src/scaffold/src/routes/home.test.js`.
- Add: `src/scaffold/src/app.ts`, `src/scaffold/src/routes/home.ts`,
  `src/scaffold/src/routes/home.test.ts`, `src/scaffold/tsconfig.json`.
- Modify: `src/scaffold.rs`, `src/scaffold/AGENTS.md`,
  `src/scaffold/index.html`.

**Changes:**

1. `src/scaffold/src/app.ts`:
   ```ts
   import { App, signal } from "zero";
   import Home from "./routes/home.ts";

   const app = new App();
   app.state("count", signal(0));
   app.route("/", Home);
   app.run("#app");
   ```
2. `src/scaffold/src/routes/home.ts`:
   ```ts
   import { html, inject, type Signal, type TemplateResult } from "zero";

   function Counter(): TemplateResult {
     return html`<p>Count: ${() => inject<Signal<number>>("count").val}</p>`;
   }

   export default function Home(): TemplateResult {
     return html`
       <h1>Hello from zero</h1>
       <button @click=${() => inject<Signal<number>>("count").update(n => n + 1)}>Increment</button>
       ${Counter()}
     `;
   }
   ```
3. `src/scaffold/src/routes/home.test.ts`:
   ```ts
   import { describe, it, expect, afterEach } from "zero/test";
   import { render, find, text, fire, cleanup } from "zero/test";
   import { signal } from "zero";
   import Home from "./home.ts";

   describe("Home", () => {
     afterEach(cleanup);

     it("renders the initial count", () => {
       const el = render(Home(), { state: { count: signal(0) } });
       expect(text(el, "p")).toBe("Count: 0");
     });

     it("increments the count when the button is clicked", () => {
       const count = signal(0);
       const el = render(Home(), { state: { count } });
       fire(find(el, "button"), "click");
       expect(text(el, "p")).toBe("Count: 1");
       fire(find(el, "button"), "click");
       expect(text(el, "p")).toBe("Count: 2");
       expect(count.val).toBe(2);
     });
   });
   ```
4. `src/scaffold/tsconfig.json`:
   ```jsonc
   // tsconfig.json — generated by `zero init`. Editor use only — `zero` ignores this file.
   {
     "compilerOptions": {
       "strict": true,
       "target": "ESNext",
       "module": "ESNext",
       "moduleResolution": "bundler",
       "allowImportingTsExtensions": true,
       "noEmit": true,
       "skipLibCheck": true
     },
     "include": ["src", "zero.d.ts", "zero-test.d.ts"]
   }
   ```
5. `src/scaffold/index.html`: unchanged — the dev-server injector (Step 7)
   probes the project and injects the `.ts` script tag. The static build
   (Step 4/5) emits its own hashed `.js` script tag in `dist/index.html`.
6. `src/scaffold.rs`:
   - Replace the `TPL_APP_JS` / `TPL_HOME_JS` / `TPL_HOME_TEST_JS` consts
     with `TPL_APP_TS`, `TPL_HOME_TS`, `TPL_HOME_TEST_TS`.
   - Add `TPL_TSCONFIG_JSON: &str = include_str!("scaffold/tsconfig.json")`.
   - In `write_to`: write `src/app.ts`, `src/routes/home.ts`,
     `src/routes/home.test.ts`, `tsconfig.json` at root, `zero.d.ts` at
     root (`crate::runtime::ZERO_TYPES_BODY`), `zero-test.d.ts` at root
     (`crate::runtime::ZERO_TEST_TYPES_BODY`). Keep `index.html`,
     `styles/app.css`, `AGENTS.md` as before.
7. `src/scaffold/AGENTS.md` (minimal-additions scope):
   - Update the project-layout block: `src/app.ts`,
     `src/routes/home.ts`, `src/routes/home.test.ts`, plus a
     `tsconfig.json` and `zero.d.ts` / `zero-test.d.ts` entry.
   - Update the Imports example block from `js` fence to `ts` fence (the
     code itself is unchanged ES module syntax — TS just adds type
     coverage).
   - Replace JS path mentions (`./routes/home.js`) with `./routes/home.ts`.
   - Add a single short paragraph under Quick start titled "JavaScript
     projects": "Authoring in plain `.js` is still fully supported — both
     extensions work everywhere. The scaffold ships `.ts` because that's
     where the documented examples live; switching the suffix is the only
     change needed to use plain JS."
   - Keep the JSDoc-conventions section; add one sentence that JSDoc is
     still the convention for `.js` files.
   - Do NOT rewrite every prose example — keep the existing structure and
     code; just retype the fences to `ts` where useful.

**Tests:**
- `src/scaffold.rs` tests get updated:
  - `write_to_emits_all_files`: check `src/app.ts`, `src/routes/home.ts`,
    `src/routes/home.test.ts`, `tsconfig.json`, `zero.d.ts`,
    `zero-test.d.ts` all exist with non-empty contents.
  - `write_to_app_ts_imports_zero`: `app.ts` contains
    `import { App, signal } from "zero"`.
  - `write_to_emits_tsconfig`: parsed JSON has `strict: true` and
    `allowImportingTsExtensions: true`.
  - `write_to_emits_zero_d_ts`: `zero.d.ts` contents start with
    `declare module "zero"`.
  - `AGENTS.md` sentinel test gets the section list adjusted if any
    section heading changed (probably none needed).

---

### Step 11: end-to-end integration coverage and docs alignment

**Goal:** Update every test that materializes the scaffold or asserts on
file paths. Add one TS-specific E2E test and one JS-regression E2E test.

**Files:**
- `tests/build_full.rs` — `app.<hash>.js` still expected; the test mostly
  just runs `init` + `build` against the new scaffold and asserts the
  manifest. It should pass with no changes except the `web/src/...` paths
  (none referenced today; check).
- `tests/e2e_init_test.rs` — the `failing_test_produces_nonzero_exit_*`
  test reads `web/src/routes/home.test.js`; change to `home.test.ts`.
- `tests/e2e_init_dev.rs` — adjust any path assertions to expect the new
  `.ts` files.
- `tests/e2e_init_build_node_eval.rs` — same path adjustments.
- `tests/cli_skeleton.rs` — no change expected; verify.
- `tests/runtime_evaluates.rs` — no change expected.
- `tests/e2e_init_typescript.rs` (new) — full TS round trip:
  `init` → file presence checks → `test` (should pass 2 tests) →
  `build --sourcemap` → assert `app.*.js.map` exists.
- `tests/e2e_init_js_project.rs` (new) — explicit JS-regression: scaffold,
  then rename `app.ts`→`app.js`, `home.ts`→`home.js`,
  `home.test.ts`→`home.test.js`, delete `tsconfig.json` (optional),
  rewrite imports from `./routes/home.ts` → `./routes/home.js`, then run
  `zero test` and `zero build`. Both succeed.

**Changes:**

- Update path assertions in existing E2E tests.
- Add the two new tests.
- README/CLAUDE.md additions: a single short paragraph in `CLAUDE.md`
  noting that `.ts` is the canonical authoring extension and that the
  test command is unchanged.

**Tests:** This step is itself the tests.

---

## Risks and Assumptions

- **swc build-time cost.** Adding `swc_core` will significantly lengthen
  clean builds and inflate the binary. This is accepted by the spec, but
  worth tracking — if the binary grows past a comfort threshold, prune
  the swc feature set (e.g. drop `ecma_visit` if unused).
- **swc API surface drift.** The exact import paths, feature names, and
  pass composition shown above (`strip`, `resolver`, `hygiene`, etc.) need
  to be verified against the pinned `swc_core` version at implementation
  time. The plan locks the *shape* of the wrapper, not the internal calls.
- **Source-map combining.** Step 5 leans on the `sourcemap` crate to merge
  per-module swc maps into a bundle map. If the merge logic turns out
  finicky for `.js` modules (identity mappings) or for the runtime body's
  line offsets, fall back to: emit a coarse map that records each module's
  starting line only. The spec doesn't require column-accurate mappings.
- **Decorator-disabled error UX.** Disabling decorators in swc means
  accidentally-used decorator syntax surfaces as a parse error — that's
  the desired outcome, but the error message comes from swc and may not
  say "decorators are disabled". If the wording is confusing, a follow-up
  can wrap the error in a friendlier `TranspileError` message; not in
  v1 scope.
- **Test runner stack traces refer to stripped JS.** Documented v1
  limitation. If user feedback later prioritizes original-TS line numbers
  in test failures, that work is downstream of step 9.
- **`zero dev` overwrites `zero.d.ts` unconditionally.** A user who hand-edits
  these files will lose changes on the next `zero dev`. Accepted; the file
  comment will say "auto-managed". A future "skip if file equals embedded"
  optimization is trivial if complaints arise.
- **Assumption: every public name in `ZERO_RUNTIME_EXPORTS` /
  `ZERO_TEST_EXPORTS` has a stable, documentable type.** If some runtime
  exports have semantics that don't round-trip cleanly to TS types
  (e.g. dynamic argument forms), the `.d.ts` may use `any` or `unknown`
  for those positions. Type quality is a follow-up; coverage (every name
  declared) is the v1 bar.
- **Assumption: the dev server's regex-free `/src/*` route is the right
  insertion point for transpilation.** If a project ever needs to serve
  `.ts` files via a non-`/src` URL prefix, this assumption breaks; not
  in scope.
