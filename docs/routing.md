---
title: Routing
nav_order: 6
---

# Routing

zero's router is explicit: you register routes on the `App`,
match the URL against them, and the matching component renders.
No file-system magic, no generated route table, no `<Link>`
component — plain `<a href>` tags are the navigation primitive,
and the framework intercepts same-origin clicks.

## Defining routes

```ts
import { App } from "zero";
import Home from "./routes/home.ts";

const app = new App();
app.route("/", Home);
app.route("/users/:id", UserPage);
app.route("/blog/*", BlogIndex);
app.route("*", NotFound);
app.run("#app");
```

Patterns support:

- **Static segments**: `/about`.
- **Named params**: `/users/:id`. Available to the component as
  `params.id` (always a string).
- **Wildcard**: a bare `*` matches everything that hasn't matched
  yet.

Routes are matched in registration order — first match wins.
Register specific routes before wildcards.

## Lazy routes

For code-splitting, pass a dynamic import:

```ts
app.route("/blog/:slug", () => import("./routes/post.ts"));
```

The framework calls the loader the first time the route is
visited, awaits the module, and treats the module's `default`
export as the route component.

## The route module's exports

A route module looks like this:

```ts
// src/routes/post.ts
import { html } from "zero";
import type { TemplateResult } from "zero";

export const meta = {
  title: ({ data }: any) => `${data.post.title} — My Blog`,
};

export async function load({ params, fetch }: any) {
  const res = await fetch(`/api/posts/${params.slug}`);
  if (!res.ok) throw { status: res.status };
  return { post: await res.json() };
}

export default function Post({ data }: any): TemplateResult {
  return html`
    <article>
      <h1>${data.post.title}</h1>
      <p>${data.post.body}</p>
    </article>
  `;
}
```

- **`default`** — the route component. Receives
  `{ data, params, query, state }`.
- **`load(ctx)`** — optional async function called before render.
  Resolves to the `data` prop on the component (the framework
  also awaits side-effectful loaders that don't return).
- **`meta`** — optional metadata. The router reads `meta.title`
  (a string or function) to update `document.title`. Other keys
  (`protected`, app-specific tags) flow through unchanged.

For larger apps, co-locate route, loader, and meta in one file
per the pattern in
[Best Practices §5](./best-practices.html#5-routes).

## The `load()` contract

```ts
export async function load({ params, query, state, fetch }) {
  // params — { id: "42" }     URL params
  // query  — { tab: "posts" } parsed search string
  // state  — your app.state() store, keyed
  // fetch  — route-scoped fetch (see below)
}
```

The framework awaits `load` before rendering, so the component
sees the data its UI needs. If `load` throws, the router routes
the error to `app.error()` (or a per-route override).

A common pattern is "side-effect-shaped" — the loader hydrates a
store, and the component reads via `inject` rather than `data`:

```ts
export async function load({ fetch }: any) {
  const res = await fetch("/api/me");
  authStore.set(await res.json());
}

export default function Profile() {
  const user = inject<Signal<User>>("user");
  return html`<h1>${() => user.val.name}</h1>`;
}
```

That style scales better when many components on a page need the
same data and you'd rather thread state through the store than
through every component's `data` prop. See
[Best Practices §5](./best-practices.html#5-routes) for the
rationale.

## Route guards

```ts
app.route("/admin", AdminPage, {
  guard: ({ state, redirect }) => {
    if (state.auth.val.user?.role !== "admin") {
      redirect("/");
      return false;
    }
  },
});
```

The guard runs after middleware and before `load()`. Return
`false` (and call `redirect`) to short-circuit the navigation.

## Nested routes

```ts
app.route("/dashboard", Dashboard, {
  children: [
    { path: "/",          load: () => import("./routes/dashboard/overview.ts") },
    { path: "/analytics", load: () => import("./routes/dashboard/analytics.ts") },
    { path: "/settings",  load: () => import("./routes/dashboard/settings.ts") },
  ],
});
```

The parent component renders the matched child via the
`children` prop:

```ts
export default function Dashboard({ children }: any) {
  return html`
    <div class="dashboard">
      <aside><nav><a href="/dashboard/analytics">Analytics</a></nav></aside>
      <section>${children}</section>
    </div>
  `;
}
```

## Navigation

```ts
// declarative — the framework intercepts same-origin clicks
html`<a href="/users/42">View user</a>`

// programmatic
import { navigate, back, forward } from "zero";
navigate("/dashboard");
navigate("/dashboard", { replace: true });
navigate("/users/42", { state: { from: "search" } });
back();
forward();
```

There is no `<Link>` or `<router-link>` component. A plain `<a
href>` with a relative or same-origin URL is the right primitive
99% of the time; reach for `navigate()` only when you're firing
the navigation from non-link UI (a button, a `setTimeout`).

## Active-link styling

The router adds attributes to the matching `<a>` tags so you can
style them in CSS without JS bookkeeping.

| Attribute            | When set                                                                      |
|----------------------|-------------------------------------------------------------------------------|
| `data-active`        | The link's `href` is a prefix of the current path (covers parent navigation). |
| `data-active-exact`  | The link's `href` exactly matches the current path.                           |

```css
a[data-active]       { font-weight: bold; }
a[data-active-exact] { color: var(--color-primary); }
```

## Route-scoped fetch

The `fetch` injected into `load()` is a thin wrapper around the
global `fetch`. It threads a navigation-scoped `AbortSignal` into
every request, so:

- Each navigation owns an `AbortController`. The route scope's
  disposal hook calls `controller.abort()` when the user
  navigates away. Every in-flight request belonging to that
  navigation is aborted automatically.
- If you also pass `init.signal`, the wrapper composes the two
  signals — an abort on either fires the request.
- Aborts surface as the standard `AbortError`. The router
  swallows `AbortError`s thrown by its own controller; an abort
  from a caller-supplied signal still propagates to `app.error()`
  so your own `try/catch` sees it.

To make `zero/http` clients participate, thread the loader's
`fetch` into the `init`:

```ts
export async function load({ fetch }: any) {
  const res = await api.get("/users", { fetch });
  return { users: res };
}
```

See [HTTP](./http.html) for the full middleware story and
[Best Practices §6](./best-practices.html#6-http) for the
one-client-per-backend pattern.

## Navigation lifecycle

```
Click <a href="/users/42">
  ├─ intercept click
  ├─ match route table
  ├─ run global middleware (app.use())
  ├─ run route guard (may redirect or short-circuit)
  ├─ show app.loading() component
  ├─ call load({ params, query, state, fetch })
  │   ├─ on success → resolved data
  │   ├─ on throw   → app.error() (unless AbortError on own controller)
  ├─ set document.title from meta.title
  ├─ run leave transition (if any)
  ├─ commit route component with { data, params, query }
  ├─ run enter transition
  └─ pushState to browser history
```

---

→ Next: [HTTP](./http.html) — `createHttp()`, middleware, error
handling, and how to thread the route's fetch into the client.

## See also

→ [Best Practices §5 Routes](./best-practices.html#5-routes)
