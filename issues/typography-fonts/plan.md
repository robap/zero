# Plan: Local Geist fonts + typography utilities

## Summary

This change replaces the Google Fonts CDN import in `_base.scss` with four
locally-bundled Geist `.woff2` files shipped inside the framework binary, and
moves the element-selector typography defaults (`h1`–`h6`, `p`, `small`,
`code`, `a`, `hr`) out of `_base.scss` into a new opt-in utility partial
`.zero/styles/_typography.scss` with twelve `.text-*` / `.divider` classes
wrapped in `@layer components`. The fonts arrive as a parallel
`binary_manifest()` returning `Vec<(&'static str, &'static [u8])>` alongside
the existing text `framework_manifest()`; both feed `zero init` and
`zero update`. A new `/.zero/fonts/*` dev-server route serves the woff2 files,
the build pipeline copies them into `dist/.zero/fonts/`, and the showcase +
examples are migrated from bare heading tags to the new utility classes.

## Prerequisites

None. All four `.woff2` source files are already sitting (untracked) in
`crates/zero-scaffold/src/scaffold/.zero/styles/`, and the license text is
at `~/Documents/code/zero_claude_design_system_files/Geist/OFL.txt`.

Open-question resolutions baked into this plan:
- **OFL.txt entry shape**: lives in `binary_manifest()` (storage consistent with the four woff2 files; spec accepts either).
- **`copy_public` rename**: renamed to `copy_tree` and called twice (once for `public/`, once for `.zero/fonts/`).
- **Manifest merge**: kept parallel (`framework_manifest` text-only + new `binary_manifest` bytes-only). No unified `FileContents` enum.
- **Heading reset**: keep `margin: 0` (byte-identical with the deleted element rules) — not `margin-block: 0`.
- **`text-link-muted` variant**: not added; showcase/examples cleanup will surface need only if it exists.
- **Single PR**: all eleven steps land together.

## Steps

- [x] **Step 1: Move font assets into `.zero/fonts/` in the scaffold tree**
- [x] **Step 2: Add `woff2` / `woff` / `ttf` / `otf` MIME types to `content_type_for`**
- [x] **Step 3: Add `/.zero/fonts/*` dev-server route**
- [x] **Step 4: Add `binary_manifest()` and extend `write_framework_files` to write bytes**
- [x] **Step 5: Extend `zero update` (compute_plan + apply) to handle binary manifest entries**
- [x] **Step 6: Rename `copy_public` → `copy_tree` and copy `.zero/fonts/` to `dist/`**
- [x] **Step 7: Create `.zero/styles/_typography.scss` + wire it into `zero.scss` + `framework_manifest()`**
- [x] **Step 8: Edit `_base.scss` — remove Google Fonts import + element selectors, add four `@font-face` blocks**
- [x] **Step 9: Migrate showcase + examples bare headings to the `.text-*` utility classes**
- [x] **Step 10: Framework-side integration test asserting compiled CSS shape**
- [x] **Step 11: Documentation updates (spec §7.1, `BEST_PRACTICES.md`, scaffold `AGENTS.md`)**

---

## Step Details

### Step 1: Move font assets into `.zero/fonts/` in the scaffold tree

**Goal:** Land the five binary assets (four `.woff2` + `OFL.txt`) in the
canonical location so subsequent `include_bytes!` calls compile. No Rust
changes; this is a pure file-tree move that keeps `cargo build` green
because nothing currently references the woff2 files from Rust.

**Files:**
- Create directory `crates/zero-scaffold/src/scaffold/.zero/fonts/`.
- Plain `mv` (files are untracked — no `git mv` needed) for all four
  woff2s from `crates/zero-scaffold/src/scaffold/.zero/styles/` to
  `crates/zero-scaffold/src/scaffold/.zero/fonts/`:
  - `Geist-VariableFont_wght.woff2`
  - `Geist-Italic-VariableFont_wght.woff2`
  - `GeistMono-VariableFont_wght.woff2`
  - `GeistMono-Italic-VariableFont_wght.woff2`
- Copy `~/Documents/code/zero_claude_design_system_files/Geist/OFL.txt`
  → `crates/zero-scaffold/src/scaffold/.zero/fonts/OFL.txt`.

