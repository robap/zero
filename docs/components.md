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
| `Button`   | optional `variant`, `size`, `disabled`, `loading`, `onClick`, `children`   | `Button({ variant: "primary", onClick: save, children: "Save" })`                |
| `Card`     | optional `variant`, `title`, `children`                                    | `Card({ title: "Profile", children: html\`<p>…</p>\` })`                         |
| `Checkbox` | `checked: Signal<boolean>`; optional `label`, `disabled`, `debounceMs`     | `Checkbox({ checked: agreed, label: "I agree" })`                                |
| `Combobox` | `value: Signal<string>`, `loadOptions: (q) => Promise<ComboboxOption[]>`; optional `initialLabel`, `size`, `placeholder`, `label`, `disabled`, `debounceMs`, `minQueryLength`, `noResultsLabel`, `loadingLabel`, `onChange` | `Combobox({ value, loadOptions: loadUsers })` |
| `Dialog`   | `open: Signal<boolean>`; optional `size`, `title`, `children`, `onClose`   | `Dialog({ open, title: "Confirm", children: html\`…\` })`                        |
| `Drawer`   | `open: Signal<boolean>`, `side`; optional `mode`, `size`, `title`, `body`, `controls` | `Drawer({ open, side: "right", mode: "push", title: "Edit user", body: form })`  |
| `Input`    | `value: Signal<string>`; optional `type`, `size`, `placeholder`, `label`, `debounceMs` | `Input({ value: name, label: "Name", type: "text" })`                            |
| `Pagination` | `page: Signal<number>`, `totalPages: Signal<number> \| Computed<number> \| number`; optional `size`, `siblingCount`, `boundaryCount`, `disabled`, `onChange`, `summary` | `Pagination({ page, totalPages: 10 })`                                          |
| `Radio`    | `selected: Signal<string>`, `name`, `value`; optional `label`, `debounceMs` | `Radio({ selected: choice, name: "size", value: "lg", label: "Large" })`         |
| `Select`   | `value: Signal<string>`, `options: SelectOption[]`; optional `label`, `debounceMs` | `Select({ value: country, options: [{ value: "us", label: "USA" }] })`           |
| `Spinner`  | optional `variant`, `size`, `label`                                        | `Spinner({ size: "lg", label: "Loading" })`                                      |
| `Tabs`     | `active: Signal<string>`, `tabs`, `panels`                                 | `Tabs({ active, tabs: [...], panels: { ... } })`                                 |
| `Table`    | `columns`, `rows: Signal<T[]>`, `rowKey`; optional `density`, `loading`, `sort`, `onSortChange` | `Table({ columns, rows, rowKey: r => r.id })`                                    |
| `TextArea` | `value: Signal<string>`; optional `rows`, `placeholder`, `label`, `debounceMs` | `TextArea({ value: notes, rows: 5, label: "Notes" })`                            |
| `Toast`    | `open: Signal<boolean>`, `message`; optional `variant`, `duration`         | `Toast({ open, message: "Saved", variant: "success" })`                          |
| `Toggle`   | `checked: Signal<boolean>`; optional `label`, `disabled`, `debounceMs`     | `Toggle({ checked: darkMode, label: "Dark mode" })`                              |

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

---

→ Next: [Routing](./routing.html) — register routes, parse
params, fetch data with `load()`, and compose nested route
trees.

## See also

→ [Best Practices §7 Component usage](./best-practices.html#7-component-usage)
