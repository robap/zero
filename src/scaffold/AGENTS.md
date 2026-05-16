# Zero — Agent & Developer Reference

`zero` is a zero-dependency frontend framework distributed as a single CLI binary. This file is the authoritative API reference for building applications against the version of `zero` that scaffolded this project. Every example here is valid against the framework's current runtime. If a feature is not described here, it is not yet implemented.

The framework exposes three import paths:

- `"zero"` — the runtime: `App`, reactivity, templates, routing, components.
- `"zero/test"` — the test runner and DOM helpers.
- `"zero/components"` — the shipped component library (Button, Input, Dialog, …).

Any identifier whose name begins with `_` or `__` is internal — do not import or rely on it.

---

## Quick start

```bash
zero init     # scaffold a project (already run — this is how AGENTS.md got here)
zero dev      # start the dev server with file watching and full-page reload
zero test     # run all *.test.ts / *.test.js / *.spec.ts / *.spec.js under the project root
zero build    # produce a production build into the configured output directory
```

`zero init` is interactive on first run and writes a `zero.toml`. Re-running it in a non-empty project root is refused — to reset, delete the project directory and run again.

The generated project layout:

```
.
├── AGENTS.md                # this file
├── .gitignore               # ignores .zero/ and dist/
├── tsconfig.json            # editor-only TS config; the CLI ignores it
├── .zero/                   # framework-owned, refreshed by `zero update` — do not edit
│   ├── zero.d.ts            # type surface for `"zero"`
│   ├── zero-test.d.ts       # type surface for `"zero/test"`
│   └── styles/
│       ├── _tokens.scss     # design tokens + theme variants
│       ├── _base.scss       # minimal reset, token-bound body
│       ├── _layout.scss     # six layout primitives
│       ├── _utilities.scss  # gap-*, pad-*, border-* utilities
│       ├── _alignment.scss  # align-*, justify-*, text-*, flex-* utilities
│       └── zero.scss        # aggregate that @use's the five partials
├── index.html               # entry HTML; <script> tags are injected automatically
├── src/
│   ├── app.ts               # builds and starts the App
│   └── routes/
│       ├── home.ts          # default route component
│       └── home.test.ts     # unit test for the home route
└── styles/
    └── app.scss            # @use '../.zero/styles/zero'; — add your styles here
```

### JavaScript projects

Authoring in plain `.js` is still fully supported — both extensions work everywhere. The scaffold ships `.ts` because that's where the documented examples live; switching the suffix is the only change needed to use plain JS, and the JSDoc conventions in this document still apply to those files.

`zero` reads `zero.toml` at the project root for the dev port, build output directory, and optional backend proxy. The CLI never modifies your source files.

---

## Imports

```ts
import {
  App, signal, computed, effect,
  html, each, ref,
  inject,
  navigate, back, forward, route,
} from "zero";

import {
  describe, it, expect,
  beforeAll, afterAll, beforeEach, afterEach,
  render, find, findAll, text, fire, cleanup, spy,
} from "zero/test";
```

Examples in this file use ES module imports with explicit extensions, matching the scaffold's own files (`./routes/home.ts`, not `./routes/home`). JS files are imported with `./routes/home.js`.

---

## Components

A component is a plain function that returns a `TemplateResult`. The function runs **once per mount**. Reactivity, not re-execution, updates the DOM.

```js
import { html } from "zero";

/**
 * @returns {import("zero").TemplateResult}
 */
export default function Greeting() {
  return html`<h1>Hello from zero</h1>`;
}
```

The `html` tagged template returns a `TemplateResult` — a small descriptor object. The runtime commits it to DOM when it is rendered (by the route system, by `render()` in tests, or as a value inside another template).

### Props

Props are plain objects. They are not deep-cloned, frozen, or observed.

```js
/**
 * @param {{ name: string }} props
 * @returns {import("zero").TemplateResult}
 */
function UserCard(props) {
  return html`<div class="card"><h2>${props.name}</h2></div>`;
}
```

If a parent passes a signal as a prop, the signal stays reactive: reading `props.someSignal.val` inside a reactive context still subscribes.

### Children and slots

There is no dedicated children API. Pass a `TemplateResult` (or a signal of one, or a list) as any prop you like.

```js
function Card(props) {
  return html`
    <div class="card">
      <h2>${props.title}</h2>
      <div class="body">${props.body}</div>
    </div>
  `;
}

function Page() {
  return Card({
    title: "Hello",
    body: html`<p>Some content.</p>`,
  });
}
```

### Events

Events use an `@event` attribute. The handler is a function. The event name follows the `@`, and modifiers are dot-separated after the event name.

