# Plan: CSS themes & expanded color palette

## Summary

Replace `.zero/styles/_tokens.scss`'s current single-file colors-plus-themes model with a two-tier system: a 55-token color palette under `_palette.scss` (gray/blue/red/green/amber × 11 steps, theme-invariant), a narrowed `_tokens.scss` that holds only non-color invariants (spacing, radius, font, etc., plus two new `--font-sans` / `--font-mono` tokens), and per-theme files under `themes/` that each define a single Sass `@mixin tokens` mapping the public `--color-*` semantic surface to palette steps. A small `_themes.scss` aggregator owns the selector strategy (`:root, [data-theme="light"]` + `[data-theme="dark"]` + `@media (prefers-color-scheme: dark) :root`), eliminating the four-block value duplication in today's `_tokens.scss`. The semantic surface expands from 7 to 13 public tokens, absorbing the success/warning/danger families that `_button.scss`, `_badge.scss`, and `_toast.scss` currently fake with hardcoded hex. Component partials are refactored to consume the semantic tokens. The committed `.zero/` copies in `showcase/` and the three examples are refreshed in lockstep so their integration tests continue to pass. Docs (AGENTS.md, BEST_PRACTICES.md, `zero-framework-spec.md` §7.1) catch up.

The approach keeps backward-compatibility for every documented public token name. New work is purely additive at the public surface; the only deletions are component-private tokens (`--button-danger-bg`, `--badge-{success,warning,danger}-*`, `--toast-{success,warning,danger}-*`) that were never documented as user API.

## Prerequisites

The spec's four Open Questions resolve as follows so execution can proceed without further consultation:

- **Palette hex values** — pick from Open Color (`https://yeun.github.io/open-color/`). It's a perceptually-uniform 11-step ramp licensed MIT, ships gray + blue + red + green + amber (called "yellow" in Open Color; we'll use Open Color's `orange` family for amber to match the visual warmth typical of "warning"). The planner records the source in a top-of-file comment in `_palette.scss`.
- **`color-scheme` property** — emit `color-scheme: light;` and `color-scheme: dark;` inside each theme's mixin. Native UI (scrollbars, default form controls) inherits the theme.
- **Dark-mode `-fg` choices** — pick foreground tokens that hit ≥ 4.5:1 contrast against the chosen palette-step background. The spec's intent table is a starting point; the planner adjusts if the chosen Open Color step doesn't meet contrast.
- **Showcase palette inspection page** — not in scope. The showcase's role is component demos.

No other issues block this work. `.zero/` infrastructure (Phase 7) and the design system (§7.1) are already in place.

## Steps

- [x] **Step 1: Land the new theme infrastructure in scaffold source + Rust manifest + inline tests**
- [x] **Step 2: Refresh committed `.zero/` copies in `showcase/`, `examples/todos/web/`, `examples/tracker/web/`**
- [x] **Step 3: Refactor `_button.scss`, `_badge.scss`, `_toast.scss` in scaffold and all committed `.zero/` copies**
- [x] **Step 4: Update integration tests (`tests/design_system.rs`, `tests/update.rs`) and add palette-presence assertions**
- [x] **Step 5: Update documentation (`zero-framework-spec.md` §7.1, `src/scaffold/AGENTS.md`, `BEST_PRACTICES.md`)**

---

## Step Details

### Step 1: Land the new theme infrastructure in scaffold source + Rust manifest + inline tests

**Goal:** Cut the framework from the single-file token model to the two-tier palette/themes model, and rename the font-size tokens to clean up the prefix collision. After this step, `zero init` and `zero update` emit the new layout; the existing 7 public color token names continue to resolve at runtime. Component partials are not yet color-refactored — they still carry their private color tokens, so visual output for an existing user is unchanged *except* that any user-side reference to `var(--font-sm|md|lg|xl)` stops working (those names are gone after this step; consumers move to `var(--font-size-{sm,md,lg,xl})`).

**Files:**

Create:
- `src/scaffold/.zero/styles/_palette.scss`
- `src/scaffold/.zero/styles/_themes.scss`
- `src/scaffold/.zero/styles/themes/_light.scss`
- `src/scaffold/.zero/styles/themes/_dark.scss`

