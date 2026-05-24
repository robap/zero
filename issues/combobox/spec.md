# Spec: Combobox component

## Problem Statement

The component library now ships sixteen components, including a `Select`
that takes a fixed `{value, label}[]` array. Every real zero app that
needs to pick from a backend-driven option set — user pickers,
tag/category suggesters, place lookups, product search inside a form —
currently hand-rolls the same input + filtered list + keyboard nav + race-
safe fetch logic. Each reinvention re-litigates the same ARIA decisions
and the same debounce/min-length plumbing, and drifts off the design-
system token palette.

This issue adds `Combobox` to the shipped component library. Scope is
deliberately tight: a controlled, single-select typeahead with inline
ghost-text completion of the first match, a dropdown of suggestions
fetched from the parent's backend via a `loadOptions(query)` callback,
debounced/min-length-gated requests, race-safe result handling, and
strict picking semantics (no free text — typed input that doesn't match
an option is discarded on blur). The component is decoupled from any
particular backend; the parent supplies `loadOptions` and Combobox
exposes only the reactive seams it needs.

## Background

### What exists today

- **Component library (Phase 9 + Table + Pagination extensions).**
  Sixteen components ship under `.zero/components/`, re-exported from
  `.zero/components/index.ts`, importable via the bare specifier
  `"zero/components"`. Each component:
  - Is a plain function: `Component<P> = (props?: P) => TemplateResult`.
  - Reads stateful props as signals; never holds internal state for
    `value`/`open`/`active`/`checked`.
  - Has a per-component SCSS partial under
    `.zero/styles/components/_{name}.scss`, every rule inside
    `@layer components { ... }`.
  - Has a `*.test.ts` neighbour that ships in the manifest and runs
    with `zero test`.
- **Scaffold / manifest plumbing.** `framework_manifest()` in
  `crates/zero-scaffold/src/lib.rs` lists every framework-owned file
  under `.zero/`. Each entry is a `TPL_*` constant pointing at
  `include_str!("scaffold/.zero/...")`. The `_components.scss` aggregate
  `@use`s every per-component partial in alphabetical order; `zero.scss`
  `@use`s `'components'` last.
- **Module resolution.** The dev-server transpile pipeline and the
  production bundler both resolve `"zero/components"` to
  `.zero/components/index.ts`. Adding a new component is one line in
  that index file plus the file itself — no resolver change.
- **Design-system tokens (Phases 7–8).** All values components are
  allowed to consume are `var(--*)` custom properties on `:root`.
  Spacing: `--space-{xs,sm,md,lg,xl}`. Colors: the semantic palette
  including `--color-{bg,surface,text,text-muted,border,border-strong,primary,primary-fg}`.
  Radii: `--radius-{sm,md,lg}`. Font sizes/weights/leadings, shadows,
  border widths — all present.
- **Showcase project.** `showcase/` is a full zero project at the repo
  root; every component has a route under `showcase/src/routes/`. Built
  by `zero build` in CI via `tests/showcase_build.rs`; served by
  `zero dev` in CI via `tests/showcase_dev.rs`. Component tests run via
  `tests/component_library.rs`. Adding a component means adding one
  route file and updating the home route's navigation cluster.
- **Reactivity primitives.** `signal`, `computed`, `effect`. Reactive
  bindings in templates auto-update when their dependent signals change.
- **Spec listed `Combobox` (then "autocomplete, combobox") as
  explicitly Out-of-Scope in the original Phase 9 components spec.**
  This issue un-defers it.
- **Existing precedents the new component echoes:**
  - `Select` — single-select contract, `value: Signal<string>`, options
    shape `{value, label}[]`. Combobox uses the same `value` model and
    a structurally identical option type (`ComboboxOption = SelectOption`-
    shaped; the plan picks whether to literally alias or duplicate).
  - `Input` — text-input markup, label slot, size variants, signal-
    driven value. Combobox's editable surface mirrors `Input`'s shape.
  - `Pagination` — signal-or-plain `disabled` prop, parent-driven async
    via a single reactive seam. Combobox follows the same disabled
    pattern but does **not** expose a signal-or-plain backend; the
    backend is exclusively the `loadOptions` callback.
  - `Dialog` — `effect` lifecycle for managing a document-level event
    listener (`keydown`). Combobox uses the same pattern to attach
    keyboard nav and outside-click handlers while the dropdown is open.

### Decisions made during refine

The user confirmed each of the following:

- **Backend shape: `loadOptions` callback.** The parent passes
  `loadOptions: (query: string) => Promise<ComboboxOption[]>`. Combobox
  owns debouncing, min-query gating, in-flight tracking, and race-safe
  ordering (latest-fetch wins). The parent never sees the query signal
  and never touches an options signal — they write a fetch function.
- **Value model: strict typeahead with inline ghost completion.** The
  user types `"foo"`; if the first matching option is `"foobar"`, the
  input displays `"foobar"` with the `"bar"` tail visually selected
  (native input selection). Pressing Enter accepts the highlighted
  ghost (or whichever option arrow-keys have highlighted in the
  dropdown). Continuing to type replaces the selected ghost tail
  naturally — the browser's own selection-aware text insertion does
  the right thing.
