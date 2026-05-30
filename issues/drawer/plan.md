# Plan: Drawer component

## Summary

Add `Drawer` to the shipped component library: a controlled, edge-anchored
side panel that slides in from `left`/`right`/`top`/`bottom` in one of two
layout modes — `overlay` (`position: fixed` over content with a
non-interactive backdrop) or `push` (in-flow flex sibling whose size animates
from `0`, so the parent layout reflows). It is a pure visual container with
three caller-owned slots (`title`, `body`, `controls`), no built-in close
affordances, no focus trap, no scroll lock. The only stateful prop is
`open: Signal<boolean>`; the parent owns close. The component is a pure render
function — no `effect`, no listeners, no timers — whose only reactive surface
is the `class`/`hidden` bindings driven by `open.val`. The DOM is always
mounted so CSS can animate both open and close. Mirrors `Dialog`'s
controlled-by-signal contract, size-variant prop, and backdrop/animation
tokens. Work lands in the scaffold (the single source of truth), is registered
in the manifest, demonstrated in the showcase (including the Shape A form-drawer
and Shape B inspector-over-table patterns), and documented.

## Prerequisites

- **`zero-framework-spec.md` does not exist in this repo** (req 39 references
  it). Several other issue specs reference the same phantom file; nothing in
  the tree provides it. **Decision: skip req 39** — there is no file to edit.
  If the user wants this doc created, that is a separate task. Flagged again in
  Risks.
