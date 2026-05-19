# Zero Framework — Technical Specification & Implementation Guide

## What This Is

A specification for **zero** — a zero-dependency, batteries-included web framework distributed as a single CLI binary. This document is a handoff for implementation. It describes every major subsystem, the design decisions behind them, and the expected behavior.

## Philosophy

- **Zero npm dependencies.** No `node_modules`. The CLI is the framework.
- **Single binary.** One tool does everything: dev server, transpiler, test runner, builder, formatter, linter, code generator.
- **No inheritance.** No classes for components. No `extends`. Composition via functions and plain objects.
- **No magic.** The developer writes the HTML entry point. The developer writes the boot script. The framework does what you told it to do, nothing more.
- **Functions, not classes.** Components are plain functions. State is reactive primitives. Everything is a value.

---

## 1. CLI Interface

The CLI is the sole entry point. No config files (except a `tsconfig.json` emitted for editor support — the CLI ignores it).

```
zero — the zero-dependency web framework

Usage: zer <command> [options]

Commands:
  zero new <name>             Scaffold a new project
  zero update                 Refresh framework files in .zero/
  zero dev                    Start dev server with HMR
  zero build                  Production build
  zero test [pattern]         Run tests
  zero mutate [pattern]       Run mutation testing
  zero check                  Type-check the project
  zero fmt                    Format all source files
  zero lint                   Lint all source files
  zero gen component <name>   Generate a component
  zero gen route <path>       Generate a route
  zero preview                Serve the production build locally
  zero upgrade                Self-update the CLI

Global Options:
  -q, --quiet              Suppress non-error output
  -v, --verbose            Verbose logging
  --no-color               Disable colored output
  --version                Print version
  -h, --help               Show help
```

### Subcommand Details

#### `zero new <name>` / `zero init`

Scaffolds a new project. `zero init` is the implemented entry point today; it walks an interactive prompt for `zero.toml`, then prints a plan of the files it will create and waits for confirmation. Pass `--yes` / `-y` to skip the prompt — intended for scripts and CI.

```
zero new my-app       → Create in ./my-app
zero new .            → Scaffold in current directory
zero init --yes       → Scaffold non-interactively in the current zero.toml's project root
```

Generated structure:

```
my-app/
├── index.html
├── tsconfig.json          # editor use only — zero ignores this
├── src/
│   ├── app.ts             # app configuration and routing
│   ├── routes/
│   │   └── home.ts        # default home route
│   └── components/        # empty, ready for components
└── styles/
    ├── vars.css            # CSS custom properties
    └── app.css             # application styles
```

#### `zero update`

Refreshes framework files in `.zero/` from the embedded binary. Compares
each file under `.zero/` against what the current CLI version would emit
and produces an Add / Update / Remove plan. Prints the plan, asks for
confirmation, and applies the operations the user accepts. Never writes
outside `.zero/`.

```
zero update              Print plan, prompt [Y/n/i], then apply
zero update --yes, -y    Skip the top-level prompt and apply everything
```

At the top-level `Apply all? [Y/n/i]` prompt:

- `Y` (default): apply every operation.
- `n`: abort, no changes made.
- `i`: enter interactive mode — `y`/`n` per operation, followed by a
  final `Apply? [Y/n]` re-confirm on the filtered plan.

If `.zero/` is already byte-identical to the binary's manifest, `zero
update` prints `"zero update: .zero/ is already up to date."` and exits
0. Declined operations are not an error: exit code is always 0 whether
the user accepted everything, nothing, or some subset. CI scripts that
want strictness should use `--yes`.

`zero update` bootstraps a missing `.zero/` automatically — the
directory is created on demand when the first framework file is
written. The only hard precondition is that `zero.toml` exists.

#### `zero dev`

```
--port, -p <n>     Port (default: 3000)
--host <addr>      Bind address (default: localhost)
--open, -o         Open browser on start
--https            Enable self-signed TLS
```

File watching with full-page reload is always on; HMR (module state preservation) and an in-page error overlay are planned (see Phase 6). Errors render in the terminal today.

#### `zero build`

```
--out, -o <dir>    Output directory (default: dist/)
--analyze          Print bundle size breakdown
--sourcemap        Emit source maps (default: off)
--target <env>     "static" | "server" | "worker" (default: static)
```

Outputs plain HTML/CSS/JS. Framework runtime should be under ~4KB.

#### `zero test [pattern]`

```
zero test                 Run all *.test.ts and *.spec.ts files
zero test auth            Run tests matching "auth"
zero test --watch         Re-run on file change
zero test --coverage      Print coverage report
zero test --update-snapshots  Refresh snapshot files
```

Built-in test API. No jsdom. Components render in z's own lightweight DOM implementation.

#### `zero mutate [pattern]`

Runs mutation testing over `src/`. Discovers every test file, runs them
once as the baseline, then for each mutation site re-runs the relevant
tests with the mutated source overlaid. See `issues/test-improvements/`
for the full spec.

```
zero mutate                    Mutate every covered `src/` file
zero mutate src/foo.ts         Mutate one file (path or substring)
zero mutate --operators arith  Restrict to operator families (CSV)
zero mutate --max-mutants 50   Cap total mutants generated
zero mutate --threads 4        Run N mutants in parallel
zero mutate -q, --quiet        Summary only; suppress per-mutant lines
```

Output: terminal summary plus a programmatic `mutation/mutation.json`
(see §3.5 of `issues/test-improvements/spec.md`). Exit code is non-zero
iff any mutant survived or errored.

#### `zero gen`

```
zero gen component Button        → src/components/Button.ts
zero gen component ui/Card       → src/components/ui/Card.ts
zero gen route /about            → src/routes/about.ts
zero gen route /users/:id        → src/routes/users/[id].ts
```

#### `zero check`

Full TypeScript type-checking. Strict mode always. No separate tsconfig needed — zero knows its own type surface.

#### `zero fmt` / `zero lint`

Built-in formatter and linter. No prettier, no eslint. Opinionated defaults, no config.

---

## 2. Entry Point & Boot Sequence

The developer owns the HTML file. The framework does not generate it, modify it, or inject into it.

### index.html

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>My App</title>
  <link rel="stylesheet" href="/styles/vars.css">
  <link rel="stylesheet" href="/styles/app.css">
</head>
<body>
  <div id="app"></div>
  <script type="module">
    import app from "./src/app.ts"
    app.run("#app")
  </script>
</body>
</html>
```

### src/app.ts

The app module builds and exports a configured app object. No side effects on import. The `run()` call is in `index.html`.

