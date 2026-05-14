# Spec: SCSS support

## Problem Statement

The framework spec (`zero-framework-spec.md` §7) currently declares **"the framework has zero CSS features"** — the build copies `.css` files, the dev server serves them as bytes, and that's it. In practice, real applications outgrow plain CSS quickly: variables, nesting, partials, and mixins are table stakes for any non-trivial style sheet. Forcing users to either hand-write everything in raw CSS or wire up an external preprocessor contradicts the framework's "batteries-included, single binary, zero npm" promise. The TypeScript work (see `issues/typescript/spec.md`) already broke the seal on transpilation as a first-class concern; CSS is the next gap.

This issue makes SCSS a first-class authoring layer. After this lands, the canonical scaffold ships SCSS, `zero dev` compiles `.scss` on the fly, `zero build` emits hashed compiled CSS, the framework's published `.d.ts` story has a parallel for styles, and spec §7 is rewritten to acknowledge SCSS as the chosen CSS authoring layer.

The user has confirmed: **full SCSS, dev-ergonomics win** — all-in, including the necessary tooling around it.

## Background

### What exists today (relevant pieces)

- **Build CSS pipeline** (`src/build/css.rs::process_css`): walks `<root>/styles/`, accepts `*.css` files, hashes each one with SHA-256 (8-char prefix), copies to `<out>/assets/<stem>.<hash>.css`, returns `(source_rel, output_rel)` pairs. Skips entirely if `styles/` doesn't exist.
- **Build index.html injection** (`src/build/index_html.rs::render`): reads the source `index.html`, builds a snippet from the manifest containing `<script type="module">` for `app.js` and `<link rel="stylesheet">` for each CSS asset, and injects it before `</head>`. **It does not strip or rewrite the user's existing `<link>` tags** — currently a quirk: a scaffold project's `<link href="/styles/app.css">` survives into the built `index.html` and would 404, alongside the hashed injected link. Fixing this is in scope for this issue.
- **Dev server file routing** (`src/dev/server.rs`, `src/dev/files.rs`): `/styles/*path` goes through `serve_under` (byte-pure passthrough). MIME type comes from `content_type_for` in `src/dev/files.rs`. There is no transpile path for stylesheets today — only for `.ts` files via `serve_under_with_transpile` and `src/dev/transpile.rs`.
- **Dev watcher** (`src/dev/watch.rs`): file changes anywhere under `<root>` trigger a broadcast on `ReloadBus`, which the SSE handler at `/_zero/events` flushes to the browser as a `reload` event. Granularity is full-page reload, not CSS hot-replacement.
- **Scaffold** (`src/scaffold/`): ships `index.html` with `<link rel="stylesheet" href="/styles/app.css">` and a one-file `styles/app.css`.
- **`zero.toml` schema** (`src/config.rs`): three sections — `[project]`, `[dev]`, `[build]`. The TypeScript issue added `[dev] sourcemap = true` (default) and `[build] sourcemap = false` (default) — both already in place and validated.

### Design decisions already made

- **Scope is full SCSS.** Variables (`$var`), nesting, mixins, functions, partials, `@use`, `@forward`, the whole modern Sass module system. This is the "first-class authoring layer" interpretation.
- **`grass` is the engine.** Pure Rust, Dart-Sass-compatible, actively maintained, no C dependency, single-binary distribution preserved. Pulled in via `Cargo.toml`. (Same shape as the swc decision in `issues/typescript`.)
- **`.scss` only.** Indented `.sass` syntax is rejected; `.css` continues to pass through unchanged. One canonical authoring extension keeps the resolver and dev server simple.
- **Dev URL strategy: link to `.scss` directly.** The user's `index.html` writes `<link rel="stylesheet" href="/styles/app.scss">`. The dev server compiles on each request and returns `text/css`. Mirrors the TypeScript pattern (`GET /src/x.ts → transpiled JS`).
- **Build URL strategy: rewrite source `<link>`.** The build compiles each top-level `.scss` (excluding `_partials.scss`) to hashed CSS at `<out>/assets/<stem>.<hash>.css`. The source `index.html`'s `<link href="/styles/app.scss">` is rewritten in the output to point at the hashed asset. This also fixes the existing quirk of stale source `<link>` tags surviving the build.
- **Source maps reuse existing keys.** `[dev] sourcemap = true` (default) and `[build] sourcemap = false` (default) gate SCSS sourcemaps just like they gate TS sourcemaps. Dev: inline; build: external `.map` files next to the hashed CSS.
- **Error model mirrors TS.** Dev: HTTP 500 with the grass error body as plain text. Build: hard fail with structured `(file, line, column, message)` error. Recovery / silent skip is not an option.
- **Spec §7 gets revised.** The "zero CSS features" stance no longer holds. Replacement wording must acknowledge SCSS as the canonical CSS authoring layer while continuing to forbid scoped styles, CSS modules, and CSS-in-JS.

