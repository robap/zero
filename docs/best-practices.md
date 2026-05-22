---
title: Best Practices
nav_order: 14
---

# Best Practices

This is the long-form companion to the [user guide](./index.html).
The chapters there define what the framework *does*; this
document covers how to *organize* a real application built on it.
Every section ends with a `→ See …` pointer at a concrete file in
the shipped examples (`../examples/counter/`, `../examples/todos/`,
`../examples/tracker/`).

The framework is intentionally light on opinions in code — it ships
primitives and a small component library, then leaves layout decisions
to the application. These conventions are not enforced by the CLI; they
are the patterns the example apps validate.

---

## 1. Project structure

### Project layout on disk

A zero project is a directory containing a `zero.toml` and a
project-root subdirectory (default `web/`). `zero init` writes this
shape; `zero build` and `zero dev` read `zero.toml` from the working
directory and walk into `<root>/`.

```
my-app/
├── zero.toml          # [project] root = "web", [dev] port = 3000, …
├── dist/              # build output (gitignored)
└── web/               # the project root
    ├── index.html
    ├── tsconfig.json
    ├── src/…
    ├── styles/…
    └── .zero/         # framework-owned (gitignored, refreshed by `zero update`)
```

The shipped `examples/` (`counter`, `todos`, `tracker`) follow this
shape exactly. The default `web` name is a convention, not a
requirement — any non-leading-dot, non-escaping relative path is
accepted by the `zero.toml` parser.

### Bootstrapping `.zero/`

`zero update` materializes the framework files into `<root>/.zero/`.
It auto-creates the directory if missing, so a fresh clone of any
zero project becomes runnable with:

```
zero update --yes
zero dev
```

`zero init` is for scaffolding a brand-new project; `zero update` is
for keeping the framework files in sync with the installed CLI
version.

### Source layout

Real apps benefit from a small, predictable layout under `<root>/`.
Adopt as much of it as your example complexity warrants.

```
src/
  app.ts             # app construction, state registration, route table
  state.ts           # typed key registry (see §2)
  routes/            # one file per route; nested folders for nested routes
    home.ts
    login.ts
    issues/
      index.ts       # /issues
      issue.ts       # /issues/:id  (plain name, no brackets)
  components/        # presentational components only
  stores/            # one module per logical store: signal + mutators
    auth.ts
    issues.ts
  lib/               # non-UI helpers (formatters, validators, guards)
styles/
  app.scss
```

`routes/` holds route components and their co-located `load()` / `meta`
exports. `stores/` owns state mutation. `components/` is for
presentational code only — components never call `signal.set()` on a
store signal. `state.ts` is the single source of truth for `inject`
keys. `lib/` is for pure helpers that don't fit elsewhere.

For dynamic segments, prefer plain names: `routes/issues/index.ts`
(collection) plus `routes/issues/issue.ts` (detail). The
singular/plural pair reads naturally; no bracket syntax is needed
because the router takes the pattern from `app.route(...)`, not from
the filename.

→ See `../examples/tracker/web/src/`.

---

## 2. State organization

State lives in two places:

- **Module-scoped signals** in `src/stores/*.ts` are constructed once
  per page load. They're registered with the app via
  `app.state(Keys.Foo, foo)` and read via `inject(Keys.Foo)` from any
  component.
- **Local signals** declared inside a component function via
  `signal(initial)` capture per-instance UI state (form drafts, hover,
  open/closed). They live for the lifetime of the render scope.

Use `src/state.ts` to declare a typed key registry — a `const Keys`
object plus a module augmentation that pins each key's value type:

```ts
import type { Signal } from "zero";
import type { AuthState } from "./stores/auth.ts";
import type { IssuesState } from "./stores/issues.ts";

export const Keys = {
  Auth:   "auth"   as const,
  Issues: "issues" as const,
} as const;

declare module "zero" {
  interface StateTypes {
    [Keys.Auth]:   Signal<AuthState>;
    [Keys.Issues]: Signal<IssuesState>;
  }
}
```

Component code reads via `inject(Keys.Auth)` with no generic argument
— TypeScript infers `Signal<AuthState>` from the augmented
`StateTypes`. Plain-string `inject<T>("name")` calls still compile
(the framework ships a fallback overload) but lose the registry's
single-source-of-truth guarantee.

