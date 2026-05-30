# Plan: Button `type` / `form` / `name` / `value` props

## Summary

Add the native submit-button surface to `zero/components`' `Button`:
`type` (default `"button"`), `form`, `name`, and `value`, plus make
`loading` gate activation (set the native `disabled` attribute and
short-circuit the click handler). This is an edit-in-place change to one
existing component — no new files, no `framework_manifest()` change. The
canonical source under `crates/zero-scaffold/src/scaffold/.zero/` is
edited first (component, test, type declaration, index re-export), then
docs and the showcase route, then the three git-tracked downstream
`.zero/` copies (`showcase/`, `examples/counter/web/`,
`examples/tracker/web/`) are regenerated so `zero update --yes` shows
no drift and the integration suite passes.

Two runtime facts were verified up front and remove the spec's open
questions:

- **`_applyAttr` (`runtime/template.js:379–382`)** removes an attribute
  when the bound value is `false`/`null`/`undefined`, sets it to `""`
  for `true`, and otherwise `String(value)`s it. So `form=${props.form}`
  (etc.) cleanly omits the attribute when the prop is `undefined`, and
  `type=${props.type ?? "button"}` always emits. No conditional-markup
  gymnastics needed.
- **The dom-shim supports `getAttribute` / `hasAttribute`**
  (`runtime/dom-shim.js:1064–1065`; absent → `getAttribute` returns
  `null`, boolean-true attr → `""`). The new tests use `getAttribute`,
  matching the existing assertion style in `Table.test.ts` /
  `Spinner.test.ts`.

## Prerequisites

None. All three spec open questions are resolved:
- `undefined` string-attribute binding → confirmed omits the attribute
  (`template.js:380`).
- `ButtonType` alias → introduced and exported (decided here).
- dom-shim accessor → `getAttribute` (returns `null` for absent).
- Committed `.zero/` copies → enumerated below (showcase + counter +
  tracker; the `crates/zero/tests/fixtures/js_agent_failures` fixture is
  excluded).

## Steps

- [x] **Step 1: Extend the canonical `Button.ts` (props, defaults, loading-gates-activation, attributes, JSDoc)**
- [x] **Step 2: Extend the canonical `Button.test.ts` with the new cases**
- [x] **Step 3: Update the type declaration and index re-export (`components.d.ts`, `index.ts`)**
- [x] **Step 4: Update docs (`docs/components.md`, `docs/api.md`)**
- [x] **Step 5: Add a submit-form demo to the showcase Button route**
- [x] **Step 6: Regenerate the tracked `.zero/` copies, verify zero drift, run the full suite**

---

## Step Details

### Step 1: Extend the canonical `Button.ts`
**Goal:** Add the four new props and the loading-gates-activation
behavior to the single source of truth. This is the core change; every
later step mirrors or documents it.

**Files:**
- `crates/zero-scaffold/src/scaffold/.zero/components/Button.ts`

**Changes:**
- Add a new exported type alias:
  `export type ButtonType = "button" | "submit" | "reset";`
- Extend `ButtonProps` (place after `size`, before `disabled`, to group
  the native attribute props together):
  ```ts
  type?: ButtonType;
  form?: string;
  name?: string;
  value?: string;
  ```
