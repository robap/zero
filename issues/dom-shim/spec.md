# Spec: DOM shim expansion — storage, timers, events, mockability

## Problem Statement

The framework's lightweight DOM shim (`runtime/dom-shim.js`) covers only
nodes, attributes, sibling/child traversal, listeners (no bubbling),
`document`, `window`, `location`, and `history`. Real apps written
against `zero` already hit gaps that the shim doesn't fill:

- `localStorage` / `sessionStorage` — the `examples/todos` storage
  helper (`examples/todos/web/src/lib/storage.ts`) guards every call
  with `typeof localStorage === "undefined"` so the in-memory branch
  works under tests. Apps that want to actually exercise persistence
  in their tests have no path.
- `matchMedia` — every shipped `ThemeToggle` (counter, todos, tracker)
  feature-detects it and silently degrades to "off" under tests; the
  dark-mode-on branch of theme code is therefore untested.
- `setTimeout` / `clearTimeout` / `setInterval` — used by
  `runtime/app.js` (loading-bar delay), `runtime/template.js`
  (`.throttle` / `.debounce` event modifiers), and the shipped
  `Toast` component. Boa does not install these globals, so any test
  that traverses those code paths fails noisily.
- Event bubbling — `Element#dispatchEvent` fires only on the direct
  target. Patterns that rely on bubbling (outside-click in
  `Dialog`, global click delegation, document-level `keydown`
  capture) silently no-op in tests.
- Element property surface — `classList`, `dataset`, `style`,
  `textContent`-as-setter, and the input-shaped properties
  (`value`, `checked`, `disabled`, `selected`) are missing. The
  showcase already does `document.documentElement.dataset.theme = t`
  which depends on `dataset`.
- Document API — `document.body`, `document.head`,
  `document.documentElement`, `document.getElementById`,
  `document.activeElement`, `document.title` are missing.
- Event constructors — `new Event()`, `new CustomEvent()`,
  `new KeyboardEvent()`, `new MouseEvent()` don't exist; tests must
  hand-roll a plain object and pass it through `fire()`.
- Observers — `IntersectionObserver`, `ResizeObserver`,
  `MutationObserver` are missing; any feature-detected component
  using them crashes instead of degrading.
- Crypto — `crypto.randomUUID()` is a natural fit for store IDs but
  isn't available.

Beyond filling gaps, the second motivation is **agent ergonomics**:
agents writing tests want to assert "the store called
`localStorage.setItem('todos', JSON.stringify(...))`" without
hand-installing a fake global per test. The shim should provide real
in-memory implementations *and* cooperate with `cleanup()` so
mutations between tests don't leak.

## Background

### Where the relevant code lives

- `runtime/dom-shim.js` — the only file that owns the shim's
  surface. This slice grows it substantially.
- `runtime/dom-shim.test.js` — `node:test` self-tests for the shim;
  every new piece grows its own coverage here.
- `runtime/test.js` — `cleanup()` lives here; this slice extends it
  to reset storage and any other per-test mutable shim state.
- `runtime/test.test.js` — covers test-API behavior including the
  cleanup contract.
- `crates/zero-runtime/src/lib.rs` — embeds `runtime/dom-shim.js` as
  `ZERO_DOM_SHIM_BODY`. Body grows; no other Rust changes needed
  here.
- `crates/zero-test-runner/src/harness.rs` — calls `eval_dom_shim`
  before user code runs. **This file gains a small Rust-side change**
  to register `setTimeout` / `clearTimeout` / `setInterval` /
  `clearInterval` / `requestAnimationFrame` /
  `cancelAnimationFrame` against Boa's job queue (see "Timer
  semantics" below). The shim cannot install real timers from
  JS-land alone — Boa's job queue is reachable only from Rust.
- `FRAMEWORK_INTERNAL_BASENAMES` (in both `harness.rs` and
  `runtime/test.js`) — already lists `dom-shim.js`; no change.

### Why a real-implementation shim, not just mocks

The user picked the "real shim + auto-reset in cleanup()" model over
both writable-globals-only and dedicated mock factories. Rationale:

- Real implementations let app logic run unmodified under tests.
  The `examples/todos` storage helper stops needing its
  `typeof localStorage === "undefined"` guards (it can keep them
  for production safety, but tests exercise the real path).
- A single auto-reset hook (`cleanup()`) is the same lifecycle every
  existing test already knows. No new API surface.
- Agents that want to assert calls use the existing `spy()`
  primitive to wrap individual methods (e.g.
  `localStorage.setItem = spy(localStorage.setItem.bind(localStorage))`).
  No new factories, no new globals to learn.

### Why real timers via job-queue integration

Boa supports a job queue; native ECMAScript Promise resolution
already flows through it. Registering `setTimeout` etc. as host
hooks that enqueue jobs lets timer-driven code (`app.js` loading-bar
delay, `.debounce`/`.throttle`, `Toast` auto-dismiss) run
naturally. Tests that await a Promise that resolves after a 0ms
timer work without explicit advance-time calls.

This does mean **timers do not honor wall-clock `ms`** under Boa:
the engine has no clock-driven event loop. All timers fire in
queue-order at the next job-queue drain. This matches what most
"timers under test" abstractions do (Jest's fake timers, vi's
shorthand) and is acceptable because the failure mode is "ordering"
not "delay duration."

### Why full bubble + capture

Real-DOM semantics are the principle of least surprise. Apps that
add a global click handler for outside-click dismissal (Dialog
backdrop, dropdown close) only get exercised if events bubble. The
capture phase costs almost nothing on top of bubbling (one extra
pre-walk) and matches what `addEventListener(type, fn, true)`
documents — keeping the shim honest if test code uses capture.

### Mockability invariants

- Every global installed by the shim (`localStorage`,
  `sessionStorage`, `matchMedia`, `navigator`, `crypto`,
  `IntersectionObserver`, etc.) is declared as a writable,
  configurable property of `globalThis` / `window`. Tests can
  reassign them in `beforeEach` without strict-mode errors.
- Every shim method on those globals is a plain function property
  (not on a prototype with non-configurable methods). Tests can
  swap `localStorage.setItem = spy(...)` without using
  `Object.defineProperty`.
- `cleanup()` (from `zero/test`) restores per-test mutable state:
  storage maps are cleared, `document.activeElement` reset to
  `null`, `document.title` reset to `""`, timer queues drained.
  It does **not** un-swap reassigned globals — tests that mutate a
  global directly are responsible for restoring it themselves
  (typically via `beforeEach` re-installing fresh state).

## Requirements

All file paths below are relative to repo root.

### 1. Web Storage (`localStorage`, `sessionStorage`)

Add to `runtime/dom-shim.js`:

- A `_makeStorage()` factory returning a Storage-shaped object
  backed by an internal `Map`:
  - Methods: `getItem(k)`, `setItem(k, v)`, `removeItem(k)`,
    `clear()`, `key(i)`.
  - Property: `length` (getter returning map size).
  - Indexed string-keying (`storage["foo"]`) is **not** supported
    — out of scope; explicitly noted below.
  - All values stringified via `String(v)` on `setItem` (matches
    real Storage).
- Two instances exported as `localStorage` and `sessionStorage`.
- Installed on both `window` and `globalThis` (configurable,
  writable).
- `cleanup()` calls `localStorage.clear()` and
  `sessionStorage.clear()` (see §9).

### 2. `matchMedia`

Add to `runtime/dom-shim.js`:

- `window.matchMedia(query)` returning a MediaQueryList-shaped
  object:
  - `.media` — the query string
  - `.matches` — defaults to `false`
  - `.addEventListener(type, fn)` / `.removeEventListener(type, fn)`
    — store handlers in an internal list
  - `.addListener(fn)` / `.removeListener(fn)` — legacy aliases
  - `.onchange` — settable property
  - `.dispatchEvent(event)` — fires registered handlers (used by
    tests to simulate a media-query change)