**Changes:** None to source code. Verify final listing of
`crates/zero-scaffold/src/scaffold/.zero/fonts/` contains exactly five files
(four `.woff2` + `OFL.txt`) and that no woff2 files remain in
`crates/zero-scaffold/src/scaffold/.zero/styles/`.

**Tests:** No new tests; `cargo test --workspace` continues to pass because
no manifest yet references the moved files.

---

### Step 2: Add `woff2` / `woff` / `ttf` / `otf` MIME types

**Goal:** Teach the dev server to send `font/woff2` for variable-Geist
requests before the dev-server route lands in Step 3, so when that route
is added it inherits the correct content-type from `content_type_for`.

**Files:** `crates/zero-dev/src/files.rs`.

**Changes:** Inside `content_type_for`, between the existing `image/x-icon`
and `text/plain` arms, add:

```rust
Some("woff2") => "font/woff2",
Some("woff")  => "font/woff",
Some("ttf")   => "font/ttf",
Some("otf")   => "font/otf",
```

`woff` / `ttf` / `otf` ship defensively even though only `woff2` is bundled
today; cost is four lines.

**Tests:** In the existing `mod tests` block in `files.rs`, add four
parallel one-liner tests modeled on `content_type_js`:

```rust
#[test]
fn content_type_woff2() {
    assert_eq!(content_type_for(Path::new("g.woff2")), "font/woff2");
}
#[test]
fn content_type_woff() {
    assert_eq!(content_type_for(Path::new("g.woff")), "font/woff");
}
#[test]
fn content_type_ttf() {
    assert_eq!(content_type_for(Path::new("g.ttf")), "font/ttf");
}
#[test]
fn content_type_otf() {
    assert_eq!(content_type_for(Path::new("g.otf")), "font/otf");
}
```

---

### Step 3: Add `/.zero/fonts/*` dev-server route

**Goal:** Serve woff2 files via the dev server so once `_base.scss`
references `/.zero/fonts/...` (in Step 8), browsers can fetch them.

**Files:** `crates/zero-dev/src/server.rs`.

**Changes:** In `build_app`, immediately after the existing
`.route("/.zero/components/*path", …)` block, add:

```rust
.route(
    "/.zero/fonts/*path",
    get(
        |State(s): State<Arc<AppState>>, Path(p): Path<String>| async move {
            serve_under(
                s.root.join(".zero").join("fonts"),
                "/.zero/fonts",
                &format!("/.zero/fonts/{p}"),
            )
            .await
        },
    ),
)
```

`serve_under` (not `_with_transpile` / `_with_sass`) is correct: woff2 files
are served byte-for-byte. No additional imports needed (`serve_under` is
already imported at module top).

**Tests:** In `server.rs`'s `mod tests`, add an integration test next to
`src_route_falls_through_to_404_for_missing_file`:

```rust
#[tokio::test]
async fn fonts_route_serves_woff2_with_correct_content_type() {
    let tmp = tempfile::tempdir().unwrap();
    let fonts_dir = tmp.path().join(".zero").join("fonts");
    std::fs::create_dir_all(&fonts_dir).unwrap();
    let body = b"\x77OF2\x00\x00\x00\x01stub-woff2-bytes";
    std::fs::write(fonts_dir.join("Geist-VariableFont_wght.woff2"), body).unwrap();
    let state = make_state(tmp.path().to_path_buf());
    let app = build_app(state);
    let req = Request::builder()
        .uri("/.zero/fonts/Geist-VariableFont_wght.woff2")
        .body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers().get("content-type").unwrap(), "font/woff2");
    let got = resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(got.as_ref(), body);
}
```

Also add a sibling 404 test for a missing font file path.

---

### Step 4: Add `binary_manifest()` and extend `write_framework_files`

**Goal:** Embed the five binary assets in the CLI binary via `include_bytes!`
and have `zero init` / `write_initial_project` materialize them on disk
alongside the existing text manifest entries.

**Files:** `crates/zero-scaffold/src/lib.rs`.

**Changes:**

