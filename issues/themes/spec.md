# Spec: CSS themes & expanded color palette

## Problem Statement

The framework's design-system tokens have three concrete problems today:

1. **Duplicated color declarations.** `.zero/styles/_tokens.scss` declares the 7 color tokens four times — once on `:root` (light), again in `@media (prefers-color-scheme: dark) :root`, again in `[data-theme="dark"]`, again in `[data-theme="light"]`. The light block is byte-identical between `:root` and `[data-theme="light"]`; the dark block is byte-identical between the media query and `[data-theme="dark"]`. The same hex value lives in two places, so any tweak has to be made twice.
2. **Components leak their own private color tokens with hardcoded hex.** `_button.scss`, `_badge.scss`, and `_toast.scss` each declare one-off tokens (`--button-danger-bg: #e03131`, `--badge-success-bg: #2f9e44`, `--toast-warning-bg: #f59f00`, etc.) because the shared palette only exposes a single `--color-primary`. Those tokens never change with theme — dark mode users see the same `#e03131` and `#f59f00` on a dark background, which looks wrong and is impossible to override cleanly.
3. **No room for derived shades.** With only `--color-primary`, components can't express hover/active/border variants without falling back to hardcoded hex or filter hacks.

The user-facing definition of done: a developer can drop a new `[data-theme="brand"]` block into their `styles/app.scss`, declare the public `--color-*` semantic tokens, and have every shipped component re-skin. The framework's *own* light and dark themes live in dedicated files, each authoring its values exactly once.

## Background

### Current state

- `.zero/styles/_tokens.scss` holds 8 categories of CSS custom properties on `:root`: spacing (5), colors (7), radius (3), font sizes (4), font weights (2), line heights (2), shadows (3), border widths (3). Plus the duplicate dark blocks described above.
- `.zero/styles/zero.scss` is the framework aggregate: `@use 'tokens'; @use 'base'; @use 'layout'; @use 'utilities'; @use 'alignment'; @use 'components';`. The user's `styles/app.scss` consumes it via `@use '../.zero/styles/zero';`.
- 14 component partials live under `.zero/styles/components/_<name>.scss`. Each is wrapped in `@layer components { ... }` so unlayered user CSS wins by default.
- Three component partials embed private color tokens with hardcoded hex:
  - `_button.scss`: `--button-danger-bg: #e03131`, `--button-danger-fg: #ffffff`
  - `_badge.scss`: `--badge-{success,warning,danger}-{bg,fg}` (6 tokens)
  - `_toast.scss`: `--toast-{success,warning,danger}-{bg,fg}` (6 tokens)
- Theme switching: `data-theme="light"` or `data-theme="dark"` on `<html>` (or any ancestor) overrides `prefers-color-scheme`. The showcase exposes an `auto | light | dark` switcher that flips `document.documentElement.dataset.theme`. There is no JS theme-toggle helper in the framework runtime.
- Distribution: `_tokens.scss` and every other file under `.zero/` is shipped from `src/scaffold/.zero/...`, embedded via `include_str!` in `src/scaffold.rs`, written by `zero init`, and refreshed by `zero update`. The manifest is `framework_manifest()` around `src/scaffold.rs:100`.
- The user's override path: re-declare the public token in `styles/app.scss` after the framework `@use` line. `.zero/` itself is `.gitignore`-d and framework-owned — users do not edit it.
- Specification reference: `zero-framework-spec.md` §7.1 documents the current token categories and theme switching behavior. This spec supersedes the "Token categories" table and "Theme switching" paragraph there.

### Inspiration: Shoelace

Shoelace (`shoelace.style/dist/themes/light.css`) ships a two-tier system: a primitive palette (`--sl-color-gray-50` … `--sl-color-gray-950`, plus blue/red/green/amber/etc. families) declared on `:root, .sl-theme-light`, and a smaller semantic surface (`--sl-color-primary-600`, `--sl-color-neutral-1000`, etc.) layered on top. Light and dark live in separate files. Components consume the semantic layer.

The model adapted to zero: ship the palette, ship two themes, but keep the *public* override surface to the semantic `--color-*` names. Users authoring app-level styles or supplying a new theme operate on semantic tokens only; they never have to touch the palette to make light/dark variants for their brand color.