- Default `.matches` is `false` for every query (no real media
  evaluation). Tests that want a specific query to match override
  by reassigning `window.matchMedia = (q) => ({ matches: q.includes('dark'), ... })`
  before the component runs.

### 3. `navigator` (minimal)

Add to `runtime/dom-shim.js`:

- `window.navigator` and `globalThis.navigator` (same instance):
  - `.userAgent` — `"zero-test-shim/1.0"`
  - `.language` — `"en-US"`
  - `.languages` — `["en-US"]`
  - `.onLine` — `true`
  - `.platform` — `""`
- All properties are plain writable data properties (tests
  override directly).

### 4. Crypto

Add to `runtime/dom-shim.js`:

- `globalThis.crypto` (also `window.crypto`):
  - `crypto.randomUUID()` — returns an RFC4122-shaped v4 UUID
    using `Math.random()`. Not cryptographically strong; clearly
    documented in a code comment. The use case is store IDs, not
    secrets.
  - `crypto.getRandomValues(typedArray)` — fills with
    `Math.random()`-derived bytes (truncated to the array's element
    size). Returns the input array.

If Boa already exposes `crypto` natively in the version we use,
detect via `if (typeof globalThis.crypto?.randomUUID !== "function")`
and only install missing methods. **Open question** (plan to
verify): the current Boa version may already ship `crypto`.

### 5. Observers (no-op)

Add to `runtime/dom-shim.js`:

- `globalThis.IntersectionObserver`, `globalThis.ResizeObserver`,
  `globalThis.MutationObserver`. Each is a constructor:
  - Stores the callback.
  - `.observe(target, options?)` — pushes `{ target, options }`
    into an internal `.observations` array (for test inspection;
    enumerable).
  - `.unobserve(target)` — removes matching entry.
  - `.disconnect()` — clears the array.
  - `.takeRecords()` (for MutationObserver) — returns `[]`.
- Callbacks are **never invoked** by the shim. Tests that want to
  trigger an observer call the callback directly via the instance
  reference if they need to.

### 6. `getComputedStyle`

Add `window.getComputedStyle(el, pseudoElt?)` returning a
CSSStyleDeclaration-like object:

- `.getPropertyValue(name)` returns `""` for any input.
- Indexed access returns `""`.
- `.length` is `0`.
- Mutators (`.setProperty`, etc.) throw, matching real-DOM
  read-only semantics on the returned object.

### 7. Element property surface

Add to the object returned by `createElement` in
`runtime/dom-shim.js`:

- **`classList`** — object with `add(...names)`, `remove(...names)`,
  `toggle(name, force?)`, `contains(name)`, `replace(old, new)`,
  `.length`, and string-indexed access. Stored as a
  whitespace-separated `class` attribute; reads/writes proxy
  through `getAttribute('class')` / `setAttribute('class', ...)`.
- **`dataset`** — `Proxy` (or plain getter object) over `data-*`
  attributes. `el.dataset.fooBar` round-trips with
  `data-foo-bar`. Setting `el.dataset.x = 5` calls
  `setAttribute('data-x', '5')`. Deleting unsets the attribute.
- **`style`** — object with `setProperty(name, value)`,
  `removeProperty(name)`, `getPropertyValue(name)`, and
  camelCase-keyed property assignment (`el.style.color = 'red'`).
  Backed by an internal `Map<string, string>`. Reading
  `el.getAttribute('style')` produces a serialized form
  (`"color: red; --x: 1"`); writing the `style` attribute parses
  it back. CSS-custom-property names (leading `--`) are honored as
  keys.
- **`textContent`** — getter returns concatenated text of all
  descendant text nodes (same walk as `text()` helper does
  today). Setter removes all children and appends one text node
  with the provided string. `null` / `undefined` set to `""`.
