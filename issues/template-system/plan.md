# Plan: Template System

## Summary

Build Phase 2 of the zero runtime: the `html` tagged template, the `commit()`
function that mounts a `TemplateResult` to real DOM, and the supporting
primitives `each()` and `ref()`. A minimal DOM shim (`runtime/dom-shim.js`)
is built alongside so tests can run in plain Node with `node:test` and zero
npm dependencies, matching Phase 1's harness.

The approach: tag-template parsing happens once per call-site (cached by the
`strings` array reference). The parser is a hand-rolled character-level state
machine that emits a cloneable `DocumentFragment` skeleton plus a `Part[]`
descriptor array. At commit time, the fragment is cloned and each `Part` is
wired up — primitives set directly, signals and reactive functions wrapped in
`effect()` so they auto-update under the currently active scope. Event
modifiers are pre-parsed at parse time, not commit time. All effects created
during `commit()` run in whatever scope is active on the caller — `commit()`
does not create its own scope.

## Prerequisites

Two open questions from the spec are resolved here so execution does not stall:

1. **Node-part bookkeeping across re-evaluations.** Each `node` part keeps a
   per-instance `currentNodes: Node[]` list and a `childScope` reference.
   On every reactive re-evaluation, the existing nodes are removed from the
   DOM, the child scope is disposed, then the new value is committed into a
   fresh child scope and its inserted nodes are recorded. The Comment anchor
   for the part is never removed — it's the stable insertion point that
   `parentNode.insertBefore(node, anchor)` references.

2. **`each()` return shape.** `each(sig, renderFn)` returns a tagged marker
   object: `{ _isEach: true, signal, renderFn }`. The node-part commit logic
   detects this marker by property check and handles it with dedicated
   per-item child-scope wiring. It is **not** a generic `() => ...` reactive
   block — `each()` needs explicit per-item scopes that the reactive-block
   path does not provide.

3. **Simplification: drop the `text` part type for Phase 2.** The spec lists
   both `text` and `node` part types, but the parser cannot statically
   determine whether a placeholder will receive a primitive or a
   `TemplateResult`. Phase 2 emits **only** `node` parts for in-content
   placeholders (Comment anchor). The node-part commit handles primitives by
   inserting a Text child of the anchor's region; this matches the spec's
   value-type table for node parts. The `text` part as a perf optimization
   is deferred — its absence does not change observable behavior.

## Steps

- [x] **Step 1: DOM shim**
- [x] **Step 2: `html` tagged template and parser**
- [x] **Step 3: `commit()` for attr parts**
- [x] **Step 4: `commit()` for node parts (TR, array, primitives, reactive)**
- [x] **Step 5: Event bindings with modifiers**
- [x] **Step 6: `ref()`**
- [x] **Step 7: `each()`**

---

## Step Details

### Step 1: DOM shim

**Goal:** Provide the minimal DOM surface the template system needs, so
`runtime/template.js` can be exercised in Node with `node:test`. The shim
must support every API the parser and commit logic call.

**Files:**
- `runtime/dom-shim.js` — new file

**Changes:**

Implement a self-contained ES module exporting a single `document` object
and the four node-type constructors used internally. The module **also has
a side-effect**: if `globalThis.document` is unset, install the shim's
`document` there. This lets `runtime/template.js` reference bare `document`
(resolving via globalThis) without importing the shim, matching the spec's
"production uses real document, tests use the shim" contract.

Node hierarchy (factory functions, not classes — keep it plain):

```js
// runtime/dom-shim.js

const ELEMENT_NODE = 1;
const TEXT_NODE = 3;
const COMMENT_NODE = 8;
const DOCUMENT_FRAGMENT_NODE = 11;

function _baseNode(nodeType) {
  return {
    nodeType,
    parentNode: null,
    childNodes: [],
    // appendChild, insertBefore, removeChild defined per-node where applicable
  };
}

function createElement(tagName) { /* ... */ }
function createTextNode(text)   { /* ... */ }
function createComment(data)    { /* ... */ }
function createDocumentFragment() { /* ... */ }
```

Each node implements at least these properties/methods (per spec):