### One structured signal vs many

- **One structured signal** when fields update together or constrain
  each other. Auth is the canonical case: `user` is only meaningful
  when `status === "loggedIn"`.
- **Separate signals** when fields update independently. Theme and the
  open-sidebar boolean are the canonical case.

→ See `../examples/tracker/web/src/state.ts` and
`../examples/todos/web/src/state.ts`.

---

## 3. Stores

A store is a plain TypeScript module that exports a signal plus the
functions that legally mutate it. There is no framework class to
extend, no provider tree, no reducer to register — the module *is* the
store.

```ts
// stores/issues.ts
export const issues: Signal<IssuesState> = signal({ items: [], loaded: false });

export function setIssues(items: Issue[]): void { /* ... */ }
export function addComment(id: string, c: Comment): void { /* ... */ }
export function updateStatus(id: string, status: IssueStatus): void { /* ... */ }
```

Component code imports the mutator functions. **Never** call `.set()`
or `.update()` on a store signal from a component or a route — every
mutation goes through a named function in the store module. This keeps
the store the one place behavior changes are authored; tests against
the store stay independent of UI.

The same rule applies to derived state: prefer a `computed()` in the
component that reads `inject(Keys.Foo).val` over a duplicated signal
mirroring it.

→ See `../examples/tracker/web/src/stores/issues.ts` and
`../examples/todos/web/src/stores/todos.ts`.

---

## 4. Status-tagged signals (the §5 working demo)

The framework defers a built-in state-machine primitive (see spec §5).
The recommended replacement is a `signal({ status, ...data })` shape
where `status` is a discriminated-union tag:

```ts
export type AuthState =
  | { status: "loggedOut" }
  | { status: "loading" }
  | { status: "loggedIn"; user: User };
```

Branches in components read `auth.val.status`. TypeScript narrows the
shape inside each branch automatically — there is no per-field
optional-check dance, and impossible combinations (e.g. `loggedOut`
with a `user`) cannot be constructed.

The store function that drives transitions stays in `stores/auth.ts`
so that the legal state graph lives in one place:

```ts
export function login(name: string): Promise<void> {
  if (!name.trim()) return Promise.reject(new Error("..."));
  auth.set({ status: "loading" });
  return Promise.resolve().then(() => {
    auth.set({ status: "loggedIn", user: { id: ..., name } });
  });
}
```

→ See `../examples/tracker/web/src/stores/auth.ts`.

---

## 5. Routes

Routes are functions registered via `app.route(pattern, component,
opts?)`. Co-locate the route's `load`, `meta`, and component in a
single file under `src/routes/`:

```ts
// routes/issues/issue.ts
export const meta = { protected: true, title: "Issue" } as const;

export async function load(ctx: { fetch: typeof fetch }): Promise<void> {
  // Hydrate the store via zero/http; thread the route-scoped fetch
  // so navigating away aborts in-flight requests automatically.
  if (inject(Keys.Issues).val.loaded) return;
  const data = await api.get<{ issues: Issue[] }>("/public/data.json", { fetch: ctx.fetch });
  setIssues(data.issues);
}

export default function IssuePage({ params }: { params: { id: string } }) {
  const issue = computed(() =>
    inject(Keys.Issues).val.items.find((it) => it.id === params.id),
  );
  return html` /* ... */ `;
}
```

Wire all three into the app's route table by importing them at the
registration site:

```ts
import IssuePage, { load as loadIssue, meta as issueMeta }
  from "./routes/issues/issue.ts";

app.route("/issues/:id", IssuePage, {
  load: loadIssue,
  meta: issueMeta,
  guard: requireAuth,
});
```

Two patterns worth pinning:

- **`load()` is side-effect-only.** The framework awaits its result
  but does not pipe the return value into the component as `data`.
  Use the loader to hydrate a store; have the component read from the
  store via `inject`. This keeps the route's reactive surface uniform
  (signals everywhere) and lets a navigation back hit cached state for
  free.
- **`meta` is the natural place for route-scoped policy.** A
  `{ protected: true }` flag plus a `requireAuth` guard composes a
  surprising amount of behavior with no framework extension.

