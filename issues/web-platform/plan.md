# Plan: Web Platform surface audit and shim expansion

## Summary

Widen the test-environment shim from "DOM" to "Web Platform" by adding the
closed set of Web Platform globals that today fall off the cliff under Boa
(Fetch, AbortController/AbortSignal, URL/URLSearchParams, TextEncoder/Decoder,
Blob/File/FormData, structuredClone, queueMicrotask). Split the new code into
sibling files under `runtime/` (`fetch-shim.js`, `url-shim.js`,
`encoding-shim.js`, `binary-shim.js`, `clone-shim.js`) that the existing
`runtime/zero-runtime` build script concatenates into `ZERO_DOM_SHIM_BODY`
alongside `dom-shim.js`. The composite shim continues to install its
identifiers onto `globalThis` as a side effect of the harness's
`eval_dom_shim` call. `fetch` is the only intentional stub — its default
rejects with a clear, actionable message. Per-test reset of overridden
`globalThis.fetch` is wired through `runtime/test.js::cleanup()` via a new
`__resetFetch__` global, matching the existing `__clearAllTimers__` pattern.
A smoke-test file `runtime/web-platform.test.js` (node:test format,
consistent with the still-unconverted runtime suite) exercises every
audited API. Documentation lands in `zero-framework-spec.md` (new §8.x
"Web Platform surface in `zero test`") and `BEST_PRACTICES.md` (new
"Testing against the Web Platform" subsection inside §9 Testing).

## Prerequisites

None. Spec open questions are resolved here:

1. **File layout** — split. New files: `runtime/fetch-shim.js`,
   `runtime/url-shim.js`, `runtime/encoding-shim.js`, `runtime/binary-shim.js`,
   `runtime/clone-shim.js`. Each is concatenated into the existing
   `ZERO_DOM_SHIM_BODY` by `crates/zero-runtime/build.rs`. `dom-shim.js` is
   left untouched.
2. **Audit method** — by-hand grep across `runtime/`, `examples/`,
   `showcase/` plus a Boa baseline probe (a one-off Rust test that asks Boa
   `typeof <Name>` for each candidate). Output: a closed list in Step 1's
   notes, transcribed into `zero-framework-spec.md` at the end (Step 10).
3. **`fetch` default behavior** — returns a rejected `Promise` with the
   message specified in the spec. Matches `runtime/http.test.js`'s existing
   stub pattern.
4. **`AbortSignal.any` semantics** — implemented per the WHATWG spec
   (composite signal that aborts when any input aborts, with `reason`
   propagated from the first aborter). Used by
   `runtime/app.js::_composeSignals` already (it falls back to manual
   composition when `AbortSignal.any` is absent — the shim now removes that
   fallback path from ever firing).
5. **`Promise.withResolvers` Boa support** — verified in Step 1's baseline
   probe. If present, no shim needed; if absent, polyfilled at the top of
   `runtime/clone-shim.js`.
6. **`structuredClone` scope** — option (c): plain objects/arrays + Date /
   RegExp / Map / Set / Error / ArrayBuffer / typed arrays. Functions, DOM
   nodes, and transferables throw `DataCloneError`-shaped errors
   (spec-correct shape; message uses the "zero test:" prefix to stay
   consistent with other shim errors).
7. **Documentation phasing** — new §8.x subsection in `zero-framework-spec.md`
   titled "Web Platform surface in `zero test`" (not a new Phase 14). The
   spec is a closed enumeration of the contract; phase markers track running
   implementation order, not contract documentation.
8. **Smoke test coverage** — `runtime/web-platform.test.js` (single file,
   node:test format) exercises every audited API at least once. Runs under
   `node --test runtime/web-platform.test.js` and stays parallel to the
   other unconverted runtime tests. After the runtime-tests/plan.md Step 3
   conversion (separate work), this file can be converted alongside it.

## Steps

- [x] **Step 1: Audit the surface and probe Boa's baseline**
- [x] **Step 2: Extend `build.rs` to concatenate sibling shim files**
- [x] **Step 3: Implement `AbortController` / `AbortSignal` / `AbortSignal.any`**
- [x] **Step 4: Implement `Headers` / `Request` / `Response` / `fetch` default**
- [x] **Step 5: Implement `URL` / `URLSearchParams`**
- [x] **Step 6: Implement `TextEncoder` / `TextDecoder`**
- [x] **Step 7: Implement `Blob` / `File` / `FormData`**
- [x] **Step 8: Implement `structuredClone` / `queueMicrotask` (+ `Promise.withResolvers` polyfill if absent)**
- [x] **Step 9: Wire `__resetFetch__` into `cleanup()` and add `runtime/web-platform.test.js` smoke tests**
- [x] **Step 10: Document in `zero-framework-spec.md` and `BEST_PRACTICES.md`**

---

## Step Details

### Step 1: Audit the surface and probe Boa's baseline

**Goal:** Lock the closed list of Web Platform identifiers the shim must
provide. Catch any name we missed in the spec, and verify which (if any)
identifiers Boa 0.21 already ships so the implementation work avoids
duplicating engine builtins. No production code changes in this step.

**Files:**
- `issues/web-platform/audit.md` (new — scratch notes; transcribed into
  the framework spec in Step 10)
- `crates/zero-test-runner/tests/web_platform_baseline.rs` (new, temporary —
  deleted at the end of Step 10 once the docs reference is settled)

**Changes:**
- **Grep pass 1 (framework):** identify every Web Platform identifier in
  `runtime/*.js` (excluding `*.test.js`). Today known: `Headers`, `Request`,
  `Response`, `fetch`, `AbortController`, `AbortSignal`, `AbortSignal.any`,
  `URLSearchParams`, `ReadableStream` (used only in `typeof` guard inside
  `_isPlainObject`; not implemented). Run:
  ```bash
  grep -nE '\b(Headers|Request|Response|fetch|AbortController|AbortSignal|URL|URLSearchParams|TextEncoder|TextDecoder|Blob|File|FormData|structuredClone|queueMicrotask|ReadableStream|WritableStream|TransformStream|WebSocket|EventSource|MessageChannel|MessagePort|BroadcastChannel|crypto|SubtleCrypto)\b' runtime/*.js | grep -v '\.test\.js'
  ```
  Verify the output matches the spec's enumeration. Record any surprise hits
  in `audit.md`.
