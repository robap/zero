# Plan: SCSS support

## Summary

Add first-class SCSS as the canonical CSS authoring layer for `zero`, without
introducing any npm dependency. The compiler is `grass` pulled in as a pure-Rust
crate, invoked per file. The dev server compiles `/styles/**/*.scss` on the fly
with inline source maps (mirroring the TS path), and `zero build` compiles each
top-level `.scss` to hashed CSS in `<out>/assets/`, optionally emitting external
`.map` files. The scaffold flips to `styles/app.scss` + `styles/vars.scss`, the
build-time `index.html` renderer rewrites the source `<link>` to the hashed
output (replacing the old inject-only behavior), and spec §7 is rewritten to
document SCSS as the framework's chosen CSS authoring layer. Plain `.css` keeps
working everywhere.

## Prerequisites

Open Questions from the spec are resolved up-front for this plan
(decisions baked in below):

- **Compile module location:** `src/sass.rs` — parallel to `src/transpile.rs`.
- **Output style:** `grass` `OutputStyle::Expanded` — set explicitly, never
  rely on the crate default.
- **Sourcemap URL form:**
  - Dev inline: `/*# sourceMappingURL=data:application/json;base64,<...> */`
    appended on a new line at the end of the CSS body.
  - Build external: `/*# sourceMappingURL=<basename>.css.map */` appended on
    a new line; the `.map` file sits next to the hashed CSS in `assets/`.
  `grass` does not emit a sourcemap (it has no public sourcemap API in the
  pinned version). Until that lands we synthesize a degenerate 1-to-1
  source map (`mappings: ""`, `sources: ["<source>"]`,
  `sourcesContent: ["<original scss>"]`, `version: 3`). This is good enough
  to surface the SCSS file in browser devtools — better mappings can be
  added later without changing call sites.
- **`<link>` rewrite matching:**
  - Case-sensitive attribute (`href`, not `HREF`).
  - Both quote styles accepted (`"..."` and `'...'`).
  - Match the path after stripping any `?query` and `#fragment`.
  - Both leading-slash forms accepted: `/styles/app.scss` and
    `styles/app.scss`. Output preserves a leading slash (`/assets/...`).
  - Implementation: a small handwritten scanner (not regex) that walks `<link`
    tags, parses their attributes, and rewrites in place. Lives in
    `src/build/index_html.rs`.
- **`grass` error mapping:** `grass::Error` exposes a span via its `Display`
  / debug fields with a `path:line:col` prefix. We use grass's public
  accessors (`span()`, `kind()`) where available; otherwise we parse the
  rendered message with the same `parse_diag` style helper used by
  `src/transpile.rs`. The mapping target is the existing-shaped
  `(file, line, column, message)` struct, named `SassError` in `src/sass.rs`.
- **`vars.scss` content:** see Step 9 below — minimal but illustrative
  (primary color, text, bg, two spacing tokens, radius) bridged to `:root`
  custom properties.
- **AGENTS.md scope:** minimal — append a short SCSS section near the
  project-layout block; do not rewrite the document.
- **Spec §13 wording:** `CSS | SCSS authoring layer; CSS variables for
  runtime theming | Variables and nesting are table stakes; runtime theming
  stays in plain CSS for zero-cost dynamism`.
- **No parallel compilation in the build for v1.** Note as a follow-up.

No other issues block this work.

## Steps

- [x] **Step 1: `grass` compile module (`src/sass.rs`)**
- [x] **Step 2: dev server route `/styles/*` compiles `.scss`**
- [x] **Step 3: `process_css` compiles `.scss`, emits hashed CSS (+ optional `.map`)**
- [x] **Step 4: `index_html::render` rewrites `<link>` hrefs**
- [x] **Step 5: scaffold flips to `app.scss` + `vars.scss`**
- [x] **Step 6: scaffold `AGENTS.md` mentions SCSS authoring**
- [x] **Step 7: framework spec §7 + §13 rewrite**
- [x] **Step 8: end-to-end integration coverage**

---

## Step Details

### Step 1: `grass` compile module (`src/sass.rs`)

**Goal:** Land the SCSS compile primitive used by both the dev server and the
build pipeline. After this step, `zero::sass` exists with the public surface
documented below, fully unit-tested. No other code changes yet — the new
module is dead weight until Steps 2 and 3 wire it in.

**Files:**
- `Cargo.toml` — add `grass`.
- `src/sass.rs` (new) — the compile function.
- `src/lib.rs` — declare `pub mod sass;`.

**Changes:**

1. `Cargo.toml`: add `grass = { version = "0.13", default-features = false,
   features = ["random"] }`. Use the latest 0.x; pin the minor in
   `Cargo.lock`. Disable `default-features` if it pulls anything beyond the
   bare compiler; re-enable only what's needed (the `random` feature gates
   `random()` — leave it in, it costs nothing).

