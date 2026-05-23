# Plan: Production minification (JS + CSS)

## Summary

Wire production minification into `zero build` end-to-end. JS is minified
via `swc_ecma_minifier` (pulled in by enabling `swc_core`'s `ecma_minifier`
feature — already at v65.1.0 in the workspace lock, no new top-level dep).
CSS is minified by switching `grass` from `OutputStyle::Expanded` to
`OutputStyle::Compressed`. Both are always-on in `zero build`; there is no
flag and no config knob. Source maps continue to be opt-in via
`--sourcemap`, and when requested the JS map composes a line-level
bundle→original map with the minifier's minified→bundle map to produce a
single minified→original map. The implementation lands in eight ordered
steps, each leaving the workspace compilable and `cargo test --workspace`
green.

## Prerequisites

None. The spec's Open Questions are all plan-internal judgment calls; this
plan resolves each in the relevant step:

- **swc_core 65's `ecma_minifier` feature** is confirmed present (Step 1).
- **Plain `.css` handling** is resolved: route plain `.css` through grass
  too (Step 6).
- **Source-map pipeline shape** is resolved: two-stage (bundle→original
  composed with minify→bundle), Option B precision — line-level, no per-
  module transpile-map chaining for v1 (Step 2 / Step 4).
- **Reserved-name list** enumerated in Step 3.
- **Showcase shrinkage threshold** stays at the spec's 30% (Step 7).
- **`bundle_unminified` exposure** uses `#[cfg(any(test, feature =
  "test-internals"))]` plus a `test-internals` feature on the crate (Step 5).
- **Legal-comment preservation in grass compressed mode** is verified in
  Step 6 and the test adjusts accordingly.
- **`sourcesContent` in JS map** — left out of v1 (Open question note,
  Step 4).

## Steps

- [x] **Step 1: Enable `ecma_minifier` on the workspace `swc_core` dep**
- [x] **Step 2: Replace the coarse JS sourcemap with a real bundle→original line map**
- [x] **Step 3: Add `crates/zero-bundler/src/minify.rs` with `minify_js`**
- [x] **Step 4: Wire `minify_js` into `bundle()` and update bundler tests**
- [x] **Step 5: Add `bundle_unminified` test-only API**
- [x] **Step 6: Switch CSS to compressed mode and route plain `.css` through grass**
- [x] **Step 7: Add size-budget assertion to `tests/showcase_build.rs` and fix downstream test fallout**
- [x] **Step 8: Update `docs/building-and-deploying.md` and `docs/config-and-cli.md`**

---

## Step Details

### Step 1: Enable `ecma_minifier` on the workspace `swc_core` dep

**Goal:** Make `swc_ecma_minifier` types available to `zero-bundler` without
adding a new top-level dependency line. Verified already present at the
right version: `swc_core 65.1.0` exposes feature `ecma_minifier` that pulls
`swc_ecma_minifier = "52.0.6"` (confirmed by reading
`~/.cargo/registry/src/.../swc_core-65.1.0/Cargo.toml`). Locks the
foundation for Steps 3–4.

**Files:**
- `Cargo.toml` (workspace root)
- `crates/zero-bundler/Cargo.toml`

**Changes:**
1. In root `Cargo.toml`, add `"ecma_minifier"` to the `swc_core` feature
   list under `[workspace.dependencies]`. The list currently is
   `["ecma_parser", "ecma_parser_typescript", "ecma_codegen",
   "ecma_transforms", "ecma_transforms_typescript", "ecma_visit",
   "ecma_ast", "common", "common_sourcemap"]` — append `"ecma_minifier"`.
2. In `crates/zero-bundler/Cargo.toml`, add `swc_core = { workspace = true
   }` to `[dependencies]`. (The crate currently uses swc transitively via
   `zero-transpile`; for direct access to the minifier types it needs the
   dep listed explicitly.)
3. No code changes in this step. `cargo build --workspace` succeeds; the
   minifier crate is downloaded and linked but unused.

**Tests:**
- `cargo build --workspace` succeeds.
- `cargo test --workspace` continues to pass — no behavior change.
- One new sanity test in `crates/zero-bundler/src/lib.rs` (or a small
  doctest) that imports `swc_core::ecma::minifier` to fail-fast if the
  feature flag silently drops in a future swc_core release:
  ```rust
  #[cfg(test)]
  #[test]
  fn swc_minifier_module_is_available() {
      use swc_core::ecma::minifier::option::MinifyOptions;
      let _ = MinifyOptions::default();
  }
  ```

