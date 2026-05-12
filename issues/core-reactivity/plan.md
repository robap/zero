# Plan: Core Reactivity

## Summary

Build the foundational reactive primitives (`signal`, `computed`, `effect`) for the zero framework JavaScript runtime. The implementation lives in `runtime/reactivity.js` as a plain ES module with JSDoc annotations and zero dependencies. The approach follows the classic **global observer stack + pull-based lazy computed** model: signals maintain a subscriber set; computed values mark themselves dirty on dependency change and re-evaluate lazily on the next `.val` read; effects re-run eagerly. An internal ownership scope system (not exported) is added last to support future component cleanup. Tests use Node.js's built-in `node:test` runner with no external dependencies.

## Prerequisites

Three open questions from the spec are resolved here so execution can proceed without further clarification:

1. **Push vs. pull invalidation for computed:** Use pull — mark dirty when a dependency changes, re-evaluate lazily on next `.val` read. This matches the spec's own recommendation.
2. **Max observer stack depth guard:** No guard in Phase 1. Circular computed dependencies are a developer error and are out of scope.
3. **Infinite loop guard for self-referential effects:** Out of scope for Phase 1.

## Steps

- [x] **Step 1: Scaffold — directory layout and test harness**
- [x] **Step 2: Signal primitive and global observer stack**
- [x] **Step 3: Computed primitive**
- [x] **Step 4: Effect primitive**
- [x] **Step 5: Ownership scope (internal)**

---

## Step Details

### Step 1: Scaffold — directory layout and test harness

**Goal:** Establish the file structure and verify the test harness works end-to-end before writing any reactive logic. Everything from Step 2 onward drops code into files created here.

**Files:**
- `runtime/reactivity.js` — create with skeleton module (no-op exports)
- `tests/reactivity.test.js` — create with one passing smoke test

**Changes:**

Create `runtime/reactivity.js` as an ES module with empty stub exports and JSDoc stubs for the three public symbols:

```js
/** @param {*} initialValue */
export function signal(initialValue) { throw new Error('not implemented'); }

/** @param {() => *} fn */
export function computed(fn) { throw new Error('not implemented'); }

/** @param {() => (void | (() => void))} fn */
export function effect(fn) { throw new Error('not implemented'); }
```

Create `tests/reactivity.test.js` using `node:test` and `node:assert`:

```js
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { signal, computed, effect } from '../runtime/reactivity.js';
```

Add a single smoke test: `signal` throws "not implemented" — just confirms the import resolves and the test runner works.

