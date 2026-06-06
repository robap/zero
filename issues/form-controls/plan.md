# Plan: Form-control ergonomics — `autofocus`/`attrs` props + Combobox `allowCustom`

> **Revised 2026-06-06 mid-execution** alongside the spec: the public
> `ref?: Ref<…>` prop was replaced by `autofocus?: boolean` +
> `attrs?: Record<string, string | number | boolean>` (additive-only).
> Steps 1–3 were rewritten; Steps 4–8 stand.

## Summary

Add `autofocus?: boolean` and `attrs?: Record<string, string | number |
boolean>` props to all seven form controls — implemented with an internal
template `ref=` binding plus one shared `_internal.ts` helper that, in a
single post-mount microtask, applies additive-only native attributes and
calls `.focus()` — and an `allowCustom?: boolean` prop on `Combobox` that
commits visible free text to `value` on blur/Enter, with case-insensitive
whole-label near-matches resolving to the existing option. All work is
scaffold-component-only — the runtime (`template.js` `_commitRef`, dom-shim
`focus()`/`activeElement`/`setAttribute`) already provides everything
needed. Steps are ordered mechanical-first (native-props plumbing), then the
Combobox behavior change in two slices (commit machinery, then key
precedence), then docs, propagation, and friction-log close-out.

## Prerequisites

None. The spec's open questions are resolved as follows (rationale in the
step details):

1. **Microtask vehicle** — `Promise.resolve().then(...)` (guaranteed in the
   QuickJS test runtime; `queueMicrotask` availability unverified). Tests
   `await Promise.resolve()` (or `wait(0)`) before asserting focus/attrs.
2. **Whitespace** — trim for both the match comparison and the committed
   value. An invisible-whitespace category is never what the user meant.
3. **Escape-then-blur** — commit on blur regardless. Escape cancels the
   dropdown, not the text (browser-like); documented in the Combobox docs
   section.
4. **Existing Enter tests** — audited: "Enter accepts the highlight" drives
   ArrowDown first, which rewrites the visible text to the option label, so
   it passes under the new visible-text-equality rule. No existing test
   encodes "highlight wins over non-matching text". The new precedence rule
   is nevertheless gated behind `allowCustom: true` so default-off behavior
   is byte-identical.

## Steps

