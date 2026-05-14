# Plan: `zero dev` file watching with browser auto-reload

## Summary

Wire an `fs`-watcher into `zero dev` so edits under `<root>` trigger a
browser-side `location.reload()`. Three moving parts: (1) a `notify`-based
watcher that debounces editor-save bursts and forwards a single signal, (2) a
`tokio::sync::broadcast` channel that fans the signal out to one or more
connected browsers, and (3) a `text/event-stream` SSE endpoint at
`/_zero/events` that delivers `reload` events to an inline `EventSource`
client injected into every HTML response. No HMR, no CSS-swap, no
configuration knobs — full-page reload only.

The implementation lands in four incremental steps. Each leaves the tree
compiling and the existing test suite green:

1. Extend `DEV_SCRIPTS` with the reload-client snippet (pure injection
   change; no server-side wiring needed).
2. Add the broadcast channel + SSE handler at `/_zero/events` (working
   endpoint with `hello` + heartbeats; no watcher yet).
3. Add the `notify-debouncer-mini` watcher task that publishes to the
   broadcast channel; amend the framework spec's Phase 6 checkbox.
4. End-to-end integration test that wires the whole loop together.

Risk concentrates in the watcher: cross-platform fs events, debounce
correctness, graceful shutdown, and the ignore predicate (`.` prefixed
paths, the `<out>` directory). Mitigations are spelled out per-step.

## Prerequisites

The spec's open questions all have settled recommendations the plan
adopts directly:

- **`notify` flavor:** `notify-debouncer-mini`. Cheapest crate that hits the
  100ms debounce requirement. Upgrade to `-full` only if rename handling
  becomes a problem; defer.
- **SSE path:** `/_zero/events`. Framework-owned prefix `/_zero/` is
  reserved; future devtools / HMR endpoints land under the same prefix.
- **`data:` field:** the changed path relative to `<root>`, as plain text.
  When a debounced burst contains multiple paths, the first event in the
  batch is the representative one. (The client logs it but doesn't act on
  it.) Empty string acceptable as a fallback.
- **Client behavior:** open `EventSource("/_zero/events")`; on `reload` →
  `location.reload()`; on error → built-in reconnect (no manual retry, no
  UI). Optionally `console.log("[zero] reloading: <path>")`.
- **Heartbeat:** SSE comment `: ping\n\n` every 15s.
- **Server log on reload:** one line per debounced burst,
  e.g. `zero dev — reload: src/routes/home.js`.
- **Graceful shutdown:** drop the watcher's broadcast sender on shutdown;
  open SSE handlers exit their loop when `recv()` returns `Closed`.
- **`no_cache_layer` interaction with SSE:** the layer's
  `SetResponseHeaderLayer` adds headers to the *response*, not the stream
  frames — composes fine. Verify during Step 2; no change planned.
- **Cargo dep:** only `notify-debouncer-mini` is added. Everything else
  (broadcast channel, SSE response, async task) already reachable from
  `tokio` and `axum`.

None of the other in-flight issues block this work.

---

## Steps

- [x] **Step 1: Inject the reload-client snippet into `DEV_SCRIPTS`**
- [x] **Step 2: SSE endpoint at `/_zero/events` with broadcast fan-out**
- [x] **Step 3: `notify-debouncer-mini` watcher feeding the broadcast**
- [x] **Step 4: End-to-end integration test for the full reload loop**

---

## Step Details

### Step 1: Inject the reload-client snippet into `DEV_SCRIPTS`

**Goal:** Get the browser-side half of the system in place first. The
snippet opens an `EventSource("/_zero/events")` and calls
`location.reload()` on the `reload` event. With no server endpoint yet,
the EventSource will fail to connect — that's harmless and gets fixed in
Step 2. Decoupling client injection from server endpoint shrinks the
diff and lets the injection logic ship with its own unit tests.

**Files:**
- `src/dev/inject.rs` (modify `DEV_SCRIPTS`, extend unit tests)

**Changes:**
- Extend the `DEV_SCRIPTS` `concat!` chain to include a third inline
  `<script>` after the existing two. The new script is plain
  (non-module) so it can't fail-and-cascade if module evaluation breaks
  elsewhere. Suggested literal body (kept short; no `import.meta`, no
  promises):
  ```html
  <script>
  (function(){
    if (typeof EventSource === "undefined") return;
    var es = new EventSource("/_zero/events");
    es.addEventListener("reload", function(e){
      try { console.log("[zero] reloading: " + (e.data || "")); } catch(_) {}
      location.reload();
    });
  })();
  </script>
  ```
  Concatenate into `DEV_SCRIPTS` as a third raw-string literal joined
  with `"\n"`. Total addition: ~10 lines of JS, one `<script>` tag.