- **Component-count language in `docs/components.md` currently reads
  "seventeen", not "eighteen"** as req 38 assumes. The real current roster is
  17 components (`COMPONENT_NAMES` in `crates/zero-scaffold/src/lib.rs`). Adding
  Drawer makes **18**. **Decision: bump "seventeen" → "eighteen"** (not the
  spec's "eighteen → nineteen"), reconciling the off-by-one.

All other open questions are resolved inline in the step details below.

## Resolved open questions (decisions baked into this plan)

- **Size token values:** keep the spec's proposal. left/right `inline-size`
  sm `16rem` / md `24rem` / lg `32rem`; top/bottom `block-size`
  sm `12rem` / md `18rem` / lg `24rem`.
- **Z-index split:** backdrop `999`, panel `1000` (explicit split, mirrors
  Dialog).
- **Push-mode role:** explicit `role="complementary"` on the `<aside>`.
- **`aria-modal="true"`** on overlay-mode panels (matches Dialog; revisit if
  focus-trapping ever lands).
- **Empty slots:** use the native `hidden` attribute on the wrapper (keeps the
  flex slot-count stable), not skip-the-wrapper.
- **CSS shape:** compact the 12 enumerated size selectors into a per-axis CSS
  custom property (`--drawer-inline` / `--drawer-block`) set by the size class
  and consumed by the side rules. (The open question explicitly allows this.)
- **Controls alignment:** `justify-content: flex-end` (trailing-edge footer
  convention).
- **Size tokens stay inside `_drawer.scss`** (no design-system-level
  `--drawer-{size}` promotion; consistent with `_dialog.scss`).
- **AGENTS/docs grouping:** Overlays, alongside `Dialog` and `Toast`.
- **`DrawerSlot` type is widened** to include the function form
  (`() => TemplateResult | string | null`). The spec's prose says slots "can be
  functions that return a `TemplateResult`" and every Background example passes
  one (`title: () => …`), so the exported type must allow it even though the
  spec's prop sketch omits it.

## Steps

- [x] **Step 1: Author the Drawer component, SCSS partial, and unit test**
- [x] **Step 2: Register Drawer in the scaffold manifest and editor declarations**
- [x] **Step 3: Add the showcase route (incl. Shape A & Shape B) and wire navigation**
- [x] **Step 4: Update documentation (AGENTS.md scaffold + docs/components.md)**
- [x] **Step 5: Full integration verification**

---

## Step Details

### Step 1: Author the Drawer component, SCSS partial, and unit test

**Goal:** Create the three framework source files under the scaffold. After
this step they exist on disk but are not yet referenced by `lib.rs`, so the
workspace still compiles and all existing tests pass. Authoring the component
and its unit test together keeps tests first-class.

**Files (create):**
- `crates/zero-scaffold/src/scaffold/.zero/components/Drawer.ts`
- `crates/zero-scaffold/src/scaffold/.zero/components/Drawer.test.ts`
- `crates/zero-scaffold/src/scaffold/.zero/styles/components/_drawer.scss`

**Changes — `Drawer.ts`:**

Imports only from `"zero"`:
```ts
import { html } from "zero";
import type { Signal, TemplateResult } from "zero";
```

Exported types:
```ts
export type DrawerSide = "left" | "right" | "top" | "bottom";
export type DrawerMode = "overlay" | "push";
export type DrawerSize = "sm" | "md" | "lg";
export type DrawerSlot =
  | TemplateResult
  | string
  | null
  | undefined
  | (() => TemplateResult | string | null);

export type DrawerProps = {
  open: Signal<boolean>;
  side: DrawerSide;
  mode?: DrawerMode;
  size?: DrawerSize;
  title?: DrawerSlot;
  body?: DrawerSlot;
  controls?: DrawerSlot;
};
```

Default export `Drawer(props: DrawerProps): TemplateResult` (pure render
function, no `effect`/listeners/timers/refs). Logic:
- Read plain config once: `const mode = props.mode ?? "overlay";`
  `const size = props.size ?? "md";` `const side = props.side;`
  `const { open } = props;`
- `const modeCls = mode === "push" ? "drawer-push" : "drawer-overlay";`
- Build the panel class as a **single reactive function** (the template parser
  cannot mix a static prefix with a `${}` slot inside an attribute, per the
  `component_source_files_emitted` test note — so the whole class string is one
  slot):
  ```ts
  const panelCls = () =>
    `drawer ${modeCls} drawer-${side} drawer-${size}` +
    (open.val ? " drawer-open" : "");
  ```
- Slot-emptiness helper (resolves a function slot, then tests null/empty):
  ```ts
  const slotEmpty = (slot: DrawerSlot): boolean => {
    const v = typeof slot === "function" ? slot() : slot;
    return v == null || v === "";
  };
  ```
- Three section wrappers, each with a reactive native `hidden` attribute and
  the slot rendered directly as a child (the template machinery handles a
  function child reactively, so reactive slots toggle correctly):
  ```ts
  const sections = html`
    <header class="drawer-title" hidden=${() => slotEmpty(props.title)}>${props.title}</header>
    <div class="drawer-body" hidden=${() => slotEmpty(props.body)}>${props.body}</div>
    <footer class="drawer-controls" hidden=${() => slotEmpty(props.controls)}>${props.controls}</footer>`;
  ```
  (Note: `.drawer-body` also gets the `hidden` treatment for consistency and to
  satisfy the "each wrapper independently obeys the rule" test; the body is
  normally non-empty in practice.)
- Backdrop only in overlay mode, reactive open class:
  ```ts
  const backdrop =
    mode === "overlay"
      ? html`<div class=${() => "drawer-backdrop" + (open.val ? " drawer-backdrop-open" : "")}></div>`
      : null;
  ```
- Panel, branching on mode for the static role/aria attributes:
  ```ts
  const panel =
    mode === "overlay"
      ? html`<aside class=${panelCls} role="dialog" aria-modal="true">${sections}</aside>`
      : html`<aside class=${panelCls} role="complementary">${sections}</aside>`;
  ```
- `return html`${backdrop}${panel}`;`

Fully JSDoc-annotate every export per the repo's JS/TS rules (`@param`,
`@returns`, `@template` where applicable; `@internal` for the helper if it is
not exported — here `slotEmpty` is a local, no JSDoc needed beyond a short
line). The default-export function gets a `@param props` / `@returns` block and
a one-paragraph description.

