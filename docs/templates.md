---
title: Templates
nav_order: 5
---

# Templates

The `html` tagged template is how you describe DOM in zero. There
is no JSX, no virtual DOM, no diff: `html` parses once per call
site and clones a `DocumentFragment` per render, with granular
reactive bindings at every `${...}` substitution.

## What `html` does

```ts
import { html } from "zero";

function Greeting() {
  return html`<h1>Hello world</h1>`;
}
```

The first time the framework sees a particular `html\`...\``
call site, it parses the static parts of the string into a real
`<template>` element and walks it to find every `${}` position.
That structure is cached on the call site. On each render, the
framework clones the template's `DocumentFragment` and patches in
the dynamic values.

The consequence: the **component function runs once**. The
framework wires up the bindings, and from then on only the bound
text nodes and attributes update — never the parent function.

`html` returns a `TemplateResult`:

```ts
interface TemplateResult {
  _template: Template;   // cached parsed structure (shared)
  _values: any[];        // the ${...} values for this instance
}
```

You don't usually touch it directly. You pass it back to the
framework — as a return value from a component, as a child in
another template, or to `render()` in a test.

## Valid substitution values

Any of these can appear inside `${...}`:

| Value                 | Behaviour                                                                       |
|-----------------------|---------------------------------------------------------------------------------|
| `string` / `number`   | Rendered as text.                                                               |
| `boolean`             | For attributes: `false`/`null`/`undefined` remove the attribute; `true` sets it to `""`. |
| `null` / `undefined`  | Render nothing.                                                                 |
| `Signal<T>`           | Auto-subscribes; the bound text node or attribute updates in place.             |
| `TemplateResult`      | Nested template; commits in place.                                              |
| `T[]` (array)         | Each item inserted in order; mixed types allowed.                               |
| `() => TemplateValue` | Reactive block — re-evaluates whenever any signal it reads changes.             |

A small example per row:

```ts
html`<p>${"hello"} ${42}</p>`;               // text
html`<button disabled=${isDisabled}>Go</button>`;          // boolean attr
html`<p>${maybeUndefined}</p>`;              // null/undefined → nothing
html`<p>${countSignal}</p>`;                 // signal → reactive text
html`<div>${Card({ title: "Hi" })}</div>`;   // nested TemplateResult
html`<ul>${[html`<li>a</li>`, html`<li>b</li>`]}</ul>`;    // array
html`<p>${() => active.val ? "on" : "off"}</p>`;          // reactive block
```

## Attribute binding

Inside `attr=${value}`, the framework picks the right strategy
based on the attribute:

```ts
html`<div class=${cls}>...</div>`            // string attribute
html`<input value=${name} />`                // value → live DOM property
html`<input disabled=${isDisabled} />`       // boolean attribute
html`<a href=${"/users/" + id}>profile</a>`  // computed string
```

For boolean-shaped HTML attributes (`disabled`, `hidden`,
`readonly`, `required`, `open`, `multiple`), the binding is a true
on/off — the attribute is added when the value is truthy and removed
when falsy. For everything else, the value is stringified.

**Live form properties.** `value` (on `<input>`, `<textarea>`,
`<select>`), `checked` (on `<input>`), and `selected` (on
`<option>`) bind to the live DOM *property*, not the content
attribute. On a form control the content attribute is only the
*default* — once the element exists the browser tracks the shown,
checked, and selected state on the property, and a late attribute
write is ignored. Binding the property is what makes a programmatic
state change actually appear in the field:

```ts
const url = signal("");
html`<input value=${url} />`;   // url.set("https://…") populates the input
```

This is a one-way binding (state → field). To push user edits back
into state, add an `@input` / `@change` handler (see below). The
guard on `value` is caret-safe: writing the value the user has
already typed is a no-op, so a controlled input never has its cursor
jumped to the end.

Static text and placeholders mix freely inside a single attribute
value:

```ts
html`<span class="chip chip--${status} active">${label}</span>`
html`<div style="color: ${color}; padding: ${pad}px">…</div>`
```

Any number of `${…}` substitutions can appear alongside static
characters in one attribute. The framework joins the pieces — every
reactive value in the attribute is tracked by a single effect, so
the attribute re-renders once per change, not once per substitution.

Boolean / null / undefined attribute semantics only apply when the
attribute value is *just* a placeholder (`disabled=${flag}`). In a
concat context (`class="a ${x} b"`), `null` and `undefined` render
as empty strings and booleans stringify to `"true"` / `"false"`.

Use a reactive block for derived attributes that depend on more
than one signal:

```ts
html`<div class=${() => `card ${active.val ? "active" : ""}`}>...</div>`
```

## Event binding

Events use `@event=${handler}`. The handler runs with the native
event object as its argument.