- **Input-shaped properties**:
  - `value` — for `<input>` / `<textarea>` / `<select>`, a plain
    string property. Round-trips with the `value` attribute when
    the attribute is set; once mutated via the property, the
    attribute and property diverge (matches real DOM). Default
    `""`.
  - `checked` — boolean property on `<input type="checkbox" / radio">`.
  - `disabled` — boolean property on any element.
  - `selected` — boolean property on `<option>`.
  - `name`, `type`, `placeholder`, `htmlFor` — plain string
    properties that round-trip with the matching attribute.

  Property/attribute coupling is intentionally simple: setting the
  property updates the corresponding attribute (and vice versa).
  Tests don't need to track divergence; the simpler model covers
  every component shipped today.

- **`className`** — string property mirroring the `class`
  attribute. Get returns `getAttribute('class') ?? ""`; set calls
  `setAttribute('class', String(v))`.

### 8. Document additions

Add to the `document` object in `runtime/dom-shim.js`:

- `document.documentElement` — eagerly-created `<html>` element.
- `document.head` — eagerly-created `<head>` element, appended to
  `documentElement`.
- `document.body` — eagerly-created `<body>` element, appended to
  `documentElement`.
- `document.getElementById(id)` — descendant walk from
  `documentElement` matching `getAttribute('id') === id`. (The
  existing `querySelector('#id')` machinery already supports this;
  `getElementById` is a thin wrapper for code that prefers it.)
- `document.activeElement` — element pointer, starts `null`.
  Updates when any element's `.focus()` is called and clears when
  `.blur()` is called. (The existing `focus()` / `blur()` no-ops
  on `Element` grow this behavior.)
- `document.title` — string property, default `""`.

### 9. Event constructors and bubbling

Add to `runtime/dom-shim.js`:

- `globalThis.Event` — constructor taking `(type, init?)`. Reads
  `bubbles`, `cancelable`, `composed` from `init`. Methods
  `preventDefault()`, `stopPropagation()`, `stopImmediatePropagation()`.
  Read-only props `defaultPrevented`, `type`, `target`,
  `currentTarget`, `eventPhase`.
- `globalThis.CustomEvent` — same as `Event` plus `.detail` read
  from `init.detail`.
- `globalThis.KeyboardEvent` — copies `key`, `code`, `altKey`,
  `ctrlKey`, `metaKey`, `shiftKey`, `repeat` from `init`.
- `globalThis.MouseEvent` — copies `clientX`, `clientY`, `screenX`,
  `screenY`, `button`, `buttons`, `altKey`, `ctrlKey`, `metaKey`,
  `shiftKey` from `init`.
- The `fire(el, type, data)` helper in `runtime/test.js` continues
  to work; it now uses these constructors internally (constructing
  a real `Event` rather than a bag of fields). Existing tests pass
  unchanged.

**Bubbling**: `Element#dispatchEvent` is rewritten to:

1. If `event.bubbles`, build the ancestor chain (target → root).
2. Run **capture phase**: walk root → target, firing listeners
   registered with `{capture: true}`. `stopPropagation` halts
   further walking; `stopImmediatePropagation` halts the rest of
   the current node's listeners too.
3. Run **target phase**: fire non-capture listeners on `target`.
4. Run **bubble phase** (only if `event.bubbles`): walk
   target.parentNode → root, firing non-capture listeners. Same
   stop semantics.

`event.target` is set once (the dispatch origin);
`event.currentTarget` updates per node. `addEventListener`
already accepts an `options` parameter; extend it to honor
`capture: true` (in addition to `once: true` which it already
honors).

If `event.cancelable && event.defaultPrevented`,
`dispatchEvent` returns `false`; otherwise `true` (matches DOM).

### 10. Timers (`setTimeout`, `setInterval`, rAF)

**Rust-side change in `crates/zero-test-runner/src/harness.rs`**:

- Install host functions `setTimeout(fn, ms)`, `clearTimeout(id)`,
  `setInterval(fn, ms)`, `clearInterval(id)`,
  `requestAnimationFrame(fn)`, `cancelAnimationFrame(id)` on
  `globalThis` via `Context::register_global_callable` (or the
  equivalent Boa builder API used elsewhere in the harness).
- `setTimeout(fn, ms)` returns a numeric id (monotonic counter).
  Internally schedules `fn` as a Boa job via the context's job
  queue. `ms` is ignored for ordering — all pending timers fire
  in the order they were registered, when the job queue is
  drained.
