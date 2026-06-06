---
title: Components
nav_order: 6
---

# Components

Components in zero are plain functions. There is no `class
Component`, no lifecycle methods, no `this`. A component is a
function that returns a `TemplateResult`, and the framework
handles the rest.

## Components are functions

```ts
import { html, signal } from "zero";

export default function Counter() {
  const count = signal(0);
  return html`
    <button @click=${() => count.update(n => n + 1)}>
      Clicked ${count} times
    </button>
  `;
}
```

The function body runs **once**, when the framework commits the
component. Any `signal`, `computed`, or `effect` calls made
during that run register against the current scope; when the
scope tears down (the component leaves the tree), they dispose
together.

After commit, the framework keeps the function around for nothing
— there is no re-render. Reactive updates happen at the granular
binding sites inside the template.

## Props

A component takes a single argument: a plain `props` object. You
call the component like any function, and the framework treats
the returned `TemplateResult` like any other value.

```ts
function UserCard(props: { name: string; age: number }) {
  return html`
    <div class="card">
      <h2>${props.name}</h2>
      <p>Age: ${props.age}</p>
    </div>
  `;
}

// usage
html`<div>${UserCard({ name: "Alice", age: 30 })}</div>`;
```

Props are not magically reactive. A static prop is just a value
captured in the closure of the component's body — it never
changes after the function returns. **A signal prop, however,
stays reactive through the boundary:**

```ts
function Greeting(props: { name: Signal<string> }) {
  return html`<h1>Hello ${props.name}</h1>`;
}

function Parent() {
  const name = signal("Alice");
  return html`
    <div>
      ${Greeting({ name })}
      <input value=${name} @input=${(e: Event) =>
        name.set((e.target as HTMLInputElement).value)} />
    </div>
  `;
}
```

The `<h1>` text updates as the user types — no re-render, the
text-node binding inside `Greeting` follows the signal.

## Children and slots

Children are not a special concept. They're a prop. You call them
whatever you like and put them wherever the template needs them.

```ts
function Card(props: { title: string; children?: TemplateResult }) {
  return html`
    <article class="card">
      <h2>${props.title}</h2>
      <div class="card-body">${props.children}</div>
    </article>
  `;
}

Card({
  title: "Welcome",
  children: html`<p>Hello from inside a card.</p>`,
});
```

Multiple slots are just multiple props:

```ts
function Page(props: {
  header?: TemplateResult;
  sidebar?: TemplateResult;
  children?: TemplateResult;
}) {
  return html`
    <div class="page">
      <header>${props.header}</header>
      <aside>${props.sidebar}</aside>
      <main>${props.children}</main>
    </div>
  `;
}
```

## Composition

Components compose by calling each other. There's no JSX
compilation step and no special wrapper — the call is just a
function call.

```ts
function App() {
  return html`
    ${Page({
      header: Header(),
      sidebar: Sidebar(),
      children: Main(),
    })}
  `;
}
```

The `T01` lint rule flags components that are referenced by JSX-
style PascalCase tags (`<Foo />`) without being invoked, which is
a common mistake when porting from React. See
[Linting](./linting.html).

## Component library reference

zero ships eighteen production-ready components under the bare
specifier `"zero/components"`. They use the design system tokens
covered in [Theming](./theming.html), so they take on your brand
once you redefine the public tokens.

