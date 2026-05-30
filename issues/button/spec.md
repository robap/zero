# Spec: Button `type` / `form` / `name` / `value` props

## Problem Statement

`zero/components`' `Button` can only render a plain click button. Its
props are `variant / size / disabled / loading / onClick / children` —
there is no way to set the underlying `<button>`'s `type`, `form`,
`name`, or `value` attributes. The moment an app needs a real form
submit button, `Button` can't express it.

This bit a real app (friction-log entry `2026-05-30`, FRAMEWORK_NOTES
line 66): the drawer `controls` slot pattern puts action buttons
*outside* the `<form>` they submit (the form lives in the `body` slot,
the buttons in the `controls` slot). Wiring Enter-to-submit / a submit
button across that boundary requires `type="submit" form="<form-id>"`
on the button. Because `Button` can carry neither, both Parts panels in
that app fell back to a raw `<button class="button button-primary
button-md" type="submit" form=…>` that hand-copies the SCSS classes
`Button` emits — losing the component's `loading` affordance and
drifting if those class names ever change.

The component library is meant to keep apps on-spec by default; a button
that can't submit a form forces every form-bearing app off the
primitive. This slice closes that gap.

## Background

### What exists today

`crates/zero-scaffold/src/scaffold/.zero/components/Button.ts` is the
canonical source (the showcase and examples carry regenerated copies
under their own committed `.zero/`). Current shape:

```ts
export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";
export type ButtonSize = "sm" | "md" | "lg";

export type ButtonProps = {
  variant?: ButtonVariant;
  size?: ButtonSize;
  disabled?: boolean;
  loading?: boolean;
  onClick?: (event: Event) => void;
  children?: TemplateResult | string;
};

export default function Button(props: ButtonProps = {}): TemplateResult {
  const variant = props.variant ?? "primary";
  const size = props.size ?? "md";
  const cls = `button button-${variant} button-${size}`;
  const spinnerCls = `button-spinner spinner spinner-${variant} spinner-sm`;
  const spinner = props.loading
    ? html`<span class=${spinnerCls} role="status" aria-label="loading"></span>`
    : null;
  const handler = (e: Event) => {
    if (props.disabled) return;
    props.onClick?.(e);
  };
  return html`<button class=${cls} disabled=${props.disabled ?? false} @click=${handler}>${spinner}${props.children ?? ""}</button>`;
}
```

Key facts about the current implementation:

- **No `type` attribute is emitted at all.** The rendered `<button>`
  therefore inherits the browser default, which for a button inside a
  `<form>` is `type="submit"`. So a `Button` dropped in a form *already*
  submits on click today — an undocumented, accidental footgun.
- **`disabled`** is passed as a plain boolean to the template binder,
  which normalizes `false`/`null`/`undefined` to "remove the attribute"
  and `true` to an empty string. It both sets the native `disabled`
  attribute (blocking native submit + click) and is re-checked in the
  JS `@click` handler.
- **`loading`** renders a leading spinner only. It does **not** set
  `disabled` and is **not** checked in the `@click` handler, so a
  loading button stays fully clickable and (in a form) submittable —
  a double-submit hazard for the exact "submitting…" use case this
  slice enables.

### Decisions made during refine

The user confirmed each of the following:

- **Add the full native submit-button surface:** `type`, `form`,
  `name`, `value`. `type` includes `"reset"`.
- **`type` defaults to `"button"`.** The component always emits an
  explicit `type`. Default `"button"` means a `Button` never
  accidentally submits an enclosing form — the safe, design-system
  convention (MUI, Chakra, etc.). This is a **deliberate behavior
  change**: any caller relying on today's implicit-submit behavior
  must now pass `type="submit"` explicitly.
- **`loading` blocks activation.** When `loading` is true the button
  becomes non-interactive: it sets the native `disabled` attribute and
  the `@click` handler short-circuits, so a loading submit button can't
  double-submit or re-fire `onClick`. `loading` is treated as a
  busy/disabled state. (Behavior change: loading buttons are no longer
  clickable while loading.)

### Touch points (all files already exist — no new files, no manifest growth)

This is an edit-in-place change to one existing component, so
`framework_manifest()` and its length-coupled assertions are
**untouched** (contrast the Drawer slice, which added files).

- `crates/zero-scaffold/src/scaffold/.zero/components/Button.ts` — the
  component.
- `crates/zero-scaffold/src/scaffold/.zero/components/Button.test.ts` —
  tests (currently 3 cases: base class, onClick fires, disabled
  suppresses click).
- `crates/zero-scaffold/src/scaffold/.zero/components.d.ts` — the
  `"zero/components"` editor declaration. Current `ButtonProps` block at
  lines ~27–34; `Button` function decl at ~35.
- `crates/zero-scaffold/src/scaffold/.zero/components/index.ts` — line 7
  re-exports `ButtonProps, ButtonSize, ButtonVariant`. A new `ButtonType`
  alias (if introduced) is added here.