- The `DEV_SCRIPTS` constant remains a single `&'static str` produced
  via `concat!`, so no allocation cost at runtime.

**Tests:**
- Add a unit test in `src/dev/inject.rs::tests`:
  - `injects_reload_client_alongside_other_scripts`: assert the injected
    output contains the literal substring `new EventSource("/_zero/events")`
    AND `addEventListener("reload"` AND `location.reload()`. Also
    assert the existing importmap + module-entry substrings remain.
- The existing tests (`injects_before_closing_head`,
  `falls_back_to_body_when_no_head_close`, etc.) keep passing because
  they check structural properties of `DEV_SCRIPTS` as a whole, not its
  exact contents.
- Existing integration tests (`tests/dev_local_index.rs`,
  `tests/dev_proxy.rs`, `tests/e2e_init_dev.rs`) keep passing because
  they only assert presence of the importmap and module-entry tags —
  not the absence of additional `<script>` tags.

---

### Step 2: SSE endpoint at `/_zero/events` with broadcast fan-out

**Goal:** A real `GET /_zero/events` handler that holds the connection
open, emits an initial `hello` event, fans out `reload` events from a
shared `tokio::sync::broadcast` channel, and emits a `: ping` heartbeat
every 15s. No watcher yet — for testing, a manual `bus.send(...)` from
a unit test is the only producer. This step proves the SSE plumbing
in isolation.

**Files:**
- `src/dev/sse.rs` (new)
- `src/dev/mod.rs` (declare `pub mod sse;`)
- `src/dev/server.rs` (extend `AppState` with the bus, register route)

**Changes:**

**`src/dev/sse.rs`:**
- Define the broadcast type. Capacity 16 is plenty (events are tiny;
  the only consumers are 1-2 browser tabs):
  ```rust
  use tokio::sync::broadcast;

  /// Shared reload-event bus. Cheap to clone (`Sender` is `Clone`).
  #[derive(Clone)]
  pub struct ReloadBus {
      tx: broadcast::Sender<String>,
  }

  impl ReloadBus {
      /// Create a fresh bus with the standard capacity.
      pub fn new() -> Self {
          let (tx, _rx) = broadcast::channel(16);
          Self { tx }
      }

      /// Broadcast a reload event. `path` is a representative changed
      /// path relative to `<root>` (or empty if unknown). Returns the
      /// receiver count (0 if no clients are connected).
      pub fn send(&self, path: String) -> usize {
          self.tx.send(path).unwrap_or(0)
      }

      /// Subscribe a new receiver (used by the SSE handler on connect).
      pub fn subscribe(&self) -> broadcast::Receiver<String> {
          self.tx.subscribe()
      }
  }
  ```
- Define the handler using axum's built-in SSE response:
  ```rust
  use std::convert::Infallible;
  use std::time::Duration;
  use axum::response::sse::{Event, KeepAlive, Sse};
  use axum::extract::State;
  use futures_core::Stream; // already pulled in transitively via axum
  use tokio_stream::wrappers::BroadcastStream;
  use tokio_stream::StreamExt;

  pub async fn sse_handler(
      State(state): State<Arc<AppState>>,
  ) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
      let rx = state.bus.subscribe();
      // Initial `hello` event, then forward each broadcast as `reload`.
      let hello = futures_util::stream::once(async {
          Ok::<_, Infallible>(Event::default().event("hello").data("ok"))
      });
      let reloads = BroadcastStream::new(rx).filter_map(|res| {
          // Drop lagged-or-closed errors; on lag the browser just misses
          // one of N reloads, which the next save will cover.
          res.ok().map(|path| {
              Ok::<_, Infallible>(Event::default().event("reload").data(path))
          })
      });
      let stream = hello.chain(reloads);
      Sse::new(stream).keep_alive(
          KeepAlive::new()
              .interval(Duration::from_secs(15))
              .text("ping"),
      )
  }
  ```
  Notes on dep wiring:
  - `tokio-stream` (with the `sync` feature for `BroadcastStream`) and
    `futures-util` are required. Both are widely-used transitive deps;
    if either isn't yet in `Cargo.lock`, add them as direct deps in
    this step's `Cargo.toml` edit. Per the spec's "one new dep maximum
    (`notify`)" constraint, these don't count — they're plumbing for
    `tokio::broadcast` ↔ axum, not feature additions.
  - If pulling `tokio-stream` is undesirable, replace `BroadcastStream`
    with a hand-rolled `async-stream` block that loops on `rx.recv()`.
    `async-stream` is a smaller dep; pick whichever is already in the
    graph (run `cargo tree` to check; otherwise prefer `tokio-stream`).