- **Dropdown: yes, in addition to the ghost.** A list of matching
  options renders below the input while the field has focus and the
  query has resolved. Arrow keys move highlight through the list and
  update the ghost completion. Enter accepts the highlighted item.
  Escape closes the dropdown without selecting. Click on a list item
  picks it.
- **Strict picking; no free text.** If the user types something with
  no matching option and blurs the field, the typed text is discarded
  and the input reverts to the last successfully selected option's
  label (or empty if nothing has been selected yet). Empty-result
  state shows a "No results" message in the dropdown. No
  `allowFreeText?: boolean` escape hatch in v1.
- **Cardinality: single-select.** `value: Signal<string>` — the picked
  option's value field. Mirrors `Select`. Multi-select is deferred.
- **Request gating: configurable debounce + min-query.**
  `debounceMs?: number` (default 200) waits for typing to settle.
  `minQueryLength?: number` (default 1) suppresses the fetch when the
  user has typed fewer chars. Below the min, the dropdown closes (or
  shows nothing); above, a fetch is scheduled `debounceMs` after the
  last keystroke. Both numbers are configurable so a parent can dial
  them to their backend's latency/cost characteristics.
- **Initial display: parent provides both value and label.** When the
  parent hands the component a pre-populated `value` (URL restore,
  defaults, etc.), they also pass `initialLabel?: string`. The
  component displays `initialLabel` until the user picks something
  else, at which point it tracks the picked option's label internally.
  This avoids any "look up the label by value via an extra fetch"
  ceremony and avoids a "show the raw value string until options
  load" inconsistency.

### Component contract

Same conventions as the existing roster:

- Variants and sizes are string-typed string-union props.
- The stateful prop the parent observes is a signal:
  `value: Signal<string>`.
- One configuration prop accepts signal-or-plain so an async parent
  can disable the whole control mid-flight without remount:
  `disabled?: Signal<boolean> | boolean`.
- Configuration is plain values: `loadOptions`, `initialLabel`,
  `placeholder`, `label`, `size`, `debounceMs`, `minQueryLength`,
  `noResultsLabel`, `loadingLabel`, `onChange`.
- Event callbacks are plain functions:
  `onChange?: (value: string, option: ComboboxOption) => void`. Called
  after `value.set(option.value)`. Receives both the value and the full
  option object so the parent can grab `label` if they want it.
- Dropdown open/closed state is **internal**, not a `Signal<boolean>`
  prop. Focus opens it (after at least one fetch settles); blur,
  Escape, and a successful pick close it. No external override.

### Props sketch

```ts
type ComboboxOption = {
  value: string;
  label: string;
};

type ComboboxSize = "sm" | "md" | "lg";

type ComboboxProps = {
  // Stateful (parent owns)
  value: Signal<string>;                          // picked option's value field

  // Backend seam
  loadOptions: (query: string) => Promise<ComboboxOption[]>;

  // Initial display when value is pre-populated
  initialLabel?: string;                          // shown until user picks something

  // Layout / display
  size?: ComboboxSize;                            // default "md"
  placeholder?: string;
  label?: string;                                 // <label class="combobox-label">

  // Reactive disable seam (mirrors Pagination)
  disabled?: Signal<boolean> | boolean;

  // Request gating
  debounceMs?: number;                            // default 200
  minQueryLength?: number;                        // default 1

  // Empty / loading state labels (localization)
  noResultsLabel?: string;                        // default "No results"
  loadingLabel?: string;                          // default "Loading…"

  // Event callback
  onChange?: (value: string, option: ComboboxOption) => void;
};
```

### Behaviour — typing and ghost completion

1. The user types into the input.
2. The component records the **caret prefix** — the text up to the
   current selection start (i.e. what the user has actually typed,
   excluding any previously-rendered ghost tail).
3. The component schedules a fetch via `loadOptions(prefix)` after
   `debounceMs` of quiet typing, but only when `prefix.length >=
   minQueryLength`. Below the threshold the dropdown closes and the
   ghost is cleared. If the user types more keys before the timer
   fires, the timer resets.
4. When the fetch resolves, the component:
   - **Drops the response if a newer fetch has been scheduled since.**
     Race-safe: each fetch carries an increasing serial; only the
     latest one's result is rendered.
   - Updates the dropdown's option list to the response.
   - If the response is non-empty, finds the first option whose
     `label` starts with `prefix` (case-insensitive). If a match
     exists, sets the input's value to the matched label and sets
     the selection range to `[prefix.length, matchedLabel.length]` —
     i.e. the prefix is unselected, the tail is selected. The
     dropdown's highlighted item is that first match.
   - If no option starts with `prefix`, the input value stays at
     `prefix` (no ghost); the dropdown shows the "No results" empty
     state.
5. Each keystroke that mutates the prefix re-runs steps 2–4. Pressing
   Backspace shortens the prefix; the next fetch happens after
   `debounceMs`.

### Behaviour — keyboard

- **ArrowDown** — moves highlight to the next option in the dropdown;
  the ghost completion updates to that option's label (prefix +
  selected tail). If the list is empty, no-op. If the dropdown is
  closed but a fetch has resolved, opens it and highlights the first
  item.
- **ArrowUp** — symmetric. Wraps from first to last (or no-op at top —
  the plan picks; spec proposes wrap for parity with most combobox
  implementations).
