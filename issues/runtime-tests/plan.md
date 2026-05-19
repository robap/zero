# Plan: Convert framework-internal runtime tests to `zero test`

## Summary

Move every `runtime/*.test.js` file off `node:test` + `node:assert/strict` (and,
for the five shim test files, off `node:vm`) and onto the public `zero/test`
API. Delete the five sandbox-based shim test files outright (no Boa equivalent
exists). Delete tests that only mirror trivial shim/parser properties; rewrite
substantive tests end-to-end through public helpers (`render`, `find`,
`findAll`, `text`, `fire`, `cleanup`, `spy`, `expect`). Convert
`runtime/web-platform.test.js` as a thin smoke test that imports only from
`zero/test` and exercises Web Platform globals directly. Convert one framework
file at a time so each step leaves both runners in a known state: unconverted
files still pass under `node --test`, converted files pass under `zero test`.
The conversion is test-side only — no changes to `ZERO_RUNTIME_EXPORTS`,
`ZERO_TEST_EXPORTS`, the loader, the harness, or any build script.

## Prerequisites

None. Open questions in the spec are resolved here:

1. **Repo-root `zero test`** — Step 1 adds a minimal `zero.toml` at the
   workspace root scoping `project.root` to `runtime/`, so discovery walks
   only the framework's own JS tests (not `examples/`, `showcase/`,
   `crates/*/tests/`).
2. **Canonical replacement command** — `cargo run -p zero -- test runtime/`
   (works without installing the CLI; documented in CLAUDE.md and README.md
   alongside a note that `zero test` works once installed).
3. **`runtime/test.test.js` name** — kept as-is to minimize churn. The
   recursion is fine; a rename is optional cleanup outside this work.
4. **Discovery picks up `runtime/`** — verified in Step 1 via a dry run.
5. **Ordering of the two new units of work** — shim-test deletion folds into
   Step 1 (it removes files that can't run under Boa at all, clearing the
   slate before Steps 2–8 convert the seven framework files).
   `web-platform.test.js` rides as its own small step (Step 9) right before
   docs/verification — independent of the seven core conversions, but kept
   discrete so the converted smoke surface is a clean checkpoint.

## Steps

- [x] **Step 1: Add repo-root `zero.toml`, delete the five shim tests, verify discovery**
- [x] **Step 2: Convert `runtime/reactivity.test.js`**
- [x] **Step 3: Convert `runtime/http.test.js`**
- [x] **Step 4: Convert `runtime/template.test.js`**
- [x] **Step 5: Convert `runtime/router.test.js`**
- [x] **Step 6: Convert `runtime/app.test.js`**
- [x] **Step 7: Convert `runtime/dom-shim.test.js`**
- [x] **Step 8: Convert `runtime/test.test.js` and annotate `runtime/test.js`**
- [x] **Step 9: Convert `runtime/web-platform.test.js`**
- [x] **Step 10: Update docs and run final verification**

---

## Step Details

### Step 1: Add repo-root `zero.toml`, delete the five shim tests, verify discovery

**Goal:** Make `zero test` runnable from the workspace root so each subsequent
step can be verified end-to-end. Resolve Open Question 1 and Open Question 4
from the spec before any framework test files are touched. Remove the five
shim test files — they exercise the shim sources inside a `node:vm` sandbox
to compare them against Node's native classes, a posture that collapses
under Boa (no native to shadow). Per spec Requirement 6, they are deleted,
not ported. The repo continues to pass under `node --test` for the remaining
files (which still import `node:test`).

**Files:**
- `zero.toml` (new, repo root)
- `runtime/binary-shim.test.js` (deleted)
- `runtime/clone-shim.test.js` (deleted)
- `runtime/encoding-shim.test.js` (deleted)
- `runtime/fetch-shim.test.js` (deleted)
- `runtime/url-shim.test.js` (deleted)