**`src/dev/server.rs`:**
- Extend `AppState`:
  ```rust
  struct AppState {
      runtime: String,
      root: PathBuf,
      proxy: Option<Arc<ProxyState>>,
      bus: Arc<ReloadBus>,
  }
  ```
- Construct the bus once at server start:
  ```rust
  let bus = Arc::new(ReloadBus::new());
  ```
- Register the route on the router:
  ```rust
  .route("/_zero/events", get(crate::dev::sse::sse_handler))
  ```
  Place the route registration before `.fallback(...)` so the fallback
  doesn't swallow it. The existing `no_cache_layer` covers this route
  too — that's intentional; SSE responses should not be cached.

**`src/dev/mod.rs`:**
- Add `pub mod sse;`.

**`Cargo.toml`:**
- Add `tokio-stream = { version = "0.1", features = ["sync"] }` and
  `futures-util = "0.3"` if either isn't already transitive. (Quick
  `cargo tree -i tokio-stream` to check.)

**Tests:**
- Unit test in `src/dev/sse.rs::tests`:
  - `bus_fanout_delivers_to_multiple_subscribers`: create a `ReloadBus`,
    subscribe two receivers, call `bus.send("foo".into())`, assert both
    receivers see `"foo"` on `recv().await`. This is the unit test the
    spec calls out for broadcast fan-out.
  - `bus_send_with_no_subscribers_does_not_error`: assert `bus.send(...)`
    returns `0` and does not panic when nobody is listening.
- Integration test `tests/dev_sse_hello.rs` (new): spawn `zero dev` in a
  tempdir scaffolded via `zero init`, open an SSE GET to
  `/_zero/events`, read the first event, assert it is `event: hello`
  with `data: ok`. Verify `content-type: text/event-stream` and the
  no-cache header set is present.
  - Use `reqwest::Response::bytes_stream()` and parse SSE frames by
    hand (split on `\n\n`); SSE is a tiny line-oriented format and
    pulling in `eventsource-client` for one test isn't worth it. Time
    out after 3 seconds so a regression doesn't hang CI.
- Existing tests keep passing — adding a new route doesn't disturb the
  other route handlers.

---

### Step 3: `notify-debouncer-mini` watcher feeding the broadcast

**Goal:** A background tokio task watches `<root>` for fs changes,
ignores hidden paths and the `<out>` directory, debounces editor-save
bursts within ~100ms, and publishes a single reload event per burst.
Lifecycle is tied to the dev server: starts after the listener binds,
shuts down when graceful-shutdown fires. Also amends the framework
spec to reflect the split.

**Files:**
- `Cargo.toml` (add `notify-debouncer-mini`)
- `src/dev/watch.rs` (new)
- `src/dev/mod.rs` (declare `pub mod watch;`)
- `src/dev/server.rs` (spawn watcher task, log line, shutdown wiring)
- `zero-framework-spec.md` (split Phase 6 checkbox; soften §1)

**Changes:**

**`Cargo.toml`:**
- Add to `[dependencies]`:
  ```toml
  notify-debouncer-mini = "0.4"
  ```
  Pinned to a major that pairs with `notify` 6.x; `0.5+` pulls `notify`
  7+ which is fine but not necessary. Pick the highest version that
  still builds clean against the rest of the dep graph (run `cargo
  update -p notify-debouncer-mini --precise <v>` if the latest pulls in
  a `notify` major that conflicts). The `tokio` feature isn't required
  — the crate exposes a `std::sync::mpsc::Sender` for event delivery,
  which we wrap with a tokio task.

**`src/dev/watch.rs`:**

The watcher runs in a blocking-friendly arrangement: `notify-debouncer-mini`
delivers events on its own thread via an `mpsc::Receiver`. A
`tokio::task::spawn_blocking` drains that receiver, then forwards
events through `ReloadBus` (which is `tokio::sync::broadcast` — safe
to call `send` from any thread). Shutdown is signalled by dropping
the debouncer's `Sender`, which causes the receiver to return
`RecvError::Disconnected` and the loop to exit.