| Component  | Required props (summary)                                                   | Example                                                                          |
|------------|----------------------------------------------------------------------------|----------------------------------------------------------------------------------|
| `Avatar`   | `alt`; optional `src`, `initials`, `size`                                  | `Avatar({ alt: "Ada", initials: "A", size: "md" })`                              |
| `Badge`    | optional `variant`, `size`, `children`                                     | `Badge({ variant: "success", children: "New" })`                                 |
| `Button`   | optional `variant`, `size`, `type`, `form`, `name`, `value`, `disabled`, `loading`, `onClick`, `children` | `Button({ variant: "primary", onClick: save, children: "Save" })`                |
| `Card`     | optional `variant`, `title`, `children`                                    | `Card({ title: "Profile", children: html\`<p>…</p>\` })`                         |
| `Checkbox` | `checked: Signal<boolean>`; optional `label`, `disabled`, `debounceMs`, `error`, `autofocus`, `attrs` | `Checkbox({ checked: agreed, label: "I agree" })`                                |
| `Combobox` | `value: Signal<string>`, `loadOptions: (q) => Promise<ComboboxOption[]>`; optional `initialLabel`, `size`, `placeholder`, `label`, `disabled`, `debounceMs`, `minQueryLength`, `noResultsLabel`, `loadingLabel`, `onChange`, `allowCustom`, `error`, `autofocus`, `attrs` | `Combobox({ value, loadOptions: loadUsers })` |
| `Dialog`   | `open: Signal<boolean>`; optional `size`, `title`, `children`, `onClose`   | `Dialog({ open, title: "Confirm", children: html\`…\` })`                        |
| `Drawer`   | `open: Signal<boolean>`, `side`; optional `mode`, `size`, `title`, `body`, `controls` | `Drawer({ open, side: "right", mode: "push", title: "Edit user", body: form })`  |
| `Input`    | `value: Signal<string>`; optional `type`, `size`, `placeholder`, `label`, `debounceMs`, `onChange`, `error`, `autofocus`, `attrs` | `Input({ value: name, label: "Name", type: "text" })`                            |
| `Pagination` | `page: Signal<number>`, `totalPages: Signal<number> \| Computed<number> \| number`; optional `size`, `siblingCount`, `boundaryCount`, `disabled`, `onChange`, `summary` | `Pagination({ page, totalPages: 10 })`                                          |
| `Radio`    | `selected: Signal<string>`, `name`, `value`; optional `label`, `debounceMs`, `error`, `autofocus`, `attrs` | `Radio({ selected: choice, name: "size", value: "lg", label: "Large" })`         |
| `Select`   | `value: Signal<string>`, `options: SelectOption[]`; optional `label`, `debounceMs`, `onChange`, `error`, `autofocus`, `attrs` | `Select({ value: country, options: [{ value: "us", label: "USA" }] })`           |
| `Spinner`  | optional `variant`, `size`, `label`                                        | `Spinner({ size: "lg", label: "Loading" })`                                      |
| `Tabs`     | `active: Signal<string>`, `tabs`, `panels`                                 | `Tabs({ active, tabs: [...], panels: { ... } })`                                 |
| `Table`    | `columns`, `rows: Signal<T[]>`, `rowKey`; optional `density`, `loading`, `sort`, `onSortChange` | `Table({ columns, rows, rowKey: r => r.id })`                                    |
| `TextArea` | `value: Signal<string>`; optional `rows`, `placeholder`, `label`, `debounceMs`, `error`, `autofocus`, `attrs` | `TextArea({ value: notes, rows: 5, label: "Notes" })`                            |
| `Toast`    | `open: Signal<boolean>`, `message`; optional `variant`, `duration`         | `Toast({ open, message: "Saved", variant: "success" })`                          |
| `Toggle`   | `checked: Signal<boolean>`; optional `label`, `disabled`, `debounceMs`, `error`, `autofocus`, `attrs` | `Toggle({ checked: darkMode, label: "Dark mode" })`                              |

The convention across the library:

- **State-shaped props use signals.** `checked`, `value`,
  `active`, `selected`, `open`, `rows` are all
  `Signal<...>` — the component subscribes to read and calls
  `.set()` to write. The parent owns the signal; the component
  reads and writes through it.
- **Display props are plain values.** `variant`, `size`,
  `label`, `placeholder`, `disabled` are strings/booleans/numbers,
  not signals — these typically don't need to change after the
  component commits, and the type-checker won't let you pass a
  signal where a string is expected. If you really need a
  reactive `disabled`, wrap the whole component call in a
  reactive block.
- **Callbacks are optional.** Toasts/dialogs/buttons expose
  optional `onClick` / `onClose` / `onDismiss` callbacks for
  imperative side-effects.