2. `src/sass.rs`: public surface mirroring `src/transpile.rs`:

   ```rust
   //! SCSS → CSS compiler used by the dev server and the build pipeline.
   //!
   //! Wraps `grass` with a narrow function-call API. Expanded output style
   //! only; no minification.

   /// Options controlling a single `compile_scss` invocation.
   pub struct SassOptions<'a> {
       /// Logical filename used for diagnostics and source-map source paths.
       pub filename: &'a str,
       /// Append `/*# sourceMappingURL=data:application/json;base64,... */`
       /// to the CSS body.
       pub inline_source_map: bool,
       /// Also return the raw source-map JSON string on the result.
       pub emit_source_map: bool,
       /// Extra directories to search for `@use` / `@forward` targets.
       /// The importing file's directory is always searched first.
       pub load_paths: &'a [std::path::PathBuf],
   }

   /// Result of a successful compile.
   #[derive(Debug)]
   pub struct SassOutput {
       /// The emitted CSS source (always expanded).
       pub code: String,
       /// Present only when `opts.emit_source_map == true`. JSON text.
       pub source_map: Option<String>,
   }

   /// Structured compile error: parser or resolution failure with location.
   #[derive(Debug)]
   pub struct SassError {
       pub file: String,
       pub line: u32,
       pub column: u32,
       pub message: String,
   }

   impl std::fmt::Display for SassError { /* "file:line:col: message" */ }
   impl std::error::Error for SassError {}

   /// Compile a `.scss` source string and return CSS.
   ///
   /// `abs_path` is the on-disk path of `source` and is used as the root for
   /// `@use` resolution. The file at `abs_path` does NOT need to exist on
   /// disk — `grass::from_string` is called with the in-memory source.
   pub fn compile_scss(
       source: &str,
       abs_path: &std::path::Path,
       opts: &SassOptions<'_>,
   ) -> Result<SassOutput, SassError>;
   ```

3. Implementation notes:
   - Configure `grass::Options::default()
       .style(grass::OutputStyle::Expanded)
       .quiet(true)
       .load_path(parent_of(abs_path))`
     and append each `opts.load_paths`. Quiet suppresses `@warn`/deprecation
     spam from third-party SCSS — we want predictable diagnostic output.
   - Use `grass::from_string(source.to_string(), &options)`. The string form
     keeps the dev server from having to write a temp file when it already
     holds the source text.
   - On error, map `grass::Error`:
     - Prefer the structured accessors when present in the pinned version
       (e.g. `err.span()` → `(line, column)`, `err.kind()` for the message).
     - If only the rendered string is available, parse with a `parse_diag`
       helper modeled on `src/transpile.rs::parse_diag` looking for the
       first `N:M` pair and using the first line as the message.
     - `file`: `opts.filename`.
   - Source map: grass does not emit one. Construct a degenerate map with:
     ```json
     {"version":3,"sources":["<filename>"],"sourcesContent":["<source>"],
      "names":[],"mappings":""}
     ```
     This satisfies the inline sourcemap contract without lying about
     mappings. Emit only when `inline_source_map || emit_source_map`.
   - Inline append form:
     `code.push_str("\n/*# sourceMappingURL=data:application/json;base64,<...> */\n")`.

4. `src/lib.rs`: add `pub mod sass;` alongside `pub mod transpile;`.

**Tests** (in `src/sass.rs`, `#[cfg(test)]`):

- `compiles_basic_scss`: input `$c: red; body { color: $c; }` produces CSS
  containing `body {` and `color: red`. The output contains no `$`.
- `compiles_nested_selectors`: input `.outer { .inner { color: red; } }`
  produces CSS containing `.outer .inner` (one selector, descendant
  combinator), no `&`.
- `resolves_partial_via_at_use`: write `_buttons.scss` to a tempdir with
  `$btn-padding: 8px;`, write `main.scss` with
  `@use 'buttons'; .btn { padding: buttons.$btn-padding; }`, compile
  `main.scss`, assert the CSS contains `padding: 8px`.
- `inline_source_map_appended_when_requested`: output ends with
  `/*# sourceMappingURL=data:application/json;base64,` followed by `*/`.
- `external_source_map_returned_when_requested`: `output.source_map` is
  `Some` and parses as JSON containing `"version":3` and a `sources` array
  whose first entry matches `opts.filename`.
- `parse_error_returns_structured_error`: input `body { color: ; }` returns
  a `SassError` with non-zero line/column and a non-empty message.
- `unknown_at_use_returns_structured_error`: input `@use 'nonexistent';`
  returns a `SassError` whose message mentions the missing module name.