- **Element**: `tagName` (uppercase), `nodeName` (alias of tagName),
  `attributes` (Map), `childNodes` (Array), `parentNode`, `nodeType`,
  `setAttribute(name, value)`, `removeAttribute(name)`,
  `getAttribute(name)`, `hasAttribute(name)`,
  `addEventListener(event, handler)`, `removeEventListener(event, handler)`,
  `appendChild(child)`, `insertBefore(child, ref)`, `removeChild(child)`,
  `cloneNode(deep)`, `firstChild`/`lastChild`/`nextSibling`/`previousSibling`
  (getters computed from parent's `childNodes`).
- **Text**: `nodeValue` (read/write), `data` (alias of `nodeValue`),
  `parentNode`, `nodeType`, `cloneNode()`, sibling getters.
- **Comment**: `data`, `nodeValue` (alias), `parentNode`, `nodeType`,
  `cloneNode()`, sibling getters.
- **DocumentFragment**: `childNodes`, `appendChild`, `insertBefore`,
  `removeChild`, `cloneNode(deep)`, `nodeType`, sibling getters where
  meaningful.

Shared helpers:
- `_appendChild(parent, child)` — detaches `child` from its current parent
  first (handles re-parenting), then pushes to `parent.childNodes` and
  sets `child.parentNode = parent`.
- `_insertBefore(parent, child, ref)` — detaches `child`, finds `ref`'s
  index in `parent.childNodes`, splices `child` in, sets `parentNode`. If
  `ref` is `null`, equivalent to `appendChild`.
- `_removeChild(parent, child)` — splices `child` out of `parent.childNodes`,
  sets `child.parentNode = null`.
- `_cloneNode(node, deep)` — produces a new node of the same type with the
  same shallow state (Element: same tag + attributes; Text/Comment: same
  data; Fragment: empty). When `deep`, recursively clones children and
  appends them via the helpers (so `parentNode` chains are correct on the
  clone). **Event listeners and ref state are NOT copied** — matches DOM
  spec for `cloneNode`.

Event dispatch: not required by Phase 2's source code (the framework only
*registers* listeners). The test surface needs to fire events at elements
to verify event-modifier behavior, so add a single `dispatchEvent(event)`
method on Element that synchronously calls every matching listener with the
event object, respecting `once` flag (remove after first fire). The event
object is the literal value passed in — no `Event` class, just a plain
object like `{ type: 'click', preventDefault() {}, stopPropagation() {}, ... }`.

Export shape:

```js
export const document = {
  createElement,
  createTextNode,
  createComment,
  createDocumentFragment,
};

if (typeof globalThis.document === 'undefined') {
  globalThis.document = document;
}
```

**Tests:** Covered indirectly by `runtime/template.test.js` in later steps.
A dedicated `dom-shim.test.js` is not in the spec's file layout; rely on
the template tests to exercise every shim API. To verify Step 1 in isolation
before Step 2 lands, run a one-off smoke check from the REPL or a scratch
file (not committed): `node -e "import('./runtime/dom-shim.js').then(m => { const el = m.document.createElement('div'); el.setAttribute('x','1'); console.log(el.getAttribute('x')); })"`.

If a `runtime/dom-shim.test.js` is desired for confidence, add ~6 tests
covering: element creation + attributes, text/comment creation + data,
appendChild parentNode wiring, insertBefore semantics, cloneNode(deep)
copies children but not listeners, dispatchEvent fires listeners and
respects once. This is optional.

---

### Step 2: `html` tagged template and parser

**Goal:** Implement `html(strings, ...values)` so it returns a
`TemplateResult` referencing a cached `Template` (parsed once per call
site). No `commit()` yet — this step verifies the parser produces the right
fragment skeleton and `Part[]` descriptors. Steps 3–7 build commit on top.

**Files:**
- `runtime/template.js` — new file
- `runtime/template.test.js` — new file

**Changes:**

Top of `runtime/template.js`:

```js
import { effect } from './reactivity.js';
// NOTE: `document` is read from the global scope at call time, not imported.

const _templateCache = new WeakMap(); // strings array → Template
```

Public `html`:

```js
export function html(strings, ...values) {
  let template = _templateCache.get(strings);
  if (!template) {
    template = _parseTemplate(strings);
    _templateCache.set(strings, template);
  }
  return { _template: template, _values: values };
}
```

`Template` shape:

```js
// { fragment: DocumentFragment, parts: Part[] }
// parts.length === values.length, parts are ordered by ${} appearance.
```

`Part` shapes (only `attr`, `event`, `ref`, `node` for Phase 2; no `text`):