- `clearTimeout(id)` looks up the id in an internal pending-map
  and marks it cancelled; the job runs but no-ops if cancelled.
- `setInterval(fn, ms)` schedules `fn`; after `fn` runs, schedules
  itself again. Bounded by `clearInterval` only.
- `requestAnimationFrame(fn)` schedules a one-shot job that calls
  `fn(currentMillis)` — `currentMillis` is a monotonic counter
  starting at 0 that advances by 16ms per call (sentinel-ish; not
  a real clock).
- The job queue is drained automatically by Boa whenever the
  context returns to its event loop (i.e. between `await`s in
  test bodies that use Promises). No new `await flushTimers()`
  helper is required for typical async test flow.

The shim's `cleanup()` cancels all pending timers (loops the
internal map and clears each id). Tests that rely on a timer
running after `cleanup()` are broken by design.

### 11. `cleanup()` extensions (`runtime/test.js`)

Update `cleanup()` to additionally:

- `localStorage.clear()` and `sessionStorage.clear()`.
- Set `document.activeElement = null`.
- Set `document.title = ""`.
- Clear all child nodes of `document.body`, `document.head`, and
  `document.documentElement` (reset to empty document).
- Cancel all pending timers (via a shim-internal
  `__clearAllTimers__()` hook the timer host registers).
- Existing behavior unchanged: dispose render-tracked scopes,
  `_setCurrentApp(null)`.

Order matters: dispose scopes first (so their cleanup functions
can still touch storage/timers if they want), then clear storage,
then cancel timers, then reset document fields.

### 12. Type declarations (`runtime/zero-test.d.ts`)

No new exports from `zero/test` for this slice — every addition
is a global shim. `runtime/zero-test.d.ts` is not the right
home for these globals.

Instead, `runtime/zero.d.ts` keeps the framework surface clean;
the **ambient global types** live in `runtime/zero.d.ts` as a
`declare global { interface Window { ... } }` block extending the
parts the shim adds beyond the standard `lib.dom` types — or, if
the project's `tsconfig.json` already references `lib: ["dom"]`,
no shim-specific types are needed because the standard DOM lib
already covers these globals. **Open question**: confirm the
emitted `tsconfig.json` includes `"lib": ["DOM", "ESNext"]`;
verify in plan phase.

### 13. Self-tests (`runtime/dom-shim.test.js`)

Grow the `node:test` suite to cover every requirement above. Each
of §1–§10 gets at least one describe block:

- **Storage**: `setItem`/`getItem` round-trip, `removeItem`, `clear`,
  `length`, separation between `localStorage` and `sessionStorage`.
- **matchMedia**: returns shape with `.matches === false` by default;
  override via reassignment.
- **navigator**: shape; override-able.
- **crypto**: `randomUUID()` returns a 36-char RFC4122-shaped string;
  `getRandomValues` returns the input typed array.
- **Observers**: constructors store callback; `observe` records;
  `disconnect` clears.
- **getComputedStyle**: returns shape; `getPropertyValue` returns
  `""`.
- **classList**: `add` / `remove` / `toggle` / `contains` /
  `replace`; reads back via `getAttribute('class')`.
- **dataset**: `el.dataset.fooBar = 'x'` ⇒ `getAttribute('data-foo-bar') === 'x'`;
  read-back; deletion.
- **style**: `setProperty` round-trips; `el.style.color = 'red'`
  serializes into the style attribute; CSS custom properties.
- **textContent**: getter walks; setter replaces children.
- **value/checked/disabled/selected**: round-trip with attributes.
- **className**: round-trip with `class` attribute.
- **Document**: `body`/`head`/`documentElement` are wired;
  `getElementById` finds by id; `activeElement` tracks focus;
  `title` round-trip.
- **Event constructors**: `new Event('foo', { bubbles: true })`
  shape; `new CustomEvent('bar', { detail: 42 })`;
  `new KeyboardEvent('keydown', { key: 'Enter' })`;
  `new MouseEvent('click', { clientX: 10 })`.