## Requirements

### File layout

The framework ships these files under `.zero/styles/`:

```
.zero/styles/
  _palette.scss        # NEW — 55 palette tokens, theme-invariant
  _tokens.scss         # narrowed — non-color tokens only (spacing, radius, font, weight, leading, shadow, border)
  _themes.scss         # NEW — aggregator + selector strategy
  themes/              # NEW — one file per theme
    _light.scss        # NEW
    _dark.scss         # NEW
  _base.scss           # unchanged
  _layout.scss         # unchanged
  _utilities.scss      # unchanged
  _alignment.scss      # unchanged
  _components.scss     # unchanged (aggregator over components/)
  components/          # individual component partials, several edited (see below)
  zero.scss            # updated @use order
```

`zero.scss` becomes:

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

`@use 'palette'` precedes `@use 'themes'` so theme files can reference palette variables. Both precede `@use 'components'` so component partials can reference the semantic tokens defined by themes.

### Palette: `_palette.scss`

55 CSS custom properties on `:root`. Five families × 11 steps. Theme-invariant — declared once, no overrides.

- Families: `gray`, `blue`, `red`, `green`, `amber`.
- Steps: `50, 100, 200, 300, 400, 500, 600, 700, 800, 900, 950`.
- Token names: `--gray-50` … `--gray-950`, `--blue-50` … `--blue-950`, `--red-50` … `--red-950`, `--green-50` … `--green-950`, `--amber-50` … `--amber-950`.
- Concrete values: pick perceptually-uniform ramps. Tailwind's Open Color steps are a known-good reference; exact values are an implementation choice for the planner. Each ramp goes light → dark across the 11 steps with `500` near the middle.
- Status as API surface: palette tokens are CSS custom properties on `:root`, so they are technically reachable by user code. Documented status is **framework-internal**: the framework reserves the right to add steps or adjust values across minor versions. Users authoring app-level styles or new themes are guided to consume the semantic `--color-*` tokens instead. This guidance is documented in §7.1 of the spec and in `BEST_PRACTICES.md`.

### Non-color tokens: `_tokens.scss` (narrowed)

`_tokens.scss` keeps every category currently in it *except* the seven color tokens and the four duplicated theme blocks, and gains two new font-family tokens. The file becomes purely theme-invariant non-color tokens, declared once on `:root`:

- Spacing (5): `--space-xs`, `--space-sm`, `--space-md`, `--space-lg`, `--space-xl`
- Radius (3): `--radius-sm`, `--radius-md`, `--radius-lg`
- Font family (2, NEW): `--font-sans`, `--font-mono`
- Font size (4, RENAMED): `--font-size-sm`, `--font-size-md`, `--font-size-lg`, `--font-size-xl`
- Font weight (2): `--weight-normal`, `--weight-bold`
- Line height (2): `--leading-tight`, `--leading-normal`
- Shadow (3): `--shadow-sm`, `--shadow-md`, `--shadow-lg`
- Border width (3): `--border-thin`, `--border-md`, `--border-thick`

Font-family token values:

- `--font-sans: system-ui, sans-serif;` — replaces the hardcoded value currently in `_base.scss` body rule.
- `--font-mono: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;` — new, available for user code blocks and any future framework component that needs monospace.

Font-size rename: the existing `--font-{sm,md,lg,xl}` tokens are renamed to `--font-size-{sm,md,lg,xl}`. This eliminates the prefix collision with the new font-family tokens (`--font-*` would otherwise mean both "family" and "size"). Touchpoints: every shipped component partial that consumes `var(--font-sm|md|lg|xl)` (button, badge, card, checkbox, dialog, input, radio, select, textarea, toggle), plus the user-owned example app stylesheets at `examples/{counter,todos,tracker}/web/styles/app.scss`. Backward compatibility is not preserved — user projects with `var(--font-md)` references must update to `var(--font-size-md)`.

Font-family is theme-invariant by default. A user who wants different fonts per theme overrides `--font-sans` inside their own `[data-theme="x"]` block in `styles/app.scss` — same mechanism as overriding a color token.

The `@media (prefers-color-scheme: dark)`, `[data-theme="dark"]`, and `[data-theme="light"]` blocks are deleted from this file — they move to `_themes.scss`.