### Where the work lands

Six surfaces:

1. **New Rust dep + helper module:** `grass`-based compile function `(source, options) -> (css, optional_sourcemap)`. Used by the dev server and the build pipeline. Parallel to `src/transpile.rs` for swc.
2. **Dev server** (`src/dev/files.rs`, `src/dev/server.rs`, new `src/dev/sass.rs` or equivalent): `/styles/*path` route gains a SCSS-aware variant. `GET /styles/app.scss` compiles and returns CSS with inline sourcemap. `GET /styles/_partial.scss` should 404 (partials are not standalone resources). `GET /styles/app.css` continues to pass `.css` through.
3. **Build** (`src/build/css.rs`, `src/build/index_html.rs`, `src/build/mod.rs`, `src/cmd/build.rs`): `process_css` extended to compile `.scss` (skipping `_*.scss`), hash the compiled CSS, and emit. Returns manifest entries keyed by the **source** path (`styles/app.scss`) for use by `index_html::render`. `render` extended to rewrite the source `<link>` href to the hashed output URL (replacing the inject-only behavior). Emit external `.map` when `[build] sourcemap = true`.
4. **Scaffold** (`src/scaffold/`): `styles/app.css` → `styles/app.scss` and a new `styles/vars.scss` (design tokens via `$vars` + bridge to `:root` custom properties). `index.html` updated to `<link rel="stylesheet" href="/styles/app.scss">`.
5. **Spec rewrite** (`zero-framework-spec.md` §7): replace "zero CSS features" wording with the new SCSS-first story. CSS variables remain recommended for runtime theming.
6. **AGENTS.md** in the scaffold: brief mention of the SCSS-first authoring path (`@use 'vars';`, partials live as `_name.scss`).

## Requirements

### Compiler integration

1. Add a `grass`-based compile module (working name `src/sass.rs`; exact location TBD). It exposes one function: take a `.scss` source path + options (sourcemap on/off, file path for diagnostics, load paths for `@use` resolution) and return compiled CSS plus an optional sourcemap.
2. The compile module MUST be invoked once per file per request/build. No global cache is required for v1; correctness over caching, parallel to the TS decision.
3. Compile failure surfaces a structured error (file, line, column, message) — not a panic. Error type is the same shape used by the TS transpiler so dev-server and build error paths can share rendering code.
4. The compiler's load-path resolution MUST honor the standard SCSS partial convention: `@use 'buttons'` resolves to `_buttons.scss` in the same directory; `@use '../shared/grid'` walks paths relative to the importing file.

### Dev server

5. `GET /styles/<path>.scss` returns compiled CSS with `Content-Type: text/css; charset=utf-8`.
6. By default, the dev-server response includes an inline sourcemap (`/*# sourceMappingURL=data:application/json;base64,... */`). When `[dev] sourcemap = false`, the inline map is omitted.
7. `GET /styles/_<name>.scss` (any path component starting with an underscore) returns HTTP 404. Partials are not addressable as standalone resources.
8. `GET /styles/<path>.css` continues to serve raw `.css` unchanged (`text/css`, no transpile).
9. Compile errors return HTTP 500 with `Content-Type: text/plain; charset=utf-8` and the grass error body (file, line, column, message) as the response body. Browser console / Network tab surfaces the error directly.
10. The watcher already broadcasts on any change under `<root>`, which causes a full-page reload — partial edits trigger the reload of any page that uses them, with no dependency tracking required.

### Build / `zero build`

11. `process_css` is extended to:
    - Walk `<root>/styles/` for both `*.css` and `*.scss`.
    - Skip any file whose name begins with `_` (SCSS partial convention).
    - For `*.scss`, invoke the compiler, write the compiled CSS to `<out>/assets/<stem>.<hash>.css`, return manifest entries with **source** path `styles/<stem>.scss` and output path `assets/<stem>.<hash>.css`.
    - For `*.css`, continue current behavior (byte-pure copy with hash).
    - Sort the returned pairs deterministically (current behavior, preserved).
