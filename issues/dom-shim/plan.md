# Plan: DOM shim expansion — storage, timers, events, mockability

## Summary

Grow `runtime/dom-shim.js` from a minimal node/element/attribute shim into a
broader test-fixture surface that real apps already depend on: web storage,
`matchMedia`, `navigator`, `crypto`, observers, `getComputedStyle`, real event
constructors with capture/target/bubble dispatch, an element property surface
(`classList` / `dataset` / `style` / `textContent` / input-shaped props),
document additions (`body` / `head` / `documentElement` / `activeElement` /
`title`), and a Rust-side timer host on the Boa harness so `setTimeout` and
friends drive `runtime/app.js`, `runtime/template.js`, and `Toast`-style code
naturally. Auto-reset is wired through the existing `cleanup()` lifecycle in
`runtime/test.js`. No new exports from `"zero"` or `"zero/test"` — every
addition is a shim-installed global, so the framework surface is unchanged.

The slice is ordered so each step leaves the workspace compiling and both
`cargo test --workspace` and `node --test runtime/*.test.js` green: foundations
first (events, element props), then document/storage/auxiliaries, then the
Rust-side timer host, then the cleanup() hook that ties timers into the
per-test lifecycle. Spec doc amendments come last.

## Prerequisites

Three open questions from the spec must be resolved during Step 0 before the
shim grows. Each is small and reversible:

1. **Does Boa 0.21.1 already expose `crypto.randomUUID`?** Verified via a
   tiny smoke probe in the harness's existing `tests` module. If yes,
   `_makeCrypto()` only installs missing methods (gated by
   `typeof globalThis.crypto?.randomUUID !== "function"`).
2. **Does the scaffolded `tsconfig.json` give DOM types out of the box?**
   Already verified: `crates/zero-scaffold/src/scaffold/tsconfig.json` sets
   `"target": "ESNext"` with no explicit `lib`, so TS defaults to
   `["DOM", "DOM.Iterable", "ScriptHost"]` — the standard DOM lib is in
   scope. No new ambient `.d.ts` declarations are needed for this slice.
3. **Job-queue API for the Rust-side timer host.** Boa 0.21.1's `Context`
   already has `run_jobs()` (called by `evaluate_module` and
   `call_and_drain`). The pattern used elsewhere — register a global
   `NativeFunction` that mutates Rust-side state, and rely on the existing
   job-queue drain — is sufficient. Step 7 prototypes one
   `setTimeout`-as-microtask first and proves the drain semantics before
   committing to the rest.

No other issues block this slice.

## Steps

- [x] **Step 0: Prerequisite probes (Boa crypto, job-queue prototype)**
- [x] **Step 1: Event constructors + capture/target/bubble dispatch**
- [x] **Step 2: Element property surface (classList, dataset, style, textContent, className, input props)**
- [x] **Step 3: Document additions (documentElement / head / body / getElementById / activeElement / title)**
- [x] **Step 4: Web Storage (`localStorage`, `sessionStorage`)**
- [x] **Step 5: Auxiliary globals (`matchMedia`, `navigator`, `crypto`, Observers, `getComputedStyle`)**
- [x] **Step 6: Rust-side timer host (`setTimeout` / `clearTimeout` / `setInterval` / `clearInterval` / `requestAnimationFrame` / `cancelAnimationFrame` / `__clearAllTimers__`)** — **deviation:** implemented entirely in JS via `Promise.resolve().then(...)` inside `runtime/dom-shim.js#_installTimerHost`. The plan's premise that "Boa's job queue is reachable only from Rust" turned out to be incorrect: the job queue drains JS-scheduled microtasks already. A Rust-side `install_timer_host` was prototyped first; it tripped Boa's MapLock GC bug on the showcase, and required unsafe `NativeFunction::from_closure` captures of `JsValue` (UAF risk). The JS-side version is smaller, GC-safe, and behaviourally identical from the test's perspective.
- [x] **Step 7: `cleanup()` extensions in `runtime/test.js`**
- [x] **Step 8: Spec / doc amendments**

---

## Step Details

### Step 0: Prerequisite probes (Boa crypto, job-queue prototype)

**Goal:** Resolve the two open questions that gate code-level decisions in
later steps. No production code lands here — only a throwaway test that
informs the design of Steps 5 and 6.

**Files:**
- `crates/zero-test-runner/src/harness.rs` (test module only)

