# Plan: Built-in CSS design system

## Summary

Replace the scaffold's two-file SCSS skeleton (`_vars.scss` + `app.scss`) with a five-file design-system layer: `_tokens.scss` (CSS custom properties for spacing, colors, radius, type, line height, shadows, border widths, with light/dark theme variants), `_base.scss` (box-sizing reset + token-bound `body`), `_layout.scss` (six layout primitives: `cluster`, `stack`, `frame`, `split`, `flank`, `grid`), `_utilities.scss` (15 utility classes: `gap-*`, `pad-*`, `border` + 4 directional variants), and a four-line `app.scss` entry. The scaffold's demo files (`index.html` and `home.ts`) are updated so a fresh `zero init` project shows the system in use: a `<meta name="color-scheme">` declaration completes the dark-mode story for browser UI, and the home route uses `stack`, `cluster`, and utility classes so the user sees the primitives the moment they run `zero dev`. The Rust scaffold (`src/scaffold.rs`) is updated to embed and emit the new partials, the existing `_vars.scss` is dropped, and integration coverage (new file `tests/design_system.rs`) verifies the end-to-end `init` → `build`/`dev` path. Documentation (`AGENTS.md` and `zero-framework-spec.md`) is extended to describe the system.

The approach treats the SCSS files as the spec's source of truth: the four partials are authored once, embedded via `include_str!`, and considered user-owned after `zero init`. The framework provides no upgrade path. The future component library will assume these classes/tokens exist.

## Prerequisites

The spec leaves a set of Open Questions that this plan resolves up front rather than deferring. The resolved values are baked into the partials authored in Step 1:

- **Spacing scale** (doubling-ish): `--space-xs: 0.25rem`, `--space-sm: 0.5rem`, `--space-md: 1rem`, `--space-lg: 1.5rem`, `--space-xl: 3rem`.
- **Light palette**: `--color-bg: #ffffff`, `--color-surface: #f5f5f5`, `--color-text: #1a1a1a`, `--color-text-muted: #5c5c5c`, `--color-primary: #2563eb`, `--color-primary-fg: #ffffff`, `--color-border: #e5e5e5`. (Primary darkened from `#3b82f6` so white-on-primary clears WCAG AA 4.5:1 — `#2563eb` is ~4.83:1 against white.)
- **Dark palette**: `--color-bg: #0f1115`, `--color-surface: #1a1d23`, `--color-text: #f5f5f5`, `--color-text-muted: #a3a3a3`, `--color-primary: #60a5fa`, `--color-primary-fg: #0f1115`, `--color-border: #2a2e35`. (Primary lifted to `#60a5fa` so dark-bg-on-primary clears AA; primary-fg uses the dark `--color-bg` value, not pure black, so the contrast budget on the chip surface matches the dark theme.)
- **Radius**: `--radius-sm: 2px`, `--radius-md: 4px`, `--radius-lg: 8px`.
- **Font sizes** (familiar t-shirt scale; not a strict ratio): `--font-sm: 0.875rem`, `--font-md: 1rem`, `--font-lg: 1.25rem`, `--font-xl: 1.5rem`.
- **Font weight**: `--weight-normal: 400`, `--weight-bold: 700`.
- **Line height**: `--leading-tight: 1.2`, `--leading-normal: 1.5`.
- **Shadows** (no token color dependency — neutral rgba): `--shadow-sm: 0 1px 2px rgba(0, 0, 0, 0.05)`, `--shadow-md: 0 4px 6px rgba(0, 0, 0, 0.1)`, `--shadow-lg: 0 10px 15px rgba(0, 0, 0, 0.1)`.
- **Border widths**: `--border-thin: 1px`, `--border-md: 2px`, `--border-thick: 4px`.
- **Frame default ratio**: `16 / 9` (spec proposal accepted).
- **Grid min column**: `16rem` (spec proposal accepted — typical card width).
- **Body font stack**: `system-ui, sans-serif` (spec proposal accepted — broad coverage, no obscure fallbacks).
- **`data-theme` location**: documented as `<html>` (only the recommendation surface — selector works on any ancestor).
- **File organization**: four partials (`_tokens`, `_base`, `_layout`, `_utilities`) — splitting is worth the file-count cost because `_utilities.scss` is the most likely deletion candidate.
- **Default primitive gap**: `--space-md` for all five non-`frame` primitives (uniform default, override via `gap-*` utility).
- **`flank` wrap behavior**: relies on natural flex wrapping (no `--flank-threshold`). The fill child's `flex: 1 1 0` combined with `flex-wrap: wrap` on the container produces "wraps when the fill child can't honour its own min content" — this is the spec's "wraps when narrow" intent without introducing a tunable knob.
- **Naming clash policy**: unprefixed identifiers — the component library will assume them.
- **Integration test location**: new file `tests/design_system.rs`.