12. A name collision between `<stem>.scss` and `<stem>.css` (e.g., `styles/app.scss` and `styles/app.css` both present) MUST error. Matches the "no extension collision" rule from `issues/typescript/spec.md`.
13. When `[build] sourcemap = true` (or `zero build --sourcemap`), `process_css` writes `assets/<stem>.<hash>.css.map` next to the hashed CSS and appends `/*# sourceMappingURL=<stem>.<hash>.css.map */` to the CSS output. Default is off.
14. `src/build/index_html.rs::render` is extended:
    - For each manifest entry with source extension `.scss` or `.css`, **rewrite any existing `<link rel="stylesheet" href="/<source>">` in the source `index.html` to point at the hashed output URL**. This replaces the current "inject only, never rewrite" behavior.
    - If no matching `<link>` exists for a stylesheet that was built, fall back to injecting before `</head>` (preserves the current path).
    - `<script>` injection is unaffected by this change.
    - The rewrite is case-sensitive on attribute names; href matching is a string-equality compare on the path after stripping `?query` and `#fragment` (TBD — see Open Questions).
15. `manifest.json` keys for SCSS-sourced files use the **source** extension: `"styles/app.scss": "assets/app.<hash>.css"`. This makes the manifest a faithful record of what was authored.

### Scaffold

16. The scaffold flips to SCSS as the canonical authoring path:
    - `src/scaffold/styles/app.css` → `src/scaffold/styles/app.scss`. Contains a minimal example of nesting plus a `@use 'vars';` to demonstrate the partial pattern.
    - New `src/scaffold/styles/vars.scss`: defines `$color-primary`, `$color-text`, `$color-bg`, `$space-sm`, `$space-md`, `$radius`, and bridges them to `:root` CSS custom properties (`:root { --color-primary: #{$color-primary}; }`) so users get both compile-time and runtime theming surface.
    - `src/scaffold/index.html`'s `<link>` tag flips from `/styles/app.css` to `/styles/app.scss`.
17. `src/scaffold/AGENTS.md` updates to note the SCSS-first authoring path. Mention partials as `_name.scss`, `@use 'vars';`, and the `vars.scss` → `:root` bridge pattern. Scope of the rewrite is minimal: a short section, not a full doc rewrite.
18. `zero init` continues to refuse to overwrite a non-empty `<root>/` directory (no change).

### Configuration (`zero.toml`)

19. **No new keys.** SCSS sourcemap emission reuses the existing `[dev] sourcemap` / `[build] sourcemap` toggles. The transpile/compile sourcemap decision is unified.

### Framework spec rewrite

20. `zero-framework-spec.md` §7 is rewritten:
    - Old wording: "The framework has zero CSS features. No scoped styles, no CSS modules, no CSS-in-JS, no class object syntax."
    - New wording must:
      - State that SCSS is the canonical CSS authoring layer.
      - Show the `<link rel="stylesheet" href="/styles/app.scss">` pattern.
      - Document the partial convention (`_name.scss` consumed via `@use`).
      - Show the CSS variables / `:root` bridge pattern for runtime theming (still recommended).
      - Continue to forbid scoped styles, CSS modules, and CSS-in-JS class objects — those are still out.
      - Note that `.css` continues to work for users who don't want SCSS.
    - The "Distribution" decision row in §13 is unchanged. The "CSS" decision row updates: "Not a framework concern" → "SCSS authoring layer, CSS variables for runtime theming".

### Backwards compatibility

21. A pure-`.css` project from before this issue MUST continue to build, dev, and test without modification:
    - `<root>/styles/*.css` files still pass through the build untouched (modulo the new `<link>` rewriting behavior, which is a strict improvement).
    - The dev server still serves `/styles/*.css` byte-pure.
    - No SCSS dependency is required if no `.scss` file is present (`grass` is linked unconditionally, but is not invoked at runtime if no `.scss` exists).

## Constraints