Modify (SCSS):
- `src/scaffold/.zero/styles/_tokens.scss` — remove color section, remove `prefers-color-scheme` block, remove both `[data-theme]` blocks. Add `--font-sans` and `--font-mono` to a new "Font family" subsection above "Font sizes." Rename font-size tokens to `--font-size-{sm,md,lg,xl}`.
- `src/scaffold/.zero/styles/_base.scss` — body rule's `font-family: system-ui, sans-serif;` becomes `font-family: var(--font-sans);`. The `font-size: var(--font-md);` line becomes `font-size: var(--font-size-md);`.
- `src/scaffold/.zero/styles/zero.scss` — reorder/extend `@use` list: `palette`, `tokens`, `themes`, `base`, `layout`, `utilities`, `alignment`, `components`.
- **Every shipped component partial that reads a font-size token** — rename `var(--font-{sm,md,lg,xl})` to `var(--font-size-{sm,md,lg,xl})` everywhere. Affected files in `src/scaffold/.zero/styles/components/`: `_badge.scss`, `_button.scss`, `_card.scss`, `_checkbox.scss`, `_dialog.scss`, `_input.scss`, `_radio.scss`, `_select.scss`, `_textarea.scss`, `_toggle.scss`. Mechanical find/replace; no other content in these files changes here (the color-token refactor of `_button.scss` / `_badge.scss` / `_toast.scss` is Step 3).

Modify (user-owned example apps — committed in the repo, not regenerated by `zero update`):
- `examples/counter/web/styles/app.scss` — rename one reference (`var(--font-lg)` → `var(--font-size-lg)`).
- `examples/todos/web/styles/app.scss` — rename one reference (`var(--font-lg)` → `var(--font-size-lg)`).
- `examples/tracker/web/styles/app.scss` — rename five references (per `grep -n "\-\-font-\(sm\|md\|lg\|xl\)"`: lines 17, 30, 79, 84, 96 — `lg`, `sm`, `sm`, `sm`, `sm`).

Modify (Rust):
- `src/scaffold.rs` — four new `include_str!` constants near the existing `TPL_*_SCSS` block; four new manifest entries in `framework_manifest()`; inline tests adjusted (see below).

**Changes:**

1. `_palette.scss` content (theme-invariant, declared once on `:root`):

   ```scss
   // Color palette — framework-internal. Hex values from Open Color
   // (https://yeun.github.io/open-color/, MIT license).
   //
   // This file is framework-owned and rewritten by `zero update`. Do not edit.
   // The user-facing API is the --color-* semantic tokens defined in
   // themes/_light.scss and themes/_dark.scss; palette steps are reserved
   // for framework use and may change between minor versions.
   :root {
     // gray  (Open Color gray)
     --gray-50:  #f8f9fa;
     --gray-100: #f1f3f5;
     --gray-200: #e9ecef;
     --gray-300: #dee2e6;
     --gray-400: #ced4da;
     --gray-500: #adb5bd;
     --gray-600: #868e96;
     --gray-700: #495057;
     --gray-800: #343a40;
     --gray-900: #212529;
     --gray-950: #0e1014;

     // blue  (Open Color blue)
     --blue-50:  #e7f5ff;
     --blue-100: #d0ebff;
     --blue-200: #a5d8ff;
     --blue-300: #74c0fc;
     --blue-400: #4dabf7;
     --blue-500: #339af0;
     --blue-600: #228be6;
     --blue-700: #1c7ed6;
     --blue-800: #1971c2;
     --blue-900: #1864ab;
     --blue-950: #0d2a4d;

     // red  (Open Color red)
     --red-50:  #fff5f5;
     --red-100: #ffe3e3;
     --red-200: #ffc9c9;
     --red-300: #ffa8a8;
     --red-400: #ff8787;
     --red-500: #ff6b6b;
     --red-600: #fa5252;
     --red-700: #f03e3e;
     --red-800: #e03131;
     --red-900: #c92a2a;
     --red-950: #5c0f0f;

     // green (Open Color green)
     --green-50:  #ebfbee;
     --green-100: #d3f9d8;
     --green-200: #b2f2bb;
     --green-300: #8ce99a;
     --green-400: #69db7c;
     --green-500: #51cf66;
     --green-600: #40c057;
     --green-700: #37b24d;
     --green-800: #2f9e44;
     --green-900: #2b8a3e;
     --green-950: #103b1a;

     // amber (Open Color orange — chosen over Open Color yellow for the
     // visual warmth typical of "warning" semantics)
     --amber-50:  #fff4e6;
     --amber-100: #ffe8cc;
     --amber-200: #ffd8a8;
     --amber-300: #ffc078;
     --amber-400: #ffa94d;
     --amber-500: #ff922b;
     --amber-600: #fd7e14;
     --amber-700: #f76707;
     --amber-800: #e8590c;
     --amber-900: #d9480f;
     --amber-950: #5c1e02;
   }
   ```