- `docs/components.md` — the summary-table `Button` row (~line 156) and
  the component-library reference subsection for Button.
- `docs/api.md` — the `Button` row (~line 127) and the type-alias list
  (~line 142) if a `ButtonType` alias is added.
- `showcase/src/routes/button.ts` — add a form-submit demo.
- **Regenerated `.zero/` copies** that must stay byte-identical to the
  manifest: `showcase/.zero/components/Button.ts` + `Button.test.ts` +
  `components.d.ts`, and the same files under
  `examples/counter/web/.zero/` and `examples/tracker/web/.zero/`. CI
  drift checks (`zero update --yes` produces no diff) must pass.

## Requirements

### Component API

1. `ButtonProps` gains four optional props:
   - `type?: ButtonType` where `ButtonType = "button" | "submit" | "reset"`.
   - `form?: string` — the id of the `<form>` the button is associated
     with (for buttons rendered outside their form).
   - `name?: string` — the button's form-control name.
   - `value?: string` — the button's form-control value (lets a
     multi-button form's submit handler identify which button fired).

2. A `ButtonType` type alias is declared and exported alongside
   `ButtonVariant` / `ButtonSize` (component source, `components.d.ts`,
   and `index.ts` re-export). The plan confirms whether to name it
   `ButtonType` or inline the union; `ButtonType` is preferred for
   consistency with the existing exported aliases.

3. `type` defaults to `"button"`. The rendered `<button>` **always**
   carries an explicit `type` attribute equal to `props.type ?? "button"`.

### Rendered markup

4. The `<button>` emits `type="${type}"` unconditionally (always
   present, default `"button"`).

5. `form`, `name`, and `value` are emitted **only when provided**. When
   the prop is `undefined`, no corresponding attribute appears on the
   element (no empty `form=""` / `name=""` / `value=""`). The plan
   verifies the template binder's handling of `undefined` string-valued
   attribute bindings and picks the idiomatic expression accordingly
   (the binder already removes attributes for `undefined` boolean
   values; string-attribute behavior must be confirmed — see Open
   Questions).

6. The existing `variant` / `size` / `loading`-spinner / `children`
   rendering is unchanged. Class names, the spinner element, and
   `children` placement stay byte-compatible with today's output for
   callers who pass none of the new props except the now-explicit
   `type="button"` attribute.

### Loading / disabled behavior

7. The button is non-interactive when **either** `disabled` **or**
   `loading` is true:
   - The native `disabled` attribute is set when `disabled || loading`.
   - The `@click` handler short-circuits (returns without calling
     `onClick`) when `disabled || loading`.

8. Setting `disabled` while `loading` (or vice versa) does not
   double-apply or error; the condition is a simple OR.

9. `disabled` alone continues to behave exactly as today (attribute set,
   click suppressed). The only behavior change to `disabled` is none —
   the change is that `loading` now joins it in gating activation.

### Tests (`Button.test.ts`)

10. Existing three cases continue to pass (base class renders; `onClick`
    fires on click; `disabled` suppresses `onClick`).

11. New cases:
    - **Default type.** A `Button` with no `type` renders a `<button>`
      whose `type` attribute is `"button"`.
    - **Explicit type.** `type: "submit"` and `type: "reset"` each
      render the matching `type` attribute.
    - **`form` attribute.** `form: "edit-form"` renders
      `form="edit-form"`; omitting `form` renders no `form` attribute.
    - **`name` / `value` attributes.** Provided values render the
      matching attributes; omitted props render neither.
    - **Loading sets disabled.** `loading: true` renders a `<button>`
      with the native `disabled` attribute present.
    - **Loading suppresses onClick.** With `loading: true`, firing a
      `click` does not call the `onClick` spy (callCount 0).
    - **Loading still renders the spinner.** The `.button-spinner`
      element is present when `loading: true` (guards against the
      disabled-coupling regressing the spinner).
    - `afterEach(cleanup)` (already present).

12. Tests use the existing `zero/test` helpers (`render`, `find`,
    `fire`, `spy`, `cleanup`). Attribute assertions read via the DOM
    element's attribute API; the plan confirms the exact accessor the
    dom-shim supports (e.g. `getAttribute("type")`).

### Type declaration & docs

13. `components.d.ts` `ButtonProps` block is extended with the four new
    props and the `ButtonType` alias, kept consistent with the source
    types. The `Button` function declaration is unchanged in shape.

14. `index.ts` line 7's `export type { ... } from "./Button.ts"` adds
    `ButtonType` (if introduced).

15. `docs/components.md`:
    - The `Button` summary-table row lists the new optional props
      (`type`, `form`, `name`, `value`).
    - The Button reference subsection documents the new props, the
      `"button"` default for `type`, the loading-blocks-activation
      behavior, and shows a submit-button-outside-its-form example
      (`type="submit" form="<id>"`) — the motivating use case.