1. After the existing block of `TPL_*: &str = include_str!(...)` constants,
   add five module-level byte constants:
   ```rust
   const FONT_GEIST: &[u8]              = include_bytes!("scaffold/.zero/fonts/Geist-VariableFont_wght.woff2");
   const FONT_GEIST_ITALIC: &[u8]       = include_bytes!("scaffold/.zero/fonts/Geist-Italic-VariableFont_wght.woff2");
   const FONT_GEIST_MONO: &[u8]         = include_bytes!("scaffold/.zero/fonts/GeistMono-VariableFont_wght.woff2");
   const FONT_GEIST_MONO_ITALIC: &[u8]  = include_bytes!("scaffold/.zero/fonts/GeistMono-Italic-VariableFont_wght.woff2");
   const FONT_OFL_TXT: &[u8]            = include_bytes!("scaffold/.zero/fonts/OFL.txt");
   ```

2. Add a new public function adjacent to `framework_manifest`:
   ```rust
   /// Returns the canonical list of framework-owned binary assets written
   /// into `.zero/`. Each tuple is `(relative_path, bytes)`. Parallel to
   /// [`framework_manifest`] for text files; consulted by `zero init` and
   /// `zero update` so binary assets follow the same single-source-of-truth
   /// rule.
   ///
   /// # Returns
   /// A vector of `(relative path, file bytes)` pairs.
   pub fn binary_manifest() -> Vec<(&'static str, &'static [u8])> {
       vec![
           (".zero/fonts/Geist-VariableFont_wght.woff2",         FONT_GEIST),
           (".zero/fonts/Geist-Italic-VariableFont_wght.woff2",  FONT_GEIST_ITALIC),
           (".zero/fonts/GeistMono-VariableFont_wght.woff2",     FONT_GEIST_MONO),
           (".zero/fonts/GeistMono-Italic-VariableFont_wght.woff2", FONT_GEIST_MONO_ITALIC),
           (".zero/fonts/OFL.txt",                               FONT_OFL_TXT),
       ]
   }
   ```

3. Extend `write_framework_files`: after the existing `for (rel, content) in
   framework_manifest()` loop, add a second loop iterating `binary_manifest()`
   that uses identical mkdir + `fs::write` logic. `fs::write` accepts both
   `&str` and `&[u8]`, so the loop body is the same shape.

**Tests:** In the `mod tests` block in `lib.rs`:

1. Extend `write_initial_project_emits_framework_files`: after the existing
   assertions, assert each binary file is present and non-empty:
   ```rust
   for rel in [
       ".zero/fonts/Geist-VariableFont_wght.woff2",
       ".zero/fonts/Geist-Italic-VariableFont_wght.woff2",
       ".zero/fonts/GeistMono-VariableFont_wght.woff2",
       ".zero/fonts/GeistMono-Italic-VariableFont_wght.woff2",
       ".zero/fonts/OFL.txt",
   ] {
       let p = root.join(rel);
       let bytes = fs::read(&p).unwrap_or_else(|_| panic!("missing {rel}"));
       assert!(!bytes.is_empty(), "{rel} is empty");
   }
   ```

2. Add a new test `binary_manifest_matches_expected_paths`:
   ```rust
   #[test]
   fn binary_manifest_matches_expected_paths() {
       let manifest = binary_manifest();
       let actual: BTreeSet<&str> = manifest.iter().map(|(p, _)| *p).collect();
       let expected: BTreeSet<&str> = [
           ".zero/fonts/Geist-VariableFont_wght.woff2",
           ".zero/fonts/Geist-Italic-VariableFont_wght.woff2",
           ".zero/fonts/GeistMono-VariableFont_wght.woff2",
           ".zero/fonts/GeistMono-Italic-VariableFont_wght.woff2",
           ".zero/fonts/OFL.txt",
       ].into_iter().collect();
       assert_eq!(actual, expected, "binary manifest path-set drift");
       assert_eq!(manifest.len(), 5, "binary manifest has duplicate keys");
       for (_, bytes) in manifest {
           assert!(!bytes.is_empty(), "binary manifest entry is empty");
       }
   }
   ```

3. `write_framework_files_writes_only_dot_zero` still passes — binary files
   already land under `.zero/fonts/`, and the loop body adds no entries
   outside `.zero/`. Extend that test's manifest check to walk both
   manifests:
   ```rust
   for (rel, _) in framework_manifest() {
       assert!(root.join(rel).exists(), "framework text file missing: {rel}");
   }
   for (rel, _) in binary_manifest() {
       assert!(root.join(rel).exists(), "framework binary file missing: {rel}");
   }
   ```

---

### Step 5: Extend `zero update` to handle the binary manifest