**Changes:**
- Add a `#[test]` that boots a Boa `Context`, runs
  `typeof globalThis.crypto?.randomUUID === "function"`, and prints the
  result via `eprintln!`. Run once with `cargo test -p zero-test-runner
  probe_boa_crypto -- --nocapture` to read the answer, then **delete the
  test** before committing Step 0. Record the answer inline in the Step 5
  notes (controls whether the shim's `_makeCrypto()` always installs or
  conditionally installs).
- Add a second `#[test]` that schedules a JS function via a host-registered
  `NativeFunction` (mimicking `setTimeout(fn, 0)`) by enqueueing a
  `JsPromise::resolve(...).then(fn, ctx)` continuation, then asserts the
  function ran after `ctx.run_jobs()`. This is the prototype for Step 6;
  keep it in the test module (rename to
  `host_microtask_runs_after_run_jobs`) — Step 6 will replace it with the
  real harness function.

**Tests:**
- Both probes are themselves `cargo test` runs; the deliberate side effect
  is `eprintln!` output the operator reads. The microtask probe stays as a
  permanent test (renamed in Step 6).

**Acceptance:**
- Operator records the `crypto.randomUUID` answer in this plan as a
  comment under Step 5 before moving on.
- The microtask probe passes, proving the job-queue drain pattern works.

---

### Step 1: Event constructors + capture/target/bubble dispatch

**Goal:** Replace the current single-target `dispatchEvent` with real-DOM
ordering (capture → target → bubble), introduce `Event` / `CustomEvent` /
`KeyboardEvent` / `MouseEvent` global constructors, and teach
`addEventListener` to honor `{capture: true}`. Update `fire()` in
`runtime/test.js` to construct a real `Event` instead of a bag-of-fields.
This is foundational because Steps 2–4 add elements (`documentElement`,
`body`, `head`) whose tests rely on bubbling, and Step 5's `matchMedia`
event registry uses the same shape.

**Files:**
- `runtime/dom-shim.js`
- `runtime/test.js`
- `runtime/dom-shim.test.js`

**Changes:**

In `runtime/dom-shim.js`:

- Add `class Event` (use a plain function-constructor style if Boa's class
  support is a concern; the rest of the shim is plain-object so prefer a
  factory function `_makeEvent(type, init)` plus
  `globalThis.Event = function Event(type, init) { return _makeEvent(type, init); }`):
  - Reads `bubbles`, `cancelable`, `composed` from `init`.
  - Methods: `preventDefault()`, `stopPropagation()`,
    `stopImmediatePropagation()`.
  - Props: `defaultPrevented` (false until prevented), `type`, `target`
    (null until dispatch), `currentTarget` (null), `eventPhase` (0/1/2/3
    constants `NONE`/`CAPTURING_PHASE`/`AT_TARGET`/`BUBBLING_PHASE`).
  - Internal `_stopPropagation` and `_stopImmediate` flags read by the
    dispatch walker.
- Add `globalThis.CustomEvent` — wraps `Event` and copies `init.detail`
  onto `.detail`.
- Add `globalThis.KeyboardEvent` — wraps `Event` and copies `key`, `code`,
  `altKey`, `ctrlKey`, `metaKey`, `shiftKey`, `repeat` from `init`.
- Add `globalThis.MouseEvent` — wraps `Event` and copies `clientX`,
  `clientY`, `screenX`, `screenY`, `button`, `buttons`, `altKey`,
  `ctrlKey`, `metaKey`, `shiftKey` from `init`.
- Rewrite `createElement`'s `addEventListener` to accept an `options`
  object containing `{once, capture}` (or a boolean alias meaning
  `capture`). Internal listener entry: `{handler, once: boolean, capture: boolean}`.
- Rewrite `createElement`'s `dispatchEvent`:
  ```
  function _dispatchEvent(target, event) {
    if (event.target == null) event.target = target;
    const path = [];
    let n = target;
    while (n) { path.push(n); n = n.parentNode; } // target → root
    const ancestors = path.slice(1).reverse();    // root → target.parent

    // Capture phase
    event.eventPhase = 1;
    for (const node of ancestors) {
      if (event._stopPropagation) break;
      _fireListenersOn(node, event, /*capture*/ true);
    }
    // Target phase
    if (!event._stopPropagation) {
      event.eventPhase = 2;
      _fireListenersOn(target, event, /*capture*/ false);
      _fireListenersOn(target, event, /*capture*/ true); // captures at-target also fire
    }
    // Bubble phase
    if (event.bubbles && !event._stopPropagation) {
      event.eventPhase = 3;
      for (let i = path.length - 1; i >= 1; i--) {
        if (event._stopPropagation) break;
        _fireListenersOn(path[i], event, /*capture*/ false);
      }
    }
    event.eventPhase = 0;
    event.currentTarget = null;
    return !(event.cancelable && event.defaultPrevented);
  }
  ```
