# Spec: Local Geist fonts + typography utilities

## Problem Statement

Two issues in `.zero/styles/_base.scss` after the recent design-system polish pass:

1. **Fonts are loaded from Google Fonts over the network.** `_base.scss` starts with `@import url("https://fonts.googleapis.com/css2?family=Geist...&family=Geist+Mono...")`. This violates the framework's zero-dependency stance: every scaffolded project now silently depends on Google's CDN, fails offline, leaks user IPs to Google on every page load, and races font loading against the rest of the boot. The user has the four canonical Geist `.woff2` files available locally and wants them bundled with the framework binary so every scaffolded project ships with the fonts on disk.

2. **Typography defaults apply via bare element selectors.** The polish pass added unconditional rules on `h1`–`h6`, `p`, `small`, `code`/`kbd`/`samp`/`pre`, `a`, and `hr` in `_base.scss`. Any `<h1>` anywhere in a user's app is now restyled by the framework, even when the author wants the tag for outline/semantics only. This conflicts with the framework's "no magic" stance (§Philosophy) and with §7.1's cascade-layer policy where component-level appearance lives in `@layer components`, not in unlayered global selectors. The defaults themselves are wanted — the *delivery mechanism* (element selectors) is wrong.

The polish pass's other additions to `_base.scss` (global `:focus-visible` ring scoped via `:where(...)`, `prefers-reduced-motion` blanket, `box-sizing` reset, `body` token-binding, body-level font-smoothing/feature-settings) are kept as-is — they are true browser-default resets and a11y/OS-pref respect, not opinionated typography. Likewise the recent `_tokens.scss`, `_themes.scss`, `_palette.scss`, and `themes/_light.scss` / `themes/_dark.scss` polish is in scope only where the typography utilities consume new tokens; the rest of those files is accepted as-is.

## Background

### Current state

- `.zero/styles/_base.scss` currently contains:
  - `@import url("https://fonts.googleapis.com/css2?family=Geist...&family=Geist+Mono...&display=swap")` at the top — the change being reverted.
  - `*, *::before, *::after { box-sizing: border-box; }` — kept.
  - `body { ... }` rule binding `--color-text`, `--color-bg`, `--font-sans`, `--font-size-md`, `--leading-normal`, plus `font-feature-settings: "ss01", "cv11"` and `-webkit-font-smoothing: antialiased` / `-moz-osx-font-smoothing: grayscale` — kept.
  - Element rules for `h1`, `h2`, `h3`, `h4`, `h5`, `h6`, `p`, `small`, `code, kbd, samp, pre`, `code` standalone, `a` and `a:hover`, `hr` — **deleted by this change**.
  - `:focus { outline: none; }` + `:where(button, a, input, textarea, select, summary, [tabindex]):focus-visible { ... }` — kept.
  - `@media (prefers-reduced-motion: reduce) { *, *::before, *::after { animation-duration: 0.01ms !important; ... } }` — kept.

- `crates/zero-scaffold/src/scaffold/.zero/styles/` already contains two of the four needed woff2 files (`Geist-VariableFont_wght.woff2`, `Geist-Italic-VariableFont_wght.woff2`) but in the **wrong location** — they are not served by the dev server, not copied by `zero build`, and not in the framework manifest. They will be moved to `.zero/fonts/` as part of this change. The other two Mono woff2 files come from `~/Documents/code/zero_claude_design_system_files/Geist_Mono/`.

- `_tokens.scss` already declares `--font-sans: "Geist", -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif;` and `--font-mono: "Geist Mono", ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;`. The font-family declarations stay; only the **loading mechanism** changes (local `@font-face` instead of Google Fonts URL).

- Scaffold distribution: `crates/zero-scaffold/src/lib.rs` defines `framework_manifest()` returning `Vec<(&'static str, &'static str)>` — the canonical list of framework-owned **text** files written into `.zero/`. The function is consulted by `zero init` (writes everything) and `zero update` (diffs and refreshes everything). It uses `include_str!` for embedding, which is text-only. There is currently **no** binary-asset path: no `include_bytes!` calls, no parallel manifest for binary files.