### Public semantic surface

The public, documented `--color-*` tokens grow from 7 to 13. Every shipped component consumes only these names and the non-color tokens above. No component declares its own private color token.

| Token | Light default | Dark default |
| --- | --- | --- |
| `--color-bg` | `var(--gray-50)` | `var(--gray-950)` |
| `--color-surface` | `var(--gray-100)` | `var(--gray-900)` |
| `--color-text` | `var(--gray-900)` | `var(--gray-50)` |
| `--color-text-muted` | `var(--gray-600)` | `var(--gray-400)` |
| `--color-border` | `var(--gray-200)` | `var(--gray-800)` |
| `--color-primary` | `var(--blue-600)` | `var(--blue-400)` |
| `--color-primary-fg` | `var(--gray-50)` | `var(--gray-950)` |
| `--color-success` | `var(--green-600)` | `var(--green-500)` |
| `--color-success-fg` | `var(--gray-50)` | `var(--gray-950)` |
| `--color-warning` | `var(--amber-500)` | `var(--amber-400)` |
| `--color-warning-fg` | `var(--gray-900)` | `var(--gray-950)` |
| `--color-danger` | `var(--red-600)` | `var(--red-500)` |
| `--color-danger-fg` | `var(--gray-50)` | `var(--gray-50)` |

Exact step choices are an implementation decision for the planner; the table above documents intent. The constraint is: every semantic token resolves to a palette value (no raw hex) and every shipped component renders with adequate contrast against `--color-bg` and `--color-surface` in both themes.

### Theme files: `themes/_light.scss` and `themes/_dark.scss`

Each theme partial defines a single Sass `@mixin tokens` containing its `--color-*` assignments. The mixin emits the declarations; the partial does not contain any CSS rule on its own.

```scss
// themes/_light.scss
@mixin tokens {
  --color-bg:           var(--gray-50);
  --color-surface:      var(--gray-100);
  --color-text:         var(--gray-900);
  --color-text-muted:   var(--gray-600);
  --color-border:       var(--gray-200);
  --color-primary:      var(--blue-600);
  --color-primary-fg:   var(--gray-50);
  --color-success:      var(--green-600);
  --color-success-fg:   var(--gray-50);
  --color-warning:      var(--amber-500);
  --color-warning-fg:   var(--gray-900);
  --color-danger:       var(--red-600);
  --color-danger-fg:    var(--gray-50);
}
```

```scss
// themes/_dark.scss
@mixin tokens {
  --color-bg:           var(--gray-950);
  // ... same names, dark-appropriate palette steps ...
}
```

Authoring constraint: every theme partial declares the same 13 token names. The mixin pattern keeps each value authored exactly once — no duplication across selectors.

### Aggregator: `_themes.scss`

`_themes.scss` owns the selector strategy. It includes each theme mixin under the appropriate selector, with the dark mixin included a second time inside the `prefers-color-scheme: dark` media query so system-preference fallback works:

```scss
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

Specificity check (must hold for the implementation to be correct):

- **No `data-theme`, system in light mode** → only `:root, [data-theme="light"]` matches → light values.
- **No `data-theme`, system in dark mode** → `:root` matches twice (specificity 1 each); media-query block comes later in source order → dark values win on source order.
- **`data-theme="light"` on a dark system** → `[data-theme="light"]` matches with specificity 10; media-query `:root` matches with specificity 1 → light values win on specificity.
- **`data-theme="dark"` on any system** → `[data-theme="dark"]` specificity 10 → dark values.

This preserves the existing behavior documented in §7.1: `prefers-color-scheme` selects dark by default; `data-theme` overrides the system preference.

### Base and component refactor

`_base.scss` and three component partials change. `_base.scss` switches to the new font-family token; the components lose their private color tokens and switch to the new semantic tokens:

- **`_base.scss`** — body rule consumes `var(--font-sans)` instead of the hardcoded `system-ui, sans-serif;` literal. No other changes.
- **`_button.scss`** — delete `--button-danger-bg` and `--button-danger-fg` from the `:root` block; `.button-danger` consumes `var(--color-danger)` and `var(--color-danger-fg)`. The `:root { ... }` block inside `@layer components { ... }` is removed entirely (it has nothing left to declare).
- **`_badge.scss`** — delete the 6 `--badge-{success,warning,danger}-{bg,fg}` tokens. `.badge-success`, `.badge-warning`, `.badge-danger` consume `var(--color-{success,warning,danger})` and `var(--color-{success,warning,danger}-fg)`. The component partial's `:root { ... }` declaration block is removed.
- **`_toast.scss`** — delete the 6 `--toast-{success,warning,danger}-{bg,fg}` tokens. `.toast-success`, `.toast-warning`, `.toast-danger` consume the matching semantic tokens. The component partial's `:root { ... }` block is removed.

The other 11 component partials already consume only `--color-*` semantic tokens and the non-color tokens; no changes required.

### Distribution: scaffold manifest

`framework_manifest()` in `src/scaffold.rs` must be updated. The diff:

- **Add** four new `include_str!` constants and manifest entries:
  - `.zero/styles/_palette.scss`
  - `.zero/styles/_themes.scss`
  - `.zero/styles/themes/_light.scss`
  - `.zero/styles/themes/_dark.scss`
- **Update** content of:
  - `.zero/styles/_tokens.scss` (color tokens and theme blocks removed; `--font-sans` and `--font-mono` added)
  - `.zero/styles/_base.scss` (body rule consumes `var(--font-sans)`)
  - `.zero/styles/zero.scss` (new `@use` order; adds `palette` and `themes`)
  - `.zero/styles/components/_button.scss` (private tokens removed; consumes semantic)
  - `.zero/styles/components/_badge.scss` (same)
  - `.zero/styles/components/_toast.scss` (same)

No entries are removed from the manifest. Existing user projects that run `zero update` see an `A` (add) line for each of the four new files and a `U` (update) line for each of the six edited files. Decline behavior is unchanged — `zero update` accepts per-operation reject in interactive mode.

### Migration notes

Backward compatibility is **not a constraint** on this change — the framework is pre-1.0 and willing to break user overrides when the design wins. As it happens, the existing 7 public color token names (`--color-bg`, `--color-surface`, `--color-text`, `--color-text-muted`, `--color-primary`, `--color-primary-fg`, `--color-border`) are preserved by this change incidentally — they're still the cleanest names for those slots. Users with override blocks in `styles/app.scss` for any of these continue to work.

The 13 component-private tokens (`--button-danger-*`, `--badge-{success,warning,danger}-*`, `--toast-{success,warning,danger}-*`) are removed. If a user happened to override one of these tokens, their override silently has no effect after the change; they should switch to the new `--color-{success,warning,danger}` tokens, which now apply globally. No migration tooling is shipped.

Users can author additional themes (`[data-theme="brand"]`, etc.) by declaring the 13 public tokens in their own SCSS partial and `@use`-ing it from `styles/app.scss` after the framework aggregate. No JS API required.

### Documentation

Three documents update:

- **`zero-framework-spec.md` §7.1** — replace the current "Token categories" table with the new categories: palette (new section, 5 families × 11 steps), spacing/radius/font/etc. (unchanged), semantic colors (the 13-token table above). Update the "Theme switching" paragraph to reference the new file layout.
- **`BEST_PRACTICES.md`** — add a "Theming" section: how to override tokens in light/dark, how to author a new `[data-theme="brand"]`, note that the palette is framework-internal.
- **`AGENTS.md`** — update the design-system mentions to reflect the new file paths.

### Tests

- Framework-side: `tests/scaffold.rs` (if it asserts the set of files written) must include the four new entries.
- A new test under `tests/` that compiles the framework SCSS and asserts that:
  1. `:root` declares the palette (55 tokens) and the non-color tokens.
  2. `[data-theme="light"]` resolves `--color-primary` to a non-empty value (and ideally the expected palette reference).
  3. `[data-theme="dark"]` resolves `--color-primary` to a different value than light.
  4. The 13 private component tokens (`--button-danger-bg`, `--badge-*`, `--toast-*`) are not present anywhere in the compiled CSS.
- Showcase build + dev tests (`tests/showcase_build.rs`, `tests/showcase_dev.rs`) continue to pass; the showcase's existing `auto | light | dark` switcher exercises the theme strategy and must visually render correctly in both modes.
- Component unit tests under `.zero/components/<Name>.test.ts` need no changes — they assert on HTML structure, not computed styles.

## Constraints

- **`zero, no magic` framework stance.** No JS theme-toggle helper. Theme switching remains pure CSS + `data-theme` attribute. (§7.1 already states this.)
- **Single compiled stylesheet.** Both themes ship in the compiled output. Users cannot opt out of dark mode at compile time — gating themes by build flag is out of scope.
- **No SCSS in user-authored components.** The framework owns the SCSS layer in `.zero/styles/`. The user's `styles/app.scss` continues to be a one-shot, user-owned file that `@use`s the framework aggregate (§7.1 distribution model).
- **Framework-internal palette.** Palette tokens are reachable as CSS custom properties (no way to hide them at runtime), but are documented as framework-internal. User-facing docs and examples never reference palette names directly.
- **Cascade-layer policy unchanged.** Component partials remain wrapped in `@layer components { ... }`. Theme partials and the palette are *unlayered* so they always apply regardless of layer order.
- **`zero update` boundary.** Every file in this spec lives under `.zero/`. No user files outside `.zero/` change as part of `zero update`.
- **No raw hex in component partials after this change.** Every component partial that declared private color tokens consumes semantic tokens instead. Future component additions follow the same rule.

## Out of Scope

- **JS theme toggle / theme manager.** Theming is handled at the CSS layer + `data-theme` attribute. The showcase's app-level signal-based switcher is an example of how users do this themselves, not a framework primitive.
- **Hover / active / focus shade tokens.** `--color-primary-hover`, `--color-primary-active`, etc. are not added in this round. Shipped components do not currently use hover/active distinctions; adding the token surface preemptively is out of scope.
- **Additional shipped themes** (high-contrast, sepia, etc.). Two themes (light, dark) ship. The file layout makes additional themes cheap to add later.
- **`--color-info` semantic token.** No shipped component uses it. Skip.
- **Renaming or removing existing public token names.** `--color-bg` etc. all stay.
- **Per-component color theming primitives** (e.g., `--button-primary-bg` indirection layer). Components consume semantic tokens directly. If a user wants a one-off button color, they override at the call site with class composition or write CSS in `styles/app.scss`.
- **Palette family choice beyond gray/blue/red/green/amber.** No teal/violet/etc. in this round. Users who need additional families can declare them in their own SCSS.
- **Compile-time palette generation** (e.g., generating ramps from a single seed color). The palette is hand-authored hex values.
- **Migration tooling.** No `zero migrate` or codemod to find user overrides of the removed component-private tokens. `zero update` shows the file diffs; users notice via visual regression in their own apps.

## Open Questions

- **Exact palette hex values.** The spec fixes the family and step structure but defers value choice to the planner. The planner should pick from a known-good perceptually-uniform ramp (Open Color, Tailwind, Radix gray etc.) and document the source in a comment at the top of `_palette.scss`.
- **Dark `--color-warning-fg` and `--color-danger-fg` choice.** Amber and red on a dark background can have either light or dark foreground depending on the chosen saturation; the planner should pick whichever gives WCAG AA contrast (4.5:1) against the dark-mode `--color-danger` / `--color-warning` values it chooses. The semantic-mapping table above is intent; final palette steps are the planner's call.
- **`color-scheme` CSS property.** Shoelace emits `color-scheme: light` / `color-scheme: dark` inside its theme blocks so native UI (scrollbars, form controls before custom styling kicks in) matches the theme. The planner should add `color-scheme: light;` to `light.tokens` and `color-scheme: dark;` to `dark.tokens` unless there is a specific reason not to.
- **Naming: `themes` directory under `.zero/styles/`.** Conflict risk with future user-owned `styles/themes/` directories is zero (different roots), but worth confirming the planner doesn't introduce one accidentally.
- **Showcase update.** The showcase currently demos every component. Should it gain a palette inspection page (a grid of all 55 palette swatches with their values)? Worth a one-line answer from the user before the planner commits to scope. Default if no answer: no palette inspection page — the showcase's role is component demos, not design-system documentation.