**Goal:** `zero update` diffs / applies binary entries with the same Add /
Update / Remove semantics it already uses for text entries, using byte
equality instead of string equality.

**Files:** `crates/zero/src/cmd/update.rs`.

**Changes:**

1. Import `binary_manifest` alongside the existing import:
   ```rust
   use zero_scaffold::{Operation, binary_manifest, framework_manifest};
   ```

2. In `compute_plan`:
   - Replace `let manifest = framework_manifest();` with two locals.
   - Replace `manifest_paths` set construction with a merge of both manifests:
     ```rust
     let text_manifest = framework_manifest();
     let bin_manifest  = binary_manifest();
     let manifest_paths: BTreeSet<PathBuf> = text_manifest
         .iter().map(|(p, _)| PathBuf::from(p))
         .chain(bin_manifest.iter().map(|(p, _)| PathBuf::from(p)))
         .collect();
     ```
   - Keep the existing text-loop, but factor it into a closure or simply
     leave it and add a second loop after it for binary entries. The binary
     loop body is:
     ```rust
     for (rel, bytes) in &bin_manifest {
         let abs = root.join(rel);
         if !abs.exists() {
             ops.push(Operation::Add(PathBuf::from(rel)));
         } else {
             let on_disk = fs::read(&abs)?;
             if on_disk != *bytes {
                 ops.push(Operation::Update(PathBuf::from(rel)));
             }
         }
     }
     ```
     The text loop already does the equivalent for `content.as_bytes()`, so
     no logic change there.

3. In `apply`, lookup logic gains a fall-through to `binary_manifest()` when
   the path is not in `framework_manifest()`:
   ```rust
   let content_bytes: Vec<u8> = if let Some((_, txt)) = framework_manifest()
       .iter().find(|(p, _)| Path::new(p) == rel.as_path()) {
       txt.as_bytes().to_vec()
   } else if let Some((_, bytes)) = binary_manifest()
       .iter().find(|(p, _)| Path::new(p) == rel.as_path()) {
       bytes.to_vec()
   } else {
       anyhow::bail!("internal: no manifest entry for {}", rel.display());
   };
   …
   fs::write(&abs, &content_bytes)?;
   ```

   The existing `Operation::Remove` arm is unchanged — it doesn't need
   manifest content.

**Tests:** Add to `update.rs`'s `mod tests`:

1. `update_with_missing_font_proposes_add`:
   ```rust
   #[test]
   fn update_with_missing_font_proposes_add() {
       let (_dir, root) = scaffold();
       fs::remove_file(root.join(".zero/fonts/Geist-VariableFont_wght.woff2")).unwrap();
       let plan = compute_plan(&root).unwrap();
       assert!(plan.contains(&Operation::Add(
           PathBuf::from(".zero/fonts/Geist-VariableFont_wght.woff2"))),
           "plan missing Add for woff2 font: {plan:?}");
   }
   ```

2. `update_with_modified_font_proposes_update`: overwrite the woff2 with
   garbage bytes, run `compute_plan`, expect an `Update`.

3. `update_yes_flag_restores_binary_drift`: mirror the existing
   `update_yes_flag_applies_all_operations` test for a font drift case —
   delete one woff2, mutate `OFL.txt`, run `run_with(&root, true, …)`, assert
   convergence (empty plan) and that the woff2's bytes match the manifest.

4. `update_with_no_drift_reports_up_to_date` already exists; verify it still
   passes once the binary manifest is included (the freshly scaffolded
   project is byte-equal to both manifests).

---

### Step 6: Rename `copy_public` → `copy_tree` and copy `.zero/fonts/` to `dist/`

**Goal:** `zero build` produces a `dist/.zero/fonts/` directory next to
`dist/public/`, so the same `/.zero/fonts/...` URLs that work in dev resolve
in production output.

**Files:** `crates/zero/src/cmd/build.rs`.

**Changes:**

1. Rename the private helper `copy_public` to `copy_tree`. The body is
   unchanged — it already takes generic `src` / `dst` parameters and only
   the name tied it to `public/`. Update both the function definition and
   the existing call site inside `run`.

2. In `run`, after the existing `public_copied` block, add:
   ```rust
   let fonts_src = root.join(".zero").join("fonts");
   let fonts_copied = if fonts_src.is_dir() {
       copy_tree(&fonts_src, &out_dir.join(".zero").join("fonts"))?
   } else {
       0
   };
   ```