- Dev server routes (`crates/zero-dev/src/server.rs:185-238`):
  - `/src/*path` → `<root>/src/` (with TS transpile)
  - `/styles/*path` → `<root>/styles/` (with SCSS compile)
  - `/public/*path` → `<root>/public/`
  - `/.zero/components/*path` → `<root>/.zero/components/` (with TS transpile)
  - No route exists for `/.zero/fonts/*path` or for any `.zero/` subdirectory other than `components/`.

- Build pipeline (`crates/zero/src/cmd/build.rs:55-83`): bundles `src/`, compiles top-level SCSS, renders `index.html`, copies `public/` recursively. **Does not copy any `.zero/` subdirectory** into the output. The font files need a new copy step or they will not exist in production output.

- MIME types (`crates/zero-dev/src/files.rs:18-37`): `content_type_for` knows js/mjs/css/scss/html/json/svg/png/jpg/ico/txt. Unknown extensions return `application/octet-stream`. `woff2` is missing — modern browsers tolerate octet-stream for fonts, but `font/woff2` is the correct MIME and the standard pattern is to set it explicitly.

- Cascade policy: `zero-framework-spec.md` §7.1 documents that component partials are wrapped in `@layer components { ... }` so user CSS in `styles/app.scss` (unlayered) wins on override without `!important`. Typography utilities follow the same model.

- The user's `styles/app.scss` is the one-shot user-owned entry: `@use '../.zero/styles/zero';`. Untouched by this change.

### Inspiration

Tailwind's `prose` class is the canonical "opt into a typography stylesheet" model — bare `<h1>` is unstyled, but `<article class="prose"><h1>...</h1></article>` gets the full typographic treatment via a single class on the container. We do not adopt the scoped-container model (descendant selectors inside `.prose` re-introduce element selectors); instead each typographic intent gets its own class applied directly to the element. The result is the same opt-in property without the descendant-selector tax.

## Requirements

### Part 1 — Bundle Geist fonts as framework-owned binary assets

#### Source-tree layout (in the zero framework repo)

```
crates/zero-scaffold/src/scaffold/.zero/fonts/
  Geist-VariableFont_wght.woff2          # sans, upright, weight 100–900
  Geist-Italic-VariableFont_wght.woff2   # sans, italic, weight 100–900
  GeistMono-VariableFont_wght.woff2      # mono, upright, weight 100–900
  GeistMono-Italic-VariableFont_wght.woff2  # mono, italic, weight 100–900
  OFL.txt                                # SIL Open Font License text
```

The four `.woff2` files are committed to the zero framework repo. The user's local copies of the two sans files (currently sitting at `crates/zero-scaffold/src/scaffold/.zero/styles/`) move to the new location; the two Mono files come from `~/Documents/code/zero_claude_design_system_files/Geist_Mono/`. The license text comes from `~/Documents/code/zero_claude_design_system_files/Geist/OFL.txt` (same license covers both — Geist is SIL OFL 1.1).

File names preserve the upstream Vercel-Geist filenames verbatim so the source of the woff2 files stays obvious. (Renaming to shorter slugs like `geist.woff2` is a one-line `include_bytes!` change later if desired; out of scope here.)

#### Binary manifest in `crates/zero-scaffold/src/lib.rs`

A new public function `binary_manifest() -> Vec<(&'static str, &'static [u8])>` parallel to the existing `framework_manifest()`. Returns five entries:

```rust
pub fn binary_manifest() -> Vec<(&'static str, &'static [u8])> {
    vec![
        (".zero/fonts/Geist-VariableFont_wght.woff2",         GEIST_WOFF2),
        (".zero/fonts/Geist-Italic-VariableFont_wght.woff2",  GEIST_ITALIC_WOFF2),
        (".zero/fonts/GeistMono-VariableFont_wght.woff2",     GEIST_MONO_WOFF2),
        (".zero/fonts/GeistMono-Italic-VariableFont_wght.woff2", GEIST_MONO_ITALIC_WOFF2),
        (".zero/fonts/OFL.txt",                                OFL_TXT),
    ]
}
```

