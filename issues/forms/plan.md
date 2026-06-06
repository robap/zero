# Plan: Form state & validation primitive (`createForm` + `error` prop)

## Summary

Add a vendored `form.ts` module to the scaffold components tree exporting
`createForm` — typed field/error/touched signals, validator functions, live
`isValid`, `reset`, `setErrors`, and a `submit()` wrapper that maps
`HttpError` 400/409 `{ errors }` bodies onto fields (unmatched keys surface in
the form-level error). In parallel, every form control gains an
`error?: Signal<string | null>` prop with a `data-field-error` test hook and
`aria-invalid`/`aria-describedby` wiring, built on a shared `_internal.ts`
error-rendering helper. Everything rides the existing scaffold manifest
mechanism (`framework_manifest()` → `zero init`/`zero update`), so no core
runtime (`runtime/*.js`) changes and no manual sync of examples/showcase —
their `.zero/` trees are gitignored and materialized by `zero update`.
The spec's **Usage sketch** section is the target developer experience —
every API decision below must keep that sketch compiling and working
verbatim; it is also the seed for the worked example in Step 7's docs.

Resolved spec open questions (verified against the code):

- **HttpError detection:** real `import { HttpError } from "zero/http"` —
  both the bundler (`crates/zero-bundler/src/resolver.rs:33`) and the test
  runner (`crates/zero-test-runner/src/loader.rs:160`) resolve `zero/http`
  from any module, including `.zero/components/*`.
- **Touched trigger:** first edit (first `set`/`update` on the field's value
  signal), via a façade signal — no new blur props on components needed.
- **Concatenation format:** unmatched server-error messages joined with a
  single space, in `Object.entries` order of the response body.