```ts
import { App, signal } from "z"
import { Layout } from "./components/Layout"

const app = new App()

// global state — plain signals
app.state("auth", signal({ status: "loggedOut", user: null }))
app.state("theme", signal("light"))

// middleware
app.use(({ route, state, redirect }) => {
  if (route.meta?.protected && state.auth.val.status === "loggedOut") {
    redirect("/login")
  }
})

// routes — lazy loaded
app.route("/", () => import("./routes/home"))
app.route("/login", () => import("./routes/login"))
app.route("/users/:id", () => import("./routes/users"))
app.route("*", () => import("./routes/404"))

// layout
app.layout(Layout)

// global loading/error UI
app.loading(() => html`<div class="loading-bar"></div>`)
app.error(({ error, retry }) => html`
  <div class="error">
    <h1>${error.status === 404 ? "Not found" : "Something broke"}</h1>
    <button @click=${retry}>Retry</button>
  </div>
`)

export default app
```

### Boot Sequence (internal)

When `app.run("#app")` is called:

```
1. Find the target element (#app)
2. Match current URL to route table
3. Run middleware chain
4. Run route guard (if any)
5. Call route's load() function (if any)
6. Render layout component with route component as children
7. Set up history listener for future navigations
```

### Key Design Rules

- `new App()` creates a configurable instance, nothing else
- `app.run()` is the only method with side effects (mounts to DOM, starts listening to history)
- `app.ts` exports the configured app — `index.html` calls `.run()`
- Multiple app instances on one page are fully supported
- Chaining is supported but optional: `new App().state(...).route(...).run("#app")`

---

## 3. Component Model

### Core Rule

Components are plain functions that return a `TemplateResult` via the `html` tagged template literal.

```ts
import { html, signal } from "z"

function Counter() {
  const count = signal(0)

  return html`
    <div class="counter">
      <p>Count: ${count}</p>
      <button @click=${() => count.set(count.val + 1)}>+</button>
      <button @click=${() => count.set(count.val - 1)}>-</button>
    </div>
  `
}
```

### Critical Behaviors

- The function runs **once**. Not on every update. Once.
- The function returns a `TemplateResult` — a lightweight object describing DOM structure. Not a string. Not actual DOM nodes (yet).
- DOM is only created when the framework **commits** the `TemplateResult` (during `app.run()`, reactive block evaluation, or `each()` rendering).
- Reactive updates are **granular** — when a signal changes, only the specific text node / attribute / block that references it updates. The component function does not re-run.

### TemplateResult Type

```ts
interface TemplateResult {
  _template: Template     // cached parsed structure (shared across calls)
  _values: any[]          // the dynamic ${...} values for this instance
}

interface Template {
  strings: TemplateStringsArray   // static parts
  element: DocumentFragment       // cloneable DOM template
  parts: Part[]                   // where to insert dynamic values
}
```

### Valid Template Values

These types can appear inside `${...}` in a template:

```ts
type TemplateValue =
  | string                    // static text
  | number                    // coerced to string
  | boolean                   // for attributes
  | null                      // renders nothing
  | undefined                 // renders nothing
  | Signal<any>               // reactive — auto-subscribes, updates on change
  | TemplateResult            // nested template
  | TemplateValue[]           // list of any of the above
  | () => TemplateValue       // reactive block — re-evaluates when dependencies change
```

### Props

Props are plain objects. Never reactive by default.

```ts
function UserCard(props: { name: string, age: number }) {
  return html`
    <div class="card">
      <h2>${props.name}</h2>
      <p>Age: ${props.age}</p>
    </div>
  `
}

// usage — called as a function, not a custom element
UserCard({ name: "Alice", age: 30 })
```

If a parent passes a signal, it stays reactive through the prop:

```ts
function Parent() {
  const name = signal("Alice")
  return html`<div>${UserCard({ name })}</div>`
}

function UserCard(props: { name: Signal<string> }) {
  return html`<h2>${props.name}</h2>`  // auto-unwraps the signal
}
```

### Children

Children are just a prop. A `TemplateResult` is a value like any other.

```ts
function Card(props: { title: string, children: any }) {
  return html`
    <div class="card">
      <h2>${props.title}</h2>
      <div class="card-body">${props.children}</div>
    </div>
  `
}

// usage
Card({
  title: "Welcome",
  children: html`<p>Hello world</p>`
})
```

Multiple slots are just multiple props:

```ts
function Page(props: { header: any, sidebar: any, children: any }) {
  return html`
    <div class="page">
      <header>${props.header}</header>
      <aside>${props.sidebar}</aside>
      <main>${props.children}</main>
    </div>
  `
}
```

### Event Handling

Events use the `@` prefix. Modifiers are dot-separated.

```ts
// basic
html`<button @click=${handler}>Go</button>`

// inline
html`<button @click=${() => count.set(0)}>Reset</button>`

// with event object
html`<input @input=${(e) => name.set(e.target.value)} />`

// modifiers
html`<form @submit.prevent=${handleSubmit}>...</form>`
html`<input @keydown.enter=${submit} />`
html`<button @click.once=${initialize}>Init</button>`
html`<div @scroll.throttle=${handleScroll} />`
```

Modifier behaviors:
- `.prevent` — calls `e.preventDefault()`
- `.stop` — calls `e.stopPropagation()`
- `.once` — listener fires once then removes itself
- `.enter`, `.escape`, `.space`, etc. — key filters for keyboard events
- `.throttle` — throttle the handler
- `.debounce` — debounce the handler

### Conditional Rendering

Use a function (reactive block) that returns markup:

```ts
function AuthStatus() {
  const auth = inject("auth")

  return html`
    <div>
      ${() => {
        if (auth.val.status === "loggedIn") return html`<span>Welcome</span>`
        if (auth.val.status === "loading") return html`<span>Loading...</span>`
        return html`<a href=${"/login"}>Log in</a>`
      }}
    </div>
  `
}
```

No `v-if`, no `#if`. Just JavaScript in a function.

### List Rendering

`each()` for efficient keyed list rendering:

```ts
function TodoList() {
  const todos = signal([
    { id: 1, text: "Learn z", done: false },
    { id: 2, text: "Build app", done: false }
  ])

  return html`
    <ul>
      ${each(todos, todo => html`
        <li class="todo ${todo.done ? 'done' : ''}">
          ${todo.text}
        </li>
      `, todo => todo.id)}
    </ul>
  `
}
```

Signature: `each(signalOfArray, renderFn, keyFn)`

- Items get individual scopes — removing an item only disposes that item's scope
- Reordering moves DOM nodes without re-creating them
- `.map()` works too but without keyed reconciliation

### Refs

```ts
function AutoFocus() {
  const input = ref<HTMLInputElement>()

  effect(() => {
    if (input.el) input.el.focus()
  })

  return html`<input ref=${input} type="text" />`
}
```

### Component Function Signature

```ts
type Component<P = {}> = (props?: P) => TemplateResult
```

---

## 4. Reactivity System

### Three Primitives

```ts
import { signal, computed, effect } from "z"
```

#### signal(initialValue)

A reactive container for a value.

```ts
const count = signal(0)

count.val                          // read: 0
count.set(5)                       // write: set to 5
count.update(n => n + 1)           // write: update with function
```

#### computed(fn)

A derived value. Recalculates automatically when any signal read inside `fn` changes.

