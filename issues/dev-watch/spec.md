# Spec: `zero dev` file watching with browser auto-reload

## Problem Statement

`zero dev` today serves files from disk and proxies/falls-back to a
local `index.html`, but the browser has no idea when those files
change. Every edit-and-see-it cycle costs the developer a manual
refresh and (worse) makes them wonder whether they actually saved,
whether the dev server actually picked the change up, or whether
Chrome is caching despite our `Cache-Control: no-store` headers.

This slice closes that gap: when a watched file under the app
subdirectory changes, the browser reloads itself. No HMR, no module
graph awareness, no state preservation — just `location.reload()`,
which is the smallest credible step beyond the current manual-refresh
state and the one most non-React frameworks ship first.

The phase-6 roadmap entry says "file watching and HMR." HMR is split
out and deferred. This spec is **only** file watching plus a single
full-page reload signal.

## Background

### What exists today

- `src/dev/server.rs` — axum router with handlers for `/zero.js`,
  `/src/**`, `/styles/**`, `/public/**`, well-known root files, and a
  fallback that either proxies (when `[dev].proxy` is set) or serves
  the local `<root>/index.html`.
- `src/dev/inject.rs` — injects two `<script>` tags before `</head>`
  in every HTML response (`DEV_SCRIPTS` const): an import map for
  `"zero"` and the module entry `/src/app.js`.
- `src/dev/files.rs`, `src/dev/local.rs`, `src/dev/proxy.rs` — disk,
  local-index, and reverse-proxy handlers respectively. All HTML
  responses route through `inject()` regardless of source.
- `src/config.rs` — `Config` carries `project.root`, `dev.port`,
  `dev.proxy`, `build.out`; loaded once at `zero dev` startup.
- No file-watching crate or async fs watcher is in `Cargo.toml` yet.
- No tests assume manual refresh; current `tests/e2e_init_dev.rs`,
  `tests/dev_serves_files.rs`, etc. test only one-shot request/response.

### The reload model

When any watched file changes, `zero dev` pushes a `reload` event
over Server-Sent Events to every connected browser tab. The browser
runs `location.reload()`. State is lost — that is acknowledged and
intended. Component-level state preservation is the job of a future
HMR spec.

```
Edit src/routes/home.js on disk
  → notify watcher fires (inotify / FSEvents / ReadDirectoryChangesW)
  → debounce ~100ms (coalesce editor-save bursts)
  → broadcast `reload` event on /_zero/events
  → every connected EventSource receives it
  → browser does location.reload()
  → browser re-requests `/` and `/src/app.js` etc.
  → HTML injection adds the watcher client back
  → EventSource reconnects, ready for the next change
```

### Why SSE, not WebSocket

For this slice the channel is purely server→browser ("reload now").
SSE is one long-lived `GET` handler in axum, no upgrade handshake,
no extra dep; the browser's built-in `EventSource` handles reconnect
with `Last-Event-ID` for free. WS gives a two-way channel the
current feature set wouldn't use; if a future HMR slice wants
two-way messaging it can add WS alongside (the SSE endpoint can stay,
or migrate, depending on what HMR actually needs).

### Why a full reload, not CSS swap

Mixed-strategy reloads (swap `<link>` href on CSS, full reload on
JS) are tempting but introduce a code path that has to know about
file extensions, the DOM, and which `<link>` tag belongs to which
file. Single-mode reload is one rule, no edge cases. CSS-swap can
land as a follow-up if developers actually feel the page-flicker
pain.

## Requirements

### Watcher

- Recursively watch the app subdirectory `<root>` (the value of
  `[project] root` from `zero.toml`), resolved at startup.
- File events that trigger a reload: create, modify, remove, rename
  (both source-side and destination-side of a rename).
- Watch starts when `zero dev` boots; stops when the process exits.
- Watching is non-blocking — fs events run on their own thread / task,
  not the request-serving path.
- Use the `notify` crate (or `notify-debouncer-mini` / `notify-debouncer-full`)
  for cross-platform fs events. The plan picks which `notify` flavor
  fits the debounce strategy.
- Debounce: coalesce events occurring within ~100ms into a single
  reload broadcast. Many editors save atomically (write temp + rename,
  multiple write syscalls, etc.) which generates 2–5 events per save;
  without debouncing the browser would reload mid-write.
- Ignore: hidden files / directories (anything whose path component
  starts with `.`), and the build output directory `<out>` if it
  happens to be nested under `<root>` (it isn't, by default, but a
  developer might set `build.out = "web/dist"`).

### SSE endpoint

- New route: `GET /_zero/events`. The `/_zero/` prefix is reserved
  for framework-owned dev-time endpoints — the leading underscore
  makes collisions with a backend route (in proxy mode) effectively
  impossible and signals to anyone reading network logs that the
  request is framework plumbing, not application traffic.
- Response: `Content-Type: text/event-stream`, `Cache-Control: no-store`
  (still subject to the existing `no_cache_layer`).