**Changes:**
- Create `zero.toml` with:
  ```toml
  [project]
  root = "runtime"

  [build]
  out = "target"
  ```
  - `project.root = "runtime"` scopes discovery to `runtime/*.test.{js,ts}`
    only, sidestepping the `examples/` / `showcase/` / `crates/*/tests/`
    trees (which are nested zero projects with their own `zero.toml`).
  - `build.out = "target"` reuses the cargo build output dir (already
    gitignored, and outside `runtime/`, so the filter is a no-op — but the
    field is required by `Config`).
  - `[project] root = "runtime"` passes `validate_relative_path` (no `..`,
    no leading `/`, no `\`).
- Delete the five shim test files. Each one (a) imports `node:test`,
  `node:assert/strict`, and `node:vm`, (b) reads its shim's source text from
  disk via `fs.readFileSync`, (c) evaluates the source inside a fresh
  `node:vm` sandbox, and (d) asserts against the sandbox's `globalThis`.
  None of that translates to Boa. The shims themselves
  (`runtime/binary-shim.js`, `clone-shim.js`, `encoding-shim.js`,
  `fetch-shim.js`, `url-shim.js`) are unchanged and remain part of
  `ZERO_DOM_SHIM_BODY` (`crates/zero-runtime/build.rs`,
  `crates/zero-runtime/src/lib.rs`).
- **No `build.rs` change needed.** `crates/zero-runtime/build.rs` enumerates
  the shim *sources*, not the test files. Spec Requirement 11 permits a
  build-script change only when a test file move would otherwise affect the
  enumeration — that is not the case here.

**Tests:**
- Dry run: `cargo run -p zero -- test` from the repo root. Expected:
  discovery finds the eight remaining `runtime/*.test.js` files
  (seven framework files + `web-platform.test.js`) and the harness attempts
  to load each. The runs fail at module resolution
  (`node:test` is an unsupported bare specifier in
  `crates/zero-test-runner/src/loader.rs`), which confirms discovery works.
  This is intentional — Steps 2–9 fix each file.
- Confirm `node --test runtime/*.test.js` still passes (no remaining
  `node:test` files were touched yet aside from the five deletions).
- Confirm `cargo test --workspace` still passes (`zero.toml` only affects
  the `zero test` runtime, not Rust tests; the deleted shim tests were not
  invoked from Rust).
- `grep -rn 'node:vm' runtime/` returns nothing (the only consumers of
  `node:vm` were the five deleted files).

---

### Step 2: Convert `runtime/reactivity.test.js`

**Goal:** Smallest file, mostly public APIs. Establishes the conversion
pattern (imports, assertion translation, removal of internal-only blocks).

**Files:**
- `runtime/reactivity.test.js`

**Changes:**
- Replace the header:
  ```js
  import { describe, it } from 'node:test';
  import assert from 'node:assert/strict';
  import { signal, computed, effect, _createScope } from './reactivity.js';
  ```
  with:
  ```js
  import { describe, it, expect } from 'zero/test';
  import { signal, computed, effect } from 'zero';
  ```
  (`html` / `render` / `cleanup` not needed in this file — no DOM/storage/
  timers touched in the kept tests.)
- Mechanical assertion swaps inside the `signal`, `computed`, and `effect`
  describes:
  - `assert.equal(a, b)` → `expect(a).toBe(b)`
  - `assert.deepEqual(a, b)` → `expect(a).toEqual(b)`
  - `assert.ok(x)` → `expect(x).toBeTruthy()`
  - `assert.ok(!x)` → `expect(x).toBeFalsy()`
- **Delete** the entire `describe('scope (internal)', ...)` block. Every
  assertion in it reaches `_createScope` directly. Scope dispose /
  nested-scope dispose / onCleanup ordering / active-scope restoration are
  exercised transitively by:
  - `runtime/test.test.js` already covers "cleanup disposes scopes so
    effects no longer fire after `cleanup()`" through the public `render()`
    + `cleanup()` API.
  - The `effect.stop()` test still in this file covers the effect-level
    disposal machinery that scopes wrap.
- No `afterEach(cleanup)` needed — none of the kept tests touch document,
  storage, or timers.

**Tests:**
- `cargo run -p zero -- test runtime/reactivity.test.js` passes; pass count
  equals the kept tests (count drops by the size of the deleted scope
  block).
- `grep -n 'node:test\|node:assert\|_createScope' runtime/reactivity.test.js`
  returns nothing.

---

### Step 3: Convert `runtime/http.test.js`

**Goal:** Public-API-only file; the only friction is `assert.rejects` (async
throw assertion) which `expect().toThrow()` does not cover. Use try/catch.

**Files:**
- `runtime/http.test.js`

**Changes:**
- Replace header:
  ```js
  import { describe, it } from 'node:test';
  import assert from 'node:assert/strict';
  import { createHttp, HttpError } from './http.js';
  ```
  with:
  ```js
  import { describe, it, expect } from 'zero/test';
  import { createHttp, HttpError } from 'zero/http';
  ```
- Keep the `makeStubFetch` helper as-is (no `node:*` deps; uses globals
  `Request` / `Response` / `Headers` provided by Boa or the shim).
- Translate `assert.rejects(promiseFactory, predicate)` to:
  ```js
  let err;
  try { await client.get('http://api.test/missing'); }
  catch (e) { err = e; }
  expect(err instanceof HttpError).toBeTruthy();
  expect(err.status).toBe(404);
  expect(err.statusText).toBe('Not Found');
  expect(err.body).toEqual({ message: 'nope' });
  ```
  Apply the same shape to the "aborted caller signal" test (assert on
  `err.name === 'AbortError'`).
- Remaining `assert.equal` / `assert.deepEqual` / `assert.ok` translate
  mechanically.

**Tests:**
- `cargo run -p zero -- test runtime/http.test.js` passes; nine tests.
- `grep -n 'node:test\|node:assert' runtime/http.test.js` returns nothing.
- No internal exports referenced (`createHttp` / `HttpError` are public).

---

### Step 4: Convert `runtime/template.test.js`

**Goal:** Largest substantive file. Replace direct `_createScope` + `commit`
plumbing with `render()`, and replace parser-shape introspection
(`_template.fragment`, `_template.parts`) with DOM-observable assertions.

**Files:**
- `runtime/template.test.js`

**Changes:**
- Replace header. Drop the `import './dom-shim.js';` side-effect import — the
  loader's `"zero"` module already wires the DOM shim globals into the Boa
  context. New imports:
  ```js
  import { describe, it, expect, render, find, findAll, text, fire, cleanup, afterEach } from 'zero/test';
  import { html, commit, ref, each, signal, effect } from 'zero';
  ```
  (`_createScope` is no longer imported.)
- For every describe that touches document/scope state, add
  `afterEach(cleanup)` inside the describe.
- **`commit() — attr parts` / `commit() — node parts`:** the existing
  pattern is:
  ```js
  const scope = _createScope();
  scope.run(() => commit(html`<div class=${c}></div>`, container));
  ...
  scope.dispose();
  ```
  Rewrite as:
  ```js
  const el = render(html`<div class=${c}></div>`);
  const div = find(el, 'div');
  ```
  The container is internal to `render`; `find` queries it. `cleanup()` in
  `afterEach` disposes the scope.
- **`html` tagged template / static fragment shape tests:** replace
  `r._template.fragment.childNodes[0]` introspection with `render(...)` +
  `find`/`text`. Concrete mappings:
  - "static html with no placeholders: fragment has correct structure" →
    render `<div>Hello</div>`, assert `find(el, 'div')` exists, `text(div)
    === 'Hello'`.
  - "static attribute value is preserved" → render `<a href="/bar">bar</a>`,
    assert `find(el, 'a').getAttribute('href') === '/bar'` and
    `text(a) === 'bar'`.
  - "boolean static attribute is set to empty string" → render
    `<input disabled>`, assert `find(el, 'input').getAttribute('disabled')
    === ''`.
  - "creates `<svg>` in the SVG namespace" / "descendants of `<svg>`" /
    "returns to HTML namespace after `</svg>`" → keep; render + `find` +
    read `.namespaceURI` (public DOM property).
- **`html` tagged template / parts-shape tests:** delete the four tests
  that assert on `r._template.parts.length`, `parts[0].type`,
  `parts[0].name`, `part.event`, `part.modifiers`. Each has a behavioral
  counterpart already in the file:
  - "node part" → covered by every `${value}` rendering in `commit() —
    node parts`.
  - "attr part" → covered by every `class=${...}` test in `commit() — attr
    parts`.
  - "event part: @click.prevent.stop parsed" → covered by `commit() —
    event bindings`.
  - "ref part" → covered by the `ref()` describe.
- **`html` cache-hit test ("same call site returns same `_template`
  reference"):** delete. The cache key is an internal optimization, not
  observable through the public API. Reactive-update tests (e.g. signal
  changes mutating in-place text/attr) already prove the template path is
  reused.
- **`commit() — event bindings`:** the existing `makeEvent(type, extra)`
  helper builds an ad-hoc plain-object event. Replace with `fire(el, type,
  data)`. Concretely:
  - `btn.dispatchEvent(makeEvent('click'))` → `fire(find(el, 'button'),
    'click')`.
  - For modifier tests (`.prevent`, `.stop`, `.once`, `.enter`,
    `.enter.prevent`), assert behavior end-to-end rather than poking event
    flags:
    - `.once`: fire twice, assert handler ran once.
    - `.stop`: render parent+button with parent handler; fire button
      click, assert parent handler did **not** run.
    - `.enter`: fire `keydown` with `{ key: 'Enter' }` then with
      `{ key: 'a' }`; assert handler ran exactly once.
    - `.enter.prevent`: combine the two — fire `keydown` with
      `key: 'Enter'` and assert the handler ran; `preventDefault` behavior
      is not observable through `fire()` (which discards
      `dispatchEvent`'s return value), so delete the explicit
      prevented-flag assertion. Coverage of `.prevent` survives in the
      `.enter.prevent` test (handler runs) and in `.stop` (modifier-chain
      parsing reaches the handler).
- **Scope-dispose tests** ("scope dispose stops signal updates to DOM",
  "scope dispose removes event listener"): rewrite as `render(...)` +
  signal set / `fire(...)` *before* `cleanup()`, then `cleanup()` + the
  same mutation, asserting no DOM change after cleanup. Move these out of
  the current describe's `afterEach(cleanup)` pattern (so cleanup is
  invoked explicitly in the test body), or keep them inside and structure
  as:
  ```js
  it('cleanup stops signal updates to DOM', () => {
    const c = signal('a');
    const el = render(html`<div class=${c}></div>`);
    const div = find(el, 'div');
    cleanup();
    c.set('b');
    expect(div.getAttribute('class')).toBe('a');
  });
  ```
  The afterEach(cleanup) call after the test body is a safe no-op (the
  tracker is already drained).
- **`ref()` describe:** straightforward — `render` instead of `commit`,
  query with `find`, assert `r.el != null` and `.tagName`. The "ref
  cleared to null on scope dispose" test uses cleanup as above.
- **`each()` describe:** the keyed-list tests rely on identity of `<li>`
  child nodes across updates. Use `render(...)` to get the container;
  query `findAll(el, 'li')` before and after `items.set(...)`; compare
  element identity (`expect(after[0]).toBe(before[0])`). For the "per-row
  scope is disposed when its key disappears" test, the assertion is on a
  captured array of disposed ids — keep the same structure, just swap
  assertions.
- **`each() — duplicate keys throw`:** convert from
  `assert.throws(() => scope.run(...), /duplicate key '1' in row 1/)` to
  ```js
  expect(() => render(html`<ul>${each(items, ...)}</ul>`)).toThrow(
    "duplicate key '1' in row 1",
  );
  ```
  (`render` will surface the underlying throw since it runs commit
  inside a scope synchronously.)

**Tests:**
- `cargo run -p zero -- test runtime/template.test.js` passes.
- `grep -n 'node:test\|node:assert\|_createScope\|_template\|_values' runtime/template.test.js`
  returns nothing.
- No new exports added; `commit`, `each`, `ref`, `html` already public.

---

### Step 5: Convert `runtime/router.test.js`

**Goal:** Replace internal-function direct tests with end-to-end navigation
tests through `App` / `navigate` / `route()`. Remove all shim-internal
reach-ins (`document.childNodes.length = 0`, `window.history._entries =
[...]`, `window._listeners.clear()`, etc.).

**Files:**
- `runtime/router.test.js`

**Changes:**
- Replace header. New imports:
  ```js
  import { describe, it, expect, beforeEach, afterEach, cleanup, spy } from 'zero/test';
  import { App, navigate, back, forward, route, html, effect } from 'zero';
  ```
  Remove all `_normalizePath`, `_parseQuery`, `_parsePathAndQuery`,
  `_compileRoutePattern`, `_matchAgainst`, `_matchRoutes`, `_joinPaths`,
  `_setCurrentApp`, raw `window`/`document` imports.
- Replace `freshMount` + `resetEnv` + `freshApp` with two public-API
  helpers installed at module top level (so they are not closures over a
  per-test variable — important for Boa GC compatibility per
  `[[boa-maplock-finalizer]]`: keep keyed / code-path-variant branches in
  their own functions, do not nest them inside `it` bodies):
  ```js
  function freshMount() {
    cleanup();
    window.history.pushState(null, '', '/');
    const mount = document.createElement('div');
    mount.setAttribute('id', 'app');
    document.body.appendChild(mount);
    return mount;
  }

  function freshApp() {
    freshMount();
    return new App()
      .route('/', () => html`<span>home</span>`)
      .route('/about', () => html`<span>about</span>`)
      .run('#app');
  }
  ```
  No `_setCurrentApp(null)` calls — `cleanup()` already does that.
  `document` and `window` are reachable as globals from the shim, so no
  imports needed.
- **Delete** these describes outright (every test reaches an `_`-prefixed
  router export with no behavioral payload beyond what `App.route` /
  `navigate` already exercise):
  - `describe('_normalizePath', ...)` — trailing-slash behavior verified
    transitively by every `app.match('/foo/')` / `navigate('/foo/')` test
    that lands the user at `/foo`.
  - `describe('_matchRoutes', ...)` — replace with two new end-to-end tests
    in `describe('navigate / route()')`: (a) param-decoded path —
    `navigate('/users/%C3%A9')` then `expect(route().params).toEqual({ id:
    'é' })`; (b) query parsed — `navigate('/users/42?tab=posts')` then
    `expect(route().query).toEqual({ tab: 'posts' })`. First-match-wins
    and wildcard-fallback are already covered by `runtime/app.test.js`
    (`match()` tests).
  - `describe('_parsePathAndQuery', ...)` — covered by the same end-to-end
    tests above plus `_parseQuery` deletion below.
  - `describe('_compileRoutePattern / _matchAgainst', ...)` — same, plus
    the multi-param coverage gets a new end-to-end test:
    `app.route('/users/:id/posts/:postId', spy(...))`,
    `navigate('/users/7/posts/99')`, assert the spy's last call was made
    with `{ params: { id: '7', postId: '99' }, ... }` (or use
    `route().params`).
  - `describe('_joinPaths', ...)` — covered by `describe('nested-route
    flattening')` further down (which exercises real joining via
    `App.route` with `children:`).
  - `describe('_parseQuery', ...)` — covered by the query end-to-end test
    above.
- **Keep and convert:**
  - `describe('navigate / back / forward / route()')` — straight
    assertion translation. For the "navigate outside running app throws"
    / "route() outside running app throws" tests: call `cleanup()` (which
    clears `_currentApp` via `_setCurrentApp(null)` internally) then
    `expect(() => navigate('/about')).toThrow('no app is running')`.
  - `describe('nested-route flattening')` — rewrite each test end-to-end
    by `app.run('#app')`, navigating to each path, and asserting the
    rendered DOM contains the expected parent + child. Avoid the
    `app._routes` private. Concrete shape:
    ```js
    it('one-level: child mounts inside parent outlet', async () => {
      freshMount();
      const Parent = ({ outlet }) => html`<div><span>parent</span>${outlet}</div>`;
      const A = () => html`<div>analytics</div>`;
      const app = new App().route('/dashboard', Parent, {
        children: [{ path: '/analytics', load: A }],
      });
      app.run('#app');
      navigate('/dashboard/analytics');
      await Promise.resolve(); await Promise.resolve();
      const mount = find(document, '#app');
      expect(find(mount, 'span')).toBeTruthy();
      expect(text(mount)).toContain('analytics');
    });
    ```
    Cover all five existing nested-flattening cases (one-level, two-level,
    parent-reuse, child loader correctness, plain top-level).
  - `describe('route-scoped fetch')` — straight translation. Replace
    `window.history.pushState(...)` + `window.dispatchEvent({ type:
    'popstate' })` with `navigate(...)` (single public call, dispatches
    the same effect). Stub `globalThis.fetch` exactly as today; restore
    in a `finally` block. Use `try/catch` + field assertions for the
    AbortError-rejection tests instead of `assert.rejects`.
- **Two route() calls return distinct objects with same underlying values:**
  the original uses `assert.notEqual(r1, r2)` (negation). `expect` has no
  `.not`. Rewrite as `expect(r1 === r2).toBeFalsy()` plus the existing
  `.toBe(r2.path)` / `.toEqual(r2.params)` checks.

**Tests:**
- `cargo run -p zero -- test runtime/router.test.js` passes.
- `grep -nE '_normalizePath|_parseQuery|_parsePathAndQuery|_compileRoutePattern|_matchAgainst|_matchRoutes|_joinPaths|_setCurrentApp|_listeners|history._entries|history._index|node:test|node:assert' runtime/router.test.js`
  returns nothing.
- Behavioral coverage: every router behavior previously asserted at the
  internal-function level is still asserted at the navigation level (param
  decoding, query parsing, trailing-slash normalize, hash drop, wildcard
  fallback, nested flattening at all three depths, abort signal
  composition).

---

### Step 6: Convert `runtime/app.test.js`

**Goal:** Translate the largest behavioral test file. Mostly mechanical —
the file already exercises `App` end-to-end. Replace shim reach-ins, ad-hoc
event objects, and direct `_setCurrentApp` calls.

**Files:**
- `runtime/app.test.js`

**Changes:**
- New imports:
  ```js
  import { describe, it, expect, beforeEach, afterEach, cleanup, fire, render, find, spy } from 'zero/test';
  import { App, inject, signal, html } from 'zero';
  ```
  Drop the dynamic `await import('./reactivity.js')` calls — `signal` is
  now a top-level import.
- `freshMount` / `resetEnv` collapse into the same helper as Step 5
  (`freshMount()` calls `cleanup()` + `window.history.pushState(null, '',
  '/')` + appends a `<div id="app">` to `document.body`). Define at module
  top level (Boa GC safety per `[[boa-maplock-finalizer]]`). Use it in
  `beforeEach`.
- **`describe('App (Step 3: scaffolding)')`:**
  - "inject outside running app throws" — replace `_setCurrentApp(null)`
    with `cleanup()` then `expect(() => inject('anything')).toThrow('no
    app is running')`.
  - "inject with `_setCurrentApp` resolves registered value" — rewrite
    end-to-end:
    ```js
    it('inject inside a running route resolves registered value', async () => {
      freshMount();
      let observed;
      new App()
        .state('color', 'blue')
        .route('/', () => { observed = inject('color'); return html`<div/>`; })
        .run('#app');
      await Promise.resolve();
      expect(observed).toBe('blue');
    });
    ```
  - "inject with unknown key throws" — similar end-to-end rewrite; throw
    surfaces through the app's error handler. Simpler alternative: use
    `render(...)` from `zero/test` which installs a stub current-app:
    ```js
    expect(() => render(html`<span>${() => inject('nope')}</span>`)).toThrow('is not registered');
    ```
    Pick whichever form is cleaner once the test is being written; both
    are public-API.
- **Click-interception tests:** the existing pattern constructs ad-hoc
  event-like plain objects (`{ type: 'click', target: anchor, button: 0,
  preventDefault() {...} }`) and calls `document.dispatchEvent(...)`.
  Replace with `fire(document, 'click', { target: anchor, button: 0 })`:
  - `fire` builds a real `MouseEvent` with `bubbles: true, cancelable:
    true`, then `Object.assign`s `data` (so `target` overrides the
    naturally-set target). The shim's `dispatchEvent` honors `target` if
    set on the event object (spec Constraints: "the shim's MouseEvent
    permits assignment of `target` (the click-interception tests rely on
    this)").
  - For `preventDefault` observation: dispatch through `fire` and rely on
    `expect(window.location.pathname).toBe(...)` to prove navigation did
    or did not happen. The current tests already make that assertion as
    the primary check; the `prevented` boolean was a redundant guard.
    Drop the explicit `prevented` assertion (or, where the test is solely
    about `!prevented`, assert that `window.location.pathname` stayed
    unchanged).
  - For metaKey / button:1: pass `{ target: anchor, button: 1 }` or
    `{ target: anchor, button: 0, metaKey: true }`.
- **`window.dispatchEvent({ type: 'popstate' })`** patterns: most are
  paired with `window.history.pushState(null, '', '/foo')`. Replace the
  pair with `navigate('/foo')` (the public API does both). The
  "abandoned scope effects do not fire" and "supersede" tests need rapid
  navigation without awaiting; `navigate` schedules through the same
  path and the existing `await Promise.resolve()` cadence still works.
  - For the "popstate re-renders" test that explicitly verifies the
    popstate path: keep one such test and use `fire(window, 'popstate')`
    after a `window.history.pushState`. `fire` constructs `Event` for
    any non-`key*` / non-`click`/`dblclick`/`mouse*` type, so popstate
    becomes a generic Event. The `window` object exposes
    `dispatchEvent` in the shim.
- **`console.error` capture** in "no error registered + throw" test: use
  `spy()` from zero/test:
  ```js
  const origErr = console.error;
  const errSpy = spy();
  console.error = errSpy;
  try { ... expect(errSpy).toHaveBeenCalledTimes(1); }
  finally { console.error = origErr; }
  ```
- **`receivedState.foo` strict-equal-to signal:** `assert.strictEqual(a, b)`
  → `expect(a).toBe(b)` (already strict).
- Mechanical assertion swaps across the entire file.

**Tests:**
- `cargo run -p zero -- test runtime/app.test.js` passes.
- `grep -nE '_setCurrentApp|_getCurrentApp|_listeners|history\._entries|node:test|node:assert' runtime/app.test.js`
  returns nothing.
- No regressions in the click-interception suite: the seven existing
  cases (plain anchor, anchor>span, target="_blank", download,
  data-external, external href, metaKey, button:1) are all covered.

---

### Step 7: Convert `runtime/dom-shim.test.js`

**Goal:** Delete the bulk of the file (it is mostly property/method
mirrors). Keep a small, behavior-focused remainder that exercises features
end-to-end through `render` + `find` + `fire`, plus public globals
(`window.location`, `window.history`, `localStorage`, `sessionStorage`,
`document.title`, `document.activeElement`).

**Files:**
- `runtime/dom-shim.test.js`

**Changes:**
- New imports:
  ```js
  import { describe, it, expect, beforeEach, afterEach, cleanup, render, find, fire, spy } from 'zero/test';
  import { html } from 'zero';
  ```
  No `Event` / `CustomEvent` / `KeyboardEvent` / `MouseEvent` imports —
  these are not in `ZERO_TEST_EXPORTS` and `fire()` is the public path.
  No `document` / `window` / `localStorage` / `sessionStorage` imports
  either — they are globals after the shim installs.
- **Delete** (trivial mirror — covered transitively by every render-test
  in the suite):
  - "createElement returns element with uppercase tagName"
  - "setAttribute / getAttribute / hasAttribute / removeAttribute"
  - "createTextNode and createComment carry data"
  - "appendChild wires parentNode"
  - "insertBefore places child at correct index"
  - "cloneNode(deep) copies children but not listeners" (clone is
    internal template-cache machinery; covered by every reused template
    test).
  - "dispatchEvent fires listeners and respects once" (covered by `fire`
    behavior in `runtime/test.test.js` selector + event tests).
  - `describe('event constructors')` — every test that constructs `new
    Event(...)` etc. directly. The capture/bubble/stop behaviors are
    rewritten below via `fire` + parent listener observation.
  - "classList.replace returns boolean", "classList.length", "classList
    add returns whatever" mirrors; "dataset.fooBar round-trip" mirror;
    "delete el.dataset.x removes attr" mirror; "style.setProperty
    supports CSS custom properties" mirror; "setAttribute('style', ...)
    populates style map" mirror; "el.value round-trips" mirror;
    "el.checked toggles attribute" mirror; "el.className mirrors class
    attribute" mirror; "el.htmlFor mirrors for attribute" mirror.
  - `describe('auxiliary globals')` — `matchMedia` property mirror,
    `navigator.userAgent override` mirror, `crypto.randomUUID` /
    `crypto.getRandomValues` mirrors, `IntersectionObserver.observations`
    (private `.observations`), `MutationObserver.takeRecords`,
    `getComputedStyle` empty-string mirror. All are property-existence
    tests with no failing behavior.
  - "querySelector('#x')" / "querySelectorAll('a')" / "closest('a')" —
    the selector grammar suite in `runtime/test.test.js` already
    exercises querySelector/All + closest end-to-end across class/id/
    attr/tag selectors. The dom-shim copies are redundant.
- **Keep + rewrite** (behavior worth direct coverage):
  1. **Event bubbling & propagation through `fire`** (replace the four
     existing event-constructor tests with three `fire`-based tests):
     ```js
     it('event bubbles: parent listener fires when child fires bubbling event', () => {
       const handler = spy();
       const el = render(html`<div @x=${handler}><span></span></div>`);
       fire(find(el, 'span'), 'x');
       expect(handler.callCount).toBe(1);
     });

     it('stopPropagation halts bubble', () => {
       const grand = spy();
       const parent = (e) => e.stopPropagation();
       const el = render(html`<div @x=${grand}><div @x=${parent}><span/></div></div>`);
       fire(find(el, 'span'), 'x');
       expect(grand.callCount).toBe(0);
     });

     it('addEventListener once fires only once', () => {
       const handler = spy();
       const el = render(html`<button></button>`);
       const btn = find(el, 'button');
       btn.addEventListener('click', handler, { once: true });
       fire(btn, 'click');
       fire(btn, 'click');
       expect(handler).toHaveBeenCalledTimes(1);
     });
     ```
     Capture-phase ordering is not directly testable through `fire` (the
     shim still honors `capture: true` listeners, but observing order
     with a spy works) — add one capture test using
     `addEventListener(..., { capture: true })` directly on rendered
     elements.
  2. **`window.history` integration** — keep four behaviors, rewritten
     without `_entries` / `_index` access:
     - "pushState advances length and updates location":
       ```js
       it('pushState advances length and updates location', () => {
         const lengthBefore = window.history.length;
         window.history.pushState(null, '', '/about?x=1');
         expect(window.history.length).toBe(lengthBefore + 1);
         expect(window.location.pathname).toBe('/about');
         expect(window.location.search).toBe('?x=1');
       });
       ```
       Use `beforeEach(() => { window.history.pushState(null, '', '/'); })`
       to reset URL state without touching `_entries`.
     - "replaceState does not advance length and rewrites top entry":
       analogous, `length` unchanged across replaceState.
     - "back() after pushes dispatches popstate and rolls location back":
       attach a `spy()` listener via `window.addEventListener('popstate',
       spy)` to observe — `back()` itself dispatches popstate inside the
       shim.
     - "pushState after back() truncates forward history": same length-
       counting structure as today, without `_entries` access.
  3. **Web storage** — keep five tests; replace `assert.equal` with
     `expect`. No internal access; localStorage/sessionStorage are
     public globals.
  4. **Document additions** — keep:
     - "document.getElementById finds element appended under body" —
       straight translation.
     - "focus() sets activeElement and blur() clears it" — translation.
     - "focusing a second element dispatches blur on the first" —
       translation; use a `spy()` for the blur handler.
     - "document.title round-trip" — translation.
  5. **`document.addEventListener` / `dispatchEvent` /
     `removeEventListener` / `once`** — keep, translate, use
     `fire(document, 'click')` instead of plain objects.
- The resulting file should be ~120–160 lines covering only behaviors
  that have no incidental coverage elsewhere.

**Tests:**
- `cargo run -p zero -- test runtime/dom-shim.test.js` passes.
- `grep -nE 'new Event|new CustomEvent|new KeyboardEvent|new MouseEvent|_listeners|history\._entries|node:test|node:assert' runtime/dom-shim.test.js`
  returns nothing.
- Direct shim coverage (event bubbling, history, storage, focus, title,
  document listeners) remains; trivial mirrors are gone.

---

### Step 8: Convert `runtime/test.test.js` and annotate `runtime/test.js`

**Goal:** Convert the test-API self-test file (recursion is fine per spec
Requirement 5), drop the `__getTestTree__` / `__resetTestTree__` direct
assertions (covered by Rust harness tests), and leave a comment in
`runtime/test.js` documenting why those two ABI exports exist.

**Files:**
- `runtime/test.test.js`
- `runtime/test.js`

**Changes:**

**`runtime/test.test.js`:**
- New imports:
  ```js
  import {
    describe, it, expect, beforeEach, afterEach,
    beforeAll, afterAll, render, find, findAll,
    text, fire, cleanup, spy,
  } from 'zero/test';
  import { signal, html, inject } from 'zero';
  ```
  Drop the two-aliased-import pattern (`describe as zeroDescribe, ...`)
  — there is only one set of test functions now.
- **Delete** the top-level `beforeEach` block that calls
  `__resetTestTree__()` and reaches into `document._listeners` /
  `window._listeners` / `document.childNodes`. Each file gets a fresh
  Boa context, so initial state is clean. Within the file, individual
  describes that need per-test DOM/storage/timer reset use
  `afterEach(cleanup)` (matching the existing `selector grammar`
  pattern). Per spec Requirement 8: "Drop the reset entirely."
- **Delete** the `describe('test tree structure', ...)` block. The
  JS↔Rust ABI of `__getTestTree__` / `__resetTestTree__` is covered by
  integration tests in `crates/zero-test-runner/src/harness.rs` (tree
  walking, hook ordering, async handling, throw semantics). Per spec
  Requirement 4 this block goes away.
- **`describe('expect matchers')`:** convert the matcher self-tests to
  the try/catch + captured-error pattern that the spec calls out
  (Requirement 5). Concrete shapes:
  ```js
  it('toBe throws on mismatch with actual and expected in message', () => {
    let caught;
    try { expect(1).toBe(2); } catch (e) { caught = e; }
    expect(caught).toBeTruthy();
    expect(caught.message).toContain('1');
    expect(caught.message).toContain('2');
  });

  it('toBe passes on strict equality', () => {
    expect(1).toBe(1); // any throw fails the test
  });
  ```
  Repeat for `toEqual`, `toContain`, `toThrow`, `toMatchSnapshot`, and
  the signal-shaped-`toEqual` test.
- **"matcher error carries `_userFrame` pointing at the assertion call
  site"** — keep, translate. Concretely:
  ```js
  it('matcher error carries _userFrame pointing at the test file', () => {
    let caught;
    try { expect(1).toBe(2); } catch (e) { caught = e; }
    expect(caught._userFrame).toBeTruthy();
    expect(/test\.test\.js:\d+:\d+$/.test(caught._userFrame)).toBeTruthy();
  });
  ```
  The basename-filter in `runtime/test.js::_FRAMEWORK_INTERNAL_BASENAMES`
  includes `test.js` but **not** `test.test.js`, so the frame is
  correctly picked. Spec Constraint "Verify `_userFrame` discovery" is
  exercised here; Step 10 re-verifies under a non-`test`-named file
  (router).
- **`describe('DOM helpers')` / `describe('selector grammar')` /
  `describe('spy primitive')` / `describe('spy matchers')`:**
  mechanical assertion translation. Keep the `afterEach(cleanup)` in
  selector grammar; add it where missing in DOM helpers (the
  cleanup-disposes-scopes / cleanup-clears-storage / cleanup-resets-title
  tests are themselves *about* cleanup — call it explicitly inside the
  test body, then let afterEach noop). For "cancels pending timers via
  `__clearAllTimers__` when present" test: keep — it asserts a behavior
  of `cleanup()` and uses only `globalThis.__clearAllTimers__`, no
  forbidden internal.
- **`describe('selector grammar')`'s** `assert.throws(() => find(el, ''),
  /empty selector/)` translates to
  `expect(() => find(el, '')).toThrow('empty selector')`.

**`runtime/test.js`:**
- Add a 4-to-6-line comment block immediately above `__getTestTree__` (or
  immediately above both functions if they stay adjacent) explaining
  that these are the JS-side ABI consumed by
  `crates/zero-test-runner/src/harness.rs`, that they are not part of
  the public `zero/test` API, and that the Rust harness's integration
  tests are the authoritative coverage of this contract. Exact phrasing
  left to the executor; example:
  ```js
  // ---------------------------------------------------------------------------
  // JS↔Rust ABI. The Rust harness in `crates/zero-test-runner/src/harness.rs`
  // calls `__getTestTree__()` to walk the test tree built up during module
  // evaluation, and `__resetTestTree__()` to rebuild it. Neither is part of
  // the public `zero/test` API. The contract is covered by the harness's own
  // Rust integration tests; do not assert on these from JS test files.
  // ---------------------------------------------------------------------------
  ```
  No code change to either function.

**Tests:**
- `cargo run -p zero -- test runtime/test.test.js` passes.
- `grep -nE '__getTestTree__|__resetTestTree__|document\._listeners|window\._listeners|node:test|node:assert' runtime/test.test.js`
  returns nothing. (`runtime/test.js` retains both as defined exports —
  that is the JS-side ABI.)
- File still self-tests the matcher API; the recursive shape works
  because the runner is already proven by Steps 2–7's converted files.

---

### Step 9: Convert `runtime/web-platform.test.js`

**Goal:** Convert the Web Platform surface smoke file (one `it` per
audited API). Imports only from `zero/test` (per spec Requirement 7 and
the matching Constraint). Constructs `new Response(...)` directly rather
than routing through `createHttp` from `./http.js`, so the test is purely
about Web Platform globals and does not import any `runtime/` module.

**Files:**
- `runtime/web-platform.test.js`

**Changes:**
- Replace header:
  ```js
  import { describe, it } from 'node:test';
  import assert from 'node:assert/strict';
  import { createHttp } from './http.js';
  ```
  with:
  ```js
  import { describe, it, expect } from 'zero/test';
  ```
  Nothing else — every API under test (`Headers`, `Request`, `Response`,
  `AbortController`, `AbortSignal`, `URL`, `URLSearchParams`,
  `TextEncoder`, `TextDecoder`, `Blob`, `File`, `FormData`,
  `structuredClone`, `queueMicrotask`, `Promise.withResolvers`) is a
  global provided by `ZERO_DOM_SHIM_BODY` at module evaluation time.
- Mechanical assertion swaps (`assert.equal` → `expect(...).toBe(...)`,
  `assert.ok` → `expect(...).toBeTruthy()`, `assert.notEqual(a, b)` →
  `expect(a === b).toBeFalsy()`).
- **First test — "Headers / Request / Response stub a full http.js
  call":** rewrite to drop the `createHttp` dependency. Construct
  `new Response(...)` directly and assert on its body and headers:
  ```js
  it('Headers / Request / Response work directly', async () => {
    const headers = new Headers({ 'Content-Type': 'application/json' });
    expect(headers.get('content-type')).toBe('application/json');

    const req = new Request('http://api.test/x', { method: 'POST' });
    expect(req.method).toBe('POST');
    expect(req.url).toBe('http://api.test/x');

    const res = new Response(JSON.stringify({ a: 1 }), {
      status: 200,
      headers: { 'Content-Type': 'application/json' },
    });
    expect(res.status).toBe(200);
    expect(await res.text()).toBe('{"a":1}');
  });
  ```
  This preserves the "Headers / Request / Response are wired up" smoke
  intent while keeping the file's imports limited to `zero/test`.
- The other ten tests (AbortController, AbortSignal.any, URL/
  URLSearchParams, TextEncoder/TextDecoder, Blob, File, FormData,
  structuredClone, queueMicrotask, Promise.withResolvers) translate
  one-for-one with mechanical assertion swaps.

**Tests:**
- `cargo run -p zero -- test runtime/web-platform.test.js` passes;
  eleven tests.
- `grep -nE 'node:test|node:assert|\./http\.js|\./.*\.js' runtime/web-platform.test.js`
  returns nothing.
- No new exports referenced; no relative imports.

---

### Step 10: Update docs and run final verification

**Goal:** Remove every remaining reference to `node --test` against
runtime tests from human-facing docs; verify the conversion is complete;
smoke-test `_userFrame` from a non-`test.test.js` test file.

**Files:**
- `CLAUDE.md`
- `README.md`

**Changes:**

**`CLAUDE.md`:**
- In the commands block, replace:
  ```
  # Run all JS runtime tests (framework-internal, unchanged)
  node --test runtime/*.test.js

  # Run a single JS test file
  node --test runtime/app.test.js

  # Run JS tests matching a name pattern
  node --test --test-name-pattern="querySelector" runtime/dom-shim.test.js
  ```
  with:
  ```
  # Run all JS runtime tests (zero test, from repo root)
  cargo run -p zero -- test runtime/

  # Run a single JS test file
  cargo run -p zero -- test runtime/app.test.js

  # Once the CLI is installed (`cargo install --path crates/zero --locked`),
  # the same commands run as `zero test runtime/` etc.
  ```
- In the JS/TS code-style section, update the prose: change "The `node
  --test runtime/*.test.js` command above (framework-internal tests) is
  unchanged; user-level tests run with `zero test`." to a sentence that
  no longer distinguishes framework-internal from user tests (both now
  run with `zero test`).

**`README.md`:**
- Prereqs block: remove the line "Node.js — only needed to run the
  runtime's own test suite (`node --test`); not required for building
  or running apps." Node is no longer required.
- "Running the runtime tests" section: replace
  ```
  node --test runtime/*.test.js
  ```
  with
  ```
  cargo run -p zero -- test runtime/
  ```
  Update the surrounding paragraph: "The framework's own JavaScript
  runtime tests run under `zero test`, the same runner user apps use."

**Final verification (no code changes):**
- `grep -rn "node:test\|node:assert\|node:vm" runtime/` returns nothing.
- `grep -rn "node --test" .` returns only historical hits inside
  `issues/` (intentional; spec history is not rewritten).
- `cargo run -p zero -- test runtime/` from the repo root: every test
  file (eight remaining `.test.js` files) runs; pass count matches the
  converted counts; no failures.
- `cargo test --workspace`: still passes (Rust side unchanged).
- `node --test runtime/*.test.js`: now fails (every remaining file
  imports `zero/test`, which Node cannot resolve). Spec acceptance —
  Node is no longer required. Confirm the failure is at module
  resolution, not a partial conversion.
- **`_userFrame` smoke test** (spec Constraint "Verify `_userFrame`
  discovery"): deliberately edit one `it` in `runtime/router.test.js`
  to `expect(1).toBe(2);`. Run
  `cargo run -p zero -- test runtime/router.test.js`. Confirm the
  reporter prints a frame whose path ends
  `runtime/router.test.js:<line>:<col>`, not `runtime/router.js` or
  `runtime/test.js`. Revert the edit.
- **Performance measurement** (spec Constraint "Performance"): note the
  wall-clock for `cargo run -p zero -- test runtime/` vs.
  `node --test runtime/*.test.js` against `main` before the conversion.
  Record the delta in the PR description; if the Boa run takes more
  than ~10× the Node run, file a follow-up against the test runner
  (per spec — accepted trade-off, but worth a heads-up).

**Tests:**
- The verification commands above all succeed.
- No new exports in `ZERO_RUNTIME_EXPORTS` / `ZERO_TEST_EXPORTS` /
  loader cases; verified via `git diff main -- crates/zero-runtime/src/lib.rs
  crates/zero-test-runner/src/loader.rs crates/zero-test-runner/src/harness.rs`.
- No build-script changes (`crates/zero-runtime/build.rs` enumerates
  shim sources, not test files).
- Status at end of plan: every remaining `runtime/*.test.js` file runs
  under `zero test`; the framework dogfoods its own runner; Node is no
  longer a contributor prerequisite.

---

## Risks and Assumptions

- **`zero test` with `project.root = "runtime"`.** Assumes the test
  subcommand does not require an `index.html` or any other file beyond
  `zero.toml`. The implementation (`crates/zero/src/cmd/test.rs::run`)
  only reads config, joins paths, walks discovery, and runs the
  harness; no `index.html` access path. If a future hardening adds that
  requirement, Step 1 needs a placeholder `runtime/index.html` (cheap
  fix).

- **Boa context per file is the per-test reset story.** Cross-file
  state isolation (history, storage, document) comes from the fresh
  Boa context per test file. Within a file, `cleanup()` handles DOM /
  storage / timers but **not** `window.history`. The plan addresses
  this with explicit `window.history.pushState(null, '', '/')` in
  `freshMount()`. If a future test interleaves navigations and reset
  state in a more elaborate way, it may need additional explicit
  history pushes — not a blocker.

- **Boa GC compatibility.** Spec Constraint
  "Boa GC compatibility" references `[[boa-maplock-finalizer]]`: keep
  keyed / code-path-variant branches in their own functions to avoid a
  process-exit panic in the Boa MapLock finalizer. Helpers added during
  conversion (`freshMount`, `freshApp`, matcher-failure capture blocks)
  are defined at module top level rather than nested inside `it` bodies
  or closures over per-test variables. The failure mode is a
  process-exit panic *after* the suite completes — not a test failure —
  so it is easy to miss in CI; verify by running each converted file
  individually and observing a clean exit code (Steps 2–9 each
  exercise this on a per-file basis).

- **`fire(document, ...)` works for click-interception.** Assumes the
  shim's `document.dispatchEvent` respects an event with a non-default
  `event.target` (from `Object.assign(ev, data)` in `fire`). This is
  how `runtime/test.js::fire` is designed; if the click-interception
  path in `runtime/app.js` reads `event.target` directly (it does —
  `_isAnchorClick` walks `event.target.closest('a')`), and the shim's
  MouseEvent permits assignment of `target`, the test path matches the
  production path. The spec Constraint confirms the shim permits
  `target` assignment ("The shim's MouseEvent permits assignment of
  `target` (the click-interception tests rely on this)"). If
  assignment ever stops working, the fallback is to construct the
  event with the target wired in via a small public helper — but that
  helper does not exist yet and would violate the "no new exports"
  constraint. The plan assumes assignment works (matches the existing
  fire behavior).

- **Some template-parser-shape tests have no public-API replacement.**
  E.g., "creates `<svg>` in the SVG namespace" relies on
  `frag.childNodes[0].namespaceURI`. Step 4 maps this to a
  `render(...)` + `find(el, 'svg').namespaceURI` rewrite. If
  `namespaceURI` is not surfaced through the rendered element (it
  should be — it's a public DOM property in the shim's element
  interface), the test gets deleted rather than reaching into
  `_template.fragment`.

- **Shim-test deletion loses raw-source coverage.** The five deleted
  files exercised each shim's source against a vm sandbox to confirm
  the shim doesn't accidentally depend on the host's native classes.
  After deletion, that property is asserted only indirectly: (a)
  `cargo run -p zero -- test runtime/` evaluates each shim under Boa
  (no host natives), so any accidental dependency on Node-only
  primitives surfaces as a load-time error; (b) `runtime/web-platform.test.js`
  (Step 9) is the canary if a specific API regresses. Per spec
  Out-of-Scope, this trade-off is accepted; if a specific mutant in a
  shim survives that previously died, address it in a follow-up — not
  by reviving the deleted test.

- **Performance.** Boa is meaningfully slower than V8. The full
  converted suite will take longer than `node --test`. Step 10 records
  the delta; the spec accepts this. If the delta crosses a developer-
  loop threshold, file a follow-up against the test runner (not
  against the conversion).

- **Coverage drop.** Deleting trivial mirror tests will reduce raw
  line counts and possibly mutation kills. The spec accepts this. If
  a specific mutant in `runtime/dom-shim.js` survives that previously
  died, address it in a follow-up — not by adding back the deleted
  mirror.

- **`expect().not` does not exist.** A handful of router/app tests use
  `assert.notEqual(...)`; `expect` has no `.not` matcher. The plan
  uses `expect(x === y).toBeFalsy()` / `expect(spy.callCount).toBe(0)`
  patterns. If during execution a test reads better with a new
  `.not`, resist — adding to the matcher surface is out of scope.