```ts
const price = signal(10)
const quantity = signal(3)
const total = computed(() => price.val * quantity.val)

total.val    // 30 — read only, no .set()
price.set(20)
total.val    // 60 — recalculated automatically
```

#### effect(fn)

A side effect that re-runs when its dependencies (signals/computeds read inside `fn`) change.

```ts
const name = signal("Alice")

const stop = effect(() => {
  console.log(`Name is: ${name.val}`)
  return () => {
    // cleanup — runs before re-execution and on dispose
    console.log("cleaning up")
  }
})

name.set("Bob")    // logs "cleaning up", then "Name is: Bob"
stop()             // dispose the effect
```

### Dependency Tracking

Dependencies are tracked **automatically**. Any `.val` read inside a `computed()` or `effect()` is registered as a dependency. No dependency arrays.

```ts
const a = signal(1)
const b = signal(2)
const c = signal(3)

const sum = computed(() => {
  if (a.val > 5) {
    return a.val + b.val  // depends on a and b
  }
  return a.val + c.val    // depends on a and c
})
// dependencies are re-tracked on each execution
```

### Ownership Scopes

Every component creates an **ownership scope**. Signals, effects, and event listeners created within that scope are registered to it. When the scope is disposed (component unmounted), everything is cleaned up automatically.

```
Component function called
  │
  ├─ new scope created
  ├─ signal() → registered with scope
  ├─ effect() → registered with scope
  ├─ event listeners → registered with scope
  └─ child components → child scopes (nested)
      │
      └─ scope.dispose()
          ├─ remove all event listeners
          ├─ unsubscribe all signal subscriptions
          ├─ run all effect cleanup functions
          └─ recurse into child scopes
```

The developer never manually cleans up anything.

### Inject

Access app-level state (registered via `app.state()`) from any component:

```ts
const theme = inject("theme")      // returns the signal registered under "theme"
const auth = inject("auth")        // returns the signal registered under "auth"
```

Fully typed — zero knows the shape of registered state.

---

## 5. State Machines (Deferred)

State machines (statechart-style, with finite states, hierarchical sub-states, guards, actions, and context) were originally planned as a first-class Phase 4 primitive. They have been **deferred indefinitely**.

### Rationale

The cases the original `machine()` API was meant to cover — auth flows, multi-step UIs, video player modes — model cleanly as `signal({ status, ...data })` registered via `app.state()` and read via the route component's `state` prop or `inject()` for non-route components. Components branch on `.val.status`:

```ts
app.state("auth", signal({ status: "loggedOut", user: null }))

function AuthStatus() {
  const auth = inject("auth")
  return html`
    ${() => auth.val.status === "loggedIn"
      ? html`<span>Welcome ${auth.val.user.name}</span>`
      : html`<a href=${"/login"}>Log in</a>`}
  `
}
```

The finite-phase discriminator that a statechart enforces (which events are legal in which state) is real, but rarely the load-bearing constraint in practice — the UI typically doesn't render the buttons that would dispatch illegal events. Adding a 300-line primitive plus generator subcommand plus integration surface for a payoff that rarely materializes is the wrong trade for a "zero, no magic" framework.

### Reservation

If a concrete application later demands phase-bounded action legality that signals don't express well, the slot in the API surface is reserved:

- `machine(definition)` factory; `factory()` produces an instance
- `m.current`, `m.ctx`, `m.send(event)`, `m.in(state)`, `m.settled()`
- `app.on(stateKey, stateName, handler)` for machine-to-machine wiring
- `zero gen machine <name>` CLI subcommand
- `src/machines/` directory in the project scaffold

Until then: model lifecycle state as a plain signal whose value carries a `status` field.

> For the canonical `signal({ status, ... })` pattern in working code, see `BEST_PRACTICES.md` and `examples/tracker/src/stores/auth.ts`.

---

## 6. Router

Explicit route definitions. No file-system conventions.

### Route Definition

```ts
app.route("/", () => import("./routes/home"))
app.route("/about", () => import("./routes/about"))
app.route("/users/:id", () => import("./routes/users"))
app.route("*", () => import("./routes/404"))
```

First match wins. Lazy imports (`() => import(...)`) provide automatic code splitting. Eager imports work too: `app.route("/", HomePage)`.

### Route Module Exports

```ts
// required — the route component
export default function UserPage({ data, params, query }) {
  return html`<h1>${data.user.name}</h1>`
}

// optional — data loading, runs before component renders
export async function load({ params, query, state, fetch }) {
  const res = await fetch(`/api/users/${params.id}`)
  if (!res.ok) throw { status: res.status }
  return { user: await res.json() }
}

// optional — route metadata
export const meta = {
  protected: true,
  title: (data) => `${data.user.name} — My App`
}
```

### Route Component Props

```ts
export default function SomePage({ data, params, query }) {
  // data   — resolved return value of load()
  // params — URL parameters: { id: "42" }
  // query  — query string: { tab: "posts" }
}
```

### Nested Routes

```ts
app.route("/dashboard", () => import("./routes/dashboard"), {
  children: [
    { path: "/",          load: () => import("./routes/dashboard/overview") },
    { path: "/analytics", load: () => import("./routes/dashboard/analytics") },
    { path: "/settings",  load: () => import("./routes/dashboard/settings") }
  ]
})
```

Parent component uses `${children}` to render matched child:

```ts
export default function Dashboard({ children }) {
  return html`
    <div class="dashboard">
      <aside><nav>...</nav></aside>
      <section>${children}</section>
    </div>
  `
}
```

### Route Groups

```ts
import { group } from "z"

group({ guard: requireAdmin }, [
  { path: "/admin/users", load: () => import("./routes/admin-users") },
  { path: "/admin/logs",  load: () => import("./routes/admin-logs") }
])
```

### Navigation

```ts
// plain <a> tags — framework intercepts same-origin clicks
html`<a href="/users/42">View user</a>`

// programmatic
import { navigate, back, forward } from "z"
navigate("/dashboard")
navigate("/dashboard", { replace: true })
navigate("/users/42", { state: { from: "search" } })
back()
forward()
```

No `<Link>` component. No `<router-link>`. Plain `<a>` tags with standard `href`. The framework intercepts same-origin clicks.

### Active Link Styling

The router adds `data-active` and `data-active-exact` attributes to `<a>` tags matching the current route. Style with CSS:

```css
a[data-active] { font-weight: bold; }
a[data-active-exact] { color: var(--color-primary); }
```

### Route Guards

```ts
app.route("/admin", () => import("./routes/admin"), {
  guard: ({ state, redirect }) => {
    if (state.auth.val.user?.role !== "admin") {
      redirect("/")
      return false
    }
  }
})
```

### Loading / Error UI

```ts
// global
app.loading(() => html`<div class="spinner"></div>`)
app.error(({ error, retry }) => html`<p>${error.message}</p>`)

// per-route override
app.route("/users/:id", () => import("./routes/users"), {
  loading: () => html`<div class="skeleton"></div>`,
  error: ({ error }) => html`<p>User not found</p>`
})
```

