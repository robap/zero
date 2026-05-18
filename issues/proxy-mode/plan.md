# Plan: Proxy-mode contract and diagnostic explainer

## Summary

When `[dev].proxy` is set, `zero dev` forwards `/` to the backend — by design
— but a backend with no `/` handler turns this contract into an opaque 404
that looks like a `zero dev` bug. This slice keeps the routing model intact
and adds a targeted diagnostic explainer that fires only on the failure
shape that triggered the bug report: a `GET /` or `GET /index.html` whose
upstream returned a non-2xx **or** is unreachable. Every other proxy path
(including `/api/*` 404s) continues to pass through verbatim. The
scaffolded `zero.toml` gains an inline comment that makes the contract
("backend serves `/`, zero injects scripts") visible at the point where the
developer writes the `proxy =` line.

## Prerequisites

Open questions from the spec resolved here:

- **Preserve upstream body in the explainer?** No. Spec default holds.
  Cleaner page; upstream body would invite confusion (e.g. a Kestrel HTML
  404 next to a zero-dev HTML explainer reads as conflicting messages).
- **Case-insensitive match for `/index.html`?** No, case-sensitive. Browsers
  normalize to lowercase; a developer hand-typing `/Index.html` will get the
  verbatim upstream 404, which is acceptable.
- **Shape of the change to `bad_gateway_response`?** Do **not** modify
  `bad_gateway_response`. Introduce a separate `explainer_response()` and
  branch at the call sites inside `proxy_request`. `bad_gateway_response`
  stays focused on the "any other path is unreachable" case and continues to
  satisfy the existing integration test in `crates/zero/tests/dev_proxy.rs`.

No prior issues block this plan.

## Steps

- [x] **Step 1: Add the explainer renderer with unit tests**
- [x] **Step 2: Wire the explainer into proxy_request's failure paths**
- [x] **Step 3: Extend the scaffolded zero.toml with the contract comment**

---

## Step Details

### Step 1: Add the explainer renderer with unit tests

**Goal:** Land the pure HTML-rendering function and its triggering predicate
in isolation, with full test coverage, before changing any wiring. This
keeps the diff in Step 2 strictly about call-site routing.

**Files:**
- `crates/zero-dev/src/proxy.rs` (modify — additions only)

**Changes:**

1. Add a private predicate at module scope:

   ```rust
   /// True when the explainer should replace the upstream/error response.
   ///
   /// The trigger is intentionally narrow: only `GET /` and `GET /index.html`
   /// — the paths a browser asks for when loading the dev server. Match is
   /// case-sensitive; the query string is ignored.
   fn is_root_html_request(method: &axum::http::Method, path: &str) -> bool {
       method == axum::http::Method::GET && (path == "/" || path == "/index.html")
   }
   ```

2. Add a private enum describing what the explainer is reporting:

   ```rust
   /// The observed failure the explainer is describing.
   enum ExplainerFailure {
       /// Upstream returned a non-2xx status.
       UpstreamStatus(StatusCode),
       /// Upstream is unreachable (connection refused, DNS failure, timeout).
       Unreachable(String),
   }
   ```

   The `Unreachable` variant carries a short string description (the
   `Display` form of the `reqwest::Error`) so the renderer doesn't borrow the
   error.