- `_fireListenersOn(node, event, capture)` walks `node._listeners.get(event.type)`,
  fires entries matching `capture`, removes `{once: true}` entries after
  firing, and honors `_stopImmediate` mid-walk. It sets
  `event.currentTarget = node`.
- Keep the old single-target behavior for `document` and `window` via
  `_makeEventTarget()` — they have no parent chain, but their listeners
  should still fire (document is the bubble destination once we hook
  documentElement → body in Step 3; for now leave `_makeEventTarget`
  unchanged).
- Backward compat: `dispatchEvent` accepts both real `Event` instances
  and the legacy `{type, ...}` plain object (older `fire()` callers,
  and the existing test in `dom-shim.test.js:66` and `:74`). Detect by
  `event instanceof Event || typeof event.preventDefault === "function"`;
  for plain objects, decorate with stop/prevent methods on the fly so the
  walker can read `_stopPropagation` etc.

In `runtime/test.js`:

- Update `fire(el, type, data)` to construct via the new constructors
  when available:
  ```
  const ctor = type.startsWith("key") ? KeyboardEvent
             : type === "click" || type.startsWith("mouse") ? MouseEvent
             : Event;
  const ev = new ctor(type, { bubbles: true, cancelable: true, ...data });
  el.dispatchEvent(ev);
  ```
  Preserve the existing public signature (`fire(el, type, data?)`). Tests
  that previously pulled `defaultPrevented` off a returned reference still
  work — `dispatchEvent` returns a boolean, and the event object itself
  exposes `.defaultPrevented`.

**Tests** (`runtime/dom-shim.test.js`):

- `new Event('x', {bubbles: true}).bubbles === true`; default `bubbles`
  is `false`.
- `new CustomEvent('y', {detail: 42}).detail === 42`.
- `new KeyboardEvent('keydown', {key: 'Enter'}).key === 'Enter'`.
- `new MouseEvent('click', {clientX: 10}).clientX === 10`.
- Bubbling: parent listener fires when child dispatches a bubbling event;
  does **not** fire when `bubbles: false`.
- `stopPropagation` halts further nodes; `stopImmediatePropagation` halts
  the rest of the current node's listeners too.
- Capture phase fires before target phase.
- `{once: true}` listener self-removes after first fire (existing test
  remains green).
- `preventDefault()` on a cancelable event causes `dispatchEvent` to
  return `false`.
- Legacy `el.dispatchEvent({type, preventDefault, stopPropagation})` still
  dispatches (covers existing test at `dom-shim.test.js:66`).

---

### Step 2: Element property surface

**Goal:** Add the missing element-level properties so component code can
mutate elements through real-DOM-shaped APIs in tests. Done after Step 1
so event-driven tests for inputs (e.g. checkbox `change` propagation) can
land alongside.

**Files:**
- `runtime/dom-shim.js`
- `runtime/dom-shim.test.js`

**Changes:** Extend the object returned by `createElement`. Each property
below is added via `Object.defineProperty` on the per-element object (not
a shared prototype — keep the shim's plain-object style so reassignment
remains easy):

- **`classList`**: an object with `add(...names)`, `remove(...names)`,
  `toggle(name, force?)` returning the resulting boolean,
  `contains(name)`, `replace(old, new)`, `.length`, and numeric-indexed
  access. Backed by reading / writing `getAttribute('class')`. A small
  helper `_classTokens(el)` returns the current token array; mutator
  methods write back via `setAttribute('class', tokens.join(' '))`.
  Empty-attribute case: removing the last class deletes the attribute.

- **`dataset`**: a `Proxy` with `get`, `set`, `has`, `deleteProperty`,
  `ownKeys`, `getOwnPropertyDescriptor` traps. `get(_, key)` →
  `getAttribute('data-' + camelToKebab(key))`. `set` writes
  `setAttribute('data-' + camelToKebab(key), String(value))`.
  `deleteProperty` calls `removeAttribute`. `ownKeys` enumerates
  every attribute starting with `data-` and converts `kebab-case` back
  to `camelCase`. Helper `_camelToKebab("fooBar") === "foo-bar"` and
  inverse `_kebabToCamel("foo-bar") === "fooBar"`.

