# Spec: Live form properties — bind `value`/`checked`/`selected` as DOM properties

## Problem Statement

`html\`<input value=${signal} />\`` cannot populate or update a form field from
application state. The template runtime commits every dynamic attribute binding
with `el.setAttribute(name, …)`. For form elements, `value`
(`<input>`/`<textarea>`/`<select>`), `checked` (`<input>`), and `selected`
(`<option>`) are **live properties**: their content attribute sets only the
*default*, and once the element exists a real browser tracks the shown/checked
state on the property. A late `setAttribute("value", …)` is ignored for what the
user sees.

The practical consequence: any programmatically-set field is broken. A presign
URL written into state never appears in its `<input>`; a `<select value=${sig}>`
never moves to the chosen option; a `checked=${sig}` toggle never reflects a
state change. It "works" today only while the user is *typing*, because typing
writes the native `.value` — which is why the docs' `value=${name} // signal
binds value` claim does not actually hold for programmatic updates.

The fix is to set the DOM **property** instead of the content attribute for
these live-property targets, on both binding paths, with a guard that avoids
clobbering the caret of a controlled input.

## Background

### Where the code lives

- `runtime/template.js`
  - `_applyAttr(el, name, v)` (line 379) — the **single-binding** commit path
    (`value=${url}`). Currently: `removeAttribute` when `false`/`null`,
    `setAttribute(name, "")` when `true`, else `setAttribute(name, String(v))`.
    Reached via `_commitAttrSingle` for reactive, function, and plain values.
  - `_setJoinedAttr(el, name, statics, values)` (line 443) — the **joined**
    commit path (`value="draft-${id}"`), where static text and one or more
    placeholders concatenate into a string that is `setAttribute`-d. Reached via
    `_commitAttrJoined`.
  - Fully **static** attributes (`<input value="x">` with no placeholder) are
    set at parse time in `flushAttr` (line 115) and never reach either commit
    path — so this change is correctly scoped to *dynamic* bindings only.
- `runtime/dom-shim.js` — the in-memory DOM used by `zero test`.
  - `_attachInputProps` (line 953) is applied to **every** element via
    `_attachElementProps` (line 1206). It defines `value` as a string property
    coupled 1:1 to the `value` attribute (`_defineStringAttrProp`, line 853) and
    `checked`/`selected` as boolean properties coupled to attribute presence
    (`_defineBoolAttrProp`, line 870).
  - `<select>` overrides `value` with an **index-based** getter/setter
    (`_attachSelectProps`, line 1085): `select.value` derives from the selected
    option, and setting it picks the matching option — it is *not* backed by a
    `value` attribute on the select.
  - `<option>` overrides `value`/`selected` (`_attachOptionProps`, line 1148).

### Why the test shim matters here

Because the shim couples `.value`↔attribute for `<input>`/`<textarea>` (and
`.checked`/`<option>.selected`↔attribute presence), today's buggy
`setAttribute` code *already* makes `field.value` / `.checked` / `.selected`
read back correctly under `zero test`. The one exception is `<select>`, whose
index-based getter ignores the stray `value` attribute — so `<select
value=${sig}>` genuinely fails today and passes after the fix.

The user's decision (2026-07-11): **the fake DOM should behave like a real
DOM**, because developers expect it to. The shim will therefore model the
real-browser split between the live property and its content attribute for
`value`/`checked`/`selected` on form controls, so that a genuine
fail-before/pass-after regression test exists for every case — not just
`<select>`.

### Real-browser semantics being modeled

For a form control, the content attribute is the *default*; the live property
diverges once it is set programmatically or by user input (the "dirty" flag):

- `input.value` returns the content attribute until the value is set
  (property assignment or typing), after which `setAttribute("value", …)` no
  longer changes what `.value` returns.
- `input.checked` vs the `checked` attribute (`defaultChecked`), and
  `option.selected` vs the `selected` attribute (`defaultSelected`), follow the
  same default-vs-live split.

## Requirements

Paths relative to repo root.

### 1. Runtime: set live form properties as properties, not attributes

- In `runtime/template.js`, introduce **one shared helper** that decides whether
  a `(el, name)` target is a live form property and, if so, sets the DOM
  property; otherwise it falls through to the existing attribute logic. Both
  `_applyAttr` (single path) and `_setJoinedAttr` (joined path) route through
  it, so `value=${url}` and `value="draft-${id}"` behave identically.
- Live-property rules:
  - `name === "value"` and `el.tagName` is `INPUT`, `TEXTAREA`, or `SELECT`:
    set `el.value` to `v == null ? "" : String(v)`, **guarded** by
    `if (el.value !== next) el.value = next` (see Requirement 2).
  - `name === "checked"` and `el.tagName === "INPUT"`: set `el.checked` to a
    boolean coercion of `v` (falsy → `false`; the string `"false"` also treated
    as `false`, mirroring the joined path where a concatenated value can be a
    string).
  - `name === "selected"` and `el.tagName === "OPTION"`: set `el.selected` to
    the same boolean coercion.
- All other attributes retain the current behavior exactly: `false`/`null` →
  `removeAttribute`; `true` → `setAttribute(name, "")`; else
  `setAttribute(name, String(v))`. The joined path continues to `setAttribute`
  the concatenated string for non-live attributes.
- Reactivity is unchanged: these commits already run inside an `effect` (single
  path via `_commitAttrSingle`, joined via `_commitAttrJoined`), so property
  writes re-run on signal change like attribute writes do today.

### 2. Caret guard (must keep)

The `el.value !== next` guard is essential and must be preserved. A controlled
input (e.g. the shipped `Input` search box) writes its signal on every
keystroke; the signal write re-runs the binding effect and sets `.value` back.
Without the guard, assigning `.value` to the string the user already typed would
move the caret to the end on every keystroke. With the guard, setting to the
current value is a no-op and the caret is left alone. `checked`/`selected` need
no such guard (boolean, no caret).

### 3. DOM shim: model the property/attribute split for form controls

- In `runtime/dom-shim.js`, make the live property diverge from its content
  attribute for form controls, so that a late `setAttribute` does **not** change
  what the property returns once the property has been set:
  - `<input>`/`<textarea>` `value`: property-backed live value that falls back
    to the `value` attribute as its default until the property is set;
    `setAttribute("value", …)` updates only the default.
  - `<input>` `checked` and `<option>` `selected`: the same default-vs-live
    split against their attribute presence (`defaultChecked`/`defaultSelected`).
- **Scope the change to real form controls.** The generic `value` prop is
  attached to every element (line 1206); elements where `value` legitimately
  reflects the attribute (e.g. `<progress>`, `<li>`, `<meter>`, `<option>` whose
  `value` falls back to text content) must keep their current attribute-coupled
  behavior. `<select>` keeps its index-based `value`. Do not regress
  `_attachSelectProps` / `_attachOptionProps`.
- Existing shim conveniences must still hold where they should: a static
  `<input value="x">` reads `.value === "x"` (attribute is the default), and a
  programmatic `.value` assignment reads back via `.value`.

### 4. Fallout audit

Making `.value`/`.checked`/`.selected` diverge from their attributes is a
behavior change to the shim. Audit and fix any existing tests or component code
that assumes coupling — e.g. setting `.value` then asserting
`getAttribute("value")`, or setting the attribute then reading the property.
Run `cargo run -p zero -- test` (JS runtime suite) and
`cargo test --workspace -- --include-ignored` (Rust incl. slow
`component_library`/`showcase_*`/`examples_*`) and reconcile every regression;
none may be left silently changed.

### 5. Tests

- `runtime/template.test.js` (or the nearest existing home) gains, for each
  live property, a **programmatic-update** case that fails before the runtime
  fix and passes after (enabled by Requirement 3):
  - `<input value=${sig}>`: updating the signal updates `field.value`;
    `setAttribute("value", …)` alone (simulating the old path) does **not**.
  - `<input type=checkbox checked=${sig}>`: toggling the signal updates
    `.checked`.
  - `<select value=${sig}>`: setting the signal selects the matching option
    (this one already fails today — keep it as the anchor case).
  - `<option selected=${sig}>`: toggling the signal updates `.selected`.
  - Joined `value="draft-${id}"`: updating `id` updates `field.value` (proves
    the joined path is fixed).
  - Caret guard: setting the signal to the value already present does not
    disturb `selectionStart`/`selectionEnd` (model whatever selection surface
    the shim exposes; `_attachInputSelection` exists at line 974).
- `runtime/dom-shim.test.js` gains direct coverage of the modeled split:
  `setAttribute("value", …)` after a property set does not change `.value`;
  the attribute still serves as the default before any property set; the same
  for `checked`/`selected`.
- Non-live attributes remain unaffected: a regression test that `class`/`href`
  and boolean attrs (`disabled`) still go through `setAttribute`.

### 6. Documentation (user-facing change — required)

- `docs/templates.md`: the `html\`<input value=${name} />\` // signal binds
  value` line (≈ line 80) currently overstates today's behavior. Update it to
  state accurately that `value`/`checked`/`selected` bind to the live DOM
  property so programmatic state updates are reflected in the field, and that
  this is a one-way state→field binding — writing user input back to state still
  requires an `@input`/`@change` handler (the existing `@input` example at
  ≈ line 123 stays the two-way idiom).