```js
html`<button @click=${() => count.update(n => n + 1)}>Increment</button>`
html`<input @input=${(e) => name.set(e.target.value)} />`
html`<form @submit.prevent=${onSubmit}>...</form>`
html`<input @keydown.enter=${onEnter} />`
html`<button @click.once=${init}>Init</button>`
```

Supported modifiers:

- `.prevent` — calls `event.preventDefault()`.
- `.stop` — calls `event.stopPropagation()`.
- `.once` — listener fires once and is removed.
- Key filters for keyboard events: `.enter`, `.escape`, `.space`, `.tab`, `.up`, `.down`, `.left`, `.right`. The handler runs only if `event.key` matches one of the listed filters.
- `.throttle` — throttle the handler at a fixed **100ms** interval. Not configurable today.
- `.debounce` — debounce the handler with a fixed **100ms** trailing delay. Not configurable today.

Modifiers compose: `@keydown.enter.prevent` filters to Enter and calls `preventDefault`.

### Reactive blocks

A function passed as a template value — `${() => …}` — is a **reactive block**. It runs immediately to produce its initial output, and re-runs whenever any signal it reads changes. Use reactive blocks for conditional rendering, computed text, and anywhere the output depends on changing state.

```js
html`<p>Count: ${() => count.val}</p>`
```

A signal passed directly also reactively updates:

```js
html`<p>Count: ${count}</p>`   // reads count.val automatically
```

The difference: a bare signal renders its current value; a reactive block can run arbitrary code, including branching and returning different templates.

### Conditional rendering

```js
function AuthStatus() {
  const auth = inject("auth");

  return html`
    <div>
      ${() => {
        if (auth.val.status === "loggedIn") {
          return html`<span>Welcome, ${auth.val.user.name}</span>`;
        }
        return html`<a href="/login">Log in</a>`;
      }}
    </div>
  `;
}
```

There is no `v-if`, `#if`, or `<Show>` component. A reactive block returning different templates is the only pattern.

### Lists with `each`

```js
import { html, each, signal } from "zero";

const todos = signal([
  { id: 1, text: "Learn zero" },
  { id: 2, text: "Build app" },
]);

function TodoList() {
  return html`
    <ul>
      ${each(todos, (todo, index) => html`
        <li>${index + 1}. ${todo.text}</li>
      `)}
    </ul>
  `;
}
```

Signature: `each(signalOfArray, (item, index) => TemplateResult)`. When the signal's array changes, `each` re-renders the list from scratch — there is no keyed reconciliation today. For frequently changing large lists, factor the per-item state out of the array so the underlying signals stay stable.

### Refs

`ref()` returns an object `{ el }`. Pass it via a `ref=${…}` attribute; after the template is committed, the `el` property points at the DOM element.

```js
import { html, ref, effect } from "zero";

function AutoFocus() {
  const input = ref();

  effect(() => {
    if (input.el) input.el.focus();
  });

  return html`<input ref=${input} type="text" />`;
}
```

---

## Reactivity

Three primitives, all imported from `"zero"`.

### `signal(initialValue)`

A reactive container.

```js
const count = signal(0);

count.val;            // read — subscribes the current reactive context
count.set(5);         // write — no-op if `===` to the current value
count.update(n => n + 1);   // write — equivalent to set(fn(val))
```

`.val` is a getter. Reading it inside a `computed`, an `effect`, or a template/reactive-block registers a subscription. Reading outside any reactive context returns a snapshot and does not subscribe.

### `computed(fn)`

A lazily-evaluated derived value.

```js
const price = signal(10);
const qty = signal(3);
const total = computed(() => price.val * qty.val);

total.val;     // 30
price.set(20);
total.val;     // 60 — recomputed on the next .val read
```

`total.val` is read-only. `computed` re-evaluates the next time `.val` is read after one of its dependencies changes; the value is not pushed eagerly.

### `effect(fn)`

A side effect that re-runs when its dependencies change.

```js
const name = signal("Alice");

const stop = effect(() => {
  console.log("name is", name.val);
  return () => {
    // optional cleanup — runs before each re-run and on stop()
  };
});

name.set("Bob");  // logs "name is Bob"
stop();           // dispose
```

Dependencies are auto-tracked on each run — no dependency arrays. Effects created during a component render are tied to that component's scope and are disposed automatically when the component is torn down (e.g. when its route is unmounted).

---

## App configuration

`new App()` builds an app instance. Configure with the methods below, then call `run(selector)` to mount. Every builder method returns `this` and **throws if called after `run()`**.