- **`style`**: object backed by an internal `Map<string, string>`. Public
  surface:
  - `setProperty(name, value)` — `_styleMap.set(name, String(value))`.
  - `removeProperty(name)` — `_styleMap.delete(name)`.
  - `getPropertyValue(name)` — `_styleMap.get(name) ?? ""`.
  - Camel-case keyed property assignment: `el.style.color = 'red'`.
    Implement via a Proxy whose `set(_, key, value)` resolves
    `_camelToKebab(key)` (or `--` prefix when `key.startsWith('--')`)
    and stores. `get(_, key)` returns the cached value or `""`.
  - Serialization (lazy on read): `getAttribute('style')` triggers
    `_serializeStyle(el)` which joins entries as
    `"color: red; --x: 1"`. Style writes through `el.style.foo = 'bar'`
    set a dirty flag; `getAttribute('style')` re-serializes when dirty.
  - Style attribute parsing: `setAttribute('style', 'color: red; --x: 1')`
    parses entries on each write; clears and repopulates the map.
- **`textContent`**:
  - Getter: walks descendant text nodes (`nodeType === 3`) and
    concatenates `nodeValue`. (Same walk as `text()` helper, but local.)
  - Setter: removes all child nodes (via `_removeChild` loop), then if
    the value is non-empty appends a single text node. `null` /
    `undefined` → empty string.

- **`className`**: getter returns `getAttribute('class') ?? ""`; setter
  calls `setAttribute('class', String(v))`.

- **Input-shaped properties** stored on the element object:
  - `value` (string, defaults `""`). Setting also calls
    `setAttribute('value', String(v))`; the spec deliberately keeps
    property/attribute coupled for simplicity.
  - `checked` (boolean, defaults `false`). Setting also calls
    `setAttribute('checked', '')` when truthy and `removeAttribute('checked')`
    when falsy.
  - `disabled`, `selected` — same coupling.
  - `name`, `type`, `placeholder`, `htmlFor` — plain string properties
    coupled with the matching attribute (`htmlFor` ↔ `for`).
- Move existing `id` and `href` getters into the same property block for
  consistency (no behavioral change).

**Tests** (`runtime/dom-shim.test.js`):

- `classList.add('a', 'b')` ⇒ `getAttribute('class') === 'a b'`.
- `classList.remove('a')` shrinks; `classList.contains('b')` true;
  `classList.toggle('c')` returns `true` and adds; second toggle returns
  `false` and removes.
- `classList.replace('b', 'x')` swaps in place.
- `dataset.fooBar = '5'` ⇒ `getAttribute('data-foo-bar') === '5'`.
- `delete el.dataset.fooBar` ⇒ `hasAttribute('data-foo-bar') === false`.
- `el.style.color = 'red'` ⇒ `getAttribute('style')` contains
  `color: red`.
- `el.style.setProperty('--x', '1')` ⇒ attribute serialization includes
  `--x: 1`.
- `setAttribute('style', 'color: red; --x: 1')` populates the style map
  (round trip via `getPropertyValue`).
- `textContent` getter walks; setter replaces children with one text node.
- `el.value = 'hi'` round-trips with `getAttribute('value')`.
- `el.checked = true` ⇒ `hasAttribute('checked') === true`;
  `el.checked = false` ⇒ attribute removed.
- `el.className = 'a b'` ⇒ `getAttribute('class') === 'a b'`.

---

### Step 3: Document additions

**Goal:** Wire up `documentElement` / `head` / `body`, `getElementById`,
`activeElement` tracking, and `document.title` so tests that touch global
document state (e.g. dark-mode `document.documentElement.dataset.theme`)
work. Comes after Steps 1 and 2 because `body` and friends are real
elements that need the new property surface.

**Files:**
- `runtime/dom-shim.js`
- `runtime/dom-shim.test.js`

**Changes:** In `runtime/dom-shim.js`:

- Eagerly construct three elements: `_documentElement = createElement('html')`,
  `_head = createElement('head')`, `_body = createElement('body')`.
- `_appendChild(_documentElement, _head); _appendChild(_documentElement, _body);`
- `_appendChild(document, _documentElement);` (uses existing
  `document.childNodes` array; `document` already has `appendChild`.)
- On the `document` object literal:
  - `get documentElement() { return _documentElement; }`
  - `get head() { return _head; }`
  - `get body() { return _body; }`
  - `getElementById(id)` — walks descendants of `_documentElement` and
    returns the first element whose `getAttribute('id') === id`. Uses
    the existing `_walkDescendants` helper.
  - `_activeElement: null` (private); `get activeElement() { return this._activeElement; }`.
  - `_title: ""` (private); `get title()` / `set title(v)` proxy to it.
- Extend `createElement`'s `focus()` to set
  `document._activeElement = this` and dispatch a `focus` event on the
  element (bubbles `false`; cancelable `false`). If `document._activeElement`
  was previously a different element, fire `blur` on it first.