Where each constant is a module-level `const X: &[u8] = include_bytes!("scaffold/.zero/fonts/...");`. The OFL.txt entry is embedded as bytes (not str) for shape consistency; the planner may opt to keep it text and concatenate manifests instead — either is acceptable as long as a fresh `zero init` writes the license file alongside the woff2 files.

`write_framework_files(root_dir)` in `crates/zero-scaffold/src/lib.rs` writes every entry in `binary_manifest()` in addition to every entry in `framework_manifest()`, creating parent directories as needed. `fs::write` accepts both `&str` and `&[u8]` — no separate write path needed.

`zero update` diff/plan/apply logic in `crates/zero/src/cmd/update.rs` (and the `Operation` reporting in `crates/zero-scaffold/src/lib.rs`) extends to binary entries: an `Add`/`Update`/`Remove` line shows the relative path the same way it does for text files. Byte equality is used for the "already up to date" check on binary entries instead of string equality.

#### Dev server route

Add to `crates/zero-dev/src/server.rs` `build_app()`:

```rust
.route(
    "/.zero/fonts/*path",
    get(
        |State(s): State<Arc<AppState>>, Path(p): Path<String>| async move {
            serve_under(s.root.join(".zero").join("fonts"), "/.zero/fonts", &format!("/.zero/fonts/{p}")).await
        },
    ),
)
```

Plain `serve_under` (no transpile, no SCSS) is correct — woff2 files are served as-is.

#### MIME type for woff2

Add to `crates/zero-dev/src/files.rs::content_type_for`:

```rust
Some("woff2") => "font/woff2",
Some("woff")  => "font/woff",
Some("ttf")   => "font/ttf",
Some("otf")   => "font/otf",
```

`woff` / `ttf` / `otf` are added defensively even though only `woff2` ships today — the surface is one trivial line per case and avoids a future "why isn't my font loading" gotcha when someone drops a `.ttf` into `public/fonts/`.

#### Build copy

In `crates/zero/src/cmd/build.rs::run`, after the existing `copy_public` step, add a font copy step:

```rust
let fonts_src = root.join(".zero").join("fonts");
let fonts_copied = if fonts_src.is_dir() {
    copy_tree(&fonts_src, &out_dir.join(".zero").join("fonts"))?
} else {
    0
};
```

`copy_tree` is the existing `copy_public` helper, renamed or generalized to copy any directory. (It already recurses and counts files — its current name is the only thing tying it to `public/`.) The build summary line gains a `{fonts_copied} font asset(s)` segment.

Output URLs match: in dev `/.zero/fonts/Geist-VariableFont_wght.woff2` is served from `<root>/.zero/fonts/`; in prod the same URL resolves to `<dist>/.zero/fonts/`.

#### `_base.scss` font-face declarations

Replace the deleted `@import url("https://fonts.googleapis.com/...")` line with four `@font-face` blocks at the top of `_base.scss`:

```scss
@font-face {
  font-family: "Geist";
  src: url("/.zero/fonts/Geist-VariableFont_wght.woff2") format("woff2-variations");
  font-weight: 100 900;
  font-style: normal;
  font-display: swap;
}
@font-face {
  font-family: "Geist";
  src: url("/.zero/fonts/Geist-Italic-VariableFont_wght.woff2") format("woff2-variations");
  font-weight: 100 900;
  font-style: italic;
  font-display: swap;
}
@font-face {
  font-family: "Geist Mono";
  src: url("/.zero/fonts/GeistMono-VariableFont_wght.woff2") format("woff2-variations");
  font-weight: 100 900;
  font-style: normal;
  font-display: swap;
}
@font-face {
  font-family: "Geist Mono";
  src: url("/.zero/fonts/GeistMono-Italic-VariableFont_wght.woff2") format("woff2-variations");
  font-weight: 100 900;
  font-style: italic;
  font-display: swap;
}
```

`format("woff2-variations")` advertises variable-weight support so browsers don't refetch. `font-display: swap` shows the fallback first and swaps to Geist when ready — same UX as the prior Google Fonts URL.