- **Bubbling**: dispatching on a child fires parent listener
  when `bubbles: true`; doesn't when `false`; `stopPropagation`
  halts further nodes; `stopImmediatePropagation` halts further
  listeners on the same node; capture phase fires before target
  phase.

### 14. Self-tests (`runtime/test.test.js`)

- `cleanup()` clears `localStorage` and `sessionStorage`.
- `cleanup()` resets `document.activeElement` and `document.title`.
- `cleanup()` cancels pending timers (assert via a timer that
  would otherwise fire later and observably mutate state).

### 15. Harness self-tests (Rust)

Add a small test in `crates/zero-test-runner/tests/` (or extend
an existing one) that runs a `.ts` file using
`setTimeout(() => done(), 0)`-style logic and asserts the timer
fired. Covers the new harness-side timer host.

### 16. Spec-text amendments

- `zero-framework-spec.md` §8 (Testing — "No Browser Required"):
  amend the line "supports only the DOM APIs that z's template
  system uses" to mention the broader surface (storage, timers,
  matchMedia, observers, real events). Update the LOC estimate
  ("~500 lines") to whatever the post-change file lands at.
- `zero-framework-spec.md` §11 (Complete API Surface): no
  changes needed; nothing new is exported from `"zero"` or
  `"zero/test"`.
- `BEST_PRACTICES.md`: add a short paragraph on "Testing browser
  APIs" — point readers at the real shim for `localStorage`,
  `matchMedia`, and timers; mention the `spy()`-wrapping pattern
  for asserting calls.
- `zero-framework-spec.md` §12 Phase list: add a new entry under
  the next available phase ("Phase N — DOM shim expansion")
  pointing at this spec.
- `issues/test-helpers/spec.md`: this slice does not change the
  selector grammar or `spy()` surface. The cross-reference note
  in "Spec text amendments" of that file (re: dom-shim selector
  limitations) is unrelated and stays.

## Constraints

- **No new exports from `"zero"` or `"zero/test"`.** Everything
  added is a shim-installed global. The framework surface stays
  the same.
- **Real timers fire in queue order, not by `ms`.** Boa has no
  wall clock. Tests that depend on relative `ms` between two
  timers must structure their code to be ordering-tolerant, or
  use the spy primitive to observe scheduling without firing.
- **Auto-cleanup is opt-in via `cleanup()`.** Tests that don't
  call `cleanup()` (or run outside `afterEach(cleanup)`) inherit
  storage leakage between tests. The scaffolded test helpers
  already wire `cleanup()` into `afterEach`; no change there.
- **Observers never fire automatically.** Tests that need them to
  fire must invoke the recorded callback themselves.
- **`getComputedStyle` returns `""` for everything.** No layout
  engine. Components that branch on computed values are out of
  scope for unit tests under this shim — that's an E2E concern.
- **`crypto.randomUUID()` is not cryptographically strong.** It
  uses `Math.random()`. Documented in a code comment. Production
  code that needs real randomness must run in a real browser
  context (and tests that depend on cryptographic strength belong
  in E2E).
- **No HTMLFormElement / form serialization.** `<form>` is just
  another element; `FormData` is out of scope. Form-component
  tests rely on event handlers, not native form submission.
- **No `innerHTML` parser.** Setting `el.innerHTML = "<b>x</b>"`
  would require an HTML parser. Out of scope; documented.
- **No layout stubs** (`getBoundingClientRect`, `offsetWidth`,
  etc.). The user chose against this in the planning round; the
  shim throws "not implemented" if accessed. Future slice if a
  real test needs it.
- **No traversal-getters slice** (`children`, `parentElement`,
  `nextElementSibling`, `previousElementSibling`,
  `Element#matches`, `Element#contains`, `Element#remove`). The
  user chose against in planning; existing `childNodes` /
  `parentNode` / `querySelector*` cover the same ground. Add
  separately if needed.