```js
import { App, signal } from "zero";
import Home from "./routes/home.ts";

const app = new App();
app.state("count", signal(0));
app.route("/", Home);
app.run("#app");
```

### `app.state(key, value)`

Register a value (typically a signal) under `key`. Retrieve it later with `inject(key)`. Throws on a duplicate key.

```js
app.state("user", signal({ id: null, name: "" }));
app.state("theme", signal("light"));
```

### `app.use(mw)`

Register a middleware. Middleware runs once per navigation, before guards and loaders, in registration order. The function receives `{ route, state, redirect }`.

```js
app.use(({ route, state, redirect }) => {
  if (route.meta?.protected && state.user.val.id == null) {
    redirect("/login");
  }
});
```

`state` is an object whose properties are the values registered via `app.state`. `redirect(path)` cancels the current navigation and starts a new one.

### `app.route(pattern, loaderOrComponent, opts?)`

Register a route. Routes are matched in registration order — first match wins.

```js
app.route("/", Home);                            // eager component
app.route("/about", () => import("./routes/about.js"));   // lazy loader
app.route("/users/:id", () => import("./routes/user.js"));
app.route("*", NotFound);                        // wildcard catch-all
```

**Patterns** support exact paths (`/about`), `:name` segments (`/users/:id`), and the bare wildcard (`*`).

**Loader form**: pass either an eager component function, or a function returning `import("...")` whose module's `.default` is the component. Lazy loaders are awaited the first time the route is visited; the resolved component is cached.

**Route options** (`opts`):

- `guard({ params, query, state, route, redirect }) => boolean | void | Promise<…>`
  Return `false` to abort the navigation; the previously-committed URL is restored. Call `redirect(path)` to navigate elsewhere.

  ```js
  app.route("/admin", AdminPage, {
    guard: ({ state, redirect }) => {
      if (state.user.val.role !== "admin") {
        redirect("/");
        return false;
      }
    },
  });
  ```

- `load({ params, query, state, fetch, route }) => Promise<void>`
  Runs before the route component renders. Use it to mutate signals held in `state` (so the component can read them via `inject`). `fetch` is bound from `globalThis.fetch`.

  ```js
  app.route("/users/:id", UserPage, {
    load: async ({ params, state, fetch }) => {
      const res = await fetch(`/api/users/${params.id}`);
      state.currentUser.set(await res.json());
    },
  });
  ```

- `meta` — a plain object merged root-to-leaf across nested routes. Read inside middleware/guards/components via `route.meta`.

- `loading` — per-route loading-UI override. Used in place of the global `app.loading` when this route's navigation exceeds 150ms.

- `error` — per-route error-UI override. Used in place of `app.error` for this route's failures.

- `children` — array of nested route descriptors. Each child entry is `{ path, load, guard?, meta?, loading?, error?, children? }` where `load` is the child's **component or lazy loader** (the field is named `load` for the child shape; despite the name, it serves the same role as `loaderOrComponent` on the parent). Required.

  ```js
  app.route("/dashboard", Dashboard, {
    children: [
      { path: "/",          load: DashOverview },
      { path: "/analytics", load: () => import("./routes/analytics.js") },
    ],
  });
  ```

  The parent component receives an `outlet` prop — a signal — that renders the matched child:

  ```js
  function Dashboard(props) {
    return html`
      <div class="dashboard">
        <nav><a href="/dashboard/analytics">Analytics</a></nav>
        <section>${props.outlet}</section>
      </div>
    `;
  }
  ```

### `app.layout(component)`

Set a single layout component that wraps every route. The layout receives `{ outlet }` — a signal whose value is the matched route's `TemplateResult`. Render it with `${props.outlet}`.

```js
function RootLayout(props) {
  return html`
    <header><a href="/">Home</a></header>
    <main>${props.outlet}</main>
    <footer>© 2026</footer>
  `;
}

app.layout(RootLayout);
```

Only one layout per app. Calling `layout` twice throws.

### `app.loading(component)` and `app.error(component)`

Global loading and error UI.

- `loading`: a zero-argument component shown when a navigation's `load`/middleware/guard chain exceeds 150ms.
- `error`: a component called with `{ error, retry }`. `retry()` re-runs the navigation that failed.

```js
app.loading(() => html`<div class="spinner"></div>`);
app.error(({ error, retry }) =>
  html`<div class="error">
    <p>${String(error)}</p>
    <button @click=${retry}>Retry</button>
  </div>`);
```

### `app.run(selector)`

Mount the app to `document.querySelector(selector)` and start the navigation lifecycle.

Side effects:

- Renders the route matching `window.location`.
- Attaches a `popstate` listener for browser back/forward.
- Attaches a document-level `click` listener that intercepts same-origin `<a>` clicks (see [Navigation](#navigation)).
- Marks this instance as the currently-running app for `inject`, `navigate`, and `route()`.

Throws if the selector matches nothing or if `run` has already been called.

### `app.match(input)`

Test helper. Matches a path-and-query string against the route table without rendering or navigating. Returns `{ route, params, query, pathname, search }` or `null`. Useful in unit tests.

```js
const m = app.match("/users/42?tab=posts");
expect(m.params.id).toBe("42");
expect(m.query.tab).toBe("posts");
```

---

## Routes

A route component is the function registered with `app.route`. The runtime invokes it with one props object:

```js
/**
 * @param {{ params: Record<string,string>, query: Record<string,string>, state: object, outlet?: object }} props
 * @returns {import("zero").TemplateResult}
 */
export default function UserPage(props) {
  return html`
    <h1>User ${props.params.id}</h1>
    <p>Tab: ${props.query.tab ?? "default"}</p>
  `;
}
```

- `props.params` — URL parameters extracted from `:name` segments, decoded.
- `props.query` — parsed query string as a plain object.
- `props.state` — a view onto values registered with `app.state(...)`. `props.state.count` returns the signal you registered under `"count"`. Inside the template you can either read `props.state.count.val` directly or render the signal: `${props.state.count}`.
- `props.outlet` — present only on parent (non-leaf) routes when nested children are configured. Render it with `${props.outlet}` to mount the matched child route.

### Active-link styling

After each successful navigation, the runtime sets attributes on `<a>` elements inside the mounted tree:

- `data-active` — the link's path is a prefix of the current path.
- `data-active-exact` — the link's path *and* query exactly match the current URL.

Style them with plain CSS:

```css
a[data-active]       { font-weight: bold; }
a[data-active-exact] { color: var(--primary); }
```

External links, hash-only links, and links with `target`, `download`, or `data-external` are skipped.

---

## Styles

The scaffold authors styles in SCSS. `zero dev` compiles `.scss` on the fly; `zero build` emits hashed CSS into `<out>/assets/`.

- `index.html` links to the SCSS entry: `<link rel="stylesheet" href="/styles/app.scss">`. The build rewrites this href to the hashed output.
- Partials use the standard underscore prefix: `styles/_buttons.scss` is consumed via `@use 'buttons';` from a sibling file. Files whose name starts with `_` are not addressable as standalone stylesheets.
- Design tokens are CSS custom properties declared in `styles/_tokens.scss`. Read them everywhere with `var(--name)` — there is no SCSS-variable bridge layer in v1.
- Plain `.css` still works — the dev server and build serve and hash `.css` files unchanged. Rename to `.scss` to opt in.

The framework forbids scoped styles, CSS modules, and CSS-in-JS. SCSS gives you variables and nesting; class names are still plain strings.

### Design system

The scaffold ships a built-in CSS design system: tokens, theme switching, layout primitives, and utility classes. The system lives in five partials plus an aggregate, all framework-owned in `.zero/styles/`, brought in by your `styles/app.scss` via `@use '../.zero/styles/zero';`:

| Partial | What it declares |
| --- | --- |
| `_tokens.scss` | CSS custom properties for spacing, colors, radius, type, line height, shadow, and border widths; light and dark theme variants. |
| `_base.scss` | Box-sizing reset and a token-bound `body` rule. No heading or paragraph styling. |
| `_layout.scss` | Six layout primitive classes: `cluster`, `stack`, `frame`, `split`, `flank`, `grid`. |
| `_utilities.scss` | Fifteen utility classes: `gap-{xs,sm,md,lg,xl}`, `pad-{xs,sm,md,lg,xl}`, `border`, `border-{t,r,b,l}`. |
| `_alignment.scss` | Twenty-seven utility classes across six families: `align-*`, `justify-*`, `align-self-*`, `justify-self-*`, `text-*`, `flex-{row,row-reverse,col,col-reverse}`. |

These partials live under `.zero/styles/` and are framework-owned — `zero update` refreshes them; do not edit them. To override a token, re-declare the CSS custom property in your `styles/app.scss` after the `@use` line. To add new utility classes, write them in `styles/app.scss` directly.

#### Layout primitives

| Class | Purpose |
| --- | --- |
| `cluster` | Horizontal flex row that wraps. Default `gap: var(--space-md)`. |
| `stack` | Vertical flex column. Default `gap: var(--space-md)`. |
| `frame` | Fixed aspect-ratio box (default `16 / 9`); children centered and clipped. Override per-instance via `--frame-ratio`. |
| `split` | Two equal-width columns. Default `gap: var(--space-md)`. Does not wrap. |
| `flank` | First child is content-sized; second fills. Wraps when narrow. Default `gap: var(--space-md)`. |
| `grid` | Auto-fitting columns of `minmax(min(100%, var(--grid-min, 16rem)), 1fr)`. Default `gap: var(--space-md)`. |

#### Spacing scale

Five steps: `xs`, `sm`, `md`, `lg`, `xl`. Each is a CSS custom property (`--space-xs` … `--space-xl`).

- `gap-{step}` sets `gap: var(--space-{step})` on any flex or grid container.
- `pad-{step}` sets `padding: var(--space-{step})` on any element.

Composition is by class-list order: `class="cluster gap-lg"` overrides the cluster's default `var(--space-md)` because `gap-lg` follows `.cluster` in the compiled CSS. No `!important`, no axis variants — write `padding-inline` directly when you need axis-specific spacing.

#### Border utilities

- `border` — `1px` solid border on all four sides using `--color-border`.
- `border-{t,r,b,l}` — same value, single side. Useful for dividers, accents, sidebar edges.

Thicker borders: override `--border-thin` locally (the design-system border utilities all read it). Width variants (`border-md`, `border-thick`) are not shipped.

#### Alignment, justification, and direction

Six families of single-property utilities live in `_alignment.scss`. They override the layout primitives' defaults by class-list order: `class="cluster align-stretch"` cancels `.cluster`'s default `align-items: center`.

| Family | Property | Values |
| --- | --- | --- |
| `align-*` | `align-items` (on a flex/grid container) | `start`, `center`, `end`, `stretch`, `baseline` |
| `justify-*` | `justify-content` (on a flex/grid container) | `start`, `center`, `end`, `between`, `around`, `evenly` |
| `align-self-*` | `align-self` (on a flex/grid child) | `start`, `center`, `end`, `stretch`, `baseline` |
| `justify-self-*` | `justify-self` (on a grid child) | `start`, `center`, `end`, `stretch` |
| `text-*` | `text-align` (logical, writing-mode-aware) | `start`, `center`, `end` |
| `flex-row` / `flex-row-reverse` / `flex-col` / `flex-col-reverse` | `flex-direction` (flip `cluster`, `flank`, etc.) | — |

No `flex-left`/`flex-end`/physical-direction aliases. `text-justify`, `place-*` shorthands, `align-content`, and wrap utilities are intentionally out of v1.

#### Theme switching

The system honors `prefers-color-scheme: dark` automatically. To override the system preference, set `data-theme="light"` or `data-theme="dark"` on an ancestor element — canonically `<html>`:

```html
<html data-theme="dark">
```

The framework ships no theme-toggle helper. Persisting a user choice across reloads is one line of JS the user writes:

```js
document.documentElement.dataset.theme = "dark"
```

The dark-mode override applies only to the seven `--color-*` tokens (`--color-bg`, `--color-surface`, `--color-text`, `--color-text-muted`, `--color-primary`, `--color-primary-fg`, `--color-border`). Spacing, radius, type, shadow, and border widths are theme-independent.

---

## Component library

`zero` ships a fixed component library under `.zero/components/`. Import via `"zero/components"`. Components are plain function components in zero's once-per-mount style; stateful props accept signals directly, so a parent owns the lifecycle and the component just reads `.val` / writes `.set()`.

```ts
import { Button, Input, Dialog } from "zero/components";
```

| Component  | What it is                              | Stateful prop |
| ---------- | --------------------------------------- | --- |
| `Avatar`   | Image or initials in a colored circle.  | — |
| `Badge`    | Small inline label.                     | — |
| `Button`   | Primary interactive button.             | — |
| `Card`     | Container with optional title.          | — |
| `Checkbox` | Native checkbox wired to a signal.      | `checked: Signal<boolean>` |
| `Dialog`   | Modal overlay with Esc-to-close.        | `open: Signal<boolean>` |
| `Input`    | Single-line text field wired to signal. | `value: Signal<string>` |
| `Radio`    | Radio button in a named group.          | `selected: Signal<string>` |
| `Select`   | Native `<select>` wired to a signal.    | `value: Signal<string>` |
| `Spinner`  | CSS-only rotating status indicator.     | — |
| `Tabs`     | Tablist with reactive panel content.    | `active: Signal<string>` |
| `TextArea` | Multi-line text field wired to signal.  | `value: Signal<string>` |
| `Toast`    | Fixed-position transient message.       | `open: Signal<boolean>` |
| `Toggle`   | Visual switch wired to a signal.        | `checked: Signal<boolean>` |

### Form inputs

```ts
import { html, signal } from "zero";
import { Input, Button } from "zero/components";

function LoginForm() {
  const email = signal("");
  return html`
    <form>
      ${Input({ value: email, type: "email", label: "Email" })}
      ${Button({ children: "Sign in" })}
    </form>
  `;
}
```

### Display

```ts
import { Card, Badge } from "zero/components";

Card({
  title: "Status",
  children: Badge({ variant: "success", children: "Healthy" }),
});
```

### Overlay

```ts
import { html, signal } from "zero";
import { Dialog, Button } from "zero/components";

const open = signal(false);
html`
  ${Button({ onClick: () => open.set(true), children: "Open" })}
  ${Dialog({ open, title: "Confirm", children: html`<p>Are you sure?</p>` })}
`;
```

### Feedback

```ts
import { html, signal } from "zero";
import { Toast, Spinner } from "zero/components";

const open = signal(false);
html`
  ${Spinner({ size: "sm" })}
  ${Toast({ open, message: "Saved", variant: "success" })}
`;
```

### Overriding component CSS

Every component partial is wrapped in `@layer components`, so any rule in your `styles/app.scss` automatically overrides framework component rules without `!important` or extra specificity. Override tokens for a sweeping change; override class rules for a targeted one.

```scss
// styles/app.scss
@use '../.zero/styles/zero';

// Bump every button's radius.
.button { border-radius: var(--radius-lg); }
```

The in-repo `showcase/` project is the canonical live example — every component rendered in its variants and sizes against a theme switcher.

---

## The .zero/ directory

`.zero/` is the framework's regenerable file boundary. It is hidden from
git (added to `.gitignore` by `zero init`) and is owned by the `zero`
CLI — `zero update` is the only command that writes there. Do not edit
files under `.zero/`. To pick up new framework assets when you upgrade
the CLI, run `zero update`.

Files currently shipped under `.zero/`:

| Path | What it is |
| --- | --- |
| `.zero/zero.d.ts` | TypeScript declarations for the `"zero"` import. |
| `.zero/zero-test.d.ts` | TypeScript declarations for the `"zero/test"` import. |
| `.zero/components.d.ts` | TypeScript declarations for the `"zero/components"` import. |
| `.zero/components/index.ts` | Re-exports every shipped component. |
| `.zero/components/<Name>.ts` | One source file per component (14 total). |
| `.zero/components/<Name>.test.ts` | One test file per component (14 total). |
| `.zero/styles/_tokens.scss` | Design tokens and theme variants. |
| `.zero/styles/_base.scss` | Minimal reset and token-bound `body` rule. |
| `.zero/styles/_layout.scss` | Six layout primitives (`cluster`, `stack`, `frame`, `split`, `flank`, `grid`). |
| `.zero/styles/_utilities.scss` | Gap, padding, and border utility classes. |
| `.zero/styles/_alignment.scss` | Alignment, justify, self, text-align, and flex-direction utility classes. |
| `.zero/styles/_components.scss` | Aggregate that `@use`'s every per-component partial. |
| `.zero/styles/components/_<name>.scss` | One SCSS partial per component (14 total). |
| `.zero/styles/zero.scss` | Aggregate that `@use`'s every partial above. |

### Updating

```bash
zero update             # prints a plan, asks to confirm, refreshes .zero/
zero update --yes       # apply without prompting (CI)
```

In interactive mode (`i` at the top-level prompt) you can accept or
reject each operation one at a time.

---

## Navigation

Plain `<a href="/path">` is intercepted automatically for same-origin links. No `<Link>` component exists or is needed.

```js
html`<a href="/users/42">View user</a>`
```

Programmatic navigation lives in the `"zero"` module:

```js
import { navigate, back, forward, route } from "zero";

navigate("/dashboard");
navigate("/dashboard", { replace: true });          // replaceState instead of pushState
navigate("/users/42", { state: { from: "search" } });

back();
forward();
```

All three throw if no app is running. `navigate` pushes (or replaces) a history entry and runs the navigation pipeline.

`<a>` clicks are **not** intercepted when:

- The click is modified (Cmd/Ctrl/Shift/Alt/middle button).
- The anchor has `target="_blank"` (or any target other than `_self`), `download`, or `data-external`.
- The href is hash-only (`#foo`) or points to a different origin.

### Reading the current route

```js
import { route } from "zero";

function Breadcrumbs() {
  const r = route();
  return html`<span>${() => r.path}</span>`;
}
```

`route()` returns a reactive view `{ path, params, query }`. The getters subscribe to the underlying signals — reading them inside a reactive block or `effect` causes that block to re-run when the route changes.

---

## App-level state

`inject(key)` returns the value registered with `app.state(key, value)` on the currently running app. Throws if no app is running or if `key` was not registered.

```js
import { html, inject } from "zero";

function Counter() {
  return html`<p>Count: ${() => inject("count").val}</p>`;
}
```

The scaffold's `src/routes/home.ts` is the canonical example of this pattern: register a signal in `src/app.ts`, read it via `inject` in a route component.

In tests, `render(tr, { state })` plays the role of `app.state` — see the next section.

---

## Testing

Tests are plain `*.test.js` or `*.spec.js` files. `zero test` discovers them under the project root and runs each in isolation. The test runner ships its own lightweight DOM — no jsdom, no browser.

### Structure

```js
import { describe, it, beforeEach, afterEach, expect } from "zero/test";

describe("Counter", () => {
  beforeEach(() => { /* per-test setup */ });
  afterEach(() => { /* per-test teardown */ });

  it("starts at zero", () => {
    expect(0).toBe(0);
  });
});
```

`describe` may be nested. `beforeAll`/`afterAll` run once per `describe` block; `beforeEach`/`afterEach` run around every `it` in the block.

### DOM helpers

```js
import { render, find, findAll, text, fire, cleanup, expect } from "zero/test";
import { signal } from "zero";
import Home from "./home.ts";

afterEach(cleanup);

it("renders and reacts to clicks", () => {
  const count = signal(0);
  const el = render(Home(), { state: { count } });

  expect(text(el, "p")).toBe("Count: 0");

  fire(find(el, "button"), "click");
  expect(text(el, "p")).toBe("Count: 1");
  expect(count.val).toBe(1);
});
```

- `render(templateResult, { state? })` — commits the template into a fresh container and returns it. The optional `state` map plays the role of `app.state` for the duration of the render; `inject(key)` resolves against this map.
- `find(el, selector)` / `findAll(el, selector)` — `querySelector` / `querySelectorAll` on the lightweight DOM. Selectors compose tag, `#id`, `.class`, `[attr]`, and `[attr=value]` (quoted or unquoted) parts against a single element (e.g. `button.btn[type=submit]`). Combinators (descendant, child, sibling), pseudo-classes, and attribute operators beyond `=` are not supported.
- `text(el, selector?)` — concatenated text content. With a selector, queries from `el` first and throws if nothing matches.
- `fire(el, type, data?)` — dispatches a synthetic event. The synthetic event object provides `preventDefault`, `stopPropagation`, and `defaultPrevented`; merge in extra fields (`key`, `target`, etc.) via `data`.
- `cleanup()` — disposes every scope created by `render` since the last `cleanup` and unregisters the test's state map. Wire it into `afterEach`.

### Assertions

`expect(value)` returns a matcher object. Currently implemented:

- `.toBe(expected)` — strict equality (`===`).
- `.toEqual(expected)` — deep equality. Plain objects, arrays, and signal-shaped objects are compared by structure.
- `.toBeTruthy()`, `.toBeFalsy()` — boolean coercion.
- `.toBeNull()` — strict equality with `null`.
- `.toContain(item)` — substring (for strings) or element membership (for arrays).
- `.toThrow(message?)` — `actual` must be a function. Asserts it throws; if `message` is given, the thrown error's message must contain it.
- `.toBeTemplateResult()` — value has the shape of a `TemplateResult` (returned by `html\`\``).
- `.toHaveBeenCalled()` — `actual` must be a spy. Passes if the spy recorded at least one call.
- `.toHaveBeenCalledTimes(n)` — passes if the spy was called exactly `n` times. Failure message includes recorded `callCount` and the full call log.
- `.toHaveBeenCalledWith(...args)` — passes if any recorded call's args deep-equal `args` (same algorithm as `.toEqual`).
- `.toHaveBeenLastCalledWith(...args)` — passes if only the most recent call's args deep-equal `args`.

`expect().toMatchSnapshot()` is **not implemented yet** — it currently throws.

### Testing components

The pattern: render the component, query, optionally dispatch events, assert on the resulting DOM.

```js
import { describe, it, expect, afterEach } from "zero/test";
import { render, find, text, fire, cleanup } from "zero/test";
import { signal } from "zero";
import Home from "./home.ts";

describe("Home", () => {
  afterEach(cleanup);

  it("increments on click", () => {
    const count = signal(0);
    const el = render(Home(), { state: { count } });

    fire(find(el, "button"), "click");
    expect(text(el, "p")).toBe("Count: 1");
    expect(count.val).toBe(1);
  });
});
```

The scaffold's `src/routes/home.test.ts` is the canonical shape — start from there.

### Spies

`spy(impl?)` returns a callable that records every invocation. Pass it as a prop, callback, or argument anywhere a function is expected; assertions about how it was called use the `toHaveBeenCalled*` matchers above.

```js
import { it, expect, spy, render, find, fire, cleanup, afterEach } from "zero/test";
import Button from "./Button.ts";

afterEach(cleanup);

it("calls onSelect on click", () => {
  const onSelect = spy();
  const el = render(Button({ label: "Go", onSelect }));
  fire(find(el, "button"), "click");
  expect(onSelect).toHaveBeenCalledTimes(1);
  expect(onSelect).toHaveBeenLastCalledWith();
});
```

Properties on a spy (all live, read every call):

- `.calls` — array of argument-arrays, one per invocation.
- `.callCount` — `calls.length`.
- `.results` — array of `{ type: "return" | "throw", value }`, one per invocation.
- `.instances` — array of `this`-bindings observed.

Methods (all return the spy for chaining):

- `.mockReturnValue(v)` — subsequent calls return `v`.
- `.mockResolvedValue(v)` — subsequent calls return `Promise.resolve(v)`.
- `.mockRejectedValue(e)` — subsequent calls return `Promise.reject(e)`.
- `.mockImplementation(fn)` — replace the underlying impl.
- `.reset()` — clear `.calls`, `.results`, `.instances`. The implementation is preserved; if you need a fresh impl too, construct a new spy.

Spies are plain values, not registered resources. `cleanup()` does **not** reset them — wire a `beforeEach` if a spy is shared across tests in a `describe`.

### Testing reactivity directly

`signal` / `computed` / `effect` work outside of a render context.

```js
import { it, expect } from "zero/test";
import { signal, computed } from "zero";

it("recomputes when a source changes", () => {
  const price = signal(10);
  const qty = signal(3);
  const total = computed(() => price.val * qty.val);

  expect(total.val).toBe(30);
  price.set(20);
  expect(total.val).toBe(60);
});
```

### Testing route matching

`app.match(input)` exercises the route table without mounting to the DOM.

```js
import { it, expect } from "zero/test";
import { App } from "zero";

it("extracts params and query", () => {
  const app = new App();
  app.route("/users/:id", () => html``);

  const m = app.match("/users/42?tab=posts");
  expect(m.params.id).toBe("42");
  expect(m.query.tab).toBe("posts");
});
```

---

## JSDoc conventions

Every JavaScript file in a zero project is fully JSDoc-annotated. The rules:

- Every exported function, class, and class method has `@param`, `@returns`, and `@template` where applicable.
- Module-level variables have `@type`.
- `@internal` marks exports that are not part of the public API.
- `@private` marks private class methods.

Canonical shape, taken from the scaffold's `src/routes/home.ts`:

```js
import { html, inject } from "zero";

/**
 * @typedef {import("zero").TemplateResult} TemplateResult
 */

/**
 * @returns {TemplateResult}
 */
function Counter() {
  return html`<p>Count: ${() => inject("count").val}</p>`;
}

/**
 * @returns {TemplateResult}
 */
export default function Home() {
  return html`
    <h1>Hello from zero</h1>
    <button @click=${() => inject("count").update(n => n + 1)}>Increment</button>
    ${Counter()}
  `;
}
```

`TemplateResult` is the type returned by `html\`\``. Import the type via `@typedef` rather than from `"zero"` at runtime.

---

## Common pitfalls

- **Components run once per mount.** Putting `signal()` at module scope shares the signal across every mount of that component. Put per-instance state inside the function body.
- **Reactive reads need a reactive context.** Reading `signal.val` from a plain expression in a template (e.g. `${count.val}`) takes a snapshot at template construction and never updates. Use the bare signal (`${count}`) or a reactive block (`${() => count.val}`).
- **`each` re-renders the whole list.** There is no keyed reconciliation. If a list mutates often and its items are expensive, restructure: keep stable per-item signals out of the array.
- **`inject(key)` throws on unknown keys.** Register every key with `app.state` (in code) or in `render(tr, { state })` (in tests) before any component reads it.
- **`app.run` must be called exactly once.** Builder methods (`state`, `use`, `route`, `layout`, `loading`, `error`) all throw if called after `run`.
- **`navigate`/`back`/`forward`/`route()`/`inject()` require a running app.** They throw outside of `app.run` and outside of `render(...)`.
- **Same-origin `<a>` clicks are intercepted by default.** Opt out with `target="_blank"`, `download`, or `data-external` on the anchor.
- **`.throttle` and `.debounce` use a fixed 100ms interval.** Not configurable today.