3. Extend the build summary `println!` to surface the new count:
   ```rust
   println!(
       "zero build — {} bytes JS, {} CSS file(s), {} public asset(s), {} font asset(s); output in {}/",
       bundle_src.len(), css_pairs.len(), public_copied, fonts_copied, out_dir.display());
   ```

**Tests:**

1. Update `copy_public_recurses_and_counts_files` → rename to
   `copy_tree_recurses_and_counts_files` (call site uses `copy_tree`). Body
   is otherwise unchanged.

2. Add `build_copies_dot_zero_fonts_into_dist`:
   ```rust
   #[tokio::test]
   async fn build_copies_dot_zero_fonts_into_dist() {
       let tmp = tempfile::tempdir().unwrap();
       let _g = CwdGuard::enter(tmp.path());
       write_minimal_project(tmp.path());
       let fonts_dir = tmp.path().join("web").join(".zero").join("fonts");
       std::fs::create_dir_all(&fonts_dir).unwrap();
       std::fs::write(fonts_dir.join("Geist-VariableFont_wght.woff2"), b"stub").unwrap();
       super::run(None).await.unwrap();
       let out = tmp.path().join("dist").join(".zero").join("fonts");
       assert!(out.join("Geist-VariableFont_wght.woff2").is_file(),
           "font not copied to dist/.zero/fonts/");
   }
   ```

---

### Step 7: Create `.zero/styles/_typography.scss` and wire it in

**Goal:** Ship the twelve utility classes that replace the deleted element
rules, wrapped in `@layer components` so unlayered user CSS in
`styles/app.scss` wins on override. Done before Step 8 deletes the element
selectors so the cumulative state of the codebase between steps 7 and 8 is
"both the old element rules and the new utility classes apply" — a strictly
additive intermediate state.

**Files:**
- Create `crates/zero-scaffold/src/scaffold/.zero/styles/_typography.scss`.
- Edit `crates/zero-scaffold/src/scaffold/.zero/styles/zero.scss`.
- Edit `crates/zero-scaffold/src/lib.rs`.

**Changes:**

1. Write `_typography.scss` with the twelve classes from spec §"Create
   `.zero/styles/_typography.scss`", wrapped in `@layer components { … }`.
   No `%heading-base` placeholder — the optional consolidation gives no
   compiled-CSS savings and adds indirection. Classes:
   `.text-display`, `.text-h1`, `.text-h2`, `.text-h3`, `.text-h4`,
   `.text-eyebrow`, `.text-body`, `.text-small`, `.text-muted`,
   `.text-code`, `.text-link`, `.divider`. Use exact property values from
   the spec, including `margin: 0` (not `margin-block: 0`) and
   `var(--color-primary-hover)` inside `.text-link:hover`.

2. In `zero.scss`, insert `@use 'typography';` on a new line between
   `@use 'alignment';` and `@use 'components';`.

3. In `lib.rs`:
   - Add `const TPL_TYPOGRAPHY_SCSS: &str = include_str!("scaffold/.zero/styles/_typography.scss");`
     next to the other style consts.
   - Add `(".zero/styles/_typography.scss", TPL_TYPOGRAPHY_SCSS),` to the
     `framework_manifest()` vec, placed between the `_alignment.scss` and
     `_components.scss` entries.
   - Update the `framework_manifest_matches_expected_path_set` test's
     `expected` set to include `".zero/styles/_typography.scss"`.

**Tests:**

1. Extend `zero_scss_contains_aggregate_uses` with `"@use 'typography'"`.

2. Add `write_initial_project_emits_typography_partial`:
   ```rust
   #[test]
   fn write_initial_project_emits_typography_partial() {
       let (_dir, root) = fresh_scaffold();
       let typo = fs::read_to_string(root.join(".zero/styles/_typography.scss")).unwrap();
       assert!(typo.contains("@layer components"), "missing @layer components");
       for cls in [".text-display", ".text-h1", ".text-h2", ".text-h3", ".text-h4",
                   ".text-eyebrow", ".text-body", ".text-small", ".text-muted",
                   ".text-code", ".text-link", ".divider"] {
           assert!(typo.contains(cls), "_typography.scss missing {cls}: {typo}");
       }
       assert!(!typo.contains("!important"));
   }
   ```

---

