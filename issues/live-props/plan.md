# Plan: Live form properties — bind `value`/`checked`/`selected` as DOM properties

## Summary

Make `value=${…}` / `checked=${…}` / `selected=${…}` bindings write the live DOM
**property** instead of the content attribute, so programmatic state actually
reaches the field in a real browser. The runtime change is one shared helper in
`runtime/template.js`, routed through by both commit paths (single `_applyAttr`
and joined `_setJoinedAttr`), with the caret-preserving `el.value !== next`
guard. Separately, the in-memory DOM shim (`runtime/dom-shim.js`) is taught the
real-browser default-vs-live split for **input/textarea `value`** and **input
`checked`**, so a genuine fail-before/pass-after regression test exists for the
case its old coupling used to mask. Existing tests that asserted the old
coupling are updated as deliberate fallout, and the templates/testing docs are
corrected.

## Prerequisites

- **Spec Requirement 3 refinement — needs user sign-off (flagged in Step 2).**
  The spec lists `selected` alongside `value`/`checked` for the shim's
  default-vs-live model. This plan intentionally does **not** decouple
  `<option>.selected` from its `selected` attribute: the shim's entire select
  machinery (`_selectedIndexOf`, `_setSelectedIndex`, `selectedOptions`,
  dom-shim.js:1046–1121) reads selectedness from the attribute, and diverging
  the property would break it with no benefit. The runtime fix still routes
  `selected=${…}` to the property (whose setter already writes the attribute and
  syncs siblings), and `<select value=${sig}>` is the real regression anchor for
  the programmatic-selection concern. `docs/testing.md`'s existing statement
  that option `selected` has "no defaultSelected / dirtiness model" therefore
  stays true. If the user wants full option-selected divergence, that is a
  larger select-model rework and should be re-scoped.
- Spec open question on the `checked` / `"false"` string coercion is resolved in
  Step 1 (kept as defensive coercion; see details).
- No dependency on other roadmap items.

## Steps

- [x] **Step 1: Runtime — shared live-property helper, both commit paths, and behavior tests**
- [x] **Step 2: DOM shim — default-vs-live split for input/textarea `value` and input `checked`, plus fallout fixes and shim-model tests**
- [x] **Step 3: Documentation — `docs/templates.md`, `docs/testing.md`, `docs/api.md`**

---

## Step Details

### Step 1: Runtime — shared live-property helper, both commit paths, and behavior tests

**Goal:** Make dynamic `value`/`checked`/`selected` bindings set the DOM
property. This is the user-facing fix and is safe to land first: under the
current (still-coupled) shim the property writes also reflect to attributes, so
the whole suite stays green, while `<select value=${sig}>` flips from broken to
working immediately.

**Files:**
- `runtime/template.js` (modify)
- `runtime/template.test.js` (add tests)

**Changes:**
- Add one `@internal` helper to `runtime/template.js`, above `_applyAttr`
  (line 379):

  ```js
  /**
   * If `(el, name)` is a live form property (value on input/textarea/select,
   * checked on input, selected on option), set the DOM property instead of the
   * content attribute and return true; otherwise return false so the caller
   * falls back to attribute handling. The content attribute is only the
   * *default*; browsers track the shown/checked/selected state on the property.
   * @internal
   * @param {Element} el
   * @param {string} name
   * @param {unknown} v
   * @returns {boolean}
   */
  function _applyLiveProp(el, name, v) {
    const tag = el.tagName;
    if (name === 'value' && (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT')) {
      const next = v == null ? '' : String(v);
      if (el.value !== next) el.value = next; // guard: don't clobber caret
      return true;
    }
    if (name === 'checked' && tag === 'INPUT') { el.checked = !!v && v !== 'false'; return true; }
    if (name === 'selected' && tag === 'OPTION') { el.selected = !!v && v !== 'false'; return true; }
    return false;
  }
  ```

  The `v !== 'false'` guard on the booleans is deliberate defensive coercion for
  the joined path, where a concatenated value arrives as the string `"false"`;
  in the common single-placeholder case `v` is already a boolean and `!!v`
  governs. Keep it.
- `_applyAttr` (line 379): first line becomes `if (_applyLiveProp(el, name, v)) return;`,
  leaving the existing `removeAttribute`/`setAttribute(true→'')`/`setAttribute(String(v))`
  logic unchanged below it.
- `_setJoinedAttr` (line 443): after building the concatenated `out` string,
  add `if (_applyLiveProp(el, name, out)) return;` before the final
  `el.setAttribute(name, out)`. This fixes `value="draft-${id}"` the same as the
  single-binding path.
- No change to `_commitAttrSingle`/`_commitAttrJoined`/effect wiring — the
  property writes already run inside the existing `effect`, so they re-run on
  signal change exactly like attribute writes.

**Tests:** New `describe('live form property bindings', …)` block in
`runtime/template.test.js` (place after the existing `select reactive selection`
block, ~line 783). All pass after this step in the current shim; after Step 2
the input-value/checked cases become genuine regression guards.
- `<input value=${sig}>`: initial `input.value` equals the signal; `sig.set('x')`
  updates `input.value` to `'x'`.
