# Spec: Form-control ergonomics — `autofocus`/`attrs` props + Combobox `allowCustom`

> **Revised 2026-06-06 during execution.** The original design (a public
> `ref?: Ref<…>` pass-through prop) was rejected by the user after seeing it
> in code: form controls should take value-style props
> (`Input({ autofocus: true, attrs: { name: "x" } })`), not framework `Ref`
> objects. Requirement 1 and its tests were rewritten; everything else
> stands.

## Problem Statement

Two open friction-log entries (`zero_demo/FRAMEWORK_NOTES.md`) keep real apps
dropping out of the shipped form controls:

- **#74** (🟢, 2026-06-06): `Input` has no ref/autofocus — 2 of the demo's 3
  forms keep a raw `<input>` *solely* to call `ref()` focus when a
  drawer/dialog opens (noted in a code comment). The component is otherwise a
  perfect fit; one missing prop forces the fallback.
- **#75** (🟡, 2026-06-06): `Combobox` is strict-select only. `revertOnBlur`
  reverts any text that isn't a known option label, and only `pick()` writes
  `value`, so a "suggest existing but allow new" field (the demo's Add Part
  category) cannot adopt it without losing new-category entry. The
  `<input list>` + `<datalist>` hand-roll from the original 2026-05-23
  combobox entry therefore *still* stands — this is the second time the gap
  has bitten.

Both are the same failure class: a shipped component that covers 95% of a form
need and forces a raw-element drop-out for the last 5%, losing the component's
affordances (error wiring, debounce, a11y attributes) and hand-duplicating its
SCSS classes. The `select-element` spec (2026-06-06) explicitly deferred both
as separate items; this item closes them together.

## Background

### Where the code lives

- `crates/zero-scaffold/src/scaffold/.zero/components/` — the canonical
  component sources: `Input.ts`, `Select.ts`, `TextArea.ts`, `Checkbox.ts`,
  `Radio.ts`, `Toggle.ts`, `Combobox.ts`, each with a sibling `.test.ts`.
  Showcase and `examples/*/web` carry copies synced from the scaffold via
  `zero update` — iterate in-repo via the showcase per the established
  workflow.
- `crates/zero-scaffold/src/scaffold/.zero/components.d.ts` — public type
  surface for `zero/components`; every prop change lands here too.
- `runtime/template.js` — the `ref=` binding: `_commitRef(el, refObj)` assigns
  `refObj.el = el` and registers a disposal effect that nulls it. A `Ref` is a
  plain `{ el }` holder from `ref()`; nothing prevents component code from
  reusing a caller-supplied ref as its own internal ref.
- `runtime/dom-shim.js` — already implements `el.focus()` (line ~1298, via
  `_focusElement`), `blur`, and `document.activeElement`, so focus behavior is
  assertable in `zero test` with **no shim changes needed**.
- `Combobox.ts` internals relevant to `allowCustom`:
  - `ctx.inputRef: Ref<HTMLInputElement>` — internal ref to the `<input>`;
    ghost completion (`applyGhost`) writes `el.value` + selection directly.
  - `pick(ctx, opt)` — the only writer of `props.value`; also sets
    `ctx.lastLabel` and fires `onChange(value, option)`.
  - `revertOnBlur(ctx)` — the strict-revert rule: if visible text is not a
    known option label, reset `el.value` to `lastLabel.val`. Wired to both
    `@blur` and the outside-mousedown effect.
  - `onKeyEnter` picks `options.val[highlight.val]` when one exists;
    `onFetchResolved` sets `highlight` to `0` whenever options arrive, and
    `applyGhost` only fires when the typed prefix grew and a case-insensitive
    `startsWith` match exists.

### Precedents this extends