- **Enter** — accepts the currently highlighted option:
  `value.set(option.value)`, sets internal `lastLabel = option.label`,
  fires `onChange?.(option.value, option)`, sets input value to
  `option.label` with no selection, closes the dropdown. No-op if no
  highlight.
- **Escape** — closes the dropdown without picking. Clears the ghost
  selection. The input value stays at whatever was visibly displayed
  before; on the next blur the strict-revert rule fires.
- **Tab** — closes the dropdown. If a highlight is active and the
  current input value equals the highlighted label (i.e. ghost is
  showing), accepts that highlight as a pick (Tab-to-complete is the
  common combobox idiom; matches browser autofill). Otherwise the
  on-blur strict-revert rule applies.
- All other keys (printable chars, Backspace, Delete, Home, End, arrow
  Left/Right) fall through to the input. The component re-derives
  prefix from the input's `selectionStart` after each input event.

### Behaviour — focus and blur

- **Focus** — does not auto-open the dropdown; opens once the first
  fetch resolves with at least one option (or, if the prefix is empty
  and a previous result list exists, immediately opens with that
  list). The plan refines whether focus alone schedules an initial
  fetch with an empty query or whether the user must type at least
  `minQueryLength` chars.
- **Blur (strict-revert rule)** — when the input loses focus and the
  current visible text is not exactly the label of any currently-known
  option, the input reverts to `lastLabel` (the label of the last
  successfully picked option) or `initialLabel` if nothing has been
  picked yet, or empty if neither exists. The `value` signal is **not**
  modified by blur — it only changes on explicit picks (Enter, Tab-to-
  complete, click). Reverting the visible text without reverting the
  signal keeps strict semantics: the parent's notion of "what's
  picked" is exactly what the user explicitly chose, never what they
  typed-and-walked-away-from.

### Behaviour — clicks

- Clicking a dropdown option picks it: same effect as Enter on a
  highlighted item.
- Clicking outside the combobox closes the dropdown and triggers the
  blur path (strict-revert).

### DOM shape

```
<div class="combobox combobox-{size} [combobox-disabled] [combobox-open]"
     role="combobox"
     aria-haspopup="listbox"
     aria-expanded="..."
     aria-owns="combobox-list-{id}">
  <label class="combobox-label" for="combobox-input-{id}">…</label>

  <div class="combobox-field">
    <input class="combobox-input"
           id="combobox-input-{id}"
           type="text"
           role="combobox"
           autocomplete="off"
           aria-autocomplete="both"
           aria-controls="combobox-list-{id}"
           [aria-activedescendant="combobox-option-{id}-{idx}"]
           [disabled]>
    <!-- spinner visible while a fetch is in flight -->
    <span class="combobox-spinner" hidden=...></span>
  </div>

  <ul class="combobox-list"
      id="combobox-list-{id}"
      role="listbox"
      hidden=...>
    <li class="combobox-option [combobox-option-active]"
        id="combobox-option-{id}-{idx}"
        role="option"
        aria-selected="..."
        @click=...>…</li>
    <!-- when results are empty -->
    <li class="combobox-empty" aria-disabled="true">No results</li>
    <!-- when a fetch is in flight and no stale options yet -->
    <li class="combobox-loading" aria-busy="true">Loading…</li>
  </ul>
</div>
```

- A per-mount unique `id` is generated (random or counter-based — the
  plan picks) and used to wire `<label>`, `<input>`, `<ul>`, and
  `<li>` together via `for`, `id`, `aria-controls`, `aria-owns`, and
  `aria-activedescendant`.
- The `combobox-open` class is set whenever the dropdown is showing.
- The `combobox-disabled` class is set when the resolved disabled
  flag is true.
- `aria-busy="true"` is set on the `<ul>` while a fetch is in flight
  (in addition to the spinner element).

### Async / backend usage (illustrative, not prescriptive)

Combobox is presentation + interaction; the parent supplies the I/O.
The canonical wiring:

```ts
const value = signal("");

const loadUsers = async (query: string): Promise<ComboboxOption[]> => {
  const res = await fetch(`/api/users?q=${encodeURIComponent(query)}`);
  const rows = await res.json();
  return rows.map(u => ({ value: u.id, label: u.name }));
};

return html`
  ${Combobox({
    value,
    label: "Assignee",
    placeholder: "Type a name…",
    loadOptions: loadUsers,
  })}
`;
```

URL-restored value with display label:

```ts
const value = signal(initialUserId);
const initialLabel = initialUserName;   // also restored from the URL/session
return html`${Combobox({ value, initialLabel, loadOptions: loadUsers })}`;
```

Disable during a parent-driven async pause:

```ts
const value = signal("");
const busy = signal(false);
return html`${Combobox({ value, disabled: busy, loadOptions: loadUsers })}`;
```

`loadOptions` is whatever the parent wrote — `fetch`, `createHttp`,
GraphQL, an in-memory filter over a static array, anything that
returns `Promise<ComboboxOption[]>`. Combobox doesn't see it. The
component owns debouncing, in-flight tracking, race ordering, and
the keyboard/UI behaviour; the parent owns the actual network call
and the option-shape transform.

## Requirements

### Component

1. New file `.zero/components/Combobox.ts` exports a single default
   `Combobox` function matching `Component<ComboboxProps>`. The
   component runs once per mount; option-list reactivity is delivered
   via signals/effects internal to the component (the parent passes
   no options signal).