- Extend `createElement`'s `blur()` to clear `document._activeElement` if
  it currently points at `this`, then dispatch a `blur` event.

**Tests** (`runtime/dom-shim.test.js`):

- `document.documentElement.tagName === 'HTML'`.
- `document.head.parentNode === document.documentElement`.
- `document.body.parentNode === document.documentElement`.
- `document.getElementById('x')` finds an element appended under `body`.
- After `el.focus()`, `document.activeElement === el`; `el.blur()`
  clears it.
- Focusing one element after another fires a `blur` on the first.
- `document.title = 'hi'` round-trips.

---

### Step 4: Web Storage

**Goal:** Provide working `localStorage` and `sessionStorage`. Independent
of Steps 1–3 mechanically, but ordered here so the cleanup-integration
test in Step 7 can rely on both timers and storage existing.

**Files:**
- `runtime/dom-shim.js`
- `runtime/dom-shim.test.js`

**Changes:** In `runtime/dom-shim.js`:

- Add `_makeStorage()` returning:
  ```
  function _makeStorage() {
    const map = new Map();
    return {
      getItem(k) { return map.has(String(k)) ? map.get(String(k)) : null; },
      setItem(k, v) { map.set(String(k), String(v)); },
      removeItem(k) { map.delete(String(k)); },
      clear() { map.clear(); },
      key(i) {
        let n = 0;
        for (const k of map.keys()) if (n++ === i) return k;
        return null;
      },
      get length() { return map.size; },
    };
  }
  ```
- Construct `const _localStorage = _makeStorage();` and
  `const _sessionStorage = _makeStorage();`.
- Install via `Object.defineProperty(globalThis, 'localStorage', { value: _localStorage, writable: true, configurable: true });`
  and same for `sessionStorage`, and also on `window`. Plain-data
  properties (no getter) so tests can reassign without `Object.defineProperty`.
- Export both as named exports from the module (`export const localStorage = ...`)
  so node:test files can import them when stubbing.

**Tests** (`runtime/dom-shim.test.js`):