```js
// { type: 'attr',  path: number[], name: string }
// { type: 'event', path: number[], event: string, modifiers: string[] }
// { type: 'ref',   path: number[] }
// { type: 'node',  path: number[] }   // path locates the anchor Comment in the fragment
```

`path` is an array of child-index integers, used at commit time to walk from
the cloned fragment to the target node (`fragment` → `childNodes[path[0]]`
→ `childNodes[path[1]]` → …). This decouples parse-time and commit-time —
the parsed `Template` does not hold references to nodes inside the
**original** fragment (only to the fragment itself, which gets cloned).

#### `_parseTemplate(strings)` — the state machine

Walk every character of every static string. Between strings (at each
boundary `i → i+1`) determine the placeholder context based on the parser's
current state.

States:

- `TEXT` — outside any tag (in element content).
- `TAG_OPEN` — just consumed `<`, expecting tag name.
- `TAG_NAME` — collecting `tagName` (letters/digits/`-`).
- `IN_TAG` — between attributes (whitespace inside an open tag).
- `ATTR_NAME` — collecting attribute name.
- `AFTER_ATTR_NAME` — saw whitespace or `=` after attribute name.
- `ATTR_VALUE_UNQUOTED` — collecting unquoted attribute value.
- `ATTR_VALUE_DQ` — collecting `"`-quoted attribute value.
- `ATTR_VALUE_SQ` — collecting `'`-quoted attribute value.
- `CLOSING_TAG` — just consumed `</`, collecting close-tag name.

While walking, the parser maintains:

- `parent: Element | DocumentFragment` — current open element (top of
  element stack).
- `elementStack: Element[]` — push on open-tag-end, pop on close-tag.
- `parts: Part[]` — accumulating output.
- `currentAttrName: string` — when in `ATTR_NAME` / `AFTER_ATTR_NAME` /
  `ATTR_VALUE_*`.
- `path: number[]` — running record of child indices from the fragment down
  to `parent`. (Updated when entering/leaving elements; combined with
  `parent.childNodes.length` to produce a child-index path for each placeholder.)

At each string-to-string boundary (between `strings[i]` and `strings[i+1]`),
record a `Part` whose `path` is derived from `parent` and its
`childNodes.length` at that moment:

- **State is `TEXT`** → emit a `node` part. Create a `Comment('')` anchor,
  append to `parent`, compute `path` to that comment, push a part.
- **State is `ATTR_VALUE_DQ` or `ATTR_VALUE_SQ` or `ATTR_VALUE_UNQUOTED`**
  → classify by `currentAttrName`:
    - Starts with `@` → emit `event` part. Split on `.` to get event name
      (after `@`) and modifiers (rest). Do **not** add this attribute to
      the element.
    - Exactly `ref` (case-sensitive) → emit `ref` part. Do not add
      attribute.
    - Otherwise → emit `attr` part with `name: currentAttrName`. Do not
      add the attribute to the element (it will be set or removed at
      commit time).
- Other states are programmer errors (e.g., placeholder inside a tag name,
  inside `<!-- comment -->`, or in a quoted-but-mid-string position). For
  Phase 2, treat these as fatal: `throw new Error('html: placeholder in
  unsupported position')`. Document the supported subset.

The character-walk handles standard HTML construct edges:

- `<` outside a quoted attribute → enter `TAG_OPEN` (if next is a letter)
  or `CLOSING_TAG` (if next is `/`).
- `>` in `IN_TAG` / `ATTR_NAME` / `AFTER_ATTR_NAME` /
  `ATTR_VALUE_UNQUOTED` → close the open tag, push element onto stack,
  enter `TEXT`. For self-closing (`/>`), emit but immediately pop.
- `/>` → self-closing: close element without pushing onto stack.
- `</tagname>` → pop element stack, return to `TEXT`.
- Whitespace handling inside tags: separator between attributes.
- Quoted attribute values: `"..."` or `'...'`, terminated by the matching
  quote.
- Attribute names: collected until `=`, whitespace, or `>`.
- Static text outside tags: append as Text node children of `parent`.
- HTML escaping: spec-compliant escaping (`&amp;` etc.) is **not** required
  for Phase 2 — write literal characters into Text nodes' `nodeValue` as
  the user wrote them. Document this as a Phase 2 deliberate omission.

#### Path computation

When emitting a part, `path` is built by walking the element stack:

```js
// `parent` is the current open element (or fragment if at top level).
// childIndex is `parent.childNodes.length - 1` if we just appended
// an anchor for a node-part, or `parent.childNodes.length - 1` (the
// open element itself, relative to its grandparent) for attr/event/ref.
```

For attr/event/ref parts, the `path` points at the **element** carrying the
attribute. For node parts, the `path` points at the **comment anchor**.

The walker maintains a parallel `parentPath: number[]` that mirrors the
element stack — pushing an entry when entering a child element, popping on
close. New parts copy `parentPath` and append the child-index of the
relevant node.

**Tests (`runtime/template.test.js`):**

- `html` returns an object with `_template` and `_values`.
- `_values` matches the dynamic args in order.
- Same call site twice → same `_template` reference (cache hit). Verified
  by calling a small helper twice and asserting `===` on `_template`.
- Parser smoke tests using inspection of `_template.fragment` and
  `_template.parts`:
    - `html\`<div>Hello</div>\`` → fragment has one div child with one
      text-node child "Hello", zero parts.
    - `html\`<p>Count: ${0}</p>\`` → fragment has one p child; p has two
      children (Text "Count: ", Comment anchor); one node part at the
      anchor.
    - `html\`<button class=${'x'}>go</button>\`` → fragment has one button
      with no `class` attribute set (placeholder-bound), text child "go";
      one attr part with `name: 'class'`.
    - `html\`<button @click.prevent.stop=${() => {}}>x</button>\`` → one
      event part with `event: 'click'`, `modifiers: ['prevent', 'stop']`,
      and no `@click.prevent.stop` attribute on the element.
    - `html\`<input ref=${{}} />\`` → one ref part, no `ref` attribute set,
      self-closing handled correctly.
    - Cache test: define a function returning `html\`<div>${'x'}</div>\``,
      call it twice, assert the two results share `._template`.

These tests do not call `commit()` — they only inspect the parsed structure.
Set `globalThis.document` from `dom-shim.js` at the top of the test file
before any `html()` call.

---

### Step 3: `commit()` for attr parts

**Goal:** Make `commit(templateResult, container)` work end-to-end for the
attribute slot — primitives and reactive values. Node parts are stubbed
(error or skipped). Event and ref parts likewise stubbed. This lets us
verify the cloning + path-walking infrastructure before tackling node-part
bookkeeping.

**Files:**
- `runtime/template.js` — add `commit` and `_commitAttr` helpers
- `runtime/template.test.js` — add tests

**Changes:**

```js
export function commit(templateResult, container) {
  const { _template, _values } = templateResult;
  const clone = _cloneFragment(_template.fragment);

  for (let i = 0; i < _template.parts.length; i++) {
    const part = _template.parts[i];
    const target = _walkPath(clone, part.path);
    const value = _values[i];

    switch (part.type) {
      case 'attr':  _commitAttr(target, part.name, value); break;
      case 'event': /* Step 5 */ break;
      case 'ref':   /* Step 6 */ break;
      case 'node':  /* Step 4 */ break;
    }
  }

  container.appendChild(clone);
}

function _walkPath(root, path) {
  let node = root;
  for (const i of path) node = node.childNodes[i];
  return node;
}

function _cloneFragment(fragment) {
  return fragment.cloneNode(true);
}
```

`_commitAttr(el, name, value)`:

```js
function _commitAttr(el, name, value) {
  if (_isSignal(value)) {
    effect(() => _applyAttr(el, name, value.val));
  } else if (typeof value === 'function') {
    effect(() => _applyAttr(el, name, value()));
  } else {
    _applyAttr(el, name, value);
  }
}

function _applyAttr(el, name, v) {
  if (v === false || v == null) el.removeAttribute(name);
  else if (v === true)          el.setAttribute(name, '');
  else                          el.setAttribute(name, String(v));
}

function _isSignal(v) {
  return v != null && typeof v === 'object' && 'val' in v &&
         typeof v.set === 'function';
}
```

`_isSignal` duck-types: `signal` exports return objects with `.val` getter
and `.set` method; `computed` exports return objects with `.val` getter only
and **no** `.set`. We must treat both as reactive in attr parts. Adjust:

```js
function _isReactiveContainer(v) {
  return v != null && typeof v === 'object' &&
         Object.getOwnPropertyDescriptor(
           Object.getPrototypeOf(v) || v, 'val'
         )?.get != null;
}
```