**Changes — `_drawer.scss`** (every rule inside `@layer components`, logical
properties only, tokens only, no `!important`, no magic numbers beyond the
documented exceptions):
```scss
@layer components {
  .drawer-backdrop {
    position: fixed;
    inset: 0;
    background: var(--color-overlay);
    backdrop-filter: blur(4px);              // literal, mirrors Dialog
    z-index: 999;                            // just under the panel
    opacity: 0;
    pointer-events: none;
    transition: opacity var(--duration-normal) var(--ease-out);
  }
  .drawer-backdrop-open {
    opacity: 1;
    pointer-events: auto;
  }

  .drawer {
    background: var(--color-bg);
    color: var(--color-text);
    border: var(--border-thin) solid var(--color-border);
    box-shadow: var(--shadow-lg);
    display: flex;
    flex-direction: column;
    overflow: hidden;                        // clips children during collapse
    transition:
      inline-size var(--duration-normal) var(--ease-out),
      block-size  var(--duration-normal) var(--ease-out),
      transform   var(--duration-normal) var(--ease-out);
  }

  // Size variants set a per-axis custom property; side rules consume it.
  .drawer-left.drawer-sm,  .drawer-right.drawer-sm  { --drawer-inline: 16rem; }
  .drawer-left.drawer-md,  .drawer-right.drawer-md  { --drawer-inline: 24rem; }
  .drawer-left.drawer-lg,  .drawer-right.drawer-lg  { --drawer-inline: 32rem; }
  .drawer-top.drawer-sm,   .drawer-bottom.drawer-sm { --drawer-block: 12rem; }
  .drawer-top.drawer-md,   .drawer-bottom.drawer-md { --drawer-block: 18rem; }
  .drawer-top.drawer-lg,   .drawer-bottom.drawer-lg { --drawer-block: 24rem; }

  // Overlay mode — fixed, sized on the side-axis, off-screen via transform.
  .drawer-overlay { position: fixed; z-index: 1000; }
  .drawer-overlay.drawer-left   { inset-block: 0; inset-inline-start: 0; inline-size: var(--drawer-inline); transform: translateX(-100%); }
  .drawer-overlay.drawer-right  { inset-block: 0; inset-inline-end:   0; inline-size: var(--drawer-inline); transform: translateX(100%); }
  .drawer-overlay.drawer-top    { inset-inline: 0; inset-block-start: 0; block-size: var(--drawer-block); transform: translateY(-100%); }
  .drawer-overlay.drawer-bottom { inset-inline: 0; inset-block-end:   0; block-size: var(--drawer-block); transform: translateY(100%); }
  .drawer-overlay.drawer-open   { transform: none; }

  // Push mode — in-flow; collapses to 0 on the side-axis, animates open.
  .drawer-push.drawer-left,  .drawer-push.drawer-right  { inline-size: 0; }
  .drawer-push.drawer-top,   .drawer-push.drawer-bottom { block-size: 0; }
  .drawer-push.drawer-open.drawer-left,
  .drawer-push.drawer-open.drawer-right  { inline-size: var(--drawer-inline); }
  .drawer-push.drawer-open.drawer-top,
  .drawer-push.drawer-open.drawer-bottom { block-size: var(--drawer-block); }

  // Section layout inside the panel.
  .drawer-title {
    flex: 0 0 auto;
    padding-block: var(--space-md);
    padding-inline: var(--space-lg);
    border-block-end: var(--border-thin) solid var(--color-border);
  }
  .drawer-body {
    flex: 1 1 auto;
    min-block-size: 0;                       // load-bearing: enables overflow
    overflow: auto;
    padding: var(--space-lg);
  }
  .drawer-controls {
    flex: 0 0 auto;
    padding-block: var(--space-md);
    padding-inline: var(--space-lg);
    border-block-start: var(--border-thin) solid var(--color-border);
    display: flex;
    gap: var(--space-sm);
    justify-content: flex-end;
  }
}
```
Notes: the cross-axis for overlay drawers is filled by `inset-block: 0`
(left/right) / `inset-inline: 0` (top/bottom) rather than `100vh`/`100vw` —
pure logical, same visual result. The native `hidden` attribute on empty
wrappers needs no extra CSS (browser default `display: none`).

