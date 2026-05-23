// Stack of currently-executing observers (computed or effect instances).
// Module-level (not globalThis) to avoid conflicts with multiple runtime copies.
let _observerStack = [];

// Currently active ownership scope (set by createScope, used by effect).
let _activeScope = null;

// Tracks effects created without an active ownership scope, so the test
// API's `cleanup()` can dispose them between tests within one Boa context.
/** @type {Set<() => void>} */
let _unownedEffects = new Set();

/**
 * Register the current observer as a subscriber of the given set.
 * Also records the set in the observer's _sources for later cleanup.
 * @param {Set} subscribers
 */
function _subscribe(subscribers) {
  const observer = _observerStack[_observerStack.length - 1];
  if (observer) {
    subscribers.add(observer);
    observer._sources.add(subscribers);
  }
}

/**
 * Remove an observer from all subscriber sets it joined, then clear its sources.
 * Called before re-evaluation so stale conditional dependencies are dropped.
 * @param {{ _sources: Set }} observer
 */
function _unsubscribeAll(observer) {
  for (const subscriberSet of observer._sources) {
    subscriberSet.delete(observer);
  }
  observer._sources.clear();
}

/**
 * Creates a reactive container with an initial value.
 * @template T
 * @param {T} initialValue
 * @returns {{ readonly val: T, set(newVal: T): void, update(fn: (v: T) => T): void }}
 */
export function signal(initialValue) {
  let _value = initialValue;
  const _subscribers = new Set();

  return {
    get val() {
      _subscribe(_subscribers);
      return _value;
    },
    /** @param {T} newVal */
    set(newVal) {
      if (newVal === _value) return;
      _value = newVal;
      for (const observer of [..._subscribers]) observer._notify();
    },
    /** @param {(v: T) => T} fn */
    update(fn) {
      this.set(fn(_value));
    },
  };
}

/**
 * Creates a lazily-evaluated derived value.
 * Re-evaluates when any dependency changes, but only on the next .val read.
 * @template T
 * @param {() => T} fn
 * @returns {{ readonly val: T }}
 */
export function computed(fn) {
  let _value;
  let _dirty = true;
  const _subscribers = new Set();

  const self = {
    _sources: new Set(),
    _notify() {
      if (_dirty) return;
      _dirty = true;
      for (const observer of [..._subscribers]) observer._notify();
    },
    get val() {
      if (_dirty) {
        _unsubscribeAll(self);
        _observerStack.push(self);
        try {
          _value = fn();
        } finally {
          _observerStack.pop();
        }
        _dirty = false;
      }
      _subscribe(_subscribers);
      return _value;
    },
  };

  return self;
}

/**
 * Runs fn immediately and re-runs it when any dependency changes.
 * fn may return a cleanup function, called before each re-run and on stop.
 * @param {() => (void | (() => void))} fn
 * @returns {() => void} stop — disposes the effect
 */
export function effect(fn) {
  let _cleanup;
  const _registeredScope = _activeScope;

  const self = {
    _sources: new Set(),
    _notify() {
      _run();
    },
  };

  function _run() {
    _unsubscribeAll(self);
    if (_cleanup) {
      _cleanup();
      _cleanup = undefined;
    }
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
    if (_cleanup) {
      _cleanup();
      _cleanup = undefined;
    }
    if (_registeredScope) _registeredScope._effects.delete(stop);
    _unownedEffects.delete(stop);
  }

  if (_activeScope) {
    _activeScope._effects.add(stop);
  } else {
    _unownedEffects.add(stop);
  }

  _run();

  return stop;
}

/**
 * Creates an internal ownership scope.
 * Effects created while scope.run() is active are registered with this scope.
 * scope.dispose() stops all registered effects and recursively disposes child scopes.
 * Not part of the public API.
 */
function createScope() {
  const _parentScope = _activeScope;

  const scope = {
    _effects: new Set(),
    _children: new Set(),
    _cleanups: [],
    dispose() {
      for (const stop of [...scope._effects]) stop();
      scope._effects.clear();
      for (const child of [...scope._children]) child.dispose();
      scope._children.clear();
      for (const fn of scope._cleanups) {
        try { fn(); } catch (_) { /* swallow */ }
      }
      scope._cleanups.length = 0;
      if (_parentScope) _parentScope._children.delete(scope);
    },
    /**
     * Register `fn` to run when this scope is disposed. Callbacks run in
     * registration order after effects and child scopes have been torn down.
     * @param {() => void} fn
     */
    onCleanup(fn) {
      scope._cleanups.push(fn);
    },
    /** @param {() => *} fn */
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

/**
 * Dispose every effect created without an active scope. Used by
 * `zero/test`'s `cleanup()` to prevent leaked top-level effects from firing
 * across tests within a single Boa context.
 * @internal
 * @returns {void}
 */
export function _disposeUnownedEffects() {
  for (const stop of [..._unownedEffects]) stop();
  _unownedEffects.clear();
}

// Exported for testing only — not part of the public API.
export { createScope as _createScope };