None of these depend on other issues, and none require coordination outside this issue. Execution may proceed directly.

## Steps

- [x] **Step 1: Author the new SCSS partials, rewrite `app.scss`, rewire `src/scaffold.rs`, and update the scaffold's Rust tests**
- [x] **Step 2: Wire the design system into the scaffold demo (`index.html` + `home.ts`) so a fresh `zero init` project shows the system in use**
- [x] **Step 3: Extend `src/scaffold/AGENTS.md`'s Styles section to document the design system**
- [x] **Step 4: Update `zero-framework-spec.md` §7 and §13 to describe the design system and the direct-CSS-custom-property pattern**
- [x] **Step 5: Add `tests/design_system.rs` integration coverage**

---

## Step Details

### Step 1: Author the new SCSS partials, rewrite `app.scss`, rewire `src/scaffold.rs`, and update the scaffold's Rust tests

**Goal:** Land the design system in the scaffold in a single atomic step. The SCSS files, the `include_str!` constants, the `write_to` writes, and the `mod tests` assertions are interdependent — deleting `_vars.scss` without simultaneously rewriting `app.scss` and the `vars_scss_bridges_tokens_to_root` test would leave the repo in a non-compiling state. After this step, `cargo test` passes and `zero init` writes the new layout.

**Files:**

Created:
- `src/scaffold/styles/_tokens.scss`
- `src/scaffold/styles/_base.scss`
- `src/scaffold/styles/_layout.scss`
- `src/scaffold/styles/_utilities.scss`

Modified:
- `src/scaffold/styles/app.scss` (rewritten to the four-line entry)
- `src/scaffold.rs` (constants, `write_to`, tests)

Deleted:
- `src/scaffold/styles/_vars.scss`

**Changes:**

1. **`src/scaffold/styles/_tokens.scss`** — single SCSS partial containing:
   - One `:root { ... }` block declaring every CSS custom property listed in Prerequisites. Light-mode color values go on `:root`. Spacing, radius, font sizes, font weights, line heights, shadows, and border widths are theme-independent and live only on `:root`.
   - One `@media (prefers-color-scheme: dark) { :root { --color-bg: ...; --color-surface: ...; --color-text: ...; --color-text-muted: ...; --color-primary: ...; --color-primary-fg: ...; --color-border: ...; } }` block — only the seven color tokens, using the dark values from Prerequisites.
   - One `[data-theme="dark"] { ... }` block with the same seven dark color values (forces dark even when system says light).
   - One `[data-theme="light"] { ... }` block with the seven light color values (forces light even when system says dark).
   - **No** `$var:` declarations. No `:#{$var}` bridge. Direct authoring only.
   - A leading comment block explaining: tokens are CSS custom properties; dark mode is honored automatically via `prefers-color-scheme`; users can override per-subtree by setting `data-theme="light"` or `data-theme="dark"` on an ancestor (canonically `<html>`); the file is user-owned and may be edited or deleted (with documented downstream consequence for the future component library).

2. **`src/scaffold/styles/_base.scss`** — minimal reset:
   ```scss
   *, *::before, *::after { box-sizing: border-box; }

   body {
     margin: 0;
     color: var(--color-text);
     background: var(--color-bg);
     font-family: system-ui, sans-serif;
     font-size: var(--font-md);
     line-height: var(--leading-normal);
   }
   ```
   No other rules. A leading comment notes that the base layer is intentionally minimal — typography defaults belong to the future component library.