### Step 8: Edit `_base.scss` — remove Google Fonts import + element selectors, add `@font-face` blocks

**Goal:** Cut the network dependency, cut the unconditional typographic
restyling of every page's heading and paragraph elements, and replace both
with local font-face declarations.

**Files:** `crates/zero-scaffold/src/scaffold/.zero/styles/_base.scss`.

**Changes:** Rewrite the file to contain, in order:

1. Updated header comment:
   ```scss
   // Reset + token-bound body rule + global :focus-visible ring +
   // reduced-motion override. Opinionated typography lives in
   // _typography.scss; this file holds only browser-reset-level rules
   // and a11y / OS-pref respect.
   ```

2. Four `@font-face` blocks (Geist normal, Geist italic, Geist Mono normal,
   Geist Mono italic) with `src: url("/.zero/fonts/<file>.woff2")
   format("woff2-variations");`, `font-weight: 100 900;`, `font-style:
   normal` or `italic`, and `font-display: swap;`. Use the exact text from
   the spec's "_base.scss font-face declarations" section.

3. Keep `*, *::before, *::after { box-sizing: border-box; }`.

4. Keep the `body { … }` rule **verbatim** (including
   `font-feature-settings: "ss01", "cv11";` and the two `font-smoothing`
   declarations).

5. **Delete** the following rule blocks entirely:
   - `h1, h2, h3, h4, h5, h6 { … }`
   - Individual `h1 { … }` through `h6 { … }`
   - `p { … }`
   - `small { … }`
   - `code, kbd, samp, pre { … }`
   - `code { padding: …; background: …; … }`
   - `a { … }` and `a:hover { … }`
   - `hr { … }`

6. Keep `:focus { outline: none; }` and the
   `:where(button, a, input, textarea, select, summary, [tabindex]):focus-visible { … }`
   rule.

7. Keep the `@media (prefers-reduced-motion: reduce) { … }` block.

After this step the only top-level selectors remaining in `_base.scss` are:
the `*, *::before, *::after` reset, `body`, `:focus`, the `:where(…)` ring,
and the four `@font-face`. No raw `h1`/`p`/`a`/etc. anywhere.

**Tests:** No new test in this file's own tests block; coverage of the
final compiled CSS shape lives in Step 10's framework-side integration
test. Existing `write_initial_project_emits_framework_files` continues to
pass — it only asserts `!base_scss.is_empty()`.

---

### Step 9: Migrate showcase + examples bare headings to utility classes

**Goal:** Restore the visual treatment of headings, paragraphs, and links
in the showcase + examples now that `_base.scss` no longer styles them.

**Files:** All `.ts` files under:
- `showcase/src/routes/`
- `examples/counter/web/src/**`
- `examples/todos/web/src/**`
- `examples/tracker/web/src/**`

**Changes:** For each file, audit every bare `<h1>` / `<h2>` / `<h3>` /
`<h4>` / `<h5>` / `<h6>` / `<p>` / `<a>` / `<small>` / `<code>` / `<hr>`
that visually depended on the deleted element rules and add an appropriate
utility class:

- `<h1>` headlines → `<h1 class="text-display">` for the largest hero or
  `<h1 class="text-h1">` for routine page titles.
- `<h2>` → `<h2 class="text-h2">`.
- `<h3>` → `<h3 class="text-h3">`.
- `<h4>` → `<h4 class="text-h4">`.
- `<h5>` → `<h4 class="text-h4">` (no separate h5 utility; size collapses to h4 size as in deleted rules).
- `<h6>` → `<h6 class="text-eyebrow">` (matches the deleted h6 uppercase + tracking-caps + text-muted styling).
- Body `<p>` carrying framework-default styling (color/leading) → `<p class="text-body">`.
- `<small>` → `<small class="text-small">`.
- Inline `<code>` styled as a chip → `<code class="text-code">`.
- `<a>` rendered as a visible link (not inside a Button) → `<a class="text-link">`. Skip `<a>` tags that already carry a component class such as `class="button"`.
- `<hr>` → `<hr class="divider">`.

