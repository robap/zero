---
title: Examples Tour
nav_order: 15
---

# Examples Tour

Three buildable apps under `examples/` cover the framework from
its smallest possible shape through a multi-page app with auth,
guards, and HTTP. Reading them in order is the fastest way to
see how the primitives compose in a real codebase.

Each example is a complete project with its own `zero.toml`:

```sh
cd examples/counter
zero dev
```

## `examples/counter/`

The smallest possible zero app — ~50 lines of code total. What
it demonstrates:

- A single `signal` registered on the app via `app.state`.
- One route, one component, one button.
- `inject<Signal<number>>("count")` reading and writing the
  app-level signal from a child component.
- The `html` tagged template at its simplest — one
  substitution, one event binding.

**Files to read first:**

- `examples/counter/web/src/app.ts` — `new App().state(...)
  .route(...).run(...)` in three lines.
- `examples/counter/web/src/routes/home.ts` — the home
  component, the increment button, the inline `Counter` that
  reads `inject(...).val` via a reactive block.

After this, you've seen every "core loop" piece of the
framework in working code. Everything else is composition.

## `examples/todos/`

A keyed-list app — add, edit, delete, filter todos with
keyboard handling and a module-level store. What it adds on
top of `counter`:

- `each(items, render, keyFn)` for stable list rendering with
  per-item scopes.
- The **module store** pattern — a single signal whose value is
  the whole shape (`{ items: Todo[]; filter: string }`), with
  helper functions to mutate it. Avoids prop-drilling without
  reaching for an app-wide store.
- A typed key registry — augmenting the
  `declare module "zero" { interface StateTypes { ... } }`
  declaration so `inject("todos")` returns the right type.
- Form input handling with `@input`, `@keydown.enter`, and
  conditional rendering via a reactive block.

**Files to read first:**

- `examples/todos/web/src/state.ts` — the app's
  `StateTypes` augmentation; one place to see every key.
- `examples/todos/web/src/stores/todos.ts` — the store
  factory pattern (functions that take and return a `Signal<T>`).
- `examples/todos/web/src/routes/home.ts` — the keyed list
  rendering and the per-item action handlers.

## `examples/tracker/`

A multi-page issue-tracker with auth, route guards, query-param
filters, nested route layouts, and a `zero/http` client wired
end-to-end. The biggest of the three. What it adds:

- An auth flow — a status-tagged store (`{ status:
  "loggedOut" | "loading" | "loggedIn", user }`) and a route
  group gated by a guard that redirects unauthenticated users.
- A `load()` per route that hydrates the store via
  `api.get(url, { fetch })` — see the route-scoped fetch
  threading in [Routing](./routing.html#route-scoped-fetch) and
  [HTTP](./http.html#route-scoped-fetch-threading).
- Nested routes (`/issues`, `/issues/:id`) where the parent
  component renders the matched child via `${children}`.
- Query-param-driven filters with two-way binding to the URL
  (changing the filter rewrites the URL; navigating back
  restores the filter).
- A `zero/http` client per backend, with an auth-header
  middleware and a 401-redirect middleware — the canonical
  one-client-per-backend pattern from
  [Best Practices §6](./best-practices.html#6-http).

**Files to read first:**

- `examples/tracker/web/src/app.ts` — every route registration
  and the `app.use(...)` middleware chain.
- `examples/tracker/web/src/stores/auth.ts` — the
  status-tagged signal pattern.
- `examples/tracker/web/src/routes/issues/` — nested route
  module structure (`index.ts`, `:id.ts`, `layout.ts`).
- `examples/tracker/web/src/lib/api.ts` — the `createHttp`
  client with its middleware stack.

## What to read after

The patterns these examples canonise are written up at length in
[Best Practices](./best-practices.html). Read it once you've
seen the examples — the practices make a lot more sense with
the worked code in mind.