- **No new npm dependencies.** SCSS support is a Rust crate (`grass`) pulled in via `Cargo.toml`. Single binary distribution preserved.
- **No C dependencies.** `libsass` and its FFI bindings are out. `grass` is pure Rust.
- **No CSS modules, no scoped styles, no CSS-in-JS, no class-object syntax.** These remain explicitly forbidden by spec §7 even after the rewrite. SCSS gives users variables and nesting; they don't unlock scoped styling.
- **No PostCSS / autoprefixer / plugin ecosystem.** `grass` runs alone. Cross-browser vendor prefixing is the user's problem (or they target evergreen browsers, which is the framework's general assumption).
- **`.scss` is the only preprocessed extension.** No `.sass` (indented), no `.less`, no `.styl`.
- **No CSS minification.** Current build doesn't minify; this issue doesn't add it. Outputs are pretty-printed CSS. (`grass` has a compressed-output mode; v1 uses expanded.) Minification is a future concern.
- **No CSS HMR.** Stylesheet edits trigger the existing full-page reload via the watcher. Granular style hot-replacement is out of scope.
- **No dependency graph for partials.** When `_buttons.scss` changes, the watcher reloads the page; any compiled stylesheet that `@use`s it will be re-fetched and re-compiled on the next request. We do not track partial-to-entry dependencies for selective rebuild. This is fine because dev compile is per-request and build is one-shot.
- **No global compile cache** in the dev-server SCSS compiler for v1. Per-request compile is fine. Parallel to the TS decision.
- **`grass` must be configured to disable any opt-in non-standard behavior.** If `grass` exposes feature flags (e.g., quiet-deps, charset), pick a deterministic default and document it in the compile module.

## Out of Scope

- **CSS modules / scoped styles / CSS-in-JS.** Spec §7 still forbids these.
- **PostCSS / autoprefixer / any plugin ecosystem.** SCSS only, via `grass`.
- **`.sass` (indented syntax), LESS, Stylus.** SCSS only.
- **CSS minification.** Future concern. Current build doesn't minify CSS; this issue doesn't change that.
- **CSS HMR (granular style hot-replacement).** Full-page reload via the existing watcher is sufficient.
- **A partial-to-entry dependency graph for incremental builds.** Compile is per-request in dev, one-shot in build; no graph needed.
- **`@import` (deprecated SCSS rule).** Users should use `@use` / `@forward` per modern Sass. We don't go out of our way to reject `@import` — grass supports it — but the scaffold and docs lead with `@use`.
- **Migrating existing `.css` projects to `.scss`.** Both coexist. Users opt in by renaming.
- **A test-runner integration for styles.** Tests deal with components, not CSS. No SCSS path in `src/test_runner/`.
- **Caching compiled CSS to disk** (e.g., `.zero-cache/`). Parallel to the TS decision; revisit if perf becomes a problem.
- **Editor / language-server integration.** Editors with built-in `.scss` support work out of the box; no `.d.ts`-equivalent is needed for styles.

## Open Questions

- **Exact location of the compile module.** `src/sass.rs` is the working name (parallel to `src/transpile.rs` for swc). Alternative: `src/build/sass.rs` + reused from `src/dev/`, or a `src/styles/` namespace. The plan phase should pick.
- **Source-map URL convention.** Inline sourcemap comment in CSS is `/*# sourceMappingURL=... */`. External-map comment for the build is the same form, file URL. Confirm grass emits these or whether we append them manually. Plan should verify.
- **`<link>` rewriting in `index_html::render` — href matching strictness.** Requirement 14 says match on the path portion after stripping `?query` and `#fragment`. The plan should confirm whether to also allow leading-slash variation (`/styles/app.scss` vs `styles/app.scss`) and what to do with quote style (`href='...'` vs `href="..."`). A small HTML-aware tokenizer or a tolerant regex are the two options; pick one.
- **`grass` output style.** Expanded vs compressed. v1 uses expanded for readability (this issue's stance). Confirm grass's default is expanded, or set it explicitly.
- **`grass` error surface shape.** What fields does `grass`'s error type expose? Map them to the shared `(file, line, column, message)` struct used by the TS transpiler. Plan should produce the mapping.
- **Scaffold AGENTS.md scope.** Minimal addition (a paragraph) vs a fuller rewrite that walks new users through `@use`, partials, and the vars-bridge pattern. Recommendation: minimal addition; the framework spec §7 carries the longer narrative.
- **Spec §13 decision-table CSS row exact wording.** "SCSS authoring layer, CSS variables for runtime theming" is a draft; plan should finalize.
- **Should `process_css` parallelize compilation?** `grass` is CPU-bound and the build is currently serial. For projects with many top-level `.scss` files this could matter. Probably defer; note as a follow-up.
- **`vars.scss` content in the scaffold.** Recommend a minimal but illustrative set of design tokens (primary color, text, bg, spacing, radius) bridged to `:root`. The plan phase should write the final scaffold contents.