Routes that only contain plain form structure or component compositions
need no edits — only the visible-typography sites change. Pick the heading
level per the existing semantics; do not rewrite the tag for visual reasons
(that's the point of the refactor).

When a single `<h1>` already has another class (e.g. `class="hero-title"`),
add the utility alongside: `class="hero-title text-display"`.

Inspect each file once; many showcase route files are nearly identical in
shape, and edits are mechanical.

**Tests:** No new unit tests for the showcase/example contents. The
existing integration tests in `crates/zero/tests/showcase_build.rs`,
`tests/showcase_dev.rs`, `tests/examples_build.rs`,
`tests/examples_tests.rs` continue to assert that build + dev-server +
test-runner succeed; they catch any class typo that breaks compilation.

---

### Step 10: Framework-side integration test asserting compiled CSS shape

**Goal:** Lock the spec's three load-bearing shape invariants into an
automated test so a future polish pass cannot silently re-introduce element
selectors or a Google Fonts URL.

**Files:** `crates/zero-scaffold/src/lib.rs` (new test in `mod tests`).

**Changes:** Add `zero-sass` (or the existing SCSS compile facility — the
crate that backs `serve_under_with_sass`) as a `[dev-dependencies]` entry
in `crates/zero-scaffold/Cargo.toml` if not already present, so tests can
compile `.zero/styles/zero.scss` from a freshly-scaffolded project.

Add a new test:

```rust
#[test]
fn compiled_zero_css_has_typography_and_fonts_and_no_element_selectors() {
    let (_dir, root) = fresh_scaffold();
    let app_scss = root.join("styles").join("app.scss");
    // Compile via the same path used by `serve_under_with_sass`:
    let css = zero_sass::compile_scss_file(&app_scss).unwrap();

    // (a) every typography class appears
    for cls in [".text-display", ".text-h1", ".text-h2", ".text-h3", ".text-h4",
                ".text-eyebrow", ".text-body", ".text-small", ".text-muted",
                ".text-code", ".text-link", ".divider"] {
        assert!(css.contains(cls), "compiled CSS missing {cls}");
    }

    // (b) all four @font-face declarations present
    let face_count = css.matches("@font-face").count();
    assert!(face_count >= 4, "expected >=4 @font-face, got {face_count}");
    assert_eq!(css.matches("font-family: \"Geist\"").count() +
               css.matches("font-family:\"Geist\"").count(), 2,
               "expected 2 Geist faces (normal + italic)");
    assert_eq!(css.matches("font-family: \"Geist Mono\"").count() +
               css.matches("font-family:\"Geist Mono\"").count(), 2,
               "expected 2 Geist Mono faces");

    // (c) no Google Fonts URL
    assert!(!css.contains("fonts.googleapis.com"),
        "compiled CSS still imports Google Fonts");

    // (d) no top-level bare element selectors for typography tags
    for sel in ["\nh1 ", "\nh2 ", "\nh3 ", "\nh4 ", "\nh5 ", "\nh6 ",
                "\np ", "\nsmall ", "\nhr ", "\nh1{", "\nh2{", "\np{", "\nhr{"] {
        assert!(!css.contains(sel),
            "compiled CSS contains forbidden element selector {sel:?}");
    }
    // `a` selectors are still acceptable inside the `:where(...)` ring;
    // assert no top-level bare `a {` or `a:hover` rule.
    assert!(!css.contains("\na {") && !css.contains("\na:hover"),
        "compiled CSS still has bare a/a:hover rule");
    // `code, kbd, samp, pre` rule must not survive.
    assert!(!css.contains("code, kbd, samp, pre"),
        "compiled CSS still groups code/kbd/samp/pre");
}
```

If `zero_sass::compile_scss_file` isn't a public surface, expose the
existing internal helper from `crates/zero-sass/` behind a `pub` (or use a
test-only adapter). The test does not need to bring up an HTTP server — it
just runs the SCSS compiler.

**Tests:** The test above is the deliverable for this step.

---

### Step 11: Documentation updates

**Goal:** Document the new utility set, the local font story, and the
"no element selectors" stance so a developer who lands here through
`zero-framework-spec.md` or `BEST_PRACTICES.md` finds correct guidance.

**Files:**
- `zero-framework-spec.md`
- `BEST_PRACTICES.md`
- `crates/zero-scaffold/src/scaffold/AGENTS.md`

**Changes:**

1. **`zero-framework-spec.md` §7.1 Design system**: insert a new paragraph
   immediately before or after the existing cascade-layer paragraph that
   reads roughly:

   > **Typography.** The framework ships twelve utility classes —
   > `.text-display`, `.text-h1`–`.text-h4`, `.text-eyebrow`, `.text-body`,
   > `.text-small`, `.text-muted`, `.text-code`, `.text-link`, `.divider` —
   > inside `.zero/styles/_typography.scss`, wrapped in `@layer components`.
   > Pick a tag for semantics (e.g. `<h1>` for page outline) and a class
   > for visual intent (`class="text-display"` for hero size). There are
   > no opinionated rules on bare element selectors in `_base.scss`;
   > unstyled `<h1>` renders with browser defaults.
   >
   > **Fonts.** Geist (sans, both styles) and Geist Mono (mono, both
   > styles) ship locally in `.zero/fonts/` as four variable-axis `.woff2`
   > files. `_base.scss` declares the four `@font-face` blocks against
   > `/.zero/fonts/...`. No network round-trip to Google Fonts.

2. **`BEST_PRACTICES.md` §7 (or §8 — wherever "Styles" / "Theming" lives)**:
   add a "Typography" subsection under the styles guidance with one short
   example:

   ```html
   <!-- semantic h1 for outline, display-size visual treatment -->
   <h1 class="text-display">Hello, world.</h1>

   <!-- inline code that should look like a chip -->
   Use <code class="text-code">signal</code> for reactive state.

   <!-- opt-in link styling -->
   See the <a class="text-link" href="/spec">spec</a>.
   ```

   Plus one sentence on the "tag for semantics, class for visual intent"
   rule.

3. **`crates/zero-scaffold/src/scaffold/AGENTS.md`** under `## Styles`: list
   the typography utility classes and where they live (`_typography.scss`).
   One short paragraph + a bullet list of the twelve class names with a
   one-line description each.

**Tests:** `write_initial_project_agents_md_has_section_sentinels` is
unaffected (no new sentinel headings). No new tests; this step is pure
documentation. The existing showcase / example tests retain their guarantee
that the framework still compiles end-to-end.

---

## Risks and Assumptions

- **`include_bytes!` resolution.** `include_bytes!("scaffold/.zero/fonts/…")`
  resolves relative to `crates/zero-scaffold/src/lib.rs`, the same as
  `include_str!`. If for some reason Cargo's build context is different
  for binary versus text embeds, Step 4 will not compile. Mitigation:
  verify by running `cargo build -p zero-scaffold` after Step 4 before
  moving on; the failure mode is a compile error with the missing path
  printed.

- **`zero-sass` test dependency.** Step 10 assumes `zero-sass` exposes (or
  can expose) a function the scaffold crate can call in `#[cfg(test)]`.
  If exposing the helper is intrusive, the alternative is to keep the
  test inside `zero-sass` itself (its inputs are just the scaffold tree;
  `zero-scaffold` is already a `dev-dependency` of nothing else relevant)
  or to move it into a workspace-level `crates/zero/tests/` integration
  test. Either swap is mechanical.

- **Showcase / examples visual regressions.** Step 9 is by-eye work. Some
  routes may continue to render fine without any class (because the
  surrounding component absorbs the styling), so the audit may miss a
  heading. Mitigation: visually inspect a `cargo run -- dev` pass over the
  showcase before declaring done.

- **`framework_manifest()` and `binary_manifest()` divergence.** The
  parallel-manifest decision means two functions, two sets of tests, two
  iteration loops, two manifest-path-set assertions. If a third asset
  family lands (e.g. images), this duplication compounds. The unified
  `FileContents` enum is deferred per spec §"Open Questions"; revisit at
  that point.

- **Binary embed budget.** `cargo build --release` will grow the `zero`
  binary by ~330KB. The CLI's release size is well below any tooling
  threshold today; no further action.

- **OFL.txt license placement.** SIL OFL 1.1 requires the license travel
  with the font. Embedding it in `binary_manifest()` and writing it next
  to the woff2 files in every scaffolded project satisfies that.

- **Backward compat.** Users who already ran `zero init` on a previous
  framework version will see, after `zero update`: a new
  `.zero/fonts/` directory (Adds), updated `_base.scss` (Update), new
  `_typography.scss` (Add), updated `zero.scss` (Update). Their existing
  `<h1>` / `<p>` / `<a>` tags now render with browser defaults until they
  apply the new utility classes. This is the documented and accepted
  migration signal per spec §Constraints "Pre-1.0 compatibility stance."
