# Plan: dom-shim `<select>` — full HTMLSelectElement surface

## Summary

Give the dom-shim's `<select>` and `<option>` elements a browser-faithful
state model so tests can assert which option a select currently shows. The
generic attribute-coupled props stay exactly as they are for every other tag;
two new attach helpers (`_attachSelectProps`, `_attachOptionProps`) redefine
the select/option-specific surface on top (legal because every existing
defineProperty is `configurable: true`). The `selected` **attribute is
treated as current state** (no default-vs-dirty model), which is what the
template's attribute-committing reactive bindings already assume. The shipped
scaffold `Select.test.ts` is then strengthened to assert real selection, and
`docs/testing.md` documents the model.

Correction to the spec's Background carried into this plan:
`runtime/dom-shim.test.js` is **not** a `node:test` file — it imports from
`'zero/test'` and runs under `zero test` like the rest of the runtime suite
(`cargo run -p zero -- test dom-shim.test.js`).

## Prerequisites

None blocking. The spec's open questions are resolved as follows:

- **`option.index`**: include (falls out of the options walk). **`option.text`**:
  skip — `textContent` serves.
- **Hook location**: a tag check at the tail of `_attachElementProps`
  (dom-shim.js ~line 1017), which already receives every element from
  `createElement` (incl. clones via `_cloneNode` and
  `document.createElement`).
- **Orphan options** (no `<select>` ancestor): `selected` getter falls back to
  attribute presence, setter touches only itself, `index` reads 0.
- **Demo strict-setter audit**: land-time task in `zero_demo`, not part of
  this repo's steps (see Step 6).

## Steps

- [x] **Step 1: Select-side helpers and `_attachSelectProps`**
- [x] **Step 2: Option-side surface and mutual exclusivity**
- [x] **Step 3: End-to-end reactive + event tests in template.test.js**
- [x] **Step 4: Strengthen scaffold `Select.test.ts`**
- [x] **Step 5: Documentation (`docs/testing.md`, verify `docs/components.md`)**
- [x] **Step 6: Full verification sweep**

---

## Step Details

### Step 1: Select-side helpers and `_attachSelectProps`

**Goal:** The derived select surface — `value`, `selectedIndex`, `options`,
`selectedOptions`, `multiple` — exists and is observable, before any
option-side behavior changes. Pure additive layer; nothing existing moves.

**Files:**
- `runtime/dom-shim.js`
- `runtime/dom-shim.test.js`

**Changes:** Add module-level `@internal` helpers (each well under 80 lines,
full JSDoc), placed near `_attachInputProps`:

```js
function _optionsOf(select)            // OPTION descendants in document order
                                       // via _walkDescendants (includes <optgroup> children)
function _optionValue(option)          // value attribute ?? textContent (browser fallback)
function _selectedIndexOf(select)      // index of first option with a `selected`
                                       // attribute; when none: 0 if (!multiple && options
                                       // .length > 0) — the browser default — else -1
function _setSelectedIndex(select, i)  // remove `selected` attr from every option;
                                       // setAttribute('selected','') on options[i]
                                       // when 0 <= i < length (out-of-range / -1 = clear)
function _attachSelectProps(el)        // Object.defineProperty group, see below
```

`_attachSelectProps(el)` redefines on top of the generic input props
(all `configurable: true, enumerable: true`):

- `value` get: `i = _selectedIndexOf(el); i === -1 ? "" : _optionValue(_optionsOf(el)[i])`.
- `value` set (strict): find first option where `_optionValue(o) === String(v)`;
  `_setSelectedIndex(el, foundIndex)` — i.e. **no match clears selection**.
  Never writes a `value` attribute on the select.
- `selectedIndex` get: `_selectedIndexOf(el)`. Set: `_setSelectedIndex(el, +v)`
  (non-finite coerces to -1 → clear).
- `options` get: `_optionsOf(el)` (plain array, recomputed per access).
- `selectedOptions` get: options whose `selected` attribute is present; for a
  non-`multiple` select with none marked but options present, `[options[0]]`
  (consistent with the default rule).
- `multiple`: `_defineBoolAttrProp(el, "multiple", "multiple")`.

Wire-up: at the tail of `_attachElementProps(el)` add
`if (el.tagName === 'SELECT') _attachSelectProps(el);` (the `OPTION` branch
lands in Step 2).

No events are dispatched anywhere in these paths (spec R5 is satisfied by
construction; asserted in Step 3).

**Tests:** New `describe('select element model', …)` in
`runtime/dom-shim.test.js` (render static templates with `html`):

- Derived value from a `selected`-marked option; `selectedIndex` matches.
- First-option default: non-`multiple`, nothing marked → `value` = first
  option's value, `selectedIndex === 0`.
- Empty select → `value === ""`, `selectedIndex === -1`.
- `multiple` with nothing marked → `selectedIndex === -1`, `value === ""`
  (no default).
- `value` setter, match → that option gains the `selected` attribute, all
  others lose it; `value` reads back.
