# Spec: Drawer component

## Problem Statement

The component library now ships eighteen components — modal `Dialog`,
inline `Toast`, the form inputs, `Pagination`, `Combobox`, `Table` — but
no off-canvas / side-panel primitive. Real zero apps that need an
in-context editor (edit-this-user, add-a-product, view-row-details,
filters panel, notifications tray) currently hand-roll a positioned
`<aside>` plus open-state coordination plus enter/exit animation plus
the layout reflow when the panel pushes page content. Each reinvention
re-litigates the same flex/transform decisions and drifts off the
design-token palette.

This issue adds `Drawer` to the shipped component library. Scope is
deliberately tight: a controlled, edge-anchored panel that slides in
from one of the four sides, in either of two layout modes — `overlay`
(floats over content with a non-interactive backdrop) or `push` (lives
in flow as a flex sibling and the parent layout reflows around it).
The drawer is intended to be **mounted once per side at the app root
and used as a singleton context surface** — the panel stays put while
the caller swaps its content reactively (edit form ↔ add form ↔ row
details). Drawer itself is a pure visual container: three caller-owned
slots (`title`, `body`, `controls`), no built-in close affordances, no
focus trap, no scroll lock. Close is fully programmatic — the parent
owns the `open` signal and drives it from whatever context state is
active.

## Background

### What exists today

- **Component library (eighteen components after Combobox + Pagination
  + Table + form inputs).** Components ship under `.zero/components/`,
  re-exported from `.zero/components/index.ts`, importable via the bare
  specifier `"zero/components"`. Each component:
  - Is a plain function: `Component<P> = (props?: P) => TemplateResult`.
  - Reads stateful props as signals; never holds internal state for
    `open`/`value`/etc.
  - Has a per-component SCSS partial under
    `.zero/styles/components/_{name}.scss`, every rule inside
    `@layer components { ... }`.
  - Has a `*.test.ts` neighbour that ships in the manifest and runs
    with `zero test`.
- **Scaffold / manifest plumbing.** `framework_manifest()` in
  `crates/zero-scaffold/src/lib.rs` lists every framework-owned file
  under `.zero/`. Each entry is a `TPL_*` constant pointing at
  `include_str!("scaffold/.zero/...")`. The `_components.scss`
  aggregate `@use`s every per-component partial in alphabetical order;
  `zero.scss` `@use`s `'components'` last.
- **Module resolution.** Adding a new component is one line in
  `.zero/components/index.ts` plus the file itself — no resolver
  change. The editor-side declaration in `.zero/components.d.ts` also
  gets a new block (kept alphabetical inside the `declare module`).
- **Layout primitives.** The design system ships `cluster`
  (horizontal flex), `stack` (vertical flex), `grow`, `pad-*`,
  `gap-*`. Real zero apps already structure their root layouts with
  these utilities, so push-mode "Drawer is a flex sibling of `<main>`"
  is a natural fit, not a new convention.
- **Design tokens.** Spacing `--space-{xs,sm,md,lg,xl}`; colors
  including `--color-{bg,surface,text,text-muted,border,overlay}`;
  radii, shadows, border widths; durations `--duration-{fast,normal,
  slow}`; easings `--ease-out`, `--ease-in-out`; existing `z-index`
  constant `1000` used by `Dialog`'s backdrop.
- **Closest existing component: `Dialog`.** Dialog is a controlled
  overlay with `open: Signal<boolean>`, a backdrop + panel, sized
  via `sm`/`md`/`lg`, mounted-when-open. Drawer borrows Dialog's
  controlled-by-signal contract, size-variant prop name, backdrop
  tokens, and animation tokens — but differs in three load-bearing
  ways:
  - Drawer anchors to an edge; Dialog centers.
  - Drawer offers a `push` mode (in-flow flex sibling); Dialog only
    overlays.
  - Drawer is always mounted (so it can animate both directions);
    Dialog renders nothing when closed.
- **Showcase project.** `showcase/` is a full zero project at the
  repo root; every component has a route under `showcase/src/routes/`
  and a link in `showcase/src/routes/home.ts`. Built/dev tested in
  CI via `tests/showcase_build.rs` and `tests/showcase_dev.rs`;
  component tests run via `tests/component_library.rs`.

### Decisions made during refine

The user confirmed each of the following:

- **Side names: `"left" | "right" | "top" | "bottom"`.** Web-
  conventional and matches CSS logical-property fallbacks. Not the
  compass shorthand from the roadmap line; not RTL-aware `start`/`end`
  (deliberate — `right` always means visual right, mirroring the
  framework's overall RTL stance which is "no special handling in
  v1").
- **Single component, `mode` prop.** Not two separate components,
  not deferred. `Drawer({ mode: "overlay" | "push" })` toggles
  between out-of-flow and in-flow rendering with the same panel
  surface, same children slots, same animation tokens.
- **Push mode = flex sibling, layout-native reflow.** The user
  rejected an earlier proposal that toggled a body class and CSS-
  selected an opt-in `.drawer-pushable` container ("very old school,
  not the modern layout system we have for zero"). The actual
  mechanism is: Drawer renders an `<aside>` element. In push mode,
  it sits in normal flow as a flex sibling of the page content.
  When `open` flips true, its `inline-size` (for `left`/`right`) or
  `block-size` (for `top`/`bottom`) animates from `0` to the
  size-variant value, and the flex parent reflows naturally. The
  user marks nothing; the layout primitives they already use
  (`cluster` / `stack`) do the work.

  ```ts
  // Idiomatic root layout for a right-side push Drawer:
  html`
    <div class="cluster">                            <!-- horizontal flex -->
      <main class="grow stack pad-xl">${routerOutlet()}</main>
      ${Drawer({ open, side: "right", mode: "push", body: ... })}
    </div>
  `;
  ```

  Drawer dropped outside a flex/grid parent in push mode simply
  doesn't push anything — it sits in normal flow. Not enforced at
  runtime; documented as a usage requirement.

- **Overlay mode = `position: fixed`.** Same component, same children,
  same `side`. The panel pins to its edge of the viewport and floats
  over content; flex position is ignored. A non-interactive backdrop
  renders behind the panel using `--color-overlay` and the same
  `backdrop-filter: blur(4px)` Dialog uses.
- **Backdrop is visual-only (overlay mode).** Renders in overlay mode
  to dim and intercept clicks against underlying content, but
  clicking it does **not** close the drawer. Caller fully owns close.
  **No backdrop in push mode at all** — the drawer is in-flow, and
  the underlying page content must stay fully interactive. This is a
  load-bearing property, not just an absence: it enables the
  "inspect-many" interaction where, e.g., a table of records is
  visible and clicking a row opens the drawer with that record's
  detail; the user can then click another row and the drawer stays
  open while its body swaps to the new record (via the singleton +
  context-replacement pattern below). Without this, push mode would
  collapse into overlay mode minus the dimming, and the row-pick
  flow would require closing the drawer between picks.
- **Programmatic-only close.** Drawer never writes to `open`. There
  is no built-in Escape handler, no backdrop-click handler, no
  built-in close button. The parent component drives `open` from
  whatever context state is active (typically a `computed` over
  context signals — see the singleton pattern below) and toggles it
  from its own form-cancel / form-submit / outside-click handlers.
  No `onClose?` callback either; the caller already knows when it
  set the signal to false.
- **Always-mounted DOM; CSS animates both directions.** Drawer's
  `<aside>` is rendered on every pass regardless of `open`. When
  `open` is false, the panel is positioned off-screen (overlay
  mode) or collapsed to zero size (push mode), and a `transition`
  on the transform / size property animates the change in either
  direction. User sees a slide-in and a slide-out, not a snap.
  Caller can still tear down child content via reactive
  substitutions (`${() => editingUser.val ? Form(...) : null}`) so
  expensive children unmount when their context is gone.
- **Three named slots: `title`, `body`, `controls`.** All three are
  caller-controlled. All three accept a `TemplateResult`, a `string`,
  or `null`/omission. The body region is the scrollable one (long
  forms scroll; the title and controls stay pinned to top/bottom of
  the panel). Caller uses these slots to render their own header
  copy, action buttons, close-X button, whatever — Drawer ships no
  built-in chrome inside them.
- **Size variants: `sm | md | lg`, default `md`.** Mirrors Dialog.
  Each variant maps to a per-axis value: left/right drawers get an
  `inline-size`; top/bottom drawers get a `block-size`. Exact token
  values are for the plan; spec proposes
  - left/right: `sm = 16rem`, `md = 24rem`, `lg = 32rem`.
  - top/bottom: `sm = 12rem`, `md = 18rem`, `lg = 24rem`.
- **Singleton-with-reactive-children is the intended usage pattern,
  not a built-in feature.** The recommended app shape is: one Drawer
  per side, mounted in the root layout. Its `open` is a `computed`
  over context signals; its slot contents are reactive substitutions
  that switch on which context is active. Drawer-the-component
  doesn't know about this pattern — it just takes `open` and three
  slot props. The showcase demonstrates the pattern; docs describe
  it; the framework ships no helper.

### The singleton + context-replacement pattern (illustrative)

Two canonical shapes the user has called out:

**Shape A — context-driven editor / form drawer.** Several "actions"
can open the drawer (edit-this-user, add-a-product). Each owns a
context signal; a `computed` over them drives `open`; reactive slot
substitutions switch the contents.

```ts
import { html, signal, computed } from "zero";
import { Drawer, Button } from "zero/components";

const editingUser = signal<User | null>(null);
const addingProduct = signal(false);

const drawerOpen = computed(
  () => editingUser.val !== null || addingProduct.val,
);

return html`
  <div class="cluster">
    <main class="grow stack pad-xl">${routerOutlet()}</main>
    ${Drawer({
      open: drawerOpen,
      side: "right",
      mode: "push",
      title: () =>
        editingUser.val ? html`<h2 class="text-h2">Edit user</h2>`
        : addingProduct.val ? html`<h2 class="text-h2">Add product</h2>`
        : null,
      body: () =>
        editingUser.val ? EditUserForm({ user: editingUser.val })
        : addingProduct.val ? AddProductForm()
        : null,
      controls: () => html`
        ${Button({ variant: "ghost", children: "Cancel",
                  onClick: () => { editingUser.set(null); addingProduct.set(false); } })}
      `,
    })}
  </div>
`;
```

**Shape B — inspector / detail drawer over a table.** A list-of-rows
is permanently visible; clicking a row opens the drawer with that
row's detail; clicking *another* row swaps the drawer's body to the
new record without closing first. This is the use case that makes
push-mode-without-backdrop load-bearing: the rows underneath must
stay clickable while the drawer is open.

```ts
const selectedRow = signal<Row | null>(null);
const drawerOpen = computed(() => selectedRow.val !== null);

return html`
  <div class="cluster">
    <section class="grow stack pad-xl">
      ${Table({
        rows: rowsSignal,
        columns,
        rowKey: r => r.id,
        onRowClick: r => selectedRow.set(r),   // ← always sets; same row re-picks fine
      })}
    </section>
    ${Drawer({
      open: drawerOpen,
      side: "right",
      mode: "push",
      title: () => selectedRow.val
        ? html`<h2 class="text-h2">${selectedRow.val.name}</h2>`
        : null,
      body: () => selectedRow.val ? RowDetail({ row: selectedRow.val }) : null,
      controls: () => html`
        ${Button({ variant: "ghost", children: "Close",
                  onClick: () => selectedRow.set(null) })}
      `,
    })}
  </div>
`;
```

Notes shared by both shapes:
- The `() => ...` substitutions in the slot props are signal-reactive
  via the existing template machinery; they re-evaluate when their
  dependencies change. No Drawer-side magic.
- Caller-driven close: setting the context signal back to its empty
  state flips the `computed`, which flips `drawerOpen` to false,
  which animates the drawer out.
- In Shape B, clicking a different row while the drawer is open does
  **not** close-then-reopen — the `open` computed stays `true`, so
  Drawer never animates out; only the body slot re-renders.

### Component contract

Same conventions as the existing roster:

- The single stateful prop is a signal: `open: Signal<boolean>`.
- Everything else is plain configuration — read once on mount, not
  reactive. (No signal-or-plain `disabled` here; Drawer has no
  disabled state in v1.)
- Slot props (`title`, `body`, `controls`) accept the same template-
  literal value type any other component's `children` accepts. They
  can be functions that return a `TemplateResult` — the existing
  template machinery already handles reactive substitution there.
- No internal signals beyond what's needed for rendering; the
  component itself is essentially a styled wrapper.
- No event callbacks. No `onClose`, no `onOpen`. The parent owns
  `open` and the context signals driving it.

### Props sketch

```ts
type DrawerSide = "left" | "right" | "top" | "bottom";
type DrawerMode = "overlay" | "push";
type DrawerSize = "sm" | "md" | "lg";

type DrawerSlot = TemplateResult | string | null | undefined;

type DrawerProps = {
  // Stateful (parent owns)
  open: Signal<boolean>;

  // Anchor edge — required.
  side: DrawerSide;

  // Layout behaviour — defaults to overlay.
  mode?: DrawerMode;

  // Size variant — defaults to md.
  size?: DrawerSize;

  // Three caller-owned slots; all optional, all nullable.
  title?: DrawerSlot;
  body?: DrawerSlot;
  controls?: DrawerSlot;
};
```

### DOM shape

Overlay mode, `open` is true:

```
<div class="drawer-backdrop drawer-backdrop-open"></div>
<aside class="drawer drawer-overlay drawer-{side} drawer-{size} drawer-open"
       role="dialog"
       aria-modal="true">
  <header class="drawer-title"
          hidden=...><!-- hidden when title slot is null --></header>
  <div    class="drawer-body"><!-- scrollable --></div>
  <footer class="drawer-controls"
          hidden=...><!-- hidden when controls slot is null --></footer>
</aside>
```

Push mode (no backdrop), `open` is true:

```
<aside class="drawer drawer-push drawer-{side} drawer-{size} drawer-open"
       role="complementary">
  <header class="drawer-title" hidden=...></header>
  <div    class="drawer-body"></div>
  <footer class="drawer-controls" hidden=...></footer>
</aside>
```

When `open` is false, the same DOM renders but without `drawer-open`
and without the `drawer-backdrop-open` class on the backdrop — CSS
positions / sizes the panel out-of-view and the backdrop becomes
`opacity: 0; pointer-events: none`. The DOM nodes stay mounted.

### CSS sketch (illustrative; plan owns the final values)

```scss
@layer components {
  // Overlay backdrop — only present in overlay mode.
  .drawer-backdrop {
    position: fixed;
    inset: 0;
    background: var(--color-overlay);
    backdrop-filter: blur(4px);
    z-index: 999;                       // just under the panel
    opacity: 0;
    pointer-events: none;
    transition: opacity var(--duration-normal) var(--ease-out);
  }
  .drawer-backdrop-open {
    opacity: 1;
    pointer-events: auto;
  }

  // Panel surface (shared by overlay and push).
  .drawer {
    background: var(--color-bg);
    color: var(--color-text);
    border: var(--border-thin) solid var(--color-border);
    box-shadow: var(--shadow-lg);
    display: flex;
    flex-direction: column;
    overflow: hidden;                   // contains children during collapse
    transition:
      inline-size var(--duration-normal) var(--ease-out),
      block-size  var(--duration-normal) var(--ease-out),
      transform   var(--duration-normal) var(--ease-out);
  }

  // Overlay mode — fixed positioning, slides via transform.
  .drawer-overlay {
    position: fixed;
    z-index: 1000;
  }
  .drawer-overlay.drawer-left   { inset-block: 0; inset-inline-start: 0; transform: translateX(-100%); }
  .drawer-overlay.drawer-right  { inset-block: 0; inset-inline-end:   0; transform: translateX(100%); }
  .drawer-overlay.drawer-top    { inset-inline: 0; inset-block-start: 0; transform: translateY(-100%); }
  .drawer-overlay.drawer-bottom { inset-inline: 0; inset-block-end:   0; transform: translateY(100%); }
  .drawer-overlay.drawer-open   { transform: none; }

  // Push mode — in-flow flex sibling. Animates size to zero when closed.
  .drawer-push.drawer-left,
  .drawer-push.drawer-right  { inline-size: 0; }
  .drawer-push.drawer-top,
  .drawer-push.drawer-bottom { block-size: 0; }

  // Size variants resolve only when open.
  .drawer-push.drawer-open.drawer-left.drawer-sm,
  .drawer-push.drawer-open.drawer-right.drawer-sm  { inline-size: 16rem; }
  .drawer-push.drawer-open.drawer-left.drawer-md,
  .drawer-push.drawer-open.drawer-right.drawer-md  { inline-size: 24rem; }
  .drawer-push.drawer-open.drawer-left.drawer-lg,
  .drawer-push.drawer-open.drawer-right.drawer-lg  { inline-size: 32rem; }
  .drawer-push.drawer-open.drawer-top.drawer-sm,
  .drawer-push.drawer-open.drawer-bottom.drawer-sm { block-size: 12rem; }
  .drawer-push.drawer-open.drawer-top.drawer-md,
  .drawer-push.drawer-open.drawer-bottom.drawer-md { block-size: 18rem; }
  .drawer-push.drawer-open.drawer-top.drawer-lg,
  .drawer-push.drawer-open.drawer-bottom.drawer-lg { block-size: 24rem; }

  // Overlay mode size variants set the size at all times (transform handles open/close).
  .drawer-overlay.drawer-left.drawer-sm,
  .drawer-overlay.drawer-right.drawer-sm  { inline-size: 16rem; block-size: 100vh; }
  // ...etc, plan enumerates.

  // Section layout inside the panel.
  .drawer-title    { flex: 0 0 auto; padding: var(--space-md) var(--space-lg);
                     border-block-end: var(--border-thin) solid var(--color-border); }
  .drawer-body     { flex: 1 1 auto; min-block-size: 0;
                     overflow: auto; padding: var(--space-lg); }
  .drawer-controls { flex: 0 0 auto; padding: var(--space-md) var(--space-lg);
                     border-block-start: var(--border-thin) solid var(--color-border);
                     display: flex; gap: var(--space-sm); justify-content: flex-end; }
}
```

Final selector shapes and the size token values are for the plan.
The above is illustrative.

## Requirements

### Component

1. New file `.zero/components/Drawer.ts` exports a single default
   `Drawer` function matching `Component<DrawerProps>`. The component
   runs once per mount; visibility / sizing reactivity comes from the
   reactive class-bindings on the `<aside>` and `.drawer-backdrop`.

2. The component exports the `DrawerSide`, `DrawerMode`, `DrawerSize`,
   and `DrawerProps` types.

3. The `open` prop is a `Signal<boolean>`. The component reads
   `open.val` reactively in the class bindings on the backdrop and
   the `<aside>` (toggling `drawer-backdrop-open` and `drawer-open`).
   It never writes to `open`.

4. `side` is required, takes the four string-literal values, and
   appears as both a class on the `<aside>` (`drawer-{side}`) and as
   a determinant for which size-axis the CSS applies. No default; a
   missing value is a TypeScript error.

5. `mode` defaults to `"overlay"`. When `"overlay"`, the component
   renders the `.drawer-backdrop` sibling and adds `drawer-overlay`
   to the `<aside>`; the `<aside>` gets `role="dialog"
   aria-modal="true"`. When `"push"`, no backdrop renders, the
   `<aside>` gets `drawer-push` and `role="complementary"` (the
   implicit role of `<aside>` is fine; an explicit `role` attribute
   matches the convention from `Dialog`'s explicit `role="dialog"`).

6. `size` defaults to `"md"` and appears as a class on the `<aside>`
   (`drawer-{size}`). The CSS owns the per-axis size resolution.

7. `title`, `body`, and `controls` slots each render inside their
   respective `<header>` / `<div>` / `<footer>` wrapper. Each
   wrapper carries a reactive `hidden` attribute keyed to whether
   the slot value is non-empty:
   - `null`, `undefined`, and empty-string slots → wrapper has the
     native `hidden` attribute and renders no content.
   - Any other value (string, `TemplateResult`, function returning
     either) → wrapper is visible and the slot renders inside.

   The `hidden`-attribute approach (rather than rendering nothing)
   keeps the flex layout slot-count stable across re-renders and
   keeps a CSS-only collapsing transition possible later. The
   binding may be a `() => ...` function so reactive slots
   (`${() => editingUser.val ? ... : null}`) toggle the wrapper's
   `hidden` correctly as the context flips.

8. The component renders **the same DOM regardless of `open`**.
   The `drawer-open` class and the `drawer-backdrop-open` class are
   the only things that flip with `open.val`. This is the always-
   mounted contract — the slide animation runs in both directions
   because the panel is always present.

9. No `effect()`. No document-level listeners. No `addEventListener`
   anywhere. No timers. No refs. The component is a pure render
   function whose only reactive surface is the class bindings.

10. The component imports nothing beyond `"zero"` itself (for
    `html` and the types). It does not import from any other
    component file and does not depend on the `_internal.ts`
    helpers.

11. No callbacks. `DrawerProps` exposes no `onClose`, `onOpen`,
    `onAnimationEnd`, etc. The parent already knows the state of
    its own `open` signal.

### CSS

12. New partial `.zero/styles/components/_drawer.scss`. Every rule
    sits inside `@layer components { ... }`.

13. Token-only values. Spacing from `--space-*`, colors from
    `--color-*`, radii from `--radius-*`, borders from `--border-*`,
    shadows from `--shadow-*`, durations from `--duration-*`, easings
    from `--ease-*`. Standard exceptions for `opacity`, `z-index`
    (Drawer's panel sits at `1000`, the backdrop at `999`; mirrors
    Dialog), and `backdrop-filter`'s `blur(4px)` literal (same as
    Dialog).

14. The four `drawer-{side}` classes set the appropriate
    `inset-*` properties for overlay mode using **logical
    properties only** (`inset-inline-start`, `inset-inline-end`,
    `inset-block-start`, `inset-block-end`). No `left`/`right`/
    `top`/`bottom` in rules that affect direction.

15. The three `drawer-{size}` classes resolve a single dimension
    per side-axis:
    - left/right: `inline-size`. Values: sm `16rem`, md `24rem`,
      lg `32rem`. Plan confirms and can adjust per design judgement.
    - top/bottom: `block-size`. Values: sm `12rem`, md `18rem`,
      lg `24rem`. Plan confirms.
    Overlay mode pairs these with the cross-axis filling the
    viewport (`block-size: 100vh` for left/right, `inline-size:
    100vw` for top/bottom). Push mode lets the cross-axis come
    from the flex parent.

16. Push-mode size collapse-to-zero behaviour:
    - When `.drawer-push` is present without `.drawer-open`, the
      relevant axis size is `0` and `overflow: hidden` clips any
      child content. The `transition` animates the size back to
      the size-variant value when `.drawer-open` is added.

17. Overlay-mode hidden-by-default behaviour:
    - When `.drawer-overlay` is present without `.drawer-open`,
      the panel is translated off-screen via `transform` along
      its side's axis (left/right: `translateX(±100%)`; top/
      bottom: `translateY(±100%)`). The `transition` animates the
      transform back to `none` when `.drawer-open` is added.

18. `.drawer-backdrop` is `position: fixed; inset: 0; z-index: 999`,
    background `var(--color-overlay)`, `backdrop-filter: blur(4px)`.
    Default state is `opacity: 0; pointer-events: none`; adding
    `.drawer-backdrop-open` flips to `opacity: 1; pointer-events:
    auto`. Transition on `opacity` only. No click handler — the
    `pointer-events: auto` only matters so the backdrop intercepts
    clicks against underlying content while open.

19. `.drawer-title`, `.drawer-body`, `.drawer-controls` lay out as
    a vertical flex inside the panel:
    - title: `flex: 0 0 auto`, top padding/border per design token
      sizing, sits at top.
    - body: `flex: 1 1 auto; min-block-size: 0; overflow: auto`.
      The `min-block-size: 0` is load-bearing — without it,
      `overflow: auto` does not engage inside a flex parent.
    - controls: `flex: 0 0 auto`, bottom padding/border, default
      `display: flex; gap: var(--space-sm); justify-content:
      flex-end` so caller-passed buttons line up right-edge by
      default. Caller can override by passing their own flex
      wrapper inside `controls` if they want left-alignment or
      space-between.
    Section padding values come from `--space-*`. The
    title/controls borders use `--color-border` and the existing
    `--border-thin` token.

20. The native `hidden` attribute on the title/controls wrappers
    is honoured by the existing browser styles (`display: none`).
    No additional CSS is needed to handle the hidden state.

21. No `!important`. No magic numbers outside the listed exceptions.
    Logical properties throughout.

### Scaffold registration

22. New `TPL_*` constants for:
    - `.zero/components/Drawer.ts`
    - `.zero/components/Drawer.test.ts`
    - `.zero/styles/components/_drawer.scss`

    Inserted in alphabetical position (after `DIALOG`, before
    `INPUT`).

23. `framework_manifest()` gains three entries, alphabetically
    positioned (`.zero/components/Drawer.ts` and `.test.ts` after
    the `Dialog.*` entries; `_drawer.scss` after `_dialog.scss`).
    The existing length-coupled assertions in
    `crates/zero-scaffold/src/lib.rs` tests and any `tests/update*.rs`
    paths that hard-code the manifest length are bumped accordingly.
    The plan enumerates the exact assertions.

24. `.zero/components/index.ts` gains two lines:
    - `export { default as Drawer } from "./Drawer.ts";`
    - `export type { DrawerProps, DrawerMode, DrawerSide, DrawerSize } from "./Drawer.ts";`
    Inserted alphabetically between the `Dialog` and `Input` blocks.

25. `.zero/styles/_components.scss` gains one line:
    `@use 'components/drawer';` inserted alphabetically between
    `dialog` and `input`.

26. `.zero/components.d.ts` (the module declaration for
    `"zero/components"`) gains a `Drawer` block in alphabetical
    position with the type aliases (`DrawerSide`, `DrawerMode`,
    `DrawerSize`) and the function signature. Exact shape follows
    the existing convention; the plan picks.

27. The editor `tsconfig.json` emitted by `zero init` requires no
    changes — `"zero/components"` already resolves to
    `.zero/components/index.ts`, and `Drawer` rides on that
    resolution.

### Tests

28. New file `.zero/components/Drawer.test.ts` exercising:
    - **Renders the base markup (overlay, closed).** With
      `open: signal(false)`, side `"right"`, default mode/size, the
      component renders an `<aside class="drawer drawer-overlay
      drawer-right drawer-md">` without `drawer-open`, plus a
      `.drawer-backdrop` sibling without `drawer-backdrop-open`.
    - **`open` toggles the open classes.** Setting `open.set(true)`
      adds `drawer-open` to the `<aside>` and `drawer-backdrop-open`
      to the backdrop. Setting back to `false` removes them. DOM
      nodes remain mounted across the toggle.
    - **Mode `"push"` skips the backdrop.** With `mode: "push"`,
      no `.drawer-backdrop` element renders. The `<aside>` has
      `drawer-push` (not `drawer-overlay`) and `role="complementary"`.
    - **Mode `"overlay"` includes the backdrop and sets ARIA.**
      Default mode renders the backdrop and the `<aside>` has
      `role="dialog" aria-modal="true"`.
    - **Side class.** Each of `"left"`, `"right"`, `"top"`,
      `"bottom"` produces the matching `drawer-{side}` class.
    - **Size class.** Default is `drawer-md`. Each of `"sm"`, `"md"`,
      `"lg"` produces the matching `drawer-{size}` class.
    - **Slot rendering — strings.** `title: "Edit user"`, `body:
      "Form goes here"`, `controls: "Buttons"`. Each appears in
      its respective wrapper. None of the wrappers carry `hidden`.
    - **Slot rendering — TemplateResults.** Same with `html`
      templates inside each slot.
    - **Slot rendering — null / undefined / empty string hides
      wrapper.** With `title: null`, the `.drawer-title` element
      has the `hidden` attribute. Same for omitted and empty-string
      slots. The `.drawer-body` and `.drawer-controls` slots
      independently obey the same rule.
    - **Slot rendering — reactive function.** A slot passed as
      `() => state.val ? "A" : "B"` updates in place when `state`
      changes, without remounting the panel.
    - **No close on backdrop click.** Click the `.drawer-backdrop`;
      `open.val` remains `true`.
    - **No close on Escape.** Dispatch a `keydown` Escape event on
      `document`; `open.val` remains `true`.
    - **Component does not import side-effecting modules.** A
      sanity check that mounting/unmounting many Drawers does not
      accumulate document listeners (no `addEventListener` calls
      observed via a spy on `document.addEventListener` inside the
      test; equivalent assertion fine if the spy infrastructure
      doesn't exist — plan picks).
    - `afterEach(cleanup)`.

29. The new test ships in the manifest (lives in
    `.zero/components/`) and runs:
    - Inside the framework's `showcase/` via
      `tests/component_library.rs`.
    - Inside every user project's `zero test` (consistent with the
      existing component-test ship-along policy).

### Showcase

30. New route file `showcase/src/routes/drawer.ts` rendering each
    combination the user is likely to want to see:
    - **Right-side overlay, default size.** A toggle button opens
      a Drawer with a title ("Edit user"), a body (a small form
      with `Input` for name and `Select` for role), and controls
      (a "Save" `Button.primary` and a "Cancel" `Button.ghost`,
      both of which set `open.set(false)`).
    - **Right-side push, default size.** Mounted as a flex sibling
      of a `<main>` block holding several paragraphs. A toggle
      button opens it; the user sees the page content reflow to
      make room.
    - **Left-side overlay.** Same shape as right-overlay; verifies
      slide direction.
    - **Top-side push, large.** Verifies the vertical-axis behaviour
      inside a `stack` parent.
    - **Bottom-side overlay, small.** Verifies the small size and
      the bottom slide direction.
    - **Singleton + context-replacement demo (Shape A — forms).**
      A row of three buttons ("Edit user A", "Edit user B", "Add
      user") drives three context signals; a `computed` derives
      `drawerOpen`; reactive substitutions in
      `title`/`body`/`controls` swap the content based on which
      context is active. Demonstrates Shape A from the spec's
      Background section. Includes a short prose explainer.
    - **Inspector demo (Shape B — table-row pick, push mode).**
      A `Table` of ~8 records sits next to a push-mode Drawer in
      a `cluster` layout. The table's `onRowClick` sets a
      `selectedRow` signal; `drawerOpen` is `computed(() =>
      selectedRow.val !== null)`. Clicking different rows in
      succession demonstrates the load-bearing property:
      underlying content stays interactive, the drawer stays
      open, and only the body re-renders. A "Close" button in
      `controls` sets `selectedRow.set(null)`. Short prose
      explainer notes why this requires push mode specifically
      (overlay's backdrop would intercept the row clicks).
    - Slot variations: at least one Drawer with `title: null`
      (so the showcase visibly proves the title region collapses),
      one with `controls: null`.
    Each instance shows a brief explanation paragraph above the
    toggle.

31. `showcase/src/app.ts` registers
    `app.route("/drawer", () => import("./routes/drawer"))`.
    Position consistent with existing alphabetical ordering of
    routes (between `/dialog` and `/input`).

32. `showcase/src/routes/home.ts` navigation cluster gains a
    `Drawer` link, alphabetically positioned.

33. The showcase's committed `.zero/components/Drawer.ts` (and
    partial + test) matches the manifest. `zero update --yes`
    from inside `showcase/` produces zero drift.

### Integration tests

34. `tests/showcase_build.rs` continues to pass against the new
    route. The plan verifies whether any per-route assertion
    exists that needs widening.

35. `tests/showcase_dev.rs` continues to pass.

36. `tests/component_library.rs` continues to pass and now
    includes `Drawer.test.ts` in its run. The plan verifies
    whether the test asserts a specific test count that needs
    bumping.

### Documentation

37. `crates/zero-scaffold/src/scaffold/AGENTS.md` `## Components`
    section gains a `Drawer` entry in the component-roster table.
    The relevant category subsection (likely "Overlays" alongside
    `Dialog` and `Toast`) gains a one-instance usage example.

38. `docs/components.md` is updated:
    - The component-count language ("eighteen components") is
      bumped to "nineteen components" (or correct count after any
      other in-flight additions — plan confirms).
    - The summary table gains a new row for `Drawer` with its
      required props and an example, in alphabetical position
      (after `Dialog`, before `Input`).
    - A short prose paragraph near the overlays section describes
      the singleton + context-replacement pattern as the intended
      shape, with the example from this spec's Background section.
      Notes the push-mode requirement that the parent layout must
      be a flex/grid container along the right axis (no runtime
      enforcement; document only).

39. `zero-framework-spec.md` — the `"zero/components"` listing
    gains a `Drawer({...})` line in the Overlays group (or
    wherever the Dialog/Toast group is named). Phase-component-
    count and any "no off-canvas primitive" out-of-scope text is
    updated.

## Constraints

- **No new Rust dependencies.** Rides on the existing `grass` SCSS
  pipeline, the existing transpiler, the existing scaffold + manifest
  plumbing.
- **No new npm dependencies.** Framework-wide.
- **No new top-level `"zero"` runtime exports.** Drawer is exposed
  only via `"zero/components"`.
- **`@layer components` for all CSS rules.** Unlayered user CSS in
  `styles/app.scss` overrides without `!important`.
- **Tokens only — no magic numbers or hex codes.** Standard
  exceptions for `opacity`, `z-index` (999 backdrop, 1000 panel —
  mirrors Dialog), `backdrop-filter: blur(4px)` (mirrors Dialog),
  and the size-variant lengths (which are the *single source of
  truth* for drawer dimensions and live in the partial).
- **Logical properties only** for direction-affecting rules
  (`inset-inline-*`, `inset-block-*`, `inline-size`, `block-size`,
  `padding-block`, `padding-inline`, etc.). No `left`/`right`/`top`/
  `bottom` in those rules.
- **One stateful prop: `open: Signal<boolean>`.** No signal-or-plain
  configuration props in v1. No reactive `disabled`. No reactive
  side / mode / size (changing those is a structural change; the
  parent can remount via a `key` if needed).
- **Component never writes to `open`.** Programmatic-only close —
  the parent is solely responsible for opening and closing.
- **No `effect()`, no document listeners, no timers, no refs.** Pure
  render function. The animation is CSS-only.
- **Always-mounted DOM.** The panel and backdrop render whether
  `open` is true or false. The `drawer-open` / `drawer-backdrop-open`
  classes are the only thing that flip with `open.val`.
- **No focus trap, no scroll lock, no body-class coordination, no
  `aria-live` announcements.** All deferred (see Out of Scope).
- **One styled form.** No headless variant; no theme override prop.
  Users needing a different look fork into `src/components/`.
- **Framework-owned.** Lives under `.zero/`. `zero update` refreshes
  it.

## Out of Scope

- **Focus management / focus trap.** The drawer does not steal
  focus on open, does not restore focus on close, does not trap
  Tab inside the panel. Matches `Dialog` v1. A future a11y polish
  slice can add focus management across both components together.
- **Body scroll lock during overlay mode.** No
  `document.body.style.overflow = "hidden"` shenanigans. Matches
  `Dialog` v1. Users who need it can do it themselves in an
  `effect()` watching `open`.
- **`aria-live` / screen-reader announcements** on open/close.
  Deferred to a future a11y polish.
- **Built-in close affordances.** No backdrop-click-to-close, no
  Escape-to-close, no `onClose?` callback, no built-in close
  button. The parent owns close fully via the `open` signal. Users
  who want any of these wire them up in their own component (one
  `effect()` for Escape, one click handler on the backdrop via
  caller-owned chrome — Drawer's backdrop is non-interactive).
- **Reactive `mode` / `side` / `size`.** These are plain values
  read once at mount. Toggling the drawer between push and overlay
  at runtime requires a remount (which the parent can force with a
  reactive `key`).
- **Drawer instances stacking / coexisting on the same side.**
  Two Drawers with `side: "right"` at the same time are visually
  undefined (they'd overlap, both at z-index 1000). The singleton-
  per-side pattern is the supported usage. No stacking-context
  bookkeeping ships.
- **Drag-to-resize.** No resize handle. Size is set via the variant
  prop only.
- **Swipe-to-close on touch devices.** Out of scope; defer to
  future mobile polish if it comes up.
- **Auto-flip in RTL.** `side: "left"` always means visual left,
  not start-edge. Matches the framework's overall RTL stance.
- **Built-in slot for a close button or X glyph.** Caller renders
  their own inside `title` or `controls`.
- **A `DrawerController` / `useDrawer()` helper or built-in
  singleton manager.** The framework ships no helper for the
  context-replacement pattern; the showcase demonstrates the
  shape and the docs explain it. If the pattern becomes
  load-bearing across many user apps, a follow-up slice can
  consider a helper.
- **Backdrop in push mode.** No backdrop renders, ever, in push
  mode. The panel is in-flow layout; "behind" doesn't exist. This
  is intentional and load-bearing: underlying content must stay
  fully interactive so patterns like row-click-to-inspect-and-
  swap work without closing the drawer between picks.
- **Animation customisation props** (duration, easing, disable-
  animation). The component uses the framework's existing
  `--duration-normal` and `--ease-out` tokens. Users who need
  different timing override the tokens in their own theme.
- **Snapshot tests.** `expect().toMatchSnapshot()` is not
  implemented in `zero/test`. Tests assert on DOM selectors and
  signal values.
- **A standalone Drawer package.** No npm publication.

## Open Questions

- **Size variant token values.** Spec proposes left/right:
  `16/24/32rem`, top/bottom: `12/18/24rem`. The plan picks final
  values (potentially after eyeballing them in the showcase).
- **Z-index split between panel and backdrop.** Spec proposes
  `999` for the backdrop and `1000` for the panel, mirroring
  `Dialog`'s `1000`. Alternative: a single `1000` with the panel
  appearing after the backdrop in source order. The plan picks;
  the visual outcome is identical, but the explicit split is
  easier to reason about when stacking new overlays later.
- **`role` attribute on the panel in push mode.** Spec proposes
  explicit `role="complementary"`. Alternative: omit the explicit
  `role` and rely on `<aside>`'s implicit role. The plan picks;
  the explicit role matches `Dialog`'s explicit `role="dialog"`
  pattern.
- **`aria-modal="true"` on overlay-mode drawers.** Spec sets it
  to match Dialog. The pedantic-a11y read is that `aria-modal`
  implies focus is trapped, which Drawer does not do. Trade-off:
  the same is true of Dialog today, and we want the SR/AT
  treatment of "this thing is modal-ish". The plan can revisit
  if the framework adds focus trapping later.
- **`hidden`-attribute on empty slot wrappers vs rendering
  nothing.** Spec proposes the `hidden` attribute so the flex
  layout stays stable. Alternative: skip the wrapper entirely
  when the slot is null (cleaner DOM when there's no title, at
  the cost of a slight reflow when the slot transitions from
  null to non-null). The plan picks.
- **CSS file: hand-enumerated `drawer-{push|overlay}.drawer-
  {side}.drawer-{size}` rules (12 selectors) vs. a smaller
  ruleset using CSS variables.** Spec presents the enumerated
  form for clarity. The plan can compact it via a per-side CSS
  variable (`--drawer-axis-size`) set by the size class and
  consumed by the side rules. Either is fine.
- **Default `controls` slot alignment.** Spec sets
  `justify-content: flex-end` so caller-passed buttons line up
  on the trailing edge. Alternative: leave it as a plain flex
  row (`flex-start`) and let the caller wrap in `cluster
  space-between` if they want the right-alignment. The plan
  picks; trailing-edge is the convention for modal/dialog/
  drawer footers across most design systems.
- **Whether to ship `--drawer-{size}` tokens at the design-
  system level** so users can override drawer widths without
  forking the partial. Spec keeps the lengths inside the
  partial for now (consistent with how `Dialog`'s sizes live
  inside `_dialog.scss`). A follow-up slice can promote them
  if the user-facing customisation story expands.
- **AGENTS.md / docs grouping.** Drawer most naturally sits in
  the Overlays group alongside `Dialog` and `Toast`. The plan
  confirms.
- **Manifest size assertion.** The plan confirms the exact
  current number after the most recent component addition and
  bumps the test by three.
- **Test for "the panel is in-flow in push mode".** Spec asserts
  the absence of the `.drawer-backdrop` and the presence of the
  `drawer-push` class. A stronger assertion would check the
  panel's parent flex layout actually reflows, but the JSDOM-
  ish in-memory DOM in `zero/test` doesn't compute layout. The
  plan accepts the class-based assertion as the practical
  equivalent.