2. The component exports the `ComboboxOption`, `ComboboxSize`, and
   `ComboboxProps` types.

3. The `value` prop is a `Signal<string>`. The component reads
   `value.val` once on mount to seed `lastValue` and never writes to
   `value` except when an explicit pick happens (click, Enter, Tab-to-
   complete). Blur never writes to `value`.

4. The `loadOptions` prop is required. The component does not import
   from `"zero/http"`, does not call `fetch` directly, and prescribes
   no response shape beyond `Promise<ComboboxOption[]>`.

5. The component maintains internal signals:
   - `query: Signal<string>` — the current caret prefix.
   - `options: Signal<ComboboxOption[]>` — last successful result.
   - `highlight: Signal<number>` — currently highlighted option index,
     or -1 when nothing is highlighted.
   - `open: Signal<boolean>` — dropdown visibility.
   - `busy: Signal<boolean>` — fetch-in-flight indicator.
   - `lastLabel: Signal<string>` — label of the last successfully
     picked option, seeded from `props.initialLabel ?? ""`.
   These are framework `signal()`s scoped to the component and torn
   down when it unmounts.

6. Debounce / race-safe fetch:
   - A single `timeoutId` is tracked across keystrokes; each new
     keystroke clears any pending timeout and schedules a new one.
   - A monotonically increasing `serial` counter is tracked; each
     fetch captures its serial on dispatch and discards its result if
     a newer fetch has started since.
   - If `query.val.length < (props.minQueryLength ?? 1)`, no fetch is
     scheduled, `options` is set to `[]`, `busy.set(false)`, dropdown
     closes. (Whether focus with an empty query causes an initial
     fetch is left to the plan; spec defaults to "no fetch until the
     user types at least `minQueryLength` chars".)

7. Inline ghost completion:
   - When a fetch resolves with a non-empty list whose first item's
     label starts with `query.val` (case-insensitive), the component
     sets the underlying `<input>`'s `.value` to that label and calls
     `setSelectionRange(query.val.length, label.length)` to highlight
     the tail.
   - When no match starts with the query, the input's `.value` stays
     at `query.val` with no selection.
   - When the highlight moves via arrow keys, the same operation
     re-runs against the newly-highlighted option's label.
   - The setting of `.value` and selection range happens imperatively
     via a `ref` on the `<input>` (or via an `effect` that captures a
     reference recorded at mount time — the plan picks).

8. Keyboard handler attached to the input via `@keydown`:
   - Implements ArrowDown, ArrowUp, Enter, Escape, Tab per the
     Background section. Each handler reads `highlight.val` and
     `options.val` to decide its action.
   - Calls `e.preventDefault()` for ArrowDown, ArrowUp, Enter, and
     Tab-to-complete; never for printable characters.
   - For Enter and Tab-to-complete, calls the pick path:
     `value.set(option.value)`, `lastLabel.set(option.label)`,
     `open.set(false)`, fires `props.onChange?.(option.value, option)`,
     and clears `highlight`.

9. Focus / blur:
   - `@focus` on the input opens the dropdown if there are options to
     show or if a previous fetch resolved successfully.
   - `@blur` runs the strict-revert: if the current input `.value`
     does not equal the label of any option in `options.val`, the
     input's `.value` is set back to `lastLabel.val` (or "" if no
     pick has ever happened). `value` is not modified. `open.set(false)`.
   - Blur is also triggered by Escape (which closes the dropdown
     without picking; the next real blur applies the revert rule).

10. Outside-click handling:
    - While the dropdown is open, a document-level `mousedown` listener
      is registered via `effect()` (mirrors `Dialog`'s `keydown`
      pattern). If the event target is outside the component root,
      `open.set(false)` and the blur path runs. The effect's cleanup
      removes the listener when the component unmounts or the
      dropdown closes.

11. Disabled handling:
    - `read(props.disabled)` is the disabled function (using the same
      `read` + `isSignal` helper pattern as `Pagination`). It is read
      inside any reactive surface that depends on disabled — the
      input's `disabled` attribute binding, the outer class binding,
      and the keydown handler's early return.
    - When disabled, the input rejects keystrokes and clicks, the
      dropdown closes, and no fetch is scheduled.

12. The component must call `loadOptions` only when a new query
    arrives **and** `minQueryLength` is satisfied **and** the
    component is not disabled. It must not call `loadOptions` on
    mount (unless the plan finds a compelling UX reason for an
    initial empty-query fetch).

13. The component does **not** clamp, normalize, or otherwise mutate
    its `value` signal at any time except on explicit picks. Setting
    `value.val` externally has no side effect on the input's visible
    text (the input reflects `lastLabel`, seeded by `initialLabel`).
    External value updates without a matching `initialLabel` update
    therefore continue to display the old label — this is a
    deliberate tradeoff to keep the API tight and avoid surprise
    fetches.

14. The component renders an empty-state `<li class="combobox-empty">`
    in the dropdown when `options.val.length === 0` and the most
    recent fetch has resolved (i.e. not while still loading). Text
    is `props.noResultsLabel ?? "No results"`.