- **Grep pass 2 (examples + showcase):** same regex across
  `examples/{counter,todos,tracker}/web/src/` and `showcase/src/`. Today
  known: only `fetch` (as `typeof fetch` in route loader signatures). Record
  in `audit.md`.
- **Grep pass 3 (documented user surface):** confirm spec Section 1's third
  set (`URL`, `URLSearchParams`, `TextEncoder`, `TextDecoder`, `Blob`,
  `File`, `FormData`, `structuredClone`, `queueMicrotask`) is the right
  superset of what users will reach for. No code action.
- **Boa baseline probe:** write a short integration test that builds a
  fresh `boa_engine::Context`, evaluates a probe script that records
  `typeof X` for each candidate, and prints the results. Concrete shape:
  ```rust
  #[test]
  fn web_platform_baseline_in_boa() {
      use boa_engine::{Context, Source};
      let names = [
          "Headers", "Request", "Response", "fetch",
          "AbortController", "AbortSignal", "URL", "URLSearchParams",
          "TextEncoder", "TextDecoder", "Blob", "File", "FormData",
          "structuredClone", "queueMicrotask",
      ];
      let mut ctx = Context::default();
      for name in names {
          let src = format!("typeof {name}");
          let result = ctx.eval(Source::from_bytes(src.as_bytes()))
              .map(|v| v.to_string(&mut ctx).unwrap().to_std_string_escaped())
              .unwrap_or_else(|_| "<eval-error>".into());
          println!("{name}: {result}");
      }
      // Also: Promise.withResolvers and AbortSignal.any
      for src in ["typeof Promise.withResolvers", "typeof AbortSignal?.any"] {
          let result = ctx.eval(Source::from_bytes(src.as_bytes()))
              .map(|v| v.to_string(&mut ctx).unwrap().to_std_string_escaped())
              .unwrap_or_else(|_| "<eval-error>".into());
          println!("{src}: {result}");
      }
  }
  ```
  Run with `cargo test -p zero-test-runner web_platform_baseline_in_boa --
  --nocapture` and record the matrix in `audit.md`. Expected outcome from
  reading Boa 0.21's feature set: every Web Platform name returns
  `undefined`; `Promise.withResolvers` returns `function` (ES2024 supported
  in 0.21). If the probe surprises us — e.g., Boa happens to ship `URL` —
  the corresponding implementation step downgrades to "skip; rely on
  Boa's builtin" and `audit.md` records the reason.
- **Audit output:** `issues/web-platform/audit.md` ends with a single
  bulleted list, copy-paste ready for the framework spec in Step 10. The
  list groups by category (Fetch / URLs / Encoding / Binary / Cloning &
  scheduling) and notes any Boa builtin used in lieu of a shim.

**Tests:**
- `cargo test -p zero-test-runner web_platform_baseline_in_boa --
  --nocapture` runs and prints the matrix.
- No production code changes; existing `cargo test --workspace` and
  `node --test runtime/*.test.js` continue to pass.

---

### Step 2: Extend `build.rs` to concatenate sibling shim files

**Goal:** Set up the multi-file shim layout before adding API
implementations. After this step, the build still produces a working shim
(empty new files are no-ops); subsequent steps add code to one file at a
time without further build-script churn.

**Files:**
- `crates/zero-runtime/build.rs`
- `runtime/fetch-shim.js` (new, empty stub with file header)
- `runtime/url-shim.js` (new, empty stub)
- `runtime/encoding-shim.js` (new, empty stub)
- `runtime/binary-shim.js` (new, empty stub)
- `runtime/clone-shim.js` (new, empty stub)

**Changes:**
- Each new `runtime/*.js` file starts as a single JSDoc-style file header
  comment and nothing else — e.g.:
  ```js
  /**
   * Fetch-API shim (Headers, Request, Response, fetch default).
   *
   * Concatenated into `ZERO_DOM_SHIM_BODY` by `crates/zero-runtime/build.rs`
   * and evaluated as a script by the test harness before user modules run.
   * No `import` / `export`; relies on globals installed by `dom-shim.js`.
   */
  ```
- `build.rs`: introduce a constant for the shim file list and walk it when
  building `zero_dom_shim_body.js`:
  ```rust
  /// Web Platform shim files concatenated after `dom-shim.js`.
  const WEB_PLATFORM_FILES: &[&str] = &[
      "fetch-shim.js",
      "url-shim.js",
      "encoding-shim.js",
      "binary-shim.js",
      "clone-shim.js",
  ];
  ```
  Add a `cargo:rerun-if-changed=` directive for each. Extend the existing
  `zero_dom_shim_body.js` writer:
  ```rust
  let mut shim_body = cleaned;          // from dom-shim.js
  if !shim_body.ends_with('\n') { shim_body.push('\n'); }
  if !alias_lines.is_empty() { shim_body.push_str(&alias_lines); }
  for f in WEB_PLATFORM_FILES {
      let path = runtime_dir.join(f);
      let raw = fs::read_to_string(&path)
          .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
      let (cleaned, alias_lines) = strip(&raw);
      shim_body.push_str(&format!("\n/* === {f} === */\n"));
      shim_body.push_str(&cleaned);
      if !cleaned.ends_with('\n') { shim_body.push('\n'); }
      if !alias_lines.is_empty() { shim_body.push_str(&alias_lines); }
  }
  ```
- The existing `lib.rs` assertions
  (`ZERO_DOM_SHIM_BODY.contains("function createElement(")` and the
  `globalThis.document = document` install) continue to pass because
  `dom-shim.js` content is unchanged.

**Tests:**
- `cargo build -p zero-runtime` succeeds.
- `cargo test -p zero-runtime` passes (the two existing
  `ZERO_DOM_SHIM_BODY` assertions hold).
- `cargo test --workspace` passes; no test should observe behavior change
  yet — the new files are no-op header comments.
- `node --test runtime/*.test.js` still passes (new files have no
  matching `*.test.js` partners yet).

---

### Step 3: Implement `AbortController` / `AbortSignal` / `AbortSignal.any`