### Route Transitions

```ts
app.route("/photos/:id", () => import("./routes/photo"), {
  transition: { enter: "fade-in", leave: "fade-out", duration: 200 }
})
```

Framework applies CSS classes during transition. CSS handles the animation.

### Accessing Current Route

```ts
import { route } from "z"

function Breadcrumbs() {
  const r = route()  // reactive
  return html`<span>${() => r.path}</span>`
}
```

### Navigation Lifecycle

```
Click <a href="/users/42">
  → intercept click
  → match route
  → run global middleware (app.use())
  → run route guard
  → show loading component
  → call load({ params, query, state, fetch })
  → update document.title from meta.title
  → run leave transition
  → render route component with { data, params, query }
  → run enter transition
  → pushState to browser history
```

### Route-scoped fetch

The `fetch` injected into `load()` is a thin wrapper around `globalThis.fetch` that threads a navigation-scoped `AbortSignal` into every request. The contract:

- Each navigation owns an `AbortController`. The route scope's disposal hook calls `controller.abort()` so navigating away aborts every in-flight request automatically.
- If `init.signal` is also supplied by the caller, the wrapper composes the two signals — an abort on either signal aborts the request.
- Behavior outside `load()` is unchanged. `globalThis.fetch` is not monkey-patched; components that call `fetch` directly receive no route-scoped signal.
- Aborts surface as the standard `AbortError` from `fetch`. The router catches `AbortError` thrown during a `load()` belonging to a controller it owns and silently drops the result — `app.error()` is not invoked for navigation-driven aborts. Caller-supplied aborts (where the caller's controller fired but the navigation controller did not) propagate to `app.error()` so the developer's own catch still sees the error.

`zero/http`'s per-call `init.fetch` override is the canonical bridge: pass `{ fetch: ctx.fetch }` from `load()` so the underlying request inherits the route abort signal. See §11 for the API and `BEST_PRACTICES.md` for worked examples.

---

## 7. CSS Strategy

**SCSS is the canonical CSS authoring layer.** `.scss` files give you variables, nesting, partials, and the modern Sass module system (`@use` / `@forward`). The framework still forbids scoped styles, CSS modules, CSS-in-JS, and class object syntax — SCSS unlocks variables and nesting, not scoped styling.

The developer writes `.scss` files and loads them via `<link>` tags in `index.html`. Design tokens are authored as CSS custom properties directly on `:root` — no SCSS-variable bridge layer. SCSS still owns nesting, partials, and `@use`/`@forward`; it just does not hold token values.

```html
<link rel="stylesheet" href="/styles/app.scss">
```

```scss
// styles/_tokens.scss — design tokens declared directly as CSS custom properties
:root {
  --color-primary: #2563eb;
  --color-text:    #1a1a1a;
  --space-md:      1rem;
  --radius-md:     4px;
  // …
}

@media (prefers-color-scheme: dark) {
  :root {
    --color-text: #f5f5f5;
    // …only color tokens override in dark mode
  }
}
```

```scss
// styles/app.scss — entry stylesheet
@use 'tokens';
@use 'base';
@use 'layout';
@use 'utilities';

.btn {
  padding: var(--space-sm) var(--space-md);
  border-radius: var(--radius-md);
  &.btn-primary {
    background: var(--color-primary);
    color: var(--color-primary-fg);
  }
}
```

Partials use the standard underscore prefix: `styles/_buttons.scss` is consumed via `@use 'buttons';`. Files whose name starts with `_` are not addressable as standalone stylesheets.

`zero dev` compiles `.scss` on the fly and serves the compiled CSS with an inline source map. `zero build` compiles each top-level `.scss` to hashed CSS in `<out>/assets/` and rewrites the source `<link>`'s href to point at the hashed asset. External source maps are emitted when `[build] sourcemap = true` (default: off). The dev inline sourcemap is gated on `[dev] sourcemap = true` (default: on).

Plain `.css` still works — the dev server and build hash and serve `.css` files unchanged. Use whichever extension fits.

Components use plain string class names:

```ts
function Button(props: { variant: string, children: any }) {
  return html`<button class="btn btn-${props.variant}">${props.children}</button>`
}
```

The only thing `zero build` does with CSS — compiled or not — is hash it, copy it to `<out>/assets/`, and rewrite source-side `<link>` hrefs to the hashed URL.

### 7.1 Design system

The scaffold ships a built-in design-system layer in `.zero/styles/`: a color palette (`_palette.scss`), non-color tokens (`_tokens.scss`), a theme aggregator (`_themes.scss`) with one partial per theme under `themes/` (`_light.scss`, `_dark.scss`), plus `_base.scss`, `_layout.scss`, `_utilities.scss`, `_alignment.scss`, and an aggregate (`zero.scss`) that `@use`'s all of them. The user's `styles/app.scss` is a one-shot, user-owned entry that imports the aggregate via `@use '../.zero/styles/zero';`. All partials are framework-owned — they live under the hidden, `.gitignore`-d `.zero/` directory and refresh via `zero update`.

**Token categories.** Tokens split into a framework-internal color palette, the public semantic color surface, and non-color invariants. All are declared as CSS custom properties on `:root`:

| Category | Tokens |
| --- | --- |
| Color palette (framework-internal) | Five families × 11 steps: `--gray-{50…950}`, `--blue-{50…950}`, `--red-{50…950}`, `--green-{50…950}`, `--amber-{50…950}`. Values from Open Color (MIT). Reserved for framework use; consume `--color-*` semantic tokens in app code. |
| Semantic colors (public) | `--color-bg`, `--color-surface`, `--color-text`, `--color-text-muted`, `--color-border`, `--color-primary`, `--color-primary-fg`, `--color-success`, `--color-success-fg`, `--color-warning`, `--color-warning-fg`, `--color-danger`, `--color-danger-fg` |
| Spacing | `--space-xs`, `--space-sm`, `--space-md`, `--space-lg`, `--space-xl` |
| Radius | `--radius-xs`, `--radius-sm`, `--radius-md`, `--radius-lg`, `--radius-xl`, `--radius-2xl`, `--radius-3xl` (largest step is the fully-rounded pill, 9999px) |
| Font family | `--font-sans`, `--font-mono` |
| Font size | `--font-size-sm`, `--font-size-md`, `--font-size-lg`, `--font-size-xl` |
| Font weight | `--weight-normal`, `--weight-medium`, `--weight-bold` |
| Line height | `--leading-tight`, `--leading-normal` |
| Shadow | `--shadow-sm`, `--shadow-md`, `--shadow-lg` |
| Border width | `--border-thin`, `--border-md`, `--border-thick` |

Theme variants override only the thirteen semantic `--color-*` tokens; everything else is theme-invariant.

**Layout primitives.** Six classes in `_layout.scss`: `cluster`, `stack`, `frame`, `split`, `flank`, `grid`. Each is a single CSS rule; layout primitives never use `margin` for spacing.

**When to reach for which primitive.** This table is mirrored verbatim into the scaffolded `AGENTS.md` so the lint diagnostic, the AGENTS.md table, and this spec all point at one canonical phrasing.

| Primitive | Reach for it when… |
| --- | --- |
| `cluster` | Default choice for any horizontal layout. Wraps for free at narrow widths — use it whenever you'd otherwise reach for `display: flex` on a row of items (toolbars, button groups, chip lists, tag rows, inline metadata). |
| `stack` | Vertical layout where items should *not* spread horizontally — a form body, a card's contents, a sidebar's list of links. (For headers and footers where the row should span full width, prefer `split` or `flank`.) |
| `split` | Two end-anchored groups separated by stretched whitespace. Canonical case: page header with brand on the left and nav/actions on the right. Anywhere `justify-content: space-between` would have been the answer. |
| `flank` | A fixed-size element next to a flexible one. Canonical case: a form row with a label on one side and an input that fills the rest; also media objects (avatar + comment body), inline icons next to flowing text. |
| `grid` | A repeating column layout that auto-fits — card grids, tile lists, dashboard widgets. Override `--grid-min` to tune the breakpoint. Not for two-column page layouts (use `flank`). |
| `frame` | Fixed-aspect-ratio media boxes — video embeds, image thumbnails, hero art. Override `--frame-ratio` to change the ratio. |

**Utility families.** Nine families across two partials, 44 utility classes total. `_utilities.scss`: `gap-{step}` (6 — `0`, `xs`, `sm`, `md`, `lg`, `xl`), `pad-{step}` (6 — same set), `border` / `border-{t,r,b,l}` (5). The `0` step on `gap` and `pad` lets a layout primitive (`class="cluster gap-0"`) cancel its default spacing without writing raw CSS. `_alignment.scss`: `align-{start,center,end,stretch,baseline}` (5), `justify-{start,center,end,between,around,evenly}` (6), `align-self-{start,center,end,stretch,baseline}` (5), `justify-self-{start,center,end,stretch}` (4), `text-{start,center,end}` (3, logical-only), `flex-{row,row-reverse,col,col-reverse}` (4). No `!important`; override is by class-list order, and `_alignment.scss` is `@use`d after `_utilities.scss` in the aggregate so its rules win where they touch the same property.

**Theme switching.** Each theme lives in its own partial under `.zero/styles/themes/` and defines a single Sass `@mixin tokens` containing its `--color-*` assignments. `_themes.scss` owns the selector strategy. Because every theme selector has the same CSS specificity — `:root` (pseudo-class) and `[data-theme="…"]` (attribute selector) are both `(0,1,0)` — the strategy uses source order to break ties: light is declared first on `:root`, the `@media (prefers-color-scheme: dark)` block emits dark next, and the `[data-theme="light"]` / `[data-theme="dark"]` rules are emitted last so an explicit attribute always wins over both the default and the OS preference. The net behavior: `prefers-color-scheme: dark` selects dark mode by default; set `data-theme="light"` or `data-theme="dark"` on `<html>` (or any ancestor) to override the system preference. Each theme mixin also emits `color-scheme: light` / `color-scheme: dark` so native UI (scrollbars, default form controls) inherits the theme. There is no JavaScript theme-toggle helper. Users authoring a brand theme declare the thirteen `--color-*` tokens in their own SCSS under a `[data-theme="brand"]` selector, then `@use` it from `styles/app.scss`.

**Distribution model.** Framework-owned and regenerable. `zero init` writes the partials into `.zero/styles/` (`_palette.scss`, `_tokens.scss`, `_themes.scss`, `themes/_light.scss`, `themes/_dark.scss`, plus `_base.scss`, `_layout.scss`, `_utilities.scss`, `_alignment.scss`, `_components.scss`, the per-component partials under `components/`, and `zero.scss`); `zero update` refreshes them when the CLI ships new content. Users override tokens by re-declaring CSS custom properties in `styles/app.scss` after the framework `@use` line — overriding by re-declaration is preserved, just no longer by editing the file that declares the tokens.

**Component layer.** The shipped component library (§11, `"zero/components"`) contributes one SCSS partial per component under `.zero/styles/components/_<name>.scss`, aggregated by `.zero/styles/_components.scss` and pulled into `zero.scss` via `@use 'components';`. Every component rule is wrapped in `@layer components { … }`, so any rule in `styles/app.scss` (which is unlayered) automatically wins on override without specificity tricks or `!important`.

**Typography.** The framework ships twelve utility classes — `.text-display`, `.text-h1`–`.text-h4`, `.text-eyebrow`, `.text-body`, `.text-small`, `.text-muted`, `.text-code`, `.text-link`, `.divider` — inside `.zero/styles/_typography.scss`, wrapped in `@layer components`. Pick a tag for semantics (e.g. `<h1>` for page outline) and a class for visual intent (`class="text-display"` for hero size). There are no opinionated rules on bare element selectors in `_base.scss`; an unstyled `<h1>` renders with browser defaults.

**Fonts.** Geist (sans, both styles) and Geist Mono (mono, both styles) ship locally in `.zero/fonts/` as four variable-axis `.woff2` files. `_base.scss` declares the four `@font-face` blocks against `/.zero/fonts/...` URLs. No network round-trip to Google Fonts. The dev server serves `/.zero/fonts/*` directly; `zero build` copies the directory into `dist/.zero/fonts/`. The SIL Open Font License text rides alongside as `.zero/fonts/OFL.txt`.

---

## 8. Testing

Built into the CLI. No external test runner.

### Running Tests

```
zero test                    # all *.test.ts / *.spec.ts
zero test auth               # matching "auth"
zero test --watch            # re-run on change
zero test --coverage         # coverage report
zero test --update-snapshots # refresh snapshots
```

### Test API

```ts
import { describe, it, expect, beforeEach, afterEach, beforeAll, afterAll, spy } from "z/test"
```

### DOM Helpers

```ts
import { render, find, findAll, text, fire, cleanup } from "z/test"
```

- `render(templateResult, opts?)` — commit to lightweight DOM, return element
- `find(el, selector)` — querySelector
- `findAll(el, selector)` — querySelectorAll
- `text(el, selector?)` — textContent
- `fire(el, event, data?)` — dispatch event
- `cleanup()` — dispose all rendered components

### No Browser Required

`zero test` uses an in-memory DOM implementation (~1500 lines) that covers what real apps reach for during tests: real-DOM event constructors with capture/target/bubble dispatch, `classList` / `dataset` / `style` / input-shaped element properties, `document.body` / `head` / `documentElement` / `activeElement` / `title`, web storage (`localStorage` / `sessionStorage`), `matchMedia`, `navigator`, `crypto`, observer constructors, and host timers (`setTimeout` / `setInterval` / `requestAnimationFrame`, scheduled through Boa's job queue). No jsdom, no happy-dom, no headless browser. Per-test mutable state — storage, timers, focus, title — auto-resets via `cleanup()`.

This is possible **because components are plain functions**, not web components. They don't depend on `HTMLElement`, `customElements`, or `shadowRoot`. A component is just a function that calls `signal()` and returns `html\`...\``. Both work without a browser.

### Testing Signals

```ts
it("updates computed values", () => {
  const price = signal(10)
  const tax = computed(() => price.val * 0.2)
  expect(tax.val).toBe(2)
  price.set(20)
  expect(tax.val).toBe(4)
})
```

### Testing Components

```ts
it("increments on click", () => {
  const el = render(Counter())
  expect(text(el, "p")).toBe("Count: 0")
  fire(find(el, "button"), "click")
  expect(text(el, "p")).toBe("Count: 1")
})
```

### Spies

`spy(impl?)` records every call. Pass it as a prop or callback; assert with `toHaveBeenCalled*` matchers.

```ts
it("calls onSelect on click", () => {
  const onSelect = spy()
  const el = render(Button({ onSelect }))
  fire(find(el, "button"), "click")
  expect(onSelect).toHaveBeenCalledTimes(1)
})
```

### Testing with Inject

```ts
it("shows login when logged out", () => {
  const el = render(NavBar(), {
    state: { auth: signal({ status: "loggedOut", user: null }) }
  })
  expect(text(el, "a")).toBe("Log in")
})
```

### Testing Routes

```ts
it("extracts params", () => {
  const match = app.match("/users/42")
  expect(match.params.id).toBe("42")
})
```

### Testing Middleware

```ts
it("redirects unauthenticated users", () => {
  const result = app.testMiddleware("/dashboard", {
    auth: signal({ status: "loggedOut", user: null })
  })
  expect(result.redirected).toBe("/login")
})
```

### Testing Route Loaders

Loaders receive `fetch` as a parameter for testability:

```ts
it("loads a user", async () => {
  const data = await load({
    params: { id: "42" },
    query: {},
    state: {},
    fetch: () => Promise.resolve({
      ok: true,
      json: () => Promise.resolve({ id: 42, name: "Alice" })
    })
  })
  expect(data.user.name).toBe("Alice")
})
```

### Assertions

```
expect(val).toBe(expected)
expect(val).toEqual(expected)           // deep equality
expect(val).toBeTruthy()
expect(val).toBeFalsy()
expect(val).toBeNull()
expect(val).toContain(item)
expect(val).toThrow(message?)
expect(val).toBeTemplateResult()
expect(val).toMatchSnapshot()
```

### In-Runner Capabilities

Beyond running tests, the runner ships first-class support for:

- **Failure source maps.** Every assertion failure carries a `SourceLoc`
  back through SWC's source map; the reporter prints
  `at <relpath>:<line>:<col>` plus a 5-line snippet with a caret under
  the failing column.
- **Coverage.** `zero test --coverage` instruments every `src/` file at
  load time, tallies per-line and per-function hits, prints a terminal
  table sorted by ascending coverage, and writes `coverage/coverage.json`.
- **Mutation testing.** `zero mutate` (separate subcommand — see §1)
  runs eight mutation-operator families against `src/`, isolates each
  mutant in a child process for panic safety, and reports
  killed / survived / errored counts plus `mutation/mutation.json`.

See `issues/test-improvements/` for the full design.

### E2E Tests

Outside z's scope. Use Playwright or Cypress against `zero dev` or `zero preview`. zero handles unit and integration tests — the 90% case.

---

## 9. Transpiler / Compiler

`z` includes its own TypeScript transpiler. It does NOT do full type-checking at transpile time (that's `zero check`). It strips types and transforms z-specific syntax.

### What the Transpiler Does

1. **Strip TypeScript types** — like esbuild, just remove type annotations
2. **Process `html` tagged templates** — at build time, parse static parts, generate cached DOM templates, wire up dynamic part descriptors
3. **Process `@event` syntax** — inside templates, `@click` becomes `addEventListener("click", ...)`
4. **Process event modifiers** — `.prevent`, `.stop`, `.once`, `.enter`, etc.
5. **Process `<style scoped>` — REMOVED. Not a feature.**
6. **Dynamic imports** — `() => import("./routes/home")` preserved for code splitting
7. **Resolve `import { ... } from "z"` paths** — map to the framework's bundled runtime

### What it Does NOT Do

- Full TypeScript type-checking (that's `zero check`)
- JSX transformation (not used — tagged templates are standard JS)
- CSS processing (CSS is plain files, served/copied as-is)

### html`` Template Compilation (Production Build)

In dev, `html` templates can be processed at runtime (parse strings, create template, cache).

In production (`zero build`), the compiler pre-processes templates for optimal performance:

```ts
// source
html`<p>Count: ${count}</p>`

// compiled (roughly)
const _tpl_1 = z.template("<p>Count: </p>", [{ type: "text", index: 1 }])
z.commit(_tpl_1, [count])
```

The template structure is created once at module load. Each instance just clones and wires up values.

---

## 10. tsconfig.json

Generated by `zero new` for editor support only. The CLI ignores it.

```jsonc
// tsconfig.json — Generated by z. Editor use only.
{
  "compilerOptions": {
    "strict": true,
    "target": "ESNext",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "paths": {
      "@/*": ["./src/*"]
    },
    "types": ["z/types"]
  },
  "include": ["src"]
}
```

`zero` ships type definitions for all its exports (`signal`, `computed`, `html`, etc.) so editors provide full autocomplete and type-checking.

---

## 11. Complete API Surface

### From `"zero"`

```ts
// App
App                                  // class — new App()
app.state(key, value)                // register global state
app.use(middleware)                   // register middleware
app.route(path, loader, opts?)       // register route
app.layout(component)                // set layout component
app.loading(component)               // set global loading UI
app.error(component)                 // set global error UI
app.run(selector)                    // mount and start
app.match(path)                      // test: match a path
app.testMiddleware(path, state)      // test: run middleware

// Reactivity
signal(value)                        // reactive value
computed(fn)                         // derived value
effect(fn)                           // side effect (returns dispose fn)

// Templates
html``                               // tagged template → TemplateResult

// Components
each(signal, renderFn, keyFn)        // keyed list rendering
ref()                                // DOM element reference

// State injection (typed-registry overload, then fallback)
interface StateTypes {}              // empty by default; user-augmented
inject<K extends keyof StateTypes>(key: K): StateTypes[K]
inject<T = unknown>(key: string): T  // fallback for un-registered keys

// Router
navigate(path, opts?)                // programmatic navigation
back()                               // history back
forward()                            // history forward
route()                              // reactive current route info
group(opts, routes)                  // group routes with shared config
```

### From `"z/test"`

```ts
// Structure
describe(name, fn)
it(name, fn)
beforeEach(fn) / afterEach(fn)
beforeAll(fn) / afterAll(fn)

// Assertions
expect(val).toBe / toEqual / toBeTruthy / toBeFalsy / toBeNull
expect(val).toContain / toThrow / toBeTemplateResult / toMatchSnapshot

// DOM
render(template, opts?)
find(el, selector) / findAll(el, selector)
text(el, selector?)
fire(el, event, data?)
cleanup()

// Async
settled()                            // wait for pending effects/transitions
```

### From `"zero/http"`

```ts
// Factory
createHttp(opts?)                    // { fetch?: typeof fetch } → HttpClient

// Client methods
client.use(mw)                       // register middleware; returns client
client.get<T>(url, init?)            // and post / put / patch / delete
client.request<T>(input, init?)      // generic (Request | URL | string)

// Per-call options (HttpInit extends RequestInit)
init.fetch?: typeof fetch            // override the constructor-time fetch
                                     // (canonical use: thread route-scoped
                                     //  fetch inside load() — see §6)

// Errors
HttpError                            // class — status, statusText, body
                                     // (non-2xx responses reject with this)
```

Middlewares run outermost-first on the way down, innermost-first on the way back up — `(req, next) => Promise<Response>`. They may short-circuit (return without calling `next`), wrap (transform the response), or rethrow. JSON request bodies that are plain objects are `JSON.stringify`'d with `Content-Type: application/json`; JSON responses are parsed; other content types return the raw `Response` as an escape hatch.

> For organization patterns (one client per backend, middleware idioms, the 401-redirect example), see `BEST_PRACTICES.md`.

### From `"zero/components"`

```ts
// Form inputs
Button(props?)                       // primary | secondary | ghost | danger
Input({ value, type?, size?, ... })  // single-line text input
TextArea({ value, rows?, ... })      // multi-line text input
Checkbox({ checked, label?, ... })   // signal-backed checkbox
Radio({ selected, name, value, ... })// radio button in a named group
Select({ value, options, ... })      // native select wired to a signal
Toggle({ checked, label?, ... })     // styled switch (role="switch")

// Data
Table({ columns, rows, rowKey, ... }) // sticky-header table over a Signal<T[]>

// Display
Card({ variant?, title?, ... })      // container with optional title
Spinner({ variant?, size?, ... })    // CSS-only rotating status indicator
Badge({ variant?, size?, ... })      // small inline label
Avatar({ alt, src?, initials?, ... })// image or initials in a colored circle

// Overlay
Dialog({ open, size?, title?, ... }) // modal with backdrop + Esc-to-close

// Feedback
Toast({ open, message, variant?, ...})// fixed-position transient message
Tabs({ active, tabs, panels })        // tablist with reactive panel content
```

All components are plain functions matching `Component<P> = (props?: P) => TemplateResult`. Stateful props accept signals directly (parent owns the lifecycle; component reads `.val` and writes via `.set()`). CSS lives under `.zero/styles/components/_<name>.scss`, wrapped in `@layer components` so user CSS in `styles/app.scss` wins on override without `!important`.

### Optional: Web Component Interop

```ts
import { define } from "z/wc"

define("my-counter", Counter, {
  attributes: ["initial-count"]
})
```

Escape hatch for embedding zero components in non-zero pages. Not the primary authoring model.

---

## 12. Implementation Priority

Suggested build order for a proof of concept:

### Phase 1 — Core Reactivity
- [x] `signal(value)` with `.val`, `.set()`, `.update()`
- [x] `computed(fn)` with automatic dependency tracking
- [x] `effect(fn)` with cleanup and dispose
- [x] Ownership scope system (create, nest, dispose)

### Phase 2 — Template System
- [x] `html` tagged template function
- [x] `TemplateResult` type
- [x] `commit()` — convert TemplateResult to real DOM
- [x] Dynamic value types: string, number, signal, function, nested template, array, null
- [x] Event binding with `@` prefix
- [x] Event modifiers (.prevent, .stop, .once, key filters)
- [x] `each()` for keyed list rendering
- [x] Keyed `each()` reconciliation via optional `keyFn`
- [x] `ref()` for DOM access

### Phase 3 — App & Router
- [x] `App` class with `.state()`, `.use()`, `.route()`, `.layout()`, `.run()`
- [x] Route matching with params and wildcards
- [x] `load()` function support
- [x] `navigate()`, `back()`, `forward()`
- [x] History API integration
- [x] `<a>` tag interception
- [x] `inject()` for app-level state access
- [x] `route()` for reactive current route
- [x] Middleware chain
- [x] Route guards
- [x] Nested routes with children
- [x] Loading / error UI
- [x] `data-active` / `data-active-exact` on links

### Phase 4 — Deferred
State machines as a first-class primitive are deferred indefinitely. See Section 5 for rationale and the reserved API slot.

### Phase 5 — Test Runner
- [x] File discovery (*.test.ts, *.spec.ts)
- [x] `describe`, `it`, `expect` API
- [x] Lightweight DOM implementation
- [x] `render()`, `find()`, `text()`, `fire()`, `cleanup()`
- [x] Compound selector grammar in dom-shim
- [x] `spy()` primitive + spy matchers (`toHaveBeenCalled`, `toHaveBeenCalledTimes`, `toHaveBeenCalledWith`, `toHaveBeenLastCalledWith`)

### Phase 6 — CLI & Dev Server
- [x] `zero init` scaffolding
- [x] `zero dev` dev server (file serving, script injection, proxy mode; no HMR)
- [x] `zero dev` file watching (full-page reload via SSE)
- [x] `zero build` production output
- [x] `zero test` integration
- [ ] `zero check` type checking
- [x] `zero lint` design-system rules over user SCSS / CSS (eleven shipped rules: L01 font-weight, L02 font-size, L03 line-height, L04 letter-spacing, L05 color literals, L06 border-radius, L07 border/border-width, L08 padding, L09 margin, L10 gap, L11 layout-primitive detection)
- [ ] `zero fmt`
- [ ] `zero gen` code generation
- [ ] `zero preview` static server

### Phase 7 — Framework Files & Upgrade Path (next; specified in `issues/update/spec.md`)
- [x] `.zero/` directory: hidden, gitignored, framework-owned
- [x] Move `zero.d.ts`, `zero-test.d.ts`, `_tokens.scss`, `_base.scss`, `_layout.scss`, `_utilities.scss`, and the framework SCSS aggregate (`zero.scss`) into `.zero/`
- [x] User's `styles/app.scss` becomes the one-shot, user-owned entry that imports the framework aggregate via relative path
- [x] `zero update` command: rewrites `.zero/` from the embedded binary; never touches user files
- [x] Pre-flight plan + confirmation on `zero init` and `zero update`
- [x] Per-operation accept/reject in `zero update` interactive mode (`Y/n/i`)
- [x] `--yes` / `-y` flag on both commands for scripts/CI

### Phase 8 — Design System Expansion
- [x] Alignment utilities: `align-start`, `align-center`, `align-end`, `align-stretch`, `align-baseline` (sets `align-items`)
- [x] Justify utilities: `justify-start`, `justify-center`, `justify-end`, `justify-between`, `justify-around`, `justify-evenly` (sets `justify-content`)
- [x] Audit for other primitive utilities the layout primitives commonly need (text alignment, flex-direction overrides) and add only the ones with clear demand
- [x] Distribution rides on Phase 7: new partials land under `.zero/styles/`, refresh via `zero update`

### Phase 9 — Component Library
- [x] Set of ready-to-use components built on the design system (15 shipped: Avatar, Badge, Button, Card, Checkbox, Dialog, Input, Radio, Select, Spinner, Table, Tabs, TextArea, Toast, Toggle)
- [x] Components are plain function components and consume only `var(--*)` tokens — they never embed colors, spacing, or radii directly
- [x] A showcase project (`showcase/`) renders every component with a light/dark theme switcher; builds with `zero build`
- [x] Distribution under `.zero/components/` — sources, tests, and SCSS partials regenerable via `zero update`
- [x] Documented in `AGENTS.md` (`## Component library`) and this spec (§7.1, §11)
- [x] Tested with `zero test` — one `*.test.ts` per component, plus framework-side integration tests (`tests/showcase_build.rs`, `tests/showcase_dev.rs`, `tests/component_library.rs`)

### Phase 10 — Internal Quality
- [x] Identify oversized functions across the Rust codebase (target: any function above ~80 lines, or with high cyclomatic complexity)
- [x] Refactor into smaller units with named intermediate steps; cover the seams with unit tests
- [x] Candidates to investigate first: `src/scaffold.rs::write_to` (will be split as part of Phase 7), `src/build/bundler.rs`, `src/dev/server.rs`, anything inside `src/test_runner/`
- [x] No behavioral changes — purely structural

### Phase 11 — Decorators (deferred indefinitely)

Phase 11 was originally scoped to layer a decorator-driven authoring surface — `@Route("/issues/:id")`, `@State("auth")`, `@Meta({ title: ... })` — on top of the framework's function-first model, with the intent of reaching terser route registration, type-inferred state access, and co-located metadata. The blocker is structural: JS/TS decorators only attach to classes and class members, and the framework's route components, stores, and middleware are plain functions. Adopting decorators would force the framework off its function-first stance for a DX payoff that materializes more cleanly as patterns. Same shape as the state-machines deferral in §5: slot reserved, no implementation work planned.

The DX wins Phase 11 chased are delivered by Phase 12 instead:
- Co-located `load`/`meta`/`default` in the route file — no decorator needed; the registration site imports them by name.
- Typed `inject` via the `StateTypes` registry — no generic argument at the call site; module augmentation pins the shape.
- A `BEST_PRACTICES.md` reference plus three shipped example apps that encode the layout decisions.

(The original "Test Improvements" content — better error messages,
coverage, mutation testing, watch mode — was a placeholder under this
slot; three of the four shipped under `issues/test-improvements/`:
source-mapped failure locations + snippets, `zero test --coverage`, and
`zero mutate` as a dedicated subcommand. Watch mode is the one
remaining item and stays unscheduled.)

### Phase 13 — DOM shim expansion (`issues/dom-shim/spec.md`)
- [x] Real `Event` / `CustomEvent` / `KeyboardEvent` / `MouseEvent` constructors with capture/target/bubble dispatch
- [x] Element property surface (`classList`, `dataset`, `style`, `textContent`, `className`, input-shaped properties)
- [x] Document additions (`documentElement` / `head` / `body` / `getElementById` / `activeElement` / `title`)
- [x] Web storage (`localStorage` / `sessionStorage`) with auto-clear in `cleanup()`
- [x] Auxiliary globals (`matchMedia`, `navigator`, `crypto`, Observers, `getComputedStyle`)
- [x] Job-queue-backed timers (`setTimeout` / `setInterval` / `requestAnimationFrame` + `clearXxx`); `cleanup()` cancels pending work via `__clearAllTimers__`

### Phase 12 — Best Practices & Example Applications
- [x] Three shipped example projects under `examples/`: `counter` (~50 LOC), `todos` (mid-size, structured signal + localStorage), `tracker` (full app — auth, routes, guards, HTTP, comments)
- [x] `StateTypes` interface + typed `inject` overload (type-surface only; runtime unchanged)
- [x] Route-scoped `fetch` in `load()` with composable abort signals (§6)
- [x] New `"zero/http"` module shipped on equal footing with `"zero/components"` and `"zero/test"`; `runtime/http.js` + `runtime/http.test.js` + `runtime/zero-http.d.ts` wired through bundler, dev server, test runner, and scaffold manifest
- [x] `BEST_PRACTICES.md` at the repo root — project structure, state organization, stores, status-tagged signals, routes, HTTP, component usage, testing, performance
- [x] `## Best practices` section appended to `src/scaffold/AGENTS.md`
- [x] `public/` asset copying in `zero build` (used by `examples/tracker/public/data.json`)
- [x] Integration tests: `tests/examples_build.rs` and `tests/examples_tests.rs` exercise all three examples

---

## 13. Key Design Decisions Summary

| Decision | Choice | Rationale |
|---|---|---|
| Component model | Pure functions | No lifecycle coupling to browser APIs, trivially testable, no re-render problem |
| Template syntax | Tagged template literals (`html\`\``) | Standard JS, no transpiler needed for syntax, editor support exists (Lit plugin) |
| Reactivity | Signals with auto-tracking | No dependency arrays, no re-render, granular updates |
| DOM strategy | Direct DOM creation, no virtual DOM | Smaller runtime, no diffing algorithm needed |
| CSS | SCSS authoring layer; CSS variables for runtime theming | Variables and nesting are table stakes; runtime theming stays in plain CSS for zero-cost dynamism |
| Design system | Built-in scaffold layer with tokens, themes, layout primitives | Framework-owned regenerable layer under `.zero/`, refreshed by `zero update` |
| Framework-file boundary | Hidden `.zero/` directory, regenerated by `zero update` | Prevents accidental edits to framework-shipped files; gives projects a versioned upgrade path |
| Entry point | Developer-owned index.html | No magic, no hidden HTML generation, full control |
| Boot | `app.run("#app")` in index.html | Explicit, visible, debuggable |
| Routing | Explicit `app.route()` calls | No file-system conventions, ordered matching, readable |
| Testing | Built-in with lightweight DOM | No jsdom, no browser, possible because components are plain functions |
| Distribution | Single CLI binary | Zero npm dependencies, one install, everything included |
| Component library | 15 components shipped under `.zero/components/`; CSS wrapped in `@layer components` | Real apps shouldn't rebuild the same primitives; `@layer` keeps user overrides predictable without prefixing |
| HTTP client | `"zero/http"` module with middleware (onion model) and per-call `init.fetch` override | Every real app fetches; shipping one obvious wrapper avoids divergent conventions across adopters and threads cleanly with the route-scoped abort signal |