- In the component body:
  - `const type: ButtonType = props.type ?? "button";`
  - Compute a single inactivity flag:
    `const inactive = (props.disabled ?? false) || (props.loading ?? false);`
  - Change the click-handler guard from `if (props.disabled) return;`
    to `if (inactive) return;`.
  - Change the rendered element from
    ```ts
    html`<button class=${cls} disabled=${props.disabled ?? false} @click=${handler}>${spinner}${props.children ?? ""}</button>`
    ```
    to
    ```ts
    html`<button class=${cls} type=${type} form=${props.form} name=${props.name} value=${props.value} disabled=${inactive} @click=${handler}>${spinner}${props.children ?? ""}</button>`
    ```
    - `type` is always present (default `"button"`).
    - `form`/`name`/`value` are full-attribute bindings; `undefined`
      values are removed by `_applyAttr` (verified `template.js:380`), so
      omitted props emit no attribute. Full-attribute interpolation is
      the supported form (partial-string `class=`-style interpolation was
      the broken case in FRAMEWORK_NOTES #32, not this).
    - `disabled=${inactive}` sets the attribute (`""`) when disabled OR
      loading, removes it otherwise — matching platform boolean-attr
      behavior and blocking native form submit while loading.
- Update the JSDoc block above the function to document: the new `type`
  (default `"button"`), `form`, `name`, `value` props; that `loading`
  now renders the spinner **and** makes the button non-interactive
  (sets `disabled`, suppresses `onClick`); keep the existing note about
  boolean-attr normalization. Maintain full JSDoc per the repo rule.
  Function stays under ~80 lines.

**Tests:** None added here (Step 2). After this step `cargo build
--workspace` compiles and the fast `cargo test --workspace` stays green
(no test consumes the canonical scaffold copy directly; downstream
copies are still the old version and remain valid).

---

### Step 2: Extend the canonical `Button.test.ts`
**Goal:** Cover the new props and the loading-gates-activation behavior
so the regenerated showcase copy exercises them under
`tests/component_library.rs`.

**Files:**
- `crates/zero-scaffold/src/scaffold/.zero/components/Button.test.ts`

**Changes:** Keep the existing three cases (base class; `onClick` fires;
`disabled` suppresses `onClick`). Add cases using the existing
`render` / `find` / `fire` / `spy` / `cleanup` imports (the `find`
result may need a `!` non-null assertion, as in the sibling component
tests):
- **Default type is `"button"`:**
  `expect(find(el, "button")!.getAttribute("type")).toBe("button")`.
- **Explicit `type`:** render with `type: "submit"` → `getAttribute("type")`
  is `"submit"`; same for `type: "reset"`.
- **`form` attribute:** `form: "edit-form"` → `getAttribute("form")` is
  `"edit-form"`; with no `form` prop,
  `expect(find(el, "button")!.getAttribute("form")).toBeNull()`.
- **`name` / `value`:** provided → matching `getAttribute` values;
  omitted → both `toBeNull()`.
- **Loading sets `disabled`:** `loading: true` →
  `expect(find(el, "button")!.getAttribute("disabled")).not.toBeNull()`
  (attribute present, value `""`).
- **Loading suppresses `onClick`:** with `loading: true` and a `spy()`
  `onClick`, `fire(find(el, "button")!, "click")` →
  `expect(onClick.callCount).toBe(0)`.
- **Loading still renders the spinner:** `loading: true` →
  `expect(find(el, ".button-spinner")).toBeTruthy()` (guards against the
  disabled coupling regressing the spinner render).

**Tests:** This step *is* the tests; they run green once the showcase
copy is regenerated in Step 6 (the canonical file is not executed
directly). To validate logic immediately, the executor may temporarily
copy the two changed files into `showcase/.zero/components/`, run
`cargo run -p zero -- test Button.test.ts` from `showcase/`, then revert
— or simply rely on Step 6's regeneration + full run.

---

### Step 3: Update the type declaration and index re-export
**Goal:** Keep editor types in sync with the source so consumers see the
new props and `ButtonType`.

**Files:**
- `crates/zero-scaffold/src/scaffold/.zero/components.d.ts`
- `crates/zero-scaffold/src/scaffold/.zero/components/index.ts`

**Changes:**
- `components.d.ts` (Button block ~lines 25–35): add
  `export type ButtonType = "button" | "submit" | "reset";` and extend
  `ButtonProps` with `type?: ButtonType; form?: string; name?: string;
  value?: string;` in the same field order as the source. The
  `Button(props?: ButtonProps): TemplateResult` declaration is unchanged.
- `index.ts` line 7: add `ButtonType` to
  `export type { ButtonProps, ButtonSize, ButtonVariant } from "./Button.ts";`
  → `export type { ButtonProps, ButtonSize, ButtonType, ButtonVariant } from "./Button.ts";`
  (alphabetical).

**Tests:** No new tests. `cargo test --workspace` stays green. If a
scaffold test asserts the *content* of these files, update its
expectation; none is expected — confirm via `cargo test -p zero-scaffold`.

---

### Step 4: Update docs
**Goal:** Document the new props, the `"button"` default, and the
loading-blocks-activation behavior; show the motivating submit-outside-
form example.

**Files:**
- `docs/components.md`
- `docs/api.md`

**Changes:**
- `docs/components.md`:
  - Update the `Button` summary-table row (~line 156) to list the new
    optional props (`type`, `form`, `name`, `value`).
  - In the Button reference/prose section, document `type` (default
    `"button"`, values `button | submit | reset`), `form`, `name`,
    `value`, and that `loading` makes the button non-interactive. Add a
    submit-button-outside-its-form example:
    ```ts
    html`
      <form id="edit-form">${/* fields */}</form>
      ${Button({ type: "submit", form: "edit-form", children: "Save" })}
    `
    ```
- `docs/api.md`: the `Button` signature row (~line 127) is unchanged
  (`Button(props?: ButtonProps)`). Add `ButtonType` to the documented
  type-alias list (~line 142, alongside `ButtonVariant`, `InputType`,
  etc.).

**Tests:** Docs only. Any docs-lint/link-check test in the workspace
must still pass; otherwise none.

---

### Step 5: Add a submit-form demo to the showcase Button route
**Goal:** Demonstrate the new capability live and give the integration
build something to render.

**Files:**
- `showcase/src/routes/button.ts`

**Changes:** Add a new `<section>` showing:
- A `<form id="showcase-edit-form">` with one or two fields and a
  `Button({ type: "submit", form: "showcase-edit-form", children: "Save" })`
  rendered *outside* the form (the motivating button-outside-form
  pattern). Wire submit (or the button's `onClick`) to a small signal so
  the demo is interactive without a network call.
- A loading submit button (`Button({ type: "submit", loading: true,
  children: "Saving…" })`) to visibly show it is non-interactive while
  loading.
- A brief explanatory paragraph in the route's existing prose style.
Keep existing sections intact; match formatting and JSDoc.

**Tests:** Exercised by `tests/showcase_build.rs` and
`tests/showcase_dev.rs` in Step 6. No unit test added.

---

### Step 6: Regenerate tracked `.zero/` copies, verify drift, run full suite
**Goal:** Bring the three git-tracked downstream copies in line with the
canonical source so `zero update --yes` is drift-free, then validate the
whole change with the slow integration suite (which executes the new
Button tests via the regenerated showcase copy).

**Files (regenerated, not hand-authored):**
- `showcase/.zero/components/Button.ts`, `Button.test.ts`,
  `components.d.ts`, `components/index.ts`
- `examples/counter/web/.zero/...` (same set)
- `examples/tracker/web/.zero/...` (same set)
- **Excluded:** `crates/zero/tests/fixtures/js_agent_failures/.zero/components/Button.ts`
  is a deliberately-broken test fixture — do **not** regenerate or edit
  it.

**Changes / procedure:**
1. Build the CLI so the manifest (built from `include_str!` of the
   canonical files edited in Steps 1–3) reflects the new content:
   `cargo build -p zero`.
2. Regenerate each project by running `cargo run -p zero -- update --yes`
   with `showcase/`, `examples/counter/web/`, `examples/tracker/web/` as
   the working directory (or the equivalent path argument the CLI takes).
3. `git diff --stat` and confirm the only changed files are the
   Button-related `.zero/` files in those three projects (plus the
   Steps 1–5 edits). Any unrelated `.zero/` rewrite indicates
   pre-existing drift — flag it, don't absorb it.
4. Run the fast suite: `cargo test --workspace`.
5. Run the slow suite: `cargo test --workspace -- --include-ignored`,
   confirming `component_library`, `showcase_build`, `showcase_dev`,
   `examples_build` / `examples_test` (if present), `lint_examples`, and
   `build_full` all pass.
6. Run `cargo run -p zero -- lint` inside `showcase/` (and optionally an
   example) to confirm the new route and component pass lint.

**Tests:** Runs the entire suite including ignored slow integration
tests. Definition of done: drift-free `git diff`, fast and slow suites
green, lint clean.

## Risks and Assumptions

- **`zero update` scope.** Assumes `zero update --yes` regenerates only
  manifest files and leaves user `src/` untouched (the documented
  contract). Pre-existing drift in a project's committed `.zero/` would
  make the Step 6 diff noisier; scope the commit to Button-related files
  and flag unrelated drift rather than absorbing it.
- **Hidden content-assertion tests.** Assumes no scaffold/integration
  test asserts an exact byte-count or full text of `Button.ts` /
  `components.d.ts` / `index.ts`, or a specific Button-test count in
  `component_library.rs`. If one exists, update its expectation in the
  same step that changes the asserted file. `framework_manifest()`
  length is **not** affected (no files added/removed).
- **`components.d.ts` presence across projects.** Confirmed all three
  projects carry `Button.ts`, `Button.test.ts`, and `components/index.ts`;
  `components.d.ts` is present in showcase and counter. If tracker (or
  any project) lacks it, `update` recreates it — still drift-converging.
- **Behavior changes are intentional.** Two output changes ship: every
  `<button>` now carries an explicit `type="button"` (previously
  absent), and `loading` buttons become non-interactive. Both agreed in
  refine. If any existing showcase/example/test relied on a loading
  button still being clickable or on the absence of a `type` attribute,
  update it to the new contract (none expected).
- **Template binder display artifact.** While inspecting
  `runtime/template.js`, tool output garbled some lines; the
  attribute-apply logic (`_applyAttr`, lines 379–382) was read cleanly
  and is intact. No runtime change is needed — the binder already
  handles `undefined` (omit) and `true`/`false` (boolean attr) exactly
  as this plan relies on.