- `value` setter, no match → `selectedIndex === -1`, `value === ""`; **no**
  `value` attribute appears on the select (`hasAttribute('value')` false).
- `selectedIndex` setter: in-range selects (others cleared); `-1` and
  out-of-range clear.
- `options` / `selectedOptions` across `<optgroup>` nesting, document order.
- `multiple` with two marked options → `selectedOptions` has both, `value`
  reads the first.
- Regression guard: an `<input>`'s `value` keeps today's attribute-coupled
  read/write behavior (set arbitrary string, read it back).

Run: `cargo run -p zero -- test dom-shim.test.js`, then the full
`cargo run -p zero -- test` to catch collateral.

### Step 2: Option-side surface and mutual exclusivity

**Goal:** `option.selected` means *current* selectedness (incl. the
default-first rule), writing it enforces single-select exclusivity, and
`option.value` gains the text-content fallback — completing spec R2/R3.

**Files:**
- `runtime/dom-shim.js`
- `runtime/dom-shim.test.js`

**Changes:**

```js
function _ownerSelect(option)       // parentNode walk (through OPTGROUP) to the
                                    // nearest SELECT ancestor, else null
function _attachOptionProps(el)     // Object.defineProperty group, see below
```

`_attachOptionProps(el)` redefines:

- `selected` get: `true` when the `selected` attribute is present; else, when
  `_ownerSelect(el)` exists, is non-`multiple`, has **no** option marked, and
  `el` is its first option → `true` (default rule, consistent with Step 1's
  `_selectedIndexOf`); else `false`. Orphan option → attribute presence only.
- `selected` set: truthy → if owner select exists and is non-`multiple`,
  remove the `selected` attribute from every other option of that select,
  then set own attribute; under `multiple` (or orphan) set own attribute
  only. Falsy → remove own attribute only.
- `value` get: `value` attribute when present, else `textContent`. Set:
  writes the attribute (unchanged from generic, restated for the new getter).
- `index` get: position within `_ownerSelect(el)`'s options (optgroup-aware);
  orphan → 0.

Wire-up: extend the Step 1 tag check —
`else if (el.tagName === 'OPTION') _attachOptionProps(el);`.

**Tests:** Extend the Step 1 describe block (or a sibling
`describe('option element model', …)`):

- `option.value` text fallback: `<option>Two</option>` reads `"Two"`; with a
  `value` attribute the attribute wins.
- `option.selected` getter default-first: nothing marked → `options[0].selected`
  is `true`, the rest `false`; under `multiple` all read `false`.
- Exclusivity: setting `selected = true` on option B removes the attribute
  from previously-marked A (assert both attribute and property);
  `select.value` now reads B.
- `multiple` independence: marking B leaves A's attribute intact.
- `selected = false` on the marked option of a non-`multiple` select reverts
  reads to the default first option.
- Orphan option (rendered outside any select): getter tracks only the
  attribute; setter affects only itself; `index === 0`.
- `option.index` across `<optgroup>` (document-order position in the owning
  select).

### Step 3: End-to-end reactive + event tests in template.test.js

**Goal:** Prove the new model composes with the template's reactive
`selected=${fn}` bindings without fights or stale state (spec R4), that
hand-driven test setup works without fake event targets, and that
programmatic writes dispatch nothing (spec R5).

**Files:**
- `runtime/template.test.js`

**Changes:** New `describe('select reactive selection', …)`:

- **Select-component shape**: build options via
  `[{value:'a',…},{value:'b',…}].map(o => html`<option value=${o.value}
  selected=${() => sig.val === o.value}>${o.label}</option>`)`, render inside
  `<select @change=${handler}>${options}</select>` with `sig = signal('b')`.
  Assert `select.value === 'b'`, `selectedOptions` length 1; `sig.set('a')`;
  re-assert `select.value === 'a'` and that b's attribute is gone.
- **Hand-write then signal**: `select.value = 'b'` by hand (exclusivity
  clears a), then `sig.set('a')` — bindings re-assert and reads are
  consistent (no loop, no stale attribute).
- **Real-element change event**: set `select.value = 'b'`, then
  `fire(select, 'change')` with **no** data bag; the `@change` handler reads
  `e.target.value === 'b'`.
- **No synthetic events**: attach a `@change` spy; perform
  `select.value = …`, `select.selectedIndex = …`, and
  `option.selected = true`; spy call count stays 0.

**Tests:** This step *is* tests. Run the full runtime suite
(`cargo run -p zero -- test`) — also watches for existing tests broken by the
strict setter or the default-first rule (see Risks).

### Step 4: Strengthen scaffold `Select.test.ts`

**Goal:** The shipped component's test asserts rendered selection and drives
the real element — closing the "quietly never asserts selection" gap (spec
R8). The component itself (`Select.ts`) needs **no** change.

**Files:**
- `crates/zero-scaffold/src/scaffold/.zero/components/Select.test.ts`

(Showcase and `examples/*/web` `.zero/` trees are gitignored and materialized
by `zero update` from this scaffold source — no manual sync; the slow
`showcase_*` / `examples_*` integration tests verify in Step 6.)