15. The component renders a loading-state `<li class="combobox-loading">`
    in the dropdown when `busy.val === true` and `options.val` is
    empty (e.g. first-ever fetch). Text is
    `props.loadingLabel ?? "Loading…"`. A separate visible spinner
    glyph also appears inside `.combobox-field` whenever
    `busy.val === true`, regardless of whether stale options are
    showing.

### CSS

16. New partial `.zero/styles/components/_combobox.scss`. Every rule
    sits inside `@layer components { ... }`.

17. Token-only values. Spacing from `--space-*`, radii from
    `--radius-*`, colors from `--color-*`, fonts from `--font-*`,
    borders from `--border-*`. No hex codes, no magic numbers except
    `transition-duration`, `opacity`, `z-index` (for the dropdown
    overlay), and spinner animation timings. The dropdown's
    `z-index` may reuse an existing constant from another component
    partial (e.g. `Dialog`'s backdrop layer) — the plan picks.

18. `.combobox` is `position: relative` so the dropdown can absolute-
    position underneath the input. `.combobox-field` is a flex row
    holding the input and the inline spinner; the spinner sits
    inside the input's inline-end padding.

19. `.combobox-input` mirrors `.input`'s padding, border, radius,
    font, and focus treatment so a Combobox visually matches an
    Input next to it. The plan reuses `Input`'s padding/font/border
    rules (either via a SCSS `@extend` if grass supports it, or by
    duplicating — the plan picks; framework-wide policy is no magic
    numbers, and copying tokens stays within that rule).

20. Size classes mirror Input's sizing:
    - `.combobox-sm .combobox-input { padding ...; font-size: var(--font-size-sm); }`
    - `.combobox-md .combobox-input { padding ...; font-size: var(--font-size-md); }`
    - `.combobox-lg .combobox-input { padding ...; font-size: var(--font-size-lg); }`
    Exact values match `Input`'s existing sizing.

21. `.combobox-list` is `position: absolute`, full inline-size of the
    field, sits below it with a small `margin-block-start`, has a
    background of `var(--color-surface)`, a border, a radius, and a
    box-shadow from the design tokens. `max-block-size` is a sane
    cap (e.g. `min(20rem, 60vh)`) with `overflow-y: auto` so long
    result lists scroll. The exact max-size cap is for the plan; it
    is one of the documented exceptions to the magic-number rule
    alongside `z-index`.

22. `.combobox-option` is a clickable list item with padding,
    `cursor: pointer`, and a hover background using the same surface
    pairing as `Button.ghost`'s hover (`var(--color-surface)` ↔
    `var(--color-bg)`). The plan picks the exact tokens.

23. `.combobox-option-active` (the keyboard-highlighted option) and
    `.combobox-option:hover` share a single visual treatment — a
    distinct background (`var(--color-primary)` with text in
    `var(--color-primary-fg)` is the safest pairing, matching
    `Button.primary`; the plan can pick a softer treatment if the
    primary contrast feels heavy).

24. `.combobox-empty` and `.combobox-loading` use
    `var(--color-text-muted)`, italic or normal style (plan picks),
    `cursor: default`, no hover effect.

25. `.combobox-spinner` is an inline `<span>` with a CSS-only spin
    animation (no SVG). Sized to fit inside the input's inline-end
    padding without enlarging the field. Hidden when not busy via
    the native `hidden` attribute (driven by a reactive binding on
    `busy.val`).

26. `.combobox-disabled` lowers `opacity` and applies
    `cursor: not-allowed`; `.combobox-disabled .combobox-input` is
    additionally non-interactive via the native `disabled` attribute
    (so this is mostly aesthetic, not behavioural).

27. `.combobox-label` mirrors `.select-label` / `.input-label` —
    `display: block`, small font, medium weight, small
    `margin-block-end`.

28. Logical properties only — no `left`/`right` in rules that affect
    text direction (`padding-inline`, `inset-inline`,
    `margin-inline-*`, etc.).

29. No `!important`. Override via the unlayered cascade per the
    existing `@layer components` convention.

### Scaffold registration

30. New `TPL_*` constants for:
    - `.zero/components/Combobox.ts`
    - `.zero/components/Combobox.test.ts`
    - `.zero/styles/components/_combobox.scss`

31. `framework_manifest()` gains three entries. Existing length-
    coupled assertions in `crates/zero-scaffold/src/lib.rs` tests and
    any `tests/update*.rs` paths that hard-code the manifest length
    are bumped accordingly. The plan enumerates the exact assertions.

32. `.zero/components/index.ts` gains two lines:
    - `export { default as Combobox } from "./Combobox.ts";`
    - `export type { ComboboxOption, ComboboxProps, ComboboxSize } from "./Combobox.ts";`
    Ordering follows the file's existing alphabetical-by-component
    convention (after `Checkbox`, before `Dialog`).

33. `.zero/styles/_components.scss` gains one line:
    `@use 'components/combobox';` inserted in alphabetical position
    (between `checkbox` and `dialog`).

34. `.zero/components.d.ts` (the module declaration for
    `"zero/components"`) gains a `Combobox` entry and the
    `ComboboxOption` / `ComboboxProps` / `ComboboxSize` type exports.
    Exact shape is for the plan.

35. The editor `tsconfig.json` emitted by `zero init` requires no
    changes — `"zero/components"` already resolves to
    `.zero/components/index.ts`, and Combobox rides on that
    resolution.