3. **`src/scaffold/styles/_layout.scss`** — six layout classes, each preceded by a one-line comment naming its origin (or "no origin" for `split`/`flank` since those are the codebase's own choices):
   - `.cluster { display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-md); }`
   - `.stack { display: flex; flex-direction: column; gap: var(--space-md); }`
   - `.frame { aspect-ratio: var(--frame-ratio, 16 / 9); overflow: hidden; display: grid; place-items: center; }` — note that `--frame-ratio` is a per-instance override knob; the fallback in `var()` lets users skip declaring `--frame-ratio` for the 16/9 default.
   - `.split { display: grid; grid-template-columns: 1fr 1fr; gap: var(--space-md); }`
   - `.flank { display: flex; flex-wrap: wrap; gap: var(--space-md); }` plus `.flank > :first-child { flex: 0 0 auto; }` and `.flank > :nth-child(2) { flex: 1 1 0; min-width: 0; }`. The `min-width: 0` is required so the fill child can shrink below its content's intrinsic min-width before flex-wrap kicks in.
   - `.grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, var(--grid-min, 16rem)), 1fr)); gap: var(--space-md); }` — `--grid-min` is a per-instance override knob; fallback in `var()` gives `16rem`. The `min(100%, ...)` clamps the column min to the container width so single very-narrow containers don't overflow.

   No margin properties anywhere in this file. A leading comment restates the no-margin constraint.

4. **`src/scaffold/styles/_utilities.scss`** — fifteen one-line classes:
   - Five `.gap-{xs,sm,md,lg,xl} { gap: var(--space-{step}); }`
   - Five `.pad-{xs,sm,md,lg,xl} { padding: var(--space-{step}); }`
   - `.border { border: var(--border-thin) solid var(--color-border); }`
   - `.border-t { border-top: var(--border-thin) solid var(--color-border); }`
   - `.border-r { border-right: var(--border-thin) solid var(--color-border); }`
   - `.border-b { border-bottom: var(--border-thin) solid var(--color-border); }`
   - `.border-l { border-left: var(--border-thin) solid var(--color-border); }`

   No `!important`. No axis variants. A leading comment notes that override is by source order — `class="cluster gap-lg"` works because `gap-lg` follows `.cluster` in the compiled CSS.

5. **`src/scaffold/styles/app.scss`** — rewritten to exactly:
   ```scss
   @use 'tokens';
   @use 'base';
   @use 'layout';
   @use 'utilities';
   ```
   No `body { ... }` rule, no `h1` rule, no other content. Users add application styles below the `@use` block.

   Sass's `@use` will emit the CSS from each partial once (in order). Since none of the partials export SCSS bindings (no `$vars`, mixins, or functions), the `@use` is purely an inclusion mechanism. The order matters: `_tokens` must come first so `--color-text` exists when `_base.scss` reads it; `_layout` and `_utilities` come after `_base` so utility-first compositions cascade correctly.

6. **`src/scaffold/styles/_vars.scss`** — deleted from disk.

7. **`src/scaffold.rs`** — surgical edits:
   - Remove the `TPL_VARS_SCSS` constant (line 19 in current `scaffold.rs`).
   - Add four new constants in the same constant block, between `TPL_APP_SCSS` and `TPL_AGENTS_MD`:
     ```rust
     const TPL_TOKENS_SCSS: &str = include_str!("scaffold/styles/_tokens.scss");
     const TPL_BASE_SCSS: &str = include_str!("scaffold/styles/_base.scss");
     const TPL_LAYOUT_SCSS: &str = include_str!("scaffold/styles/_layout.scss");
     const TPL_UTILITIES_SCSS: &str = include_str!("scaffold/styles/_utilities.scss");
     ```
   - `TPL_APP_SCSS` stays — it still points at `scaffold/styles/app.scss`, whose contents are now the four-line entry.
   - In `write_to`, replace the `_vars.scss` `fs::write` call with four new ones (`_tokens.scss`, `_base.scss`, `_layout.scss`, `_utilities.scss`). The existing `fs::create_dir_all(root_dir.join("styles"))?` already creates the directory. Order: write `_tokens.scss` first (matches `@use` order, purely cosmetic), then `_base.scss`, `_layout.scss`, `_utilities.scss`, then `app.scss` last.

8. **`src/scaffold.rs` tests** — in `mod tests`:
   - **`write_to_emits_all_files`**: replace the two `_vars.scss` assertions with `assert!(!read_to_string(_tokens.scss).is_empty())`, `_base.scss`, `_layout.scss`, `_utilities.scss`. Update the `app.scss` assertion: replace `assert!(app_scss.contains("@use 'vars'"))` with assertions that `app_scss` contains `"@use 'tokens'"`, `"@use 'base'"`, `"@use 'layout'"`, and `"@use 'utilities'"`.
   - **`vars_scss_bridges_tokens_to_root`**: renamed to `tokens_scss_declares_tokens_directly`. Asserts:
     - `_tokens.scss` contains `--color-primary:` (anywhere — light declaration on `:root`).
     - `_tokens.scss` does **not** contain `$color-primary` (no SCSS variable bridge survives).
     - `_tokens.scss` contains `@media (prefers-color-scheme: dark)` (system-preference dark block).
     - `_tokens.scss` contains `[data-theme="dark"]` (explicit dark override).
     - `_tokens.scss` contains `[data-theme="light"]` (explicit light override).
   - **`write_to_index_html_links_to_scss`**: unchanged (still `/styles/app.scss`).
   - Other existing tests (`write_to_app_ts_imports_zero`, `write_to_agents_md_has_section_sentinels`, `write_to_emits_home_test_ts`): unchanged. None reference `_vars.scss`.

9. Run `cargo test --lib` after the edits. All scaffold tests must pass before moving to Step 2.

**Tests:**
- `write_to_emits_all_files` — verifies that `_tokens.scss`, `_base.scss`, `_layout.scss`, `_utilities.scss`, and `app.scss` all land in `<root>/styles/`, are non-empty, and `app.scss` contains all four `@use` directives.
- `tokens_scss_declares_tokens_directly` — verifies the no-SCSS-var-bridge contract and the presence of all three dark-mode pathways.
- `write_to_index_html_links_to_scss` — unchanged. Confirms the `<link>` still points at the SCSS entry.
- The remaining unchanged scaffold tests confirm nothing else broke.
- `cargo test` overall — confirms the existing SCSS dev/build integration tests (`scss_build.rs`, `scss_dev.rs`) still pass against the new scaffold output. Those tests don't assert on `_vars.scss` directly; they just run `zero init` and then compile `app.scss`, which will now expand the four partials.

---

### Step 2: Wire the design system into the scaffold demo (`index.html` + `home.ts`)

**Goal:** A fresh `zero init` project should render with the design system *visible* — proper light/dark backgrounds, system font, a stacked layout with breathing room, and a button that looks intentional rather than browser-default. This step doesn't add features; it edits the two scaffold files whose output a user sees first when they run `zero dev` after `zero init`. Step 1 made the classes exist; Step 2 puts them on screen.

**Files:**
- `src/scaffold/index.html`
- `src/scaffold/src/routes/home.ts`

**Changes:**

1. **`src/scaffold/index.html`** — small adjustments so the dark-mode story is complete from the very first render:
   - Add `<meta name="color-scheme" content="light dark">` to `<head>`, immediately after the existing `<meta name="viewport">` line. This tells the browser that the document supports both schemes, so user-agent UI (scrollbars, default form controls, focus outlines) follows the active theme. Without it, scrollbars and default controls stay in light mode even when `prefers-color-scheme: dark` is active and the page's `--color-bg` has gone dark.
   - Optionally (and shipped here): add a short HTML comment above `<html lang="en">` documenting the theme override:
     ```html
     <!-- To force a theme regardless of system preference, set data-theme="light" or data-theme="dark" on <html>. -->
     ```
   - The `<link rel="stylesheet" href="/styles/app.scss">` line is **unchanged**. The `<div id="app"></div>` is **unchanged** — the framework's "developer owns index.html, no magic" stance means we do not inject classes on `<body>` or `#app`. Visible structure (the `stack`, `cluster`, etc.) goes inside the route component (`home.ts`), where the user can see and edit it.

   Final `index.html`:
   ```html
   <!DOCTYPE html>
   <!-- To force a theme regardless of system preference, set data-theme="light" or data-theme="dark" on <html>. -->
   <html lang="en">
   <head>
     <meta charset="UTF-8">
     <meta name="viewport" content="width=device-width, initial-scale=1.0">
     <meta name="color-scheme" content="light dark">
     <title>{{title}}</title>
     <link rel="stylesheet" href="/styles/app.scss">
   </head>
   <body>
     <div id="app"></div>
   </body>
   </html>
   ```

2. **`src/scaffold/src/routes/home.ts`** — rewrite to use design-system classes. The component still has the same counter behaviour (state via `inject`, button increments) so the existing `home.test.ts` continues to pass without modification — `text(el, "p")` still finds the `<p>` inside `Counter()`, and `find(el, "button")` still finds the single button.

   New body:
   ```ts
   import { html, inject } from "zero";
   import type { Signal, TemplateResult } from "zero";

   function Counter(): TemplateResult {
     return html`<p>Count: ${() => inject<Signal<number>>("count").val}</p>`;
   }

   export default function Home(): TemplateResult {
     return html`
       <main class="stack pad-xl">
         <h1>Hello from zero</h1>
         <div class="cluster gap-md">
           <button class="pad-sm border" @click=${() => inject<Signal<number>>("count").update(n => n + 1)}>Increment</button>
           ${Counter()}
         </div>
       </main>
     `;
   }
   ```

   What this demonstrates to a new user reading their fresh scaffold:
   - `stack pad-xl` on `<main>` — the page content is a vertical stack with `--space-xl` (3rem) of padding around it. First exposure to a layout primitive + a utility class.
   - `cluster gap-md` on the inner row — horizontal flex row with `--space-md` between the button and the counter readout. Demonstrates the override pattern (`cluster`'s default gap is `var(--space-md)`, and `gap-md` happens to match; both are shown to make the override semantics obvious).
   - `pad-sm border` on the button — gives the bare `<button>` a padded, bordered appearance using only utility classes. Reinforces the spec's stance that the design system does **not** ship a `.btn` class — buttons belong to the future component library, and until then, utility composition is the path.
   - `<h1>` stays unstyled (the base layer doesn't style headings) — uses browser default, demonstrating the minimal-base-layer principle. If users want heading typography, they add it to `app.scss` or wait for the component library.

3. **`src/scaffold/src/routes/home.test.ts`** — **unchanged.** The assertions are structural (presence of one `<p>` and one `<button>`, click semantics), not visual. The new markup still satisfies them.

**Tests:**
- `write_to_index_html_links_to_scss` (existing in `scaffold.rs` tests) — continues to assert `<link rel="stylesheet" href="/styles/app.scss">`. Unchanged.
- Existing scaffold test `write_to_emits_all_files` already asserts `home_ts.contains("Hello from zero")` — the new home.ts still contains that string. Unchanged.
- Existing scaffold test `write_to_app_ts_imports_zero` is unaffected (we don't touch `app.ts`).
- Scaffolded-project test: when `zero init` is run, the resulting project's `home.test.ts` still passes against the new `home.ts` (this is exercised transitively by `tests/e2e_init_test.rs` and the dev/build integration tests that run the scaffold output).
- Consider adding one targeted assertion to `scaffold.rs`'s `write_to_emits_all_files` test: `assert!(home_ts.contains("class=\"stack pad-xl\""))` — a sentinel that proves the design-system-using home.ts shipped and didn't silently regress to the old shape. This is the only Rust-side test change in Step 2.

---

### Step 3: Extend `src/scaffold/AGENTS.md`'s Styles section to document the design system

**Goal:** Make the design system discoverable to AI agents and humans reading the scaffold's own reference doc. The Styles section is currently 7 lines (lines 496–505); it gets extended (not rewritten — the existing partial-convention and forbidden-features paragraphs stay).

**Files:**
- `src/scaffold/AGENTS.md`

**Changes:**

After the existing line "Plain `.css` still works..." (line 503) and the "The framework forbids scoped styles..." paragraph (line 505), insert a new subsection. The full appended content:

```markdown
### Design system

The scaffold ships a built-in CSS design system: tokens, theme switching, layout primitives, and utility classes. The system lives in four partials, all `@use`-d from `app.scss`:

| Partial | What it declares |
| --- | --- |
| `_tokens.scss` | CSS custom properties for spacing, colors, radius, type, line height, shadow, and border widths; light and dark theme variants. |
| `_base.scss` | Box-sizing reset and a token-bound `body` rule. No heading or paragraph styling. |
| `_layout.scss` | Six layout primitive classes: `cluster`, `stack`, `frame`, `split`, `flank`, `grid`. |
| `_utilities.scss` | Fifteen utility classes: `gap-{xs,sm,md,lg,xl}`, `pad-{xs,sm,md,lg,xl}`, `border`, `border-{t,r,b,l}`. |

After `zero init`, the partials are normal project files. Edit them, delete them, or replace them. The framework does not regenerate or upgrade them. The future `zero` component library assumes the classes and tokens declared here exist — deleting layers downstream of `_tokens.scss` (e.g. `_utilities.scss`) is safe; deleting `_tokens.scss` itself will break components that read `--color-primary`, `--space-md`, etc.

#### Layout primitives

| Class | Purpose |
| --- | --- |
| `cluster` | Horizontal flex row that wraps. Default `gap: var(--space-md)`. |
| `stack` | Vertical flex column. Default `gap: var(--space-md)`. |
| `frame` | Fixed aspect-ratio box (default `16 / 9`); children centered and clipped. Override per-instance via `--frame-ratio`. |
| `split` | Two equal-width columns. Default `gap: var(--space-md)`. Does not wrap. |
| `flank` | First child is content-sized; second fills. Wraps when narrow. Default `gap: var(--space-md)`. |
| `grid` | Auto-fitting columns of `minmax(min(100%, var(--grid-min, 16rem)), 1fr)`. Default `gap: var(--space-md)`. |

#### Spacing scale

Five steps: `xs`, `sm`, `md`, `lg`, `xl`. Each is a CSS custom property (`--space-xs` … `--space-xl`).

- `gap-{step}` sets `gap: var(--space-{step})` on any flex or grid container.
- `pad-{step}` sets `padding: var(--space-{step})` on any element.

Composition is by class-list order: `class="cluster gap-lg"` overrides the cluster's default `var(--space-md)` because `gap-lg` follows `.cluster` in the compiled CSS. No `!important`, no axis variants — write `padding-inline` directly when you need axis-specific spacing.

#### Border utilities

- `border` — `1px` solid border on all four sides using `--color-border`.
- `border-{t,r,b,l}` — same value, single side. Useful for dividers, accents, sidebar edges.

Thicker borders: override `--border-thin` locally (the design-system border utilities all read it). Width variants (`border-md`, `border-thick`) are not shipped.

#### Theme switching

The system honors `prefers-color-scheme: dark` automatically. To override the system preference, set `data-theme="light"` or `data-theme="dark"` on an ancestor element — canonically `<html>`:

```html
<html data-theme="dark">
```

The framework ships no theme-toggle helper. Persisting a user choice across reloads is one line of JS the user writes:

```js
document.documentElement.dataset.theme = "dark"
```

The dark-mode override applies only to the seven `--color-*` tokens (`--color-bg`, `--color-surface`, `--color-text`, `--color-text-muted`, `--color-primary`, `--color-primary-fg`, `--color-border`). Spacing, radius, type, shadow, and border widths are theme-independent.
```

Additionally, update the project-layout `tree` block earlier in `AGENTS.md` (around line 39–42, the section listing `styles/_vars.scss` and `styles/app.scss`) to list all five files:

```
└── styles/
    ├── _tokens.scss        # SCSS partial — design tokens + theme variants
    ├── _base.scss          # SCSS partial — minimal reset, token-bound body
    ├── _layout.scss        # SCSS partial — six layout primitives
    ├── _utilities.scss     # SCSS partial — gap-*, pad-*, border-* utilities
    └── app.scss            # entry stylesheet — @use 'tokens'; ... 'utilities';
```

Also update the existing "Design tokens live in `styles/_vars.scss`..." line (around line 502): replace it with:

```markdown
- Design tokens are CSS custom properties declared in `styles/_tokens.scss`. Read them everywhere with `var(--name)` — there is no SCSS-variable bridge layer in v1.
```

**Tests:**
- `write_to_agents_md_has_section_sentinels` (existing) — verifies the `## Styles` section sentinel still exists. Unchanged, still passes.
- No new automated test for AGENTS.md content. The integration test in Step 4 covers the CSS surface; AGENTS.md is reference text not under automated assertion.

---

### Step 4: Update `zero-framework-spec.md` §7 and §13 to describe the design system and the direct-CSS-custom-property pattern

**Goal:** Bring the canonical framework spec in line with the new scaffold. The §7 narrative about SCSS as the authoring layer is preserved; the inline `_vars.scss` example (the SCSS-variable bridge pattern) is replaced with the new direct-CSS-custom-property pattern, and a new sub-section describing the design system is appended. §13 gets one new row.

**Files:**
- `zero-framework-spec.md`

**Changes:**

1. **§7 example block (lines 777–805 area)** — replace the `_vars.scss` SCSS code block (the one with `$color-primary: #3b82f6;` and the `:root { --color-primary: #{$color-primary}; ... }` bridge) with the new direct-CSS-custom-property pattern:

   ```scss
   // styles/_tokens.scss — design tokens declared directly as CSS custom properties
   :root {
     --color-primary: #2563eb;
     --color-text:    #1a1a1a;
     --space-md:      1rem;
     --radius-md:     4px;
     // …
   }

   @media (prefers-color-scheme: dark) {
     :root {
       --color-text: #f5f5f5;
       // …only color tokens override in dark mode
     }
   }
   ```

   Adjust the surrounding prose: the existing line "CSS custom properties remain the recommended pattern for runtime theming (e.g. dark mode); SCSS variables are compile-time only." is replaced with: "Design tokens are authored as CSS custom properties directly on `:root` — no SCSS-variable bridge layer. SCSS still owns nesting, partials, and `@use`/`@forward`; it just does not hold token values."

   Replace the `app.scss` example block immediately below the tokens example with the new entry-stylesheet shape:

   ```scss
   // styles/app.scss — entry stylesheet
   @use 'tokens';
   @use 'base';
   @use 'layout';
   @use 'utilities';

   .btn {
     padding: var(--space-sm) var(--space-md);
     border-radius: var(--radius-md);
     &.btn-primary {
       background: var(--color-primary);
       color: var(--color-primary-fg);
     }
   }
   ```

2. **New §7 sub-section** — appended after the existing partial-prefix paragraph (line 807) and before "Components use plain string class names":

   ```markdown
   ### 7.1 Design system

   The scaffold ships a built-in design-system layer in `styles/`: five files (`_tokens.scss`, `_base.scss`, `_layout.scss`, `_utilities.scss`, `app.scss`) that establish a stable foundation for the future component library. After `zero init`, the files are user-owned — editable, deletable, or wholesale replaceable. There is no upgrade path; the framework never patches scaffolded files in-place.

   **Token categories.** Seven categories live in `_tokens.scss`, all declared as CSS custom properties on `:root`:

   | Category | Tokens |
   | --- | --- |
   | Spacing | `--space-xs`, `--space-sm`, `--space-md`, `--space-lg`, `--space-xl` |
   | Colors | `--color-bg`, `--color-surface`, `--color-text`, `--color-text-muted`, `--color-primary`, `--color-primary-fg`, `--color-border` |
   | Radius | `--radius-sm`, `--radius-md`, `--radius-lg` |
   | Font size | `--font-sm`, `--font-md`, `--font-lg`, `--font-xl` |
   | Font weight | `--weight-normal`, `--weight-bold` |
   | Line height | `--leading-tight`, `--leading-normal` |
   | Shadow | `--shadow-sm`, `--shadow-md`, `--shadow-lg` |
   | Border width | `--border-thin`, `--border-md`, `--border-thick` |

   Dark-mode variants override only the seven color tokens.

   **Layout primitives.** Six classes in `_layout.scss`: `cluster`, `stack`, `frame`, `split`, `flank`, `grid`. Each is a single CSS rule; layout primitives never use `margin` for spacing.

   **Utility families.** Three families in `_utilities.scss`: `gap-{step}` (5 classes), `pad-{step}` (5 classes), `border` / `border-{t,r,b,l}` (5 classes). No `!important`; override is by class-list order.

   **Theme switching.** `prefers-color-scheme: dark` selects dark mode by default. Set `data-theme="light"` or `data-theme="dark"` on `<html>` (or any ancestor) to override the system preference. There is no JavaScript theme-toggle helper.

   **Distribution model.** `zero init` writes the partials and leaves them alone. Users own them after init. The future `zero` component library assumes these classes and tokens exist; users who delete the design system accept that downstream components won't render correctly.
   ```

3. **§13 (Key Design Decisions Summary)** — add one row to the table, immediately after the existing CSS row (line 1165):

   ```markdown
   | Design system | Built-in scaffold layer with tokens, themes, layout primitives | Common patterns shouldn't be hand-rolled per project; future component library has a stable foundation |
   ```

No other §13 rows change. The existing "CSS | SCSS authoring layer; CSS variables for runtime theming" row stays — it's about the broader strategy, not the design system specifically.

**Tests:**
- No automated tests. `zero-framework-spec.md` is documentation; integration tests cover the runtime behaviour separately.

---

### Step 5: Add `tests/design_system.rs` integration coverage

**Goal:** Prove the end-to-end path: `zero init` writes the new partials, the build pipeline compiles `app.scss` into CSS containing the expected tokens and class rules, the scaffold's demo `home.ts` references design-system classes (so the build doesn't ship dead CSS), and no SCSS-variable form leaks through. This is the same integration shape as `tests/scss_build.rs` but asserts on design-system-specific content.

**Files:**
- `tests/design_system.rs` (new)

**Changes:**

Create `tests/design_system.rs` with a `write_scss_project` helper and `find_asset` helper that mirror `tests/scss_build.rs` (copy them — they're tiny). Then add two tests:

1. **`build_emits_design_system_css`** — scaffolds with `zero init`, runs `zero build`, reads the hashed CSS from `dist/assets/`, and asserts the compiled CSS contains:
   - `--color-primary:` (proves a token survived compilation)
   - `--space-md:` (proves spacing tokens compile)
   - `--border-thin:` (proves border-width tokens compile)
   - `.cluster {` and `.stack {` (proves layout primitives compile)
   - `.gap-md {` (proves utility classes compile)
   - `.border {` and `.border-t {` (proves border utilities compile)
   - `@media (prefers-color-scheme: dark)` (proves the dark block survives)
   - `[data-theme="dark"]` and `[data-theme="light"]` (proves the explicit overrides survive)

   And does **not** contain `$color-primary` (proves the SCSS-variable form is gone — same shape of assertion as `scss_build.rs`'s existing `$space-md` check).

2. **`build_design_system_passes_contrast_smoke`** — lightweight smoke that the compiled CSS includes a `color: var(--color-text)` rule and a `background: var(--color-bg)` rule under the `body` selector. This is not a real contrast test (would require parsing colors and running WCAG math); it verifies the wiring is intact. The contrast values themselves are baked into the partial and reviewed manually in Step 1.

3. **`scaffold_home_uses_design_system_classes`** — scaffolds with `zero init` and reads `<root>/src/routes/home.ts` from disk. Asserts the source string contains `class="stack pad-xl"`, `class="cluster gap-md"`, and `class="pad-sm border"`. This guards against the demo regressing back to an un-styled `<h1>` + bare `<button>` shape — if a future edit drops the classes, the build's compiled CSS would still contain `.stack { ... }` etc. (because `_utilities.scss` and `_layout.scss` are written unconditionally), and the CSS-side tests above would still pass. This test closes that gap by asserting the *consumer* side.

The tests use `assert_cmd::Command::cargo_bin("zero")` to invoke the binary, matching the existing pattern in `tests/scss_build.rs`.

**Tests:**
- The two tests above are the coverage for Step 5. They re-use the integration harness (`assert_cmd`, `tempfile`) already in `Cargo.toml`'s dev-dependencies. No new dev-dependency is required.
- Existing `tests/scss_build.rs` and `tests/scss_dev.rs` continue to pass; their assertions are agnostic to the design-system content (they check the SCSS → CSS pipeline mechanics, not which CSS).

---

## Risks and Assumptions

- **Token value risk.** The light-mode `--color-primary` was bumped from the spec's implied `#3b82f6` (current `_vars.scss`) to `#2563eb` to clear WCAG AA 4.5:1 contrast against white. If the user wants to keep the brighter `#3b82f6`, they should darken `--color-primary-fg` instead (e.g. to `#0f1115`) — but with default white-on-blue button UI in mind, `#2563eb` is the safer call. Easy to revise during step 1 if the user pushes back.
- **`flank` wrap behaviour.** The plan accepts the spec's natural-flex-wrap behaviour rather than introducing `--flank-threshold`. If real-world use shows the wrap point is unpredictable, a future iteration adds a `min-width`-based threshold knob. Not a blocker for v1.
- **Sass `@use` semantics for CSS-only partials.** `grass` (and modern Sass generally) emits the CSS from `@use 'tokens'` as expected, even though the partial exposes no SCSS bindings. The existing `tests/scss_dev.rs` and `tests/scss_build.rs` already exercise `@use` against a partial that does export an SCSS variable; the new partials' all-CSS-no-bindings case is the simpler subset and should work without surprises. If `grass` proves quirky about this, fallback is `@import` (legacy but supported) — but this is unlikely.
- **No upgrade path.** Existing scaffolded projects on disk keep their `_vars.scss` + old `app.scss`. The framework does not migrate them. This is explicit in the spec and accepted. If users complain, a future `zero upgrade` could add design-system-aware migration; out of scope here.
- **Naming collisions.** Twenty-one global identifiers (`cluster`, `stack`, `frame`, `split`, `flank`, `grid`, plus utility classes) land in the global namespace. The plan accepts the cost. If the user later wants a prefix (`z-cluster`, etc.), it is a one-way door — the future component library will be built against the chosen names.
- **`prefers-color-scheme` browser support.** Evergreen browser baseline (per existing framework spec). Older browsers fall back to the `:root` defaults, which means light mode. No fallback shim is shipped.
- **Test runtime cost.** Step 4's tests invoke `zero init` and `zero build`, which is slower than a unit test. The existing `scss_build.rs` tests already pay this cost; one more file with two tests adds ~2× the existing scss-build time. Acceptable.