---

### Step 2: Replace the coarse JS sourcemap with a real bundle→original line map

**Goal:** Today's `build_combined_sourcemap` in `bundler.rs` registers
source paths but emits no mappings. Step 4 will compose this map with the
minify map; for that composition to point at real source positions, the
bundle map must carry real line-level mappings. Doing the source-map
upgrade as its own step keeps the diff reviewable and lets the existing
`bundle_emits_source_map_when_requested` test become more rigorous before
minification is layered on.

**Files:**
- `crates/zero-bundler/src/bundler.rs`

**Changes:**
1. Replace `build_combined_sourcemap(&emit_order)` with a function
   `build_bundle_source_map(out: &str, emit_order: &[ModuleId], factory_spans:
   &[(ModuleId, usize, usize)]) -> anyhow::Result<String>` that:
   - Takes the bundle text, the emit order, and per-module bundle line
     spans (`(module, first_line_inclusive, last_line_exclusive)`).
   - Walks each bundle line. If the line falls inside a `ModuleId::User`
     span, emits a v3 source-map segment for that line mapping to
     `(source_index, 0, 0)`. Lines outside any user span (PREAMBLE,
     wrapper boilerplate, runtime/http synthetic factories) get no
     mapping (gap in `mappings`).
   - For user modules, the `sources` array contains the module's
     relative path string (e.g. `"./src/app.ts"`).
   - Imprecise on column (always 0) and on intra-module line offset
     (always points to source line 0). This is the spec's "Option B" —
     enough for stack-trace tooling to point at the right file. An
     Open Question is logged for upgrading to per-module line precision
     later.
2. To compute `factory_spans`, modify `emit_factories` to return — in
   addition to writing into `out` — a `Vec<(ModuleId, usize, usize)>`
   where each tuple records the inclusive-first / exclusive-last bundle
   line index for the module's factory body. The `bundle()` function
   passes this vector into `build_bundle_source_map`. Line counting is
   done by counting `'\n'` bytes before/after the
   `__zero_define(...) { ... }` block insertion.
3. Update the function signature comment on `bundle()` accordingly.

**Tests:**
- Existing `bundle_emits_no_source_map_by_default` continues to pass
  unchanged.
- Update `bundle_emits_source_map_when_requested`: keep the v3/version
  + sources assertions, add an assertion that `"mappings"` is non-empty
  (`!map_json.contains(r#""mappings":""#)` — empty mappings serializes
  as `"mappings":""` in v3 JSON; the new map will contain at least a
  short BASE64-VLQ string).
- Add a new test `bundle_source_map_resolves_user_line` that bundles a
  trivial `src/app.ts` and asserts: parsing the produced JSON via the
  `sourcemap` crate's `SourceMap::from_reader` returns at least one
  token whose `get_source()` ends with `./src/app.ts`.

---

### Step 3: Add `crates/zero-bundler/src/minify.rs` with `minify_js`

**Goal:** A self-contained, separately testable JS-minifier wrapper around
`swc_ecma_minifier::optimize`. Wired into `bundle()` in Step 4. Splitting
the SWC minifier configuration into its own module isolates the swc
plumbing (Globals, SourceMap, MangleOptions, Compress options) from the
bundler's module-graph walker.

**Files:**
- `crates/zero-bundler/src/minify.rs` (new)
- `crates/zero-bundler/src/lib.rs` — add `pub mod minify;`

**Changes:**
1. New module `minify.rs` exposing:
   ```rust
   pub struct MinifyOutput {
       pub code: String,
       pub source_map: Option<String>,
   }

   /// Minify a JS bundle string. When `bundle_source_map` is provided
   /// AND `emit_source_map` is true, the returned map is composed
   /// (bundle→original then minified→bundle, yielding minified→original).
   /// When `bundle_source_map` is `None`, no map is produced.
   pub fn minify_js(
       code: &str,
       bundle_source_map: Option<&str>,
       emit_source_map: bool,
   ) -> anyhow::Result<MinifyOutput>;
   ```