**Goal:** Install the abort primitives. These come before `fetch` because
`Request`'s `init.signal` plumbing references `AbortSignal`, and
`runtime/app.js::_composeSignals` already calls `AbortSignal.any` when
available — once the shim provides it, the manual-composition fallback
becomes dead code inside `zero test` (still alive in browsers that lack
`AbortSignal.any`).

**Files:**
- `runtime/fetch-shim.js`

**Changes:**
- Implement an `EventTarget`-shaped base shared between `AbortSignal` and
  any future event-target needs the file may grow. Keep it module-private:
  ```js
  function _makeAbortEventTarget() {
    const listeners = new Map(); // type → Set<Function>
    return {
      addEventListener(type, fn) {
        let set = listeners.get(type);
        if (!set) { set = new Set(); listeners.set(type, set); }
        set.add(fn);
      },
      removeEventListener(type, fn) {
        const set = listeners.get(type);
        if (set) set.delete(fn);
      },
      dispatchEvent(ev) {
        const set = listeners.get(ev.type);
        if (!set) return true;
        for (const fn of [...set]) fn.call(this, ev);
        return true;
      },
    };
  }
  ```
  Note (per `boa-maplock-finalizer` memory): keep this helper a top-level
  named function; do not inline branchy variants. The Map use here is
  contained inside the helper instance and never escapes across files.
- `AbortSignal` class:
  ```js
  class AbortSignal {
    constructor() {
      this._aborted = false;
      this._reason = undefined;
      Object.assign(this, _makeAbortEventTarget());
      this.onabort = null;
    }
    get aborted() { return this._aborted; }
    get reason() { return this._reason; }
    throwIfAborted() { if (this._aborted) throw this._reason; }
  }
  ```
  Plus three static helpers as class statics, each its own named function
  to keep Boa's GC happy:
  - `AbortSignal.abort(reason)` → returns an already-aborted signal.
  - `AbortSignal.timeout(ms)` → returns a signal that aborts via
    `setTimeout(_, ms)` on the shim's timer queue (the `ms` is ignored by
    the shim; aborts on next job drain). Reason is a `DOMException`-shaped
    plain object (`{ name: 'TimeoutError', message: '...' }`).
  - `AbortSignal.any(signals)` → returns a composite signal that aborts
    when any input aborts, propagating the first aborter's reason.
- `AbortController` class:
  ```js
  class AbortController {
    constructor() { this.signal = new AbortSignal(); }
    abort(reason) {
      const sig = this.signal;
      if (sig._aborted) return;
      sig._aborted = true;
      sig._reason = reason ?? _makeAbortError();
      if (typeof sig.onabort === 'function') sig.onabort({ type: 'abort' });
      sig.dispatchEvent({ type: 'abort' });
    }
  }
  function _makeAbortError() {
    const err = new Error('signal is aborted without reason');
    err.name = 'AbortError';
    return err;
  }
  ```
- Install on `globalThis` only when missing (per the shim's existing
  feature-detect pattern):
  ```js
  if (typeof globalThis.AbortController === 'undefined') {
    Object.defineProperty(globalThis, 'AbortController', {
      value: AbortController, writable: true, configurable: true,
    });
  }
  if (typeof globalThis.AbortSignal === 'undefined') {
    Object.defineProperty(globalThis, 'AbortSignal', {
      value: AbortSignal, writable: true, configurable: true,
    });
  }
  ```

**Tests:**
- New file: `runtime/fetch-shim.test.js` (node:test format; matches the
  existing `runtime/*.test.js` pattern). For this step add a
  `describe('AbortController / AbortSignal')` block covering:
  - new controller; `signal.aborted === false`, `signal.reason ===
    undefined`.
  - `controller.abort()` flips `aborted` to `true`, populates `reason`
    with an `AbortError`-named Error, fires `'abort'` event listeners
    once.
  - `controller.abort('user reason')` populates `reason === 'user reason'`.
  - `signal.throwIfAborted()` throws the stored reason after abort.
  - `AbortSignal.abort('x').aborted === true` and `reason === 'x'`.
  - `AbortSignal.timeout(0)` fires abort on next microtask drain (await
    a Promise to settle the shim's timer queue, then assert
    `signal.aborted === true`).
  - `AbortSignal.any([a, b])`: aborts when either input aborts; first
    aborter's reason wins.
  - `AbortSignal.any([alreadyAborted])` returns a signal whose `aborted
    === true` immediately.
- Run: `node --test runtime/fetch-shim.test.js` passes.
- `cargo test --workspace` still passes (build script picks up new file
  via the rerun-if-changed declaration from Step 2).
- Boa-side smoke: `cargo test -p zero --test examples_tests tracker_tests_pass`
  passes (existing `runtime/app.js::_composeSignals` now picks the
  `AbortSignal.any` branch; the test should observe no behavior change).

---

### Step 4: Implement `Headers` / `Request` / `Response` / `fetch` default

**Goal:** Install the Fetch API surface. `fetch` is an intentional stub
that rejects with the spec's actionable error; users override
`globalThis.fetch` per test. `Headers`, `Request`, `Response` are real
implementations sized to satisfy `runtime/http.js` and
`runtime/http.test.js`'s stub pattern.

**Files:**
- `runtime/fetch-shim.js`

**Changes:**
- Append to the file built in Step 3. Implementation order inside the
  file: `Headers` → `Request` → `Response` → `fetch` default → globals
  install. Each class's body stays well under 80 lines; helpers split out
  to named module-level functions.