- `expanded_output_style_is_default`: input `a{color:red}` produces CSS that
  spans multiple lines (i.e., the selector and the declaration are not on
  the same line — proof we're not in compressed mode).

---

### Step 2: dev server route `/styles/*` compiles `.scss`

**Goal:** `GET /styles/app.scss` returns compiled CSS with an inline source
map (when `[dev] sourcemap = true`). Partials 404. `.css` continues to pass
through. This step adds the SCSS-aware variant of the file-serving helper
and switches the `/styles/*path` route to use it.

**Files:**
- `src/dev/sass.rs` (new) — `serve_scss_file` analogous to
  `src/dev/transpile.rs::serve_typescript_file`.
- `src/dev/files.rs` — add `serve_under_with_sass`.
- `src/dev/server.rs` — switch the `/styles/*path` route to the new helper.
- `src/dev/mod.rs` — `pub mod sass;`.

**Changes:**

1. `src/dev/sass.rs` (mirror of `src/dev/transpile.rs`):
   ```rust
   //! Dev-server endpoint that compiles `.scss` files on the fly.

   use std::path::PathBuf;
   use axum::{body::Body, http::{StatusCode, header}, response::{IntoResponse, Response}};
   use crate::sass::{SassOptions, compile_scss};

   /// Read `abs_path`, run it through grass, and return CSS.
   ///
   /// 200 on success (`text/css; charset=utf-8`).
   /// 500 with plain-text body on compile failure.
   /// 404 if the file cannot be read.
   pub async fn serve_scss_file(
       abs_path: PathBuf,
       logical_path: String,
       inline_source_map: bool,
   ) -> Response {
       let source = match tokio::fs::read_to_string(&abs_path).await {
           Ok(s) => s,
           Err(_) => return (StatusCode::NOT_FOUND, "not found").into_response(),
       };
       match compile_scss(
           &source,
           &abs_path,
           &SassOptions {
               filename: &logical_path,
               inline_source_map,
               emit_source_map: false,
               load_paths: &[],
           },
       ) {
           Ok(out) => (
               StatusCode::OK,
               [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
               Body::from(out.code),
           ).into_response(),
           Err(e) => (
               StatusCode::INTERNAL_SERVER_ERROR,
               [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
               format!(
                   "zero dev: scss error\n  {}:{}:{}\n  {}",
                   e.file, e.line, e.column, e.message
               ),
           ).into_response(),
       }
   }
   ```

2. `src/dev/files.rs`: add `serve_under_with_sass`, modeled on
   `serve_under_with_transpile`:
   - Same path-traversal guard (`..` reject + canonicalize-and-check).
   - Inspect the last path segment of `uri_path` (lowercased):
     - If any segment of the relative path starts with `_` **and** the file
       has the `.scss` extension → return 404 (partials are not addressable).
     - Else if the extension is `scss` → canonicalize, then call
       `crate::dev::sass::serve_scss_file(canonical, uri_path.to_string(),
       inline_source_map)`.
     - Else → fall through to `serve_file_within` (byte-pure passthrough,
       including `.css`).
   - The partial check uses URI segments, not OS path components, because
     the request path is the source of truth — we don't want a request like
     `/styles/x/_y.scss` to compile.

3. `src/dev/server.rs`: change the `/styles/*path` route handler:
   ```rust
   .route(
       "/styles/*path",
       get(
           |State(s): State<Arc<AppState>>, Path(p): Path<String>| async move {
               serve_under_with_sass(
                   s.root.join("styles"),
                   "/styles",
                   &format!("/styles/{p}"),
                   s.dev_sourcemap,
               )
               .await
           },
       ),
   )
   ```

4. `src/dev/mod.rs`: add `pub mod sass;`.

**Tests** (in `src/dev/files.rs` and the new `src/dev/sass.rs`):

- `content_type_scss` (in `files.rs`): asserts SCSS is no longer treated as
  octet-stream — `content_type_for("a.scss")` returns
  `"text/css; charset=utf-8"`. (This is so any path that *does* reach
  `serve_file_within` with a `.scss` file still returns the right MIME — a
  defense-in-depth check; the partial-404 path beats it to the punch in
  normal flow.)
- `partial_request_returns_404`: tempdir with `styles/_x.scss`, request
  `/styles/_x.scss` returns 404. (`serve_under_with_sass` test.)
- `nested_partial_returns_404`: tempdir with `styles/forms/_inputs.scss`,
  request `/styles/forms/_inputs.scss` returns 404.
- `scss_request_returns_compiled_css`: tempdir with `styles/app.scss`
  containing `$c: red; body { color: $c; }`, request `/styles/app.scss`
  returns 200 with `text/css; charset=utf-8` and body containing `red`.
- `scss_response_has_inline_sourcemap_when_enabled`: same setup, body ends
  with `/*# sourceMappingURL=data:application/json;base64,` (use
  `inline_source_map = true`).
- `scss_response_omits_sourcemap_when_disabled`: same setup with
  `inline_source_map = false`, body does not contain
  `sourceMappingURL`.
- `compile_error_returns_500_plain_text`: tempdir with malformed scss,
  request returns 500 with `text/plain` content-type and a body containing
  the filename.
- `css_request_passes_through`: tempdir with `styles/legacy.css`, request
  `/styles/legacy.css` returns 200, body byte-equal to the file.
- `traversal_rejected_in_sass_handler`: `/styles/../secret.txt` returns 403.

---

### Step 3: `process_css` compiles `.scss`, emits hashed CSS

**Goal:** Build-side parity with the dev server. `process_css` now walks
both `.css` and `.scss`, compiles SCSS through `src/sass.rs`, writes hashed
CSS into `<out>/assets/`, and optionally emits an external `.map` file.
Manifest entries are keyed by the **source** path (so `styles/app.scss`
maps to `assets/app.<hash>.css`).

**Files:**
- `src/build/css.rs`
- `src/cmd/build.rs` — pass `emit_sourcemap` and `root` into `process_css`.

**Changes:**

1. `src/build/css.rs::process_css` signature changes:
   ```rust
   pub fn process_css(
       root: &Path,
       out: &Path,
       emit_sourcemap: bool,
   ) -> anyhow::Result<Vec<(String, String)>>;
   ```
   Walk `<root>/styles/`:
   - Skip files whose name begins with `_` (any extension).
   - For `*.css`: current byte-pure copy-and-hash behavior, unchanged.
   - For `*.scss`:
     - Read the source.
     - Call `crate::sass::compile_scss(&source, &path, &SassOptions {
         filename: &format!("styles/{filename}"),
         inline_source_map: false,
         emit_source_map: emit_sourcemap,
         load_paths: &[],
       })`. On error, propagate via `anyhow::bail!` with the
       `file:line:col: message` format (use `SassError::to_string()`).
     - Compute the SHA-256 hash over the compiled CSS bytes (same 8-char
       prefix scheme as today). This makes the hash reflect the *output*,
       which is what browsers cache.
     - Write `assets/<stem>.<hash>.css`.
     - If `emit_sourcemap` and the compile returned a sourcemap:
       - Write `assets/<stem>.<hash>.css.map` with the sourcemap JSON.
       - Append `\n/*# sourceMappingURL=<stem>.<hash>.css.map */\n` to the
         CSS body before writing it to disk (or write the body first, then
         append — choose the ordering that keeps a single file write).
     - Append `(source_rel = "styles/<stem>.scss", output_rel =
       "assets/<stem>.<hash>.css")` to the pairs vec.
   - Skip any other extension.
   - Collision check: before the walk, build two `HashSet<String>` of stems
     (one for `.css`, one for `.scss`, partials excluded). If they intersect,
     `anyhow::bail!("styles/{stem}: both .scss and .css present; rename
     one")`. This matches the TS spec's extension-collision rule.
   - Sort the returned pairs by source path (current behavior, preserved).

2. `src/cmd/build.rs`: update the `process_css` call:
   ```rust
   let css_pairs = process_css(&root, &out_dir, emit_sourcemap)?;
   ```
   No other change here; `emit_sourcemap` is already in scope.

**Tests** (in `src/build/css.rs`):

- `process_css_handles_css_only` (replaces `process_css_hashes_and_copies`):
  one `.css` file produces one pair with source `styles/app.css` and output
  `assets/app.<hash>.css`. Existing test, signature updated to pass
  `emit_sourcemap = false`.
- `process_css_returns_empty_when_no_styles_dir`: unchanged, with the new
  arg.
- `process_css_compiles_scss`: tempdir with `styles/app.scss` containing
  `$c: red; body { color: $c; }`. Returns one pair with source
  `styles/app.scss` and output `assets/app.<hash>.css`. The output file
  exists and contains `red`, not `$c`.
- `process_css_skips_underscore_partials`: tempdir with `styles/app.scss`
  (uses `@use 'buttons'`) and `styles/_buttons.scss`. Returns exactly one
  pair, source `styles/app.scss`. No `_buttons.<hash>.css` is emitted.
- `process_css_emits_sourcemap_when_enabled`: tempdir with `styles/app.scss`,
  call with `emit_sourcemap = true`. The output `assets/app.<hash>.css.map`
  exists and contains `"version":3`. The CSS body ends with
  `/*# sourceMappingURL=app.<hash>.css.map */`.
- `process_css_no_sourcemap_by_default`: same setup, `emit_sourcemap =
  false`, no `.map` file, no `sourceMappingURL` comment.
- `process_css_errors_on_stem_collision`: tempdir with both `styles/app.css`
  and `styles/app.scss`. `process_css` returns `Err` and the error message
  mentions both extensions or the offending stem.
- `process_css_propagates_scss_errors`: tempdir with malformed
  `styles/app.scss`, returns `Err` whose message contains
  `styles/app.scss`.
- `process_css_sorts_pairs_deterministically`: tempdir with `styles/b.scss`
  and `styles/a.css`. Output pairs are alphabetically ordered.

---

### Step 4: `index_html::render` rewrites `<link>` hrefs

**Goal:** When the build emits `assets/app.<hash>.css`, the source
`index.html`'s `<link rel="stylesheet" href="/styles/app.scss">` is
rewritten to `<link rel="stylesheet" href="/assets/app.<hash>.css">`. If
the manifest contains a CSS asset for which no matching `<link>` exists in
the source, fall back to injecting before `</head>` (current behavior).

This step is purely an HTML-rewrite change and is independent of grass —
the previous step's manifest entries are already in the right shape.

**Files:**
- `src/build/index_html.rs`

**Changes:**

1. Replace `render`'s simple snippet-build-and-inject loop with a two-phase
   strategy:
   - For each manifest entry whose `out_rel` ends with `.css` and whose
     `source_rel` starts with `styles/`: attempt to rewrite an existing
     `<link rel="stylesheet" href="...">` whose href matches the source.
     Track which entries succeeded.
   - For entries that didn't match any `<link>`, inject a fresh
     `<link rel="stylesheet" href="/<out_rel>">` snippet before `</head>`.
   - `app.js` injection (the `<script>`) is unchanged.

2. New helper `rewrite_link_hrefs`:
   ```rust
   /// Walk `html`, find every `<link ... href="..." ...>` tag, and for each
   /// entry in `pairs` where the href matches `source_rel` (or `/source_rel`,
   /// modulo `?query` and `#fragment`), replace the href with
   /// `/<output_rel>`. Returns the modified HTML and the set of source paths
   /// that were successfully rewritten.
   ///
   /// Quote style is preserved. Other attributes on the same tag are
   /// preserved verbatim. Match on `href` attribute name only (lowercase).
   fn rewrite_link_hrefs(
       html: &str,
       pairs: &[(String, String)],
   ) -> (String, std::collections::HashSet<String>);
   ```

   Implementation: a tiny state-machine scanner.
   - Scan for `<link` (case-insensitive on the tag name, ASCII only).
   - For each `<link>` tag, find the closing `>` and within that span find
     `href` (lowercase). Accept both `'...'` and `"..."` quotes. Extract
     the href value.
   - Normalize the href: strip `?query`/`#fragment`, then strip a leading
     `/`. The result is what we compare against `source_rel`.
   - If a matching `source_rel` exists, splice in `/<output_rel>` in place
     of the original href value (preserving quotes). Record the match in
     the rewritten set.
   - Skip self-closing tags and `</link>` (there is no closing-tag form,
     but defensively ignore anything that isn't a start tag).

3. `render` body:
   ```rust
   let src = std::fs::read_to_string(root.join("index.html"))?;

   let (mut html, rewritten) = rewrite_link_hrefs(&src, manifest);

   let mut snippet = String::new();
   for (logical, out_rel) in manifest {
       if logical == "app.js" {
           snippet.push_str(&format!(
               r#"<script type="module" src="/{out_rel}"></script>"#));
           snippet.push('\n');
       } else if out_rel.ends_with(".css") && !rewritten.contains(logical) {
           snippet.push_str(&format!(
               r#"<link rel="stylesheet" href="/{out_rel}">"#));
           snippet.push('\n');
       }
   }

   let result = inject_before_head_close(&html, &snippet);
   std::fs::write(out.join("index.html"), result)?;
   Ok(())
   ```

**Tests** (in `src/build/index_html.rs`):

- `render_rewrites_scss_link` (new): source `index.html` has
  `<link rel="stylesheet" href="/styles/app.scss">`, manifest has
  `("styles/app.scss", "assets/app.abc12345.css")`. Output `index.html`
  contains `<link rel="stylesheet" href="/assets/app.abc12345.css">` and
  does not contain `app.scss`.
- `render_rewrites_css_link_with_single_quotes`: same as above but the
  source uses `href='/styles/app.scss'`. Output preserves single quotes
  around the new href.
- `render_rewrites_link_without_leading_slash`: source has
  `href="styles/app.scss"` (no leading `/`). Output href is
  `/assets/app.<hash>.css` (with leading slash).
- `render_rewrites_link_stripping_query_and_fragment`: source has
  `href="/styles/app.scss?v=1"`. Match still succeeds; output is the
  hashed URL with no query.
- `render_rewrites_css_link`: a pre-existing `.css` source `<link>` is
  rewritten too — this is the strict-improvement behavior the spec calls
  out.
- `render_falls_back_to_injection_when_no_link`: source `index.html` has
  no `<link>` at all. Manifest contains a CSS entry. Output contains an
  injected `<link rel="stylesheet" href="/assets/...">` before `</head>`.
- `render_injects_script_and_link` (existing): updated — the manifest still
  uses `styles/app.css` as the source key, but because there's no matching
  `<link>` in the test's source HTML, the injection path is exercised.
  Existing assertions stay green.
- `render_does_not_mutate_unrelated_links`: source has
  `<link rel="icon" href="/favicon.ico">` and the SCSS `<link>`. Only the
  SCSS link is rewritten; the icon link is untouched. (This proves we're
  matching by href value, not by tag presence.)
- `rewrite_link_hrefs_returns_match_set`: unit test of the helper, asserts
  the second return value contains the source path that matched.

---

### Step 5: scaffold flips to `app.scss` + `vars.scss`

**Goal:** New projects ship SCSS by default. Existing `.css` projects are
unaffected (the build and dev paths both still serve plain CSS).

**Files:**
- `src/scaffold.rs`
- `src/scaffold/index.html`
- `src/scaffold/styles/app.css` — **delete**.
- `src/scaffold/styles/app.scss` (new).
- `src/scaffold/styles/vars.scss` (new).

**Changes:**

1. `src/scaffold/index.html`: change the stylesheet `<link>`:
   ```html
   <link rel="stylesheet" href="/styles/app.scss">
   ```

2. `src/scaffold/styles/vars.scss` (new):
   ```scss
   // Design tokens. Edit these to retheme the app.
   //
   // Compile-time use: `@use 'vars';` then `vars.$color-primary`.
   // Runtime use: the `:root` block below bridges each token to a CSS
   // custom property, so plain CSS can read `var(--color-primary)`.

   $color-primary: #3b82f6;
   $color-text:    #1a1a1a;
   $color-bg:      #ffffff;

   $space-sm: 0.5rem;
   $space-md: 1rem;

   $radius: 4px;

   :root {
     --color-primary: #{$color-primary};
     --color-text:    #{$color-text};
     --color-bg:      #{$color-bg};
     --space-sm:      #{$space-sm};
     --space-md:      #{$space-md};
     --radius:        #{$radius};
   }
   ```

3. `src/scaffold/styles/app.scss` (new):
   ```scss
   @use 'vars';

   body {
     font-family: system-ui, sans-serif;
     padding: vars.$space-md * 2;
     color: var(--color-text);
     background: var(--color-bg);
   }

   h1 {
     color: vars.$color-primary;
   }
   ```
   This demonstrates: `@use` for partials, namespace-qualified vars,
   nesting (single level is enough; the doc explains it), and the
   `:root`-bridge runtime story.

4. `src/scaffold.rs`:
   - Replace `const TPL_APP_CSS: &str = include_str!("scaffold/styles/app.css");`
     with two new constants:
     ```rust
     const TPL_APP_SCSS:  &str = include_str!("scaffold/styles/app.scss");
     const TPL_VARS_SCSS: &str = include_str!("scaffold/styles/vars.scss");
     ```
   - In `write_to`, replace
     `fs::write(root_dir.join("styles").join("app.css"), TPL_APP_CSS)?;`
     with:
     ```rust
     fs::write(root_dir.join("styles").join("app.scss"), TPL_APP_SCSS)?;
     fs::write(root_dir.join("styles").join("vars.scss"), TPL_VARS_SCSS)?;
     ```

5. Delete `src/scaffold/styles/app.css`.

**Tests** (in `src/scaffold.rs`):

- Existing `write_to_emits_all_files` updated: replace the
  `styles/app.css` assertion with assertions that
  `styles/app.scss` and `styles/vars.scss` both exist and are non-empty,
  and that `styles/app.scss` contains `@use 'vars'`.
- New `write_to_index_html_links_to_scss`: read the rendered
  `index.html` and assert it contains `<link rel="stylesheet" href="/styles/app.scss">`.
- New `vars_scss_bridges_tokens_to_root`: read `styles/vars.scss` and
  assert it contains both a `$color-primary:` line and a
  `--color-primary: #{$color-primary};` line. This protects the
  documented pattern from regression.

---

### Step 6: scaffold `AGENTS.md` mentions SCSS authoring

**Goal:** New users see the SCSS-first authoring path in the project's
own docs. Keep the addition tight — one short section, not a rewrite.

**Files:**
- `src/scaffold/AGENTS.md`

**Changes:**

1. Update the project layout block (near line 25-41 in current
   `AGENTS.md`) to show:
   ```
   └── styles/
       ├── vars.scss        # SCSS partial — design tokens
       └── app.scss         # entry stylesheet — @use 'vars';
   ```

2. Add a new `## Styles` section, placed after the `## Routes` section and
   before `## Navigation`. Roughly:

   ```markdown
   ## Styles

   The scaffold authors styles in SCSS. `zero dev` compiles `.scss` on the
   fly; `zero build` emits hashed CSS into `<out>/assets/`.

   - `index.html` links to the SCSS entry: `<link rel="stylesheet"
     href="/styles/app.scss">`. The build rewrites this href to the hashed
     output.
   - Partials use the standard underscore prefix: `styles/_buttons.scss`
     is consumed via `@use 'buttons';` from a sibling file. Files whose
     name starts with `_` are not addressable as standalone stylesheets.
   - Design tokens live in `styles/vars.scss` and are bridged to `:root`
     CSS custom properties so plain CSS can read them via `var(--name)`.
     Use `vars.$token` inside SCSS, `var(--token)` outside.
   - Plain `.css` still works — the dev server and build serve and hash
     `.css` files unchanged. Rename to `.scss` to opt in.

   The framework forbids scoped styles, CSS modules, and CSS-in-JS. SCSS
   gives you variables and nesting; class names are still plain strings.
   ```

3. Add `## Styles` to the `write_to_agents_md_has_section_sentinels`
   test's sentinel list.

**Tests** (in `src/scaffold.rs`):

- Extend the existing `write_to_agents_md_has_section_sentinels` to
  include `"## Styles"` in the list.

---

### Step 7: framework spec §7 + §13 rewrite

**Goal:** Update the spec to acknowledge SCSS as the canonical CSS
authoring layer. Keep all of the other §7 prohibitions (scoped styles,
CSS modules, CSS-in-JS, class object syntax) intact.

**Files:**
- `zero-framework-spec.md`

**Changes:**

1. Replace the contents of `## 7. CSS Strategy` (currently lines 767-803)
   with text along the lines of:

   ```markdown
   ## 7. CSS Strategy

   **SCSS is the canonical CSS authoring layer.** `.scss` files give you
   variables, nesting, partials, and the modern Sass module system
   (`@use` / `@forward`). The framework still forbids scoped styles,
   CSS modules, CSS-in-JS, and class object syntax — SCSS unlocks
   variables and nesting, not scoped styling.

   The developer writes `.scss` files and loads them via `<link>` tags in
   `index.html`. CSS custom properties remain the recommended pattern for
   *runtime* theming (e.g. dark mode); SCSS variables are compile-time
   only.

   ```html
   <link rel="stylesheet" href="/styles/app.scss">
   ```

   ```scss
   // styles/vars.scss — design tokens
   $color-primary: #3b82f6;
   $color-text:    #1a1a1a;
   $space-md:      1rem;
   $radius:        4px;

   :root {
     --color-primary: #{$color-primary};
     --color-text:    #{$color-text};
     --space-md:      #{$space-md};
     --radius:        #{$radius};
   }
   ```

   ```scss
   // styles/app.scss — entry stylesheet
   @use 'vars';

   body {
     color: var(--color-text);
     padding: vars.$space-md * 2;
   }

   .btn {
     border-radius: vars.$radius;
     &.btn-primary { background: vars.$color-primary; }
   }
   ```

   Partials use the standard underscore prefix: `styles/_buttons.scss`
   is consumed via `@use 'buttons';`. Files whose name starts with `_`
   are not addressable as standalone stylesheets.

   `zero dev` compiles `.scss` on the fly and serves the compiled CSS
   with an inline source map. `zero build` compiles each top-level
   `.scss` to hashed CSS in `<out>/assets/` and rewrites the source
   `<link>`'s href to point at the hashed asset. External source maps
   are emitted when `[build] sourcemap = true` (default: off). The dev
   inline sourcemap is gated on `[dev] sourcemap = true` (default: on).

   Plain `.css` still works — the dev server and build hash and serve
   `.css` files unchanged. Use whichever extension fits.

   Components use plain string class names:

   ```ts
   function Button(props: { variant: string, children: any }) {
     return html`<button class="btn btn-${props.variant}">${props.children}</button>`
   }
   ```

   The only thing `zero build` does with CSS — compiled or not — is hash
   it, copy it to `<out>/assets/`, and rewrite source-side `<link>` hrefs
   to the hashed URL.
   ```

2. §13 decision table: update the CSS row. Change

   `| CSS | Not a framework concern | Developer loads stylesheets in HTML, uses CSS variables |`

   to

   `| CSS | SCSS authoring layer; CSS variables for runtime theming | Variables and nesting are table stakes; runtime theming stays in plain CSS for zero-cost dynamism |`

**Tests:** Documentation-only — no automated test. Manually re-read §7 +
§13 after edits.

---

### Step 8: end-to-end integration coverage

**Goal:** Lock the new behavior in with end-to-end tests, not just unit
tests on the helper functions.

**Files:**
- `tests/scss_dev.rs` (new) — exercises the dev server against a tempdir
  project. Mirrors the shape of any existing `tests/` integration tests
  (`assert_cmd` + `tempfile` are already in `dev-dependencies`).
- `tests/scss_build.rs` (new) — exercises `zero build`.

If `tests/` already houses a `dev_server.rs` or similar, follow whatever
naming and bootstrap pattern is there. If not, write the two files
freestanding.

**Changes:**

1. `tests/scss_dev.rs`:
   - Spawn `zero dev` in a tempdir project that contains the new SCSS
     scaffold output.
   - HTTP `GET /styles/app.scss` → 200, `text/css`, body contains
     `color:` and `red` (or whatever the scaffold ends up with), body
     ends with the inline sourcemap marker when
     `[dev] sourcemap = true`.
   - HTTP `GET /styles/_vars.scss` → 404.
   - HTTP `GET /styles/nonexistent.scss` → 404 (file missing — not a
     partial path).
   - Write a malformed `styles/bad.scss`, request `/styles/bad.scss` →
     500 with `text/plain` body containing the filename.

2. `tests/scss_build.rs`:
   - Tempdir project with the scaffold files.
   - Run `zero build`.
   - `dist/assets/` contains `app.<hash>.css` and (with
     `--sourcemap`) `app.<hash>.css.map`.
   - `dist/index.html` contains
     `<link rel="stylesheet" href="/assets/app.<hash>.css">` and does
     NOT contain `app.scss`.
   - `dist/manifest.json` contains the key
     `"styles/app.scss"` → `"assets/app.<hash>.css"`.
   - Stem-collision: add `styles/app.css` next to the scaffold's
     `styles/app.scss`, run `zero build`, assert non-zero exit and the
     error message mentions the stem `app` or `app.scss`.

3. Backward-compat smoke test (can live in either of the above):
   - Tempdir project with only `styles/legacy.css` (no SCSS). `zero
     build` succeeds; the `.css` is hashed and rewritten in the
     `<link>`.

**Tests:** The integration files *are* the tests. No subordinate unit
tests in this step.

---

## Risks and Assumptions

- **`grass` sourcemap surface.** The plan assumes the pinned `grass`
  version does not emit real sourcemaps and that a degenerate
  `mappings: ""` map is acceptable. If a future `grass` ships real
  sourcemaps, only the body of `compile_scss` changes — call sites are
  insulated. If browsers reject a degenerate map and refuse to show the
  SCSS source at all, fall back to emitting no map (drop the
  `sourceMappingURL` comment when `mappings == ""`).
- **`grass::Error` shape.** The mapping to `(file, line, column,
  message)` depends on grass exposing either a span accessor or a
  predictable rendered format. If neither holds in the pinned version,
  the parser-regex fallback (`parse_diag`) may produce
  `line = 1, column = 1` for some classes of error. That's a degraded
  but non-broken outcome; lock it in as a known limitation rather than
  expanding scope.
- **HTML `<link>` rewriter false positives.** The hand-rolled scanner
  matches `<link ... href="...">` shapes. HTML inside `<script>` /
  `<style>` / comments could in theory contain literal `<link` text.
  We're not building a full HTML parser; the assumption is that
  `index.html` in zero projects is small, hand-written, and does not
  embed `<link>`-looking text inside other elements. If that assumption
  breaks, we add a comment- and `<script>`-skipping pass.
- **Hash stability across grass versions.** Because we hash the
  *compiled* CSS, a `grass` upgrade can change every CSS hash even when
  source SCSS is unchanged. That's the right cache-busting behavior
  (the bytes the browser receives genuinely changed) but worth
  understanding when reading a diff.
- **No partial→entry dep graph.** A partial edit forces a full-page
  reload, which causes a fresh compile of every page's stylesheets on
  the next request. For projects with very large SCSS graphs this could
  be noticeable in dev. Out of scope for v1.
- **`@import` is implicitly allowed because grass supports it.** The
  scaffold and docs lead with `@use`. We don't reject `@import`; if a
  user pastes legacy SCSS in, it just works. Note this in the spec's
  §7 only if it's worth the words.
- **Spec rewrite is authoritative.** If the §7 rewrite contradicts
  anything elsewhere in `zero-framework-spec.md` (or in `AGENTS.md`),
  the rewrite wins for this issue; reconcile other docs in a follow-up.