The body rule's `font-feature-settings: "ss01", "cv11";` line stays — Geist's stylistic alternates apply once the font loads. `font-smoothing` declarations stay.

### Part 2 — Move typography from element selectors to utility classes

#### Delete from `_base.scss`

Remove these rule blocks entirely:

- `h1, h2, h3, h4, h5, h6 { margin: 0; font-weight: ...; line-height: ...; color: ...; letter-spacing: ...; }`
- Individual `h1 { ... }`, `h2 { ... }`, `h3 { ... }`, `h4 { ... }`, `h5 { ... }`, `h6 { ... }`
- `p { margin: 0; color: var(--color-text); }`
- `small { font-size: ...; color: ...; }`
- `code, kbd, samp, pre { font-family: ...; font-size: ...; }`
- `code { padding: ...; background: ...; border-radius: ...; border: ...; }`
- `a { color: ...; text-decoration: ...; text-decoration-thickness: ...; text-underline-offset: ...; transition: ...; }`
- `a:hover { color: ...; }`
- `hr { border: none; border-block-start: ...; margin-block: ...; }`

What remains in `_base.scss`: the file header comment (rewritten — see below), `* { box-sizing }`, the `body { ... }` rule, the `:focus-visible` ring, the reduced-motion blanket.

The file header comment is rewritten to reflect the narrower scope, e.g.:

```scss
// Reset + token-bound body rule + global :focus-visible ring +
// reduced-motion override. Opinionated typography lives in
// _typography.scss; this file holds only browser-reset-level rules
// and a11y / OS-pref respect.
```

#### Create `.zero/styles/_typography.scss`

A new framework partial, wrapped in `@layer components { ... }` so user CSS in unlayered `styles/app.scss` wins on override. Twelve classes total:

```scss
@layer components {
  // Display + heading variants — set size, weight, leading, tracking,
  // and reset the default margin. Choose a tag by semantics (h1–h6, p,
  // span, …) and a class by visual intent.
  .text-display {
    font-size: var(--font-size-display);
    font-weight: var(--weight-bold);
    line-height: var(--leading-tight);
    letter-spacing: var(--tracking-tight);
    margin: 0;
    color: var(--color-text);
  }
  .text-h1 {
    font-size: var(--font-size-2xl);
    font-weight: var(--weight-semi);
    line-height: var(--leading-tight);
    letter-spacing: var(--tracking-snug);
    margin: 0;
    color: var(--color-text);
  }
  .text-h2 {
    font-size: var(--font-size-xl);
    font-weight: var(--weight-semi);
    line-height: var(--leading-tight);
    letter-spacing: var(--tracking-snug);
    margin: 0;
    color: var(--color-text);
  }
  .text-h3 {
    font-size: var(--font-size-lg);
    font-weight: var(--weight-semi);
    line-height: var(--leading-tight);
    letter-spacing: var(--tracking-snug);
    margin: 0;
    color: var(--color-text);
  }
  .text-h4 {
    font-size: var(--font-size-md);
    font-weight: var(--weight-semi);
    line-height: var(--leading-tight);
    letter-spacing: var(--tracking-snug);
    margin: 0;
    color: var(--color-text);
  }
  .text-eyebrow {
    font-size: var(--font-size-sm);
    font-weight: var(--weight-semi);
    text-transform: uppercase;
    letter-spacing: var(--tracking-caps);
    color: var(--color-text-muted);
    margin: 0;
  }

  // Body + supporting variants.
  .text-body {
    font-size: var(--font-size-md);
    font-weight: var(--weight-normal);
    line-height: var(--leading-normal);
    color: var(--color-text);
    margin: 0;
  }
  .text-small {
    font-size: var(--font-size-sm);
    color: var(--color-text-muted);
  }
  .text-muted {
    color: var(--color-text-muted);
  }

  // Inline code.
  .text-code {
    font-family: var(--font-mono);
    font-size: 0.9em;
    padding: 0.1em 0.35em;
    background: var(--color-surface);
    border-radius: var(--radius-sm);
    border: var(--border-thin) solid var(--color-border);
  }

  // Anchor styling — opt-in. Use on any <a> that should look like a
  // link rather than rely on a bare <a> being styled by default.
  .text-link {
    color: var(--color-primary);
    text-decoration: underline;
    text-decoration-thickness: 1px;
    text-underline-offset: 0.18em;
    transition: color var(--duration-fast) var(--ease-out);

    &:hover {
      color: var(--color-primary-hover);
    }
  }

  // Horizontal divider for <hr> or any block-level separator.
  .divider {
    border: none;
    border-block-start: var(--border-thin) solid var(--color-border);
    margin-block: var(--space-md);
  }
}
```