2. Implementation outline (one pass, no AST rebuild across module
   boundaries):
   - Parse `code` via `swc_core::ecma::parser` into a `Program::Script`
     (the bundle is top-level statements; not an ES module). Use a
     fresh `SourceMap` (call it `cm`).
   - Build `MinifyOptions { compress: Some(CompressOptions::default()),
     mangle: Some(MangleOptions::default()), wrap: false, enclose: false,
     ..Default::default() }`.
   - Populate `MangleOptions.reserved` with the literal names
     `__zero_modules`, `__zero_cache`, `__zero_define`, `__zero_require`,
     `exports`, `module`, `default` (as `swc_atoms::Atom` / `JsWord`).
     `MangleOptions.props` stays `None` (no property mangling).
     `MangleOptions.top_level` stays `false` (don't mangle bundle-top-
     level names — defensive against the reserved list missing one).
     `MangleOptions.keep_class_names` and `keep_fn_names` stay `false`
     (standard aggression).
   - Call `swc_core::ecma::minifier::optimize(program, cm.clone(),
     Some(&comments), Some(&extra), &options, &ExtraOptions { ... })`
     where `comments` is a `swc_common::comments::SingleThreadedComments`
     instance populated during parse. SWC's optimizer preserves `/*!`
     legal comments automatically when comments are tracked.
   - Emit the optimized AST via `swc_core::ecma::codegen::Emitter` with
     `Config { minify: true, ..Default::default() }`. Track a srcmap
     position buffer (`Vec<(BytePos, LineCol)>`) iff `emit_source_map`.
   - When `emit_source_map`:
     - Build the minify→bundle source map from `cm.build_source_map(...)`.
     - If `bundle_source_map` is `Some`, parse it via
       `sourcemap::SourceMap::from_reader` and compose: walk every
       token in the minify map; for each token's
       `(src_line, src_col)` (a position in the bundle text), look up
       the bundle map's token at that position and use its source +
       line as the composed mapping target.
       Build a fresh `sourcemap::SourceMapBuilder`, add each composed
       mapping with `add_raw`, and serialize.
     - If `bundle_source_map` is `None`, return the raw minify map
       (sources = `["<bundle>"]`, useful only for debugging the minifier).
   - When `!emit_source_map`, skip srcmap collection entirely (faster).
3. The module is `pub` — `bundler.rs` calls it. No public API for
   downstream crates beyond what `bundler::bundle` returns.

**Tests:** in a `#[cfg(test)] mod tests` block inside `minify.rs`:
- `minifies_simple_function` — minify `function add(a, b) { return a +
  b; } console.log(add(1, 2));`. Assert output is meaningfully shorter
  (≤ 60% of input length) and contains neither `add` as a *function
  declaration* identifier (locals get mangled) nor `\n  ` (no indent).
- `preserves_reserved_names` — minify a bundle-shaped fixture that
  uses every reserved name (`__zero_define('x', function(exports,
  __zero_require) { exports.y = 1; }); __zero_require('x');`). Assert
  the minified output still contains the literal string
  `__zero_define`, `__zero_require`, `exports`.
- `does_not_mangle_property_names` — minify
  `const o = { someProperty: 1 }; console.log(o['someProperty']);`.
  Assert `someProperty` is still in the output (proves
  `mangle.props` is off).
- `preserves_legal_comments` — minify
  `/*! @license MIT */ const x = 1; /* regular */ console.log(x);`.
  Assert `@license` is in the output, `regular` is not.
- `source_map_request_returns_some` and
  `source_map_none_when_not_requested` — confirm the `Option` shape
  contract.
- `composes_with_bundle_source_map` — call `minify_js` with a small
  hand-built bundle source map (one user-source line); assert the
  returned composed map's `sources` includes the original user-source
  path.

---

### Step 4: Wire `minify_js` into `bundle()` and update bundler tests

**Goal:** Production builds now minify. `bundle()`'s public contract
(`BundleOutput { code, source_map }`) is unchanged; `code` is the minified
bundle. When `emit_sourcemap` is true, `source_map` is the composed
minified→original map.

**Files:**
- `crates/zero-bundler/src/bundler.rs`

**Changes:**
1. At the end of `bundle()`, after `build_bundle_source_map` (Step 2)
   has produced the optional bundle map, call:
   ```rust
   let MinifyOutput { code: min_code, source_map: composed_map } =
       crate::minify::minify_js(&out, source_map.as_deref(), emit_sourcemap)?;
   Ok(BundleOutput { code: min_code, source_map: composed_map })
   ```
   `out` is the existing un-minified bundle string; `source_map` is the
   bundle→original map produced in Step 2.