2. `themes/_light.scss`:

   ```scss
   // Light theme — maps the public --color-* semantic tokens to palette steps.
   //
   // This file is framework-owned and rewritten by `zero update`. To override
   // any of these tokens for your project, re-declare them in `styles/app.scss`
   // after the `@use '../.zero/styles/zero';` line, or under a custom
   // `[data-theme="brand"]` selector.
   @mixin tokens {
     color-scheme: light;

     --color-bg:          var(--gray-50);
     --color-surface:     var(--gray-100);
     --color-text:        var(--gray-900);
     --color-text-muted:  var(--gray-600);
     --color-border:      var(--gray-200);

     --color-primary:     var(--blue-600);
     --color-primary-fg:  var(--gray-50);

     --color-success:     var(--green-700);
     --color-success-fg:  var(--gray-50);

     --color-warning:     var(--amber-500);
     --color-warning-fg:  var(--gray-900);

     --color-danger:      var(--red-700);
     --color-danger-fg:   var(--gray-50);
   }
   ```

3. `themes/_dark.scss`:

   ```scss
   // Dark theme — maps the public --color-* semantic tokens to palette steps.
   //
   // This file is framework-owned and rewritten by `zero update`. To override
   // any of these tokens for your project, re-declare them in `styles/app.scss`
   // after the `@use '../.zero/styles/zero';` line, or under a custom
   // `[data-theme="brand"]` selector.
   @mixin tokens {
     color-scheme: dark;

     --color-bg:          var(--gray-950);
     --color-surface:     var(--gray-900);
     --color-text:        var(--gray-50);
     --color-text-muted:  var(--gray-400);
     --color-border:      var(--gray-800);

     --color-primary:     var(--blue-400);
     --color-primary-fg:  var(--gray-950);

     --color-success:     var(--green-500);
     --color-success-fg:  var(--gray-950);

     --color-warning:     var(--amber-400);
     --color-warning-fg:  var(--gray-950);

     --color-danger:      var(--red-400);
     --color-danger-fg:   var(--gray-950);
   }
   ```

4. `_themes.scss`:

   ```scss
   // Theme aggregator. Each theme partial defines a single `@mixin tokens`
   // containing its --color-* assignments; this file owns the selector
   // strategy that decides which theme applies under which condition.
   //
   // Cascade rules (verified):
   //   - No data-theme + light system pref → light values via :root
   //   - No data-theme + dark system pref  → dark values win on source order
   //     (both rules have specificity 1; media-query block is later)
   //   - data-theme="light" anywhere       → light values win on specificity (10 > 1)
   //   - data-theme="dark"  anywhere       → dark values win on specificity (10 > 1)
   @use 'themes/light';
   @use 'themes/dark';

   :root,
   [data-theme="light"] {
     @include light.tokens;
   }

   [data-theme="dark"] {
     @include dark.tokens;
   }

   @media (prefers-color-scheme: dark) {
     :root {
       @include dark.tokens;
     }
   }
   ```

5. Narrowed `_tokens.scss`:

   ```scss
   // Non-color design tokens. All values are theme-invariant. Color tokens
   // live in _palette.scss (framework-internal) and themes/*.scss (public
   // --color-* semantic surface).
   //
   // This file is framework-owned and rewritten by `zero update`. To override
   // a token, re-declare the custom property in your `styles/app.scss` after
   // the `@use` line.
   :root {
     // Spacing
     --space-xs: 0.25rem;
     --space-sm: 0.5rem;
     --space-md: 1rem;
     --space-lg: 1.5rem;
     --space-xl: 3rem;

     // Radius
     --radius-sm: 2px;
     --radius-md: 4px;
     --radius-lg: 8px;

     // Font family
     --font-sans: system-ui, sans-serif;
     --font-mono: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;

     // Font sizes
     --font-size-sm: 0.875rem;
     --font-size-md: 1rem;
     --font-size-lg: 1.25rem;
     --font-size-xl: 1.5rem;

     // Font weights
     --weight-normal: 400;
     --weight-bold:   700;

     // Line heights
     --leading-tight:  1.2;
     --leading-normal: 1.5;

     // Shadows
     --shadow-sm: 0 1px 2px rgba(0, 0, 0, 0.05);
     --shadow-md: 0 4px 6px rgba(0, 0, 0, 0.1);
     --shadow-lg: 0 10px 15px rgba(0, 0, 0, 0.1);

     // Border widths
     --border-thin:  1px;
     --border-md:    2px;
     --border-thick: 4px;
   }
   ```

6. `_base.scss`:

   ```scss
   *, *::before, *::after { box-sizing: border-box; }

   body {
     margin: 0;
     color: var(--color-text);
     background: var(--color-bg);
     font-family: var(--font-sans);
     font-size: var(--font-size-md);
     line-height: var(--leading-normal);
   }
   ```