- `setItem('a', 1)` then `getItem('a') === '1'` (note coercion).
- `removeItem('a')` then `getItem('a') === null`.
- `clear()` empties; `length` reflects size.
- `key(0)` returns the first inserted key.
- `localStorage` and `sessionStorage` do not share state (setting one
  doesn't affect the other).
- Reassignment works: `globalThis.localStorage = customFake` succeeds
  without throwing.

---

### Step 5: Auxiliary globals — `matchMedia`, `navigator`, `crypto`, Observers, `getComputedStyle`

**Goal:** Round out the global-surface stubs. None of these depend on
prior steps' code; ordered here so doc and timer steps come last.

**Step 0 probe result:** Boa 0.21.1 ships no `crypto` global at all
(`typeof globalThis.crypto === "undefined"`). The shim installs the full
object unconditionally — no `randomUUID`-detection gate is needed.

**Files:**
- `runtime/dom-shim.js`
- `runtime/dom-shim.test.js`

**Changes:** In `runtime/dom-shim.js`:

- `_makeMediaQueryList(query)` returning a real-EventTarget-shaped object:
  - `.media = query`, `.matches = false`, `.onchange = null`.
  - Internal `_listeners` map for `change` events.
  - `.addEventListener` / `.removeEventListener` (delegate to
    `_makeEventTarget`-style internals — share the same helper as
    `document` and `window`).
  - `.addListener(fn)` and `.removeListener(fn)` — legacy aliases that
    call through to `addEventListener('change', fn)` /
    `removeEventListener('change', fn)`.
  - `.dispatchEvent(event)` — fires registered handlers, plus
    `onchange` if set.
- `window.matchMedia = (query) => _makeMediaQueryList(query)`.
- `const _navigator = { userAgent: "zero-test-shim/1.0", language: "en-US", languages: ["en-US"], onLine: true, platform: "" };`
  installed on both `window.navigator` and `globalThis.navigator` (same
  instance, writable, configurable).
- `_makeCrypto()` returning `{ randomUUID(): string, getRandomValues(arr): typed }`.
  - `randomUUID()` uses `Math.random()`:
    ```
    const bytes = new Array(16);
    for (let i = 0; i < 16; i++) bytes[i] = Math.floor(Math.random() * 256);
    bytes[6] = (bytes[6] & 0x0f) | 0x40; // version 4
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // variant
    const hex = bytes.map(b => b.toString(16).padStart(2, '0')).join('');
    return `${hex.slice(0,8)}-${hex.slice(8,12)}-${hex.slice(12,16)}-${hex.slice(16,20)}-${hex.slice(20)}`;
    ```
  - `getRandomValues(arr)` fills with `Math.random()`-derived bytes per
    element size (treat each element as 4-byte for `Uint32Array`,
    1-byte for `Uint8Array`, etc.). Returns `arr`.
  - Code comment: "Not cryptographically strong; uses Math.random().
    Use case is store IDs, not secrets."
- Conditional install gated on the Step 0 probe result:
  ```
  if (typeof globalThis.crypto?.randomUUID !== "function") {
    globalThis.crypto = _makeCrypto();
  }
  ```
  Also expose on `window`.
- `_makeObserver(takeRecords)` factory returning a constructor:
  ```
  function _makeObserver(name) {
    return function Observer(callback) {
      this.callback = callback;
      this.observations = [];
      this.observe = (target, options) => { this.observations.push({ target, options }); };
      this.unobserve = (target) => { this.observations = this.observations.filter(o => o.target !== target); };
      this.disconnect = () => { this.observations.length = 0; };
      if (name === 'Mutation') this.takeRecords = () => [];
    };
  }
  globalThis.IntersectionObserver = _makeObserver('Intersection');
  globalThis.ResizeObserver = _makeObserver('Resize');
  globalThis.MutationObserver = _makeObserver('Mutation');
  ```
- `window.getComputedStyle = function(el, pseudo) {
    return {
      getPropertyValue: () => "",
      setProperty: () => { throw new Error("getComputedStyle result is read-only"); },
      length: 0,
    };
  };` (numeric index access returning `""` is left as a future
  enhancement — not load-bearing for shipped components.)

**Tests** (`runtime/dom-shim.test.js`):

- `matchMedia('(min-width: 800px)')` returns an object with
  `.matches === false`, `.media === '(min-width: 800px)'`.
- Reassigning `window.matchMedia` to a stub that returns
  `{matches: true, ...}` works without errors.
- `matchMedia('q').addEventListener('change', fn);
  matchMedia('q').dispatchEvent({type:'change'})` — note that each call
  returns a fresh MQL; the test fires on the same instance to verify
  the listener wiring.
- `navigator.userAgent === 'zero-test-shim/1.0'`; overrideable.
- `crypto.randomUUID()` returns a 36-char string with dashes at the
  expected positions and `4` at character 14.
- `crypto.getRandomValues(new Uint8Array(8))` returns the same typed
  array (length 8); values are numbers in range [0, 255].
- `new IntersectionObserver(cb).observe(el)` records the observation
  on `.observations`; `disconnect()` clears.
- `MutationObserver` has `takeRecords()` returning `[]`.
- `getComputedStyle(el).getPropertyValue('color') === ''`.

---

### Step 6: Rust-side timer host

**Goal:** Install `setTimeout` / `clearTimeout` / `setInterval` /
`clearInterval` / `requestAnimationFrame` / `cancelAnimationFrame` on the
Boa context's `globalThis` via the existing `NativeFunction` /
`ObjectInitializer` patterns. Also install `__clearAllTimers__` so
`cleanup()` (Step 7) can drain pending work between tests. Comes after
all the JS-side shim work because it requires the Rust changes are
landed and tested independently first.

**Files:**
- `crates/zero-test-runner/src/harness.rs`
- `crates/zero-test-runner/tests/timer_host.rs` (new file)

**Changes:**

In `harness.rs`:

- Add a `TimerHost` struct (placed near `install_console`) carrying a
  monotonic id counter and a pending map. Use `Rc<RefCell<...>>` for
  shared mutability across `NativeFunction` closures:
  ```
  use std::cell::RefCell;
  use std::collections::HashMap;
  struct TimerHostState {
    next_id: u32,
    pending: HashMap<u32, PendingTimer>,
  }
  struct PendingTimer {
    callback: JsValue,       // the JS function
    kind: TimerKind,         // Once | Interval | Raf
    cancelled: bool,
  }
  enum TimerKind { Once, Interval, Raf, }
  ```
- `fn install_timer_host(ctx: &mut Context)` creates one
  `Rc<RefCell<TimerHostState>>` and registers four
  `NativeFunction::from_copy_closure(move |this, args, ctx| { ... })`
  callbacks (the closure form captures the `Rc` clone).
- `setTimeout(fn, _ms)`:
  - Read `args[0]` as a JS function. Read `args[1]` as a number (ignored
    for ordering but parsed for compatibility).
  - Increment `next_id`; insert
    `PendingTimer { callback, kind: Once, cancelled: false }`.
  - Schedule the callback as a microtask: construct
    `JsPromise::resolve(JsValue::undefined(), ctx).then(callback, None, ctx)`.
    The `then` form runs the callback when the job queue drains.
    Wrap the callback so it checks the `cancelled` flag and short-circuits
    when true. Concretely: register an intermediate
    `NativeFunction::from_copy_closure` that reads the id from a
    captured `u32`, looks up the entry in `state.borrow_mut()`, returns
    early if cancelled or missing, otherwise calls the real `callback`
    and removes the entry.
  - Return `JsValue::Integer(id as i32)`.
- `clearTimeout(id)`:
  - Read `args[0]` as an integer id; if present in the map, set
    `cancelled = true` (and remove). No-op when absent.
  - Returns undefined.
- `setInterval(fn, ms)`:
  - Same insertion as Once, but `kind = Interval`. The wrapper, after
    invoking `callback`, re-enqueues itself (calls the same closure
    again with the same id, keeping `cancelled = false` until the user
    cancels). Pseudocode:
    ```
    fn enqueue(state, id, ctx) {
      JsPromise::resolve(JsValue::undefined(), ctx)
        .then(wrap_with_reenqueue(id, state), ...)
    }
    ```
- `clearInterval(id)`: same as `clearTimeout`.
- `requestAnimationFrame(fn)`:
  - Same as setTimeout, but the wrapped callback receives one numeric
    argument: a monotonic counter stored on `state`, advancing by 16
    each call.
  - Returns id.
- `cancelAnimationFrame(id)`: same as `clearTimeout`.
- `__clearAllTimers__()`: walks `state.pending` and sets every entry's
  `cancelled = true`. Used by `cleanup()` in Step 7.
- Call `install_timer_host(&mut context)` from `run_with_loader` right
  after `install_console`.

In `crates/zero-test-runner/tests/timer_host.rs`:

- New integration test, mirrors the in-module tests but lives in `tests/`:
  - `setTimeout_fires_after_run_jobs` — writes a tiny TS file that
    schedules `setTimeout(() => globalThis.__hit__ = 1, 0)` and
    awaits a `Promise.resolve()` before asserting `__hit__ === 1`.
    Runs through `run_file`; the outcome must be `Passed`.
  - `clearTimeout_cancels` — schedules, clears, then awaits; asserts
    the callback did **not** run.
  - `setInterval_repeats` — schedules an interval that increments a
    counter to 3 before clearing itself.
  - `__clearAllTimers___drains_pending` — schedules several timers,
    calls `__clearAllTimers__()`, awaits, asserts none fired.
- Delete the temporary `host_microtask_runs_after_run_jobs` probe from
  Step 0.

**Tests:** The integration suite above plus the existing harness suite
must remain green. Run `cargo test -p zero-test-runner` end of step.

**Notes / risks:** The Boa `Promise.then` continuation pattern is the
core mechanism. If `then(callback, None, ctx)` does not accept a raw
`JsValue` callback in 0.21.1, fall back to constructing a thenable
via `JsPromise::new(executor, ctx)` and calling `resolve()` from a
host closure. Document the chosen approach in a code comment so future
maintainers don't have to re-derive it.

---

### Step 7: `cleanup()` extensions in `runtime/test.js`

**Goal:** Tie everything together: per-test mutable shim state resets
between `it`s. Comes after Step 6 so the `__clearAllTimers__` symbol
exists.

**Files:**
- `runtime/test.js`
- `runtime/test.test.js`

**Changes:** Update `cleanup()` in `runtime/test.js`:

```js
export function cleanup() {
  // 1. Existing: dispose render-tracked scopes first so their cleanup
  //    callbacks can still touch storage / timers if they want.
  for (const { scope } of _renderTracker) scope.dispose();
  _renderTracker.length = 0;
  _setCurrentApp(null);

  // 2. Clear storage.
  if (typeof localStorage !== "undefined") localStorage.clear();
  if (typeof sessionStorage !== "undefined") sessionStorage.clear();

  // 3. Cancel pending timers (Boa-only; absent under node:test).
  if (typeof globalThis.__clearAllTimers__ === "function") {
    globalThis.__clearAllTimers__();
  }

  // 4. Reset per-document mutable state.
  if (typeof document !== "undefined") {
    document._activeElement = null;
    document._title = "";
    // Empty body, head, documentElement.
    for (const root of [document.body, document.head, document.documentElement]) {
      if (root && root.childNodes) root.childNodes.length = 0;
    }
  }
}
```

Order matters and is enforced by the comments: dispose scopes → clear
storage → cancel timers → reset document fields. Each block is feature-
detected so this same function works in both node:test and Boa.

**Tests** (`runtime/test.test.js`):

- After `cleanup()`, `localStorage.length === 0`. Pre-populate via
  `localStorage.setItem('a', '1')` first.
- After `cleanup()`, `document.title === ''`. Pre-populate first.
- After `cleanup()`, `document.activeElement === null`. Pre-focus an
  element first.
- After `cleanup()`, `document.body.childNodes.length === 0`. Append a
  child first.
- The existing render-tracking test still passes (regression).

---

### Step 8: Spec / doc amendments

**Goal:** Sync the prose specs with the new shim surface. No code
behavior changes here.

**Files:**
- `zero-framework-spec.md`
- `BEST_PRACTICES.md`

**Changes:**

- `zero-framework-spec.md` §8 (Testing — "No Browser Required"):
  amend the line "supports only the DOM APIs that z's template system
  uses" to mention storage, timers, `matchMedia`, observers, and real
  events. Update the LOC estimate to the post-change file size.
- `zero-framework-spec.md` §11 (Complete API Surface): no changes —
  nothing new is exported from `"zero"` or `"zero/test"`.
- `zero-framework-spec.md` §12 Phase list: add a new entry "Phase N —
  DOM shim expansion" pointing at `issues/dom-shim/spec.md`.
- `BEST_PRACTICES.md`: add a short "Testing browser APIs" paragraph
  pointing readers at the real shim for `localStorage`, `matchMedia`,
  timers, and the `spy()`-wrapping pattern for asserting calls (e.g.
  `localStorage.setItem = spy(localStorage.setItem.bind(localStorage))`).

**Tests:** No tests — text-only changes. Verify with `git diff` and
re-read the amended sections for accuracy.

---

## Risks and Assumptions

1. **Boa `Promise.then` callback signature in 0.21.1.** Step 6's timer
   host hinges on enqueueing a JS callback as a microtask. The signature
   used by other parts of the harness (`run_jobs()` for Promises produced
   by user code) should generalize, but `then(callback_value, None, ctx)`
   may not accept a raw `JsValue`. If so, fall back to
   `JsPromise::new(executor, ctx)` where the executor's resolve immediately
   schedules the callback — same observable effect, slightly more
   ceremony. The Step 0 prototype shakes this out before commitment.

2. **Job-queue drain frequency.** The harness already drains in two places
   (`evaluate_module` and `call_and_drain`'s Promise loop). Timer-driven
   user code that does not `await` anything will not run — but the spec
   acknowledges this: "Tests that await a Promise that resolves after a
   0ms timer work without explicit advance-time calls." Pure
   `setTimeout(fn, 0)` followed by synchronous `expect(...)` will not
   work; tests must either `await Promise.resolve()` (which drains the
   queue) or be async. Document this in the timer-host code comment.

3. **Shim object identity vs. mockability invariant.** The spec demands
   every shim method be a plain function property (not on a prototype)
   so tests can reassign with `localStorage.setItem = spy(...)`. The plan
   honors this for storage, matchMedia, and crypto. The element-level
   property surface (Step 2) uses `Object.defineProperty` per-element
   *not* per-prototype, so per-element override remains trivial.

4. **Event class detection.** Components may use
   `event instanceof MouseEvent`. The plan installs `MouseEvent` as a
   constructor-returning-plain-object — `instanceof` will fail. If
   real-world apps hit this, the constructors can later be promoted to
   `class` form; for this slice we accept that limitation (none of the
   shipped components in `examples/` rely on `instanceof`).

5. **Style attribute parsing edge cases.** Quoted values
   (`content: "foo;bar"`), `!important`, and CSS comments are not
   supported. The component surface in the repo doesn't use these; if a
   future component does, the parser grows in a separate slice. Plan
   uses split-on-`;` then split-on-`:` with `trim()` — known incorrect
   for quoted values; documented in code comment.

6. **`document.documentElement` already used by reactivity code?** Quick
   grep before Step 3 to confirm we are not shadowing a previously-set
   property. (`grep -n documentElement runtime/*.js` returned nothing,
   so safe.) If something is later added that pre-creates
   `documentElement`, the eager construction in Step 3 must move to a
   lazy getter.

7. **Existing tests rely on `document.childNodes.length = 0` for reset**
   (see `runtime/test.test.js:29`). After Step 3, `document.childNodes`
   starts with one entry (`_documentElement`), not zero. Update the
   existing `beforeEach` in `test.test.js` to either skip that reset (the
   new `cleanup()` covers it) or reset to `[_documentElement]`. This is
   a small fix to land alongside Step 3 — flag it in the step's "Files"
   if missed.