2. Errors from `minify_js` bubble up as `anyhow::Error`. No fallback
   to un-minified output (spec Requirement 8 — silent fallback would
   hide regressions).
3. The `build_combined_sourcemap` symbol added in Step 2 (now
   `build_bundle_source_map`) stays — its output is the input to the
   composition.

**Tests:** update tests in `crates/zero-bundler/src/bundler.rs`:
- `bundle_with_ts_entry_strips_types_and_imports_zero` — keep the
  assertions that look for *string literal* keys
  (`__zero_define('./src/app.ts'`, `__zero_require('zero')`). These
  survive minification because they're literal string contents. Drop
  any assertion that depends on the bundle being multi-line or
  formatted (none currently — verified).
- `bundle_inlines_zero_http_when_imported` — same. The
  `__zero_define('zero/http'` and `__zero_require('zero/http')`
  literals survive. Drop or rewrite the
  `bundled.contains("function createHttp(")` assertion — `createHttp`
  is a top-level identifier of the synthetic Http module (mangle.top_
  level=false preserves it), but the formatter may emit it as
  `function createHttp(e){...}` rather than with a space. Tighten to
  `bundled.contains("createHttp")` plus `bundled.contains("exports.createHttp=")`.
- `bundle_mixed_ts_and_js_dependencies` — keep string-literal key
  assertions; keep `!bundled.contains(": number")` (transpile happens
  before minify).
- `bundle_errors_when_both_entries_present` — unaffected.
- Add new tests (in a `#[cfg(test)] mod tests` already present):
  - `bundle_is_minified` — bundle a fixture with verbose source (lots
    of whitespace and a multi-line function); assert
    `out.code.len() < unminified_estimate` where the estimate is the
    sum of source lengths (proxy for "minify ran"). Tighten to
    `out.code.lines().count() <= 10` (minified output is usually one
    or two lines).
  - `bundle_evaluates_under_boa` — bundle a fixture whose `src/app.ts`
    is:
    ```ts
    let result = 0;
    function add(a: number, b: number) { return a + b; }
    result = add(2, 3);
    (globalThis as any).result = result;
    ```
    Then `boa_engine::Context::default().eval(Source::from_bytes(&bundle))`
    and read back `globalThis.result`, asserting it equals 5. Catches
    any mangle-induced semantic breakage. Add `boa_engine = {
    workspace = true }` to `[dev-dependencies]` in
    `crates/zero-bundler/Cargo.toml`.
  - `bundle_preserves_reserved_names` — bundle a small fixture;
    assert the minified output contains the literals `__zero_define`,
    `__zero_require`, `__zero_modules`, `__zero_cache`, plus the
    literal `'./src/app.ts'` (the module-ID string key).
  - `bundle_source_map_contains_real_mappings` — bundle with
    `emit_sourcemap = true`; parse the returned map via
    `sourcemap::SourceMap::from_reader`; assert at least one token's
    source path ends with `./src/app.ts`.
  - `bundle_preserves_legal_comments` — bundle a fixture whose
    `src/app.ts` contains `/*! KEEP-ME */ const x = 1;` and a
    regular `/* drop-me */`. Assert `KEEP-ME` is in `out.code`,
    `drop-me` is not.

**Note on `sourcesContent`:** The composed map does **not** embed
`sourcesContent`. Add an Open Question note in the spec (or in a code
comment) that future versions could embed it cheaply if the
`sourcemap::SourceMapBuilder` API supports it without re-reading
sources.

---

### Step 5: Add `bundle_unminified` test-only API

**Goal:** Step 7's showcase size-budget assertion needs the un-minified
bundle for comparison. Exposing this via a `#[cfg]`-gated function keeps
it out of the public crate surface but available to integration tests in
the `zero` crate.

**Files:**
- `crates/zero-bundler/src/bundler.rs`
- `crates/zero-bundler/Cargo.toml`
- `crates/zero/Cargo.toml`