Actually, since `signal()` and `computed()` return plain objects with `val`
as a getter, the safest portable check is:

```js
function _isReactive(v) {
  if (v == null || typeof v !== 'object') return false;
  // Walk the property descriptors to find a `val` getter.
  const desc = Object.getOwnPropertyDescriptor(v, 'val');
  return !!desc && typeof desc.get === 'function';
}
```

Phase 1's `signal` and `computed` both use object literals with `get val()`,
so `Object.getOwnPropertyDescriptor(v, 'val')` returns a descriptor with
`.get`. Use this check throughout Step 3+.

**Tests:**

- Static attribute: `html\`<div class=${'a'}></div>\`` committed into a
  fragment container — the div has `class="a"`.
- `false` removes the attribute, `true` sets to `""`.
- `null` / `undefined` remove the attribute.
- Signal: `const c = signal('a')`; `commit(html\`<div class=${c}></div>\`, …)`;
  inspect; then `c.set('b')` and inspect again — attribute updates.
- Reactive function: `commit(html\`<div data-x=${() => some.val * 2}></div>\`, …)`
  — value reflects formula and updates when dependency changes.
- Commit runs inside an active scope: wrap commit in
  `_createScope().run(() => commit(...))`, then dispose the scope and
  confirm signal updates no longer mutate the DOM (the underlying effect
  was disposed).

---

### Step 4: `commit()` for node parts

**Goal:** Implement node-part commit covering every case from the spec's
value-type table: primitives, signals, reactive functions, `TemplateResult`,
arrays of any of the above, and the `null/undefined` "render nothing" case.
Establish the bookkeeping that supports transitioning between value types
on reactive re-evaluation (the spec's open question).

**Files:**
- `runtime/template.js` — add `_commitNode` and helpers
- `runtime/template.test.js` — add tests

**Changes:**

Each node part owns:

- `anchor: Comment` — stable insertion marker, set up at parse time and
  located via path-walk.
- `currentNodes: Node[]` — nodes currently inserted **immediately before**
  `anchor`'s next sibling — i.e., between `anchor` and `anchor.nextSibling`.

Hmm — for unambiguous bookkeeping, the cleanest model is: nodes are
inserted **after** the anchor. So `currentNodes` are positioned in the
parent's `childNodes` at indices `[anchorIndex + 1 .. anchorIndex + N]`.
We track them explicitly. On update, we remove the tracked nodes and
insert new ones using `parentNode.insertBefore(newNode, anchor.nextSibling)`
in order.

```js
function _commitNode(anchor, value) {
  const state = { currentNodes: [], childScope: null };
  _applyNodeValue(anchor, value, state);
  return state;
}

function _applyNodeValue(anchor, value, state) {
  // Tear down any nodes/scope from a previous update.
  _clearNodeSlot(anchor, state);

  if (value == null) {
    // render nothing
    return;
  }

  if (_isReactive(value)) {
    // Signal — wrap in effect that re-applies on change.
    effect(() => _applyNodeValueLeaf(anchor, value.val, state));
    return;
  }

  if (typeof value === 'function') {
    // Reactive block — wrap in effect calling fn().
    if (value._isEach) {
      // Step 7 — for now, error or skip; tests for Step 4 do not pass an each.
      throw new Error('each() not implemented in this step');
    }
    effect(() => _applyNodeValueLeaf(anchor, value(), state));
    return;
  }

  _applyNodeValueLeaf(anchor, value, state);
}
```

`_applyNodeValueLeaf` handles a settled (non-reactive) value:

```js
function _applyNodeValueLeaf(anchor, value, state) {
  // For reactive callers (signal/fn), this is called on every re-run.
  // Tear down only the *content* (currentNodes + childScope), not the outer
  // wrapping effect.
  _clearNodeContent(state);

  if (value == null) return;

  if (Array.isArray(value)) {
    for (const item of value) _appendNodeItem(anchor, item, state);
    return;
  }

  _appendNodeItem(anchor, value, state);
}

function _appendNodeItem(anchor, value, state) {
  if (value == null) return;

  if (_isTemplateResult(value)) {
    // Commit into a synthetic container (a DocumentFragment), then move
    // its children to live just after `anchor`. Track them in state.
    const frag = document.createDocumentFragment();
    commit(value, frag);
    while (frag.childNodes.length > 0) {
      const node = frag.childNodes[0];
      anchor.parentNode.insertBefore(node, _nextSiblingAfter(anchor, state));
      state.currentNodes.push(node);
    }
    return;
  }

  // Primitives: string, number — insert a Text node.
  const text = document.createTextNode(String(value));
  anchor.parentNode.insertBefore(text, _nextSiblingAfter(anchor, state));
  state.currentNodes.push(text);
}

function _isTemplateResult(v) {
  return v != null && typeof v === 'object' &&
         v._template != null && Array.isArray(v._values);
}

function _nextSiblingAfter(anchor, state) {
  // Insertion goes immediately after the last currentNode (or after anchor
  // if no current content). Returns the reference node for insertBefore.
  if (state.currentNodes.length === 0) return anchor.nextSibling;
  const last = state.currentNodes[state.currentNodes.length - 1];
  return last.nextSibling;
}

function _clearNodeContent(state) {
  for (const node of state.currentNodes) {
    if (node.parentNode) node.parentNode.removeChild(node);
  }
  state.currentNodes.length = 0;
}

function _clearNodeSlot(anchor, state) {
  _clearNodeContent(state);
  // Step 6/7 will extend this to dispose child scopes too.
}
```

Wire `_commitNode` into the switch in `commit`:

```js
case 'node': _commitNode(target, value); break;
```

Note: the `state` object created inside `_commitNode` is scoped to the
closure of the outer call. The effect created when `value` is reactive
captures `state` by closure, so subsequent re-runs operate on the same
`currentNodes` list and accumulate cleanly.

**Tests:**

- Static `TemplateResult` value: `html\`<div>${html\`<span>x</span>\`}</div>\``
  — commit; div has one child span with text "x".
- Static array: `html\`<ul>${['a','b','c']}</ul>\`` — ul has three Text
  children.
- Primitive: `html\`<p>${5}</p>\`` — p has one Text child "5".
- `null` / `undefined`: `html\`<p>${null}</p>\`` — p has zero non-anchor
  children after the anchor.
- Signal of primitive: `const n = signal(1); commit(html\`<p>${n}</p>\`, …); n.set(2);`
  — text updates from "1" to "2".
- Reactive function returning TR: `() => html\`<span>${n.val}</span>\`` —
  on `n.set(...)`, the span is rebuilt with the new value.
- Transition across types: a single signal `v = signal('a')` cycling
  `'a' → null → ['x','y'] → html\`<i>z</i>\` → 'final'` — verify only the
  current value's nodes are present after each cycle (no leftover nodes
  from prior values).