- `docs/testing.md`: if it documents the in-memory DOM's value/checked/selected
  behavior, note that these now model the browser's default-vs-live-property
  split (a `setAttribute` after a property set does not change the property).
  If it makes no such claim, no change — state that in the plan so docs are
  considered rather than forgotten.
- `docs/api.md`: no export surface changes; verify no stale claim, otherwise no
  change.

## Constraints

- Repo style: functions under ~80 lines; the shared live-property helper joins
  the existing small-helper pattern in `template.js`. Full JSDoc on any new
  function; `@internal` for non-public helpers. Strong types, no `any`.
- No change to the reactive commit structure (`effect`, `_commitAttrSingle`,
  `_commitAttrJoined`) beyond routing through the new helper.
- The caret guard (Requirement 2) is non-negotiable — controlled inputs must not
  lose their caret.
- Shim realism must be **scoped**: only real form controls diverge; every other
  element's `value` stays attribute-coupled, and `<select>`/`<option>` overrides
  are preserved.
- `zero test` and the full Rust suite (`--include-ignored`) stay green after the
  fallout audit.

## Out of Scope

- Live-property binding for non-form elements that also expose a `value`
  property in browsers (`<progress>`, `<meter>`, `<li value>`, `<param>`) —
  these keep attribute binding; revisit only if a real need appears.