- Holds the connection open. Sends:
  - An initial `event: hello\ndata: ok\n\n` on connect (handy for
    diagnostics; also flushes response headers so the browser's
    `EventSource` fires its `onopen`).
  - `event: reload\ndata: <path>\n\n` on each debounced file-change
    burst. `<path>` is the changed path relative to `<root>` (or a
    representative one if multiple files changed in the burst). The
    client doesn't act on the path in this slice — it just calls
    `location.reload()` — but logging the changed file to the
    console is a free debugging aid.
- Heartbeat: send an SSE comment (`: ping\n\n`) every 15s so the
  connection survives idle proxies and the browser doesn't decide
  it's dead.
- Multiple concurrent clients are supported (e.g., the developer has
  the app open in two tabs / two browsers). Broadcast fan-out via a
  `tokio::sync::broadcast` channel.
- The endpoint stays subject to the existing no-cache headers layer.
  Path traversal protection N/A (no disk path involved).

### Client injection

- Extend `DEV_SCRIPTS` in `src/dev/inject.rs` to include a third
  inline script that opens an `EventSource("/_zero/events")` and, on
  `reload` events, calls `location.reload()`.
- The client script is small (≤ ~20 lines) and is the only piece of
  JavaScript framework-injected logic the browser runs. Source it
  inline in `DEV_SCRIPTS` (not as a separate `<script src=...>`),
  to keep the injection a single string concat and avoid adding a
  new served endpoint.
- The client should:
  - Open `new EventSource("/_zero/events")`.
  - On `reload` event → `location.reload()`.
  - On `error` (connection lost) → let `EventSource`'s built-in
    reconnect handle it; no manual retry loop, no UI.
  - Optionally log `[zero] reloading: <path>` on each event for
    visibility in DevTools.
- Injection runs in **both** modes — backend-proxied HTML and the
  local `<root>/index.html` fallback. Existing
  `src/dev/inject.rs::inject` is the single hook; this slice only
  changes what `DEV_SCRIPTS` contains.

### Proxy mode

- Backend HTML template changes are **not** watched. The proxy
  forwards opaque HTML; we have no way to know which files on the
  backend produced it. If the developer edits a backend template,
  they refresh manually (this is the same as today). Document the
  limitation; do not paper over it.
- Backend API endpoints are similarly invisible — but those don't
  need reloading (the browser re-fetches them on the next user
  action). Not a concern.
- Watching is unchanged in proxy mode: the same `<root>` paths are
  watched, the same reload signal is sent. The browser reloading
  causes the backend to be re-hit (which is what we want — the
  developer may have edited their *frontend* and the backend's HTML
  references it).

### Server lifecycle