→ See `../examples/tracker/web/src/routes/issues/issue.ts` and
`../examples/tracker/web/src/app.ts`.

---

## 6. HTTP

`zero/http` ships a small, middleware-aware fetch wrapper. Construct
one client per logical backend in `src/lib/`, with **no middleware at
construction time**:

```ts
// src/lib/api.ts
import { createHttp } from "zero/http";
export const api = createHttp();
```

Register middleware in `src/app.ts`, before `app.run()`. The
composition root is the right place for cross-cutting policy: auth
headers, 401 redirects, retry, logging. Keeping middleware out of
stores means a domain store like `stores/issues.ts` owns only state —
HTTP transport is somebody else's problem.

```ts
// src/app.ts
import { api } from "./lib/api.ts";
import { navigate } from "zero";

api.use(async (req, next) => {
  const res = await next(req);
  if (res.status === 401) navigate("/login");
  return res;
});
app.run("#app");
```

`client.use(mw)` mutates the middleware list in place; registering
from `app.ts` before `app.run()` guarantees every middleware is in
place before the first request fires. Apps with multiple backends
declare one client per backend (e.g. `lib/billing.ts`, `lib/auth.ts`)
and register their respective middleware in the same `app.ts` block.

Middlewares run outermost-first on the way down, innermost-first on
the way back up — the standard onion model. Canonical examples:

- **Auth header injector** — `req.headers.set("Authorization", token)`
  before `next(req)`.
- **401 → login redirect** — call `navigate("/login")` when the
  response status is 401; return the response unchanged so the caller
  still sees a rejection via `HttpError`.
- **Short-circuit** — return a synthetic `Response` without calling
  `next()` to mock or cache.

Inside a `load()`, thread the injected `fetch` through `init.fetch`:

```ts
await api.get<T>("/public/data.json", { fetch: ctx.fetch });
```

This routes the request through the route-scoped abort signal: a
mid-load navigation cancels the in-flight fetch and the router
swallows the resulting `AbortError`. See spec §6 for the full
contract.

Non-2xx responses reject with `HttpError` carrying `status`,
`statusText`, and (if JSON) the parsed body. Network failures surface
the underlying `TypeError`; aborts surface as `AbortError`.

→ See `../examples/tracker/web/src/lib/api.ts` and
  `../examples/tracker/web/src/app.ts`.

---

## 7. Component usage

**Prefer `zero/components` over raw HTML for every interactive
primitive.** The shipped library covers `Button`, `Input`, `Checkbox`,
`Toggle`, `Select`, `Radio`, `TextArea`, `Dialog`, `Tabs`, `Card`,
`Avatar`, `Badge`, `Spinner`, and `Toast`. They take props that
include both static values and signals, so they bind reactively
without ceremony.

Reach for raw `<button>` / `<input>` / `<select>` in two specific
cases:

1. **The shipped component cannot express the required behavior.**
   The canonical example is
   `../examples/todos/web/src/components/FilterBar.ts`: the button's variant
   must track a filter signal reactively, and `Button.variant` is not
   a reactive prop. Drop a `//` comment naming the missing capability
   so a reader knows why the rule is broken.
2. **You are building a new presentational component the library
   does not ship.** The canonical example is the per-app `Header`:
   there is no shipped `Header` component, so each app builds one
   from layout classes (`cluster`, `stack`, `pad-*`, `gap-*`) and
   design tokens. When you do this, **wrap shipped primitives** rather
   than re-implement them — `ThemeToggle` is built on top of the
   shipped `Toggle`, not from a raw checkbox.

Plain DOM containers (`<main>`, `<section>`, `<header>`, `<nav>`,
`<ul>`, `<li>`, `<form>`, `<label>`, `<span>`, `<a>`, `<svg>`) are not
"components" under this rule — use them freely.

→ See `../examples/tracker/web/src/components/Header.ts`,
`../examples/tracker/web/src/components/ThemeToggle.ts`, and
`../examples/todos/web/src/components/FilterBar.ts`.

---

## 8. Theming

