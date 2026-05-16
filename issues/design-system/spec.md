# Spec: Built-in CSS design system

> **Note:** This spec captured the design system as of the
> design-system issue. Requirements 19–20 (scaffold file paths) are
> superseded by `issues/update/spec.md`: `_tokens.scss`, `_base.scss`,
> `_layout.scss`, `_utilities.scss`, and the aggregate `zero.scss` now
> live under `.zero/styles/` and are framework-owned and regenerable
> via `zero update`. The user's `styles/app.scss` remains user-owned
> and is the place to add styles or override tokens.

## Problem Statement

`zero` ships SCSS as the canonical authoring layer (see `issues/scss/spec.md`), but the scaffold's CSS surface is intentionally tiny: `styles/_vars.scss` defines six SCSS tokens and bridges them to CSS custom properties; `styles/app.scss` declares one `body` rule and one `h1` rule. There is no layout vocabulary, no theme story, and no base layer that makes the tokens visible on the page.

That floor is too low for two reasons. First, real applications spend nontrivial time hand-rolling the same five or six layout patterns (cluster, stack, frame, split, flank, grid) and the same handful of spacing/padding utilities. Second — and more importantly — `zero`'s near-term roadmap is a component library (buttons, trays, etc.) that will depend on a stable set of CSS classes, custom properties, and a working light/dark theme contract. Without that contract in place, the component library has no foundation to build on.

This issue introduces a built-in CSS design system: a fixed token palette declared as CSS custom properties, a light + dark theme story that respects both the system preference and an explicit `data-theme` override, a minimal token-aware base layer, six layout primitives (cluster, stack, frame, split, flank, grid), and a small utility surface (`gap-*`, `pad-*` on a t-shirt scale). The artifacts are SCSS partials that `zero init` copies into the project's `styles/` directory. After init, the files are the user's — editable, deletable, or wholesale replaceable. The future component library will assume these classes and tokens exist; users who delete them accept that components downstream will not render correctly.

## Background

### What exists today (relevant pieces)

- **`src/scaffold/styles/_vars.scss`** — six SCSS variables (`$color-primary`, `$color-text`, `$color-bg`, `$space-sm`, `$space-md`, `$radius`) bridged to a `:root { --color-...: #{$...}; }` block. Both forms are addressable: SCSS code can `@use 'vars'` and read `vars.$color-primary`, plain CSS reads `var(--color-primary)`.
- **`src/scaffold/styles/app.scss`** — entry stylesheet. Contains `@use 'vars'`, a `body { ... }` rule using a mix of `vars.$space-md` and `var(--color-text)`, and an `h1 { color: vars.$color-primary }` rule.
- **`src/scaffold/index.html`** — `<link rel="stylesheet" href="/styles/app.scss">`. Build rewrites this href to the hashed output (see `issues/scss/spec.md` requirement 14).
- **`src/scaffold.rs`** — embeds `_vars.scss` and `app.scss` as `include_str!` constants, writes them to `<root>/styles/` during `write_to`. Tests assert the files land and contain specific markers (e.g. `app.scss` must contain `@use 'vars'`; `_vars.scss` must contain `$color-primary:` and the `--color-primary` bridge).
- **`src/scaffold/AGENTS.md`** — has a `## Styles` section that documents the SCSS partial convention, the `vars.scss` → `:root` bridge, and the framework's "no scoped styles" stance.
- **`zero-framework-spec.md` §7** — documents SCSS as the canonical authoring layer and shows the `vars.scss` → `:root` bridge pattern as the recommended runtime-theming approach.
- **`grass` SCSS compiler** (`src/sass.rs`) — runs per request in dev and per file in build, with partial support via `@use 'name'` → `_name.scss` resolution from the importing file's directory.

### Decisions already made

The user has confirmed these in the refine session preceding this spec:

- **Distribution model: scaffold-emits, user-owned.** `zero init` writes the design-system SCSS into the project's `styles/` directory. After that they are normal project files — the framework does not regenerate, patch, or upgrade them. The user can edit, delete, or replace them. The future component library will assume the classes and tokens exist; users who delete the design system accept that those components won't render correctly.
- **Theme switching: system preference + explicit override.** `prefers-color-scheme: dark` selects dark mode by default. `[data-theme="light"]` and `[data-theme="dark"]` on a parent element (canonically `<html>`) override the system preference.
- **Modifier scale: t-shirt sizes.** Utilities (`gap-*`, `pad-*`) use the same `xs / sm / md / lg / xl` suffixes as the spacing tokens.
- **Composition: independent utility classes.** `gap-md` is a single rule (`gap: var(--space-md)`) that works on any flex/grid container. Primitives ship with a sensible default that the utility overrides via class-order. No per-primitive `--gap`-style custom-property indirection.
- **Base layer: minimal reset + token-bound `body`.** Box-sizing reset, `body { color: var(--color-text); background: var(--color-bg); font-family: ... }`. No heading/paragraph typography defaults — those belong to the future component library.
- **`split` is 50/50.** Two children, equal width, no wrapping behavior. A separate `flank` primitive covers the "first child is content-sized, second fills remainder" pattern (the *Sidebar*).
- **Tokens are CSS custom properties, not SCSS variables.** The existing `_vars.scss` model (SCSS var + bridge) is dropped. The design system declares `:root { --color-primary: ...; }` directly. SCSS variables are not used for tokens. (SCSS still owns nesting, partials, `@use` — tokens just don't get the `$var` form anymore.)

### Token surface

Seven categories, all declared as CSS custom properties on `:root` (light theme) with overrides under `@media (prefers-color-scheme: dark) :root`, `[data-theme="dark"]`, and `[data-theme="light"]`:

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

Light-mode values are the defaults on `:root`. Dark-mode values override the **color** category only (the seven `--color-*` tokens, including `--color-border`). Spacing, radius, font sizes/weights, line heights, shadows, and border widths are theme-independent in v1.

### Layout primitives

Six primitives, each a CSS class. Each ships with a sensible default for its primary spacing slot (gap or padding); the default is the relevant `--space-*` token. Utility classes override via class order.

| Class | Semantics |
| --- | --- |
| `cluster` | `display: flex; flex-wrap: wrap; align-items: center; gap: var(--space-md)`. Children flow horizontally and wrap. |
| `stack` | `display: flex; flex-direction: column; gap: var(--space-md)`. Children stack vertically with consistent spacing. |
| `frame` | `aspect-ratio: 16 / 9; overflow: hidden; display: grid; place-items: center`. Children (typically media) are centered and clipped to the aspect box. The `--frame-ratio` custom property allows per-instance override. |
| `split` | `display: grid; grid-template-columns: 1fr 1fr; gap: var(--space-md)`. Two children, equal width, no wrapping. |
| `flank` | `display: flex; gap: var(--space-md); flex-wrap: wrap`. First child is content-sized (`flex: 0 0 auto`); second child fills (`flex: 1 1 0; min-width: 0`). When the container is narrow, the second child's `flex-basis` plus `flex-wrap` pushes it to a new line. |
| `grid` | `display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 16rem), 1fr)); gap: var(--space-md)`. Children flow into as many equal columns as fit; the `16rem` minimum is exposed as `--grid-min` for per-instance override. |

### Utility surface

Three utility families in v1. Each declares one CSS rule per class.

| Family | Classes |
| --- | --- |
| `gap-*` | `gap-xs`, `gap-sm`, `gap-md`, `gap-lg`, `gap-xl`. Each sets `gap: var(--space-{step})`. |
| `pad-*` | `pad-xs`, `pad-sm`, `pad-md`, `pad-lg`, `pad-xl`. Each sets `padding: var(--space-{step})`. |
| `border-*` | `border`, `border-t`, `border-r`, `border-b`, `border-l`. Each sets the corresponding side(s) to `var(--border-thin) solid var(--color-border)`. |

`gap-*` and `pad-*` are intentionally axis-agnostic; `border-*` carries direction variants because single-side borders are the common case (top accents, bottom dividers, sidebar edges). Border *width* variants (`.border-md`, `.border-thick`) are not in v1 — users override `--border-thin` locally for thicker borders. Margin utilities and axis-specific spacing utilities (`pad-x-*`, `gap-y-*`) are explicitly out of scope — see Out of Scope.

### File organization (recommendation; finalized in plan)

The design system ships as a set of SCSS partials in `src/scaffold/styles/`:

```
styles/
├── _tokens.scss      # :root + media-query + [data-theme] declarations
├── _base.scss        # minimal reset + body rule using tokens
├── _layout.scss      # the six layout primitive classes
├── _utilities.scss   # gap-* and pad-* utility classes
└── app.scss          # entry: @use 'tokens'; @use 'base'; @use 'layout'; @use 'utilities';
```

The existing `_vars.scss` is **deleted**. The existing `app.scss` is rewritten to the entry shape shown above. Users who want to drop a layer (e.g. delete `_utilities.scss`) remove the `@use` line in `app.scss` and delete the file.

## Requirements

### Tokens

1. A new partial `src/scaffold/styles/_tokens.scss` declares every CSS custom property listed in the Background's token table. All declarations live in `:root { ... }`. There are **no** SCSS variable declarations in this file (no `$color-primary`, no bridge syntax). The custom properties are authored directly with literal values.
2. Dark-mode overrides for the seven `--color-*` tokens are declared in two additional blocks in the same file:
   - `@media (prefers-color-scheme: dark) { :root { --color-bg: ...; ... } }`
   - `[data-theme="dark"] { --color-bg: ...; ... }` (same values as the media query block; chosen so the explicit override matches the system-preference path).
3. An explicit light-mode override exists for the inverse case:
   - `[data-theme="light"] { --color-bg: ...; ... }` (mirrors the `:root` defaults — needed so that a user can force light mode even when `prefers-color-scheme: dark` is active).
4. The spacing, radius, font-size, font-weight, line-height, and shadow categories are declared **only** under `:root` and do not appear in any dark-mode or `[data-theme]` block. They are theme-independent in v1.
5. Light-mode color values and dark-mode color values are chosen to be visually distinct and pass WCAG AA contrast for `--color-text` against `--color-bg` and `--color-primary-fg` against `--color-primary`. Concrete values are deferred to the plan phase (see Open Questions).

### Base layer

6. A new partial `src/scaffold/styles/_base.scss` declares:
   - `*, *::before, *::after { box-sizing: border-box; }`
   - `body { margin: 0; color: var(--color-text); background: var(--color-bg); font-family: system-ui, sans-serif; font-size: var(--font-md); line-height: var(--leading-normal); }`
7. The base layer does **not** style headings (`h1`-`h6`), paragraphs, links, lists, code, form elements, or any other element. The user's existing visible-content choices are not overridden beyond box-sizing and the `body` rule.

### Layout primitives

8. A new partial `src/scaffold/styles/_layout.scss` defines six CSS classes — `.cluster`, `.stack`, `.frame`, `.split`, `.flank`, `.grid` — with the rules specified in the Background's "Layout primitives" table.
9. Every primitive that has a primary spacing slot uses a `--space-*` token by default (currently `--space-md` for all five non-`frame` primitives). The choice is documented in a comment on each rule.
10. `.frame` exposes `--frame-ratio` (default `16 / 9`) as a per-instance override knob. The rule reads `aspect-ratio: var(--frame-ratio);` and users override with `style="--frame-ratio: 1 / 1"` inline or in a nested rule.
11. `.grid` exposes `--grid-min` (default `16rem`) as a per-instance override knob. The rule reads `grid-template-columns: repeat(auto-fit, minmax(min(100%, var(--grid-min)), 1fr));`.
12. `.flank` keeps the order convention "first child = content-sized, second child = fills". Reversing the order (right-flank) is handled by the user with `flex-direction: row-reverse` on a parent or by reordering children — not by a separate `flank-right` class in v1.
13. Class names are flat and unprefixed (no `zero-cluster`, no `ds-cluster`). Naming collisions with user code are the user's problem; the design system owns these six identifiers.

### Utility classes

14. A new partial `src/scaffold/styles/_utilities.scss` declares fifteen classes — `.gap-xs`, `.gap-sm`, `.gap-md`, `.gap-lg`, `.gap-xl`, `.pad-xs`, `.pad-sm`, `.pad-md`, `.pad-lg`, `.pad-xl`, `.border`, `.border-t`, `.border-r`, `.border-b`, `.border-l` — each consisting of one CSS rule. `gap-*` sets `gap: var(--space-{step})`; `pad-*` sets `padding: var(--space-{step})`; `.border` sets `border: var(--border-thin) solid var(--color-border)`; `.border-t` / `.border-r` / `.border-b` / `.border-l` set `border-top` / `border-right` / `border-bottom` / `border-left` with the same value.
15. `gap-*` and `pad-*` are axis-agnostic (one rule each, no `gap-x`/`gap-y` variants). `border-*` is direction-specific by design (see Background's "Utility surface" rationale).
16. Utility classes do not use `!important`. Override order is by class-list order in the source CSS and by the user writing more specific selectors when needed.

### Entry stylesheet

17. The existing `src/scaffold/styles/app.scss` is rewritten to a four-line entry that pulls the partials in order:
    ```scss
    @use 'tokens';
    @use 'base';
    @use 'layout';
    @use 'utilities';
    ```
    No other rules. Users add their own application styles below the `@use` block. The file remains the entry that `index.html`'s `<link>` points at.
18. The existing `src/scaffold/styles/_vars.scss` is **deleted**. Its constants are subsumed by `_tokens.scss` in CSS-variable form, with the rename `--space-sm` → `--space-sm` (unchanged), `--space-md` → `--space-md` (unchanged), `--radius` → `--radius-md`, plus the new tokens for the expanded surface.

### Scaffold (`src/scaffold.rs`)

19. The `include_str!` constant list is updated:
    - Remove `TPL_VARS_SCSS` and its write call.
    - Add `TPL_TOKENS_SCSS`, `TPL_BASE_SCSS`, `TPL_LAYOUT_SCSS`, `TPL_UTILITIES_SCSS` constants pointing at the new partials.
    - Keep `TPL_APP_SCSS` but point it at the rewritten four-line entry.
    - `write_to` writes each of the five files into `<root>/styles/`.
20. Existing scaffold tests are updated:
    - `write_to_emits_all_files` — replace the `_vars.scss` assertions with assertions that `_tokens.scss`, `_base.scss`, `_layout.scss`, `_utilities.scss`, and the rewritten `app.scss` all exist and are non-empty. Assert `app.scss` contains all four `@use` directives.
    - `vars_scss_bridges_tokens_to_root` — replaced by `tokens_scss_declares_color_primary` which asserts `_tokens.scss` contains `--color-primary:` (without a corresponding `$color-primary` SCSS variable) and contains the dark-mode override block (`@media (prefers-color-scheme: dark)` and `[data-theme="dark"]`).
    - `write_to_index_html_links_to_scss` — unchanged.
21. `zero init` continues to refuse to overwrite a non-empty `<root>/` directory (no change).

### `index.html`

22. No changes. The existing `<link rel="stylesheet" href="/styles/app.scss">` continues to point at the entry; the build still rewrites the href to the hashed output via the existing `issues/scss` machinery.

### `AGENTS.md` (scaffold copy)

23. The existing `## Styles` section in `src/scaffold/AGENTS.md` is extended (not rewritten) with:
    - A short overview of the design system: layout primitives, utility classes, theme switching.
    - A pointer table listing each primitive class and what it does (one line each).
    - A pointer table listing the spacing scale and how `gap-*` / `pad-*` consume it.
    - A short note on the border utilities (`border`, `border-t/r/b/l`) and how to thicken via local `--border-thin` override.
    - The theme-switching contract: `prefers-color-scheme` is honored by default; set `data-theme="light"` or `data-theme="dark"` on a parent (typically `<html>`) to override.
    - A note that the partials are user-owned: edit/delete freely, but the future `zero` component library assumes these classes and tokens exist.

### Framework spec (`zero-framework-spec.md`)

24. Section 7 ("CSS Strategy") is extended with a new sub-section documenting the design system:
    - Token categories (the seven listed above).
    - Layout primitives (the six listed above).
    - Utility families (`gap-*`, `pad-*`).
    - Theme switching contract (system + override).
    - "Scaffold-emits, user-owned" distribution model.
    The existing wording about SCSS as the authoring layer, the partial convention, and the `:root` bridge for runtime theming is updated: the `:root` bridge example becomes the new direct-CSS-custom-property pattern (no SCSS-variable bridge), and the new sub-section is added below it.
25. Section 13 ("Key Design Decisions Summary") gets a new row:
    `| Design system | Built-in scaffold layer with tokens, themes, layout primitives | Common patterns shouldn't be hand-rolled per project; future component library has a stable foundation |`.

### Integration coverage

26. An integration test (under `tests/`, parallel to the SCSS integration tests) covers the end-to-end path:
    - Scaffold a project with `zero init`.
    - Run `zero dev` (or `zero build`).
    - `GET /styles/app.scss` returns CSS that contains `--color-primary:`, `--space-md:`, `--border-thin:`, `.cluster {`, `.gap-md {`, `.border {`, `.border-t {`, and the dark-mode `@media (prefers-color-scheme: dark)` block.
    - The compiled CSS does not contain `$color-primary` (proves no SCSS variables leaked through).

## Constraints

- **No JavaScript.** The design system is pure CSS (delivered via SCSS partials). No theme-toggle helper script ships in v1; users wire up their own `document.documentElement.dataset.theme = "dark"` if they want a UI toggle.
- **No SCSS variables for tokens.** Tokens are CSS custom properties only. `_tokens.scss` contains zero `$var:` declarations. SCSS is still used for partials, `@use`, and nesting elsewhere — just not for tokens.
- **No new Rust dependencies.** The system rides on the existing `grass` SCSS pipeline; nothing else is added to `Cargo.toml`.
- **No new npm dependencies.** (Framework-wide constraint, restated.)
- **No CSS-in-JS, no scoped styles, no CSS modules.** Framework-wide constraint; restated to be explicit that the design system does not introduce any of these.
- **No `!important` in utility classes.** Override is by source order and selector specificity, not by escalation.
- **No `margin` in layout primitives.** The six layout classes (`cluster`, `stack`, `frame`, `split`, `flank`, `grid`) and their children-selectors (e.g. `.stack > * + *`) must achieve spacing with `gap`, `padding`, `flex`, or `grid` properties — not with `margin`. The plan phase should confirm there is no margin-based fallback hiding anywhere in the layout partial. The only acceptable margin in the design system is `body { margin: 0 }` in `_base.scss` (a reset, not a layout mechanism). If a primitive cannot be expressed without margin (none in the current six are believed to need it), flag it in Open Questions before writing CSS — margin is a last resort, not a default.
- **No vendor prefixing.** The framework targets evergreen browsers; CSS uses the unprefixed standard properties (`aspect-ratio`, `gap`, `inset`, etc.).
- **No breakpoint media queries in v1.** Responsiveness comes from intrinsic layout (flex-wrap, auto-fit grid, flank's natural wrap). No `--bp-sm`-style tokens, no `@media (min-width: ...)` blocks in the layout/utility partials. The `prefers-color-scheme` media query in `_tokens.scss` is the only media query the design system ships.
- **No content beyond the listed surface.** No buttons, forms, tables, alerts, badges, modals, tooltips, etc. in v1 — those belong to the future component library.
- **The scaffold files are user-owned post-init.** No "upgrade" path. If a future framework version changes the design-system canon, users on existing projects keep what they have until they manually re-pull.

## Out of Scope

- **Component library.** Buttons, trays, modals, tooltips, form controls, tables, alerts, badges. Future work. This issue lays the foundation; the components live in a separate issue and assume the foundation exists.
- **Theme toggle helper.** No `setTheme(...)` function, no localStorage persistence, no `<theme-toggle>` component. Users wire this up themselves with one or two lines of JS.
- **Typography defaults beyond `body`.** Headings, paragraphs, lists, links, code blocks — not styled by the base layer. The future component library may introduce a typography stack.
- **Axis-aware utility variants.** `pad-x-md`, `pad-y-lg`, `gap-x-sm` are not shipped. Users who need them write a one-off class or use `padding-inline` / `padding-block` directly.
- **Margin utilities (`m-*`).** Out for v1. Spacing should come from primitive gaps (cluster, stack) or explicit padding, not from per-element margins.
- **Color utilities (`text-primary`, `bg-surface`).** Out for v1. The component library will introduce semantic color application; bare color utilities are a slippery slope toward Tailwind-scale CSS surface.
- **Breakpoint tokens and responsive utilities.** No `--bp-md`, no `md:gap-lg` syntax. Layout primitives rely on intrinsic responsiveness.
- **Z-index tokens, transition tokens, animation tokens.** Deferred until a concrete component needs them.
- **CSS reset beyond minimal.** No normalize.css-scale neutralization of browser defaults. Just `box-sizing` and the `body` rule.
- **A documentation site or playground.** `AGENTS.md` documents the surface; the framework spec mirrors it. No separate doc site in this issue.
- **Right-flank, top-flank, etc. variants of `flank`.** One direction only; users compose with `flex-direction: row-reverse` or by reordering markup.
- **Switcher primitive.** Two-columns-flip-to-rows-below-threshold is covered approximately by `grid` with `auto-fit`. No dedicated `switcher` class in v1.
- **Cover primitive.** Full-viewport-centered-content is one rule the user can write; no dedicated class in v1.
- **Migrating existing projects' `_vars.scss` to the new structure.** New scaffolds get the new layout; existing scaffolds keep what they have. There is no upgrade command.

## Open Questions

- **Concrete token values.** Spacing scale: `--space-xs` = `0.25rem`? `0.125rem`? `--space-md` = `1rem` (matches current `_vars.scss`)? `--space-xl` = `2rem`? `4rem`? The plan should propose specific values for all 22 tokens (5 spacing, 7 colors × 2 themes, 3 radius, 4 font-size, 2 font-weight, 2 line-height, 3 shadow, 3 border width). Use a published, well-reasoned scale (e.g. a major-third type scale for fonts, a doubling scale for spacing) rather than ad-hoc numbers. Suggested border widths: `--border-thin` = `1px`, `--border-md` = `2px`, `--border-thick` = `4px`.
- **Light and dark color palette.** Specific hex/oklch values for the seven color tokens in each theme. The plan should produce values that meet WCAG AA for `--color-text` against `--color-bg` and `--color-primary-fg` against `--color-primary`. Concretely: pick two palettes (light + dark) and write them into `_tokens.scss`.
- **Should `data-theme` live on `<html>` or `<body>`?** Convention is `<html>` (matches `prefers-color-scheme` cascade, avoids FOUC issues with body-level overrides). The plan should document the recommendation in `AGENTS.md` and the spec but the CSS works on either since `[data-theme]` is an ancestor selector.
- **File organization granularity.** Four partials (`_tokens`, `_base`, `_layout`, `_utilities`) is the recommendation. Alternative: one `_design.scss` containing everything. The plan should confirm the split is worth the file-count overhead — the argument for splitting is that users can delete `_utilities.scss` without touching tokens; the argument against is five files where one would do.
- **Default cluster/stack/split/flank/grid gap value.** Spec proposes `--space-md` for all. The plan should confirm or pick per-primitive defaults (e.g. `stack` might want a larger default).
- **Frame default aspect ratio.** Spec proposes `16 / 9`. Alternative: `1 / 1` (square) or no default (require explicit `--frame-ratio`). The plan should pick.
- **`flank` wrap threshold.** With `flex-wrap: wrap` and `flex: 1 1 0` on the fill child, wrapping happens naturally when the fill child's `flex-basis` plus the content child's intrinsic width exceeds the container. There's no explicit threshold. The plan should confirm this matches the spec's "wraps when narrow" intent, or introduce a `--flank-threshold` custom property and use a `min-width`-with-flex-basis trick to make the threshold explicit.
- **`grid` minimum column width default.** Spec proposes `16rem`. The plan should confirm this is right for "cards" and similar typical use; an alternative is `20rem` or `12rem`.
- **Should the `body` font-family be `system-ui, sans-serif` or a more specific stack?** Spec proposes the former. The plan can substitute a richer stack if there's a strong reason (e.g. wider system coverage).
- **Naming clash policy.** Six unprefixed layout classes (`cluster`, `stack`, `frame`, `split`, `flank`, `grid`) and fifteen utility classes (`gap-*`, `pad-*`, `border`/`border-t/r/b/l`) is twenty-one identifiers in the global namespace. The spec accepts this as a deliberate cost. The plan should confirm the user agrees, or introduce a short prefix (`z-cluster`, `z-stack`, ...) — but introducing a prefix is a one-way door because the future component library will use these classes.
- **Existing scaffold AGENTS.md `## Styles` section — extend or replace?** Spec says extend. The plan should produce the exact rewritten text.
- **Framework-spec §7 — surgical edit or full rewrite of the section?** Spec says surgical (the SCSS narrative stays; the design-system sub-section is appended; the `_vars.scss` bridge example is replaced with a direct-CSS-custom-property example). The plan should produce the exact diff.
- **Should the integration test live alongside `tests/scss_*` or get its own file?** Recommendation: new file `tests/design_system.rs`. The plan should confirm.