- Nested commit: a TemplateResult containing a TemplateResult — both
  render correctly, attributes wire up.

---

### Step 5: Event bindings with modifiers

**Goal:** Wire up event parts at commit time. Modifiers were parsed and
stored in the part descriptor at parse time (Step 2). Commit wraps the
user's handler to apply modifier behavior, registers the listener, and
arranges for `removeEventListener` to run when the owning scope disposes.

**Files:**
- `runtime/template.js` — add `_commitEvent` helper
- `runtime/template.test.js` — add tests

**Changes:**

```js
const KEY_MODIFIERS = {
  enter:  'Enter',
  escape: 'Escape',
  space:  ' ',
  tab:    'Tab',
  up:     'ArrowUp',
  down:   'ArrowDown',
  left:   'ArrowLeft',
  right:  'ArrowRight',
};

function _commitEvent(el, eventName, modifiers, handler) {
  const wrapped = _wrapEventHandler(modifiers, handler);
  const options = modifiers.includes('once') ? { once: true } : undefined;
  el.addEventListener(eventName, wrapped, options);

  // Cleanup: run when the owning scope disposes. effect() with no signal
  // reads still registers with the active scope and runs its cleanup on
  // disposal. We rely on effect's cleanup-return mechanism.
  effect(() => () => el.removeEventListener(eventName, wrapped, options));
}

function _wrapEventHandler(modifiers, handler) {
  const keyFilters  = modifiers.filter(m => m in KEY_MODIFIERS);
  const hasPrevent  = modifiers.includes('prevent');
  const hasStop     = modifiers.includes('stop');
  const throttleMs  = modifiers.includes('throttle')  ? 100 : 0;
  const debounceMs  = modifiers.includes('debounce')  ? 100 : 0;

  let baseHandler = (e) => {
    if (keyFilters.length > 0 && !keyFilters.some(m => e.key === KEY_MODIFIERS[m])) return;
    if (hasPrevent) e.preventDefault?.();
    if (hasStop)    e.stopPropagation?.();
    return handler(e);
  };

  if (throttleMs > 0) baseHandler = _throttle(baseHandler, throttleMs);
  if (debounceMs > 0) baseHandler = _debounce(baseHandler, debounceMs);

  return baseHandler;
}

function _throttle(fn, ms) {
  let last = 0;
  return (...args) => {
    const now = Date.now();
    if (now - last < ms) return;
    last = now;
    return fn(...args);
  };
}

function _debounce(fn, ms) {
  let timer;
  return (...args) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn(...args), ms);
  };
}
```