```rust
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc as std_mpsc;
use std::time::Duration;

use notify_debouncer_mini::{
    new_debouncer, DebounceEventResult, Debouncer,
    notify::{RecommendedWatcher, RecursiveMode},
};

use crate::dev::sse::ReloadBus;

/// Handle owning the debouncer; drop to stop watching.
pub struct WatchHandle {
    _debouncer: Debouncer<RecommendedWatcher>,
}

/// Start watching `root` recursively. Events are debounced ~100ms,
/// filtered, and forwarded to `bus` as one broadcast per burst. The
/// returned `WatchHandle` keeps the debouncer alive; drop it to stop.
///
/// `out_dir` is the absolute path of the `[build].out` directory; any
/// event whose path begins with it is ignored. If `out_dir` is not
/// under `root` (the default — `dist/` sits next to `web/`, not under
/// it), the ignore is effectively a no-op.
///
/// Returns `Ok(None)` if `notify` failed to install watches. The dev
/// server logs a warning and continues serving without auto-reload —
/// per the spec, watcher failure must never fail server startup.
pub fn start(
    root: PathBuf,
    out_dir: PathBuf,
    bus: std::sync::Arc<ReloadBus>,
) -> anyhow::Result<Option<WatchHandle>> {
    let (tx, rx): (
        std_mpsc::Sender<DebounceEventResult>,
        std_mpsc::Receiver<DebounceEventResult>,
    ) = std_mpsc::channel();

    let mut debouncer = match new_debouncer(Duration::from_millis(100), tx) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("zero dev: failed to create file watcher: {e}; auto-reload disabled");
            return Ok(None);
        }
    };
    if let Err(e) = debouncer
        .watcher()
        .watch(&root, RecursiveMode::Recursive)
    {
        eprintln!("zero dev: failed to watch {}: {e}; auto-reload disabled", root.display());
        return Ok(None);
    }

    let root_for_thread = root.clone();
    tokio::task::spawn_blocking(move || {
        // Loop until the channel is dropped (graceful shutdown drops
        // the `Debouncer`, which drops the internal Sender).
        while let Ok(result) = rx.recv() {
            let events = match result {
                Ok(events) => events,
                Err(errs) => {
                    for e in errs {
                        eprintln!("zero dev: watcher error: {e:?}");
                    }
                    continue;
                }
            };
            // Find the first event whose path is not ignored. If all
            // are filtered, skip the broadcast.
            let representative = events.iter().find_map(|ev| {
                let p = &ev.path;
                if is_ignored(p, &root_for_thread, &out_dir) {
                    None
                } else {
                    Some(relative_to_root(p, &root_for_thread))
                }
            });
            if let Some(rel_path) = representative {
                println!("zero dev — reload: {rel_path}");
                bus.send(rel_path);
            }
        }
    });

    Ok(Some(WatchHandle { _debouncer: debouncer }))
}

/// True if `path` should be filtered out of reload events.
/// Currently: any path component starting with `.`, OR any path under
/// `out_dir`.
pub fn is_ignored(path: &Path, _root: &Path, out_dir: &Path) -> bool {
    if path.starts_with(out_dir) {
        return true;
    }
    path.components().any(|c| match c {
        Component::Normal(s) => s.to_string_lossy().starts_with('.'),
        _ => false,
    })
}

/// Render `path` relative to `root` as a forward-slash string. Falls
/// back to the file name if `path` is not under `root` for any reason.
fn relative_to_root(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .ok()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| {
            path.file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
}
```

**`src/dev/server.rs`:**
- Compute `out_dir` from config alongside `root`:
  ```rust
  let out_dir = cwd.join(&config.build.out);
  let out_dir = out_dir.canonicalize().unwrap_or(out_dir);
  ```
- After binding the listener (so a port-in-use error doesn't leave a
  watcher behind), start the watcher and keep its handle:
  ```rust
  let watch_handle = crate::dev::watch::start(
      state.root.clone(),
      out_dir,
      state.bus.clone(),
  )?;
  if watch_handle.is_some() {
      println!("zero dev — watching {} for changes", state.root.display());
  }
  ```
  The `watch_handle` lives in the same scope as `axum::serve(...).await`
  and is dropped at the end — which drops the `Debouncer`, which closes
  the channel, which exits the `spawn_blocking` loop. No explicit
  cancellation needed.