### Tests

36. New file `.zero/components/Combobox.test.ts` exercising:
    - **Renders the base markup.** With `value: signal("")` and a
      `loadOptions` returning `[]`, `find(el, ".combobox")`,
      `find(el, ".combobox-input")`, and `find(el, ".combobox-list")`
      succeed. The list is hidden initially.
    - **Typing triggers a debounced fetch.** Set
      `debounceMs: 50, minQueryLength: 1`. Fire an `input` event
      setting the input to `"f"`. Advance the test clock 50 ms (or
      use a `setTimeout(resolve, 60)`-based test helper — see
      `Pagination.test.ts` for the convention if it exists; the plan
      picks the mechanism). Assert `loadOptions` was called once
      with `"f"`.
    - **`minQueryLength` gates the fetch.** With `minQueryLength: 2`,
      typing `"f"` triggers no fetch; typing `"fo"` does.
    - **Race safety.** Two fetches dispatched in order with the
      second resolving first. Only the second's results render; the
      first's resolution is dropped. Use a `loadOptions` spy that
      returns hand-controlled promises.
    - **Ghost completion.** With `loadOptions` returning
      `[{ value: "foobar", label: "foobar" }]`, type `"foo"`. After
      the fetch resolves, the input's `.value` is `"foobar"` and
      `selectionStart === 3, selectionEnd === 6`.
    - **ArrowDown / ArrowUp move highlight and update ghost.** With
      three matching options, ArrowDown highlights index 0 and
      ghosts label 0; ArrowDown highlights index 1 and ghosts label 1;
      ArrowUp returns to index 0.
    - **Enter accepts the highlight.** Fires `value.set(option.value)`
      and `onChange` spy with `(value, option)`. Dropdown closes.
      Input visible value equals the picked label, no selection.
    - **Tab-to-complete.** With a ghost showing (typed `"foo"`,
      matched `"foobar"`), firing a Tab `keydown` accepts the match —
      same effect as Enter. Without a ghost (typed text exactly
      matches no option), Tab closes the dropdown but does not pick.
    - **Escape closes without picking.** Dropdown closes; `value`
      unchanged; `onChange` not called.
    - **Blur strict-revert.** Type `"xyz"` with no matching option.
      Fire `blur`. Input visible value reverts to `lastLabel.val`
      (or `initialLabel` if provided, or `""`). `value` signal
      unchanged.
    - **Click on a dropdown option picks it.** Same effect as Enter.
    - **`initialLabel` displays until first pick.** With `value:
      signal("u-42"), initialLabel: "Alice"`, the input's visible
      value is `"Alice"` on mount. After picking a different option,
      the visible value becomes the new label.
    - **Disabled (plain boolean).** With `disabled: true`, the input
      has the `disabled` attribute; typing fires no input events; no
      fetch is scheduled.
    - **Disabled (signal).** With `disabled: signal(false)`, the
      input is enabled. After `disabled.set(true)`, the input gains
      the `disabled` attribute and the dropdown (if open) closes.
    - **No-results state.** With `loadOptions` returning `[]` for
      query `"xyz"`, the dropdown contains a `.combobox-empty`
      element with the configured `noResultsLabel`.
    - **Loading state.** While a `loadOptions` promise is pending
      after the debounce fires, `find(el, ".combobox-spinner")` is
      not hidden; `find(el, ".combobox-loading")` is present in the
      dropdown when `options` is empty. Once the promise resolves,
      both clear.
    - **Size variant.** With `size: "sm"`, outer wrapper carries
      `combobox-sm`. Default carries `combobox-md`.
    - **`onChange` semantics.** Fired exactly once per pick. Not
      fired on Escape, blur, or no-op interactions. Receives both
      the value string and the option object.
    - `afterEach(cleanup)`.

37. The new test ships in the manifest (lives in
    `.zero/components/`) and runs:
    - Inside the framework's `showcase/` via
      `tests/component_library.rs`.
    - Inside every user project's `zero test` (consistent with the
      existing component-test ship-along policy from Phase 9).

38. The test mechanism for debounce timing (waiting for real
    `setTimeout` vs. injecting a fake clock) is for the plan. The
    spec proposes: use real `setTimeout` with small `debounceMs`
    values (e.g. `debounceMs: 5`) and small `await new Promise(r =>
    setTimeout(r, 20))` waits — keeps tests honest about real
    behaviour, costs ~50 ms total per assertion. Fake clocks are
    avoided framework-wide.

### Showcase

39. New route file `showcase/src/routes/combobox.ts` rendering:
    - **Static dataset Combobox.** A `loadOptions` that filters an
      in-memory list (e.g. 30 country names) by case-insensitive
      prefix and returns within a `setTimeout(resolve, 120)` so the
      typeahead behaviour, loading spinner, and ghost completion are
      observable. Demonstrates the default `md` size with no
      `initialLabel`.
    - **Size variants.** Two more instances, `sm` and `lg`, against
      the same static dataset. Pair them with their current `value`
      shown in a reactive text binding beneath each so users can see
      the picked value updating.
    - **`initialLabel` example.** A fourth instance pre-populated
      with `value: signal("us")` and `initialLabel: "United States"`,
      demonstrating the restore-from-URL pattern. Below the input, a
      small "Reset" button calls `value.set("")` and resets the
      input's visible state — the plan picks the exact mechanism
      (probably remount via a `key` signal, since the component
      does not auto-sync visible text to external value changes per
      Requirement 13).
    - **Async / mocked-backend example.** A fifth instance with a
      `loadOptions` that hits a fake backend via a
      `setTimeout(resolve, 500)` + jittered results, paired with a
      `disabled: busy` signal that the route flips to `true` while
      the parent simulates an unrelated in-flight operation. A
      comment in the source explicitly notes that a real app would
      call its own fetch logic here.
    - Each instance shows the current `value.val` below the pager
      via a reactive text binding for clarity.