**Changes:**
1. Refactor `bundle()` so its tail step (calling `minify::minify_js`) is
   the only step that differs from an un-minified variant. Concretely,
   extract a private `fn bundle_unminified_inner(config, emit_sourcemap)
   -> anyhow::Result<BundleOutput>` that returns the un-minified output
   + the bundle→original map. `bundle()` calls
   `bundle_unminified_inner` and then runs `minify_js` on its result.
2. Add to `crates/zero-bundler/Cargo.toml`:
   ```toml
   [features]
   test-internals = []
   ```
3. Expose a public function:
   ```rust
   #[cfg(any(test, feature = "test-internals"))]
   pub fn bundle_unminified(
       config: &Config,
       emit_sourcemap: bool,
   ) -> anyhow::Result<BundleOutput> {
       bundle_unminified_inner(config, emit_sourcemap)
   }
   ```
4. In `crates/zero/Cargo.toml`, change the `zero-bundler` dev-dep to
   include the feature:
   ```toml
   [dev-dependencies]
   zero-bundler = { path = "../zero-bundler", features = ["test-internals"] }
   ```
   (The runtime `[dependencies]` entry stays without the feature, so
   `zero` ships without `test-internals` enabled. Cargo feature
   unification across the same crate name will turn it on for the test
   build only when dev-deps are active, which is the desired scope.)

**Tests:**
- One unit test in `bundler.rs` that calls `bundle_unminified` and
  `bundle` against the same fixture, asserting that the un-minified
  output is strictly longer than the minified output.
- The Step 7 showcase test will exercise this function end-to-end.

---

### Step 6: Switch CSS to compressed mode and route plain `.css` through grass

**Goal:** Bundle-shipped CSS is minified. Hash recomputes against
compressed bytes by construction. Plain `.css` and compiled `.scss`
produce uniform compressed output.

**Files:**
- `crates/zero-sass/src/lib.rs`
- `crates/zero-bundler/src/css.rs`

**Changes:**
1. In `zero-sass/src/lib.rs`, change `compile_scss`'s grass options:
   ```rust
   let mut options = grass::Options::default()
       .style(grass::OutputStyle::Compressed)  // was Expanded
       .quiet(true)
       .load_path(parent);
   ```
   Update the module's top-of-file doc comment from "Expanded output
   style only; no minification." to "Compressed output style; output
   is minified."
   Update the `SassOutput.code` docstring from "always expanded" to
   "always compressed".
2. In `crates/zero-bundler/src/css.rs`, replace `copy_css_with_hash`'s
   path with one that calls `zero_sass::compile_scss` on the plain
   `.css` source too. Grass accepts CSS as a degenerate SCSS dialect;
   the compressed-output round-trip normalizes whitespace.
   Concretely: collapse `copy_css_with_hash` and `compile_scss_with_hash`
   into a single helper that takes the raw source and the logical
   filename, calls `compile_scss`, hashes the compressed bytes, writes
   the file (with optional `.map`). The `match ext { "css" => ...,
   "scss" => ... }` branch in `process_css` can fall through to the
   same helper.
3. The hash continues to be computed from the compiled CSS bytes — no
   change needed in the hashing path (it was already over
   `compiled.code.as_bytes()`); the new shared helper inherits this.
