---
title: Reactivity
nav_order: 4
---

# Reactivity

zero's reactivity is built around three primitives: `signal`,
`computed`, and `effect`. If you internalize them, the rest of the
framework follows. If you're coming from React, the names rhyme
with `useState`, `useMemo`, and `useEffect` — but the mechanics are
simpler.

## What a signal is

A **signal** is a reactive value. You read it with `.val`, write
it with `.set()`, and you can transform the current value with
`.update()`.

```ts
import { signal } from "zero";

const count = signal(0);

count.val;          // 0
count.set(5);
count.val;          // 5
count.update(n => n + 1);
count.val;          // 6
```

In a template, you embed the signal itself (not `.val`):

```ts
import { html, signal } from "zero";

function Counter() {
  const count = signal(0);
  return html`
    <p>${count}</p>
    <button @click=${() => count.update(n => n + 1)}>+</button>
  `;
}
```

The framework sees `${count}` is a signal, binds the text node to
it, and patches the node in place when the signal changes. The
component function never re-runs.

```ts
function NameField() {
  const name = signal("world");
  return html`
    <h1>Hello ${name}</h1>
    <input value=${name} @input=${(e: Event) =>
      name.set((e.target as HTMLInputElement).value)} />
  `;
}
```

## If you're coming from React

| React          | zero       | Note                                                              |
|----------------|------------|-------------------------------------------------------------------|
| `useState`     | `signal`   | Read via `.val`; write via `.set()` / `.update()`. No destructured setter, no batching. |
| `useMemo`      | `computed` | No deps array — dependencies are tracked automatically and re-collected on each run. |
| `useEffect`    | `effect`   | No deps array. Cleanup is a function returned from the effect body, not a second hook. |

Three contrasting snippets:

```jsx
// React
const [count, setCount] = useState(0);
const doubled = useMemo(() => count * 2, [count]);
useEffect(() => {
  document.title = `count: ${count}`;
}, [count]);
```

```ts
// zero
const count = signal(0);
const doubled = computed(() => count.val * 2);
effect(() => {
  document.title = `count: ${count.val}`;
});
```

No deps arrays. The framework watches what each function actually
reads and re-runs only when one of those reads changes. The
sections below say what "reads" means precisely.

## What a computed is

A **computed** is a read-only derived value. It re-evaluates only
when one of its dependencies changes, and only when something
actually reads it (it's lazy).

```ts
import { signal, computed } from "zero";

const price = signal(9.99);
const quantity = signal(3);
const total = computed(() => price.val * quantity.val);

total.val;            // 29.97
quantity.set(5);
total.val;            // 49.95 — recomputed on read
```

Use `computed` for derived values you read more than once. For a
one-off transformation inside a template you can just write
`${() => price.val * quantity.val}` — that's a reactive block,
covered in [Templates](./templates.html).

## What an effect is

An **effect** is a side-effectful function that re-runs whenever
any signal it reads changes.

```ts
import { signal, effect } from "zero";

const count = signal(0);

effect(() => {
  console.log("count is now", count.val);
});
// → logs "count is now 0" immediately, then once per .set().
```

If the body returns a function, the framework treats that as the
cleanup — it runs before each re-run, and once more when the
owning scope disposes.

```ts
effect(() => {
  const id = setInterval(() => count.update(n => n + 1), 1000);
  return () => clearInterval(id);
});
```

A common pattern is auto-focus on mount:

```ts
import { html, ref, effect } from "zero";

function AutoFocus() {
  const input = ref();
  effect(() => input.el?.focus());
  return html`<input ref=${input} />`;
}
```

`effect` returns a `stop()` function. You rarely call it — the
owning scope (a component, a route) cleans up automatically when
it unmounts.

## Auto-tracking explained

There are no deps arrays in zero. Instead, each `computed` /
`effect` body runs inside a context that records every signal
`.val` it reads. Those reads become its dependencies. On the next
run, the dependencies are cleared and re-collected from scratch.

That last detail matters: dependencies are re-collected **per
run**, so they can change shape with the control flow.

```ts
const showAdmin = signal(false);
const userName  = signal("Ada");
const adminId   = signal(42);

effect(() => {
  if (showAdmin.val) {
    console.log("admin", adminId.val);   // depends on adminId, but only when showAdmin is true
  } else {
    console.log("user", userName.val);   // depends on userName, but only when showAdmin is false
  }
});
```

Changing `adminId` only re-runs the effect when `showAdmin.val`
is true. Flip `showAdmin` to false, and `adminId.set(99)` no
longer fires the log. No deps array could have expressed that
without explicit branching.

What counts as a read:

- `signal.val` and `computed.val` reads inside a reactive body.
- Reads inside synchronously-called helper functions, since
  they run during the same body.
- **Not** reads inside `setTimeout`, `Promise.then`, or any
  async continuation — by the time those run, the tracking
  context is no longer active.

## Ownership scopes & cleanup

Every reactive primitive belongs to a **scope**. The framework
opens a scope when it commits a component or navigates a route;
signals and effects created inside that scope dispose together
when the scope tears down (component unmount, route change).

In practice you rarely think about scopes. The rules are:

- Call `signal()`, `computed()`, `effect()` from inside a
  component body, route component, or `load()` function, and
  cleanup is automatic.
- The `stop()` returned from `effect()` is there for the rare
  case where you want manual control — you almost never need it.
- Module-level `signal()` calls *do* persist for the life of the
  page — they're useful for shared stores, but pay attention,
  because the `R03` lint rule (see [Linting](./linting.html))
  flags module-level `effect()` calls as scope leaks.

The [Components](./components.html) chapter goes into the
scope/mount story in more detail.

## Common pitfalls

**Reading `.val` outside a reactive context.** It works, but
nothing subscribes. The value you got back is a snapshot and
won't update.

```ts
const count = signal(0);
const snapshot = count.val;
// snapshot is now `0` and stays `0`, even after count.set(...)
```

If you want to react to changes, do the read inside `effect`,
`computed`, or an `html` template.

**`${signal}` vs `${signal.val}` in templates.** Both compile,
but only the first auto-subscribes.

```ts
html`<p>${count}</p>`        // reactive — text node patches on change
html`<p>${count.val}</p>`    // rendered once, then frozen
```

The framework can't tell whether you meant the live signal or the
snapshot. `zero lint`'s `R01` rule catches the mistake — see
[Linting](./linting.html).

**Stale closures inside effects.** The auto-tracking story
already handles this: every run reads through `.val`, so there's
no stale closure to capture. Just make sure you read the signal
inside the body, not in an outer scope:

```ts
// Bad: count is captured by value, the effect never re-runs.
const snapshot = count.val;
effect(() => console.log(snapshot));

// Good: read inside the body.
effect(() => console.log(count.val));
```

**Module-level effects leak.** A bare `effect()` at the top of a
module has no scope to attach to, so it lives forever. `R03`
flags this. Either move the effect inside a component / `load()`
function, or — if it really is application-level — wire it
through `app.use()` so the framework manages its lifetime.

---

→ See [`examples/counter/`](../examples/counter/) for the
smallest possible signal-driven app.

→ Next: [Templates](./templates.html) — everything you can
substitute into an `html` template.