Exact values mirror what `_base.scss` declared on the corresponding elements, with one drift: `.text-link` references `var(--color-primary-hover)` (a token the recent theme polish added) instead of re-deriving a hover color. That token exists in both `_light.scss` and `_dark.scss`, so it resolves correctly under both themes.

The planner may consolidate the shared `margin: 0; color: var(--color-text); line-height: ...; letter-spacing: ...; font-weight: ...` across `.text-display`/`.text-h1`–`.text-h4` into a `%heading-base` SCSS placeholder selector (`%heading-base { ... }`, then `.text-display { @extend %heading-base; ... }`) if doing so reduces the file without affecting the compiled CSS shape. Optional, not load-bearing.

#### Aggregator update

`.zero/styles/zero.scss` gains one line:

```scss
@use 'palette';
@use 'tokens';
@use 'themes';
@use 'base';
@use 'layout';
@use 'utilities';
@use 'alignment';
@use 'typography';   // NEW
@use 'components';
```

`@use 'typography'` is placed after `@use 'alignment'` and before `@use 'components'`. Order within the `@layer components` block doesn't matter for typography vs. shipped components (the layer is the same), but textual proximity matches the "appearance utilities before components" reading order.

#### Scaffold manifest update

`framework_manifest()` in `crates/zero-scaffold/src/lib.rs` gains one new entry:

```rust
(".zero/styles/_typography.scss", TPL_TYPOGRAPHY_SCSS),
```

Plus a new `const TPL_TYPOGRAPHY_SCSS: &str = include_str!("scaffold/.zero/styles/_typography.scss");`.

