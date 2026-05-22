---
title: Theming
nav_order: 9
---

# Theming

zero ships a complete design system in `.zero/styles/`: a color
palette, a public token surface, layout primitives, utility
classes, light/dark themes, and a typography scale. You author
with the tokens; you ship a brand by re-declaring thirteen of
them.

## The design surface

There are three layers, in increasing order of stability:

1. **Framework-internal palette.** Five color families × 11
   steps each: `--gray-{50…950}`, `--blue-{50…950}`,
   `--red-{50…950}`, `--green-{50…950}`, `--amber-{50…950}`.
   Values from [Open Color](https://yeun.github.io/open-color/)
   (MIT). Reserved for framework use — don't consume these in
   app code.

2. **Public semantic tokens.** Thirteen color tokens that *are*
   meant for app code: `--color-bg`, `--color-surface`,
   `--color-text`, `--color-text-muted`, `--color-border`,
   `--color-primary`, `--color-primary-fg`, `--color-success`,
   `--color-success-fg`, `--color-warning`, `--color-warning-fg`,
   `--color-danger`, `--color-danger-fg`.

3. **Non-color invariants.** Spacing, radius, font, size,
   weight, line-height, shadow, border-width tokens. Stay
   constant across themes.

The full token table:

| Category    | Tokens                                                                                                       |
|-------------|--------------------------------------------------------------------------------------------------------------|
| Semantic colors | `--color-bg`, `--color-surface`, `--color-text`, `--color-text-muted`, `--color-border`, `--color-primary`, `--color-primary-fg`, `--color-success`, `--color-success-fg`, `--color-warning`, `--color-warning-fg`, `--color-danger`, `--color-danger-fg` |
| Spacing     | `--space-xs`, `--space-sm`, `--space-md`, `--space-lg`, `--space-xl`                                         |
| Radius      | `--radius-xs`, `--radius-sm`, `--radius-md`, `--radius-lg`, `--radius-xl`, `--radius-2xl`, `--radius-3xl` (pill) |
| Font family | `--font-sans`, `--font-mono`                                                                                 |
| Font size   | `--font-size-sm`, `--font-size-md`, `--font-size-lg`, `--font-size-xl`                                       |
| Font weight | `--weight-normal`, `--weight-medium`, `--weight-bold`                                                        |
| Line height | `--leading-tight`, `--leading-normal`                                                                        |
| Shadow      | `--shadow-sm`, `--shadow-md`, `--shadow-lg`                                                                  |
| Border width| `--border-thin`, `--border-md`, `--border-thick`                                                             |

Use them in SCSS like any CSS custom property:

```scss
.card {
  background: var(--color-surface);
  border: var(--border-thin) solid var(--color-border);
  border-radius: var(--radius-md);
  padding: var(--space-md);
}
```

## Layout primitives

Six classes in `.zero/styles/_layout.scss`. Each is a single CSS
rule and never uses `margin` for spacing — gaps come from
`gap-*` utilities.

| Primitive | Reach for it when…                                                                                                                                                                |
|-----------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `cluster` | **Default for any horizontal layout.** Wraps for free at narrow widths — use it whenever you'd reach for `display: flex` on a row of items (toolbars, button groups, chip lists). |
| `stack`   | Vertical layout where items should *not* spread horizontally — a form body, a card's contents, a sidebar's list of links.                                                         |
| `split`   | Two end-anchored groups separated by stretched whitespace. Canonical case: page header with brand on the left and nav/actions on the right. Anywhere `space-between` would do.    |
| `flank`   | A fixed-size element next to a flexible one. Canonical case: a form row with a label and an input that fills the rest; media objects (avatar + body), icons next to flowing text. |
| `grid`    | A repeating auto-fit column layout — card grids, tile lists, dashboard widgets. Override `--grid-min` to tune the breakpoint. (Not for two-column page layouts — use `flank`.)    |
| `frame`   | Fixed-aspect-ratio media boxes — video embeds, image thumbnails, hero art. Override `--frame-ratio` to change the ratio.                                                          |

Example:

```html
<header class="split pad-md">
  <a href="/" class="text-h4">brand</a>
  <nav class="cluster gap-md">
    <a href="/docs">Docs</a>
    <a href="/blog">Blog</a>
  </nav>
</header>
```

## Utility families

Forty-four utility classes across two partials. Compose them on
elements to handle the long tail of layout without hand-written
CSS.

| Family            | Classes                                                                                              |
|-------------------|------------------------------------------------------------------------------------------------------|
| `gap-*`           | `gap-0`, `gap-xs`, `gap-sm`, `gap-md`, `gap-lg`, `gap-xl`                                            |
| `pad-*`           | `pad-0`, `pad-xs`, `pad-sm`, `pad-md`, `pad-lg`, `pad-xl`                                            |
| `border` / sides  | `border`, `border-t`, `border-r`, `border-b`, `border-l`                                             |
| Alignment         | `align-{start,center,end,stretch,baseline}`                                                          |
| Justify           | `justify-{start,center,end,between,around,evenly}`                                                   |
| Align-self        | `align-self-{start,center,end,stretch,baseline}`                                                     |
| Justify-self      | `justify-self-{start,center,end,stretch}`                                                            |
| Text alignment    | `text-{start,center,end}` (logical, RTL-safe)                                                        |
| Flex direction    | `flex-{row,row-reverse,col,col-reverse}`                                                             |

The `0` step on `gap` and `pad` lets a layout primitive cancel
its default spacing without raw CSS:
`class="cluster gap-0"`.

No `!important`. Overriding happens via class-list order: the
alignment partial is `@use`d after the utilities partial in the
aggregate, so alignment rules win where they touch the same
property.

## Light and dark

Theme variants override only the thirteen semantic `--color-*`
tokens; everything else stays the same.

The selector strategy:

1. **Light is the default.** `:root` carries the light token
   values.
2. **`@media (prefers-color-scheme: dark)`** overrides them with
   the dark theme.
3. **`[data-theme="light"]` / `[data-theme="dark"]`** override
   both — the explicit attribute always wins.

Set `<html data-theme="dark">` to force dark regardless of OS
preference. Each theme mixin also emits the CSS
`color-scheme` property, so native UI (scrollbars, default
form controls) follows the theme.

There is no JavaScript helper for toggling. If you want a
runtime toggle, write five lines that set the attribute on
`<html>`.

## Authoring a brand theme

A brand theme is the thirteen public `--color-*` tokens
redefined under a selector. Add a file under your project's
`styles/`, then `@use` it from `styles/app.scss`:

```scss
// styles/_brand.scss
[data-theme="brand"] {
  color-scheme: light;
  --color-bg:           #fffdf6;
  --color-surface:      #fff;
  --color-text:         #1a1a1a;
  --color-text-muted:   #555;
  --color-border:       #e6e0d0;
  --color-primary:      #6d2bff;
  --color-primary-fg:   #fff;
  --color-success:      #1f9d55;
  --color-success-fg:   #fff;
  --color-warning:      #b9650a;
  --color-warning-fg:   #fff;
  --color-danger:       #b91c1c;
  --color-danger-fg:    #fff;
}
```

```scss
// styles/app.scss
@use '../.zero/styles/zero';
@use 'brand';
```

Then activate it: `<html data-theme="brand">`. All shipped
components — and any of your own CSS that consumes `--color-*`
tokens — pick up the new palette without further changes.

## Typography

Twelve utility classes in `.zero/styles/_typography.scss`:

```
.text-display
.text-h1
.text-h2
.text-h3
.text-h4
.text-eyebrow
.text-body
.text-small
.text-muted
.text-code
.text-link
.divider
```

Pick a semantic tag (`<h1>` for the page outline) **and** a
visual class (`class="text-display"` for hero size). The two
don't have to match — a `<h2>` with `class="text-h4"` is the
right answer when the visual hierarchy doesn't follow the
document outline.

Fonts: **Geist** (sans) and **Geist Mono** ship locally as four
variable-axis `.woff2` files under `.zero/fonts/`. `_base.scss`
declares the `@font-face` blocks against `/.zero/fonts/...`
URLs. No network round-trip to Google Fonts. The SIL Open Font
License text rides alongside as `.zero/fonts/OFL.txt`.

## Override an individual token

You don't have to author a whole theme to shift one value.
Re-declare the token in `styles/app.scss` after the framework
`@use` line:

```scss
@use '../.zero/styles/zero';

:root {
  --radius-md: 8px;                   /* tighter rounding */
  --space-md:  1.25rem;               /* slightly more breathing room */
}
```

Token overrides are a normal CSS cascade — the last declaration
wins. Component partials (in `@layer components`) and
unlayered styles in `styles/app.scss` interact predictably.

For longer rationale and worked examples on building a brand,
see [Best Practices §8 Styles](./best-practices.html#8-styles).
