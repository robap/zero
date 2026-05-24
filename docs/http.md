---
title: HTTP
nav_order: 8
---

# HTTP

`zero/http` is the framework's HTTP client. It's a thin wrapper
around `fetch` that adds middleware, typed error handling, JSON
ergonomics, and seamless threading of route-scoped `AbortSignal`s.

## Why a wrapper

Every realistic frontend talks to a backend. A bare `fetch` call
forces every component to re-implement the same boilerplate:
header injection, JSON encoding/decoding, 401-redirect, error
classification, abort wiring. `zero/http` centralises that into
a client object you build once per backend and inject everywhere.

## Constructing a client

```ts
import { createHttp } from "zero/http";

export const api = createHttp();
```

The returned `HttpClient` has the standard HTTP verbs plus a
generic `request`:

```ts
api.get<T>(url, init?);              // GET — returns Promise<T>
api.post<T>(url, body?, init?);      // POST with optional body
api.put<T>(url, body?, init?);
api.patch<T>(url, body?, init?);
api.delete<T>(url, init?);
api.request<T>(input, init?);        // Request | URL | string
```

Optional constructor parameter: `createHttp({ fetch })` lets you
inject a custom `fetch` implementation — useful for tests, or
for swapping in the route-scoped `fetch` from a `load()` call.

## JSON I/O

Bodies that are plain objects (anything other than a `string`,
`FormData`, `Blob`, `ArrayBuffer`, or `URLSearchParams`) are
serialised with `JSON.stringify` and the `Content-Type:
application/json` header is set automatically:

```ts
await api.post("/users", { name: "Ada" });
// → POST /users
//   Content-Type: application/json
//   {"name":"Ada"}
```

Responses with a JSON `Content-Type` are parsed before the
promise resolves; other responses return the raw `Response`
object so you can stream binary, text, etc.

```ts
const user = await api.get<User>("/users/42");
// user is typed User, parsed from JSON

const res  = await api.get<Response>("/files/icon.png");
// res is the raw Response — `await res.blob()` etc.
```

## Errors

Non-2xx responses throw `HttpError`:

```ts
import { HttpError } from "zero/http";

try {
  await api.post("/users", { name: "" });
} catch (e) {
  if (e instanceof HttpError) {
    e.status;     // 400
    e.statusText; // "Bad Request"
    e.body;       // parsed JSON if available, else text
  }
}
```

Other failures surface as their natural errors:

| Situation                             | Error type                           |
|---------------------------------------|--------------------------------------|
| Non-2xx response                      | `HttpError` (`status`, `body`)       |
| Network failure (DNS, offline, CORS)  | `TypeError`                          |
| Aborted via signal                    | `DOMException` (`name: "AbortError"`) |

## Middleware

`client.use(mw)` registers a middleware. The signature mirrors
familiar onion-model middleware:

```ts
type Middleware = (
  req: Request,
  next: (req: Request) => Promise<Response>,
) => Promise<Response>;
```

Middlewares run **outermost-first on the way down, innermost-
first on the way up** — exactly like Koa or any onion-model
stack. Three canonical examples:

**Auth header injection.**

```ts
api.use(async (req, next) => {
  const token = localStorage.getItem("token");
  if (token) {
    const withAuth = new Request(req, {
      headers: { ...Object.fromEntries(req.headers), Authorization: `Bearer ${token}` },
    });
    return next(withAuth);
  }
  return next(req);
});
```

**401-redirect.**

```ts
api.use(async (req, next) => {
  const res = await next(req);
  if (res.status === 401) navigate("/login");
  return res;
});
```

**Short-circuit (mock or cache).**

```ts
api.use(async (req, next) => {
  if (req.method === "GET" && cache.has(req.url)) {
    return new Response(cache.get(req.url), { headers: { "Content-Type": "application/json" } });
  }
  const res = await next(req);
  if (req.method === "GET" && res.ok) cache.set(req.url, await res.clone().text());
  return res;
});
```

A middleware can call `next` zero, one, or many times. Returning
a `Response` without calling `next` short-circuits the chain;
calling `next` more than once re-runs the inner chain (useful for
retry middleware).

## Route-scoped fetch threading

Inside a route's `load()`, the framework passes a `fetch` whose
`AbortSignal` is wired to the navigation lifecycle (see
[Routing § Route-scoped fetch](./routing.html#route-scoped-fetch)).
To make your `zero/http` client participate, pass it as
`init.fetch`:

```ts
export async function load({ fetch, params }: any) {
  const user = await api.get<User>(`/users/${params.id}`, { fetch });
  return { user };
}
```

Now if the user clicks another link before the request finishes,
the navigation controller aborts the in-flight request — no
stale data race, no zombie promise.

The `init.fetch` override is per-call; the client's
constructor-time `fetch` (if any) is the default.

## Spying on fetch in tests

Every client method (`get`, `post`, `put`, `patch`, `delete`,
`request`) normalises its arguments into a single `Request` and
passes **that one `Request`** to the injected `fetch`. The injected
`fetch` never receives a `(url, init)` pair — only `(req)`.

This is the same shape middleware sees (middleware operates on
`Request` — see [Middleware](#middleware) above), so the boundary
stays uniform from middleware all the way down to the network call.

When you spy on `fetch` in a test, read `(arg as Request).url` /
`.method` / `.headers` — not `arg` itself:

```ts
import { createHttp } from "zero/http";
import { spy } from "zero/test";

it("requests the user record", async () => {
  const fetchSpy = spy<typeof fetch>(
    async () => new Response(JSON.stringify({ id: 42 }), {
      headers: { "Content-Type": "application/json" },
    }),
  );
  const api = createHttp({ fetch: fetchSpy });

  await api.get("/users/42");

  const req = fetchSpy.calls[0][0] as Request;
  expect(req.url).toMatch(/\/users\/42$/);
  expect(req.method).toBe("GET");
});
```

`req.url` is the fully resolved URL (relative URLs are resolved
against the document base). Use a regex or `endsWith` rather than
strict equality if you care about the path but not the origin.

## One client per backend

Reach for one `HttpClient` instance per backend you talk to.
Stash it in a module so all callers share the same middleware
stack:

```ts
// src/api/index.ts
import { createHttp } from "zero/http";

export const api = createHttp();
api.use(authHeader);
api.use(redirectOnUnauthorized);
```

That keeps middleware DRY, makes error handling consistent, and
gives you exactly one place to mock when testing.

The full discussion of organising HTTP code in a larger app is
in [Best Practices §6](./best-practices.html#6-http).

---

→ Next: [Testing](./testing.html) — the `zero test` runner,
DOM helpers, and the in-memory web platform.

## See also

→ [Best Practices §6 HTTP](./best-practices.html#6-http)
