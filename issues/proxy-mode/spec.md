# Spec: Proxy-mode contract and diagnostic explainer

## Problem Statement

When `[dev].proxy` is configured, `zero dev` forwards every unmatched
request to the upstream backend — including `/` and `/index.html`. The
original design assumes the backend serves the HTML shell at `/` and
`zero dev` injects the dev scripts (import map, `/src/app.ts`, SSE
reload client) into that response.

This contract is invisible. A developer pointing the proxy at an
API-only backend (e.g. a minimal ASP.NET app exposing only `/api/*`)
sees the dev server "respond with 404" at `http://localhost:3000/` and
naturally reads it as a `zero dev` bug. The dev server is doing exactly
what it was designed to do, but the failure mode looks like
misconfiguration of `zero`, not of the backend.

This slice makes the contract obvious without changing the underlying
routing model. When a developer hits the most common dev-setup
mistake — proxy is configured, backend has no `/` route, or backend
isn't running yet — they get an inline explainer instead of an opaque
upstream 404 or a generic 502.

## Background

### What exists today

- `crates/zero-dev/src/server.rs::build_app` (lines 185-286) defines
  the dev-server router. All known static routes (`/src/*`,
  `/styles/*`, `/public/*`, `/.zero/components/*`, `/.zero/fonts/*`,
  `/zero.js`, `/zero-http.js`, `/_zero/events`, `/favicon.ico`,
  `/robots.txt`) are handled locally and never reach the proxy.
- The `.fallback(...)` block (lines 265-283) branches on
  `state.proxy`: if a proxy is configured, every fallback request is
  forwarded via `proxy_request`; otherwise `serve_local_index` returns
  the local `index.html` with dev scripts injected.
- `crates/zero-dev/src/proxy.rs::proxy_request`:
  - Forwards the request verbatim minus hop-by-hop headers.
  - Filters cache/validator headers off the upstream response.
  - **Injects dev scripts into any `text/html` response via
    `inject(&body_bytes, app_entry_href)`** (lines 119-123). This is
    the load-bearing detail behind the proxy-mode contract: the
    backend's HTML doesn't need any awareness of `zero`; the dev server
    fills in the boot script and SSE client at runtime.
  - On reqwest error (connection refused, timeout, DNS failure),
    returns a hardcoded 502 page (`bad_gateway_response`, lines
    177-190) reading `<h1>zero dev</h1><p>Cannot reach backend at
    <url>: <err></p>`.
- `crates/zero-dev/src/local.rs::serve_local_index` does the
  symmetrical work for no-proxy mode: read `<root>/index.html`, run
  it through `inject()`, serve as 200.
- `crates/zero-scaffold/` (via `zero init`) emits the project skeleton,
  including `zero.toml`. The current scaffold's `zero.toml` doesn't
  contain a `[dev].proxy` line by default; the user writes it in
  themselves when they want a backend.

### The proxy-mode contract (made explicit)

When `[dev].proxy = "http://..."` is set in `zero.toml`:

1. Asset requests for `/src/*`, `/styles/*`, `/public/*`, `/.zero/*`,
   and the well-known root files are served by `zero dev` directly.
   These never touch the backend.
2. **Every other request — including `/` and `/index.html` — is
   forwarded to the backend.** The backend is responsible for
   responding with HTML at `/` (and for client-side route reloads, at
   whatever paths the SPA owns).
3. When the backend's response is `text/html`, `zero dev` injects the
   import map, `/src/app.ts` boot tag, and SSE reload client into the
   `<head>`. The backend's HTML does not need to know about `zero`.
4. In a deployed environment (out of scope here), the backend handles
   the same `/` request directly and serves the production HTML
   referencing the cache-busted `dist/` bundle. The dev contract is
   chosen specifically to match that deployment shape.

### What the bug report actually surfaced

The reporter's setup is the contract-violating case: the .NET backend
serves only `/api/health` and has no `/` handler, so Kestrel returns
its built-in 404. `zero dev`'s proxy faithfully forwards that 404,
which is then surfaced to the developer as if `zero dev` were broken.
The fix is not to override the proxy's behavior in general — it is to
detect this specific shape (request for `/` or `/index.html`, upstream
failure) and replace the response body with a diagnostic that names
the real problem.

### Why this isn't a routing change

The alternative — making `zero dev` serve a local `index.html` at `/`
even in proxy mode — would break the symmetry with the deployed
environment, where the backend genuinely owns `/`. It would also
require the dev to maintain a separate local `index.html` that isn't
the source of truth for production. Keeping the contract intact and
making it discoverable is the smaller, more honest change.

## Requirements

### Behavior

1. The diagnostic explainer page is returned **only** when **all** of
   the following hold:
   - Proxy mode is active (`state.proxy` is `Some`).
   - The incoming request's path is exactly `/` or `/index.html`
     (case-sensitive; query string ignored for the match).
   - The request method is `GET`.
   - **And one of**:
     a. The upstream returned a non-2xx HTTP status, **or**
     b. The upstream is unreachable (connection refused, DNS failure,
        timeout — i.e. any `reqwest::Error` from `client.send()`).
2. In all other cases, proxy behavior is unchanged. Pass-through 404s
   on `/api/*` and other paths must continue to surface verbatim so
   the developer sees their backend's real responses.