- **Children are templates.** Where a component accepts children,
  it accepts `TemplateResult` or a `string`. Pass `html\`…\``
  for arbitrary markup.

Signal-writing components (`Input`, `TextArea`, `Checkbox`, `Toggle`,
`Radio`, `Select`) accept `debounceMs?: number` to delay the signal
write by N milliseconds on the trailing edge, mirroring the
`@event.debounce:<ms>` template modifier for cases where the inner
control's event slot isn't reachable through the component API.
Defaults to `0` (synchronous). For `Input`/`TextArea` the DOM shows
the typed text immediately while the signal still holds the previous
value during the window; an external write to the signal in that
window still re-renders immediately, exactly as it does without
`debounceMs` — not a new footgun. For the click-driven components
(`Checkbox`/`Toggle`/`Radio`/`Select`) the control toggles in the DOM
immediately and only the signal write is delayed, so the visible state
and the signal diverge during the window — usually you want to debounce
the downstream effect, not the signal write. Note that
`Combobox.debounceMs` means something different (the gap before
`loadOptions` runs after the last keystroke), so the same prop name
carries a different meaning on that component.

`Input` and `Select` also accept `onChange?: (value: string) => void`,
invoked with the new value after each user edit (after the signal
write, inside the same `debounceMs` window). Reach for it whenever you
need to *react* to an edit — fire a search query, sync external state —
instead of watching the `value` signal with an `effect`. The effect
route is a footgun when the app also writes the signal from outside
(reset, query-param sync): the obvious two-effect bidirectional mirror
silently reverts user edits because the sync effect subscribes to the
mirror it compares against. A direct callback, like `Button.onClick`,
has no such failure mode. `onChange` fires only on user edits, never on
programmatic `value.set(...)` calls.