- **No global swap restoration in cleanup()**. If a test does
  `globalThis.matchMedia = customStub`, `cleanup()` does not
  restore the original. Tests that swap globals are responsible
  for resetting in `beforeEach`. Auto-restore is intentionally
  deferred; the spec for that primitive lives in the test-helpers
  follow-up if real demand emerges.

## Out of Scope

- IndexedDB.
- `document.cookie`.
- `fetch` (assumed provided by Boa or by user dependency
  injection; the `"zero/http"` module's tests pass `fetch` via
  `init.fetch`).
- Web Workers (`Worker`, `MessageChannel`).
- Service workers / Cache API.
- `History API` enhancements beyond what the shim already has.
- `Notification`, `Permissions`, `Clipboard` APIs.
- WebSocket / EventSource.
- Pointer Events (only `MouseEvent` / `KeyboardEvent` / `Event` /
  `CustomEvent` are constructed; `PointerEvent` is out).
- Touch Events.
- `Range`, `Selection`, `getSelection()`.
- `Element#animate` / Web Animations API.
- Real `innerHTML` parsing.
- Layout stubs (`getBoundingClientRect`, `offsetWidth`, etc.).
- Element traversal getters (`children`, `parentElement`,
  `nextElementSibling`, `previousElementSibling`, `matches`,
  `contains`, `remove`).
- A `mockGlobal(name, value)` helper or any dedicated mock factory.
- A `replace(obj, key, spy)` helper.
- Module mocking.
- Fake/manual timer-advance API (`advanceTimers(ms)`).
- Auto-restore of swapped globals on `cleanup()`.

## Open Questions

- **Does Boa's current pinned version expose `crypto.randomUUID`
  natively?** If yes, the shim skips installing it. Plan verifies
  by running a smoke script under the harness; if missing, the
  shim's polyfill installs.
- **Does the emitted `tsconfig.json` reference `lib: ["DOM"]`?**
  If yes, no shim-specific ambient types are needed and the
  user's editor already understands `localStorage`, `matchMedia`,
  etc. Plan verifies by reading
  `crates/zero-scaffold/src/scaffold/tsconfig.json`.
- **Job-queue API in the pinned Boa version.** The Rust-side
  timer registration depends on `Context::register_global_callable`
  (or equivalent) and on the job-queue draining behavior between
  Promise awaits. Plan confirms the exact API and lands a
  prototype before committing to the full surface.
- **`getComputedStyle` on detached elements.** Real DOM returns
  empty for detached elements anyway, so the stub's "always
  empty" behavior is harmless; doc-comment notes this.
- **Style attribute parsing.** Setting
  `el.setAttribute('style', 'color: red; --x: 1')` should
  populate the `style` map. The reverse (mutating `el.style`
  updates `el.getAttribute('style')`) is required. Choice
  between (a) lazy serialize on read, (b) eager serialize on
  write. Recommendation: (a) lazy — defer to plan.
- **Event ordering across capture/target/bubble vs. the
  `{once: true}` removal.** Once-listeners that fire during
  capture should self-remove before the bubble phase. Plan
  confirms the implementation order.
- **`dispatchEvent` return value when not cancelable.** Real DOM
  returns `true` if `preventDefault()` was not called or the
  event is non-cancelable. Shim matches; documented in a
  comment so the bubble-phase rewrite doesn't accidentally
  always return `true`.
- **Should `Element.prototype.focus()` dispatch a `focus` event?**
  Real DOM does (and a `blur` on the previously-focused element).
  Recommendation: yes — small, matches expectations,
  `cleanup()` resets `activeElement` anyway. Plan confirms.
- **Storage events.** Real browsers dispatch `storage` events on
  `window` when a *different document* mutates the same storage
  key. The shim has one document; the spec defers `storage`
  events to a follow-up. Tests that need them re-implement via
  the spy pattern.
- **Indexed storage access (`localStorage["key"]`).** Real
  Storage supports it via Proxy. The shim does **not**. If a real
  test uses bracket syntax instead of `.getItem(...)`, fall back
  to documenting the limitation; adding Proxy support is cheap
  if demand appears.