3. The explainer response:
   - Status: preserve the upstream status when there is one (e.g. 404
     stays 404). For the unreachable case, use 502.
   - Content-Type: `text/html; charset=utf-8`.
   - Body: HTML containing:
     - **Diagnostic line**: a single sentence stating what happened,
       naming the upstream URL and the observed failure. Examples:
       - `Your backend at http://localhost:5080 returned 404 for /`
       - `Could not reach your backend at http://localhost:5080: connection refused`
     - **Contract explanation**: a short paragraph describing the
       proxy-mode contract: "In proxy mode, `zero dev` forwards
       requests to your backend. Your backend is expected to serve the
       HTML at `/`. `zero dev` will inject the dev scripts (import
       map, app entry, reload client) into that response
       automatically."
   - The page must be self-contained — no external assets, no copy-
     pasteable shell, no doc links. Just diagnosis and contract.
4. The explainer page is HTML, but it is **not** a real app shell. It
   is not run through `inject()`; the SPA does not boot inside it. The
   developer is expected to fix the backend (start it, add a `/`
   handler) and reload.

### Configuration

5. No new configuration is introduced. The trigger is implicit and the
   message is fixed.

### Scaffold

6. When `zero init` (and `zero update`, to the extent it touches user
   files — note that today it does not rewrite `zero.toml`) emits the
   scaffolded `zero.toml`, the `[dev]` section gains a commented-out
   example with a one- or two-line explanation directly above it:

   ```toml
   [dev]
   port = 3000
   # Optional: forward unmatched requests to a backend dev server.
   # The backend must serve HTML at `/`; zero dev injects dev scripts
   # into that response and continues to serve /src, /styles, /public.
   # proxy = "http://localhost:5080"
   ```

   The exact wording is at the planner's discretion as long as both
   "backend must serve HTML at `/`" and "zero dev injects scripts" are
   communicated.

### Tests

7. Unit tests in `crates/zero-dev/src/proxy.rs` (extending the
   existing `tests` module) covering:
   - `/` with upstream 404 → explainer body, status 404, content-type
     `text/html`.
   - `/index.html` with upstream 404 → same.
   - `/api/health` with upstream 404 → upstream body passes through,
     no explainer.
   - `/` with upstream 200 + HTML → existing inject path; no
     explainer.
   - `/` with upstream unreachable → explainer body, status 502.
   - `/api/health` with upstream unreachable → existing 502 "Cannot
     reach backend" page; no explainer.
   - Explainer body contains both the diagnostic line (with the
     configured upstream URL) and the contract paragraph.
8. Scaffold test: the `zero.toml` emitted by `zero init` contains the
   commented proxy example and the contract explanation.

## Constraints

- No change to the dev server's routing logic outside the fallback
  handler's failure paths. The set of locally-handled prefixes stays
  identical.
- No change to the proxy's success path. A backend that already serves
  HTML at `/` must see exactly the same injected response it sees
  today (modulo header ordering).
- No new config keys in `zero.toml`. The contract is implicit; the
  explainer is on by default and not configurable.
- The explainer page must be plain HTML with no `<script>` tags and
  no external resource references — it is shown when the dev
  environment is in a broken state, so it must not depend on anything
  in that environment loading.
- Existing tests in `crates/zero-dev/` must continue to pass without
  modification (except where they assert behavior that this spec
  intentionally changes — currently none, since no existing test
  covers an upstream 404 at `/`).

## Out of Scope

- Any form of "SPA fallback" routing where `zero dev` serves
  `index.html` for unmatched non-asset paths in proxy mode.
- An `[dev] proxy_paths = [...]` (or equivalent) config to scope
  proxying to specific path prefixes. The two supported modes remain
  "no proxy, zero serves index.html" and "proxy everything, backend
  serves index.html".
- Production / deployed-environment behavior. The backend's
  responsibility for serving `/` in production is real but out of
  scope for this spec; the dev contract is designed to mirror it, not
  to replace it.
- Changes to `zero build` or `zero preview`.
- WebSocket upgrade support (still returns 501).
- Documentation in `zero-framework-spec.md` or `BEST_PRACTICES.md`.
  The only documentation touched by this slice is the inline comment
  in scaffolded `zero.toml`.
- Auto-reload after the developer fixes the backend. The current
  expectation is that the developer hits refresh manually once the
  backend is up.

## Open Questions

- Should the upstream's response body be preserved at all in the
  explainer (e.g. as a `<details>` block at the bottom)? Current spec
  says no — diagnosis + contract only — but if the backend's 404
  message ever contains useful info (a path it expected, a stack
  trace) the dev loses it. Punted to the plan phase; default is "do
  not show the upstream body".
- The trigger checks the literal path `/index.html`. Should it also
  match `/Index.html` and other case variants? Probably not worth it;
  browsers don't generate uppercase paths and the developer typing
  `/Index.html` by hand will get the verbatim upstream 404, which is
  fair. Confirm during planning.
- For the unreachable-backend case, today's 502 page is hardcoded in
  `bad_gateway_response`. The cleanest implementation replaces that
  function's body with the explainer when the request path matches `/`
  or `/index.html`, and leaves it alone otherwise. Verify this shape
  during planning.