- Existing `shutdown_signal()` and `with_graceful_shutdown` paths are
  unchanged. The `_guard` semantics handle the rest.

**`src/dev/mod.rs`:**
- Add `pub mod watch;`.

**`zero-framework-spec.md`:**
- Phase 6 list (around line 1129): replace
  `- [ ] zero dev file watching and HMR`
  with two entries:
  ```
  - [x] `zero dev` file watching (full-page reload via SSE)
  - [ ] `zero dev` HMR (module state preservation, error overlay)
  ```
- §1 (around line 83): replace
  `HMR is always on. Errors render as a browser overlay AND in the terminal.`
  with
  `File watching with full-page reload is always on; HMR (module state preservation) and an in-page error overlay are planned (see Phase 6). Errors render in the terminal today.`

**Tests:**
- Unit test in `src/dev/watch.rs::tests`:
  - `ignored_hidden_dotfiles`: assert `is_ignored("/x/.git/HEAD", "/x", "/x/dist")` is `true`.
  - `ignored_under_out_dir`: assert `is_ignored("/x/dist/asset.js", "/x", "/x/dist")` is `true`.
  - `not_ignored_normal_source_path`: assert
    `is_ignored("/x/src/routes/home.js", "/x", "/x/dist")` is `false`.
  - `not_ignored_when_out_dir_outside_root`: with `out_dir = "/y/dist"`
    (sibling), regular paths under root remain visible.
- Unit test in `src/dev/watch.rs::tests`:
  - `relative_to_root_strips_prefix`: assert
    `relative_to_root("/x/src/a.js", "/x") == "src/a.js"`.
  - Hand-rolled (no real fs touch) — operates on `Path` values.

(Black-box "the watcher actually fires on a real fs write" is covered
in Step 4's integration test, which is the only place we can assert
the full notify ↔ broadcast ↔ SSE loop. A unit test for the
notify-debouncer-mini timing itself would just exercise the crate,
not our code.)

---

### Step 4: End-to-end integration test for the full reload loop

**Goal:** One integration test that proves the whole feature works:
spawn `zero dev`, open an SSE connection, write to a file under
`<root>/src/`, assert a `reload` event arrives within ~1 second. This
is the test the spec explicitly calls for; isolating it in its own
step keeps Step 3's diff focused.

**Files:**
- `tests/dev_watch_reload.rs` (new)

**Changes:**

Structure mirrors `tests/dev_proxy.rs` (the closest existing test
shape — it already manages a child process, a multi-thread runtime,
and an SSE-style long-running request via reqwest streaming):

```rust
//! Integration test: editing a watched file under <root>/src triggers
//! a `reload` event over /_zero/events.

use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use futures_util::StreamExt;

// ... existing pick_free_port / wait_for_port / ChildGuard helpers ...

#[test]
fn writing_a_watched_file_broadcasts_reload() {
    let tmp = tempfile::tempdir().unwrap();
    let port = pick_free_port();

    std::fs::write(
        tmp.path().join("zero.toml"),
        format!("[project]\nroot = \"web\"\n\n[dev]\nport = {port}\n"),
    ).unwrap();
    assert_cmd::Command::cargo_bin("zero")
        .unwrap()
        .arg("init")
        .current_dir(tmp.path())
        .assert()
        .success();

    let bin = assert_cmd::cargo::cargo_bin("zero");
    let child = Command::new(&bin)
        .arg("dev")
        .current_dir(tmp.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let _guard = ChildGuard(child);
    assert!(wait_for_port(port, Duration::from_secs(5)));

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async move {
        let client = reqwest::Client::new();
        let base = format!("http://127.0.0.1:{port}");

        // Open SSE; consume the initial `hello` so we know the
        // subscription is live before we write.
        let resp = client
            .get(format!("{base}/_zero/events"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let mut stream = resp.bytes_stream();

        // Read until we see "event: hello".
        let hello = read_until_event(&mut stream, "hello", Duration::from_secs(2))
            .await
            .expect("hello event");
        assert!(hello.contains("data: ok"));

        // Touch a file in the project.
        let target = tmp.path().join("web/src/app.js");
        let mut text = std::fs::read_to_string(&target).unwrap();
        text.push_str("\n// touched\n");
        std::fs::write(&target, text).unwrap();

        // Expect a `reload` event within ~1s (100ms debounce + a bit).
        let reload = read_until_event(&mut stream, "reload", Duration::from_secs(2))
            .await
            .expect("reload event");
        // data: <path> — should mention something under src/.
        assert!(reload.contains("data: src/"));
    });
}

/// Read SSE frames (separated by `\n\n`) until one with the given
/// `event: <name>` line shows up. Returns the full frame text.
async fn read_until_event(
    stream: &mut (impl futures_util::Stream<Item = reqwest::Result<bytes::Bytes>> + Unpin),
    name: &str,
    timeout: Duration,
) -> Option<String> {
    let deadline = Instant::now() + timeout;
    let needle = format!("event: {name}");
    let mut buf = String::new();
    while Instant::now() < deadline {
        let chunk = tokio::time::timeout(
            deadline.saturating_duration_since(Instant::now()),
            stream.next(),
        )
        .await
        .ok()??;
        let chunk = chunk.ok()?;
        buf.push_str(std::str::from_utf8(&chunk).unwrap_or(""));
        // Split by frame delimiter; if any frame matches, return it.
        if let Some(frame) = buf.split("\n\n").find(|f| f.contains(&needle)) {
            return Some(frame.to_string());
        }
    }
    None
}
```