3. Add a private renderer:

   ```rust
   fn render_explainer_html(proxy_base: &url::Url, failure: &ExplainerFailure) -> String
   ```

   Output structure (raw, no external assets, no `<script>`):

   ```html
   <!doctype html>
   <html lang="en">
   <head>
     <meta charset="utf-8">
     <title>zero dev — backend not serving /</title>
     <style>
       body { font-family: system-ui, sans-serif; max-width: 40rem; margin: 4rem auto; padding: 0 1rem; line-height: 1.5; color: #222; }
       code { background: #f3f3f3; padding: 0 0.25rem; border-radius: 3px; }
       h1 { font-size: 1.25rem; margin-bottom: 0.5rem; }
       p.diagnostic { font-weight: 600; }
     </style>
   </head>
   <body>
     <h1>zero dev</h1>
     <p class="diagnostic">{diagnostic_sentence}</p>
     <p>
       In proxy mode, <code>zero dev</code> forwards requests to your backend.
       Your backend is expected to serve the HTML at <code>/</code>. <code>zero dev</code>
       will inject the dev scripts (import map, app entry, reload client)
       into that response automatically.
     </p>
   </body>
   </html>
   ```

   Where `{diagnostic_sentence}` is:
   - `Your backend at {proxy_base} returned {code} for /` for
     `UpstreamStatus(code)` (e.g. `... returned 404 for /`).
   - `Could not reach your backend at {proxy_base}: {err}` for
     `Unreachable(err)`.

   The function HTML-escapes `proxy_base` (via `url::Url::as_str` — already
   safe since URLs don't contain `<`/`>`/`&`/`"`/`'`) and the error string
   (use a tiny inline escape for `&<>"'` to keep things bulletproof).
   `proxy_base.as_str()` ends in a trailing `/`; strip exactly one trailing
   `/` for cosmetics so the diagnostic reads `http://localhost:5080` rather
   than `http://localhost:5080/`.

4. Add a private builder that wraps the rendered HTML in a `Response`:

   ```rust
   fn explainer_response(
       status: StatusCode,
       proxy_base: &url::Url,
       failure: ExplainerFailure,
   ) -> Response
   ```

   Sets `Content-Type: text/html; charset=utf-8`, sets `Content-Length` to
   the body length, returns a `Response<Body>` with `status` and the
   rendered HTML.

**Tests:** New unit tests under the existing `tests` module in `proxy.rs`.

- `is_root_html_request_matches_get_root_and_index_html`: GET `/` and
  GET `/index.html` return true; GET `/api/health`, POST `/`, GET `/Index.html`
  return false.
- `render_explainer_html_includes_upstream_url_and_status_for_status_variant`:
  produces body containing `http://localhost:5080` (no trailing slash) and
  the literal `404`.
- `render_explainer_html_includes_url_and_error_for_unreachable_variant`:
  body contains the URL and the error string.
- `render_explainer_html_includes_contract_paragraph`: body contains both
  the substring `forwards requests to your backend` and
  `serve the HTML at` (the two load-bearing phrases of the contract
  paragraph — checked separately so a reword doesn't silently drop one).
- `render_explainer_html_has_no_script_tags`: rendered body contains no
  `<script` substring (constraint from the spec).
- `explainer_response_sets_html_content_type_and_status`: returned
  `Response` has `Content-Type: text/html; charset=utf-8` and the supplied
  status.

### Step 2: Wire the explainer into proxy_request's failure paths

**Goal:** Use the Step-1 renderer at the two failure paths inside
`proxy_request`. After this step, the bug-report scenario (`/ → 404`)
returns the explainer; every other proxy behavior is byte-identical to
today.

**Files:**
- `crates/zero-dev/src/proxy.rs` (modify)

**Changes:**

1. At the top of `proxy_request`, capture the request method and the
   path **before** consuming the body:

   ```rust
   let req_method = req.method().clone();
   let req_path = req.uri().path().to_string();
   ```

   (`path_and_query` is captured a few lines below for the upstream URL;
   that line stays.)

2. In the `client.request(...).send().await` `Err(e)` arm (currently
   `return bad_gateway_response(proxy_base, e);`), branch:

   ```rust
   Err(e) => {
       if is_root_html_request(&req_method, &req_path) {
           return explainer_response(
               StatusCode::BAD_GATEWAY,
               proxy_base,
               ExplainerFailure::Unreachable(e.to_string()),
           );
       }
       return bad_gateway_response(proxy_base, e);
   }
   ```

3. After the `let status = ...` line that converts the upstream status,
   add a branch **before** the existing `content-type` lookup:

   ```rust
   if is_root_html_request(&req_method, &req_path) && !status.is_success() {
       // Drain the body (and drop it) to free the upstream connection.
       let _ = upstream_resp.bytes().await;
       return explainer_response(
           status,
           proxy_base,
           ExplainerFailure::UpstreamStatus(status),
       );
   }
   ```

   This is the only structural change to the success path: the contract is
   that the explainer is shown when the upstream *failed at the root*, so
   we have to consult `status` before deciding to inject. A 2xx HTML at `/`
   continues into the existing `if content_type.starts_with("text/html")`
   branch and gets injected exactly as today. A non-2xx at any non-root
   path continues into the existing content-type branch (HTML gets
   injected; non-HTML streams through) — verifying parity here is the
   point of the `/api/health → 404` pass-through test.

   `!status.is_success()` covers everything outside `200..=299`,
   intentionally including 3xx redirects: with `redirect::Policy::none()`
   the upstream's redirect would otherwise pass straight through, and a
   redirect at `/` is also a misconfiguration we'd want to call out.

4. `bad_gateway_response` is **unchanged**.

**Tests:** New tests in the existing `tests` module in `proxy.rs`. Each
follows the same shape as `html_response_is_injected_with_scripts`: spin
up an axum stub backend on `127.0.0.1:0`, hit it through `proxy_request`,
assert.

- `explainer_replaces_upstream_404_at_root`: stub returns 404 at `/`;
  request `/`; assert status 404, body contains both the diagnostic line
  (`returned 404 for /`) and the contract paragraph, content-type is HTML.
- `explainer_replaces_upstream_404_at_index_html`: same but stub serves
  `/index.html`; assert same shape with status 404.
- `non_root_404_passes_through_verbatim`: stub returns 404 at
  `/api/health` with a plain-text body `"not found"`; request
  `/api/health`; assert status 404 and body **equals** `"not found"`
  (no explainer markup, no `forwards requests to your backend`).
- `successful_html_root_response_is_injected_not_replaced` (regression):
  stub returns 200 HTML at `/`; assert body contains the importmap
  script and does **not** contain the contract paragraph.
- `unreachable_backend_at_root_returns_explainer`: target a port with
  nothing listening (use `127.0.0.1:1` like `unreachable_backend_returns_502`
  does); request `/`; assert status 502, body contains
  `Could not reach your backend` and the contract paragraph.
- `unreachable_backend_at_non_root_keeps_existing_502`: same unreachable
  port but request `/anything`; assert status 502 and body contains
  `Cannot reach backend` (the existing
  `bad_gateway_response` wording, which we haven't touched).

The existing `unreachable_backend_returns_502` test hits `/anything` and
will keep passing unchanged.

The integration test in `crates/zero/tests/dev_proxy.rs::proxy_returns_502_when_backend_unreachable`
hits `/anything` and asserts `body.contains("Cannot reach backend")` — also
unchanged.

### Step 3: Extend the scaffolded zero.toml with the contract comment

**Goal:** Make the proxy-mode contract visible at the moment a developer
adds `proxy = "..."` to `zero.toml`, so the same bug never lands in a
second project.

**Files:**
- `crates/zero-config/src/toml_writer.rs` (modify)

**Changes:**

1. In `render_toml`, replace the single-line commented example with a
   three-line block (comments first, then the commented example), but
   **only** when `input.proxy` is `None`. When `proxy` is `Some(...)` (the
   user already understands the feature), emit the active line only — no
   comment block.

   The relevant block becomes:

   ```rust
   match &input.proxy {
       Some(p) if !p.is_empty() => out.push_str(&format!("proxy = \"{p}\"\n")),
       _ => {
           out.push_str("# Optional: forward unmatched requests to a backend dev server.\n");
           out.push_str("# Your backend must serve HTML at `/`; zero dev injects dev scripts\n");
           out.push_str("# into that response and continues to serve /src, /styles, /public.\n");
           out.push_str("# proxy = \"http://localhost:8080\"\n");
       }
   }
   ```

   The example URL stays `http://localhost:8080` (matching the existing
   default) rather than `http://localhost:5080` from the spec — the spec
   says exact wording is at the planner's discretion, and keeping `8080`
   minimizes churn in adjacent tests/docs that might reference it.

2. The existing test `rendered_toml_omits_proxy_when_none` still passes:
   it asserts `cfg.dev.proxy.is_none()`, and commented lines don't affect
   parsing.

**Tests:** Extend the existing `tests` module in
`crates/zero-config/src/toml_writer.rs`.

- `rendered_toml_contains_contract_comment_when_proxy_none`: render with
  `proxy: None`; assert the rendered text contains
  `# Optional: forward unmatched requests`,
  `must serve HTML at`, and `injects dev scripts`.
- `rendered_toml_omits_contract_comment_when_proxy_set`: render with
  `proxy: Some("http://localhost:8080".into())`; assert the rendered text
  does **not** contain `# Optional: forward unmatched requests`. (Rationale:
  when the user already runs proxied, the comment is noise.)
- `rendered_toml_still_round_trips_through_config_parser` (extend the
  existing round-trip): re-parse the proxy-None rendering and confirm
  `cfg.dev.proxy.is_none()` and `cfg.dev.port == 3000` (with sample input).

No changes are needed in `crates/zero-scaffold/` (it does not render
`zero.toml`) or in `crates/zero/src/cmd/init.rs` (it calls `render_toml`
and writes the result unchanged). No changes in `crates/zero/src/cmd/update.rs`
either — it explicitly does not touch `zero.toml`.

---

## Risks and Assumptions

- **Trailing-slash cosmetics.** `url::Url::as_str()` on
  `http://localhost:5080` returns `http://localhost:5080/`. The renderer
  strips exactly one trailing `/`. If a developer ever configures
  `proxy = "http://localhost:5080/api"` (already a sub-path), the
  diagnostic would say `... returned 404 for /` against `http://localhost:5080/api`,
  which is still readable. Acceptable; the proxy itself does not currently
  support sub-path bases beyond what `set_path` does, and this slice does
  not change that.
- **HTML escaping of the error string.** `reqwest::Error::to_string()`
  output is generally well-formed but not guaranteed to be HTML-safe.
  Step 1 includes a tiny inline escape pass for `& < > " '` in the renderer
  to keep the page bulletproof.
- **3xx at `/` triggers the explainer.** Treating any non-2xx as a failure
  pulls 3xx into the explainer net. A backend that legitimately redirects
  `/` to another path would currently see the redirect surfaced to the
  browser (since `redirect::Policy::none()` is the configured policy);
  after this change, the redirect at `/` is replaced with the explainer.
  This is acceptable — a redirect at the dev-mode root is a setup mistake
  in this contract — and it can be re-narrowed to 4xx/5xx in a follow-up
  if a real use case turns up.
- **Test stub timing.** The new `proxy_request` tests use the same
  `start_backend` helper that the existing tests use; if those tests are
  ever flaky on this machine they'll get flakier in proportion. No new
  timing dependencies are introduced.
- **No new config keys.** This plan honors the spec's "no new config"
  rule. If the explainer ever needs an off-switch (e.g. for screenshotting
  the raw upstream 404), that's a separate slice.