16. `docs/api.md` — the `Button` row is unchanged in signature
    (`Button(props?: ButtonProps)`), but if a `ButtonType` alias is
    added it joins the documented alias list (~line 142).

### Showcase

17. `showcase/src/routes/button.ts` gains a demo section showing a
    `type="submit"` `Button` wired to a `<form>` — ideally the
    button-outside-form pattern (`form="<id>"`) that motivated the
    slice — plus a `loading` submit button demonstrating that it is
    non-interactive while loading. The section includes a brief
    explanatory paragraph consistent with the route's existing style.

### Regeneration / drift

18. After the source `Button.ts` / `Button.test.ts` / `components.d.ts`
    change, the committed `.zero/` copies under `showcase/`,
    `examples/counter/web/`, and `examples/tracker/web/` are
    regenerated so `zero update --yes` from inside each produces zero
    drift. The plan enumerates exactly which committed copies exist and
    must be refreshed (and confirms whether examples commit `.zero/` or
    gitignore it).

19. Integration tests that build/run the showcase and examples
    (`tests/showcase_build.rs`, `tests/showcase_dev.rs`,
    `tests/component_library.rs`, and any examples build test) continue
    to pass with the new route content and the extra Button tests.

## Constraints

- **No new files; no manifest change.** This edits existing manifest
  entries in place. `framework_manifest()` and its length assertions are
  untouched.
- **No new Rust or npm dependencies.** Rides the existing scaffold /
  transpile / SCSS pipeline.
- **No CSS change.** `_button.scss` is untouched — `type`/`form`/
  `name`/`value` are attribute-only; the `disabled` styling
  (`.button:disabled`) already covers the loading state once `loading`
  sets the native `disabled` attribute.
- **Backward compatibility, with one intentional break.** Output stays
  byte-compatible except: (a) every `<button>` now carries an explicit
  `type="button"` (previously absent), and (b) `loading` buttons are now
  non-interactive. Both are deliberate, documented behavior changes
  agreed in refine.
- **Strong typing.** `type` is the `ButtonType` union, not `string`.
  Props remain optional. No `any`.
- **JSDoc.** The component's existing JSDoc block is updated to document
  the new props, the `"button"` default, and the loading-gates-
  activation behavior, per the repo's full-JSDoc rule.
- **Framework-owned.** The fix lands in the scaffold source; user
  projects receive it via `zero update`.

## Out of Scope

- **Flipping the friction-log entry.** FRAMEWORK_NOTES line 66 lives in
  the separate `zero_demo` repo. Marking it `[x]` with a FIXED
  annotation happens there after this lands, not in this slice.
- **`Button` as an `<a>` / link button** (`href` prop, anchor
  rendering). Not requested; submit-button surface only.
- **`formaction` / `formmethod` / `formenctype` / `formtarget` /
  `formnovalidate`** override attributes. Rarely reached; can be a
  follow-up if an app needs per-button form-action overrides.
- **`aria-busy` / richer loading semantics** beyond setting `disabled`
  and the existing `role="status"` spinner. A broader a11y pass is
  separate.
- **A `Drawer` `controls`-slot convenience** (e.g. a built-in submit
  button slot). Out of scope; `Drawer` stays a pure container and the
  caller composes `Button({ type: "submit", form })`.
- **Form-validation plumbing** on `Button` or `Input`. Validation is
  the parent's job (restated from the component-library spec).
- **Changing `disabled` semantics.** `disabled`'s behavior is unchanged;
  only `loading` is newly coupled to activation gating.

## Open Questions

- **`undefined` string-attribute binding.** Does the template binder
  remove a string-valued attribute when the bound value is `undefined`
  (the way it does for boolean attributes), so `form=${props.form}` with
  `props.form === undefined` emits no attribute? If not, the plan picks
  the idiomatic conditional expression (e.g. building the markup so the
  attribute is omitted, or relying on whatever the binder's documented
  contract is). This determines the exact JSX-less `html` expression for
  `form` / `name` / `value`.
- **`ButtonType` alias vs inline union.** Preference is a named exported
  `ButtonType` for symmetry with `ButtonVariant` / `ButtonSize`; the
  plan confirms and updates `index.ts` + `components.d.ts` + `docs/api.md`
  accordingly.
- **dom-shim attribute accessor in tests.** Confirm the exact API the
  in-memory DOM exposes for reading `type` / `form` / `name` / `value`
  and the presence of the `disabled` attribute (`getAttribute`,
  `hasAttribute`, or a property), and whether descendant/attribute
  selectors in `find` can target them.
- **Which committed `.zero/` copies exist.** The plan enumerates the
  exact set (showcase + which examples) and whether examples commit or
  gitignore `.zero/`, to know which files to regenerate for the drift
  check.
- **Showcase form demo shape.** Whether to demonstrate the
  button-outside-form (`form="<id>"`) pattern specifically, or a simpler
  in-form submit. Preference is the outside-form pattern since it's the
  motivating case; the plan confirms it renders cleanly in the showcase
  layout.
