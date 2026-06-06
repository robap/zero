# Spec: dom-shim `<select>` — full HTMLSelectElement surface

## Problem Statement

A test cannot assert *which option a `<select>` currently shows*. The dom-shim
gives every element the same generic attribute-backed `value` property, so
`select.value` reads the `value` **attribute** (always `""` for a select)
instead of deriving from the selected option the way `HTMLSelectElement.value`
does in a browser. Consequences logged as friction entry #77
(`zero_demo/FRAMEWORK_NOTES.md`, 🟡, Area: zero/test):

- `expect(select.value).toBe("b")` is impossible — it always reads `""`.
- A real, browser-visible bug (a `lit_str` mutant on a select-mirror default,
  `q.category ?? "" → "zero"`) is unkillable in-harness because no assertion
  can observe rendered selection.
- The shipped `Select` component's own `Select.test.ts` quietly never asserts
  selection, and drives changes with a *fake* event target
  (`fire(select, "change", { target: { value: "b" } })`) because the element
  itself can't carry the state.
- Hand-setting `select.value = "x"` in test setup writes a `value` attribute
  and selects nothing — divergent from what the same code does in a browser.

### Re-diagnosis (current build, 2026-06-06)

The friction entry's headline — "the reactive `selected=${() => …}` binding
materializes as neither the attribute nor the property" — **does not
reproduce**. A scratch test replicating the exact shape `Select` emits
(`<option value=${o.value} selected=${() => value.val === o.value}>` inside a
`${options}` array child) shows the `selected` attribute and property both
materialize and track the signal, including after `value.set(...)`.
`template.js::_commitAttrSingle` wraps function values in an effect and
`_applyAttr` does boolean add/remove; `dom-shim.js::_defineBoolAttrProp(el,
"selected", "selected")` couples prop↔attribute. Same stale-report pattern as
friction entries #46/#54/#64. The *real* gap is the missing
`HTMLSelectElement` surface described above.

## Background

### Where the relevant code lives

- `runtime/dom-shim.js`
  - `_attachInputProps(el)` (~line 953) — attaches `value` (string-attr prop),
    `checked` / `disabled` / `selected` (bool-attr props), `name`, `type`,
    `placeholder`, `htmlFor`, and the selection-range surface to **every**
    element, tag-blind, from `createElement` (~line 1051 via the attach helper
    at ~line 1023).
  - `_defineStringAttrProp` / `_defineBoolAttrProp` (~lines 853/870) — the
    generic prop↔attribute couplers. `configurable: true`, so tag-specific
    overrides can redefine on top.
  - `_walkDescendants` — document-order walk, reusable for collecting options
    (including inside `<optgroup>`).
- `runtime/dom-shim.test.js` — `node:test` self-tests for the shim; gains the
  select-model coverage.
- `runtime/template.test.js` — `zero test`-run tests; the right home for
  end-to-end "reactive binding → observable selection" cases.
- `crates/zero-runtime/src/lib.rs` — embeds `runtime/dom-shim.js` verbatim;
  body grows, no Rust change.
- `crates/zero-scaffold/src/scaffold/.zero/components/Select.ts` /
  `Select.test.ts` — the shipped component and its under-asserting test.
  Showcase and `examples/*/web` carry copies synced from the scaffold via
  `zero update` (iterate in-repo via the showcase, per workflow).
- `docs/testing.md` § DOM helpers — documents the in-memory DOM surface;
  currently silent on form-element semantics.

### Decisions already made with the user

1. **Full HTMLSelectElement surface** (not a minimal derived-value patch):
   `value`, `selectedIndex`, `options`, `selectedOptions`, option text-content
   value fallback, and `multiple` support.
2. **Strict browser semantics for the `value` setter**: assigning a string
   with no matching option clears the selection (`selectedIndex === -1`,
   `value` reads `""`). No lenient fallback to the old writable-prop behavior —
   tests relying on it are asserting fiction and should surface. Audit the
   demo's hand-set patterns at land time.

## Requirements

All paths relative to repo root.

### 1. `<select>` element surface (tag-aware overrides)

On elements whose tag is `select`, redefine on top of the generic input props:

- **`value`** (getter): the `value` of the first selected option in document
  order; `""` when no option is selected or the select has no options.
- **`value`** (setter): selects the first option whose `value` equals the
  assigned string (after `String()` coercion) and deselects all others; if no
  option matches, **clears all selection** (`selectedIndex === -1`). Never
  writes a `value` attribute on the select.
- **`selectedIndex`** (getter): index of the first selected option within
  `options`, or `-1` when none.
- **`selectedIndex`** (setter): selects the option at that index (deselecting
  others); out-of-range or `-1` clears all selection.
- **`options`**: array-like (a plain array is acceptable) of all descendant
  `<option>` elements in document order, **including options nested in
  `<optgroup>`**. Recomputed per access (no live-collection bookkeeping).
- **`selectedOptions`**: same, filtered to selected options.
- **`multiple`**: boolean prop coupled to the `multiple` attribute (the
  existing `_defineBoolAttrProp` shape).

### 2. Default selection (browser-like)

A non-`multiple` select with at least one option and *no* option marked
selected reports its **first option** as the current selection: `value` reads
the first option's value, `selectedIndex` reads `0`, and `options[0].selected`
reads `true`. A `multiple` select with nothing marked reports
`selectedIndex === -1` and `value === ""`. (Ignore the `size` attribute's
effect on defaults — out of scope.)

### 3. `<option>` element surface

- **`selected`** (getter): the option's *current* selectedness — `true` when
  its `selected` attribute is present, or when it is the default-selected
  first option per Requirement 2.
- **`selected`** (setter): sets/removes the `selected` attribute. Setting
  `true` on an option inside a non-`multiple` select **removes the `selected`
  attribute from every sibling option of that select** (mutual exclusivity).
  Setting under a `multiple` select touches only that option.
- **`value`** (getter): the `value` attribute when present, else the option's
  **text content** (browser fallback). Setter writes the attribute.

The shim treats the `selected` **attribute as current state** — it does not
model the spec's default-vs-dirty distinction (`defaultSelected`,
selectedness dirtiness after user interaction). Document this as an
intentional divergence; it is what the template's attribute-committing
reactive bindings already assume, and the scratch verification confirms those
bindings re-assert correctly on signal change.

### 4. Interaction with reactive bindings

The shipped `Select` binds `selected=${() => value.val === o.value}` per
option; each binding's effect re-runs only on signal change. Exclusivity
enforcement (Requirement 3) must not fight these bindings: removing a sibling
option's attribute does not trigger its effect, and the next signal change
re-asserts every option's state. Add an end-to-end test proving signal-driven
selection, `select.value` reads, and hand-driven writes compose without
loops or stale state.

### 5. No synthetic events

Programmatic writes (`select.value = …`, `selectedIndex = …`,
`option.selected = …`) never dispatch `change` or `input` events — browser
behavior. Tests drive component callbacks by setting state then firing the
event explicitly (`fire(select, "change")`), which now works **without a fake
`target`** because `e.target.value` reads the derived value.

### 6. Inputs and textareas are untouched

The generic `value` / `checked` props on non-select elements keep today's
exact behavior — Combobox and Input depend on the attribute-coupled input
`value`. Only `select`- and `option`-tagged elements gain overrides.

### 7. Tests

- `runtime/dom-shim.test.js`: unit coverage of every getter/setter above —
  derived value, strict no-match setter clearing, `selectedIndex` get/set
  (incl. out-of-range), `options`/`selectedOptions` across `<optgroup>`,
  first-option default (non-multiple) vs no default (`multiple`), option
  text-content value fallback, mutual exclusivity (non-multiple) vs
  independent selection (`multiple`), no events on programmatic writes.
- `runtime/template.test.js` (runs under `zero test`): the `Select`-shaped
  reactive case — render, assert `select.value` / `selectedOptions`, flip the
  signal, re-assert; plus hand-set `select.value` followed by
  `fire(select, "change")` reaching an `@change` handler with the right
  `e.target.value`.

### 8. Shipped `Select.test.ts` strengthened (scaffold)