40. `showcase/src/app.ts` registers
    `app.route("/combobox", () => import("./routes/combobox"))`.
    Position consistent with existing alphabetical ordering of
    routes.

41. `showcase/src/routes/home.ts` navigation cluster gains a
    `Combobox` link, alphabetically positioned.

42. The showcase's committed `.zero/components/Combobox.ts` (and
    partial + test) matches the manifest. `zero update --yes` from
    inside `showcase/` produces zero drift.

### Integration tests

43. `tests/showcase_build.rs` continues to pass against the new
    route. The plan verifies whether any per-route assertion exists
    that needs widening.

44. `tests/showcase_dev.rs` continues to pass.

45. `tests/component_library.rs` continues to pass and now includes
    `Combobox.test.ts` in its run. The plan verifies whether the
    test asserts a specific test count that needs bumping.

### Documentation

46. `crates/zero-scaffold/src/scaffold/AGENTS.md` `## Components`
    section gains a `Combobox` entry in the component-roster table.
    The relevant category subsection (likely "Form Inputs"
    alongside `Input` / `Select`) gains a one-instance usage example.

47. `docs/components.md` is updated:
    - The component-count language ("sixteen components") is bumped
      to "seventeen components".
    - The summary table gains a new row for `Combobox` with its
      required props and an example, in alphabetical position
      (after `Checkbox`, before `Dialog`).

48. `zero-framework-spec.md` §11 — `"zero/components"` listing gains
    a `Combobox({...})` line in the Form Inputs group.

49. `zero-framework-spec.md` §12 — Phase-9 component-count line is
    bumped, and the parenthetical list adds `Combobox`. The original
    "combobox, autocomplete" out-of-scope line is removed.

50. `zero-framework-spec.md` §13 (Key Design Decisions Summary) may
    gain a row for the Combobox's loadOptions seam vs. the parent-
    owns-everything signal pattern used by Pagination — the plan
    judges whether this distinction is worth recording at the spec
    level or is sufficiently captured by the per-component spec.

## Constraints

- **No new Rust dependencies.** Rides on the existing `grass` SCSS
  pipeline, the existing transpiler, the existing scaffold + manifest
  plumbing.
- **No new npm dependencies.** Framework-wide.
- **No new top-level `"zero"` runtime exports.** Combobox is exposed
  only via `"zero/components"`.
- **`@layer components` for all CSS rules.** Unlayered user CSS in
  `styles/app.scss` overrides without `!important`.
- **Tokens only — no magic numbers or hex codes.** Standard
  exceptions for `opacity`, `transition-duration`, `z-index`, the
  dropdown's `max-block-size` cap, and spinner animation timings.
- **Stateful prop is a signal.** `value: Signal<string>`. One
  configuration prop accepts signal-or-plain so async parents can
  disable mid-flight: `disabled?: Signal<boolean> | boolean`.
  Everything else is a plain value read once at mount.
- **No backend awareness inside the component.** Combobox does not
  import from `"zero/http"`, does not assume a response shape beyond
  `ComboboxOption[]`, does not assume REST/GraphQL/anything. Parent
  does all I/O via `loadOptions`.
- **Strict single-select.** No free-text mode, no multi-select, no
  uncontrolled mode. Picking is the only way to update `value`.
- **Pages — uh, options — are zero-indexed internally** (for
  `highlight` and array operations) but **values are arbitrary
  strings owned by the parent's domain model** (user IDs, country
  codes, whatever — the component does not interpret them).
- **No internal state outside what's derivable.** The component
  maintains a few internal signals (query, options, highlight, open,
  busy, lastLabel) for its UI behaviour, but never holds a copy of
  `value` — that lives in the parent's signal.
- **Decoupled from any backend abstraction.** Combobox does not
  ship a `createComboboxSource()` helper, does not integrate with
  `zero/http` directly, and the framework's HTTP layer requires no
  changes.
- **No web components, no scoped styles, no CSS-in-JS.** Framework-
  wide.
- **One styled form.** No headless variant. Users needing a
  different look fork into `src/components/`.
- **Framework-owned.** Lives under `.zero/`. `zero update` refreshes
  it.

## Out of Scope

- **Multi-select.** No `values: Signal<string[]>` mode, no chips
  inside the input, no `multiple?: boolean` switch. Deferred to a
  potential future `MultiCombobox` or `Combobox` extension.
- **Free-text mode.** No `allowFreeText?: boolean`. Typed input
  that doesn't match an option is always discarded on blur. Users
  needing free text reach for `Input`.
- **Custom option rendering.** No `renderOption?: (option) =>
  TemplateResult` slot. Options render as plain text from
  `option.label`. Users needing icons / two-line items / etc. fork.