- Two-way binding sugar (a `v-model`-style directive). This item makes the
  existing one-way `value=${sig}` binding correct; writing input back to state
  still uses an explicit `@input`/`@change` handler.
- `<select multiple>` multi-value binding; `<textarea>` defaultValue-via-child-
  text nuances beyond the `.value` property behaving correctly.
- Any change to event binding, `each`, refs, or node commits.
- Reworking the generic all-element `value` prop in the shim into per-tag
  attachments beyond what Requirement 3's scoping needs.

## Open Questions

- **Shim divergence fidelity.** How faithfully to model the dirty flag: the
  minimal model is "`.value` getter returns an internal live value once set,
  else the attribute; `.value` setter sets the internal value; `setAttribute`
  sets only the default." The plan should confirm this is enough for the tests
  and doesn't disturb components that read `.value`/`getAttribute("value")`
  (Combobox ghost-completion writes `el.value` directly — verify it still
  round-trips).
- **`checked`/`"false"` coercion.** Confirm whether the string `"false"` can
  actually reach these paths (joined `checked="${flag}"` is unusual). If not,
  the extra `v !== "false"` guard is dead defensive code — keep or drop per the
  plan's reading; plain-boolean coercion is the common case.
- **Fallout scope.** The exact set of existing tests/components relying on
  shim value↔attribute coupling is unknown until the plan greps for
  `getAttribute("value")` / `.value =` / `setAttribute("checked"` /
  `setAttribute("selected"` usages.