- **aria-describedby ids:** a `uniqueId(prefix)` counter helper in
  `_internal.ts` (same pattern as `Combobox.ts`'s `_comboboxIdCounter`).
- **`.zero/components.d.ts`:** hand-maintained — updated in the same steps
  that touch the corresponding sources.

## Prerequisites

None.

## Steps

- [x] **Step 1: `uniqueId` + `errorNode` helpers in `_internal.ts`**
- [x] **Step 2: `error` prop on text-entry controls (Input, TextArea, Select)**
- [x] **Step 3: `error` prop on remaining controls (Checkbox, Radio, Toggle, Combobox)**
- [x] **Step 4: `createForm` core (`form.ts`) — fields, touched, validation, `isValid`, `reset`, `setErrors`**
- [x] **Step 5: `submit()` wrapper with HttpError 400/409 mapping**
- [x] **Step 6: integration guards + full slow suite**
- [x] **Step 7: docs — `components.md`, `http.md`, `api.md`**
- [ ] **Step 8: flip the friction-log entry in zero_demo**

---

## Step Details

> **Verification loop for scaffold component changes:** scaffold sources are
> `include_str!`-embedded, so component/test changes are exercised via
> `cargo test -p zero --test component_library -- --include-ignored --nocapture`
> (copies `showcase/`, runs `zero update --yes` from the freshly built binary,
> then `zero test`). Scaffold-level assertions run with
> `cargo test -p zero-scaffold`. Use both per step.

### Step 1: `uniqueId` + `errorNode` helpers in `_internal.ts`

**Goal:** Shared plumbing for Steps 2–3 so seven components don't each invent
id generation and error markup.

**Files:**
- `crates/zero-scaffold/src/scaffold/.zero/components/_internal.ts`
- `crates/zero-scaffold/src/scaffold/.zero/components/_internal.test.ts`

**Changes:**
- `export function uniqueId(prefix: string): string` — module-level counter,
  returns `` `${prefix}-${n}` ``. `@internal`, full JSDoc.
- `export function errorNode(error: Signal<string | null> | undefined, id: string): TemplateResult`
  — returns a reactive slot:
  `html`${() => (error && error.val != null ? html`<small class="text-muted" id=${id} data-field-error="">${error.val}</small>` : html``)}``.
  Composes the existing `.text-muted` utility (per design-system reuse rule —
  no new SCSS partial). `@internal`, full JSDoc. Import `html` into
  `_internal.ts` (currently type-only imports).
- Two derived attribute-value helpers used by components, to keep each
  component diff one-line-ish:
  `export function ariaInvalid(error: Signal<string | null> | undefined): () => string`
  (returns `() => (error?.val != null ? "true" : "false")`) and
  `export function ariaDescribedBy(error: Signal<string | null> | undefined, id: string): () => string`
  (returns `() => (error?.val != null ? id : "")`).

**Tests:** extend `_internal.test.ts`: `uniqueId` returns distinct,
prefix-carrying ids; `errorNode` renders the `<small>` with `data-field-error`
and the message when the signal is non-null, renders nothing when null /
undefined prop, and updates reactively when the signal flips.

### Step 2: `error` prop on text-entry controls (Input, TextArea, Select)

**Goal:** The three controls the demo's forms actually wrap with hand-rolled
error nodes get the built-in affordance first (they share the
`${labelNode}<control>` shape).

**Files:**
- `crates/zero-scaffold/src/scaffold/.zero/components/Input.ts` (+ `.test.ts`)
- `crates/zero-scaffold/src/scaffold/.zero/components/TextArea.ts` (+ `.test.ts`)
- `crates/zero-scaffold/src/scaffold/.zero/components/Select.ts` (+ `.test.ts`)
- `crates/zero-scaffold/src/scaffold/.zero/components.d.ts`

**Changes (same pattern × 3):**
- Add to the props type:
  `error?: Signal<string | null>` with JSDoc: "Optional error message signal;
  when non-null the control renders the message below itself, sets
  `aria-invalid`, and links the message via `aria-describedby`."
- In the component body: `const errId = uniqueId("input-error")` (prefix per
  component: `input-error` / `textarea-error` / `select-error`); append
  `${errorNode(props.error, errId)}` after the control element in the
  returned template; add
  `aria-invalid=${ariaInvalid(props.error)} aria-describedby=${ariaDescribedBy(props.error, errId)}`
  to the `<input>`/`<textarea>`/`<select>` element.
- `components.d.ts`: add the `error?: Signal<string | null>;` line to
  `InputProps`, `TextAreaProps`, `SelectProps`.

**Tests (per component, in its `.test.ts`):**
- no `error` prop → no `[data-field-error]` in the rendered tree,
  `aria-invalid` is `"false"`;
- `error: signal("msg")` → `[data-field-error]` present with text, control has
  `aria-invalid="true"` and `aria-describedby` equal to the error node's `id`;
- setting the signal back to `null` removes the node and flips `aria-invalid`.

### Step 3: `error` prop on remaining controls (Checkbox, Radio, Toggle, Combobox)

**Goal:** Complete the spec's "all form controls" requirement using the
helpers proven in Step 2.

**Files:**
- `crates/zero-scaffold/src/scaffold/.zero/components/Checkbox.ts` (+ `.test.ts`)
- `crates/zero-scaffold/src/scaffold/.zero/components/Radio.ts` (+ `.test.ts`)
- `crates/zero-scaffold/src/scaffold/.zero/components/Toggle.ts` (+ `.test.ts`)
- `crates/zero-scaffold/src/scaffold/.zero/components/Combobox.ts` (+ `.test.ts`)
- `crates/zero-scaffold/src/scaffold/.zero/components.d.ts`

**Changes:**
- Checkbox/Radio/Toggle return a `<label>` root; the error node must be a
  **sibling after the closing `</label>`** (inside the label, clicking the
  message would toggle the control): `html`<label ...>...</label>${errorNode(...)}``.
  aria attributes go on the inner `<input>`.
- Combobox: thread `error?: Signal<string | null>` through `ComboboxProps`
  and `ComboboxCtx`; place the error node after the component's root element;
  aria attributes on its inner `<input>` (which already carries
  `aria-autocomplete` etc.). Reuse `uniqueId` rather than touching the
  existing `_comboboxIdCounter` (leave that refactor alone).
- `components.d.ts`: `error?` line on `CheckboxProps`, `RadioProps`,
  `ToggleProps`, `ComboboxProps`.

**Tests:** same trio of cases as Step 2 per component; for Checkbox/Radio/
Toggle additionally assert the error node is **not** a descendant of the
`<label>`.

### Step 4: `createForm` core (`form.ts`)

**Goal:** The state primitive itself, registered end-to-end in the scaffold so
`import { createForm } from "zero/components"` works in a generated project.
Submit/server mapping is deferred to Step 5 so this step stays reviewable.

**Files:**
- `crates/zero-scaffold/src/scaffold/.zero/components/form.ts` (new)
- `crates/zero-scaffold/src/scaffold/.zero/components/form.test.ts` (new)
- `crates/zero-scaffold/src/scaffold/.zero/components/index.ts`
- `crates/zero-scaffold/src/scaffold/.zero/components.d.ts`
- `crates/zero-scaffold/src/lib.rs`

**Changes — `form.ts` (fully JSDoc'd, no `any`):**

```ts
export type FieldConfig<K extends string> = {
  initial: string;
  /** Per-field validator; return an error message or null. */
  validate?: (value: string, values: Record<K, string>) => string | null;
};

export type FormField = {
  value: Signal<string>;       // façade — marks touched on first write
  error: Signal<string | null>;
  touched: Signal<boolean>;
};

export type FormConfig<K extends string> = {
  fields: Record<K, FieldConfig<K>>;
  /** Cross-field validator; fills only keys without a per-field error. */
  validate?: (values: Record<K, string>) => Partial<Record<K, string>>;
};

export type SubmitAction<K extends string> =
  (values: Record<K, string>) => void | Promise<void>;

export type Form<K extends string> = {
  fields: Record<K, FormField>;
  isValid: Computed<boolean>;
  error: Signal<string | null>;
  values(): Record<K, string>;
  reset(): void;
  setErrors(errors: Partial<Record<K, string>>): void;
  submit(action: SubmitAction<K>): (e: Event) => Promise<void>;
};

export function createForm<K extends string>(config: FormConfig<K>): Form<K>;
```

- **Façade value signal:** for each field, an inner `signal(initial)` is
  wrapped in an object implementing the full `Signal` interface
  (`val` getter delegating to inner, `set`, `update`) — `set`/`update` first
  mark `touched`, write the inner signal, then **re-validate that field iff it
  currently has an error** (per-field validator, then the cross-field result
  for that key), so errors clear live as the user fixes them but never appear
  before submit/`setErrors` (spec req. 6).
- **`runValidators()` (private):** snapshot `values()`, run each per-field
  validator, then merge the cross-field validator's result for keys that
  don't already have an error (matching the demo's critical≤reorder
  precedence). Returns `Partial<Record<K, string>>`.
- **`isValid`:** `computed(() => no keys in runValidators())` — live, reads
  every value signal so it tracks.
- **`reset()`:** restore initials, clear all field errors, all `touched`,
  and the form-level `error`.
- **`setErrors(errors)`:** for every declared field, `error.set(errors[k] ?? null)`
  (clear-then-set, matching the demo's `applyFieldErrors`).
- **`values()`:** plain snapshot `{ k: fields[k].value.val }`. No trimming —
  callers normalize in their submit action.
- `submit()` is declared in this step but implemented in Step 5 (this step's
  body may throw "not implemented" only if the step would otherwise not
  compile — prefer landing Steps 4+5 in one commit if that feels artificial;
  they are split for review size, and `form.test.ts` in this step covers
  everything except `submit`).
- Keep every function under ~80 lines — `createForm` should delegate to
  small private helpers (`makeField`, `runValidators`, `applyErrors`).

**Changes — registration:**
- `index.ts`: `export { createForm } from "./form.ts";` and
  `export type { FieldConfig, Form, FormConfig, FormField, SubmitAction } from "./form.ts";`
- `components.d.ts`: mirror the declarations inside
  `declare module "zero/components"`.
- `lib.rs`: `const TPL_FORM_TS` / `TPL_FORM_TEST_TS` via `include_str!`;
  add `(".zero/components/form.ts", TPL_FORM_TS)` and the test entry to
  `framework_manifest()`. Do **not** add to the test-local `COMPONENT_NAMES`
  (that list drives `.ts`/`.test.ts`/`.scss` triplet checks; `form` has no
  SCSS partial — same category as `_internal.ts`).
- `lib.rs` tests: add assertions that `framework_manifest()` contains both
  `form.ts` paths, that `TPL_COMPONENTS_INDEX_TS` re-exports `createForm`,
  and that `TPL_COMPONENTS_DTS` declares `createForm`.

**Tests — `form.test.ts` (describe name `"createForm"`):**
- fields expose initial values; `touched` starts false, flips on first
  `set`, survives further edits, resets with `reset()`;
- `isValid` is live: false with a failing required validator, true after the
  value signal is fixed — and no field `error` was populated by that;
- per-field validator receives `(value, values)`; cross-field validator only
  fills un-errored keys;
- `setErrors` sets named fields and clears unnamed ones; `reset()` restores
  initials and clears errors/touched/form error;
- editing a field that has an error re-validates just that field (error
  clears when fixed, switches message when a different rule now fails);
  editing an un-errored field surfaces nothing.

### Step 5: `submit()` wrapper with HttpError 400/409 mapping

**Goal:** The piece that kills the three-way drift the friction log observed —
one canonical client-side gate + server-error unwrapping.

**Files:**
- `crates/zero-scaffold/src/scaffold/.zero/components/form.ts`
- `crates/zero-scaffold/src/scaffold/.zero/components/form.test.ts`

**Changes:**
- `import { HttpError } from "zero/http";` at the top of `form.ts`.
- `submit(action)` returns an async handler that, in order:
  1. `e.preventDefault()`;
  2. marks every field touched;
  3. runs `runValidators()`, applies the result to field errors
     (clear-then-set), clears the form-level `error`;
  4. returns without calling `action` if any error;
  5. `await action(values())` — success path does nothing further (caller
     closes dialogs / notifies in `action`);
  6. `catch`: if `err instanceof HttpError && (err.status === 400 || err.status === 409)`
     and `err.body?.errors` is a non-empty object: keys matching declared
     fields → that field's error signal; messages under unmatched keys →
     joined with a single space (entries order) into the form-level `error`.
     If `errors` is missing/empty, or for **any other thrown error**
     (other statuses, non-HttpError), set the generic form-level message
     `"Could not save. Try again."` (module-level const). `submit` never
     rethrows.
- The generic message is a non-configurable constant in v1 (matches the demo
  verbatim).

**Tests (extend `form.test.ts`, using a throwing fake action — no fetch):**
- client-side invalid → `action` not called, field errors applied, all
  fields touched;
- valid → `action` called with the values snapshot; no errors set;
- 400 with `{ errors: { name: "taken" } }` → `fields.name.error` set, form
  `error` stays null;
- 409 mixed `{ errors: { name: "taken", partId: "gone", other: "x" } }` →
  `name` mapped, form `error === "gone x"` (unmatched, space-joined, never
  dropped);
- 400 with empty/missing `errors` → generic form error;
- 500 `HttpError` and a plain `Error` → generic form error, nothing thrown;
- a second submit clears the previous form-level error.

### Step 6: Integration guards + full slow suite

**Goal:** Make the new surface load-bearing in CI the same way the other
components are, and prove the whole tree still materializes and passes.

**Files:**
- `crates/zero/tests/component_library.rs`

**Changes:** add `"createForm"` to the hard-coded report-name list (the
comment there says the list is intentionally hard-coded so a dropped test
file fails loudly — `form.test.ts`'s describe name is `"createForm"`).

**Tests:** run the gates from CLAUDE.md:
- `cargo test --workspace` (fast loop — scaffold unit tests included);
- `cargo test --workspace -- --include-ignored` (slow: `component_library`,
  `e2e_init_*`, `examples_*`, `showcase_*`, `build_full`, `lint_examples`) —
  this is what proves `zero update`-materialized projects type-strip, bundle,
  lint, and test cleanly with `form.ts` and the new props;
- glance at `cargo llvm-cov` only if a Rust path changed meaningfully (this
  item is almost entirely scaffold TS; lib.rs changes are data).

### Step 7: Docs — `components.md`, `http.md`, `api.md`

**Goal:** Spec requirements 10–12; the convention only works if both sides
can read it.

**Files:**
- `docs/components.md`
- `docs/http.md`
- `docs/api.md`

**Changes:**
- `components.md`: new **Forms** section documenting `createForm` — full API
  (config, returned shape, touched semantics, live `isValid` vs gated error
  display, `reset`/`setErrors`/`values`), plus a worked example: a small
  dialog form with two fields, one cross-field rule, and a `submit` action
  doing a `http.post` — explicitly framed as the replacement for the
  hand-rolled signal-pair pattern. Add the `error` prop to each form
  control's documented props (Input, TextArea, Select, Checkbox, Radio,
  Toggle, Combobox), noting the `data-field-error` hook and aria wiring.
  Match the page's existing per-component format.
- `http.md`: new **Server validation errors** subsection: the
  `400`/`409` + `{ "errors": { "<field>": "<message>" } }` body convention,
  how `createForm().submit()` consumes it (matched keys → field errors;
  unmatched keys → form-level error, concatenated; empty → generic), and a
  note that backends should key `errors` by the client's field names.
- `api.md`: add `createForm` + exported types under the `zero/components`
  section of the flat reference, and the `error` prop to each control's
  entry, following the page's existing line format.

**Tests:** none mechanical; re-read `docs/index.md` blurbs to confirm no
index-level description needs a touch (Components blurb says "seventeen
shipped components" — `createForm` is a function, not a component; update the
count/wording only if the components.md intro wording forces it).

### Step 8: Flip the friction-log entry in zero_demo

**Goal:** Spec requirement 13 — close the loop with the originating project.

**Files:**
- `../zero_demo/FRAMEWORK_NOTES.md`

**Changes:** per that file's convention, flip the 2026-06-06 🟡
**no form state/validation/server-error primitive** entry to `- [x]` and
append `**FIXED <date>** (<commit/PR>): createForm + error prop on all form
controls shipped in zero/components; {errors} 400/409 convention documented
in http.md.` Leave the separate ref/autofocus 🟢 entry untouched (out of
scope). Note: re-read the entry before editing — the file was recently
modified and is append-only by convention.

**Tests:** none.

## Risks and Assumptions

- **Façade signal compatibility.** Assumes template bindings and components
  only use `val`/`set`/`update` (the declared `Signal` interface). If the
  runtime's `isSignal`-style duck-typing checks identity or hidden internals
  anywhere on the write path, the façade needs to become a real `signal()`
  plus an `effect`-free wrapper — verify early in Step 4 with `form.test.ts`
  binding a façade signal through `Input`.
- **TS generic inference.** `Record<K, FieldConfig<K>>` should infer `K`
  from object-literal keys; if inference degrades to `string`, switch the
  config to a mapped-type formulation. Pure type-level risk — runtime
  (type-stripped by swc) is unaffected either way.
- **Bare `data-field-error=""` attribute** is static template text, not a
  binding, so parser support is assumed safe; if the parser objects, give it
  the value `"true"`.
- **`aria-describedby=""` when no error** mirrors the existing
  `placeholder=${props.placeholder ?? ""}` pattern. If empty-string aria
  attributes prove noisy, the fallback is accepting always-present
  `aria-describedby` pointing at a sometimes-absent id.
- **Step 4/5 split** leaves `submit` unimplemented for one step; if a
  placeholder body offends the "compilable, test-passing" rule reviewers
  expect, execute Steps 4–5 as one commit — the plan keeps them separate
  purely for description clarity.
- **Combobox is the largest diff** (props + ctx threading, ~600-line file
  with its own a11y wiring); if it balloons, it can land as its own commit
  within Step 3 without blocking the rest.
- **Docs page formats** (components.md per-component layout, api.md line
  format) are asserted by convention, not tests — match them by reading the
  pages at execution time, not from this plan's memory.