4. The Sass module's existing `SassOptions { inline_source_map,
   emit_source_map, load_paths }` shape is unchanged.

**Tests:**
- In `zero-sass/src/lib.rs`:
  - Update `compiles_basic_scss`: replace `out.code.contains("body
    {")` with `out.code.contains("body{")` (no space in compressed
    mode); replace `out.code.contains("color: red")` with
    `out.code.contains("color:red")`.
  - Update `compiles_nested_selectors`: `.outer .inner` becomes
    `.outer .inner{...}` — adjust assertion to
    `out.code.contains(".outer .inner")` (the selector text survives,
    only whitespace inside `{...}` changes; existing assertion is
    already lax enough).
  - Update `resolves_partial_via_at_use`: `padding:8px` instead of
    `padding: 8px`.
  - Update `inline_source_map_appended_when_requested` — no
    formatting change to the appended URL itself, but verify the
    assertion still finds `sourceMappingURL=`.
  - Add `compressed_output_drops_whitespace`: compile
    `body { color: red;\n  padding: 8px; }` and assert
    `!out.code.contains("\n  ")` and `!out.code.contains("  ")`.
- In `zero-bundler/src/css.rs`:
  - Update `process_css_handles_css_only`: input
    `body { color: red; }` produces compressed `body{color:red}`;
    assert the output file contains `body{color:red}`.
  - Update `process_css_compiles_scss`: tighten `assert!(css
    .contains("red"))` (still passes); add `assert!(!css.contains("
    "))` — no double-space.
  - Update `process_css_skips_underscore_partials` — assertion shape
    unchanged.
  - Update `process_css_emits_sourcemap_when_enabled` — `.map` file
    still emitted; assertion shape unchanged.
  - Update `process_css_no_sourcemap_by_default` — assertion shape
    unchanged.
  - Update `process_css_propagates_scss_errors` — assertion shape
    unchanged (grass error path is the same in compressed mode).
  - Update `process_css_sorts_pairs_deterministically` — assertion
    shape unchanged.
  - Add `process_css_hash_is_stable_across_whitespace_changes`:
    write `body { color: red; }` and `body{color:red;}` to two
    different temp roots, run `process_css` on each, assert the
    output hashed filenames are **identical** (proves the hash is
    over compressed bytes).
  - Add `process_css_compresses_plain_css`: write `body { padding:
    8px; }` to `styles/app.css`, run `process_css`, read the output
    file, assert it equals `body{padding:8px}\n` (or whatever
    grass's exact compressed serialization is — re-confirm in the
    actual run; adjust if grass emits without trailing newline or
    uses a different shape).

---

### Step 7: Add size-budget assertion to `tests/showcase_build.rs` and fix downstream test fallout

**Goal:** End-to-end proof that minification meaningfully shrinks the
showcase bundle, and clean up other integration tests whose substring
assertions assumed un-mangled / un-compressed output.

**Files:**
- `crates/zero/tests/showcase_build.rs`
- `crates/zero/tests/build_smoke.rs`
- `crates/zero/tests/build_full.rs` (review only; likely no change)
- `crates/zero/tests/build_sourcemap.rs` (review only; likely no change)

**Changes:**
1. `tests/showcase_build.rs`: after the existing assertions on
   `js_body` / `css_body`, add:
   ```rust
   // JS shrinks by ≥30% vs the un-minified equivalent.
   let config = zero_config::Config::load_from_path(tmp.path()).unwrap();
   let cwd_guard = /* set_current_dir to tmp.path() with a lock */;
   let unminified = zero_bundler::bundle_unminified(&config, false)
       .unwrap()
       .code;
   drop(cwd_guard);
   let min_len = js_body.len() as f64;
   let unmin_len = unminified.len() as f64;
   assert!(
       min_len <= unmin_len * 0.70,
       "minified bundle ({min_len} bytes) not <= 70% of un-minified ({unmin_len} bytes)"
   );

   // CSS shape: no four-space runs, no double newlines.
   assert!(
       !css_body.contains("    ") && !css_body.contains("\n\n"),
       "CSS appears un-minified: {css_body}"
   );
   ```
   The CWD guard uses the same pattern as
   `crates/zero/src/cmd/build.rs`'s `CwdGuard` (a `CWD_LOCK` mutex);
   either import that or replicate it in `tests/common/mod.rs`. If
   `common::prepare_showcase` already chdirs, document and reuse.
2. `tests/build_smoke.rs`: the assertions `bundle.contains("function
   signal(")`, `bundle.contains("class App")`, and
   `bundle.contains("Home")` will break because:
   - `signal` is a function name local to the runtime synthetic
     module's factory — minifier may mangle it (it's not top-level in
     the bundle; the factory's `function(exports, __zero_require) {
     ... function signal(...) ... }` makes `signal` a local).
     Actually, looking at `rewrite_runtime_exports`, the runtime is
     emitted as a top-level `__zero_define('zero', function(exports,
     __zero_require) { ... })` — so `signal` *is* local to that
     factory. It will be mangled.
   - `class App` — `App` is also local to the runtime factory; will
     be mangled.
   - `Home` — local to `routes/home.ts`'s factory; will be mangled.

   Update assertions to look for the *exported* names as property
   keys, which are guaranteed preserved (mangle.props=false):
   ```rust
   assert!(bundle.contains("exports.signal"), "bundle missing signal export");
   assert!(bundle.contains("exports.App"), "bundle missing App export");
   assert!(bundle.contains("exports.default"), "bundle missing default export (Home)");
   ```
   The `node --check` syntax-validity check at the bottom stays
   unchanged (it asserts valid JS, which minification preserves).
3. `tests/build_full.rs` — assertions are over manifest shape /
   index.html shape, not bundle contents. No change expected.
   Re-run to confirm.
4. `tests/build_sourcemap.rs` — asserts `map.contains("./src/app.ts")`
   in the sources array. After Step 2 + Step 4, the composed map's
   `sources` still contains `./src/app.ts` (the chain ends at the
   original source path). Assertion passes unchanged.

**Tests:** the changes here ARE the tests. Run
`cargo test -p zero --tests` after this step; the four touched
integration tests should all pass. Run `cargo test --workspace` to
confirm no regressions elsewhere.

---

### Step 8: Update `docs/building-and-deploying.md` and `docs/config-and-cli.md`

**Goal:** User-facing docs reflect that production builds are always
minified.

**Files:**
- `docs/building-and-deploying.md`
- `docs/config-and-cli.md`

**Changes:**
1. `docs/building-and-deploying.md`:
   - In the section right after the `dist/` tree example, add one
     sentence: "Both `app.<hash>.js` and `app.<hash>.css` are minified
     — production builds always minify; there is no flag."
   - In the flag table, edit the `--sourcemap` row's "Behaviour"
     column to: "Emit external source maps (default off). When enabled,
     the JS map composes positions in the minified bundle back to the
     original source files."
2. `docs/config-and-cli.md`:
   - Under the `zero build` subsection (after the flag table), add a
     one-line note: "Production output is always minified (both JS
     and CSS). The dev server is unaffected."
   - No new rows in the flag table.
3. No changes to `docs/why-zero.md` (minification is implementation
   detail, not positioning).
4. No edits to `issues/scss/spec.md` or `issues/cli-bootstrap/spec.md`
   — those are historical records.

**Tests:** none — pure docs. Verify the markdown renders correctly
locally by skimming.

---

## Risks and Assumptions

- **Assumption: swc_ecma_minifier 52.0.6 API matches swc_core 65.1.0
  AST shapes.** The crate is bundled via swc_core's own feature, so
  the versions are pinned together by swc_core's release. Confirmed by
  the workspace's existing Cargo.lock entries (swc_ecma_ast 23.0.0,
  swc_ecma_codegen 26.0.1). Risk: if swc_core's optional dep on
  swc_ecma_minifier is loose, a `cargo update` could pull in a
  mismatched version. Mitigation: pin via `Cargo.lock` (already
  committed); the Step 1 sanity test catches future drift.
- **Risk: minifier breaks the CJS-shim contract.** Mitigated by the
  reserved-name list (Step 3), `mangle.top_level: false`,
  `mangle.props: None`, the eval-under-Boa round-trip test (Step 4),
  and the showcase integration test (Step 7). If a real user pattern
  trips this, the fix is to extend the reserved list.
- **Risk: source-map composition produces broken maps for users with
  non-standard editors.** Mitigated by keeping the v1 map line-level
  only (Option B) — any consumer that handles v3 maps at all handles
  line-level. Real column precision is logged as an open question.
- **Risk: grass compressed mode emits something a browser misparses.**
  Mitigated by the round-trip CSS test (Step 6) and the showcase
  build assertion (Step 7). grass's compressed mode is well-trodden
  territory; no known correctness issues at the version pinned.
- **Risk: minification slows down `zero build` noticeably.** The
  showcase bundle is small enough that minifier wall-time should be
  sub-second. If it ever becomes a concern, the answer is parallelism
  (`ecma_minifier_concurrent` feature), not opt-out.
- **Assumption: no user code today relies on the bundle being human-
  readable.** `zero dev` continues to serve un-minified per-file ESM,
  so debugging workflows are unaffected. If a user is parsing the
  production bundle textually (e.g. an external tool grepping
  `app.<hash>.js`), this would break — acceptable for a v1 quality
  upgrade.
- **Risk: feature-flag plumbing for `test-internals` leaks
  `bundle_unminified` into release builds via Cargo's feature
  unification.** Mitigation: the feature is enabled only in the
  `zero` crate's `[dev-dependencies]`, not its `[dependencies]`.
  Verified by `cargo build --release -p zero` after Step 5 — should
  not pull `test-internals` in.