- `<input type="checkbox" checked=${sig}>`: toggling the boolean signal updates
  `input.checked`.
- `<select value=${sig}>` **anchor** (fails before this step): given options
  a/b/c, initial `sel.value` matches the signal and `sig.set('c')` selects
  option c (`sel.value === 'c'`, `sel.selectedOptions` is `[c]`).
- `<option selected=${sig()}>` single-option case: toggling updates
  `option.selected`.
- Joined `<input value="draft-${id}">`: `id.set(2)` updates `input.value` to
  `'draft-2'`.
- Confirm the existing `select reactive selection` tests (template.test.js:715)
  still pass unchanged (per-option `selected=${…}` now routes through the
  property setter; behavior converges).

---

### Step 2: DOM shim — default-vs-live split for input/textarea `value` and input `checked`, plus fallout fixes and shim-model tests

**Goal:** Make the fake DOM behave like a real browser for the properties the
runtime now sets, so `setAttribute` no longer masks the bug the fix addresses.
This is where the input-value regression guard becomes real. It is a deliberate
behavior change, so the two existing coupling assertions are updated in the same
step to keep every suite green.

**Files:**
- `runtime/dom-shim.js` (modify)
- `runtime/dom-shim.test.js` (rewrite one test, add shim-model tests)
- `crates/zero-scaffold/src/scaffold/.zero/components/form.test.ts` (update assertion)
- `showcase/.zero/components/form.test.ts`,
  `examples/{counter,todos,tracker}/web/.zero/components/form.test.ts`
  (propagated copies — same one-line change)

**Changes:**
- Add two `@internal` helpers to `runtime/dom-shim.js` near `_defineStringAttrProp`
  (line 853):

  ```js
  // Live input value: property returns the live value once set, else the
  // `value` attribute (the default). setAttribute updates only the default —
  // a late setAttribute cannot change what `.value` returns after a set,
  // matching a real browser's dirty-value flag.
  function _defineLiveValueProp(el) {
    Object.defineProperty(el, 'value', {
      get() { return el._liveValue !== undefined ? el._liveValue : (_getAttribute(el, 'value') ?? ''); },
      set(v) {
        el._liveValue = v == null ? '' : String(v);
        const len = el._liveValue.length; // browser moves caret to end on assignment
        el._selStart = len;
        el._selEnd = len;
      },
      configurable: true,
      enumerable: true,
    });
  }

  function _defineLiveCheckedProp(el) {
    Object.defineProperty(el, 'checked', {
      get() { return el._liveChecked !== undefined ? el._liveChecked : _hasAttribute(el, 'checked'); },
      set(v) { el._liveChecked = !!v; },
      configurable: true,
      enumerable: true,
    });
  }
  ```

- In `_attachInputProps` (line 953), replace the unconditional
  `_defineStringAttrProp(el, 'value', 'value')` and
  `_defineBoolAttrProp(el, 'checked', 'checked')` with tag-scoped branches:

  ```js
  if (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA') _defineLiveValueProp(el);
  else _defineStringAttrProp(el, 'value', 'value');
  if (el.tagName === 'INPUT') _defineLiveCheckedProp(el);
  else _defineBoolAttrProp(el, 'checked', 'checked');
  ```

  `_attachInputSelection(el)` (which initializes `el._selStart/_selEnd = 0`) is
  called later in the same function (line 962), so the value setter's caret
  reset is safe. `_attachSelectProps`/`_attachOptionProps` run *after*
  `_attachInputProps` in `_attachElementProps` (line 1206–1208) and re-`Object.defineProperty`
  the select/option `value` and option `selected` — so those overrides are
  preserved and non-form elements (`<progress>`, `<li>`, `<meter>`, `<div>`)
  keep the attribute-coupled `value`/`checked`.
- **Scope note:** do **not** touch `<option>.selected` (dom-shim.js:1155) — see
  Prerequisites. It stays attribute-backed so the select machinery is intact.

**Fallout fixes (same step, inseparable from the decoupling):**
- `runtime/dom-shim.test.js:511–517` — the test named *"input value keeps the
  generic attribute-coupled behavior"* now asserts the wrong (old) model.
  Rewrite it to the browser model: a static `<input value="start">` reads
  `input.value === 'start'` (default from attribute); after `input.value = 'typed'`,
  `input.value === 'typed'` but `input.getAttribute('value') === 'start'`; a
  subsequent `input.setAttribute('value', 'later')` leaves `input.value === 'typed'`.
- `form.test.ts:375` (*"binds the façade value signal through Input"*) — the
  `Input` binds `value=${sig}` dynamically (no static attribute), so after
  decoupling `input.getAttribute('value')` is `null`. Change the assertion from
  `expect(input.getAttribute('value')).toBe('abc')` to
  `expect(input.value).toBe('abc')`. Apply the identical change to all **five**
  copies (scaffold canonical + `showcase` + the three `examples/*/web`).
  Propagate via the established workflow: edit the scaffold source, then rebuild/
  install the CLI (`cargo install --path crates/zero --locked`) and run
  `zero update --yes` in `showcase` and each example to sync — or edit the four
  non-scaffold copies directly if faster; both leave identical bytes.