- The forms item (#73, shipped 2026-06-06) added `error?: Signal<string|null>`
  to **all seven** form controls in one pass — the per-control consistency
  precedent this item follows for `ref`.
- `onChange?` on `Input`/`Select` (#69) established "direct callback over
  effect-bridge" as the reaction pattern; `allowCustom` commits fire the
  existing `Combobox.onChange` rather than inventing a new channel.

### Decisions already made with the user

1. **Value-style props, not a `Ref` pass-through** *(revised 2026-06-06)*:
   - `autofocus?: boolean` — the component focuses its underlying element
     after mount. Implemented with an internal ref + a microtask `.focus()`
     call (the native `autofocus` attribute only acts on document insertion,
     which is unreliable for drawer/dialog content rendered later).
   - `attrs?: Record<string, string | number | boolean>` — broad native
     attribute passthrough applied to the underlying element post-commit.
     **Additive-only collision policy:** a key is skipped when the element
     already carries that attribute, so component-owned attributes
     (`class`, `value`, `aria-invalid`, Radio's `name`, …) always win.
     `true` sets an empty attribute, `false` skips the key, numbers are
     stringified. Plain values only (display-prop convention) — not
     reactive, no event handlers.
2. **All seven form controls get both props** — Input, Select, TextArea,
   Checkbox, Radio, Toggle, Combobox — matching the `error?` precedent, not
   just the `Input` named in the friction entry.
3. **`allowCustom` near-match rule: auto-pick the existing option.** Text that
   case-insensitively equals a loaded option's whole label resolves to that
   option (its canonical `value` + `label`, normal `pick()` path) rather than
   committing a case-variant duplicate as custom text. Same spirit as the
   ghost matcher, which is already case-insensitive. Only truly novel text
   commits as custom.

## Requirements

All paths relative to repo root; component paths are the scaffold sources.

### 1. `autofocus` + `attrs` props on every form control

- All seven controls (`Input`, `TextArea`, `Select`, `Checkbox`, `Radio`,
  `Toggle`, `Combobox`) accept:
  - `autofocus?: boolean` — when `true`, the component calls `.focus()` on
    its underlying element in a microtask after mount
    (`document.activeElement` then reads that element). For the
    label-wrapped controls (`Checkbox`, `Radio`, `Toggle`) and `Combobox`,
    the focused element is the inner `<input>`.
  - `attrs?: Record<string, string | number | boolean>` — native attributes
    applied to the same underlying element in the same microtask.
    Additive-only: keys whose attribute is already present on the element
    are skipped (component-owned attributes win). `true` → empty attribute
    (`required=""`), `false` → key skipped entirely, numbers stringified.
- Both implemented via an internal `ref()` bound in the template plus one
  shared `@internal` helper in `_internal.ts` (single microtask servicing
  both props; no-op when neither prop is given).
- When both props are omitted, rendered output and behavior are
  byte-identical to today (the internal ref binding is provably inert:
  `_commitRef` assigns `.el` and registers one disposal effect).
- `Combobox`: `attrs`/`autofocus` target the internal typeahead `<input>`;
  ghost completion owns `el.value` and the selection range, which `attrs`
  cannot touch anyway (additive-only, and `value` is a property, not an
  attribute, on the shim input).

### 2. `Combobox.allowCustom`

- New prop `allowCustom?: boolean`, default `false`. Default-off behavior is
  unchanged in every observable way (strict revert, `pick()`-only writes).
- With `allowCustom: true`, a **commit** of the visible input text happens on:
  - **blur** (both the `@blur` handler and the outside-mousedown path that
    currently call `revertOnBlur`), and
  - **Enter**, when the visible text is not an accepted ghost/highlight pick
    (see Requirement 3).
- Commit semantics for visible text `t`:
  - If `t` case-insensitively equals the whole label of a currently loaded
    option, resolve to that option via the normal `pick()` path (canonical
    `value`/`label`, `onChange(value, option)` fires). Match consults only
    `ctx.options.val` — the most recently loaded list; no extra fetch.
  - Otherwise: `props.value.set(t)`, `ctx.lastLabel.set(t)`, dropdown closes,
    and `onChange(t, { value: t, label: t })` fires with a synthesized option.
    Callers who care whether a commit was custom compare against their known
    options.
  - Empty text commits `""` (clears the value) — a creatable field the user
    deliberately emptied must not resurrect the previous label.
  - A commit must be idempotent-quiet: blur after Enter on the same text must
    not fire `onChange` a second time (signal `.set` dedupes the value write;
    the callback needs the same guard).

### 3. Enter/Tab precedence under `allowCustom`

Enter currently picks `options[highlight]` whenever one exists, and
`onFetchResolved` always highlights index 0 — so with `allowCustom` a naive
"highlight wins" rule would make Enter on typed text `"Xyz"` silently pick an
unrelated first suggestion instead of committing `"Xyz"`. Required rule:

- Enter picks the highlighted option **only when the visible text equals that
  option's label** (i.e. the user accepted a ghost completion or arrowed onto
  the option, which rewrites the visible text); otherwise Enter commits the
  visible text per Requirement 2.
- Tab keeps its current shape (pick when visible text equals the highlighted
  label, else close); the close → blur path then performs the custom commit
  naturally.
- Escape continues to close without committing; the subsequent blur commit
  still applies (browser-like: Escape cancels the dropdown, not the text).
  If the plan finds this surprising in tests, it may instead make Escape
  restore `lastLabel` — decide there, document either way.

### 4. Tests

- Each control's sibling `.test.ts` gains: `autofocus: true` makes the
  underlying element `document.activeElement` after a microtask (and the
  right element — `tagName`/`type` — for the label-wrapped controls);
  `attrs` keys land as attributes (string, `true` → `""`, number
  stringified), `false` keys are absent, and a component-owned key
  (e.g. `class`) is *not* overridden; omitting both props leaves markup
  unchanged.
- `Combobox.test.ts` gains the `allowCustom` matrix: custom commit on blur,
  custom commit on Enter with non-matching text, ghost-accepted Enter still
  picks the option, case-insensitive near-match resolves to the existing
  option (canonical value), empty-text commit clears, no double `onChange` on
  Enter-then-blur, outside-click commit, and `allowCustom: false` strict
  revert unchanged.
- Showcase / examples copies updated via `zero update`; the slow `showcase_*`
  / `examples_*` integration tests pass under
  `cargo test --workspace -- --include-ignored`.

### 5. Documentation (user-facing change — required)

- `docs/components.md`: `autofocus` + `attrs` added to the seven controls'
  prop summaries (one shared prose note explaining the focus-on-open
  drawer/dialog pattern and the additive-only `attrs` collision rule, with a
  short snippet); the Combobox section documents `allowCustom` semantics —
  commit triggers, the case-insensitive near-match auto-pick, the
  synthesized-option `onChange` shape — and that `attrs`/`autofocus` target
  the inner typeahead input.
- `crates/zero-scaffold/src/scaffold/.zero/components.d.ts`: all new props
  with JSDoc, in the same pass as the implementations (the test-matcher-drift
  entry is the cautionary tale for letting `.d.ts` and runtime drift).
- `docs/api.md`: verify the flat reference picks up the new props; update if
  it enumerates per-component props.

### 6. Friction-log close-out (land time, in `zero_demo`)

Flip #74 and #75 to `- [x]` with `**FIXED YYYY-MM-DD**` annotations. Verify in
the demo: the two raw focus-`<input>`s adopt `Input` + `autofocus`, and the
Add Part category `<datalist>` hand-roll adopts `Combobox` with `allowCustom`.

## Constraints

- **No new runtime/dom-shim surface** — `focus()`/`activeElement` already
  exist; this item is scaffold-component-only (plus its docs/types).
  **Amended during execution (user-approved):** one dom-shim *bugfix* (not
  new surface) — `_makeEventTarget.addEventListener` never recorded the
  `capture` flag, so `_fireListenersOn`'s `entry.capture !== capture` check
  skipped every document/window listener for events bubbling up from
  descendants. Discovered by the outside-click commit test; fixed with a
  regression test (`dom-shim.test.js` "document listener fires for events
  bubbling up from descendants").
- Controlled-component contract holds: no new internal state observable by the
  parent; `value` stays the single source of truth; `allowCustom` adds writers
  to it but no parallel state.
- `allowCustom: false` (and omitted-`autofocus`/`attrs`) behavior must be
  provably unchanged — existing component tests pass unmodified except where
  they assert the old Enter-picks-any-highlight shape, which Requirement 3
  intentionally tightens; audit those individually.
- No SCSS changes — neither feature has a visual surface.
- Repo style: functions under ~80 lines (Combobox's commit logic joins the
  existing small-helper pattern), full JSDoc, strong types (`AllowCustom`
  needs no new exported types beyond the prop).
- Iterate via the in-repo showcase (`zero update --yes` there), not scratch
  dirs.

## Out of Scope

- Friction entry #78 (`Intl.DateTimeFormat` option validation) — separate
  item, different layer (test runtime).
- A public `ref?: Ref<…>` prop — rejected during execution in favor of
  `autofocus`/`attrs`; revisit only if observe-the-element needs outgrow
  them.
- Reactive `attrs` (signal/computed values) and event handlers in `attrs` —
  plain values only.
- Multi-select / tags mode for Combobox; creatable-option *persistence*
  (callers own what happens to a new value).
- `autofocus`/`attrs` on non-form components (Button, Dialog, Table, …).
- Any change to ghost-completion, debounce/race, or dropdown semantics beyond
  the Enter-precedence tightening in Requirement 3.
- Deduplication beyond the case-insensitive whole-label match (no trimming
  rules, no diacritic folding).

## Open Questions

- **Microtask vehicle**: `Promise.resolve().then(...)` vs `queueMicrotask`
  for the post-mount apply — whichever the test runtime supports; plan
  decides (both behave identically for the focus/attrs case).
- **Whitespace in custom commits**: commit `"  Widgets "` verbatim or
  trimmed? Recommendation: trim for the match comparison *and* the committed
  value (an invisible-whitespace category is never what the user meant), but
  the plan should confirm against how the demo's datalist hand-roll behaves.
- **Escape-then-blur** (Requirement 3): commit or restore `lastLabel`? Spec
  recommends commit-on-blur regardless; plan validates against test
  ergonomics and documents the choice.
- **Existing Enter tests**: which current `Combobox.test.ts` cases encode the
  old "highlight always wins" Enter rule and need updating vs. preserving —
  scope unknown until the plan reads them.