**Changes — `Drawer.test.ts`** (`import … from "zero/test"`, `afterEach(cleanup)`):
Cover every case from spec req 28:
- **Base markup (overlay, closed):** `open = signal(false)`, `side: "right"`,
  defaults → `find(el, "aside.drawer")` has classes
  `drawer-overlay drawer-right drawer-md`, does **not** have `drawer-open`;
  a `.drawer-backdrop` sibling exists without `drawer-backdrop-open`.
- **`open` toggles classes:** `open.set(true)` adds `drawer-open` to the aside
  and `drawer-backdrop-open` to the backdrop; `open.set(false)` removes both;
  the same node references remain (assert via `find` returning truthy across
  toggles — DOM stays mounted).
- **Push skips backdrop:** `mode: "push"` → `findAll(el, ".drawer-backdrop").length === 0`,
  aside has `drawer-push` (not `drawer-overlay`) and `role="complementary"`.
- **Overlay backdrop + ARIA:** default mode → backdrop present, aside has
  `role="dialog"` and `aria-modal="true"`.
- **Side class:** loop `["left","right","top","bottom"]` → matching `drawer-{side}`.
- **Size class:** default `drawer-md`; loop `["sm","md","lg"]` → matching class.
- **Slots — strings:** `title/body/controls` strings appear inside their
  wrappers; none of the wrappers carry `hidden` (`!el.hasAttribute("hidden")`).
- **Slots — TemplateResults:** same with `html`…`` values.
- **Slots — null/undefined/empty hide wrapper:** `title: null` → `.drawer-title`
  `hasAttribute("hidden")`; same for omitted and `""`; verify each of
  title/controls independently.
- **Slots — reactive function:** `() => state.val ? "A" : "B"`; assert text
  updates after `state.set(...)` without the aside being replaced.
- **No close on backdrop click:** dispatch a click on `.drawer-backdrop`;
  `open.val` stays `true` (trivially, since there is no handler — the assertion
  documents the contract).
- **No close on Escape:** dispatch a `keydown` Escape on `document` (or simply
  assert `open.val` stays `true`); Drawer has no listener, so it stays open.
- **No accumulated document listeners:** save `document.addEventListener`,
  replace with a counting wrapper, render+`cleanup` several Drawers, restore,
  assert the counter is `0`. (Picked over a spy lib since none exists.)

Use `hasAttribute("hidden")` / class checks on `find(el, …)` rather than
`[hidden]` attribute selectors, to avoid relying on attribute-selector support
in the test DOM shim.

**Tests:** `Drawer.test.ts` itself is the unit coverage. It does not run under
`cargo test` yet (it executes via `zero test` once shipped — Step 2 ships it).
This step's correctness gate is simply that the workspace still builds:
`cargo build --workspace`.

### Step 2: Register Drawer in the scaffold manifest and editor declarations

**Goal:** Wire the three new files into the single-source-of-truth manifest and
the editor-facing declarations, alphabetically positioned after `Dialog`,
before `Input`. Bump every length-/roster-coupled assertion. After this step
`cargo test -p zero-scaffold` passes and `zero update` materializes Drawer into
any project.

**Files (modify):**
- `crates/zero-scaffold/src/lib.rs`
- `crates/zero-scaffold/src/scaffold/.zero/components/index.ts`
- `crates/zero-scaffold/src/scaffold/.zero/styles/_components.scss`
- `crates/zero-scaffold/src/scaffold/.zero/components.d.ts`