- [x] **Step 1: `applyNative` helper + `autofocus`/`attrs` on Input, TextArea, Select**
- [x] **Step 2: `autofocus`/`attrs` on Checkbox, Radio, Toggle**
- [x] **Step 3: `autofocus`/`attrs` on Combobox (inner typeahead input)**
- [x] **Step 4: Combobox `allowCustom` — commit machinery + blur/outside-click paths**
- [x] **Step 5: Combobox `allowCustom` — Enter/Tab/Escape precedence**
- [x] **Step 6: Documentation — components.md + api.md verify**
- [x] **Step 7: Propagate to showcase/examples, full suite incl. slow tests**
- [x] **Step 8: Friction-log close-out in zero_demo (#74, #75)** *(scope reduced by user at land time: components pulled into the demo via `zero update` and the friction log flipped with FIXED annotations, but the demo's own source adoption — raw focus-inputs → `Input({autofocus})`, datalist → `Combobox({allowCustom})` — is left to the demo's owner)*

---

## Step Details

### Step 1: `applyNative` helper + `autofocus`/`attrs` on Input, TextArea, Select

**Goal:** The shared post-mount mechanism plus the three text/choice
controls — the focus-on-open case from friction #74. Done first because it
establishes the pattern (helper signature, JSDoc wording, test shape) Steps
2–3 copy.

**Files:**
- `crates/zero-scaffold/src/scaffold/.zero/components/_internal.ts` + `_internal.test.ts`
- `crates/zero-scaffold/src/scaffold/.zero/components/Input.ts` + `Input.test.ts`
- `crates/zero-scaffold/src/scaffold/.zero/components/TextArea.ts` + `TextArea.test.ts`
- `crates/zero-scaffold/src/scaffold/.zero/components/Select.ts` + `Select.test.ts`
- `crates/zero-scaffold/src/scaffold/.zero/components.d.ts`

**Changes:**
- `_internal.ts` gains the shared type + helper (full JSDoc, `@internal`):

  ```ts
  export type NativeAttrs = Record<string, string | number | boolean>;

  /** Post-mount native-prop application: additive attrs, then focus. */
  export function applyNative(
    controlRef: Ref<Element>,
    attrs?: NativeAttrs,
    autofocus?: boolean,
  ): void
  ```

  No-op (no microtask) when `attrs` is nullish and `autofocus !== true`.
  Otherwise `Promise.resolve().then(...)`: bail if `controlRef.el` is null
  (cleaned up before the tick); for each `attrs` entry skip `false` values
  and keys where `el.hasAttribute(key)` is already true, else
  `setAttribute(key, value === true ? "" : String(value))`; finally
  `if (autofocus === true) el.focus()`.
- Each component imports `ref` from `"zero"`, `type Ref`, and
  `applyNative` / `type NativeAttrs` from `./_internal.ts`.
- Props gain `autofocus?: boolean` and `attrs?: NativeAttrs` (JSDoc: focus
  after mount; additive-only native attributes — component-owned attributes
  win).
- In each component body: `const controlRef: Ref<HTML…Element> = ref();`,
  `applyNative(controlRef, props.attrs, props.autofocus);`, and
  `ref=${controlRef}` on the control element in the template. The internal
  binding is inert for callers (`_commitRef` assigns `.el` + one disposal
  effect — no markup change).
- `components.d.ts`: export `NativeAttrs` and add both props to
  `InputProps` / `TextAreaProps` / `SelectProps`, same pass.

**Tests:**
- `_internal.test.ts`: `applyNative` unit coverage — string/number/boolean
  attr application, `false` skipped, additive-only (pre-existing attribute
  untouched), focus fired, null-ref bail, no-op fast path.
- Per control, one new `it`: render with
  `autofocus: true, attrs: { name: "x", "data-test": "y", required: true, class: "nope" }`;
  `await Promise.resolve()` (or `wait(0)`); assert the element (found by
  tag) carries `name`/`data-test`/`required=""`, its `class` is still the
  component's, and `document.activeElement` is the element. Existing
  base-markup tests double as the omitted-props regression guard.

### Step 2: `autofocus`/`attrs` on Checkbox, Radio, Toggle

**Goal:** Same props on the toggle-style controls, applied to the underlying
`<input>` (not the wrapper `<label>`), completing the non-Combobox six.

**Files:**
- `crates/zero-scaffold/src/scaffold/.zero/components/Checkbox.ts` + `Checkbox.test.ts`
- `crates/zero-scaffold/src/scaffold/.zero/components/Radio.ts` + `Radio.test.ts`
- `crates/zero-scaffold/src/scaffold/.zero/components/Toggle.ts` + `Toggle.test.ts`
- `crates/zero-scaffold/src/scaffold/.zero/components.d.ts`

**Changes:** Identical pattern to Step 1: both props on all three props
types; internal `ref()` bound on the inner `<input … />` element (these are
self-closing void elements inside a `<label>` — the props target the input,
JSDoc says so explicitly); `applyNative(...)` call. Note Radio renders
`name`/`value` attributes from first-class props, so `attrs.name` /
`attrs.value` are skipped by the additive rule — JSDoc mentions it.
`components.d.ts` updated in the same pass.

**Tests:** Same shape as Step 1 per control, plus: the focused/attributed
element is the checkbox/radio/switch input (`getAttribute("type")`, for
Toggle `role="switch"`), proving it's the input and not the wrapper; for
Radio, `attrs: { name: "smuggled" }` does **not** override the prop-rendered
`name`.

### Step 3: `autofocus`/`attrs` on Combobox (inner typeahead input)

**Goal:** Combobox accepts the same props, targeting its internal typeahead
`<input>`. Done before the `allowCustom` steps so those steps work against
the final props shape.

**Files:**
- `crates/zero-scaffold/src/scaffold/.zero/components/Combobox.ts` + `Combobox.test.ts`
- `crates/zero-scaffold/src/scaffold/.zero/components.d.ts`

**Changes:**
- `ComboboxProps` gains `autofocus?: boolean` and `attrs?: NativeAttrs`
  (JSDoc: applied to the inner typeahead input).
- In `Combobox()`, one call after ctx construction:
  `applyNative(ctx.inputRef, props.attrs, props.autofocus);` — the internal
  ref already exists; no other plumbing.
- `components.d.ts` updated in the same pass.

**Tests:** New `it` in `Combobox.test.ts`:
- Render with `autofocus: true, attrs: { name: "category" }` and
  `staticLoader([])`; after a microtask the inner `.combobox-input` carries
  `name="category"` and is `document.activeElement`. Note: dom-shim
  `focus()` dispatches a `focus` event, which runs `handleFocus` — with no
  resolved options this is a no-op; assert the dropdown stays hidden
  (guards the interaction).
- Ghost completion still works alongside `autofocus` (repeat the ghost-fill
  assertion with the new props passed) — proves the microtask doesn't
  disturb typeahead state.

### Step 4: Combobox `allowCustom` — commit machinery + blur/outside-click paths

**Goal:** The behavioral core of friction #75: free text commits to `value`
on dismissal instead of strict-reverting. Key handling is deferred to Step 5
so this step's surface is exactly "what happens when the field is dismissed".

**Files:**
- `crates/zero-scaffold/src/scaffold/.zero/components/Combobox.ts` + `Combobox.test.ts`
- `crates/zero-scaffold/src/scaffold/.zero/components.d.ts`

**Changes:**
- `ComboboxProps` gains `allowCustom?: boolean` (default `false`), JSDoc
  describing commit-on-blur/Enter, the case-insensitive near-match auto-pick,
  and the synthesized-option `onChange` shape. `ComboboxCtx` gains
  `allowCustom: boolean` (resolved once in the ctx construction like
  `debounceMs`).
- New `@internal` helper, < 80 lines:

  ```ts
  /** Commit the visible text per the allowCustom rules. @internal */
  function commitText(ctx: ComboboxCtx): void
  ```

  Logic:
  1. `const el = ctx.inputRef.el; if (el == null) return;`
  2. `const t = el.value.trim();`
  3. **Idempotence guard:** if `t === ctx.lastLabel.val`, close quietly
     (`open.set(false)`, `highlight.set(-1)`) and return — no signal write,
     no `onChange`. This covers blur-after-Enter, blur-after-pick, and
     focus-then-blur with `initialLabel` untouched.
  4. **Near-match:** `const m = ctx.options.val.find(o => o.label.toLowerCase() === t.toLowerCase());`
     if found, `pick(ctx, m)` and return (canonical value/label, normal
     `onChange(value, option)`).
  5. **Custom commit:** `ctx.props.value.set(t)`; `ctx.lastLabel.set(t)`;
     `el.value = t` (normalizes trimmed whitespace and any ghost tail);
     `ctx.open.set(false)`; `ctx.highlight.set(-1)`;
     `ctx.props.onChange?.(t, { value: t, label: t })`. Empty `t` flows
     through the same path — commits `""` (clears).
- New `@internal` dispatcher replacing the two `revertOnBlur` call sites
  (`@blur` handler and `registerOutsideClick`):

  ```ts
  /** Dismissal: strict-revert or allowCustom commit. @internal */
  function handleDismiss(ctx: ComboboxCtx): void {
    if (ctx.allowCustom) commitText(ctx);
    else revertOnBlur(ctx);
  }
  ```

  `revertOnBlur` itself is unchanged — the default-off path stays
  byte-identical.
- `components.d.ts` updated in the same pass.

**Tests:** New `describe("Combobox allowCustom")` block (or `it`s in the
existing one):
- Blur commits novel text: type `"widgets"`, blur → `value.val === "widgets"`,
  input shows `"widgets"`, `onChange` called once with
  `("widgets", { value: "widgets", label: "widgets" })`.
- Case-insensitive near-match auto-picks: options containing
  `{ value: "a1", label: "alpha" }`, type `"ALPHA"` (suppress ghost by
  backspacing or use non-prefix case), blur → `value.val === "a1"`, input
  shows canonical `"alpha"`, `onChange` got the existing option object.
- Whitespace trimmed: type `"  widgets "`, blur → committed value and visible
  text are `"widgets"`.
- Empty clears: with `initialLabel: "Old"` and `value: signal("old-id")`,
  clear the field, blur → `value.val === ""`, `onChange("", …)` fired.
- Idempotent dismissal: after a commit, fire `blur` again → `onChange`
  still called exactly once, value unchanged.
- Outside-click commits: open via typing, dispatch `mousedown` on
  `document` (outside the root) → same commit as blur.
- Default-off regression: the existing strict-revert test re-run with
  `allowCustom` omitted is untouched and still passes (no edits to it).

### Step 5: Combobox `allowCustom` — Enter/Tab/Escape precedence

**Goal:** Enter commits typed text instead of silently picking an unrelated
auto-highlighted suggestion; ghost-accepted Enter still picks. Separated from
Step 4 because it rewires `onKeyEnter` rather than the dismissal path.

**Files:**
- `crates/zero-scaffold/src/scaffold/.zero/components/Combobox.ts` + `Combobox.test.ts`

**Changes:**
- `onKeyEnter` reworked:

  ```ts
  function onKeyEnter(ctx: ComboboxCtx, e: KeyboardEvent): void {
    e.preventDefault();
    const opt = ctx.options.val[ctx.highlight.val];
    const el = ctx.inputRef.el;
    if (!ctx.allowCustom) {            // default-off: today's behavior, untouched
      if (opt) pick(ctx, opt);
      return;
    }
    if (opt && el && el.value === opt.label) {  // ghost accepted / arrowed onto
      pick(ctx, opt);
      return;
    }
    commitText(ctx);
  }
  ```

  (Visible-text equality here is exact, not case-insensitive — the ghost
  always writes the option's canonical label, so equality means "the user is
  looking at the option's own label". Case-insensitive resolution still
  happens inside `commitText` step 4.)
- `onKeyTab` unchanged — its existing "pick only when visible text equals the
  highlighted label, else close" shape already matches the precedence rule;
  the close → native blur → `handleDismiss` path performs the custom commit.
- `onKeyEscape` unchanged — closes without committing; a subsequent blur
  commits (decision #3 above).

**Tests:**
- Enter on novel text with populated dropdown: loader returns options whose
  labels don't equal the typed text and don't prefix-match (so no ghost,
  e.g. type `"Xyz"` against the `ABC` fixture via a loader that ignores the
  query) → Enter commits `"Xyz"`, does **not** pick `options[0]`.
- Ghost-accepted Enter still picks: type `"a"` (ghost fills `"alpha"`),
  Enter → `value.val === "a1"`, `onChange` got the real option.
- ArrowDown-then-Enter still picks the arrowed option under
  `allowCustom: true` (visible text was rewritten by `moveHighlight`).
- Escape-then-blur commits: type `"Xyz"`, Escape (dropdown closes, no
  commit, `onChange` not called), blur → commit fires once.
- Tab on novel text: Tab closes without picking; subsequent blur commits.
- Default-off: existing "Enter accepts the highlight" test unmodified.

### Step 6: Documentation — components.md + api.md verify

**Goal:** User-facing reference reflects both features (spec Requirement 5).

**Files:**
- `docs/components.md`
- `docs/api.md` (verify; edit only if it enumerates per-component props)
- (`components.d.ts` was kept in sync in Steps 1–4; verify nothing drifted.)

**Changes:**
- `docs/components.md` reference table: add `autofocus` + `attrs` to the
  optional-prop summaries of `Checkbox`, `Combobox`, `Input`, `Radio`,
  `Select`, `TextArea`, `Toggle`; add `allowCustom` to the `Combobox` row.
- New prose paragraph after the `error?` paragraph (the seven-controls
  precedent it mirrors): every form control accepts `autofocus?: boolean`
  (focuses the underlying element after mount — the drawer/dialog
  focus-on-open case) and `attrs?: Record<string, string | number |
  boolean>` (additive-only native attributes: component-owned attributes
  win, `true` → empty attribute, `false` → skipped, numbers stringified);
  show a short snippet; note that for the label-wrapped controls and
  `Combobox` both props target the inner `<input>`.
- New `## Combobox` section (alongside Button / Drawer / Table sort / Forms)
  documenting `allowCustom`: commit triggers (blur, outside click, Enter on
  non-ghost text), the case-insensitive whole-label auto-pick, trimming,
  empty-clears, the synthesized `{ value: t, label: t }` `onChange` shape and
  how callers detect a custom commit, Escape-cancels-dropdown-not-text, and
  the strict-revert default when the flag is off.
- `docs/api.md`: check whether it lists per-component props; if yes, add the
  new ones; if it only lists exports, no change.

**Tests:** None (docs). `zero lint` clean if it covers docs examples;
otherwise a read-through.

### Step 7: Propagate to showcase/examples, full suite incl. slow tests

**Goal:** Showcase and `examples/*/web` carry synced copies of the scaffold
components; the slow integration tests pin them. This step makes the whole
workspace coherent.

**Files:** `showcase/` and `examples/*/web` component copies (via
`zero update --yes` in each, per the established in-repo workflow — no
scratch dirs).

**Changes:**
- Run the update flow in `showcase/` and each example so the `.zero/`
  component copies match the scaffold.
- No showcase route changes required by the spec; leave
  `showcase/src/routes/{input,combobox}.ts` as-is unless the update flow
  surfaces a break.

**Tests:**
- `cargo test --workspace -- --include-ignored` — the full suite including
  `showcase_*`, `examples_*`, `component_library`, `build_full`,
  `lint_examples`.
- `cargo run -p zero -- test` from repo root for the JS runtime suite
  (should be untouched — no runtime files change in this item; a clean run
  proves it).

### Step 8: Friction-log close-out in zero_demo (#74, #75)

**Goal:** Spec Requirement 6 — the demo adopts the features and the friction
log records the fixes.

**Files:** `../zero_demo/FRAMEWORK_NOTES.md`; demo sources (after
`zero update` there).

**Changes:**
- In the demo: `zero update` to pull the new components; replace the two raw
  focus-`<input>`s with `Input` + `autofocus` (the code comments mark them);
  replace the Add Part category `<input list>` + `<datalist>` hand-roll with
  `Combobox({ allowCustom: true, … })`.
- Run the demo's own suite (`zero test`, `zero lint`) to confirm adoption.
- Flip #74 and #75 to `- [x]` with `**FIXED 2026-06-XX**` annotations per the
  log's format (one sentence each on what shipped).

**Tests:** The demo's suite green after adoption; manual check that
focus-on-open still works in the drawer forms via the demo's existing tests
(or a small added assertion using `document.activeElement`).

## Risks and Assumptions

- **Unconditional internal ref binding is inert.** Verified in
  `template.js`: `_commitRef` assigns `.el` and registers one disposal
  effect; no markup or event-path change. The existing base-markup tests
  double as the regression guard.
- **Post-mount microtask timing.** `applyNative` runs after the current
  task; if a test (or app) tears the component down in the same task, the
  null-ref bail covers it. Tests must `await Promise.resolve()` / `wait(0)`
  before asserting — easy to forget, and a forgotten await looks like a
  real failure.
- **dom-shim `focus()` dispatches a `focus` event.** For Combobox this runs
  `handleFocus`, which can reopen the dropdown when resolved options exist.
  Step 3's test covers the empty case; demo adoption (Step 8) is where a
  surprise would surface — watch for dropdown-reopen-on-programmatic-focus
  and, if it bites, note it in the Combobox docs (it is browser-accurate
  behavior).
- **The idempotence guard keys on `lastLabel` alone.** If the parent
  externally rewrites `value` without `lastLabel` knowing (legal — value is
  caller-owned), a blur with visible text equal to `lastLabel` will not
  re-commit. This matches the strict-mode behavior (blur never writes) and
  is judged acceptable; flagged here so the executor doesn't "fix" it ad hoc.
- **`fireInput` test helper writes `el.value` directly** — fine for
  simulating typing, but `allowCustom` tests must remember the trailing
  ghost state: after a prefix-matching type, `el.value` is the full ghost
  label, so blur in that state near-match-picks the ghosted option. Tests
  that want novel-text commits must use non-matching text or backspace the
  ghost first (the existing backspace test shows how).
- **Assumption: `zero update` cleanly propagates component copies to
  showcase/examples** (it has for every prior component item). If an example
  pins old component text in a golden test, `--include-ignored` will surface
  it in Step 7.
- **`docs/api.md` shape unknown** until Step 6 opens it; the step includes
  the conditional so no replanning is needed either way.