- **Grouped options.** No section headers in the dropdown. The
  `ComboboxOption` shape is flat. Grouping is a fork.
- **Static filter-only mode.** No special-cased "synchronous,
  filter-a-local-array" path. The user supplies a `loadOptions` that
  returns a resolved Promise; that's the entire contract. The
  showcase's static example demonstrates the idiom.
- **Built-in fetch / HTTP helpers.** No `createComboboxSource()`, no
  `usePagedFetch()`-style adapter, no integration with `zero/http`.
  Documentation and showcase demonstrate the wiring pattern; the
  framework ships no helper.
- **Auto-abort on query change.** Combobox does not own an
  `AbortController` and does not abort in-flight fetches when the
  query changes. The race-safety guarantee is at the result-handling
  layer (latest-serial-wins), not at the network layer. Parents
  who want true abort semantics can wrap their fetch with one
  themselves.
- **Persistence across navigation / reload.** Parent's job (URL
  path params, URL search params, localStorage, etc.).
- **Form validation integration.** No `error?: string` prop, no
  invalid-state ARIA. Validation is the parent's job, mirroring
  `Input`'s contract.
- **Caching of past queries.** Each query is a fresh fetch.
  Caching is the parent's concern (memoize their `loadOptions` if
  desired).
- **`aria-live` announcements** for result counts / loading
  transitions. Out of scope; future a11y polish.
- **Touch / mobile virtual-keyboard polish.** Out of scope; the
  native `<input>` and the design-token sizing should be acceptable
  on touch devices but no special-cased handling ships.
- **Snapshot tests.** `expect().toMatchSnapshot()` is not
  implemented in `zero/test`. Tests assert on DOM selectors and
  signal values.
- **A standalone Combobox package.** No npm publication.

## Open Questions

- **Initial fetch on focus with empty query.** Spec says no fetch
  until the user types `minQueryLength` chars. Alternative: focus
  alone triggers a `loadOptions("")` to populate the dropdown
  immediately (useful for "show top 10 results by default"
  patterns). The plan picks; if the alternative is chosen, the
  decision should be reflected in a prop (e.g.
  `fetchOnFocus?: boolean`, default `false`) rather than hard-
  wired.
- **ArrowUp wrap-around.** Spec proposes wrap (last → first, first
  → last). Alternative: no wrap (no-op at boundaries). The plan
  picks; wrap is more common in combobox implementations.
- **Tab-to-complete behaviour.** Spec accepts the highlight as a
  pick. Alternative: Tab just closes the dropdown and moves focus
  per native behaviour (the parent can still pick the visible ghost
  via Enter). The plan picks; Tab-to-complete is more idiomatic but
  slightly trickier to implement correctly without breaking native
  focus order.
- **Ghost case-sensitivity.** Spec uses case-insensitive
  startsWith. Alternative: locale-aware via
  `String.prototype.localeCompare`. Spec proposes the cheap form;
  the plan can revisit if i18n becomes a project concern.
- **Highlight default index after a fetch.** Spec implicitly
  assumes highlight = 0 after a successful fetch (matches the ghost
  shown). Alternative: highlight = -1 by default, only set to 0 by
  ArrowDown. Spec proposes 0 because the ghost is already showing
  index 0's label; mismatched state would be confusing.
- **Outside-click listener timing.** Spec proposes a `mousedown`
  document listener while open. Alternative: `pointerdown` for
  touch parity. The plan picks.
- **Dropdown positioning.** Spec absolute-positions below the
  input. No flip-to-top when the dropdown would overflow the
  viewport. Alternative: implement a flip via measuring the input's
  position and the dropdown's height before showing. Spec defers
  this to a future a11y/UX polish; the plan can include it if
  cheap.
- **`combobox-list` `z-index` constant.** Spec proposes reusing
  `Dialog`'s backdrop stacking context. The plan picks the exact
  number and either inlines it or factors it into a token.
- **Whether to expose `ComboboxOption` as an alias of
  `SelectOption`.** Spec proposes a duplicated structural type to
  keep Combobox self-contained. Alternative: literally
  `export type { SelectOption as ComboboxOption } from "./Select.ts"`.
  The plan picks; duplication is the path of least coupling.
- **Showcase reset mechanism for the `initialLabel` instance.**
  Spec hints at remount via a `key` signal. Alternative: expose a
  `reset()` method on the component (breaks the plain-function
  contract). The plan picks; remount is consistent with framework
  patterns.
- **AGENTS.md / docs grouping.** Where Combobox sits in the
  component-category taxonomy — almost certainly Form Inputs
  alongside `Input` / `Select`. The plan confirms.
- **Manifest size assertion.** The plan confirms the exact current
  number after the Pagination addition and bumps the test by three.
- **Async test waits.** Spec proposes real `setTimeout` waits with
  tiny `debounceMs`. The plan confirms or proposes a fake-clock
  helper if one already exists in `zero/test`.
- **Whether `Combobox` should also expose its options-result count
  via an `onResults?: (options: ComboboxOption[]) => void`
  callback** for parents that want to render a "showing N of M"
  summary. Spec says no (out of scope per minimal API surface);
  the plan can revisit if the showcase or a downstream user finds
  it load-bearing.