The throttle/debounce default interval (100ms) is chosen because the spec
does not specify a syntax for the user to provide one. Document this as a
limitation; future work could extend modifier syntax (`@scroll.throttle:200`)
to make it configurable.

Wire into the commit switch:

```js
case 'event':
  _commitEvent(target, part.event, part.modifiers, value);
  break;
```

**Tests:**

- Basic `@click=${handler}`: commit, dispatch a click via shim's
  `dispatchEvent`, handler called.
- `.prevent`: pass an event object with a spy `preventDefault`; verify it
  was called.
- `.stop`: same for `stopPropagation`.
- `.once`: dispatch twice; handler called once. Verify shim's once handling.
- `.enter`: dispatch with `{ key: 'Enter' }` — handler called; dispatch
  with `{ key: 'a' }` — not called.
- Combined `.enter.prevent`: filtered to Enter AND preventDefault called.
- Cleanup on scope dispose: commit inside a scope, dispose scope, dispatch
  event — handler is **not** called (listener removed).

---

### Step 6: `ref()`

**Goal:** Implement `ref()` so `ref=${myRef}` populates `myRef.el` at commit
time and clears it back to `null` on scope disposal.

**Files:**
- `runtime/template.js` — add `ref` export and `_commitRef` helper
- `runtime/template.test.js` — add tests

**Changes:**

```js
export function ref() {
  return { el: null };
}

function _commitRef(el, refObj) {
  refObj.el = el;
  effect(() => () => { refObj.el = null; });
}
```

Wire into the commit switch:

```js
case 'ref': _commitRef(target, value); break;
```

Robustness: if the user passes something that is not a `{ el: ... }`
object (e.g., a plain function), throw a descriptive error. Phase 2 keeps
this minimal — accept any object with an `el` writable property, ignore
malformed values silently for now.

**Tests:**

- `const r = ref(); commit(html\`<input ref=${r} />\`, frag);` — `r.el`
  is the input Element.
- Dispose: commit inside scope, dispose, `r.el` is `null`.
- Multiple refs in one template: each populated correctly.

---

### Step 7: `each()`

**Goal:** Implement unkeyed list rendering with per-item child scopes.
Each item's effects (set up inside its `renderFn`) are owned by its own
child scope and disposed when the array changes or the parent scope disposes.

**Files:**
- `runtime/template.js` — add `each` export and the each-marker handling in
  `_applyNodeValue`
- `runtime/template.test.js` — add tests

**Changes:**

```js
export function each(sig, renderFn) {
  return { _isEach: true, signal: sig, renderFn };
}
```

Update `_applyNodeValue` in Step 4 to detect the each marker on the value
**before** the generic function/reactive checks. Note: `each()` returns an
object, not a function — order of detection doesn't matter as long as it
runs before the `Array.isArray` and `_isTemplateResult` checks (since the
marker is neither).

```js
function _applyNodeValue(anchor, value, state) {
  _clearNodeSlot(anchor, state);

  if (value == null) return;

  if (value && value._isEach) {
    _commitEach(anchor, value, state);
    return;
  }

  if (_isReactive(value))     { /* effect → leaf */ return; }
  if (typeof value === 'function') { /* reactive block */ return; }
  _applyNodeValueLeaf(anchor, value, state);
}
```

`_commitEach`:

```js
function _commitEach(anchor, eachMarker, state) {
  const { signal: arrSig, renderFn } = eachMarker;
  // Per-item child scopes so disposing the whole each on re-evaluation
  // tears down each item's effects.
  state.itemScopes = state.itemScopes || [];

  effect(() => {
    _disposeItemScopes(state);
    _clearNodeContent(state);

    const items = arrSig.val;
    if (!Array.isArray(items)) return;

    for (let i = 0; i < items.length; i++) {
      const scope = _createScope();
      state.itemScopes.push(scope);
      scope.run(() => {
        const tr = renderFn(items[i], i);
        // Insert into a temporary fragment, then move children before
        // anchor.nextSibling (or after the last currentNode).
        const frag = document.createDocumentFragment();
        commit(tr, frag);
        while (frag.childNodes.length > 0) {
          const node = frag.childNodes[0];
          anchor.parentNode.insertBefore(node, _nextSiblingAfter(anchor, state));
          state.currentNodes.push(node);
        }
      });
    }
  });
}

function _disposeItemScopes(state) {
  if (!state.itemScopes) return;
  for (const s of state.itemScopes) s.dispose();
  state.itemScopes.length = 0;
}
```

`_createScope` must be imported from `./reactivity.js`:

```js
import { effect, _createScope } from './reactivity.js';
```

Extend `_clearNodeSlot` in Step 4 to also dispose item scopes:

```js
function _clearNodeSlot(anchor, state) {
  _disposeItemScopes(state);
  _clearNodeContent(state);
}
```

**Tests:**

- Static list: `const items = signal(['a','b']); commit(html\`<ul>${each(items, (it) => html\`<li>${it}</li>\`)}</ul>\`, …);`
  ul has two li children with the expected text.
- Array change: `items.set(['x','y','z'])` — ul has three li children.
- Per-item effects torn down: each `renderFn` creates an effect that
  increments a counter; after changing the array, dispose the parent
  scope; updating the underlying signals does not mutate the counters
  further.
- Empty array: renders nothing (only the anchor remains).
- Re-eval shrinks list: `['a','b','c']` → `['a']` — only one li remains.
- Index passed correctly: `renderFn(item, index) => html\`<li>${index}: ${item}</li>\``.

---

## Risks and Assumptions

- **HTML parser scope:** The hand-rolled parser supports tags, attributes
  (quoted and unquoted), text content, self-closing tags, and close tags.
  It does **not** support: HTML comments inside the template, CDATA
  sections, processing instructions, DOCTYPE, or character-entity decoding.
  Placeholders inside an HTML comment, inside a tag name, or inside a
  `<script>`/`<style>` block are treated as errors. This is acceptable for
  Phase 2 but should be documented.
- **`document` global:** The implementation reads `document` from the
  global scope at call time. Tests rely on `dom-shim.js`'s side-effect
  installing `globalThis.document`. If a future test file imports
  `template.js` before `dom-shim.js`, the parser/commit calls will throw
  `ReferenceError`. Mitigation: a single `import './dom-shim.js'` at the
  top of every template test file (relied on by every test in the
  template suite).
- **Strict-equality signal check on attr parts:** When a signal's value
  changes, the wrapping `effect` re-runs and `setAttribute` is called even
  if the rendered string didn't change (e.g., signal goes from `true` →
  `'true'`). Acceptable in Phase 2; the DOM happily accepts redundant
  attribute writes.
- **Throttle/debounce defaults:** The `100ms` interval is hard-coded.
  This is a known limitation. If the framework adopts numeric modifier
  syntax later (`@scroll.throttle:200`), it can be retrofitted without
  breaking changes.
- **Each is unkeyed:** the spec is explicit about this. Keyed
  reconciliation (DOM-node reuse, reordering) is a future phase. For an
  array of N items, every array mutation does an O(N) teardown + rebuild.
  Acceptable for Phase 2's surface; large lists will perform poorly until
  keyed `each()` lands.
- **WeakMap cache key:** Template caching uses the `strings` array as the
  WeakMap key. If a test re-evaluates the same source-code template
  literal across module reloads, the keys are different — caching is
  per-module-instance. Not a runtime issue in browsers; could cause
  test-isolation surprises if a test forces module reset.
- **No batching:** Multiple signal updates in the same tick re-run the
  same effect multiple times. Inherited from Phase 1. Batching is a
  framework-wide future concern.
- **DOM shim coverage drift:** As Phase 2 grows, the shim's `dispatchEvent`
  and event-listener bookkeeping must stay close enough to real DOM
  semantics that tests don't pass on the shim and fail in browsers. The
  test suite running against the shim is the only check; consider adding
  a browser smoke test in Phase 5.