**Tests:** One test — `import { signal } from '../runtime/reactivity.js'` resolves without error, and calling `signal(0)` throws (confirming it's the stub, not a module-not-found failure).

Run with: `node --test tests/reactivity.test.js`

---

### Step 2: Signal primitive and global observer stack

**Goal:** Implement the complete `signal` API and the global observer stack that all subsequent primitives depend on. This step introduces the core subscription mechanism; `computed` and `effect` in Steps 3–4 are built on top of it without modifying it.

**Files:**
- `runtime/reactivity.js` — replace signal stub; add global observer stack internals

**Changes:**

Add two module-level variables at the top of `reactivity.js`:

```js
// Stack of currently-executing observers (computed or effect instances).
// Module-level (not globalThis) to avoid conflicts with multiple runtime copies.
let _observerStack = [];

// Currently active ownership scope (set by createScope, used by effect).
let _activeScope = null;
```

Define an observer registration helper used by both `signal` and `computed` when their `.val` is read:

```js
function _subscribe(subscribers) {
  const observer = _observerStack[_observerStack.length - 1];
  if (observer) {
    subscribers.add(observer);
    observer._sources.add(subscribers); // back-reference for cleanup
  }
}
```

Implement `signal(initialValue)`:

```js
export function signal(initialValue) {
  let _value = initialValue;
  const _subscribers = new Set();

  return {
    get val() {
      _subscribe(_subscribers);
      return _value;
    },
    set(newVal) {
      if (newVal === _value) return;
      _value = newVal;
      for (const observer of [..._subscribers]) observer._notify();
    },
    update(fn) {
      this.set(fn(_value));
    },
  };
}
```

Observer instances (created in Steps 3–4) must expose:
- `_sources`: `Set` of subscriber-sets they are registered in (for unsubscription)
- `_notify()`: method called when a dependency changes

**Tests:** Add to `tests/reactivity.test.js`:

- `signal(v).val` returns `v`
- `.set(newVal)` updates `.val`
- `.update(fn)` passes current value to `fn` and applies the result
- Setting the same value (`===`) does not trigger notification (use a counter effect to verify — but since effect is not yet implemented, use a manual subscriber mock: directly push a spy object onto `_observerStack` and verify `.val` read registers it, and `.set()` calls `_notify`)
- Strict-equality check: `signal(0).set(0)` → subscriber `_notify` not called

Since `effect` is not implemented yet, the subscriber notification test can use an inline mock:

```js
// Directly exercise the observer stack for notification testing.
let notified = 0;
const mockObserver = { _sources: new Set(), _notify() { notified++; } };
_observerStack.push(mockObserver); // requires exporting _observerStack for tests OR testing indirectly
```

To avoid exporting internals, the notification-count test can be deferred to Step 4 and replaced here with simpler assertions: set value, read value, confirm new value is returned.

---

### Step 3: Computed primitive

**Goal:** Implement lazy pull-based derived values. A computed is both a subscriber (it reads signals) and a notifier (other computeds/effects can depend on it). This step does not touch signal or the observer stack implementation.

**Files:**
- `runtime/reactivity.js` — replace computed stub

**Changes:**

Add a helper to unsubscribe an observer from all its current sources (used before re-evaluation):

```js
function _unsubscribeAll(observer) {
  for (const subscriberSet of observer._sources) {
    subscriberSet.delete(observer);
  }
  observer._sources.clear();
}
```

Implement `computed(fn)`:

```js
export function computed(fn) {
  let _value;
  let _dirty = true;
  const _subscribers = new Set();

  const self = {
    _sources: new Set(),
    _notify() {
      if (_dirty) return; // already dirty, avoid redundant work
      _dirty = true;
      for (const observer of [..._subscribers]) observer._notify();
    },
    get val() {
      if (_dirty) {
        _unsubscribeAll(self);       // clear stale deps
        _observerStack.push(self);   // track new deps during fn()
        try {
          _value = fn();
        } finally {
          _observerStack.pop();
        }
        _dirty = false;
      }
      _subscribe(_subscribers);      // register caller as our subscriber
      return _value;
    },
  };

  return self;
}
```

Key behaviors:
- `_dirty` starts `true` so the first `.val` read always evaluates `fn`.
- `_unsubscribeAll` before each re-evaluation ensures stale dependencies from conditional branches are cleared.
- Computed is not exported with `.set()` or `.update()` — the returned object only has `.val`.

**Tests:**

- `computed(() => sig.val + 1)` returns derived value
- Does not re-run `fn` until `.val` is read after a dependency changes (verify with a call-count spy)
- Dependency changes: update signal → computed `.val` returns new value
- Conditional branches: `computed(() => cond.val ? a.val : b.val)` — after `cond` flips, `b` no longer causes re-eval (use call counter)
- Computed depending on computed: `c2 = computed(() => c1.val * 2)`

---

### Step 4: Effect primitive

**Goal:** Implement eagerly-executing side effects with automatic cleanup and a `stop` function. Effects are the only primitive that runs immediately and re-runs without a `.val` trigger from external code.

**Files:**
- `runtime/reactivity.js` — replace effect stub

**Changes:**

Implement `effect(fn)`:

```js
export function effect(fn) {
  let _cleanup = undefined;

  const self = {
    _sources: new Set(),
    _notify() {
      _run();
    },
  };

  function _run() {
    _unsubscribeAll(self);
    if (_cleanup) { _cleanup(); _cleanup = undefined; }
    _observerStack.push(self);
    try {
      const result = fn();
      if (typeof result === 'function') _cleanup = result;
    } finally {
      _observerStack.pop();
    }
  }

  function stop() {
    _unsubscribeAll(self);
    if (_cleanup) { _cleanup(); _cleanup = undefined; }
    // Register with active scope if one exists
  }

  // Register with active scope (for Step 5)
  if (_activeScope) _activeScope._effects.add(stop);

  _run(); // execute immediately

  return stop;
}
```

**Tests:**

- `effect(fn)` runs `fn` immediately on creation
- Re-runs `fn` when a dependency signal changes
- Cleanup function returned by `fn` is called before the next re-run
- Cleanup function is called when `stop()` is invoked
- `stop()` prevents further re-runs after being called
- Effect depending on computed: updates when the computed's underlying signal changes

---

### Step 5: Ownership scope (internal)

**Goal:** Add the internal scope system so future phases can group effects under a component lifetime and dispose them all at once. This is purely internal — `createScope` is not exported. No public API changes; the tests use a thin internal testing surface.

**Files:**
- `runtime/reactivity.js` — add `createScope` function (not exported); wire `_activeScope` into `effect`

**Changes:**

The `effect` implementation in Step 4 already includes the hook `if (_activeScope) _activeScope._effects.add(stop)`. Step 5 implements the scope that this hook registers with.

Add `createScope()` (not exported):

```js
function createScope() {
  const _parentScope = _activeScope;
  const scope = {
    _effects: new Set(),
    _children: new Set(),
    dispose() {
      for (const stop of scope._effects) stop();
      scope._effects.clear();
      for (const child of scope._children) child.dispose();
      scope._children.clear();
      if (_parentScope) _parentScope._children.delete(scope);
    },
    run(fn) {
      const prev = _activeScope;
      _activeScope = scope;
      if (_parentScope) _parentScope._children.add(scope);
      try {
        return fn();
      } finally {
        _activeScope = prev;
      }
    },
  };
  return scope;
}
```

Usage pattern (internal, for future component system):
```js
const scope = createScope();
scope.run(() => {
  effect(() => { /* ... */ }); // auto-registered with scope
});
scope.dispose(); // stops all effects, disposes child scopes
```

To make scope testable without exporting it, export a single test-only helper that is clearly marked as internal:

```js
// Exported only for testing — not part of the public API.
export { createScope as _createScope };
```

This follows the convention used by frameworks like SolidJS (`_$createSignal` etc.) and matches the "plain JS" constraint without any build-time tree-shaking.

**Tests:**

- `createScope().run(() => effect(...))` — `dispose()` stops the effect (verify cleanup called, no further re-runs)
- Child scopes: nested `createScope()` inside a parent scope's `run()` — parent `dispose()` recursively disposes child
- `_activeScope` returns to its previous value after `scope.run()` (no scope leak)

---

## Risks and Assumptions

- **Node.js version:** `node:test` requires Node ≥ 18. The plan assumes a modern Node.js is available in the development environment; if not, the test harness choice needs to change (but not the implementation).
- **ES module import in tests:** `runtime/reactivity.js` uses ES module syntax (`export`). `node --test` with ES modules works from Node 20.6+ without flags; earlier Node 18 requires `--experimental-vm-modules`. If the environment has an older Node, tests may need a `package.json` with `"type": "module"`.
- **`_createScope` export for tests:** Exporting an internal with a `_` prefix is a pragmatic choice for Phase 1 testing. If the project adopts a framework-level test harness in Phase 5, this export should be removed or gated behind a test build flag.
- **Glitch-free computed propagation:** The current pull model does not prevent "diamond dependency" glitches (where a computed depending on two paths of the same signal re-evaluates twice). Glitch-free propagation requires a topological sort or push-then-pull scheme, which is explicitly out of scope for Phase 1. If downstream phases surface this as a bug, the computed implementation will need revision.
- **No cycle detection:** Circular computed dependencies (`a = computed(() => b.val); b = computed(() => a.val)`) will stack-overflow. This is documented as a developer error, not a framework concern in Phase 1.