zero ships two themes — light and dark — plus a 13-token public
`--color-*` semantic surface that every shipped component consumes:
`--color-bg`, `--color-surface`, `--color-text`, `--color-text-muted`,
`--color-border`, `--color-primary` / `--color-primary-fg`,
`--color-success` / `--color-success-fg`, `--color-warning` /
`--color-warning-fg`, and `--color-danger` / `--color-danger-fg`.

**Override an individual token.** Re-declare it in `styles/app.scss`
after the `@use '../.zero/styles/zero';` line. The override applies
globally; place it under a `[data-theme="…"]` selector for a
per-theme override.

```scss
@use '../.zero/styles/zero';

:root {
  --color-primary: #6b46c1;
}
```

**Author a brand theme.** Declare the thirteen `--color-*` tokens
under a `[data-theme="brand"]` selector in your own SCSS partial,
then `@use` it from `styles/app.scss`. Apply via `<html
data-theme="brand">`.

```scss
// styles/_brand.scss
[data-theme="brand"] {
  color-scheme: light;
  --color-bg:          #ffffff;
  --color-surface:     #f6f3ff;
  --color-text:        #1a1a1a;
  --color-text-muted:  #5b5b5b;
  --color-border:      #e0d6ff;
  --color-primary:     #6b46c1;
  --color-primary-fg:  #ffffff;
  --color-success:     var(--green-700);
  --color-success-fg:  #ffffff;
  --color-warning:     var(--amber-500);
  --color-warning-fg:  #1a1a1a;
  --color-danger:      var(--red-700);
  --color-danger-fg:   #ffffff;
}
```

```scss
// styles/app.scss
@use '../.zero/styles/zero';
@use 'brand';
```

**The framework's internal color palette** (`--gray-*`, `--blue-*`,
`--red-*`, `--green-*`, `--amber-*` — 55 tokens) is reachable as CSS
custom properties but is *not* part of the public API. Its values and
step set may change between minor versions. Stick to the `--color-*`
semantic tokens in app code; reach for palette steps only inside a
custom theme partial where you're already committed to one.

### Typography

`_base.scss` does not style bare `<h1>`–`<h6>`, `<p>`, `<a>`,
`<small>`, `<code>`, or `<hr>` — pick a tag for semantics and a
utility class from `.zero/styles/_typography.scss` for visual intent.

```html
<!-- semantic h1 for outline, display-size visual treatment -->
<h1 class="text-display">Hello, world.</h1>

<!-- inline code that should look like a chip -->
Use <code class="text-code">signal</code> for reactive state.

<!-- opt-in link styling -->
See the <a class="text-link" href="/spec">spec</a>.
```

The twelve shipped utilities are `.text-display`, `.text-h1`–
`.text-h4`, `.text-eyebrow`, `.text-body`, `.text-small`,
`.text-muted`, `.text-code`, `.text-link`, and `.divider`. They live
inside `@layer components`, so a re-declaration of the same property
in unlayered `styles/app.scss` always wins.

Geist (sans, normal + italic) and Geist Mono (mono, normal + italic)
ship locally in `.zero/fonts/`; nothing fetches over the network.

---

## 9. Testing

Each example ships unit tests for every store, every route, and every
non-trivial component. The patterns:

- **Store tests** import the signal and its mutators directly, reset
  via a `beforeEach`, and assert the visible state after each
  operation. They never touch the DOM.
- **Component tests** use `render(Component())` from `zero/test`,
  `find`/`findAll` for queries, `fire` for events, and an
  `afterEach(cleanup)` to dispose the render scope. Component tests
  may seed state via `render(..., { state: { [Keys.X]: signal } })`
  to exercise the `inject` path.
- **Route tests** are component tests with a richer state payload.
  Route `load()` functions are *not* invoked by `render()` —
  pre-seed the store the route reads from, then render the page.

→ See `../examples/todos/web/src/stores/todos.test.ts`,
`../examples/todos/web/src/routes/home.test.ts`, and
`../examples/tracker/web/src/routes/issues/issue.test.ts`.

### Testing browser APIs

The test runner ships real in-memory implementations of the browser
APIs apps reach for: `localStorage` / `sessionStorage`, `matchMedia`,
`navigator`, `crypto.randomUUID`, `setTimeout` / `setInterval` /
`requestAnimationFrame`, `IntersectionObserver` /
`ResizeObserver` / `MutationObserver`, and real `Event` /
`KeyboardEvent` / `MouseEvent` constructors with capture/target/bubble
dispatch. App code that calls these globals runs unmodified under
`zero test` — no `typeof window === "undefined"` guards needed at
test time.

