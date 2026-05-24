# Plan: Combobox component

## Summary

Ship a seventeenth component, `Combobox`, under `.zero/components/`. It is
a controlled, single-select typeahead: an `<input>` with inline ghost-text
completion of the first matching option, a dropdown of suggestions, and a
`loadOptions(query)` callback owned by the parent. The component owns
debouncing, min-query gating, race-safe result handling (latest-serial-
wins), keyboard navigation (ArrowUp/Down/Enter/Escape/Tab-to-complete), and
strict-revert-on-blur semantics. It exposes one stateful prop
(`value: Signal<string>`), one signal-or-plain config prop
(`disabled?: Signal<boolean> | boolean`), and plain configuration
otherwise. No HTTP awareness, no free-text mode, no multi-select.

Work is sequenced so each step leaves the workspace compiling and tests
green: (1) extend the dom-shim with the input-selection APIs the component
needs at runtime and tests need to verify; (2) author the scaffold files
(component source + partial + placeholder test) so `include_str!`
resolves; (3) wire them into the manifest, components index, aggregate
SCSS, and `components.d.ts`; (4) bump the scaffold-test and
`component_library.rs` rosters; (5) fill out the full test suite;
(6) add the `/combobox` showcase route and home-nav entry; (7) refresh
AGENTS.md and `docs/components.md` / `docs/index.md`. Every spec open
question is resolved below.

## Prerequisites

None. All decisions from the spec's **Open Questions** are resolved in
**Risks and Assumptions** below. `zero-framework-spec.md` (referenced by
Requirements 48–50) does not exist in this repo and is dropped from the
plan, mirroring the Pagination precedent.

## Steps

- [x] **Step 1: Extend the dom-shim with input selection APIs**
- [x] **Step 2: Add Combobox scaffold templates (component, partial, placeholder test)**
- [x] **Step 3: Wire Combobox into the manifest, index, aggregate SCSS, and `components.d.ts`**
- [x] **Step 4: Bump the scaffold test roster**
- [x] **Step 5: Bump the `component_library.rs` test roster**
- [x] **Step 6: Fill in `Combobox.test.ts`**
- [x] **Step 7: Add the `/combobox` showcase route and home-nav entry**
- [x] **Step 8: Refresh AGENTS.md, docs/components.md, and docs/index.md**

---

## Step Details

### Step 1: Extend the dom-shim with input selection APIs

**Goal:** The Combobox sets `input.value` and selection range when ghost
completion fires. The current dom-shim (`runtime/dom-shim.js`) defines a
string-attr `value` property but does not model `selectionStart`,
`selectionEnd`, or `setSelectionRange()`. Without them the component's
`setSelectionRange()` call throws under `zero test`, and the spec's
`selectionStart === 3, selectionEnd === 6` assertion cannot run. This
step adds the minimum surface needed.

**Files (modified):**

- `runtime/dom-shim.js`
- `runtime/dom-shim.test.js`

**Changes:**

- In `_attachInputProps(el)` (around line 787) append:

  ```js
  let _selStart = 0;
  let _selEnd = 0;
  Object.defineProperty(el, "selectionStart", {
    get() { return _selStart; },
    set(v) { _selStart = Number.isFinite(+v) ? +v : 0; },
    configurable: true,
    enumerable: true,
  });
  Object.defineProperty(el, "selectionEnd", {
    get() { return _selEnd; },
    set(v) { _selEnd = Number.isFinite(+v) ? +v : 0; },
    configurable: true,
    enumerable: true,
  });
  el.setSelectionRange = function(start, end) {
    _selStart = Number.isFinite(+start) ? +start : 0;
    _selEnd = Number.isFinite(+end) ? +end : 0;
  };
  ```

  These properties live on every element the shim creates (the shim does
  not specialize per-tag), which matches the way `value`/`checked`
  already attach universally. The Combobox is the only consumer in v1.