Every form control — `Input`, `Select`, `TextArea`, `Checkbox`,
`Radio`, `Toggle`, `Combobox` — accepts
`error?: Signal<string | null>`. While the signal holds a message the
control renders it below itself in a `<small class="text-muted">`
carrying a stable `data-field-error` attribute (the test hook), sets
`aria-invalid="true"` on the underlying control, and links the message
via `aria-describedby`. When the signal is `null` (or the prop is
omitted) nothing renders and `aria-invalid` is `"false"`. Bind
`form.fields.<name>.error` from [`createForm`](#forms) directly to this
prop — or any `Signal<string | null>` you manage yourself.

The same seven controls also accept two native-element props:

- **`autofocus?: boolean`** — the component focuses its underlying
  element each time that element is committed to the DOM: on first
  render, and again every time a dialog/drawer re-opens and re-renders
  its body. The first field of a panel form takes focus on every open,
  with no raw `<input>` fallback or hand-rolled ref plumbing:

  ```ts
  Dialog({
    open,
    title: "Add part",
    children: html`<form>
      ${Input({ value: name, label: "Name", autofocus: true })}
      …
    </form>`,
  });
  ```

- **`attrs?: Record<string, string | number | boolean>`** — additional
  native attributes for the underlying element (`name`,
  `autocomplete`, `min`/`max`, `data-*`, …). Applied **additively**:
  any attribute the component renders itself (`class`, `type`,
  `aria-invalid`, `Radio`'s `name`, …) wins, and the colliding key is
  skipped — `attrs` extends the element, it never fights the
  component. `true` sets an empty attribute (`required: true` →
  `required=""`), `false` skips the key, numbers are stringified.
  Values are plain (not reactive) and there is no event-handler
  smuggling — use the component's callbacks for behavior.

For the label-wrapped controls (`Checkbox`, `Radio`, `Toggle`) and
`Combobox`, both props target the inner `<input>`, not the wrapper.
Both apply in a microtask after the element commits, so a test
asserting on them must `await Promise.resolve()` first.

Props typed `Signal<T> | T` accept a `Computed<T>` too where noted:
`Pagination.totalPages`, `Pagination.disabled`, and
`Combobox.disabled`. Pass a plain value when it's static, a `Signal`
when the parent mutates it, or a `Computed` when it's derived from
other reactive state (e.g.
`computed(() => Math.ceil(totalCount.val / pageSize))`).

The signatures above are the public surface; the source of truth
is `crates/zero-scaffold/src/scaffold/.zero/components.d.ts`,
copied into your project as `.zero/components.d.ts` by `zero
init` / `zero update`.

For component patterns in larger apps — when to reach for the
shipped components vs. raw HTML, how to compose them, when to
build your own — see [Best Practices §7](./best-practices.html#7-component-usage).

## Button

Beyond `variant` / `size`, `Button` exposes the native submit-button
surface so it can drive real forms:

- **`type`** — `"button" | "submit" | "reset"`. Defaults to
  `"button"`, so a `Button` **never** accidentally submits an enclosing
  `<form>`. Pass `type: "submit"` explicitly for a submit button. (This
  is the design-system convention; a bare `<button>` in a form defaults
  to `submit`, but `Button` does not.)
- **`form`** — the `id` of the `<form>` this button submits. Lets you
  render the submit button *outside* its form — e.g. a form in a
  drawer's `body` slot with its Save/Cancel buttons in the `controls`
  slot:

  ```ts
  html`
    <form id="edit-form">${/* fields */}</form>
    ${Button({ type: "submit", form: "edit-form", children: "Save" })}
  `
  ```
- **`name` / `value`** — the button's form-control name and value, so a
  multi-button form's submit handler can tell which button fired.

`form`, `name`, and `value` are emitted only when provided; an omitted
prop renders no attribute. `loading` renders the leading spinner **and**
makes the button non-interactive — it sets the native `disabled`
attribute and short-circuits `onClick` — so a "Saving…" button can't
double-submit or re-fire while busy.

## Drawer

`Drawer` is an edge-anchored side panel. It is a pure visual container:
three caller-owned slots (`title`, `body`, `controls`), no built-in close
affordances, no focus trap, no scroll lock. The only stateful prop is
`open`; the parent owns close. The DOM is always mounted so CSS animates
both the open and the close. `mode` is `"overlay"` (fixed over content,
with a non-interactive backdrop) or `"push"` (an in-flow flex sibling
whose side-axis size animates from `0`, so the parent layout reflows).

The intended shape is **one drawer per side, mounted once in the root
layout, used as a singleton context surface**: the panel stays put while
the caller swaps its contents reactively. `open` is typically a `computed`
over context signals, and each slot is a `() => …` function that switches
on which context is active.

```ts
// Shape A — context-driven forms. Several actions open the same drawer.
const editingUser = signal<User | null>(null);
const addingProduct = signal(false);
const open = computed(() => editingUser.val !== null || addingProduct.val);

Drawer({
  open,
  side: "right",
  mode: "push",
  title: () => (editingUser.val ? "Edit user" : addingProduct.val ? "Add product" : null),
  body: () => {
    const u = editingUser.val;
    if (u) return EditUserForm({ user: u });
    return addingProduct.val ? AddProductForm() : null;
  },
  controls: () => html`${Button({ variant: "ghost", children: "Cancel", onClick: clear })}`,
});
```

```ts
// Shape B — inspector over a table. Push mode is load-bearing here.
const selectedRow = signal<Row | null>(null);
const open = computed(() => selectedRow.val !== null);

Table({ columns, rows, rowKey: r => r.id, onRowClick: r => selectedRow.set(r) });
Drawer({ open, side: "right", mode: "push", /* title/body read selectedRow */ });
```

Push mode renders **no backdrop**, ever — the underlying content stays
fully interactive. That is what lets the inspector pattern work: with the
drawer open you can click a different table row, and the body swaps to the
new record without the drawer closing first. An overlay backdrop would
intercept those clicks.

Push mode only reflows when the drawer is a flex/grid child along the
relevant axis — mount it as a sibling of your content inside a `cluster`
(for `left`/`right`) or `stack` (for `top`/`bottom`). This is a usage
requirement, documented but not enforced at runtime: dropped outside such
a parent, a push drawer simply sits in normal flow and pushes nothing.

## Table sort

`Table` supports per-column sort. Mark a column with `sortable: true`
and pass a `sort` signal that the parent owns:

```ts
import { signal } from "zero";
import { Table } from "zero/components";
import type { SortState, TableColumn } from "zero/components";

const sort = signal<SortState | null>(null);
const columns: TableColumn<User>[] = [
  { key: "name",  label: "Name",  sortable: true },
  { key: "score", label: "Score", sortable: true, align: "end" },
];

// Client-side: Table sorts a copy of `rows` itself.
Table({ columns, rows, rowKey, sort });

// Server-side: pass onSortChange. Table emits intent; the parent
// re-fetches and updates `rows`. Table does not reorder locally.
Table({ columns, rows, rowKey, sort, onSortChange: (next) => refetch(next) });
```

| Mode         | `onSortChange` set | Behavior                                                                |
|--------------|--------------------|-------------------------------------------------------------------------|
| Client-side  | No                 | `sort.set(next)`; renders `rows` reordered via column `compare` or default. |
| Server-side  | Yes                | `sort.set(next)` then `onSortChange(next)`; renders `rows` as the parent provides. |

Clicking a sortable header cycles **asc → desc → unsorted** on the active
column. Clicking a different sortable column resets to asc on the new
column.

The default comparator handles numbers (subtraction), strings
(`localeCompare`), and nullish values (sorted last in asc, first in
desc). For mixed-type columns or custom orderings, pass
`compare: (a, b) => number` on the column.

## Combobox

By default `Combobox` is **strict-select**: the parent owns
`value: Signal<string>`, only picking a fetched option writes it, and
any free text left in the field reverts to the last picked label on
blur. That is the right contract when the value must be one of a known
set (user pickers, foreign keys).

For "suggest existing but allow new" fields — a category picker that
should also accept a brand-new category — pass
**`allowCustom: true`**. The visible text then *commits* instead of
reverting:

```ts
const category = signal("");
Combobox({
  value: category,
  loadOptions: searchCategories,
  allowCustom: true,
  onChange: (value, option) => {
    // option is the picked ComboboxOption, or the synthesized
    // { value: text, label: text } for a brand-new entry.
  },
});
```

The commit rules:

- **Triggers** — blur, clicking outside the control, or pressing Enter
  when the visible text is not an accepted ghost completion. Tab keeps
  its usual behavior (accepts a showing ghost; otherwise the focus move
  blurs and the blur commits). Escape closes the dropdown without
  committing — the text is still committed when focus eventually
  leaves.
- **Near-match resolution** — committed text is trimmed, and if it
  case-insensitively equals the whole label of a currently loaded
  option, that option is picked instead (canonical `value` and `label`,
  regular `onChange`). Typing `hardware` when `Hardware` exists never
  creates a case-variant duplicate.
- **Novel text** — `value` is set to the trimmed text and `onChange`
  fires once with a synthesized `{ value: text, label: text }` option.
  Callers that care whether an entry is new compare the value against
  their known options.
- **Empty text commits `""`** — deliberately clearing the field clears
  the value; it does not resurrect the previous label.
- **Enter precedence** — with the dropdown open, Enter picks the
  highlighted option only when the visible text equals that option's
  label (you accepted the ghost or arrowed onto it). Otherwise Enter
  commits your text — unrelated suggestions are never silently picked
  over what you typed.

Commits are idempotent: dismissing the field again without an edit
neither rewrites `value` nor re-fires `onChange`.

`Combobox` also accepts the shared [`autofocus` / `attrs`
props](#component-library-reference); both target the inner typeahead
`<input>`. Note the component owns that input's visible text (ghost
completion and selection ranges), so `attrs` is for additive
attributes like `name` — exactly what its additive-only rule enforces.

## Forms

`createForm` is the form-state primitive: one call declares the
fields, their initial values, and their validators, and returns typed
reactive state you bind straight onto the controls. It replaces the
hand-rolled pattern of a value+error signal pair per field, a
`validate()` routine, an `applyFieldErrors`/`resetFormState` duo, and
per-form `HttpError` unwrapping.

```ts
import { html } from "zero";
import { Input, createForm } from "zero/components";
import { createLocation } from "../store.ts";

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
  },
  // Cross-field rules; fills only fields without a per-field error.
  validate: (values) =>
    values.code.trim() === values.name.trim()
      ? { name: "Name must differ from code." }
      : {},
});

// Validates, gates, and maps HttpError 400/409 {errors} automatically.
// The action only builds the typed body and handles success.
const onSubmit = form.submit(async (values) => {
  await createLocation({
    code: values.code.trim(),
    name: values.name.trim(),
  });
  // Success handling is yours: close the dialog, toast, navigate.
});

const body = html`
  <form class="stack gap-md" @submit=${onSubmit}>
    ${() => (form.error.val ? html`<p class="text-muted" role="alert">${form.error}</p>` : html``)}
    ${Input({ value: form.fields.code.value, label: "Code", error: form.fields.code.error })}
    ${Input({ value: form.fields.name.value, label: "Name", error: form.fields.name.error })}
    <button class="button button-primary button-md" type="submit"
      disabled=${() => !form.isValid.val}>Add location</button>
  </form>
`;

// Reopening the dialog later: form.reset();
```

### Configuration

`createForm(config)` takes:

- **`fields`** — a record of field declarations. Field names flow
  through as a string-literal union, so `form.fields.<name>` and the
  `values` record are fully typed. Each field has:
  - `initial: string` — the initial value, also restored by `reset()`.
    v1 field values are strings (matching `Input`/`Select`/`TextArea`;
    keep numeric inputs as strings and convert at submit time).
  - `validate?: Validator | Validator[]` — per-field validation, where
    `Validator = (value, values) => string | null`; return a message or
    `null`. Receives all current values for rules that need context. An
    array runs in declaration order and the **first non-null message
    wins**; a single function (today's style) works unchanged. Arrays
    freely mix [built-in rules](#built-in-rules) and hand-written
    functions.
- **`validate?: (values) => Partial<Record<field, string>>`** — the
  cross-field validator. Runs after per-field validators and fills only
  keys that don't already carry a per-field error. Function-only — rules
  and arrays apply per-field.

Validators are plain functions — there is no string DSL and no async
validation (server-side uniqueness belongs on the 409 path; see
[HTTP § Server validation errors](./http.html#server-validation-errors)).
The [built-in rules](#built-in-rules) below are factories that *return*
plain validator functions.

### Built-in rules

The most common validations ship as typed rule factories in
`zero/components`. Each returns a plain single-argument validator
(`Rule = (value: string) => string | null`), so rules drop straight
into `validate:` — alone, in arrays, or mixed with hand-written
functions.

| Rule | Signature | Default message | Empty value |
|------|-----------|-----------------|-------------|
| `required` | `required(message?: string)` | `This field is required.` | fails |
| `minLength` | `minLength(n: number, opts?)` | `Must be at least {n} character(s).` | passes |
| `maxLength` | `maxLength(n: number, opts?)` | `Must be {n} character(s) or fewer.` | passes |
| `intRange` | `intRange(min: number, max: number, opts?)` | `Must be a whole number between {min} and {max}.` | passes |
| `pattern` | `pattern(re: RegExp, opts?)` | `Invalid format.` | passes |
| `email` | `email(opts?)` | `Enter a valid email address.` | passes |

Semantics:

- `required`, `minLength`, `maxLength`, and `email` operate on the
  **trimmed** value; `pattern` tests the raw value against `re` (a
  passed `g`/`y` flag is stripped internally so a stateful regex can't
  alternate results).
- `intRange` accepts an optional leading `+`/`-` and leading zeros,
  and rejects decimals, exponents (`1e3`), and any other non-digit
  input; bounds are inclusive.
- `pattern`'s default message names nothing about the pattern — pass a
  custom message wherever the rule can actually fire.
- `email` is pragmatic (`x@y.z` shaped, no whitespace), not RFC 5322 —
  real verification belongs on the server.

**Empty values pass by default** for every rule except `required()`,
so optional fields with constraints compose without hand-written
functions — `validate: maxLength(200)` accepts an empty notes field
but caps a filled one. Make a field mandatory by composing
`required()` in front, or opt a single rule into enforcing empties
with `allowEmpty: false`.

`opts` is `string | RuleOptions` — a plain string is shorthand for the
custom message:

```ts
maxLength(200, "Keep notes under 200 chars.");
intRange(1, 999, { allowEmpty: false });
pattern(/^[A-Z]{3}-\d{4}$/, { message: "Use the AAA-0000 format." });
```

Before/after — the hand-written ternary from the example above:

```ts
import { createForm, maxLength, required } from "zero/components";

// Before: every form re-types the same checks.
code: {
  initial: "",
  validate: (v) =>
    v.trim() === "" ? "Code is required."
    : v.trim().length > 10 ? "Code must be 10 characters or fewer."
    : null,
},

// After: first failing rule's message wins.
code: {
  initial: "",
  validate: [required("Code is required."), maxLength(10)],
},
```

Arrays mix rules and plain functions — a rule for the common part, a
function for the bespoke part:

```ts
validate: [required(), (v) => (v === "admin" ? "Reserved name." : null)],
```

### Returned shape

For each declared field, `form.fields.<name>` exposes:

- **`value: Signal<string>`** — bind to a control's `value` prop.
  Writing marks the field touched.
- **`error: Signal<string | null>`** — bind to a control's `error`
  prop.
- **`touched: Signal<boolean>`** — `false` until the user first edits
  the field; reset by `reset()`.

And the form itself:

- **`isValid`** — a live computed: `true` iff running every validator
  over the current values yields no errors. Drive a disabled submit
  button with it. Reading it never populates any field's `error`
  signal — validation *display* is gated on submit.
- **`error: Signal<string | null>`** — the form-level error
  (unmatched server keys, network failures).
- **`values()`** — a plain snapshot `{ field: string }`. No trimming;
  normalize in your submit action.
- **`reset()`** — restores initials and clears all field errors, all
  `touched` flags, and the form-level error. Call it when reopening a
  dialog.
- **`setErrors(errors)`** — applies a `Record<field, string>` to field
  error signals; declared fields not present are cleared.
- **`submit(action)`** — wraps your action into an async `@submit`
  handler (next section).

Field errors appear only via `submit()` or `setErrors()` — never from
merely reading `isValid` — with one refinement: once a field *shows* an
error, editing it re-validates just that field live, so the message
clears the moment the user fixes the value (or switches if a different
rule now fails).

### Submit

`form.submit(action)` returns an async event handler for `@submit`.
In order, it:

1. calls `preventDefault()` on the event;
2. marks every field touched;
3. runs all validators and applies the result to field errors, clearing
   the form-level error;
4. returns without calling `action` if anything failed;
5. awaits `action(values())` — your action builds the typed request
   body and handles success (close the dialog, toast, navigate);
6. on a thrown `HttpError` with status `400`/`409` and a non-empty
   `errors` object in the body, maps keys matching declared fields onto
   those fields' error signals and joins messages under unmatched keys
   into the form-level `error` (never silently dropped). Any other
   failure — missing/empty `errors`, other statuses, network errors —
   sets the generic form-level message `"Could not save. Try again."`.

The handler never rethrows, so a `@submit` binding needs no try/catch.
The wire convention it consumes is documented in
[HTTP § Server validation errors](./http.html#server-validation-errors).

---

→ Next: [Routing](./routing.html) — register routes, parse
params, fetch data with `load()`, and compose nested route
trees.

## See also

→ [Best Practices §7 Component usage](./best-practices.html#7-component-usage)
