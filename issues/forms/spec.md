# Spec: Form state & validation primitive (`createForm` + `error` prop)

## Problem Statement

Every form built on zero hand-rolls the same four things: a pair of signals per
field (value + error), a validate-on-submit path plus a live `isValid` computed,
a reset routine, and the `HttpError` 400/409 → `{ errors }` field-mapping with a
form-level fallback. The zero_demo friction log (`FRAMEWORK_NOTES.md`,
2026-06-06 🟡) records the cost concretely: its three dialogs
(`AddPartDialog`, `AddLocationDialog`, `AdjustPartDialog`) each reimplemented
this stack and drifted into three different server-error unwrappings — one
silently drops server error keys that don't match a form field, another
special-cases a single key (`partId`) into the form-level error by hand.
`AddPartDialog.ts` is ~340 lines, roughly 150 of which are pure form-state
boilerplate (a 19-signal `FormState`, `applyFieldErrors`, `resetFormState`,
`useSubmit`).

This item ships a `createForm` primitive in `zero/components`, an
`error?: Signal<string | null>` prop on every form control, and documents the
`{ errors: Record<field, string> }` 400/409 response convention so client and
server agree on the wire shape.

## Background

- **Components are vendored.** `zero/components` resolves to scaffolded TS
  files in the user project's `.zero/components/` (source of truth:
  `crates/zero-scaffold/src/scaffold/.zero/components/`). Each component has a
  sibling `.test.ts`. Shared non-component helpers live in `_internal.ts`
  (e.g. `debounce`). The same vendored tree is committed under
  `examples/*/web/.zero/` and exercised by the `component_library` /
  `examples_*` integration tests and the `showcase/` app.
- **Form controls today:** `Input`, `Select`, `TextArea`, `Checkbox`, `Radio`,
  `Toggle`, `Combobox`. They take `value: Signal<...>` and optional `label`,
  `onChange`, etc. None has an error affordance; the demo wraps each control in
  a `stack gap-xs` div with a hand-rolled
  `<small class="text-muted" data-field-error=...>` node.
- **Reactivity:** `signal()`, `computed()` from `zero`
  (`runtime/reactivity.js`, typed in `runtime/zero.d.ts`).
- **HTTP errors:** `HttpError` (`runtime/http.js:8`) carries `status`,
  `statusText`, `body` (parsed JSON when the response was JSON). The demo's
  backend returns `400`/`409` with `{ errors: Record<field, string> }`; this is
  convention, not yet documented.
- **Observed validation patterns** (what the API must express without new
  concepts): required + trimmed length caps, integer-in-range checks on
  string-valued number inputs, and one cross-field rule
  (critical ≤ reorder point).

## Usage sketch

Non-normative, but the API must make exactly this shape work — it is the
demo's `AddLocationDialog` (~180 lines of form plumbing) reduced to its
intent:

```ts
import { html } from "zero";
import { Button, Input, createForm } from "zero/components";
import { createLocation } from "../store.ts";
import { closeDrawer } from "../../../shared/stores/drawer.ts";

const form = createForm({
  fields: {
    code: {
      initial: "",
      validate: (v) =>
        v.trim() === "" ? "Code is required."
        : v.trim().length > 10 ? "Code must be 10 characters or fewer."
        : null,
    },
    name: {
      initial: "",
      validate: (v) => (v.trim() === "" ? "Name is required." : null),
    },
    slotsTotal: {
      initial: "10",
      validate: (v) =>
        intInRange(v, 1, 999) === null
          ? "Slots total must be between 1 and 999."
          : null,
    },
  },
  // Cross-field rules go here; only fills fields without a per-field error.
  // validate: (values) => ({ ... }),
});

// Validates, gates, and maps HttpError 400/409 {errors} automatically.
// The action only builds the typed body and handles success.
const onSubmit = form.submit(async (values) => {
  await createLocation({
    code: values.code.trim(),
    name: values.name.trim(),
    slotsTotal: Number(values.slotsTotal),
  });
  closeDrawer();
});

const body = html`
  <form class="stack gap-md" @submit=${onSubmit}>
    ${() => (form.error.val ? html`<p class="text-muted" role="alert">${form.error}</p>` : html``)}
    ${Input({ value: form.fields.code.value, label: "Code", error: form.fields.code.error })}
    ${Input({ value: form.fields.name.value, label: "Name", error: form.fields.name.error })}
    ${Input({ value: form.fields.slotsTotal.value, type: "number", label: "Slots total", error: form.fields.slotsTotal.error })}
    <button class="button button-primary button-md" type="submit"
      disabled=${() => !form.isValid.val}>Add location</button>
  </form>
`;

// Reopening the dialog later: form.reset();
```

What this kills, per field: the hand-written value+error signal pair, the
`applyFieldErrors` / `resetFormState` routines, the `stack gap-xs` wrapper div
with a hand-rolled `<small data-field-error>` node, and the per-form
`HttpError` unwrapping in `useSubmit`.

## Requirements

### `createForm`

1. New vendored module in the scaffold components tree (e.g.
   `.zero/components/form.ts`), exported from the components `index.ts` so
   `import { createForm } from "zero/components"` works. Fully JSDoc-annotated
   and strongly typed (field names flow through as a string-literal union; no
   `any`).
2. `createForm` takes a declaration of fields, each with an initial value and
   an optional per-field validator `(value, values) => string | null`, plus an
   optional form-level cross-field validator
   `(values) => Partial<Record<field, string>>`. Validator-function style only —
   no built-in rule DSL.