- File-watching task starts after the listener binds successfully
  (so a bind-failure exit doesn't leave a watcher behind).
- Graceful shutdown: when the existing Ctrl-C shutdown signal fires,
  the watcher task is cancelled. The plan picks whether this is a
  `CancellationToken`, a `tokio::select!` against the shutdown future,
  or dropping the channel sender to signal the watcher to exit.
- One log line at server start: `zero dev — watching <root> for changes`,
  printed alongside the existing `listening on http://...` line.
- No per-event log line is required (the SSE broadcast itself is the
  output the developer cares about); plan may add one for debugging
  but it shouldn't be on by default.

### Tests

- Unit: debouncer collapses N events within the window into one
  broadcast (test the debounce primitive in isolation if the chosen
  crate exposes one; otherwise test it via the watcher's public
  surface).
- Unit: the broadcast channel fan-outs to multiple subscribers (i.e.,
  two `EventSource` connections both see the same `reload` event).
- Integration: spin up `zero dev` against a temp project (à la
  `tests/e2e_init_dev.rs`), open an SSE connection via `reqwest`,
  write to a file under `<root>/src/`, assert a `reload` event arrives
  within ~1s.
- Integration: same as above but verify HTML responses now contain
  the SSE client snippet (extension of an existing dev-injection
  test).
- The existing dev-server tests must keep passing — adding the SSE
  route, watcher, and injection line must not break any current
  request-path tests.

## Constraints

- **One new dep maximum: `notify` (or a `notify-debouncer-*` variant).**
  Everything else (axum SSE response, broadcast channels) is already
  reachable from `axum` / `tokio`.
- **No WebSocket, no upgrade headers, no two-way messaging in this
  slice.** SSE is one-way by design.
- **No HMR.** Full page reload only. No `import.meta.hot`-style API.
  No module-graph awareness. No partial DOM update.
- **No CSS hot-swap.** Even though it's a tempting middle ground, it
  introduces a code path keyed on file extension; keep this slice
  one-rule-one-behavior.
- **No watcher in production builds.** `zero build` (already shipped)
  is unaffected. The watcher and the SSE route exist only in `zero dev`.
- **No watching of files outside `<root>`.** `zero.toml` changes
  require a manual restart (and probably should — port changes,
  proxy changes can't be applied to a running server cleanly).
- **The injected reload-client snippet must work in any HTML the
  backend produces** (no assumptions about which JS framework, build
  step, or CSP the backend uses). If the backend sends a strict CSP
  blocking inline scripts, the injection visibly fails in the
  browser console; that's a known limitation, not a bug to fix here
  (a CSP nonce-handshake is a much larger spec).
- **Watcher startup must not fail the server.** If `notify` returns
  an error setting up watches (rare; permission issues, too many
  watches), log a clear warning and continue serving — the developer
  loses auto-reload but the server still works. They can hit refresh
  manually.

## Out of Scope

- HMR / module-level state preservation (separate later spec).
- CSS hot-swap (`<link>` href rewriting without page reload).
- Error overlay rendered into the page on syntax errors / runtime
  errors. The browser console is the dev's debugging surface for
  now.
- Watching `zero.toml` for changes (restart-required).
- Watching the backend in proxy mode (we don't know what the
  backend's source files are).
- Per-file granularity in the client (the snippet just calls
  `location.reload()`; the changed-path is logged but unused).
- Configurable debounce window (`[dev].watch_debounce_ms` etc.).
  100ms is fine for now; revisit when someone reports needing to
  tune it.
- Ignore-pattern configuration (`.zeroignore`, `[dev].watch_ignore`).
  Hard-coded ignores for hidden dirs and `<out>` suffice.
- WebSocket transport.
- Polling fallback for filesystems without native fs events (network
  mounts, some Docker setups). `notify` falls back to polling on
  its own where needed; if a specific environment breaks, address it
  then.
- Restart-on-server-binary-change. (Cargo / `cargo watch` is the
  developer's tool for that; not zero's job.)

## Open Questions

- **Which `notify` variant.** `notify` (raw events) plus a hand-rolled
  debounce, vs. `notify-debouncer-mini` (simple debounce, no metadata),
  vs. `notify-debouncer-full` (debounce + de-dup + rename tracking,
  heavier). Recommendation: start with `notify-debouncer-mini` — it
  hits the 100ms-debounce requirement with the least surface area; the
  plan can upgrade to `-full` if rename handling proves messy.
- **SSE endpoint path.** Settled on `/_zero/events`. The leading
  underscore on the namespace prefix avoids any plausible collision
  with a backend route in proxy mode (real apps almost never serve
  paths beginning with `_`) and matches the convention several other
  frameworks use (`/_next/`, `/_astro/`, etc.). Future HMR / devtools
  endpoints should live under the same `/_zero/` prefix. Open
  follow-up: should `/zero.js` also move under this prefix (e.g.,
  `/_zero/runtime.js`) for consistency? Out of scope for this slice
  but worth raising when the next dev-server endpoint lands.
- **What `data:` field carries.** Suggestion above: changed path
  relative to `<root>`. Alternatives: empty string, a JSON blob with
  `{path, kind}`, the full event count. Recommendation: ship the
  relative path as plain text; JSON is overkill until there's a
  second consumer.
- **Should the client snippet do anything beyond `location.reload()`?**
  E.g., a small "reloading…" toast, or a debounce on rapid-fire
  reloads so the page doesn't fight itself if the server burst-fires.
  Recommendation: nothing. `EventSource` is reliable, the server
  debounces upstream, and a toast is UI surface that doesn't belong
  in framework-injected code.
- **Heartbeat interval.** 15s is a safe default (well under typical
  proxy idle timeouts). Plan may tune.
- **What about the dev-server output noise.** Today we print one
  line per request. Should the reload broadcast print a line too
  ("reload: src/routes/home.js")? Recommendation: yes, one line per
  debounced burst — it confirms "the server saw your edit," which is
  the question developers actually have when a refresh doesn't
  reflect their change.
- **Does the SSE response need to bypass the `no_cache_layer`?**
  The layer adds `Cache-Control: no-store` etc. to every response,
  which is correct for an SSE stream. The layer should compose
  fine; flag if axum's response body stream interacts badly with
  `SetResponseHeaderLayer` (unlikely, but worth a sanity check during
  implementation).
- **Race on connect-just-before-edit.** If the developer saves the
  file in the ~milliseconds between the browser issuing the SSE GET
  and the handler subscribing to the broadcast channel, the event
  is lost and the browser doesn't reload. Recommendation: ignore —
  the next save catches it, and the window is tiny. A
  `Last-Event-ID`-based replay would be a much larger feature.
- **Framework spec amendment.** §1 currently says "HMR is always on.
  Errors render as a browser overlay AND in the terminal." This slice
  ships full-reload (not HMR) and no overlay. Phase 6 in §12 has
  this as a single checkbox `[ ] zero dev file watching and HMR`;
  splitting that into two boxes (`file watching` ✓ after this slice,
  `HMR` still pending) is a small spec-text edit the implementation
  PR should include.