- **Broader audit:** grep `runtime/**` and
  `crates/zero-scaffold/src/scaffold/.zero/components/**` (and the
  showcase/examples copies) for `getAttribute("value")`, `getAttribute("checked")`,
  `setAttribute("value"`, `setAttribute("checked"`, and `.value =`/`.checked =`
  followed by an attribute read, to catch any other coupling assumption. Fix
  each; none may be left silently changed.

**Tests:**
- `runtime/dom-shim.test.js` — the rewritten value test above, plus a new
  `checked` analog: a static `<input checked>` reads `.checked === true`;
  `input.checked = false` then reading `.checked === false` while
  `input.hasAttribute('checked')` is still `true` (default unchanged); a late
  `setAttribute('checked','')` does not flip `.checked` back.
- `runtime/template.test.js` — add the **caret guard** case to the Step-1
  block: render `<input value=${sig}>` with `sig = signal('')`; simulate native
  typing with `input.value = 'ab'` then `input.setSelectionRange(1, 1)`; call
  `sig.set('ab')` so the binding effect re-runs with a value equal to the DOM's;
  assert `input.selectionStart === 1` (guard skipped the assignment). Without
  the guard the shim's value setter would have reset the caret to `2`.
- Run `cargo run -p zero -- test` (JS runtime) and
  `cargo test --workspace -- --include-ignored` (Rust incl.
  `component_library`, `showcase_*`, `examples_*`) — all green after the
  propagation and audit.

---

### Step 3: Documentation — `docs/templates.md`, `docs/testing.md`, `docs/api.md`

**Goal:** Bring the user-facing docs in line with the now-correct behavior. The
spec flagged `docs/templates.md` as overstating today's behavior.

**Files:**
- `docs/templates.md` (modify)
- `docs/testing.md` (modify)
- `docs/api.md` (verify only)

**Changes:**
- `docs/templates.md` (~line 80): the `html\`<input value=${name} />\` // signal
  binds value` line and surrounding prose. State accurately that
  `value`/`checked`/`selected` bind to the **live DOM property**, so programmatic
  state updates are reflected in the field (previously a late attribute write was
  ignored by the browser). Add a short sentence that this is a one-way
  state→field binding — writing user input back into state still needs an
  `@input`/`@change` handler (the existing `@input` example ~line 123 remains the
  two-way idiom). Keep it tight; no new section required.
- `docs/testing.md` (DOM-model section, ~line 139–175): add a short bullet group
  for input/textarea `value` and input `checked` describing the new
  default-vs-live model — the content attribute is the default, the property is
  the live value, and a `setAttribute` after a property set does not change the
  property (mirrors the browser). **Leave** the existing select/option paragraph
  (lines 139–175), including "no defaultSelected / dirtiness model" for option
  `selected`, unchanged — it stays true under this plan.
- `docs/api.md`: no export surface changes; scan for any stale claim about
  attribute binding of form values and correct if present, otherwise no change
  (note in the execute log that api.md was reviewed).

**Tests:** Docs-only; covered by the repo's existing docs/link checks if any run
under the workspace suite. No new tests.

## Risks and Assumptions

- **Option-selected scope deviation.** The plan narrows spec Requirement 3's
  shim divergence to `value`/`checked` and leaves `<option>.selected`
  attribute-backed (Prerequisites). If the user insists on modeling option
  divergence, Step 2 grows into a select-model rework (rewire `_selectedIndexOf`
  et al. to a live `_selected`), and the existing `select reactive selection`
  and `select element model` tests must be re-audited. Confirm before executing.
- **Effect ordering with the option `selected` property setter.** The setter
  clears sibling `selected` attributes when set truthy; per-option reactive
  bindings all depend on the same signal and re-run in registration order, so
  the state converges (verified against template.test.js:737–761). If a future
  binding shape re-runs effects out of order this assumption could wobble; the
  existing tests guard it.
- **Caret reset in the shim value setter.** Setting `.value` now moves
  `selectionStart/End` to the end (browser-faithful). The existing
  `setSelectionRange` test (dom-shim.test.js:215) sets `.value` *before*
  `setSelectionRange`, so it is unaffected; the audit must confirm no other test
  sets `.value` after positioning the caret and then asserts the old position.
  Combobox ghost completion sets `el.value` then `setSelectionRange`, so it
  round-trips correctly.
- **Propagation completeness.** The `form.test.ts` assertion exists in five
  copies; missing any one fails a slow integration test. The
  `--include-ignored` run is the backstop and must be executed, not assumed.
- **`tagName` case.** The helper compares against uppercase (`INPUT`/`TEXTAREA`/
  `SELECT`/`OPTION`); the shim uppercases `tagName` (dom-shim.js:1239) and real
  DOM HTML elements are uppercase, so the comparison holds in both environments.