3. For each declared field the returned form exposes:
   - `value: Signal<string>` — bind directly to a control's `value` prop.
   - `error: Signal<string | null>` — bind directly to a control's `error` prop.
   - `touched: Signal<boolean>` — false until the user first interacts with the
     field; reset by `reset()`.
4. The form exposes:
   - `isValid` — a live computed that is `true` iff running all validators over
     the current values yields no errors. Suitable for driving a disabled
     submit button (this is how all three demo forms use it).
   - `error: Signal<string | null>` — the form-level error.
   - `reset()` — restores every field to its initial value, clears all field
     errors, all `touched` flags, and the form-level error.
   - `setErrors(errors)` — applies a `Record<field, string>` to field error
     signals (clearing fields not present, matching the demo's
     `applyFieldErrors` semantics).
   - `submit(action)` — returns an async event handler for `@submit`.
5. `submit(action)` behavior, in order:
   - `preventDefault()` on the event.
   - Mark all fields touched, run all validators, apply resulting field errors,
     clear the form-level error.
   - If any validation error: return without calling `action`.
   - Call `await action(values)` (the caller builds its typed request body and
     performs the request/success handling inside `action`).
   - On `HttpError` with status 400 or 409 and a body containing
     `errors: Record<string, string>`: keys matching declared fields set those
     field errors; messages under **unmatched keys are surfaced in the
     form-level error** (concatenated), never silently dropped. If `errors` is
     absent or empty, set a generic form-level message.
   - Any other thrown error (other statuses, non-`HttpError`): generic
     form-level message. `submit` never rethrows.
6. Validation errors are *displayed* against the touched/submit model (the live
   `isValid` must not cause errors to render before interaction): a field's
   error signal is only populated by submit, `setErrors`, or — if per-field
   on-interaction validation is implemented — after the field is touched.

### `error` prop on form controls

7. Every form control — `Input`, `Select`, `TextArea`, `Checkbox`, `Radio`,
   `Toggle`, `Combobox` — accepts `error?: Signal<string | null>`. When the
   signal is non-null the component renders the message in a designated error
   element carrying a stable test hook (the demo's convention is a
   `data-field-error` attribute) and styled via existing design-system
   classes/utilities (no new bespoke CSS rules where an existing class
   composes).
8. When an error is present the underlying control sets `aria-invalid` and
   associates the message via `aria-describedby`.
9. Each touched component's vendored `.test.ts` gains coverage for the error
   prop (renders on non-null, absent on null, aria wiring); `form.ts` gets a
   sibling `form.test.ts` covering validation, touched, reset, `setErrors`,
   `isValid`, and the full `submit` matrix (client-side reject, success,
   400/409 matched keys, unmatched keys → form error, empty `{errors}`,
   non-HttpError throw).

### Convention & docs (user-facing — required)

10. `docs/http.md`: document the `{ errors: Record<field, string> }` body
    convention for `400`/`409` responses and how `createForm.submit` consumes
    it (including the unmatched-key → form-level behavior).
11. `docs/components.md`: reference section for `createForm` (full API,
    a worked example replacing the hand-rolled pattern) and the new `error`
    prop on each form control.
12. `docs/api.md`: add `createForm` (and any exported types) and the `error`
    prop to the flat reference.
13. After the fix lands, flip the friction-log entry in
    `zero_demo/FRAMEWORK_NOTES.md` per that file's convention.

### Sync

14. The vendored copies under `examples/*/web/.zero/` and the `showcase` app
    must be regenerated/updated by whatever mechanism the existing
    `component_library` / `examples_*` tests enforce, and those slow
    integration tests (`cargo test --workspace -- --include-ignored`) must
    pass.

## Constraints

- Lives in the vendored components tree only — **no core runtime changes**
  (`runtime/*.js` untouched) and no new import surface beyond
  `zero/components`.
- v1 field values are `string`-typed (matching `Input`/`Select`/`TextArea`;
  the demo keeps numeric inputs as strings until submit-time conversion).
- Validate-on-submit is the contract; `isValid` is live but display of errors
  is gated (req. 6). No async validators.
- CLAUDE.md style rules apply: functions < ~80 lines, full JSDoc, strong types.
- Error rendering must compose existing design-system classes (`.text-muted`,
  spacing utilities) rather than introducing parallel rules.

## Out of Scope

- The separate friction-log entry about `Input` ref/autofocus pass-through
  (2026-06-06 🟢) — tracked independently; do not fold it in here.
- Async / server-roundtrip validators (uniqueness stays on the 409 path).
- A declarative rule DSL (`required`, `maxLength`, …).
- Boolean/array field values (Checkbox/Toggle-backed form state) — the `error`
  prop lands on those controls, but `createForm` v1 manages string fields.
- Dirty-tracking / unsaved-changes warnings, field arrays, nested objects,
  multi-step wizards.
- Migrating zero_demo's three forms (happens in the demo repo after release).

## Open Questions

- **Touched trigger:** is `touched` set on first value edit (works with today's
  components, no new props) or on blur (conventional, but requires a blur
  pass-through on each control)? First-edit is the cheaper default; decide in
  plan.
- **HttpError detection inside a vendored component module:** import
  `HttpError` from `zero/http` (does the bundler/test-runner resolve that
  import from `.zero/components/`?) or structurally check
  `err.status` / `err.body` to avoid coupling `zero/components` to `zero/http`?
- **Form-level concatenation format** for multiple unmatched server keys
  (separator, ordering).
- **`aria-describedby` id generation** — components currently render no ids;
  decide a collision-safe scheme.
- Whether `.zero/components.d.ts` (scaffold) needs regeneration or is
  hand-maintained alongside `index.ts`.