Existing entries for `_base.scss` and `zero.scss` are not added — those already exist; their content changes (caught by `zero update`'s byte-equality diff).

### Part 3 — Component partials' use of element-internal styling

The component partials under `.zero/styles/components/_*.scss` are *not* required to switch from any element selectors they may have to utilities — those rules are scoped to a single component class (e.g. `.button h3 { ... }` inside `_button.scss`) and the principle being applied is "no global element selectors in `_base.scss`," not "no descendant element selectors anywhere." If a component partial happens to style an inner `<h2>` or `<a>` already scoped to its parent class, that stays.

If a planner discovers a global (unscoped) element selector inside a component partial during the change, it is flagged but not fixed in this spec — that is a separate audit.

### Part 4 — Showcase, examples, and docs

Three other touchpoints in the repo reference the typography surface or font setup:

- **`showcase/`** — exercises every shipped component for visual review. Currently renders headings via bare `<h1>` / `<h2>` / `<h3>` tags relying on the element-selector defaults. After this change, those bare tags render with browser defaults (no margin reset, no theme color, no Geist sizing). The showcase must switch to `class="text-display"` / `class="text-h1"` / etc. on its existing headings. The planner reviews `showcase/src/` and patches each bare heading. No new pages — this is a cleanup pass.

- **`examples/{counter,todos,tracker}/web/src/...`** — same audit, same fix. Any bare `<h1>`/`<h2>`/`<p>`/`<a>` in the three example apps that visibly depended on framework typography defaults gets the appropriate `.text-*` class. Routes that just render plain HTML form structure may not need any changes.

- **`zero-framework-spec.md` §7.1** — the "Typography" or design-system section gets a short paragraph documenting the utility class set and the "no element selectors" stance. **`BEST_PRACTICES.md`** — add a "Typography" subsection under styles describing how to pick a tag for semantics and a class for visual intent (one short example). **`src/scaffold/AGENTS.md`** — add to the `## Styles` section that typography utilities live in `_typography.scss` and list the class names.

### Part 5 — Tests

- **`crates/zero-scaffold/src/lib.rs` test module** — `write_initial_project_emits_framework_files` asserts the new typography partial is written and non-empty, and asserts the four woff2 files plus `OFL.txt` exist under `.zero/fonts/` and are non-empty after `write_initial_project`. `framework_manifest_matches_expected_path_set` gains `.zero/styles/_typography.scss`. A new test `binary_manifest_matches_expected_paths` asserts the five binary entries.

- **`crates/zero-scaffold/src/lib.rs::write_framework_files_writes_only_dot_zero`** — already asserts every framework file lands under `.zero/`. Re-verify with the binary entries included (they're already under `.zero/fonts/`).

- **`crates/zero-dev/src/files.rs` test module (if present) or a new test** — assert `content_type_for(Path::new("a.woff2"))` returns `"font/woff2"`.

- **`crates/zero-dev/src/server.rs` integration tests** — a new test exercising `GET /.zero/fonts/Geist-VariableFont_wght.woff2` returns 200 with `content-type: font/woff2` and a non-empty body. Follows the pattern of existing dev-server route tests.

- **`crates/zero/src/cmd/build.rs` tests** — assert that running build over a project with `.zero/fonts/` produces `dist/.zero/fonts/*.woff2`. The existing `copy_public_recurses_and_counts_files` test stays; add `copy_fonts_emits_dist_fonts` modeled on it.

- **A new framework-side integration test** that compiles `.zero/styles/zero.scss` and asserts:
  - `.text-display`, `.text-h1`, `.text-h2`, `.text-h3`, `.text-h4`, `.text-eyebrow`, `.text-body`, `.text-small`, `.text-muted`, `.text-code`, `.text-link`, `.divider` all appear in the compiled CSS.
  - No top-level rule selector in the compiled CSS targets `h1`, `h2`, `h3`, `h4`, `h5`, `h6`, `p`, `small`, `code, kbd, samp, pre`, bare `code`, bare `a`, `a:hover`, or `hr` (i.e. no element-only selector outside of `body`, the `*` reset, and the `:where(...)` focus ring).
  - The compiled CSS contains all four `@font-face` declarations with `font-family: "Geist"` (×2, one normal + one italic) and `font-family: "Geist Mono"` (×2).
  - The compiled CSS does **not** contain `fonts.googleapis.com`.

- **Showcase + examples integration** — `tests/showcase_build.rs`, `tests/showcase_dev.rs`, `tests/examples_build.rs`, `tests/examples_tests.rs` continue to pass. Visual regressions inside the showcase / examples are caught by the human reviewer; automated tests assert that build succeeds and the dev server serves the fonts (already covered above).

## Constraints

- **Zero npm dependencies, zero network dependencies at runtime.** No Google Fonts URL. The framework binary contains the woff2 bytes; nothing on the network is needed to display Geist after `zero init`. (§Philosophy.)
- **`zero update` writes only inside `.zero/`.** Both text and binary entries land under `.zero/`. No user-owned files (index.html, tsconfig.json, styles/app.scss, src/*) are touched. (§7 themes-spec invariant.)
- **`@layer components` for typography utilities.** Matches the cascade-layer policy in §7.1: user CSS in unlayered `styles/app.scss` wins on override without `!important`.
- **No element selectors in `_typography.scss`.** Each utility targets a class, never a bare tag. The whole point of the refactor.
- **No `!important` anywhere new.** Same as the existing utility/component layer policy.
- **No JS runtime work.** Pure scaffold + dev-server + build-pipeline + SCSS changes. The runtime crates (`zero-runtime`, `zero-transpile`, `zero-bundler` JS bits) are untouched.
- **Binary embed budget.** Four woff2 files total ~330KB on disk (~68KB sans, ~72KB italic, ~95KB mono, ~95KB mono italic). The CLI binary grows by roughly that amount. Acceptable cost for the zero-dependency stance.
- **Font filenames preserved.** Upstream Vercel-Geist names ship verbatim (`Geist-VariableFont_wght.woff2`, etc.) so the provenance is obvious to anyone inspecting `.zero/fonts/`.
- **OFL.txt ships alongside.** SIL Open Font License 1.1 requires the license text to accompany the font. One file, ~4.5KB; included in `binary_manifest()`.
- **Pre-1.0 compatibility stance.** Existing user projects with bare `<h1>`–`<h6>`, `<p>`, `<a>`, `<code>`, `<hr>` will render with browser defaults after `zero update`. No migration tooling; the visual regression in user apps is the migration signal. (Documented in the §7.1 update.)

## Out of Scope

- **Hosting fonts as a CDN-style dist-only asset.** All fonts ship on disk in user projects under `.zero/fonts/` and get copied to `dist/`. No "fetch from cdn.zero.dev" mode.
- **Subsetting the Geist woff2 files.** The variable-weight files ship as-is. Subsetting to a specific Unicode range is a future-only optimization.
- **Static (non-variable) Geist weights.** Only the two variable files per family (upright + italic) ship. Users who want a single static weight pull it themselves and reference it from their own `@font-face` in `styles/app.scss`.
- **A `Heading` / `Text` component.** The opt-in is a CSS class, not a function component. (Option B from refinement was considered and declined.)
- **A `.prose` scoped block.** No descendant-element-selector mode. (Option C declined.)
- **Refactoring component partials that have internal element selectors.** Audit but do not fix in this round. (Part 3.)
- **Backward-compatibility shims** for the removed element selectors. Pre-1.0 framework, willing to break.
- **The other recent `_tokens.scss` / `_themes.scss` / `_palette.scss` polish changes.** Accepted as-is. Only the typography utilities consume any new tokens from that pass.
- **Renaming or generalizing the `copy_public` helper into a project-wide build infrastructure.** Build infrastructure stays minimal; the helper either gains a small rename or grows a sibling — planner's call.
- **MIME types beyond woff2 / woff / ttf / otf.** No audio, video, or wasm MIME registration.
- **Switching `.zero/fonts/` storage to a flatter layout (e.g. `.zero/assets/fonts/`).** Keep it shallow: `.zero/fonts/<file>.woff2`.
- **A `zero gen font` subcommand or a font-management API.** Out of scope. Users who want additional families author `@font-face` themselves in `styles/app.scss`.

## Open Questions

- **OFL.txt entry shape.** The license file is plain text but is grouped with the woff2 files for distribution. Planner picks: embed it via `include_bytes!` in `binary_manifest()` (consistent storage with the woff2 files) or via `include_str!` in `framework_manifest()` (consistent storage with other text files). Either works; user-visible outcome is identical.
- **`copy_public` rename vs. duplicate.** Two reasonable shapes for the build copy step: (a) rename `copy_public` to `copy_tree` and call it twice (once for `public/`, once for `.zero/fonts/`), or (b) leave `copy_public` alone and add a sibling `copy_fonts` with the same body. The planner picks based on what reads cleanly in `build.rs`.
- **Whether `framework_manifest()` and `binary_manifest()` should be merged.** A unified `(rel_path, FileContents)` where `FileContents` is `Text(&'static str)` | `Binary(&'static [u8])` is cleaner long-term, but is a bigger refactor than the change needs. Default: keep them parallel; revisit if a third asset family lands.
- **Whether to add `text-link-muted` / `text-link-subtle` variants.** Not in the 12-class set; if the showcase or examples reveal a need for non-primary-colored links during the cleanup pass, add one variant. Default: don't; the user re-colors at the call site.
- **Whether `.text-display` / `.text-h1`–`.text-h4` should reset `margin-block` instead of `margin`.** The deleted element rules used `margin: 0`. Modern style would use `margin-block: 0` to preserve any inline-axis margins. Pick `margin: 0` for byte-identical migration; planner may switch to `margin-block: 0` if it produces no behavioral difference in the showcase.
- **Naming of the showcase's heading audit.** Whether to do this as one PR or split. Suggest one PR — the typography classes are user-facing API, and shipping them without the showcase exercising them undermines the change.