**Changes — `lib.rs`:**
- Add three `include_str!` constants after the `DIALOG` block (lib.rs ~line 58):
  ```rust
  const TPL_DRAWER_TS: &str = include_str!("scaffold/.zero/components/Drawer.ts");
  const TPL_DRAWER_TEST_TS: &str = include_str!("scaffold/.zero/components/Drawer.test.ts");
  const TPL_DRAWER_SCSS: &str = include_str!("scaffold/.zero/styles/components/_drawer.scss");
  ```
- In `framework_manifest()`, add three tuples after the Dialog entries (~line 169):
  ```rust
  (".zero/components/Drawer.ts", TPL_DRAWER_TS),
  (".zero/components/Drawer.test.ts", TPL_DRAWER_TEST_TS),
  (".zero/styles/components/_drawer.scss", TPL_DRAWER_SCSS),
  ```
- In the test module `COMPONENT_NAMES` (~line 321), insert `"Drawer",` between
  `"Dialog",` and `"Input",`.
- In `framework_manifest_matches_expected_path_set` (~line 986), add the same
  three paths to the `expected` set after the Dialog paths, and update the
  comment `// 17 components × (source, test, scss partial) = 51 entries.` →
  `// 18 components × (source, test, scss partial) = 54 entries.`
  (The test compares sets and `manifest.len() == expected.len()`, so both sides
  must gain the three entries; no separate hard-coded integer to change.)

**Changes — `index.ts`:** insert between the Dialog and Input blocks:
```ts
export { default as Drawer } from "./Drawer.ts";
export type { DrawerProps, DrawerMode, DrawerSide, DrawerSize } from "./Drawer.ts";
```

**Changes — `_components.scss`:** add `@use 'components/drawer';` between
`@use 'components/dialog';` and `@use 'components/input';`.

**Changes — `components.d.ts`:** add a `Drawer` block between the Dialog block
(ends ~line 79) and the Input block, matching the existing convention:
```ts
export type DrawerSide = "left" | "right" | "top" | "bottom";
export type DrawerMode = "overlay" | "push";
export type DrawerSize = "sm" | "md" | "lg";
export type DrawerSlot =
  | TemplateResult
  | string
  | null
  | undefined
  | (() => TemplateResult | string | null);
export type DrawerProps = {
  open: Signal<boolean>;
  side: DrawerSide;
  mode?: DrawerMode;
  size?: DrawerSize;
  title?: DrawerSlot;
  body?: DrawerSlot;
  controls?: DrawerSlot;
};
export function Drawer(props: DrawerProps): TemplateResult;
```
(`Signal` and `TemplateResult` are already imported at the top of the module
block; no import change needed.)

**Tests:** `cargo test -p zero-scaffold` — the iterating tests
(`components_index_re_exports_each_listed`, `component_source_files_emitted`,
`component_test_files_emitted`, `component_partials_use_layer_components`,
`components_aggregate_uses_each_partial`, `components_dts_declares_each_listed`,
`framework_manifest_matches_expected_path_set`) all now cover `Drawer`
automatically via `COMPONENT_NAMES`/the expected set. Confirm the base-class
regex test passes against `` `drawer ` `` in the template.

### Step 3: Add the showcase route (incl. Shape A & Shape B) and wire navigation

**Goal:** Demonstrate every meaningful Drawer configuration plus both canonical
usage patterns, and register the route + home link.

**Files:**
- create `showcase/src/routes/drawer.ts`
- modify `showcase/src/app.ts`
- modify `showcase/src/routes/home.ts`

**Changes — `drawer.ts`** (`import { html, signal, computed } from "zero";`
`import { Drawer, Button, Input, Select, Table } from "zero/components";`),
default export `DrawerRoute(): TemplateResult` rendering, each under a short
`<p class="text-body">` explainer:
1. **Right overlay, default size** — toggle button; `title: "Edit user"`,
   `body`: a small form (`Input` for name + `Select` for role), `controls`:
   "Save" `Button` (`variant: "primary"`) + "Cancel" (`variant: "ghost"`), both
   calling `open.set(false)`.