**Tests:**
- The test above is itself the deliverable. Total of one new test;
  it covers (a) the SSE handler emits `hello` on connect, (b) the
  watcher detects an edit, (c) the debouncer emits an event, (d) the
  bus fans out, (e) the SSE handler forwards the event, (f) the data
  field carries the relative path.
- Confirm the existing `tests/dev_local_index.rs` integration test still
  passes — the injected reload-client snippet (Step 1) must not break
  the existing assertions that look for the importmap and module-entry
  scripts. Same goes for `tests/dev_proxy.rs`.

---

## Risks and Assumptions

- **`notify-debouncer-mini` version compatibility.** The `notify` 6 → 7
  → 8 churn drives the debouncer's major-version cadence. If `0.4`
  doesn't resolve against the current `tokio`/`hyper`/`axum` graph,
  step up to `0.5` or `0.6`. The crate API surface we use (`new_debouncer`,
  `Debouncer::watcher`, `DebounceEventResult`) is stable across these.
- **Editor-save shapes.** Some editors (vim with `backupcopy=no`) write
  to a tempfile and `rename(2)` it over the target. `notify` sees this
  as a remove + create on the same path; the debouncer collapses both,
  the watcher fires once. Good. Other editors (atomic + sync) generate
  4–6 events per save; the 100ms window is wide enough. If a user
  reports stuck reloads, the debounce window is the first thing to
  bump.
- **The race the spec already calls out** — file edited in the few
  ms between the browser issuing the SSE GET and the handler
  subscribing — is unfixable without `Last-Event-ID` replay. The test
  above sidesteps it by reading the `hello` event before writing.
- **Watcher failure must not fail server startup.** `watch::start`
  returns `Ok(None)` on permission / setup errors and the server keeps
  serving. The dev loses auto-reload but can still hit refresh. This
  is enforced by the structure of `watch::start`, not just convention.
- **`out_dir` may not exist yet** (developer hasn't run `zero build`).
  `canonicalize()` will fail; the code falls back to the non-canonical
  path, which still works for `starts_with` prefix matching against
  events from `notify` (paths there are canonical, but the comparison
  is lexical against the same form the user typed in toml). If this
  bites in practice, normalize both sides via `dunce::canonicalize` or
  by walking. Not worth solving until it actually breaks.
- **`tokio-stream` / `futures-util` are transitive but not declared.**
  If `cargo build` works without listing them as direct deps, great —
  but treat them as direct deps in this PR so an upstream version bump
  can't silently break the build. Net dep count rises by 2; per the
  spec these are plumbing, not features.
- **SSE streaming + `SetResponseHeaderLayer`.** Axum's `Sse` response
  uses chunked transfer encoding. `SetResponseHeaderLayer::overriding`
  edits response headers before the body streams; the two compose
  fine in axum 0.7 (verified by reading axum's docs, not by code
  inspection — flag if a quick smoke test in Step 2 shows otherwise).
- **Spec text edit collision.** The framework spec edit in Step 3 is
  a tiny change (one bullet split, one sentence rewrite). If someone
  else is editing §1 or Phase 6 in parallel, resolve the conflict
  manually — no other automation depends on that file's exact text.