`crates/zero-scaffold/src/scaffold/.zero/components/Select.test.ts` gains
selection assertions: the option matching the signal renders selected
(`select.value`, `selectedOptions`), selection tracks signal changes, and the
change-event tests drive the real element (set `select.value`, fire `change`
with no fake target). Propagate to showcase and `examples/*/web` via
`zero update` per the existing workflow; the slow `showcase_*` / `examples_*`
integration tests must pass with `--include-ignored`.

### 9. Documentation

- `docs/testing.md` § DOM helpers: add a short "Form elements" note — the
  select model (derived `value`, `selectedIndex`, `options` /
  `selectedOptions`, strict no-match setter, first-option default, `multiple`),
  the option `value` text fallback, the no-synthetic-events rule, and the
  selected-attribute-as-current-state divergence.
- `docs/components.md` (Select section): only if its testing guidance shows
  the fake-target pattern — update the example to the real-element pattern.
  Verify; no other component doc change expected.

### 10. Friction-log close-out (land time, in `zero_demo`)

Flip #77 to `- [x]` with a `**FIXED YYYY-MM-DD**` annotation that **also
corrects the headline**: the selected attribute/property reflection was
already working; the shipped fix is the HTMLSelectElement surface. Re-run the
demo's previously-unkillable select-mirror `lit_str` mutant to confirm it is
now killable, and audit demo tests that hand-set `select.value` for
strict-setter breakage.

## Constraints

- **No new public API** from `"zero"` or `"zero/test"` — this is shim surface
  only; `find` / `findAll` / `fire` / `render` signatures unchanged.
- **Generic props stay generic.** Don't make `_attachInputProps` tag-dispatch
  for inputs; layer select/option overrides via the existing
  `configurable: true` redefinition so non-select behavior is provably
  unchanged.
- **No dirtiness model.** `selected` attribute *is* current state; no
  `defaultSelected`, no user-interaction dirty flag. Documented divergence.
- **No live `HTMLCollection` semantics** for `options` / `selectedOptions` —
  recompute on access.
- Keep functions under ~80 lines; full JSDoc per repo style (`@internal` for
  non-public exports).
- Embedded-string parity: `crates/zero-runtime/src/lib.rs` picks up the grown
  `dom-shim.js` with no Rust edit.
- Boa-era per-element-closure cautions in the shim (`setSelectionRange`
  module-scope pattern) are historical (runner is QuickJS now), but match the
  surrounding code's structure anyway.

## Out of Scope

- `size` attribute effects on default selection or rendering.
- Form association (`select.form`, `option.form`), validation API
  (`checkValidity`, `validity`, `willValidate`), `labels`.
- `defaultSelected` / selectedness-dirtiness modeling.
- Keyboard/UI interaction semantics (typeahead, arrow navigation).
- `option.disabled` / `optgroup.disabled` affecting selectability (disabled
  options remain selectable programmatically; note in docs only if cheap).
- Friction entries #74 (`Input` autofocus/ref), #75 (`Combobox` allowCustom),
  #78 (`Intl` option validation) — separate items.
- Any change to `Combobox`, `Input`, or the input/textarea `value` model.
- `add()` / `remove()` / `item()` / `namedItem()` methods on the select or
  its options collection.

## Open Questions

- **`option.text` and `option.index`**: cheap browser niceties (`text` =
  trimmed text content, `index` = position in the owning select's options).
  Recommendation: include `index` (it falls out of the `options` walk and
  helps assertions), skip `text` (`textContent` already serves). Plan decides.
- **Where tag-awareness hooks in**: a tag check inside the attach helper
  called by `createElement`, vs. a post-attach `_attachSelectProps(el)` /
  `_attachOptionProps(el)`. Plan decides; constraint is only that non-select
  elements are untouched.
- **Owning-select discovery for `option.selected`**: parent walk to the
  nearest `select` ancestor (handles `<optgroup>` nesting). Confirm orphan
  options (no select ancestor) degrade gracefully — getter falls back to
  attribute presence, setter touches only itself.
- **Demo strict-setter audit**: which `zero_demo` tests hand-set
  `select.value` to strings without a matching option, and what they should
  assert instead. Land-time task; scope unknown until run.