2. **Right push, default size** — mounted as a flex sibling of a `<main>` (wrap
   both in `class="cluster"`; main is `class="grow stack pad-xl"`) holding
   several paragraphs; toggle button; content reflows when open.
3. **Left overlay** — same shape as #1, `side: "left"` (verifies direction).
4. **Top push, large** — `side: "top"`, `mode: "push"`, `size: "lg"`, inside a
   `class="stack"` parent (verifies vertical-axis reflow).
5. **Bottom overlay, small** — `side: "bottom"`, `size: "sm"`.
6. **Shape A — context-driven forms (push):** three buttons ("Edit user A",
   "Edit user B", "Add user") drive `editingUser`/`addingProduct`-style context
   signals; `drawerOpen = computed(() => …)`; reactive `() => …` substitutions
   in `title`/`body`/`controls` swap content by active context. Prose explainer.
7. **Shape B — inspector over a Table (push):** a `Table` of ~8 records next to
   a push Drawer in a `class="cluster"` layout. `selectedRow = signal<Row|null>(null)`;
   `Table`'s `onRowClick: r => selectedRow.set(r)`;
   `drawerOpen = computed(() => selectedRow.val !== null)`; `title`/`body` are
   `() => selectedRow.val ? … : null`; `controls`: a "Close" `Button` setting
   `selectedRow.set(null)`. Prose explains the load-bearing point — push mode is
   required so rows stay clickable while the drawer is open (overlay's backdrop
   would intercept the clicks), and re-picking a different row swaps the body
   without closing.
8. **Slot variations:** include at least one instance with `title: null` (proves
   the title region collapses) and one with `controls: null`.

End with `<a class="showcase-nav-link" href="/">Back</a>` (matches dialog.ts).
Fully JSDoc-annotate the default export.

**Changes — `app.ts`:** add `import DrawerRoute from "./routes/drawer.ts";`
(after the Dialog import) and `app.route("/drawer", DrawerRoute);` between the
`/dialog` and `/input` route registrations. (Note: the showcase uses eager
imports `app.route("/x", XRoute)`, not the lazy `() => import(...)` form the
spec text mentions — follow the existing eager convention.)

**Changes — `home.ts`:** add `{ name: "Drawer", href: "/drawer" }` to the
`components` array between the `Dialog` and `Input` entries.

**Tests:** covered by Step 5's `showcase_build`/`showcase_dev` runs. No new
unit test here (the route is demonstration code).

### Step 4: Update documentation

**Goal:** Document Drawer in the agent reference and the components doc,
including both usage shapes and the push-mode layout requirement.

**Files:**
- `crates/zero-scaffold/src/scaffold/AGENTS.md`
- `docs/components.md`

**Changes — `AGENTS.md`:** add a `Drawer` row to the `## Components` roster
table (alphabetical, after `Dialog`) and, in the Overlays subsection alongside
`Dialog`/`Toast`, a one-instance usage example, e.g.:
```ts
${Drawer({ open, side: "right", mode: "push", title: "Edit user", body: form, controls: actions })}
```
Note the push-mode requirement (parent must be a flex/grid container on the
relevant axis; not enforced at runtime).

**Changes — `docs/components.md`:**
- Bump the count phrase **"seventeen components" → "eighteen components"**
  (line ~147). (See Prerequisites — the spec's "eighteen → nineteen" is
  off-by-one against the real roster.)
- Add a `Drawer` row to the summary table, alphabetical (after the `Dialog`
  row ~line 160, before `Input`): required props `open: Signal<boolean>`, `side`;
  optional `mode`, `size`, `title`, `body`, `controls`; with a short example.
- Add a prose paragraph near the overlays section describing the singleton +
  context-replacement pattern (both Shape A — forms, and Shape B — inspector
  over a table), noting that push mode renders **no** backdrop so underlying
  content stays interactive (load-bearing for the row-pick flow), and that the
  parent layout must be a flex/grid container along the drawer's axis (document
  only; no runtime enforcement).