- The `value` setter must keep the selection bounded. When `el.value` is
  assigned to a shorter string than `_selStart`/`_selEnd`, browsers
  clamp them; the shim should too. Inside the existing
  `_defineStringAttrProp(el, "value", "value")` setter path, after the
  attribute is written, clamp `_selStart` / `_selEnd` to
  `Math.min(_selStart, value.length)` when they were defined. To keep
  the change local, do the clamping inside the new
  `setSelectionRange()` body by reading `el.value.length` (no
  cross-callback wiring needed for v1 — Combobox always re-asserts
  selection right after writing `.value`, so the clamping in browsers
  vs. shim doesn't change observable behaviour).

- `dom-shim.test.js` gains one short block:

  ```js
  describe("input selection APIs", () => {
    afterEach(cleanup);
    it("setSelectionRange writes selectionStart/End", () => {
      const el = createElement("input");
      el.value = "foobar";
      el.setSelectionRange(3, 6);
      expect(el.selectionStart).toBe(3);
      expect(el.selectionEnd).toBe(6);
    });
    it("defaults to 0/0", () => {
      const el = createElement("input");
      expect(el.selectionStart).toBe(0);
      expect(el.selectionEnd).toBe(0);
    });
  });
  ```

  Imports follow the existing test file's patterns. The CLAUDE.md
  ≤80-line rule is preserved (the new function bodies are 1–3 lines
  each).

**Tests:** `cargo run -p zero -- test dom-shim.test.js` passes
(including the two new cases). The rest of the runtime tests continue
to pass — these APIs were undefined before so no existing tests can
have asserted their absence.

---

### Step 2: Add Combobox scaffold templates (component, partial, placeholder test)

**Goal:** Land the three Combobox files the manifest will reference so
the `include_str!()` additions in Step 3 don't fail to compile. After
this step the files exist but the manifest still has 16 components.

**Files (new):**

- `crates/zero-scaffold/src/scaffold/.zero/components/Combobox.ts`
- `crates/zero-scaffold/src/scaffold/.zero/components/Combobox.test.ts`
  (placeholder: `import { describe } from "zero/test"; describe("Combobox", () => {});`)
- `crates/zero-scaffold/src/scaffold/.zero/styles/components/_combobox.scss`

**Changes — `Combobox.ts`:**

Header and exports:

```ts
import { html, signal, effect, ref } from "zero";
import type { Signal, TemplateResult, Ref } from "zero";

export type ComboboxSize = "sm" | "md" | "lg";

export type ComboboxOption = {
  value: string;
  label: string;
};

export type ComboboxProps = {
  value: Signal<string>;
  loadOptions: (query: string) => Promise<ComboboxOption[]>;
  initialLabel?: string;
  size?: ComboboxSize;
  placeholder?: string;
  label?: string;
  disabled?: Signal<boolean> | boolean;
  debounceMs?: number;
  minQueryLength?: number;
  noResultsLabel?: string;
  loadingLabel?: string;
  onChange?: (value: string, option: ComboboxOption) => void;
};
```

Internal helpers copied from `Pagination.ts` (NOT re-exported; the
constraint forbids new public runtime exports):

```ts
function isSignal<T>(p: Signal<T> | T): p is Signal<T> { ... }
function read<T>(p: Signal<T> | T): T { ... }
```

A module-level monotonic counter for per-mount IDs:

```ts
let _comboboxIdCounter = 0;
```

The default export. Body capped at ~80 lines by extracting four named
helpers above it: `scheduleFetch`, `applyGhost`, `pick`, `revertOnBlur`.

```ts
export default function Combobox(props: ComboboxProps): TemplateResult {
  const size: ComboboxSize = props.size ?? "md";
  const debounceMs = props.debounceMs ?? 200;
  const minQueryLength = props.minQueryLength ?? 1;
  const noResultsLabel = props.noResultsLabel ?? "No results";
  const loadingLabel = props.loadingLabel ?? "Loading…";
  const id = ++_comboboxIdCounter;
  const inputId = `combobox-input-${id}`;
  const listId = `combobox-list-${id}`;
  const optionId = (i: number): string => `combobox-option-${id}-${i}`;

  const query = signal("");
  const options = signal<ComboboxOption[]>([]);
  const highlight = signal(-1);
  const open = signal(false);
  const busy = signal(false);
  const lastLabel = signal(props.initialLabel ?? "");
  const resolved = signal(false);  // a fetch has resolved at least once

  const inputRef: Ref<HTMLInputElement> = ref();
  const rootRef: Ref<HTMLElement> = ref();

  let timer: ReturnType<typeof setTimeout> | null = null;
  let serial = 0;
  // ... see helpers below
}
```

Helper signatures and behaviour:

- `scheduleFetch(prefix: string): void`
  - Reads `read(props.disabled)`; returns early if disabled.
  - Clears the pending `timer`.
  - If `prefix.length < minQueryLength`, sets `options.set([])`,
    `busy.set(false)`, `highlight.set(-1)`, `open.set(false)`, and
    returns.
  - Otherwise schedules a `setTimeout` for `debounceMs`. The callback
    captures `mySerial = ++serial`, sets `busy.set(true)`, opens the
    dropdown (`open.set(true)`), and calls
    `props.loadOptions(prefix).then(opts => { if (mySerial !== serial)
    return; busy.set(false); resolved.set(true); options.set(opts);
    highlight.set(opts.length > 0 ? 0 : -1); applyGhost(prefix, opts);
    })`. A `.catch` clears `busy` and treats the failure as an empty
    result (`options.set([])`, `highlight.set(-1)`) — same UX as
    no-match.

- `applyGhost(prefix: string, opts: ComboboxOption[]): void`
  - Returns if `inputRef.el == null`. Otherwise picks the first option
    whose `label.toLowerCase().startsWith(prefix.toLowerCase())`. If
    one exists, sets `inputRef.el.value = match.label` then
    `inputRef.el.setSelectionRange?.(prefix.length, match.label.length)`.
    The `?.` guards browsers that lack the API on a synthetic input;
    the dom-shim Step 1 also provides it under `zero test`.
  - If no match, sets `inputRef.el.value = prefix` (no selection).

- `pick(opt: ComboboxOption): void`
  - `props.value.set(opt.value)`; `lastLabel.set(opt.label)`;
    `options.set([])` is **not** done (keeps last result list for
    re-open); `highlight.set(-1)`; `open.set(false)`. Imperatively
    set `inputRef.el!.value = opt.label` and clear selection
    (`setSelectionRange?.(opt.label.length, opt.label.length)`).
    Finally fires `props.onChange?.(opt.value, opt)`.

- `revertOnBlur(): void`
  - If `inputRef.el == null`, returns. Let `cur = inputRef.el.value`.
    If `options.val.some(o => o.label === cur)` — i.e. the visible
    text is already exactly one of the known options — leave it.
    Otherwise set `inputRef.el.value = lastLabel.val`. `props.value`
    is not modified. `open.set(false)`. `highlight.set(-1)`.

Event handlers on the input:

- `@input=${(e: Event) => { const t = e.target as HTMLInputElement;
  const prefix = t.value.slice(0, t.selectionStart ?? t.value.length);
  query.set(prefix); scheduleFetch(prefix); }}` — the slice up to
  `selectionStart` strips any previously rendered ghost tail (the user
  typed into the unselected prefix; the selected tail was
  auto-replaced by the typed key per native input semantics).

- `@keydown=${onKey}` where `onKey(e)` switches on `e.key`:
  - `"ArrowDown"`: `e.preventDefault()`. If `options.val.length === 0`
    return. If `!open.val && resolved.val` set `open.set(true)`. Set
    `highlight.set((highlight.val + 1) % options.val.length)`. Call
    `applyGhost(query.val, options.val)` against the new highlight
    (helper variant `applyGhostForIndex(idx)` is cleaner — wraps
    `applyGhost(query.val, [options.val[idx]!])`).
  - `"ArrowUp"`: symmetric with wrap (`(highlight.val - 1 +
    options.val.length) % options.val.length`).
  - `"Enter"`: `e.preventDefault()`. If `highlight.val >= 0 &&
    options.val[highlight.val]` then `pick(options.val[highlight.val]!)`.
    Otherwise no-op.
  - `"Escape"`: `open.set(false)`; `highlight.set(-1)`.
    `e.preventDefault()` to swallow form-cancel side-effects.
  - `"Tab"`: if `highlight.val >= 0 && options.val[highlight.val]` and
    `inputRef.el?.value === options.val[highlight.val].label` (i.e. a
    ghost is currently visible), `e.preventDefault()` and
    `pick(options.val[highlight.val]!)`. Otherwise just close the
    dropdown (`open.set(false)`); native tab focus shift then proceeds
    and the `@blur` handler runs `revertOnBlur`.
  - Default: do nothing (let the browser handle it).

- `@focus=${() => { if (read(props.disabled)) return; if
  (resolved.val) open.set(true); }}` — does not auto-fetch with an
  empty query (Open-Question decision: `fetchOnFocus = false`).

- `@blur=${revertOnBlur}`.

The dropdown `<ul>` items have `@click=${() => pick(opt)}` and a
`@mousedown=${(e) => e.preventDefault()}` (or
`@mousedown.prevent=${() => {}}`) so the click fires before the
input's blur handler steals focus. The `.prevent` modifier is already
supported by the template DSL.

Outside-click handler via `effect()` (mirrors Dialog.ts:29):

```ts
effect(() => {
  if (!open.val) return;
  const onDown = (e: MouseEvent) => {
    const root = rootRef.el;
    if (!root) return;
    if (e.target && root.contains(e.target as Node)) return;
    revertOnBlur();
  };
  document.addEventListener("mousedown", onDown);
  return () => document.removeEventListener("mousedown", onDown);
});
```

Template (compact, but each attribute is on its own line for diff
clarity — actual file may inline for ~80-line target):

```ts
const wrapperCls = (): string =>
  `combobox combobox-${size}${open.val ? " combobox-open" : ""}${
    read(props.disabled) ? " combobox-disabled" : ""
  }`;
const listHidden = (): boolean => !open.val;
const ariaExpanded = (): string => (open.val ? "true" : "false");
const ariaActiveDescendant = (): string | null =>
  highlight.val >= 0 ? optionId(highlight.val) : null;

const labelNode: TemplateResult | null = props.label
  ? html`<label class="combobox-label" for=${inputId}>${props.label}</label>`
  : null;

const dropdownBody = (): TemplateResult | null => {
  if (busy.val && options.val.length === 0) {
    return html`<li class="combobox-loading" aria-busy="true">${loadingLabel}</li>`;
  }
  if (resolved.val && options.val.length === 0) {
    return html`<li class="combobox-empty" aria-disabled="true">${noResultsLabel}</li>`;
  }
  return html`${options.val.map((o, i) => html`
    <li
      class=${() => "combobox-option" + (highlight.val === i ? " combobox-option-active" : "")}
      id=${optionId(i)}
      role="option"
      aria-selected=${() => highlight.val === i ? "true" : "false"}
      @mousedown.prevent=${() => {}}
      @click=${() => pick(o)}
    >${o.label}</li>
  `)}`;
};

return html`
  <div
    class=${wrapperCls}
    ref=${rootRef}
    role="combobox"
    aria-haspopup="listbox"
    aria-expanded=${ariaExpanded}
    aria-owns=${listId}
  >
    ${labelNode}
    <div class="combobox-field">
      <input
        ref=${inputRef}
        class="combobox-input"
        id=${inputId}
        type="text"
        role="combobox"
        autocomplete="off"
        aria-autocomplete="both"
        aria-controls=${listId}
        aria-activedescendant=${ariaActiveDescendant}
        placeholder=${props.placeholder ?? ""}
        value=${() => lastLabel.val}
        disabled=${() => read(props.disabled) ?? false}
        @input=${onInput}
        @keydown=${onKey}
        @focus=${onFocus}
        @blur=${onBlur}
      >
      <span class="combobox-spinner" hidden=${() => !busy.val} aria-hidden="true"></span>
    </div>
    <ul
      class="combobox-list"
      id=${listId}
      role="listbox"
      hidden=${listHidden}
      aria-busy=${() => busy.val ? "true" : "false"}
    >${dropdownBody}</ul>
  </div>
`;
```

Note: `value=${() => lastLabel.val}` runs once on mount to seed the
initial display from `initialLabel`. The component thereafter mutates
`inputRef.el.value` imperatively (in `applyGhost` and `pick`) — the
reactive binding is **not** re-asserted on every keystroke, matching
Requirement 13 (the input does not auto-sync visible text to external
`value` changes). To prevent the reactive value binding from
clobbering the imperatively-set text every time `lastLabel` happens
to change, the binding reads `lastLabel` (which only changes on
pick, not on input) rather than `props.value`. This means the seed
runs exactly once with `props.initialLabel ?? ""`, and after first
pick the binding re-asserts the picked label — which matches what we
want, because by that point `pick()` has already imperatively set
the same text.

JSDoc on every exported symbol and helper per CLAUDE.md JS/TS style
(`@param`, `@returns`, `@template` where applicable). Helpers carry
`@internal`.

**Changes — `_combobox.scss`:**

Follows the spec DOM shape and Requirements 16–29. Token-only values
with the spec-permitted exceptions for `z-index`, `opacity`,
`transition-duration`, the dropdown `max-block-size` cap, and spinner
animation timings. Logical properties throughout. No `!important`.

```scss
// Combobox — input + dropdown typeahead with inline ghost completion.
// Input padding/borders/focus mirror `.input`. Dropdown stacks below
// the field via `position: absolute`; `z-index: 900` keeps it above
// inline content and below Dialog (z-index 1000).
@layer components {
  .combobox {
    position: relative;
    display: inline-block;
    inline-size: 100%;
    max-inline-size: 24rem;
  }
  .combobox-field {
    position: relative;
    display: flex;
    align-items: center;
  }
  .combobox-input {
    flex: 1 1 auto;
    inline-size: 100%;
    padding-inline: var(--space-md);
    padding-block: var(--space-sm);
    border: var(--border-thin) solid var(--color-border-strong);
    border-radius: var(--radius-sm);
    background: var(--color-bg);
    color: var(--color-text);
    font: inherit;
    line-height: var(--leading-normal);
    transition:
      border-color var(--duration-fast) var(--ease-out),
      box-shadow   var(--duration-fast) var(--ease-out);
  }
  .combobox-input::placeholder { color: var(--color-text-subtle); }
  .combobox-input:hover:not(:disabled) { border-color: var(--gray-500); }
  .combobox-input:focus {
    outline: none;
    border-color: var(--color-primary);
    box-shadow: 0 0 0 var(--ring-width) var(--ring-color);
  }
  .combobox-input:disabled {
    opacity: 0.5;
    cursor: not-allowed;
    background: var(--color-surface);
  }

  .combobox-sm .combobox-input { padding-inline: var(--space-sm); padding-block: var(--space-xs); font-size: var(--font-size-sm); }
  .combobox-md .combobox-input { padding-inline: var(--space-md); padding-block: var(--space-sm); font-size: var(--font-size-md); }
  .combobox-lg .combobox-input { padding-inline: var(--space-md); padding-block: var(--space-md); font-size: var(--font-size-lg); }

  .combobox-label {
    display: block;
    font-size: var(--font-size-sm);
    font-weight: var(--weight-medium);
    margin-block-end: var(--space-xs);
    color: var(--color-text);
  }

  .combobox-list {
    position: absolute;
    inset-inline-start: 0;
    inset-block-start: 100%;
    margin-block-start: var(--space-xs);
    inline-size: 100%;
    max-block-size: min(20rem, 60vh);
    overflow-y: auto;
    list-style: none;
    padding: 0;
    margin-inline: 0;
    background: var(--color-surface);
    border: var(--border-thin) solid var(--color-border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-lg);
    z-index: 900;
  }
  .combobox-list[hidden] { display: none; }

  .combobox-option {
    padding-inline: var(--space-md);
    padding-block: var(--space-sm);
    cursor: pointer;
    color: var(--color-text);
  }
  .combobox-option:hover,
  .combobox-option-active {
    background: var(--color-primary);
    color: var(--color-primary-fg);
  }

  .combobox-empty,
  .combobox-loading {
    padding-inline: var(--space-md);
    padding-block: var(--space-sm);
    color: var(--color-text-muted);
    cursor: default;
  }

  .combobox-spinner {
    position: absolute;
    inset-inline-end: var(--space-sm);
    inline-size: 1em;
    block-size: 1em;
    border: 2px solid var(--color-border);
    border-block-start-color: var(--color-primary);
    border-radius: 50%;
    animation: combobox-spin 0.6s linear infinite;
  }
  .combobox-spinner[hidden] { display: none; }

  @keyframes combobox-spin {
    to { transform: rotate(360deg); }
  }

  .combobox-disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
}
```

**Tests:** `cargo test -p zero-scaffold` continues to pass — the new
files exist on disk but aren't yet referenced by `framework_manifest()`
or `COMPONENT_NAMES`. (Test roster bumps happen in Steps 4 and 5.)

---

### Step 3: Wire Combobox into the manifest, index, aggregate SCSS, and `components.d.ts`

**Goal:** Make Combobox a first-class manifest entry so `zero init` and
`zero update` materialize it, and so editors and templates resolve it
via `"zero/components"`.

**Files (modified):**

- `crates/zero-scaffold/src/lib.rs`
- `crates/zero-scaffold/src/scaffold/.zero/components/index.ts`
- `crates/zero-scaffold/src/scaffold/.zero/components.d.ts`
- `crates/zero-scaffold/src/scaffold/.zero/styles/_components.scss`

**Changes:**

- `lib.rs`:
  - Three new `TPL_COMBOBOX_*` constants, alphabetical position
    (between `TPL_CHECKBOX_*` at L49–51 and `TPL_DIALOG_*` at L52–54):

    ```rust
    const TPL_COMBOBOX_TS: &str = include_str!("scaffold/.zero/components/Combobox.ts");
    const TPL_COMBOBOX_TEST_TS: &str = include_str!("scaffold/.zero/components/Combobox.test.ts");
    const TPL_COMBOBOX_SCSS: &str = include_str!("scaffold/.zero/styles/components/_combobox.scss");
    ```

  - Three new `framework_manifest()` entries, alphabetical position
    (after the Checkbox triple at L156–158, before the Dialog triple
    at L159–161):

    ```rust
    (".zero/components/Combobox.ts", TPL_COMBOBOX_TS),
    (".zero/components/Combobox.test.ts", TPL_COMBOBOX_TEST_TS),
    (".zero/styles/components/_combobox.scss", TPL_COMBOBOX_SCSS),
    ```

- `index.ts` — add (between Checkbox L10–11 and Dialog L12–13):

  ```ts
  export { default as Combobox } from "./Combobox.ts";
  export type { ComboboxOption, ComboboxProps, ComboboxSize } from "./Combobox.ts";
  ```

- `components.d.ts` — add a `Combobox` block between `CheckboxProps` /
  `Checkbox(...)` (L45–50) and `DialogSize` (L52). Matches the spec's
  prop sketch verbatim:

  ```ts
  export type ComboboxSize = "sm" | "md" | "lg";
  export type ComboboxOption = { value: string; label: string };
  export type ComboboxProps = {
    value: Signal<string>;
    loadOptions: (query: string) => Promise<ComboboxOption[]>;
    initialLabel?: string;
    size?: ComboboxSize;
    placeholder?: string;
    label?: string;
    disabled?: Signal<boolean> | boolean;
    debounceMs?: number;
    minQueryLength?: number;
    noResultsLabel?: string;
    loadingLabel?: string;
    onChange?: (value: string, option: ComboboxOption) => void;
  };
  export function Combobox(props: ComboboxProps): TemplateResult;
  ```

- `_components.scss` — insert `@use 'components/combobox';` between
  `@use 'components/checkbox';` (L9) and `@use 'components/dialog';`
  (L10).

**Tests:** `cargo test -p zero-scaffold` now needs to know about
Combobox (Step 4 below). After this step alone the path-set assertion
and the `COMPONENT_NAMES` iteration will fail — that's the signal to
do Step 4.

---

### Step 4: Bump the scaffold test roster

**Goal:** Make `zero-scaffold`'s unit tests aware of Combobox so the
length-coupled path-set assertion, the alphabetical name roster, and
the derived per-name iteration tests all match the new manifest.

**Files (modified):**

- `crates/zero-scaffold/src/lib.rs` — only the `#[cfg(test)] mod tests`
  block.

**Changes:**

- `COMPONENT_NAMES` (L313–330) gains `"Combobox"` in alphabetical
  position:

  ```rust
  const COMPONENT_NAMES: &[&str] = &[
      "Avatar", "Badge", "Button", "Card", "Checkbox", "Combobox",
      "Dialog", "Input", "Pagination", "Radio", "Select", "Spinner",
      "Table", "Tabs", "TextArea", "Toast", "Toggle",
  ];
  ```

- `framework_manifest_matches_expected_path_set` (L950) — add the three
  Combobox paths to the `expected` BTreeSet in alphabetical position
  (between the Checkbox triple at L987–989 and the Dialog triple at
  L990–992):

  ```rust
  ".zero/components/Combobox.ts",
  ".zero/components/Combobox.test.ts",
  ".zero/styles/components/_combobox.scss",
  ```

  Update the inline comment from
  `// 16 components × (source, test, scss partial) = 48 entries.` to
  `// 17 components × (source, test, scss partial) = 51 entries.`
  The `assert_eq!(manifest.len(), expected.len(), …)` is self-bumping.

- The iterating tests
  (`components_index_re_exports_each_listed`,
  `component_source_files_emitted`,
  `component_test_files_emitted`,
  `component_partials_use_layer_components`,
  `components_aggregate_uses_each_partial`,
  `components_dts_declares_each_listed`) all derive from
  `COMPONENT_NAMES` — the single edit above covers them.

**Tests:** `cargo test -p zero-scaffold` passes. Every per-component
existence assertion now covers Combobox.

---

### Step 5: Bump the `component_library.rs` test roster

**Goal:** The showcase integration test that runs every component's
`*.test.ts` hard-codes the 16-name list. Bump to 17 so a missing
Combobox test is a clear failure.

**Files (modified):**

- `crates/zero/tests/component_library.rs`

**Changes:**

- The inline array in `showcase_test_runs_all_component_tests` (L35–52)
  gains `"Combobox"` between Checkbox and Dialog:

  ```rust
  for name in [
      "Avatar", "Badge", "Button", "Card", "Checkbox", "Combobox",
      "Dialog", "Input", "Pagination", "Radio", "Select", "Spinner",
      "Table", "Tabs", "TextArea", "Toast", "Toggle",
  ] { ... }
  ```

- No other integration test (`showcase_build.rs`, `showcase_dev.rs`,
  `design_system.rs`, `update.rs`) carries a length-coupled assertion
  against the component count — verified by reading them. They continue
  to pass unchanged.

**Tests:** `cargo test -p zero --test component_library` passes after
Step 6 fills the test file. Until then it fails because the report
contains no `Combobox` test name — the placeholder added in Step 2
runs `describe("Combobox", () => {})` with zero cases.

(Actually: the placeholder `describe` block produces no `it` lines,
which means the substring check `stdout.contains("Combobox")` may not
find the name. To keep the workspace green between Steps 5 and 6, the
placeholder added in Step 2 should include a single trivial passing
case: `it("is exported", () => { expect(Combobox).toBeTruthy(); });`.
Step 6 expands this into the full suite.)

---

### Step 6: Fill in `Combobox.test.ts`

**Goal:** Replace the placeholder with the full test suite from
Requirement 36, adapted to the in-memory DOM shim.

**Files (modified):**

- `crates/zero-scaffold/src/scaffold/.zero/components/Combobox.test.ts`

**Changes:**

Single `describe("Combobox", () => { ... })` block with
`afterEach(cleanup);`. Imports:

```ts
import { describe, it, expect, afterEach } from "zero/test";
import { render, find, findAll, fire, cleanup, text, spy } from "zero/test";
import { signal } from "zero";
import Combobox from "./Combobox.ts";
import type { ComboboxOption } from "./Combobox.ts";
```

Small helpers above `describe`:

```ts
const wait = (ms: number): Promise<void> =>
  new Promise((r) => setTimeout(r, ms));

const staticLoader = (opts: ComboboxOption[]) =>
  async (q: string): Promise<ComboboxOption[]> =>
    opts.filter((o) => o.label.toLowerCase().startsWith(q.toLowerCase()));

const fireInput = (input: Element, value: string, selectionStart?: number) => {
  (input as any).value = value;
  if (selectionStart != null) (input as any).selectionStart = selectionStart;
  fire(input, "input", { target: input });
};
```

`fireInput` mutates the input's `value` and `selectionStart` directly
on the shim element before dispatching `input`. The component reads
`e.target.value` and `e.target.selectionStart` — matching real-browser
semantics where the browser has already applied the keystroke before
the event fires.

The `it` cases (one per Requirement 36 bullet):

1. **Renders the base markup.** `value: signal(""), loadOptions:
   staticLoader([])`. Asserts `find(el, ".combobox")`,
   `find(el, ".combobox-input")`, and `find(el, ".combobox-list")` are
   non-null. `find(el, ".combobox-list")!.hasAttribute("hidden")` is
   true.

2. **Typing triggers a debounced fetch.** `debounceMs: 5,
   minQueryLength: 1`. `loadOptions = spy(staticLoader(...))`.
   `fireInput(input, "f", 1)`; `await wait(20)`. Asserts
   `loadOptions.callCount === 1` with arg `"f"`.

3. **`minQueryLength` gates the fetch.** `minQueryLength: 2`.
   `fireInput(input, "f", 1)`; `await wait(20)`;
   `expect(loadOptions.callCount).toBe(0)`. `fireInput(input, "fo", 2)`;
   `await wait(20)`; `expect(loadOptions.callCount).toBe(1)`.

4. **Race safety.** Two hand-controlled `Promise.withResolvers`-shaped
   promises returned in order. `loadOptions` returns `pA` then `pB`.
   `fireInput(input, "a", 1); await wait(20); fireInput(input, "ab", 2);
   await wait(20);`. Resolve `pB` first with one option set, then `pA`
   with a different set. Assert the rendered options reflect `pB`'s
   set; `pA`'s resolution is dropped.

5. **Ghost completion.** `loadOptions: staticLoader([{ value: "foobar",
   label: "foobar" }]), debounceMs: 5`. `fireInput(input, "foo", 3);
   await wait(20);`. Asserts `(input as any).value === "foobar"`,
   `(input as any).selectionStart === 3`, `(input as any).selectionEnd
   === 6`. (Step 1's dom-shim extension makes these well-defined.)

6. **ArrowDown / ArrowUp move highlight and update ghost.** Three
   matching options `[{value:"a1",label:"alpha"},
   {value:"a2",label:"alphabet"}, {value:"a3",label:"alps"}]`.
   `fireInput(input, "a", 1); await wait(20);`. After fetch resolves,
   highlight is 0 and `input.value === "alpha"`. `fire(input, "keydown",
   { key: "ArrowDown" })` → highlight 1, `input.value === "alphabet"`.
   `fire(input, "keydown", { key: "ArrowDown" })` → highlight 2.
   `fire(input, "keydown", { key: "ArrowUp" })` → highlight 1.
   Wrap-around: from highlight 0, ArrowUp lands on 2.

7. **Enter accepts the highlight.** With three options and highlight
   on 1: `fire(input, "keydown", { key: "Enter" })`. Asserts
   `value.val === "a2"`, `onChange` spy called once with
   `("a2", { value: "a2", label: "alphabet" })`. Dropdown closes
   (`find(el, ".combobox-list")!.hasAttribute("hidden")` true). Input
   visible value equals `"alphabet"`.

8. **Tab-to-complete.** Setup as in case 5 (ghost showing). `fire(input,
   "keydown", { key: "Tab" })` — same effect as Enter (pick fires,
   `onChange` called, dropdown closes). Second case: type a string with
   no match, `fire(input, "keydown", { key: "Tab" })` — dropdown
   closes, `value.val` unchanged, `onChange` not called.

9. **Escape closes without picking.** With ghost showing,
   `fire(input, "keydown", { key: "Escape" })`. Dropdown hidden.
   `value.val` unchanged. `onChange` not called.

10. **Blur strict-revert.** `fireInput(input, "xyz", 3); await wait(20);`
    — no matches. `fire(input, "blur")`. Asserts `(input as any).value
    === ""` (no `initialLabel`, no prior pick). `value.val === ""`.
    Repeat with `initialLabel: "Default"` — after blur, `input.value
    === "Default"`.

11. **Click on a dropdown option picks it.** With three options
    rendered, `fire(findAll(el, ".combobox-option")[1]!, "click")`.
    Same assertions as Enter case.

12. **`initialLabel` displays until first pick.** `value: signal("u-42"),
    initialLabel: "Alice"`. `expect((find(el,"input") as any).value).toBe("Alice")`.
    Type something matching, pick it, assert visible value updates.

13. **Disabled (plain boolean).** `disabled: true`. Input has
    `disabled` attribute. `fireInput(input, "f", 1); await wait(20);`
    — `loadOptions` not called.

14. **Disabled (signal).** `disabled = signal(false)`. Type, fetch
    fires, dropdown opens. `disabled.set(true)`. Input gains
    `disabled`; dropdown hides; subsequent typing does not fetch.

15. **No-results state.** `loadOptions` returns `[]`.
    `fireInput(input, "xyz", 3); await wait(20);`. Asserts
    `find(el, ".combobox-empty")` is non-null and its text matches
    the configured `noResultsLabel ?? "No results"`.

16. **Loading state.** Long-running `loadOptions` (manual resolver).
    After `fireInput(...)` and `await wait(20)`, the dropdown is open,
    `find(el, ".combobox-spinner")` does NOT have `hidden`, and
    `find(el, ".combobox-loading")` is rendered. After resolving with
    `[]`, both states clear (`hidden` returns; `.combobox-loading`
    replaced by `.combobox-empty`).

17. **Size variant.** `size: "sm"` puts `combobox-sm` on
    `find(el, ".combobox")`. Default puts `combobox-md`.

18. **`onChange` semantics.** Single pick → exactly one call. Escape,
    blur, and dropdown re-renders never call it. Receives `(value,
    option)`.

The whole file is ≤ ~250 lines (target). Each `it()` body stays under
~30 lines.

**Tests:** `cargo run -p zero -- test Combobox.test.ts` from
`runtime/` (or in a scaffolded showcase). `cargo test -p zero --test
component_library` passes (the `"Combobox"` substring appears in
the report). `cargo test --workspace` clean.

---

### Step 7: Add the `/combobox` showcase route and home-nav entry

**Goal:** Manual / visual verification surface for Combobox and CI
input to `showcase_build.rs` / `showcase_dev.rs`.

**Files (new):**

- `showcase/src/routes/combobox.ts`

**Files (modified):**

- `showcase/src/app.ts` — `import ComboboxRoute from "./routes/combobox.ts";`
  in alphabetical position (between `CheckboxRoute` at L7 and
  `DialogRoute` at L8). `app.route("/combobox", ComboboxRoute);` in the
  same alphabetical position (between L40 and L41).
- `showcase/src/routes/home.ts` — `{ name: "Combobox", href:
  "/combobox" }` in the `components` array (between Checkbox at L11
  and Dialog at L12).

**Changes — route body:**

Five instances per Requirement 39:

```ts
import { html, signal } from "zero";
import type { TemplateResult } from "zero";
import { Combobox } from "zero/components";
import type { ComboboxOption } from "zero/components";

const COUNTRIES: ComboboxOption[] = [
  { value: "ar", label: "Argentina" },
  { value: "au", label: "Australia" },
  { value: "br", label: "Brazil" },
  { value: "ca", label: "Canada" },
  { value: "cn", label: "China" },
  { value: "de", label: "Germany" },
  // … 30 entries total, sorted
  { value: "us", label: "United States" },
];

const filterCountries = async (q: string): Promise<ComboboxOption[]> => {
  await new Promise((r) => setTimeout(r, 120));
  return COUNTRIES.filter((c) =>
    c.label.toLowerCase().startsWith(q.toLowerCase()),
  );
};

export default function ComboboxRoute(): TemplateResult {
  const v1 = signal("");
  const v2 = signal("");
  const v3 = signal("");
  const v4 = signal("us");
  const v5 = signal("");
  const busy = signal(false);
  const resetKey = signal(0);

  // Example 5 — pretend backend.
  const slowFetch = async (q: string): Promise<ComboboxOption[]> => {
    await new Promise((r) => setTimeout(r, 500));
    return COUNTRIES.filter((c) =>
      c.label.toLowerCase().includes(q.toLowerCase()),
    );
  };
  // Real apps would call `fetch` / `createHttp` / GraphQL here —
  // Combobox doesn't care, the contract is `Promise<ComboboxOption[]>`.

  return html`
    <main class="showcase-page stack pad-xl">
      <h1 class="text-h1">Combobox</h1>

      <section class="stack gap-sm">
        <h2 class="text-h2">Default (md)</h2>
        ${Combobox({ value: v1, label: "Country",
                     placeholder: "Type a country…",
                     loadOptions: filterCountries })}
        <p class="text-body">Picked: ${() => v1.val}</p>
      </section>

      <section class="stack gap-sm">
        <h2 class="text-h2">Small</h2>
        ${Combobox({ value: v2, size: "sm",
                     placeholder: "Type a country…",
                     loadOptions: filterCountries })}
        <p class="text-body">Picked: ${() => v2.val}</p>
      </section>

      <section class="stack gap-sm">
        <h2 class="text-h2">Large</h2>
        ${Combobox({ value: v3, size: "lg",
                     placeholder: "Type a country…",
                     loadOptions: filterCountries })}
        <p class="text-body">Picked: ${() => v3.val}</p>
      </section>

      <section class="stack gap-sm">
        <h2 class="text-h2">Initial label (URL-restore)</h2>
        ${() => {
          // Remount via `resetKey` so reset can re-seed `initialLabel`.
          const _ = resetKey.val;
          return Combobox({
            value: v4,
            initialLabel: "United States",
            label: "Country",
            loadOptions: filterCountries,
          });
        }}
        <button class="button button-secondary button-sm"
                @click=${() => { v4.set("us"); resetKey.set(resetKey.val + 1); }}>
          Reset
        </button>
        <p class="text-body">Picked: ${() => v4.val}</p>
      </section>

      <section class="stack gap-sm">
        <h2 class="text-h2">Async (mocked) + disabled signal</h2>
        ${Combobox({ value: v5, disabled: busy,
                     placeholder: "Slow search…",
                     debounceMs: 300,
                     loadOptions: slowFetch })}
        <div class="cluster gap-sm">
          <button class="button button-secondary button-sm"
                  @click=${() => { busy.set(true); setTimeout(() => busy.set(false), 1500); }}>
            Simulate busy
          </button>
          <span class="text-small">busy: ${() => String(busy.val)}</span>
        </div>
        <p class="text-body">Picked: ${() => v5.val}</p>
      </section>

      <a class="showcase-nav-link" href="/">Back</a>
    </main>
  `;
}
```

**Tests:** `cargo test -p zero --test showcase_build` and `--test
showcase_dev` continue to pass. The dev test only asserts the
importmap and a single `"Avatar"` substring — Combobox rides on the
same plumbing, no widening needed.

---

### Step 8: Refresh AGENTS.md, docs/components.md, and docs/index.md

**Goal:** Documentation surfaces all agree the library has seventeen
components and Combobox is one of them.

**Files (modified):**

- `crates/zero-scaffold/src/scaffold/AGENTS.md`
- `docs/components.md`
- `docs/index.md`

**Files explicitly NOT modified:**

- `zero-framework-spec.md` — does not exist in this repo (verified
  with `find`). Requirements 48–50 reference documentation surfaces
  that don't apply here. Same precedent as the Pagination plan.
- `docs/best-practices.md` — Requirement 50 is judgment-call; the
  Combobox showcase route already demonstrates the canonical static
  and async patterns end-to-end. No best-practices subsection ships.

**Changes:**

- `crates/zero-scaffold/src/scaffold/AGENTS.md`:
  - In the `## Component library` table (between `Checkbox` row at
    L269 and `Dialog` row at L270), insert:
    ``| `Combobox` | `value: Signal<string>`                          |``
  - In the `## The .zero/ directory` table (L301–313), bump three
    cells from `(16 total)` to `(17 total)`:
    - `.zero/components/<Name>.ts`
    - `.zero/components/<Name>.test.ts`
    - `.zero/styles/components/_<name>.scss`

- `docs/components.md`:
  - L147: `zero ships sixteen production-ready components` →
    `zero ships seventeen production-ready components`.
  - Insert a new row in the summary table between `Checkbox` (L158)
    and `Dialog` (L159):
    ```
    | `Combobox` | `value: Signal<string>`, `loadOptions: (q) => Promise<ComboboxOption[]>`; optional `initialLabel`, `size`, `placeholder`, `label`, `disabled`, `debounceMs`, `minQueryLength`, `noResultsLabel`, `loadingLabel`, `onChange` | `Combobox({ value, loadOptions: loadUsers })` |
    ```

- `docs/index.md`:
  - Locate the `sixteen shipped components` string and bump to
    `seventeen shipped components`. (Identical edit to the Pagination
    plan's edit pattern.)

**Tests:** None directly exercised by integration tests. Visual
verification via `cargo test -p zero-scaffold` (the
`agents_md_has_section_sentinels` test continues to pass — sentinels
unchanged). Manual `cargo build --workspace` confirms `include_str!`
resolves.

---

## Risks and Assumptions

### Resolved open questions (from spec §"Open Questions")

- **Initial fetch on focus with empty query.** **No** — focus alone
  does not fetch. The user must type at least `minQueryLength` chars.
  Rationale: matches the spec default, avoids "always fire a search on
  tab-through a form" surprise, and keeps the API surface minimal. A
  `fetchOnFocus?: boolean` prop is **not** added in v1; can be added
  later additively if a real consumer needs it.
- **ArrowUp wrap-around.** **Wrap** (last → first, first → last).
  Matches most combobox implementations and pairs naturally with
  highlight-default-zero (see below).
- **Tab-to-complete behaviour.** **Accept the highlighted ghost as a
  pick.** Matches browser autofill idiom. Implementation guards
  against accidental picks: only fires when a ghost is currently
  visible (`inputRef.el.value === options.val[highlight.val].label`),
  otherwise Tab is just a dropdown-close + native focus shift.
- **Ghost case-sensitivity.** **Case-insensitive** via
  `.toLowerCase().startsWith(...)`. Locale-aware is deferred.
- **Highlight default after a fetch.** **`0`** when results are
  non-empty (matches the ghost shown), `-1` when empty.
- **Outside-click listener.** **`mousedown`** (spec default). Touch
  parity via `pointerdown` deferred — the dom-shim does not model
  pointer events, and `mousedown` works in all desktop browsers under
  test.
- **Dropdown positioning.** Absolute-below only — no flip-to-top. The
  spec's deferred-to-future a11y polish remains deferred; spec does
  not require it.
- **`combobox-list` `z-index`.** **`900`** — deliberately below
  `Dialog`'s `1000` (so a dialog containing a Combobox covers a
  stray-dropdown if any), above inline content. Hardcoded as a magic
  number per the spec's permitted `z-index` exception (alongside
  Dialog's hardcoded `1000`).
- **`ComboboxOption` exposure.** **Duplicate** the `{value, label}`
  shape rather than aliasing `SelectOption`. Keeps Combobox self-
  contained; the structural compatibility is enough that a future
  user can assign one to the other without type ceremony.
- **Showcase reset mechanism.** **Remount via a `key` signal** —
  Example 4 wraps the Combobox call in a reactive substitution
  (`${() => { const _ = resetKey.val; return Combobox({…}); }}`) so
  incrementing `resetKey` re-invokes the component function with a
  fresh `initialLabel` seed. Avoids the plain-function-contract
  break that a `reset()` method would introduce.
- **AGENTS.md / docs grouping.** Alphabetical, no Form-Inputs
  sub-cluster (matches existing AGENTS.md layout).
- **Manifest size.** Current manifest (post-Pagination) has 51
  entries (49 components ×… plus the design-system overhead). The
  path-set assertion is the only length-coupled check, and it
  self-bumps via `expected.len()`. The inline comment is updated
  from `// 16 components × ... = 48 entries.` to `// 17 components ×
  ... = 51 entries.`.
- **Async test waits.** **Real `setTimeout` with tiny `debounceMs`**
  (per spec). The component's debounce is configurable; tests use
  `debounceMs: 5` and `await new Promise(r => setTimeout(r, 20))`.
  Per-test cost ≈ 20 ms × ~10 timing-sensitive cases = ~200 ms; well
  within the test-runner's tolerance.
- **`onResults` callback.** **Not added.** Out of scope per the
  spec's minimal-API stance.

### Assumptions worth flagging

- **dom-shim selection-API extension is safe and minimal.** The shim
  defines element properties per-element rather than via prototypes,
  so attaching `selectionStart` / `selectionEnd` /
  `setSelectionRange` in `_attachInputProps` happens uniformly across
  all created elements. No tag-specialization plumbing exists or
  needs adding. The two-line cost is bounded; only Combobox consumes
  the surface today.

- **Imperative `inputRef.el.value` vs. reactive `value=${() =>
  lastLabel.val}` binding.** The component sets the input's visible
  text both ways: once at mount (the binding seeds it from
  `lastLabel`/`initialLabel`), and continuously thereafter via
  imperative writes inside `applyGhost` and `pick`. Because the
  binding depends only on `lastLabel`, and `lastLabel` only changes
  on a successful pick (after which `pick` has already imperatively
  set the same text), the binding never clobbers the user's typed
  text mid-keystroke. If a regression appears where the binding
  re-fires unexpectedly, the fallback is to drop the binding and
  seed via an `effect(() => { if (inputRef.el)
  inputRef.el.value = props.initialLabel ?? ""; }, [])` that runs
  once.

- **`@mousedown.prevent` on dropdown options.** Required so a click
  on an option fires before the input's `blur` handler steals focus
  and runs `revertOnBlur`. The `.prevent` modifier is already
  supported by the template DSL (verified in
  `runtime/template.test.js:387–429`).

- **`document.addEventListener` in tests.** The Combobox's outside-
  click `effect()` registers on `document.mousedown`. The dom-shim
  models a `document` global with `addEventListener` (used by
  `Dialog`'s `keydown` effect today — verified). No additional shim
  work required.

- **No new public runtime exports.** `isSignal` / `read` are local
  duck-typed helpers, copied from `Pagination.ts`. If a future
  refactor extracts them into a shared internal module under
  `.zero/components/_helpers.ts`, it is a separate PR with no
  contract change.

- **JSDoc compliance.** Every exported symbol carries `@param` /
  `@returns` annotations per CLAUDE.md. Internal helpers carry
  `@internal`. Function bodies stay under ~80 lines via the
  extracted `scheduleFetch` / `applyGhost` / `pick` / `revertOnBlur`
  helpers.

- **Showcase route's `setTimeout`-driven fakeFetch.** Used in the
  async example only. A clarifying comment in the source notes that
  a real app substitutes `fetch` / `createHttp` / GraphQL / etc. —
  Combobox doesn't care.

- **`zero-framework-spec.md` non-existence.** Confirmed via `find
  /home/rob/Documents/code/zero -iname "zero-framework-spec*"`. The
  Pagination plan also dropped Requirements 42–45 for the same
  reason. Combobox follows that precedent.