Per-test mutable state (storage maps, pending timers, focused
element, document title, body/head subtree) is reset automatically by
`cleanup()` from `zero/test`. The scaffolded `afterEach(cleanup)`
already wires this in.

To assert that a store called a specific Web API, wrap the method
with `spy()` and check the recorded calls:

```ts
import { spy, cleanup } from "zero/test";

beforeEach(() => {
  localStorage.setItem = spy(localStorage.setItem.bind(localStorage));
});
afterEach(cleanup);

it("persists the new todo to localStorage", () => {
  addTodo("buy milk");
  expect(localStorage.setItem).toHaveBeenCalledWith("todos", JSON.stringify([{ text: "buy milk" }]));
});
```

Timers fire in **registration order**, not by wall-clock `ms` — there
is no event loop with real time. The natural way to drain pending
timers in a test is to `await Promise.resolve()` between scheduling
and the assertion; that's what the harness does for promise-driven
test code already. If a test needs to swap a global wholesale (e.g.
`window.matchMedia = (q) => ({ matches: q.includes("dark"), ... })`),
restore the original in `beforeEach` — `cleanup()` does not
auto-restore reassigned globals.

### Testing against the Web Platform

`zero test` ships hand-written implementations of the Web Platform APIs
real apps use at test time: `Headers` / `Request` / `Response` / `fetch`,
`AbortController` / `AbortSignal`, `URL` / `URLSearchParams`,
`TextEncoder` / `TextDecoder`, `Blob` / `File` / `FormData`,
`structuredClone`, and `queueMicrotask`. See
[Testing § Web Platform surface](./testing.html#web-platform-surface)
for the closed list and per-API contracts.

`fetch` is the one intentional stub: its default implementation rejects
with a clear, actionable message. Override `globalThis.fetch` per test
and `cleanup()` restores the default automatically. The canonical
pattern (mirrored on `runtime/http.test.js::makeStubFetch`):

```ts
import { beforeEach, afterEach, cleanup } from "zero/test";

function makeStubFetch(routes: Record<string, unknown>) {
  return async (input: RequestInfo | URL) => {
    const url = typeof input === "string"
      ? input
      : input instanceof Request ? input.url : input.toString();
    const body = routes[url];
    return new Response(JSON.stringify(body), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });
  };
}

beforeEach(() => { globalThis.fetch = makeStubFetch({ "/x": { value: 42 } }); });
afterEach(cleanup);  // restores the default-rejecting stub
```

Anything outside the audited list surfaces as a `ReferenceError`. If your
code needs `ReadableStream`, `WebSocket`, `IndexedDB`, or any other
browser API the runner doesn't ship, stub it yourself in `beforeEach` and
restore in `afterEach`. The runner deliberately refuses to silently mock
— a missing global is meant to be visible.

---

## 10. Performance

A handful of high-leverage rules:

- **Lazy-load every route except the entry route.** Pass a
  `() => import("./routes/issues/index.ts")` loader to `app.route`
  instead of an eager component reference. The framework caches the
  resolved component after the first visit; subsequent navigations
  hit the cache.
- **Split a store when its consumers diverge.** A single
  `signal({ ...everything })` re-runs every subscriber on every
  mutation. If two consumers care about disjoint fields, split into
  two signals.
- **Keep `computed()` bodies narrow.** A `computed` re-runs whenever
  any signal it read on the previous run changes. Reading wide
  surfaces (`store.val` rather than `store.val.items`) widens the
  invalidation graph.
- **Prefer `each()` with a stable key over `.map()` for any list
  that reorders or churns.** `each()` diffs by key; `.map()`
  rebuilds the whole subtree.

→ See `../examples/tracker/web/src/routes/issues/index.ts` for the
`each(visible, ...)` pattern.

---

## Related docs

- [API](./api.html) — flat reference of every public export.
- `crates/zero-scaffold/src/scaffold/AGENTS.md` — agent-facing
  guidance shipped with every scaffolded project; forward-points
  back here.