**Req 39 (`zero-framework-spec.md`) is intentionally skipped** — the file does
not exist (see Prerequisites/Risks).

**Tests:** none (docs). `AGENTS.md` has section-sentinel tests in `lib.rs`; the
Drawer additions are inside the existing `## Components` section and add no new
top-level headings, so no sentinel test changes are required — but Step 5
re-runs `cargo test -p zero-scaffold` to confirm.

### Step 5: Full integration verification

**Goal:** Prove the whole slice green end to end, including the slow
integration tests that exercise init/build/dev/test flows.

**Files:** none (verification only).

**Changes / commands:**
- `cargo test --workspace` — fast suite.
- `cargo test --workspace -- --include-ignored` — runs the slow integration
  tests: `component_library` (regenerates showcase `.zero/` from the manifest
  via `zero update --yes`, runs `zero test`, asserts each component name
  appears — **note: the hard-coded name list in `component_library.rs` does
  NOT include Drawer**, see below), `showcase_build`, `showcase_dev`,
  `e2e_init_*`, `lint_examples`, etc.
- **`crates/zero/tests/component_library.rs` name list:** it has its own
  hard-coded array mirroring `COMPONENT_NAMES` (Avatar…Toggle). The test asserts
  each listed name appears in the `zero test` report; it does **not** fail if a
  name is missing from the list. To keep it honest, **add `"Drawer"` to that
  array** (between `"Dialog"` and `"Input"`). This is the one integration-test
  edit required (req 36).
- **`showcase_build.rs` / `showcase_dev.rs`:** reviewed — they assert on
  minification and the home route, not on a per-route list or a component
  count, so no widening is needed (req 34/35 satisfied as-is). Re-running them
  confirms the new route compiles and serves.
- Spot-check the showcase visually if practical (`cargo run -p zero -- dev`
  inside a prepared showcase) — open `/drawer`, toggle each instance, confirm
  slide animation both directions, push reflow, and that Shape B rows stay
  clickable with the drawer open. If the environment can't run a browser, say
  so explicitly rather than claiming visual success.

**Tests:** the full `--include-ignored` workspace run is the gate.

## Risks and Assumptions

- **`zero-framework-spec.md` is absent.** Req 39 cannot be done; it is skipped.
  If the user actually maintains this file elsewhere (or wants it created),
  that changes Step 4. Confirm during review.
- **Component count off-by-one.** The plan bumps "seventeen → eighteen" in
  `docs/components.md`, contradicting the spec's literal "eighteen → nineteen".
  Assumption: the real roster (17 today, 18 after Drawer) is authoritative. If
  another component is landing concurrently, re-confirm the number.
- **`component_library.rs` has a second, independent name list** (not derived
  from `COMPONENT_NAMES`). It must be edited by hand (Step 5). Easy to miss.
- **Reactive `hidden` on a function slot calls the slot twice** (once for the
  `hidden` check, once for the child render). Assumption: slot functions are
  pure/cheap (the spec's examples are ternaries over signals). Safe given the
  contract, but worth noting.
- **`DrawerSlot` widened to include functions** beyond the spec's prop sketch.
  Assumption: this matches intent (the prose and every example pass functions).
  If the user wants the narrower type, callers would need `as any`/wrapping —
  confirm.
- **Test DOM shim coverage.** The "no close on Escape / backdrop click" and
  "no accumulated listeners" assertions assume `dispatchEvent` / monkeypatching
  `document.addEventListener` work in the shim. If `dispatchEvent` is
  unsupported, fall back to asserting `open.val` stays `true` directly (the
  contract holds regardless, since Drawer registers no listeners).
- **Transform-based slide uses physical `translateX/Y`.** This is intentional
  and matches the spec; the logical-property requirement governs `inset-*`/
  size, not the transform axis. In RTL, `left`/`right` remain visual (matches
  the framework's stated RTL stance).