```ts
html`<button @click=${() => count.update(n => n + 1)}>+</button>`
html`<input @input=${(e: Event) => name.set((e.target as HTMLInputElement).value)} />`
html`<form @submit=${handleSubmit}>...</form>`
```

Handlers are bound once per element; they're attached via
`addEventListener` and removed automatically when the element
leaves the DOM.

## Event modifiers

Append dot-separated modifiers to the event name:

```ts
html`<form @submit.prevent=${onSubmit}>...</form>`
html`<a href="/x" @click.stop=${handle}>x</a>`
html`<button @click.once=${initialize}>Init</button>`
html`<div @scroll.throttle=${onScroll}>...</div>`
html`<input @input.debounce=${onSearch} />`
html`<input @input.debounce:250=${onSearch} />`
html`<input @keydown.enter=${submit} />`
html`<input @keydown.escape=${close} />`
```

The full set:

| Family       | Modifiers                                                                                       |
|--------------|-------------------------------------------------------------------------------------------------|
| Side-effect  | `.prevent` (`preventDefault`), `.stop` (`stopPropagation`), `.once`                             |
| Timing       | `.throttle` / `.throttle:<ms>` (default 100 ms), `.debounce` / `.debounce:<ms>` (default 100 ms) |
| Key filter   | `.enter`, `.escape`, `.space`, `.tab`, `.up`, `.down`, `.left`, `.right`                        |

The `:<ms>` suffix is only valid on `.throttle` and `.debounce`;
T02 flags it elsewhere. Malformed intervals (`:abc`, `:0`, `:-5`)
are runtime errors.

Modifiers compose: `@click.prevent.stop=${...}`,
`@keydown.enter.prevent=${...}`. Order doesn't matter — modifiers
are applied as a set, not a pipeline.

The `T02` lint rule flags typos in modifier names. See
[Linting](./linting.html).

## Reactive blocks

Wrapping an expression in a function makes it a **reactive block**:
the framework runs it once at commit, tracks the signals it reads,
and re-runs it whenever any of those signals change. The result
replaces the previous output in place.

```ts
function AuthStatus() {
  const auth = inject<Signal<{ status: string }>>("auth");
  return html`
    <div>
      ${() => {
        if (auth.val.status === "loggedIn") return html`<span>Welcome</span>`;
        if (auth.val.status === "loading")  return html`<span>Loading…</span>`;
        return html`<a href="/login">Log in</a>`;
      }}
    </div>
  `;
}
```

Reactive blocks are the right tool for any conditional or
control-flow situation. They cooperate with the auto-tracking
described in [Reactivity](./reactivity.html#auto-tracking-explained).

A signal substituted directly (`${signal}`) is an even cheaper
case — just a text-node binding — and you should prefer it when
the only thing you're doing is reading the value. Reach for
`() => ...` when you need branches, expressions, or more than one
read combined.

## `each()` — keyed lists

For lists where items can be added, removed, reordered, or
updated independently, use `each`. It takes a signal whose value
is an array, a render function, and an optional key extractor.

```ts
import { html, signal, each } from "zero";

const todos = signal([
  { id: 1, text: "Learn signals" },
  { id: 2, text: "Write a component" },
]);

html`
  <ul>
    ${each(
      todos,
      (todo) => html`<li>${todo.text}</li>`,
      (todo) => todo.id   // key — defaults to identity if omitted
    )}
  </ul>
`;
```

Why keys matter: zero matches old and new items by key and
**moves** existing DOM nodes for the items that survive a re-render
instead of destroying and rebuilding them. That preserves DOM
state (input focus, scroll position) and disposes per-item effects
only for items that actually left the list.

Each render function call opens its own reactive scope, so a
signal you create inside the per-item closure tears down when
that item is removed.

The `T03` lint rule flags `each(...)` calls without an explicit
key extractor. See [Linting](./linting.html).

## `ref()` — element handles

Sometimes you need a direct reference to a DOM node — to focus
an input, to measure a layout, to integrate a non-zero library.
`ref()` returns a small object with a single `el` property,
populated after the element commits.

```ts
import { html, ref, effect } from "zero";

function AutoFocus() {
  const input = ref<HTMLInputElement>();
  effect(() => input.el?.focus());
  return html`<input ref=${input} />`;
}
```

`ref` plays well with `effect`: the effect runs after the
component commits, so by the time the body executes, `input.el`
is populated. If the element is conditionally rendered, the
reactive block updates `input.el` on each mount/unmount and the
effect re-runs.

For broader scope/cleanup story, see
[Reactivity § Ownership scopes & cleanup](./reactivity.html#ownership-scopes--cleanup).

---

→ Next: [Components](./components.html) — how components
compose, pass props, and slot in children.