- **`Headers`:**
  - Map-shaped, but keys stored lower-cased to enforce case-insensitivity.
    Use a plain object + insertion-order array (mirroring
    `_makeStorage()`'s pattern in `dom-shim.js`) rather than a `Map`, to
    keep distance from `boa-maplock-finalizer`.
  - Constructor accepts: another `Headers` instance, a plain object, an
    array of `[name, value]` pairs, or `undefined`.
  - Methods: `get(name)`, `set(name, value)`, `has(name)`, `delete(name)`,
    `append(name, value)` (comma-joined per spec), `forEach(cb, thisArg?)`,
    `entries()`, `keys()`, `values()`, `[Symbol.iterator]` aliased to
    `entries`.
- **`Request`:**
  - Constructor `(input, init?)` where `input` is a string URL, a `URL`
    instance, or another `Request`. When `input` is a `Request`, copy
    its fields then overlay `init`.
  - Fields: `url` (string), `method` (uppercased; default `'GET'`),
    `headers` (`Headers` instance), `body` (the init body verbatim;
    string / Blob / URLSearchParams / FormData / ArrayBuffer / typed
    array all accepted at construction time — they are surface-only,
    consumption is `text()` / `json()` on the body), `signal`
    (`AbortSignal` instance; default = a fresh non-aborted one), `mode`,
    `credentials`, `cache`, `redirect`, `referrer`, `integrity`
    (all pass-through string fields with sensible defaults; not enforced).
  - Body consumption: `text()` returns `Promise<string>` derived from
    the stored body (`String(body)` for strings, `await body.text()` for
    Blob, etc.). `json()` is `text().then(JSON.parse)`. `arrayBuffer()` /
    `blob()` reject with a "not supported in zero test" message —
    use the intentional-stub error contract.
- **`Response`:**
  - Constructor `(body?, init?)`. `body` accepts the same shapes as
    `Request`. `init` carries `status` (default 200), `statusText`
    (default `''`), `headers` (default empty `Headers`).
  - `ok` getter: `status >= 200 && status < 300`. `redirected` always
    `false`, `type` always `'default'`, `url` empty string.
  - Body consumption: same `text()` / `json()` shape as `Request`.
    Construct from string or URLSearchParams trivially; from typed array
    / ArrayBuffer use `String.fromCharCode(...new Uint8Array(buf))`
    (latin1 round-trip; tests don't exercise non-ASCII responses).
- **`fetch` default:**
  ```js
  function _zeroDefaultFetch() {
    return Promise.reject(new Error(
      'zero test: globalThis.fetch is not implemented. ' +
      "Stub it in your test's beforeEach (or pass init.fetch to the " +
      'call) — see runtime/http.test.js for the pattern.'
    ));
  }
  if (typeof globalThis.fetch === 'undefined') {
    Object.defineProperty(globalThis, 'fetch', {
      value: _zeroDefaultFetch, writable: true, configurable: true,
    });
  }
  globalThis.__resetFetch__ = () => {
    globalThis.fetch = _zeroDefaultFetch;
  };
  ```
  `__resetFetch__` is the per-test reset hook called by
  `runtime/test.js::cleanup()` in Step 9.
- Install `Headers`, `Request`, `Response` on `globalThis` using the
  same `Object.defineProperty(..., writable: true, configurable: true)`
  pattern.

**Tests:**
- Extend `runtime/fetch-shim.test.js` with:
  - `describe('Headers')`: constructor accepts object / array / Headers /
    undefined; `set` then `get` round-trip; case-insensitivity (`set('X-A',
    '1')` then `get('x-a') === '1'`); `append` joins with `, `; iteration
    yields lowercased names.
  - `describe('Request')`: constructor with string URL and object init;
    `method` uppercased; `headers` populated from init; copy-construct
    from another `Request` overlays init; `signal` defaults to
    non-aborted; `text()` round-trips a string body; `json()` parses;
    `arrayBuffer()` rejects with the "not supported" message.
  - `describe('Response')`: constructor with body + init; `ok` reflects
    status; `headers` reads init; `text()` and `json()` consume body.
  - `describe('fetch default')`: bare `fetch('/x')` rejects with a
    message containing `zero test: globalThis.fetch is not implemented`;
    the rejection is a Promise (not a synchronous throw).
- `node --test runtime/fetch-shim.test.js` passes.
- `node --test runtime/http.test.js` still passes — the file already
  uses real Web globals via Node, so adding the shim doesn't disturb it.
- `cargo test -p zero-test-runner` continues to pass.
- Boa smoke: write a one-off ad-hoc Rust test (delete at end of step) that
  builds a context, evals the dom shim, then evals
  `'typeof Headers === "function" && typeof Request === "function" && typeof Response === "function" && typeof fetch === "function"'`
  and asserts the result is `"true"`. Confirms the installs land under
  Boa's eval, not just under Node's globals.

---

### Step 5: Implement `URL` / `URLSearchParams`

**Goal:** Ship `URL` and `URLSearchParams` so user code (and any future
framework module) can parse query strings and assemble URLs without ad-hoc
string splitting. Pure JS, hand-written, sized to the standard surface.

**Files:**
- `runtime/url-shim.js`
- `runtime/url-shim.test.js` (new)

**Changes:**
- **`URLSearchParams`:**
  - Internal storage: an insertion-order array of `[name, value]` pairs
    (matches WHATWG's "list of name-value tuples"). Avoid `Map` per
    `boa-maplock-finalizer`.
  - Constructor accepts: a string (with or without leading `?`), another
    `URLSearchParams`, a plain object, or an array of pairs. Decoding:
    `%20` → space, `+` → space, percent-decode the rest. Encoding for
    `toString`: `encodeURIComponent` with the spec's small set of
    overrides (`+` for spaces in form-urlencoded). Stick to
    `encodeURIComponent` directly — the apps in scope here don't exercise
    the edge cases (`!`, `'`, `(`, `)`, `~`).
  - Methods: `get` (first match or `null`), `getAll` (all matches),
    `has`, `set` (replaces all with one), `append`, `delete`, `sort`
    (stable, by name), `forEach`, `entries`, `keys`, `values`,
    `toString`, `[Symbol.iterator]` aliased to `entries`. `size` getter
    returns total pair count.
- **`URL`:**
  - Constructor `(input, base?)`. Internal parse via a small named helper
    `_parseUrl(str)` returning `{ protocol, hostname, port, pathname,
    search, hash, username, password }`. Handle `base` by parsing `base`
    first and resolving `input` against it (only relative paths starting
    with `/` or `.` need real resolution; the apps in scope rarely use
    relative-URL construction).
  - Getters and setters for `protocol`, `hostname`, `port`, `host`
    (`hostname[:port]`), `pathname`, `search`, `hash`, `username`,
    `password`, `origin` (read-only). `searchParams` is a
    `URLSearchParams` lazily constructed from `search`; mutations write
    back to `search` (per spec).
  - `toString()` / `toJSON()` reassemble the parts.
  - Static: `URL.canParse(input, base?)` returns `true` if `new URL(...)`
    would not throw. ES2024 — also in modern browsers; cheap to ship.
- **Implementation discipline:** the URL parser is the biggest single
  function in this file. Split into named helpers (`_extractProtocol`,
  `_extractHostPort`, `_extractPath`, `_extractQuery`, `_extractFragment`)
  to stay under the 80-line rule. Keep each helper a top-level function;
  do not nest closures over shared state across branches (per
  `boa-maplock-finalizer`).
- Install on `globalThis` with the same defineProperty pattern.

**Tests:**
- `runtime/url-shim.test.js` covers:
  - `URLSearchParams`: constructor forms; `get` / `getAll` / `has` /
    `set` / `append` / `delete`; iteration order; `toString` encodes;
    constructor decodes `+` as space.
  - `URL`: simple absolute parse (`https://example.com/a/b?x=1#h`);
    field round-trips; relative against base (`new URL('/x', 'https://h/y')`
    → `pathname === '/x'`); `searchParams.get` reads from query;
    `searchParams.set` writes back to `.search`; `toString` reassembles;
    `URL.canParse('not a url')` returns `false`.
- `node --test runtime/url-shim.test.js` passes.
- `cargo test --workspace` still passes.

---

### Step 6: Implement `TextEncoder` / `TextDecoder`

**Goal:** UTF-8 string ↔ bytes interop. Sized to the simplest path —
`encode(str)` returns a `Uint8Array`; `decode(buf)` returns the original
string. No `encodeInto`, no streaming `decode({ stream: true })`.

**Files:**
- `runtime/encoding-shim.js`
- `runtime/encoding-shim.test.js` (new)

**Changes:**
- **`TextEncoder`:**
  ```js
  class TextEncoder {
    constructor() { this.encoding = 'utf-8'; }
    encode(str) {
      const s = String(str ?? '');
      const out = [];
      _encodeUtf8Into(s, out);
      return new Uint8Array(out);
    }
  }
  ```
  `_encodeUtf8Into` is a named top-level helper containing the per-codepoint
  branch (1/2/3/4-byte forms). Each branch is its own short block; do not
  inline arbitrary if/else cascades — split into `_encode2byte`,
  `_encode3byte`, `_encode4byte` (per `boa-maplock-finalizer`).
- **`TextDecoder`:**
  ```js
  class TextDecoder {
    constructor(encoding = 'utf-8') {
      if (String(encoding).toLowerCase() !== 'utf-8' && String(encoding).toLowerCase() !== 'utf8') {
        throw new RangeError(`zero test: TextDecoder only supports utf-8 (got ${encoding})`);
      }
      this.encoding = 'utf-8';
    }
    decode(input) {
      if (input == null) return '';
      const bytes = input instanceof Uint8Array
        ? input
        : input instanceof ArrayBuffer
          ? new Uint8Array(input)
          : new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
      return _decodeUtf8(bytes);
    }
  }
  ```
  `_decodeUtf8(bytes)` walks bytes and reconstructs codepoints. Split
  branches into `_decode2byte`, etc.
- Install on `globalThis` with defineProperty.

**Tests:**
- `runtime/encoding-shim.test.js`:
  - encode/decode round-trip for ASCII, 2-byte (`'é'`), 3-byte (`'€'`),
    4-byte (`'😀'`) codepoints.
  - `decode(undefined)` returns `''`.
  - `decode(arrayBuffer)` works (typed array view path).
  - `new TextDecoder('latin1')` throws `RangeError` with the
    "zero test:" prefix.
- `node --test runtime/encoding-shim.test.js` passes.

---

### Step 7: Implement `Blob` / `File` / `FormData`

**Goal:** The remaining "upload-shaped" trio. Implementations are minimal
but spec-faithful enough to play with `Headers` / `Request` / `Response`
and `FormData` body construction.

**Files:**
- `runtime/binary-shim.js`
- `runtime/binary-shim.test.js` (new)

**Changes:**
- **`Blob`:**
  - Constructor `(parts, options)`. `parts` is an array of strings,
    `ArrayBuffer`, typed arrays, or other `Blob`s. `options.type` becomes
    `this.type` (default `''`).
  - Internal storage: a single concatenated `Uint8Array` (`_bytes`) built
    at construction time via a named helper `_concatParts(parts)`. Helper
    walks each part type with its own dispatch branch.
  - `size` getter returns `_bytes.byteLength`. `type` getter returns the
    stored type.
  - Methods: `text()` (returns `Promise<string>` via UTF-8 decode of
    `_bytes`), `arrayBuffer()` (returns `Promise<ArrayBuffer>` via
    `_bytes.buffer.slice(0)`), `slice(start, end, contentType)`
    (returns a new `Blob` over `_bytes.subarray(...)`).
- **`File`:**
  - Subclass of `Blob`. Constructor `(parts, name, options)`. Adds
    `this.name` and `this.lastModified` (default `Date.now()`).
- **`FormData`:**
  - Internal storage: insertion-order array of `[name, value]` pairs
    (not a `Map`).
  - Constructor accepts an optional `HTMLFormElement` — the shim's
    forms aren't populated, so this throws "zero test: FormData
    constructor with form element is not supported." per the
    intentional-stub error contract.
  - Methods: `append(name, value, filename?)`, `set(name, value)`,
    `get`, `getAll`, `has`, `delete`, `entries`, `keys`, `values`,
    `forEach`, `[Symbol.iterator]`. Values are stored verbatim; Blob /
    File values get a `filename` (third arg or `value.name` or
    `'blob'`).
- Install on `globalThis` with defineProperty.

**Tests:**
- `runtime/binary-shim.test.js`:
  - `Blob`: construct from string parts; `text()` returns the
    concatenation; `size` matches byte length; `type` matches options;
    `slice()` returns a Blob covering the sub-range.
  - `Blob` from `Uint8Array` and mixed parts (string + typed array).
  - `File`: extends Blob; `name` and `lastModified` populate.
  - `FormData`: append / set / get / getAll / has / delete; iteration
    order matches insertion; appending a Blob attaches a default filename.
  - `new FormData(htmlForm)` throws the spec-shaped stub message.
- `node --test runtime/binary-shim.test.js` passes.

---

### Step 8: Implement `structuredClone` / `queueMicrotask` (+ `Promise.withResolvers` polyfill if absent)

**Goal:** The remaining utility globals. Each is small in isolation; co-
locating them keeps the file count tractable.

**Files:**
- `runtime/clone-shim.js`
- `runtime/clone-shim.test.js` (new)

**Changes:**
- **`structuredClone(value, options?)`:**
  - Recursive deep clone. Uses an internal cycle-tracking `WeakMap` keyed
    by source object → cloned object. (`WeakMap` is ECMA-built into Boa
    and not implicated in `boa-maplock-finalizer`'s `Map` finalizer
    issue — the bug is specific to `Map`'s host-side `MapLock`.)
  - Supported source types: primitives (return as-is), `Array`, plain
    `Object`, `Date`, `RegExp`, `Map`, `Set`, `Error` (preserves `name`
    + `message` + `stack`), `ArrayBuffer` (slice to a new buffer),
    typed arrays (new view over cloned buffer). `URL`, `Blob`, `File`,
    `FormData` clone by re-invoking their constructors over cloned data.
  - Unsupported: functions, DOM nodes (`if (value && typeof value.nodeType === 'number')`),
    `Promise`, `WeakMap` / `WeakSet`. Each throws an Error with `name =
    'DataCloneError'` and message `"zero test: structuredClone: <type>
    cannot be cloned"`.
  - `options.transfer` — unsupported; if non-empty, throw with a stub
    message.
  - Split the dispatch into named helpers: `_cloneArray`,
    `_clonePlainObject`, `_cloneTypedArray`, `_cloneError`, etc., to
    stay under the 80-line rule and to keep each branch a separate
    function call (per `boa-maplock-finalizer`).
- **`queueMicrotask(callback)`:**
  - Schedule via the same path the existing `setTimeout` shim uses:
    `Promise.resolve().then(() => callback())`. Wrap to catch and
    rethrow asynchronously so a throwing callback doesn't reject the
    chain.
  ```js
  function queueMicrotask(callback) {
    if (typeof callback !== 'function') {
      throw new TypeError('queueMicrotask: callback must be a function');
    }
    Promise.resolve().then(() => { callback(); });
  }
  ```
- **`Promise.withResolvers` polyfill:**
  - If `typeof Promise.withResolvers === 'function'`, skip. Otherwise:
    ```js
    Promise.withResolvers = function withResolvers() {
      let resolve, reject;
      const promise = new Promise((res, rej) => { resolve = res; reject = rej; });
      return { promise, resolve, reject };
    };
    ```
  - The Step 1 baseline probe determines whether the polyfill ships or
    is omitted. If omitted, the file still gets a one-line comment
    documenting the Boa-builtin check, so Step 10's spec section can
    cite the source of truth.
- Install on `globalThis` with defineProperty for `structuredClone` and
  `queueMicrotask`.

**Tests:**
- `runtime/clone-shim.test.js`:
  - `structuredClone({ a: 1, b: [2, 3] })` returns deep copy with no
    shared references.
  - Circular references: `const o = {}; o.self = o; structuredClone(o)`
    produces a clone whose `.self === cloneItself` (preserves the cycle).
  - Round-trips for `Date`, `RegExp`, `Map`, `Set`, typed array,
    `Error`.
  - Throws `DataCloneError`-named error on function, on a value with
    `nodeType` (simulate a DOM node).
  - `queueMicrotask(fn)` runs `fn` after `await Promise.resolve()`.
  - `queueMicrotask(123)` throws `TypeError`.
  - `Promise.withResolvers()` returns `{ promise, resolve, reject }`
    where `resolve(x)` settles the promise to `x`.
- `node --test runtime/clone-shim.test.js` passes.

---

### Step 9: Wire `__resetFetch__` into `cleanup()` and add `runtime/web-platform.test.js` smoke tests

**Goal:** Close the per-test reset loop, then prove the full Web Platform
surface lands under Boa by running a single smoke test that uses every
audited API at least once. This step is where the spec's Requirement 6
("`runtime/http.js` runs end-to-end under `zero test`") is observable —
we can't yet run `runtime/http.test.js` under `zero test` (that's
runtime-tests/plan.md Step 3), but the smoke test exercises the same path.

**Files:**
- `runtime/test.js`
- `runtime/web-platform.test.js` (new)

**Changes:**
- In `runtime/test.js::cleanup()`, append after the existing
  `__clearAllTimers__` block:
  ```js
  if (typeof globalThis.__resetFetch__ === 'function') {
    globalThis.__resetFetch__();
  }
  ```
  Order: scope dispose → app reset → storage clear → timer cancel → fetch
  reset → document fields. Fetch reset goes after timer cancel so any
  in-flight stubbed fetch's microtask drains first, then the global is
  restored.
- **`runtime/web-platform.test.js`** — a single-file smoke test, node:test
  format, structured as one `describe('web platform surface')` with
  one `it` per audited API:
  - `it('Headers / Request / Response stub a full http.js call')` —
    construct a fake `fetch` that returns `new Response(JSON.stringify({a:1}), { headers: { 'Content-Type': 'application/json' } })`,
    call `createHttp({ fetch: stub })`, assert the parsed body is
    `{a: 1}`. (Imports `createHttp` from `./http.js`, exercising the
    runtime module's `new Headers()` / `new Request()` call sites.)
  - `it('AbortController fires abort and propagates reason')` —
    construct, abort with a reason, observe `signal.reason`.
  - `it('AbortSignal.any fires when first input aborts')` — two
    controllers, abort the second, composite aborts.
  - `it('URL and URLSearchParams round-trip a query string')`.
  - `it('TextEncoder/TextDecoder round-trip a non-ASCII string')`.
  - `it('Blob.text() returns the constructed parts')`.
  - `it('File extends Blob and adds name')`.
  - `it('FormData append/get round-trip')`.
  - `it('structuredClone deep-copies a nested object with a cycle')`.
  - `it('queueMicrotask runs the callback after a microtask boundary')`.
  - `it('Promise.withResolvers gives external resolve/reject')`.
  - `it('fetch default rejects with the stub message')` — bare
    `globalThis.fetch('/x')` rejects; message contains
    `'zero test: globalThis.fetch is not implemented'`.

**Tests:**
- `node --test runtime/web-platform.test.js` passes.
- `node --test runtime/*.test.js` (full pre-existing runtime suite) still
  passes — `cleanup()`'s new `__resetFetch__` call is a no-op when the
  shim isn't loaded (the file under test runs in Node, where the global
  is unset and the call is feature-detected).
- Boa-side: build a temporary test inside
  `crates/zero-test-runner/tests/` that loads the full `ZERO_DOM_SHIM_BODY`
  and evaluates a short script asserting every global is now present:
  ```js
  const names = ['Headers','Request','Response','fetch','AbortController','AbortSignal','URL','URLSearchParams','TextEncoder','TextDecoder','Blob','File','FormData','structuredClone','queueMicrotask'];
  for (const n of names) if (typeof globalThis[n] === 'undefined') throw new Error('missing: ' + n);
  'ok';
  ```
  Assert the eval result is `"ok"`. Delete this Rust file at the end of
  Step 10 (its assertion is documented by the spec section instead).
- `cargo test -p zero --test examples_tests tracker_tests_pass` continues
  to pass — confirms no GC panic from the new code.

---

### Step 10: Document in `zero-framework-spec.md` and `BEST_PRACTICES.md`

**Goal:** Make the boundary explicit. The audit from Step 1 lands in the
framework spec as a closed enumeration; the user-facing testing guide
gains a short "Testing against the Web Platform" subsection. Phase 13 in
the framework spec is left intact (historical); a new §8.x subsection
documents the expanded surface.

**Files:**
- `zero-framework-spec.md`
- `BEST_PRACTICES.md`
- Cleanup: delete `crates/zero-test-runner/tests/web_platform_baseline.rs`
  and the ad-hoc Boa-eval probe from Step 9. Delete
  `issues/web-platform/audit.md` once its contents are transcribed.

**Changes:**

**`zero-framework-spec.md`** — insert a new subsection inside §8 Testing,
placed immediately after "No Browser Required" (line ~975). Title:
"Web Platform surface in `zero test`". Body:

```
### Web Platform surface in `zero test`

`zero test` ships hand-written implementations of the Web Platform APIs
that real apps and the framework's own modules reach for at test time.
Anything on this list is present on `globalThis` as soon as a test file
imports `"zero/test"`; anything not on this list is outside the test
environment's scope and surfaces as Boa's standard `ReferenceError` —
stub it in your test setup.

**Fetch API**
- `Headers` — case-insensitive, `get`/`set`/`has`/`delete`/`append`/`forEach`/iteration.
- `Request` / `Response` — constructors, `text()` / `json()`. Streaming
  bodies (`arrayBuffer()`, `blob()`) reject with a clear stub message.
- `fetch` — default rejects with: "zero test: globalThis.fetch is not
  implemented. Stub it in your test's beforeEach (or pass init.fetch to
  the call) — see runtime/http.test.js for the pattern." `cleanup()`
  restores the default after each test.
- `AbortController` / `AbortSignal` — full standard shape, including
  `AbortSignal.abort(reason)`, `AbortSignal.timeout(ms)`,
  `AbortSignal.any([...])`.

**URLs**
- `URL` — constructor, getters/setters for protocol/hostname/port/
  pathname/search/hash, `searchParams`, `toString`, `URL.canParse`.
- `URLSearchParams` — constructor from string/object/array/instance,
  `get`/`getAll`/`set`/`append`/`has`/`delete`/`sort`, iteration,
  `toString`.

**Encoding**
- `TextEncoder` / `TextDecoder` — UTF-8 only. No `encodeInto`, no
  streaming `decode`. `new TextDecoder('latin1')` throws a clear
  stub message.

**Binary data**
- `Blob` — constructor, `text()`, `arrayBuffer()`, `slice()`, `size`,
  `type`.
- `File` — extends `Blob`; adds `name`, `lastModified`.
- `FormData` — `append`/`set`/`get`/`getAll`/`has`/`delete`, iteration.
  `new FormData(htmlForm)` is not supported and throws.

**Cloning & scheduling**
- `structuredClone` — plain objects/arrays/Date/RegExp/Map/Set/Error/
  ArrayBuffer/typed arrays. Functions, DOM nodes, Promises throw a
  `DataCloneError`-shaped error.
- `queueMicrotask` — schedules via Boa's job queue, same path the timer
  host uses.
- `Promise.withResolvers` — provided by Boa 0.21 (ES2024). [If Step 1's
  probe finds it absent, replace this bullet with: "polyfilled at shim
  load time."]

**The "clear error" discipline.** Any API the shim installs but does not
implement throws an error of the form
`"zero test: <API> is not implemented. <one-sentence action the user
can take>."` This is the only mechanism that surfaces gaps as actionable
messages; everything else outside this list surfaces as
`ReferenceError`.

**Out of scope** (matches Phase 13's existing fence, restated for
completeness): streaming APIs (`ReadableStream` / `WritableStream` /
`TransformStream`), `WebSocket` / `EventSource`, Web Workers,
`IndexedDB`, `SubtleCrypto.digest`, `Notifications`, `Geolocation`,
`MediaDevices`, `WebRTC`. Reach for them inside a test and stub them
yourself; the test environment does not pretend to provide them.
```

(The bracketed conditional in the `Promise.withResolvers` bullet
resolves in Step 1 based on the baseline probe; the executor writes the
final form.)

In §13 "Key Design Decisions Summary", append one row:

```
| Web Platform shim | Hand-written, closed enumerated list, ~+1500 LOC under runtime/*-shim.js | Boa ships nothing from the Web Platform; jsdom/happy-dom is too heavy for Boa; widening the fence to "Web Platform" (not just DOM) closes the gap behind one audit pass |
```

**`BEST_PRACTICES.md`** — append to §9 Testing, after the existing
"Testing browser APIs" subsection (line ~528):

```
### Testing against the Web Platform

`zero test` ships hand-written implementations of the Web Platform APIs
real apps use at test time: `Headers` / `Request` / `Response` /
`fetch`, `AbortController` / `AbortSignal`, `URL` / `URLSearchParams`,
`TextEncoder` / `TextDecoder`, `Blob` / `File` / `FormData`,
`structuredClone`, and `queueMicrotask`. See `zero-framework-spec.md`
§8 "Web Platform surface in `zero test`" for the closed list and per-
API contracts.

`fetch` is the one intentional stub: its default implementation rejects
with a clear, actionable message. Override `globalThis.fetch` per test
and `cleanup()` restores the default automatically. The canonical
pattern (mirrored on `runtime/http.test.js::makeStubFetch`):

\`\`\`ts
import { beforeEach, afterEach, cleanup } from "zero/test";

function makeStubFetch(routes: Record<string, unknown>) {
  return async (input: RequestInfo | URL) => {
    const url = typeof input === "string" ? input : input instanceof Request ? input.url : input.toString();
    const body = routes[url];
    return new Response(JSON.stringify(body), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });
  };
}

beforeEach(() => { globalThis.fetch = makeStubFetch({ "/x": { value: 42 } }); });
afterEach(cleanup);  // restores the default-rejecting stub
\`\`\`

Anything outside the audited list surfaces as a `ReferenceError`. If
your code needs `ReadableStream`, `WebSocket`, `IndexedDB`, or any
other browser API the runner doesn't ship, stub it yourself in
`beforeEach` and restore in `afterEach`. The runner deliberately
refuses to silently mock — a missing global is meant to be visible.
```

**Final cleanup:**
- Delete `crates/zero-test-runner/tests/web_platform_baseline.rs`.
- Delete any temporary Boa-eval probe files added during Steps 4 and 9.
- Delete `issues/web-platform/audit.md` once its content lives in the
  framework spec.

**Tests:**
- `cargo test --workspace` passes.
- `node --test runtime/*.test.js` passes (full suite — pre-existing files
  plus the four new shim test files plus `runtime/web-platform.test.js`).
- `cargo test -p zero --test examples_tests` passes (no GC regressions
  from new code; the existing `tracker` example test exercises
  `AbortSignal.any` via `runtime/app.js::_composeSignals`).
- Manual: `grep -n "Web Platform" zero-framework-spec.md` shows the new
  subsection; `grep -n "Testing against the Web Platform"
  BEST_PRACTICES.md` shows the new section.
- `runtime/http.js` is now usable end-to-end under `zero test`: the next
  party that resumes `issues/runtime-tests/plan.md` Step 3 can convert
  `runtime/http.test.js` without further runtime work. (That edit lives
  in the runtime-tests plan; not in this one.)

---

## Risks and Assumptions

- **Boa-builtin overlap.** The baseline probe in Step 1 may surface that
  Boa 0.21 already ships `URL` / `URLSearchParams` / `TextEncoder` /
  `TextDecoder` / `structuredClone` / `queueMicrotask`. If any
  identifier returns `'function'` from the probe, the corresponding
  implementation step downgrades to "skip; install nothing; rely on
  builtin." The feature-detect `if (typeof globalThis.X === 'undefined')`
  install pattern means the shim is a no-op when the builtin exists, so
  the downgrade is mechanical — but record it in `audit.md` so the spec
  section in Step 10 cites the source of truth (builtin vs. shim).

- **Body-bytes round-trip in `Request` / `Response`.** The minimum
  implementation stores the body as the construction-time value and
  reconstructs in `text()` / `json()`. The runtime tests
  (`runtime/http.test.js`) only exercise string and `JSON.stringify`
  bodies. If a future user test passes a typed-array body and reads it
  back, the latin1 round-trip path in the spec may lose data. Acceptable
  for the current scope; if it bites, escalate to a UTF-8-aware path.

- **`Headers` case-insensitivity edge cases.** The spec mandates lower-
  cased keys internally; iteration yields lowercased names. Some user
  test stubs may compare header names case-sensitively — this is a
  pre-existing risk (real browsers and Node's `Headers` already
  lowercase). No change in posture from the framework's perspective.

- **`structuredClone` `Map` cloning under Boa.** The implementation
  constructs new `Map` instances during clone. Per `boa-maplock-finalizer`,
  per-call `Map` instances inside helper functions are fine; the bug
  triggers only when keyed branches of a long-lived helper accumulate
  `Map` writes across calls. The clone helper is invoked per-clone and
  releases its tracker `WeakMap` on return — low risk. Verify with the
  tracker examples test (`cargo test -p zero --test examples_tests
  tracker_tests_pass`) at the end of Step 8.

- **URL parser edge cases.** Hand-written URL parsing is famously
  finicky (IDN, percent-encoded hosts, IPv6 literals, scheme-relative
  refs). The implementation here is sized to standard `https://host/path?q#h`
  shapes plus relative-against-base for `/abs` and `./rel`. If a real
  user test trips an edge case, the path forward is a targeted fix in
  `url-shim.js`, not a rewrite — the spec accepts incremental hardening
  of these APIs as users find gaps.

- **Smoke-test recursion.** `runtime/web-platform.test.js` imports
  `./http.js` to exercise the Headers/Request/Response chain end-to-end.
  Under Node, `./http.js` resolves directly (no module-loader trickery).
  Under Boa (post-runtime-tests Step 3 conversion), the bare specifier
  `'zero/http'` resolves through the test runner's loader instead. The
  smoke file as written here stays on the relative path so it runs
  unchanged under Node today; the runtime-tests plan author switches it
  to `'zero/http'` when they convert it.

- **No engine swap, no loader change.** The plan strictly stays inside
  the shim files (`runtime/*-shim.js`), the build script
  (`crates/zero-runtime/build.rs`), and `runtime/test.js::cleanup()`. No
  changes to `ZERO_RUNTIME_EXPORTS` / `ZERO_TEST_EXPORTS`, no changes
  to `crates/zero-test-runner/src/loader.rs` or `harness.rs` (other
  than potentially deleting the temporary probe test file at the end of
  Step 10). If any step finds it can't avoid touching those crates, stop
  and revise this plan instead of bypassing the constraint.