7. `zero.scss`:

   ```scss
   @use 'palette';
   @use 'tokens';
   @use 'themes';
   @use 'base';
   @use 'layout';
   @use 'utilities';
   @use 'alignment';
   @use 'components';
   ```

8. `src/scaffold.rs` (around `src/scaffold.rs:19`):

   Add four new constants in alphabetical/grouped order with the other `TPL_*_SCSS` constants:

   ```rust
   const TPL_PALETTE_SCSS: &str = include_str!("scaffold/.zero/styles/_palette.scss");
   const TPL_THEMES_SCSS: &str = include_str!("scaffold/.zero/styles/_themes.scss");
   const TPL_THEME_LIGHT_SCSS: &str = include_str!("scaffold/.zero/styles/themes/_light.scss");
   const TPL_THEME_DARK_SCSS: &str = include_str!("scaffold/.zero/styles/themes/_dark.scss");
   ```

   In `framework_manifest()` (around `src/scaffold.rs:104`), add four entries adjacent to the existing styles entries. Place them so the manifest reads in roughly file-system order:

   ```rust
   (".zero/styles/_palette.scss", TPL_PALETTE_SCSS),
   (".zero/styles/_tokens.scss", TPL_TOKENS_SCSS),
   (".zero/styles/_themes.scss", TPL_THEMES_SCSS),
   (".zero/styles/themes/_light.scss", TPL_THEME_LIGHT_SCSS),
   (".zero/styles/themes/_dark.scss", TPL_THEME_DARK_SCSS),
   (".zero/styles/_base.scss", TPL_BASE_SCSS),
   // ... existing entries follow ...
   ```

9. Inline tests in `src/scaffold.rs`:

   - `tokens_scss_declares_tokens_directly` (`src/scaffold.rs:531`) — rewrite. It must now assert against the right files: `_tokens.scss` contains `--space-md:`, `--font-sans:`, and `--font-size-md:`, and does *not* contain `--color-primary:`, `@media`, or `--font-md:` (the old size-token name); `themes/_light.scss` contains `--color-primary:`; `themes/_dark.scss` contains `--color-primary:`; `_themes.scss` contains `@media (prefers-color-scheme: dark)`, `[data-theme="dark"]`, and `[data-theme="light"]`. Rename the test to `tokens_and_themes_split_correctly` to reflect its new scope.
   - `write_initial_project_emits_framework_files` (`src/scaffold.rs:420`, line 432–433) — change the `_tokens.scss` assertion target from `--color-primary:` to `--space-md:` (a token that now actually lives in `_tokens.scss`). Add a parallel block that reads `themes/_light.scss` and asserts `--color-primary:` is present there.
   - `zero_scss_contains_aggregate_uses` (`src/scaffold.rs:568`) — add `@use 'palette'` and `@use 'themes'` to the list of expected needles.
   - `framework_manifest_matches_expected_path_set` (`src/scaffold.rs:678`) — add the four new paths to the `expected` BTreeSet.

   No new helper functions needed — the existing `fresh_scaffold()` fixture suffices.

**Tests:**

After this step, run `cargo test` from the workspace root. All existing tests should pass with the adjusted assertions. Key tests touched:

- `scaffold.rs` inline tests (above) verify file split and aggregator wiring.
- `tests/design_system.rs::build_emits_design_system_css` continues to pass — it checks the compiled CSS for `--color-primary`, `prefers-color-scheme: dark`, `[data-theme="dark"]`, `[data-theme="light"]` — all still present in the compiled output (from different source files).
- `tests/design_system.rs::build_design_system_passes_contrast_smoke` continues to pass.
- `tests/scss_build.rs` and `tests/scss_dev.rs` compile the project's SCSS; the new `@use` graph must compile cleanly.
- `tests/update.rs::update_restores_modified_recreates_deleted_removes_stray` — the assertion at `tests/update.rs:50–53` still references `--color-primary:` in `_tokens.scss`. **Defer this test fix to Step 4** (it's an integration-test concern, not a scaffold concern). For Step 1 to pass `cargo test`, update this assertion alongside the scaffold changes; the rationale lives in Step 4.

Bundling the `tests/update.rs` fix here is unavoidable because Step 1 breaks it. Adjust at `tests/update.rs:50–53` to assert `--space-md:` (a token that now actually lives in `_tokens.scss` post-restore).

### Step 2: Refresh committed `.zero/` copies in `showcase/`, `examples/todos/web/`, `examples/tracker/web/`

**Goal:** The three subprojects with committed `.zero/` directories (showcase, todos, tracker) need their copies brought into sync with the new scaffold sources so their integration tests run against the new theme system. Without this, `tests/showcase_build.rs`, `tests/showcase_dev.rs`, `tests/examples_build.rs`, and `tests/examples_tests.rs` would fail — Step 1 renamed `--font-{sm,md,lg,xl}` to `--font-size-*`, but the subproject component partials still reference the old names, so SCSS would compile with empty `font-size: ;` rules. Lockstep refresh is required, not optional.

**Files:**

For each subproject root in `{showcase, examples/todos/web, examples/tracker/web}`:

Create:
- `<root>/.zero/styles/_palette.scss` — copy from `src/scaffold/.zero/styles/_palette.scss`
- `<root>/.zero/styles/_themes.scss` — copy
- `<root>/.zero/styles/themes/_light.scss` — copy
- `<root>/.zero/styles/themes/_dark.scss` — copy

Modify:
- `<root>/.zero/styles/_tokens.scss` — overwrite with the narrowed content from Step 1
- `<root>/.zero/styles/_base.scss` — overwrite (font-family swap + size token rename)
- `<root>/.zero/styles/zero.scss` — overwrite (new `@use` order)
- `<root>/.zero/styles/components/{_badge,_button,_card,_checkbox,_dialog,_input,_radio,_select,_textarea,_toggle}.scss` — overwrite. These carry the font-size-token rename from Step 1. (`_toast.scss` doesn't read font-size tokens — no rename, but it gets refreshed in Step 3 for the color refactor.)

**Changes:**

The content of each file matches Step 1 byte-for-byte. Use `cp` from the scaffold source rather than re-authoring, to guarantee parity. Equivalent operation: from the workspace root, for each project root above, run:

```sh
mkdir -p <root>/.zero/styles/themes
cp src/scaffold/.zero/styles/_palette.scss   <root>/.zero/styles/_palette.scss
cp src/scaffold/.zero/styles/_themes.scss    <root>/.zero/styles/_themes.scss
cp src/scaffold/.zero/styles/themes/_light.scss <root>/.zero/styles/themes/_light.scss
cp src/scaffold/.zero/styles/themes/_dark.scss  <root>/.zero/styles/themes/_dark.scss
cp src/scaffold/.zero/styles/_tokens.scss    <root>/.zero/styles/_tokens.scss
cp src/scaffold/.zero/styles/_base.scss      <root>/.zero/styles/_base.scss
cp src/scaffold/.zero/styles/zero.scss       <root>/.zero/styles/zero.scss
for c in badge button card checkbox dialog input radio select textarea toggle; do
  cp "src/scaffold/.zero/styles/components/_${c}.scss" "<root>/.zero/styles/components/_${c}.scss"
done
```

**Tests:**

- `tests/showcase_build.rs` — compiles the showcase end-to-end. Asserts current-shape build output; should still pass because the public API (`--color-*` token names, `data-theme`, `prefers-color-scheme`) is preserved.
- `tests/showcase_dev.rs` — dev-server smoke; same.
- `tests/examples_build.rs` and `tests/examples_tests.rs` — build/test each example; same.
- Manual verification (no automated test): `node --test` against the showcase / examples still passes — their component tests don't touch styles.

### Step 3: Refactor `_button.scss`, `_badge.scss`, `_toast.scss` in scaffold and all committed `.zero/` copies

**Goal:** Remove the component-private color tokens (`--button-danger-bg/fg`, `--badge-{success,warning,danger}-{bg,fg}`, `--toast-{success,warning,danger}-{bg,fg}`) and switch the consuming rules to the new public `--color-{success,warning,danger}` semantic tokens. After this step, the three components are fully theme-aware — `danger` looks correct in dark mode, etc.

**Files:**

For each location in `{src/scaffold, showcase, examples/todos/web, examples/tracker/web}/.zero/styles/components/`:

Modify:
- `_button.scss`
- `_badge.scss`
- `_toast.scss`

**Changes:**

New `_button.scss`:

```scss
@layer components {
  .button {
    display: inline-flex;
    align-items: center;
    gap: var(--space-xs);
    padding-inline: var(--space-md);
    padding-block: var(--space-sm);
    border: var(--border-thin) solid transparent;
    border-radius: var(--radius-md);
    font: inherit;
    font-weight: var(--weight-bold);
    line-height: var(--leading-tight);
    cursor: pointer;
  }

  .button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .button-sm {
    padding-inline: var(--space-sm);
    padding-block: var(--space-xs);
    font-size: var(--font-size-sm);
  }

  .button-md {
    padding-inline: var(--space-md);
    padding-block: var(--space-sm);
    font-size: var(--font-size-md);
  }

  .button-lg {
    padding-inline: var(--space-lg);
    padding-block: var(--space-md);
    font-size: var(--font-size-lg);
  }

  .button-primary {
    background: var(--color-primary);
    color: var(--color-primary-fg);
  }

  .button-secondary {
    background: var(--color-surface);
    color: var(--color-text);
    border-color: var(--color-border);
  }

  .button-ghost {
    background: transparent;
    color: var(--color-text);
  }

  .button-danger {
    background: var(--color-danger);
    color: var(--color-danger-fg);
  }
}
```

(The `:root { --button-danger-bg: ...; --button-danger-fg: ...; }` block is removed entirely.)

New `_badge.scss`:

```scss
@layer components {
  .badge {
    display: inline-flex;
    align-items: center;
    padding-inline: var(--space-sm);
    padding-block: var(--space-xs);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-sm);
    font-weight: var(--weight-bold);
    line-height: var(--leading-tight);
  }

  .badge-sm {
    font-size: var(--font-size-sm);
    padding-inline: var(--space-xs);
  }

  .badge-md {
    font-size: var(--font-size-md);
  }

  .badge-default {
    background: var(--color-surface);
    color: var(--color-text);
  }

  .badge-primary {
    background: var(--color-primary);
    color: var(--color-primary-fg);
  }

  .badge-success {
    background: var(--color-success);
    color: var(--color-success-fg);
  }

  .badge-warning {
    background: var(--color-warning);
    color: var(--color-warning-fg);
  }

  .badge-danger {
    background: var(--color-danger);
    color: var(--color-danger-fg);
  }
}
```

New `_toast.scss`:

```scss
@layer components {
  .toast {
    position: fixed;
    inset-block-end: var(--space-lg);
    inset-inline-end: var(--space-lg);
    padding: var(--space-sm) var(--space-md);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-md);
    z-index: 900;
  }

  .toast-info {
    background: var(--color-surface);
    color: var(--color-text);
  }

  .toast-success {
    background: var(--color-success);
    color: var(--color-success-fg);
  }

  .toast-warning {
    background: var(--color-warning);
    color: var(--color-warning-fg);
  }

  .toast-danger {
    background: var(--color-danger);
    color: var(--color-danger-fg);
  }
}
```

After authoring in `src/scaffold/.zero/styles/components/`, copy each updated file to the three subproject copies:

```sh
for root in showcase examples/todos/web examples/tracker/web; do
  cp src/scaffold/.zero/styles/components/_button.scss "$root/.zero/styles/components/_button.scss"
  cp src/scaffold/.zero/styles/components/_badge.scss  "$root/.zero/styles/components/_badge.scss"
  cp src/scaffold/.zero/styles/components/_toast.scss  "$root/.zero/styles/components/_toast.scss"
done
```

**Tests:**

- `tests/component_library.rs` — verifies the components compile, render, and produce expected DOM. No CSS content assertions here; should still pass.
- Per-component unit tests under `.zero/components/<Name>.test.ts` (Button, Badge, Toast) — assert on HTML structure / class names, not computed styles. Unchanged.
- `tests/showcase_build.rs` / `tests/showcase_dev.rs` — full build and dev smoke. Should still pass; the showcase exercises all variants.
- `tests/examples_build.rs` — should pass.

### Step 4: Update integration tests and add palette-presence assertions

**Goal:** Lock in the new behavior with assertions that would catch regression. The `tests/design_system.rs` file gains palette and semantic-token checks. The component-private tokens that were removed in Step 3 get a negative assertion so they don't quietly creep back. `tests/update.rs` gets adjusted to assert against a token that actually lives in `_tokens.scss` post-refactor.

**Files:**

Modify:
- `tests/design_system.rs`
- `tests/update.rs` (already adjusted in Step 1 to keep that step green; this step adds any further refinements documented below)

Optionally create:
- `tests/themes.rs` — only if `tests/design_system.rs` grows past ~250 lines; otherwise add the new assertions inline to `tests/design_system.rs`. Default: inline.

**Changes:**

In `tests/design_system.rs::build_emits_design_system_css` (after the existing assertions), add:

1. Palette presence — the compiled CSS contains the palette tokens:

   ```rust
   for needle in [
       "--gray-50:",
       "--gray-950:",
       "--blue-600:",
       "--red-700:",
       "--green-700:",
       "--amber-500:",
   ] {
       assert!(
           css.contains(needle),
           "compiled CSS missing palette token {needle}: {css}"
       );
   }
   ```

2. New semantic tokens present:

   ```rust
   for needle in [
       "--color-success:",
       "--color-success-fg:",
       "--color-warning:",
       "--color-warning-fg:",
       "--color-danger:",
       "--color-danger-fg:",
       "--font-sans:",
       "--font-mono:",
       "--font-size-md:",
   ] {
       assert!(
           css.contains(needle),
           "compiled CSS missing semantic token {needle}: {css}"
       );
   }
   ```

   And the old size-token names are gone:

   ```rust
   for needle in ["--font-sm:", "--font-md:", "--font-lg:", "--font-xl:"] {
       assert!(
           !css.contains(needle),
           "compiled CSS still declares old size-token {needle}: {css}"
       );
   }
   ```

3. Component-private tokens no longer leak:

   ```rust
   for needle in [
       "--button-danger-bg",
       "--button-danger-fg",
       "--badge-success-bg",
       "--badge-warning-bg",
       "--badge-danger-bg",
       "--toast-success-bg",
       "--toast-warning-bg",
       "--toast-danger-bg",
   ] {
       assert!(
           !css.contains(needle),
           "compiled CSS contains removed component-private token {needle}: {css}"
       );
   }
   ```

4. `color-scheme` declarations make it through:

   ```rust
   assert!(
       css.contains("color-scheme: light") || css.contains("color-scheme:light"),
       "compiled CSS missing color-scheme: light declaration"
   );
   assert!(
       css.contains("color-scheme: dark") || css.contains("color-scheme:dark"),
       "compiled CSS missing color-scheme: dark declaration"
   );
   ```

5. New test `build_emits_font_family_token_consumption` — verifies the body rule consumes `var(--font-sans)`:

   ```rust
   #[test]
   fn build_emits_font_family_token() {
       let tmp = tempfile::tempdir().unwrap();
       write_scss_project(tmp.path());
       Command::cargo_bin("zero").unwrap().arg("build").current_dir(tmp.path()).assert().success();
       let assets_dir = tmp.path().join("dist/assets");
       let css_file = find_asset(&assets_dir, "app.", ".css").expect("no hashed CSS found");
       let css = std::fs::read_to_string(assets_dir.join(&css_file)).unwrap();
       assert!(
           css.contains("font-family: var(--font-sans)") || css.contains("font-family:var(--font-sans)"),
           "body font-family does not consume --font-sans: {css}"
       );
   }
   ```

In `tests/update.rs::update_restores_modified_recreates_deleted_removes_stray` (around line 50): change the post-restore assertion to a token that actually lives in the narrowed `_tokens.scss`:

```rust
assert!(
    tokens_str.contains("--space-md:"),
    "tokens not restored: {tokens_str}"
);
```

(Already applied in Step 1 to keep `cargo test` green. This step is the documentation of the rationale.)

**Tests:**

- `cargo test` from the workspace root — all assertions added above must pass against the changes from Steps 1–3.
- Run `node --test runtime/*.test.js` — JS runtime tests are unaffected by SCSS changes but worth running to confirm nothing in `scaffold/` accidentally broke a JS-side fixture.

### Step 5: Update documentation

**Goal:** Bring user-facing docs in line with the new theme system. Specifically: the spec's §7.1, the scaffold's `AGENTS.md`, and `BEST_PRACTICES.md`. Without this step, users would read outdated descriptions of "the seven `--color-*` tokens" and "`_tokens.scss` holds light and dark variants."

**Files:**

Modify:
- `zero-framework-spec.md` (§7.1, lines ~874–900)
- `src/scaffold/AGENTS.md` (sections "Styles", "Design system", "Theme switching", and the file-overview table at line 706)
- `BEST_PRACTICES.md` — add a "Theming" section. Locate by reading the file and inserting after the existing CSS-related section (likely near the line-362 area where it discusses wrapping shipped primitives).

**Changes:**

1. `zero-framework-spec.md` §7.1:
   - Update the "Token categories" table. Replace the single "Colors" row (7 tokens) with two rows:
     - "Color palette (framework-internal)" — five families × 11 steps, list one example per family, note `--gray-{50…950}`, `--blue-{50…950}`, etc.
     - "Semantic colors (public)" — the 13 `--color-*` tokens.
   - Add a "Font family" row with `--font-sans`, `--font-mono`.
   - Update the "Theme switching" paragraph to describe the new layout: themes are partials under `.zero/styles/themes/`, each defines a `@mixin tokens`, `_themes.scss` owns the selector strategy. Note that the user override path is unchanged (re-declare semantic tokens in `styles/app.scss`).
   - Update the "Distribution model" paragraph: enumerate the new files (`_palette.scss`, `_themes.scss`, `themes/_light.scss`, `themes/_dark.scss`).

2. `src/scaffold/AGENTS.md`:
   - "Styles" section (around line 514): change "Design tokens are CSS custom properties declared in `styles/_tokens.scss`" to describe the new split (palette + non-color tokens in `_tokens.scss` + themes).
   - "Design system" partial table (line 521 onward): add four rows for `_palette.scss`, `_themes.scss`, `themes/_light.scss`, `themes/_dark.scss`. Update the `_tokens.scss` row description to "Non-color design tokens (spacing, radius, font family/size/weight, line height, shadow, border)."
   - "Theme switching" subsection (line 575 onward): rewrite to reflect the new layout. The user-facing API for `data-theme="light|dark"` and `prefers-color-scheme` is unchanged; the description of where the values live changes.
   - The line stating "The dark-mode override applies only to the seven `--color-*` tokens" (line 589): update to "the thirteen `--color-*` semantic tokens" and list them.
   - File-overview table at line 706: add the four new paths with one-line descriptions.

3. `BEST_PRACTICES.md`:
   - New "Theming" section. Outline:
     - "zero ships two themes, light and dark, plus a 13-token public `--color-*` semantic surface."
     - "To override an individual token: re-declare it in `styles/app.scss` after the `@use` line."
     - "To author a brand theme: declare all 13 `--color-*` tokens under a `[data-theme=\"brand\"]` selector in your own SCSS, then `@use` it from `styles/app.scss`. Apply via `<html data-theme=\"brand\">`."
     - "The framework's internal color palette (`--gray-*`, `--blue-*`, etc.) is reachable as CSS custom properties but is not part of the public API — its values and step set may change between minor versions. Stick to `--color-*` semantics in your own styles."
     - One short worked example: a brand theme that sets `--color-primary` to a green and re-uses the framework palette for everything else.

**Tests:**

Docs-only step; no automated test coverage. Verify by reading the diff, running `cargo test` (no regressions from docs edits — these files aren't `include_str!`'d into tests), and confirming the spec's intent is faithfully reflected.

---

## Risks and Assumptions

- **Assumption — Open Color values pass WCAG AA contrast in both themes.** The chosen palette steps in `themes/_light.scss` (e.g., `--color-primary: var(--blue-600)` on `--color-bg: var(--gray-50)`) and the dark equivalents should hit 4.5:1 for normal text. If a specific pair fails contrast, the planner adjusts the palette step *before* shipping — the file layout doesn't change. Mitigation: run a manual contrast check (e.g., `oklch` distance) on each pair during Step 1 author, before committing.
- **Risk — Subproject `.zero/` copies drift.** Step 2 copies files manually. If any byte mismatches `src/scaffold/.zero/`, `zero update --yes` against the binary would produce a non-empty plan, suggesting an unintended diff. Mitigation: `cp` rather than re-author; before declaring Step 2 done, run `diff -r src/scaffold/.zero/styles showcase/.zero/styles` (and the two examples) — output must be empty.
- **Note — A user's project overrode a removed component-private token.** A user who in their `styles/app.scss` declared `--badge-success-bg: #....` will see their override silently stop applying after upgrade. Backward compatibility is not a constraint on this change (framework is pre-1.0); this is accepted as part of the cleanup. Mention in the changelog / PR description.
- **Risk — Sass mixin pattern introduces a subtle scoping issue.** `@use 'themes/light';` then `@include light.tokens;` is the documented Sass pattern, but each `@include` site re-emits all 13 declarations. The compiled CSS will contain the dark mixin body twice — once under `[data-theme="dark"]` and once under the media query's `:root`. That's intentional and required (specificity needs both selectors), but it does double-count those bytes in the compiled output. Mitigation: acceptable; the gzip-deduped size cost is negligible.
- **Assumption — The `:root, [data-theme="light"]` combined-selector specificity behavior matches the spec's cascade analysis.** Verified by hand-walk of the four cases in the spec, but the planner should also test by spot-checking the showcase with each combination (`data-theme="light"` on a dark-system browser; no attribute on a dark-system browser; etc.) before declaring Step 1 done.
- **Assumption — `tests/design_system.rs` currently runs `zero init --yes` then `zero build` and reads the compiled CSS.** Verified by reading the test. The added assertions in Step 4 inherit this flow, so they cover the full chain from scaffold templates → compiled output.
- **Risk — Stale `var(--font-md)` references after the rename.** Step 1 renames every `var(--font-{sm,md,lg,xl})` call site in the scaffold sources and the user-owned example app stylesheets. If a call site is missed, SCSS compiles silently with `font-size: ;` and the affected element falls back to inherited size. Mitigation: after Step 1, grep `grep -rn "\-\-font-\(sm\|md\|lg\|xl\)[^a-z]" src/ examples/` from the workspace root — output must be empty. After Step 2, run the same grep extended to `showcase/` — also empty.
