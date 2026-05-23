---
title: Components
nav_order: 5
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

zero ships sixteen production-ready components under the bare
specifier `"zero/components"`. They use the design system tokens
covered in [Theming](./theming.html), so they take on your brand
once you redefine the public tokens.

| Component  | Required props (summary)                                                   | Example                                                                          |
|------------|----------------------------------------------------------------------------|----------------------------------------------------------------------------------|
| `Avatar`   | `alt`; optional `src`, `initials`, `size`                                  | `Avatar({ alt: "Ada", initials: "A", size: "md" })`                              |
| `Badge`    | optional `variant`, `size`, `children`                                     | `Badge({ variant: "success", children: "New" })`                                 |
| `Button`   | optional `variant`, `size`, `disabled`, `loading`, `onClick`, `children`   | `Button({ variant: "primary", onClick: save, children: "Save" })`                |
| `Card`     | optional `variant`, `title`, `children`                                    | `Card({ title: "Profile", children: html\`<p>…</p>\` })`                         |
| `Checkbox` | `checked: Signal<boolean>`; optional `label`, `disabled`                   | `Checkbox({ checked: agreed, label: "I agree" })`                                |
| `Dialog`   | `open: Signal<boolean>`; optional `size`, `title`, `children`, `onClose`   | `Dialog({ open, title: "Confirm", children: html\`…\` })`                        |
| `Input`    | `value: Signal<string>`; optional `type`, `size`, `placeholder`, `label`   | `Input({ value: name, label: "Name", type: "text" })`                            |
| `Pagination` | `page: Signal<number>`, `totalPages: Signal<number> \| number`; optional `size`, `siblingCount`, `boundaryCount`, `disabled`, `onChange`, `summary` | `Pagination({ page, totalPages: 10 })`                                          |
| `Radio`    | `selected: Signal<string>`, `name`, `value`; optional `label`              | `Radio({ selected: choice, name: "size", value: "lg", label: "Large" })`         |
| `Select`   | `value: Signal<string>`, `options: SelectOption[]`; optional `label`       | `Select({ value: country, options: [{ value: "us", label: "USA" }] })`           |
| `Spinner`  | optional `variant`, `size`, `label`                                        | `Spinner({ size: "lg", label: "Loading" })`                                      |
| `Tabs`     | `active: Signal<string>`, `tabs`, `panels`                                 | `Tabs({ active, tabs: [...], panels: { ... } })`                                 |
| `Table`    | `columns`, `rows: Signal<T[]>`, `rowKey`; optional `density`, `loading`    | `Table({ columns, rows, rowKey: r => r.id })`                                    |
| `TextArea` | `value: Signal<string>`; optional `rows`, `placeholder`, `label`           | `TextArea({ value: notes, rows: 5, label: "Notes" })`                            |
| `Toast`    | `open: Signal<boolean>`, `message`; optional `variant`, `duration`         | `Toast({ open, message: "Saved", variant: "success" })`                          |
| `Toggle`   | `checked: Signal<boolean>`; optional `label`, `disabled`                   | `Toggle({ checked: darkMode, label: "Dark mode" })`                              |

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

The signatures above are the public surface; the source of truth
is `crates/zero-scaffold/src/scaffold/.zero/components.d.ts`,
copied into your project as `.zero/components.d.ts` by `zero
init` / `zero update`.

For component patterns in larger apps — when to reach for the
shipped components vs. raw HTML, how to compose them, when to
build your own — see [Best Practices §7](./best-practices.html#7-component-usage).

---

→ Next: [Routing](./routing.html) — register routes, parse
params, fetch data with `load()`, and compose nested route
trees.

## See also

→ [Best Practices §7 Component usage](./best-practices.html#7-component-usage)