**Changes:**

- New test "renders the signal-selected option": value `signal("b")`, two
  options; `const sel = find(el, "select") as HTMLSelectElement;`
  `expect(sel.value).toBe("b")`; `expect(sel.selectedOptions.length).toBe(1)`.
- New test "selection tracks signal changes": `value.set("a")` →
  `expect(sel.value).toBe("a")`.
- Rework "updates its signal on change events", "honours debounceMs", and
  "invokes onChange …": replace
  `fire(select, "change", { target: { value: "b" } })` with
  `sel.value = "b"; fire(sel, "change");` — the handler now reads the derived
  `e.target.value`. Assertions unchanged.

TypeScript note: `find` returns `Element`; cast to `HTMLSelectElement` where
select members are read (shim's `selectedOptions` is a plain array —
`.length` and `Array.from` both typecheck against the DOM lib type).

**Tests:** Iterate via the in-repo showcase per the established workflow:
build the CLI, run `zero update --yes` in `showcase/`, then `zero test`
there (the scaffold TDD loop). All existing Select tests must stay green with
the reworked event pattern.

### Step 5: Documentation

**Goal:** The user-facing reference describes the new form-element model
(spec R9).

**Files:**
- `docs/testing.md`
- `docs/components.md` (verify-only)

**Changes:**

- `docs/testing.md`, § DOM helpers, after the selector-support paragraph
  (~line 130): new **Form elements** subsection covering — `select.value`
  derives from the selected option (first-option default when nothing is
  marked on a non-`multiple` select); strict setter (no matching option
  clears selection, reads `""`); `selectedIndex`, `options`,
  `selectedOptions` (plain arrays, recomputed per access); `multiple`;
  `option.value` text-content fallback; programmatic writes never dispatch
  `change`/`input` (set state, then `fire(select, "change")`); and the
  documented divergence — the `selected` attribute *is* current state, no
  `defaultSelected`/dirtiness model. Include a short before/after snippet
  showing the real-element pattern replacing the fake-target pattern.
- `docs/components.md`: grep confirmed no fake-target example exists in the
  Select section; re-verify after Step 4 and change nothing unless one is
  found.

**Tests:** None (prose). Sanity-read; per the avoid-overvalidating rule, no
doc build.

### Step 6: Full verification sweep

**Goal:** Everything green, including the slow integration tests that
materialize showcase/examples from the scaffold; quality gates met.

**Files:** None new (fixes only if the sweep finds breakage).

**Changes / commands:**

- `cargo test --workspace -- --include-ignored` — includes `showcase_*`,
  `examples_*`, `e2e_init_*`, `build_full`, `lint_examples`, which prove the
  reworked `Select.test.ts` passes in materialized projects.
- `cargo run -p zero -- test` — full runtime JS suite.
- Glance at `cargo llvm-cov --workspace --summary-only` for outliers in
  touched crates (none expected — changes are JS-side).
- **Land-time, in `zero_demo` (separate repo, not this build):** flip
  friction entry #77 to `- [x]` with a `**FIXED YYYY-MM-DD**` annotation that
  also corrects the headline (selected-attribute reflection already worked;
  the fix is the HTMLSelectElement surface); re-run the select-mirror
  `lit_str` mutant to confirm it's killable; audit demo tests that hand-set
  `select.value` for strict-setter breakage.

**Tests:** The sweep itself.

## Risks and Assumptions

- **Strict setter / default-first rule may break existing tests.** Any
  runtime, showcase, or example test that hand-sets `select.value` to an
  unmatched string and reads it back, or that assumed "nothing selected"
  reads on an unmarked select, will surface in Steps 3–6. A repo grep during
  Step 1 (`select.value`, `\.selected` in `*.test.*`) scopes this early;
  known instance: scaffold `Select.test.ts`'s fake-target pattern, reworked
  in Step 4. Combobox tests use an input + button list (not native options),
  so they should be untouched — verified by the full suite.
- **`fire` semantics**: `fire(el, type)` constructs the event and
  `dispatchEvent` sets `target` to the element, so the real-element pattern
  needs no test.js change. If `Object.assign(ev, data)` with an empty bag
  ever interfered with `target`, Step 3's test would catch it.
- **Recompute-per-access collections** assume test-scale DOM sizes; no
  caching/invalidaton is attempted. If a pathological suite shows cost, a
  follow-up can memoize on childNodes mutation — out of scope here.
- **Attribute-as-current-state divergence** is assumed acceptable for test
  code (decided in the spec); anything needing browser dirtiness semantics is
  explicitly unsupported and documented.
- **Embedded parity**: `crates/zero-runtime/src/lib.rs` embeds
  `runtime/dom-shim.js` verbatim; the grown body needs no Rust edit. If that
  embedding ever becomes size-gated, Step 1 would surface it immediately.
- **TS casts in scaffold tests** assume the user-project tsconfig includes the
  DOM lib (it does today — existing tests reference `HTMLSelectElement` in
  `Select.ts` props).
