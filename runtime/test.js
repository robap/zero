import { commit } from "./template.js";
import { _createScope } from "./reactivity.js";
import { _getCurrentApp, _setCurrentApp } from "./app.js";
import { Event, KeyboardEvent, MouseEvent } from "./dom-shim.js";

/**
 * @typedef {{ name: string, parent: DescribeNode|null, children: Array<DescribeNode|ItNode>, beforeAll: Function[], afterAll: Function[], beforeEach: Function[], afterEach: Function[] }} DescribeNode
 */

/**
 * @typedef {{ name: string, fn: Function, parent: DescribeNode }} ItNode
 */

/**
 * @param {string} name
 * @param {DescribeNode|null} parent
 * @returns {DescribeNode}
 */
function makeDescribe(name, parent) {
  return { name, parent, children: [], beforeAll: [], afterAll: [], beforeEach: [], afterEach: [] };
}

/** @type {DescribeNode} */
let _root = makeDescribe("", null);

/** @type {DescribeNode} */
let _current = _root;

/** @type {Array<{ scope: object, container: object }>} */
let _renderTracker = [];

/**
 * Group tests under a named suite. The callback may be sync or async.
 * @param {string} name
 * @param {() => void | Promise<void>} fn
 * @returns {void | Promise<void>}
 */
export function describe(name, fn) {
  const node = makeDescribe(name, _current);
  _current.children.push(node);
  const prev = _current;
  _current = node;
  const result = fn();
  _current = prev;
  if (result && typeof result.then === "function") return result;
}

/**
 * Register a single test case.
 * @param {string} name
 * @param {() => void | Promise<void>} fn
 * @returns {void}
 */
export function it(name, fn) {
  _current.children.push({ name, fn, parent: _current });
}

/**
 * Register a hook that runs before each `it` in the enclosing `describe`.
 * @param {() => void | Promise<void>} fn
 * @returns {void}
 */
export function beforeEach(fn) {
  _current.beforeEach.push(fn);
}

/**
 * Register a hook that runs after each `it` in the enclosing `describe`.
 * @param {() => void | Promise<void>} fn
 * @returns {void}
 */
export function afterEach(fn) {
  _current.afterEach.push(fn);
}

/**
 * Register a hook that runs once before all `it`s in the enclosing `describe`.
 * @param {() => void | Promise<void>} fn
 * @returns {void}
 */
export function beforeAll(fn) {
  _current.beforeAll.push(fn);
}

/**
 * Register a hook that runs once after all `it`s in the enclosing `describe`.
 * @param {() => void | Promise<void>} fn
 * @returns {void}
 */
export function afterAll(fn) {
  _current.afterAll.push(fn);
}

// ---------------------------------------------------------------------------
// JS↔Rust ABI. The Rust harness in `crates/zero-test-runner/src/harness.rs`
// calls `__getTestTree__()` to walk the test tree built up during module
// evaluation, and `__resetTestTree__()` to rebuild it. Neither is part of
// the public `zero/test` API. The contract is covered by the harness's own
// Rust integration tests; do not assert on these from JS test files.
// ---------------------------------------------------------------------------

/**
 * Return the root of the test tree collected during module evaluation.
 * @internal
 * @returns {DescribeNode}
 */
export function __getTestTree__() {
  return _root;
}

/**
 * Reinitialize the test tree.
 * @internal
 * @returns {void}
 */
export function __resetTestTree__() {
  _root = makeDescribe("", null);
  _current = _root;
}

/**
 * Render a TemplateResult into an isolated container with optional stub state.
 * Registers the scope and container with the cleanup tracker.
 * Returns the container element so callers can query any rendered descendant
 * regardless of how many root elements the template produces.
 * @param {object} tr - TemplateResult produced by `html`.
 * @param {{ state?: Record<string, unknown> }} [opts]
 * @returns {object} The container element holding all rendered children.
 */
export function render(tr, opts = {}) {
  const stateMap = new Map(Object.entries(opts.state ?? {}));
  const stub = {
    _state: stateMap,
    /**
     * @param {string} key
     * @returns {unknown}
     */
    _getState(key) {
      if (!stateMap.has(key)) throw new Error(`inject: key "${key}" is not registered`);
      return stateMap.get(key);
    },
  };
  _setCurrentApp(stub);
  const scope = _createScope();
  const container = document.createElement("div");
  scope.run(() => commit(tr, container));
  _renderTracker.push({ scope, container });
  return container;
}

/**
 * Query a single descendant of `el` matching `selector`.
 * @param {object} el
 * @param {string} selector
 * @returns {object|null}
 */
export function find(el, selector) {
  return el.querySelector(selector);
}

/**
 * Query all descendants of `el` matching `selector`.
 * @param {object} el
 * @param {string} selector
 * @returns {object[]}
 */
export function findAll(el, selector) {
  return el.querySelectorAll(selector);
}

/**
 * Return concatenated text content of all text node descendants.
 * If `selector` is provided, first queries from `el`; throws if it matches nothing.
 * @param {object} el
 * @param {string} [selector]
 * @returns {string}
 */
export function text(el, selector) {
  const target = selector ? el.querySelector(selector) : el;
  if (selector && target == null) throw new Error(`text: selector "${selector}" matched nothing`);
  let out = "";
  (function walk(node) {
    if (node.nodeType === 3) out += node.nodeValue;
    if (node.childNodes) for (const c of node.childNodes) walk(c);
  })(target);
  return out;
}

/**
 * Dispatch a synthetic event on `el`. Uses the appropriate Event/KeyboardEvent/
 * MouseEvent constructor based on `type`, then layers any extra `data` fields
 * onto the event so older test code that passed e.g. `{target: {value: ...}}`
 * still observes the same shape inside handlers.
 * @param {object} el
 * @param {string} type
 * @param {Record<string, unknown>} [data]
 * @returns {void}
 */
export function fire(el, type, data = {}) {
  const ctor =
    type.startsWith("key") ? KeyboardEvent
      : (type === "click" || type === "dblclick" || type.startsWith("mouse")) ? MouseEvent
        : Event;
  const ev = new ctor(type, { bubbles: true, cancelable: true, ...data });
  Object.assign(ev, data);
  el.dispatchEvent(ev);
}

/**
 * Reset per-test mutable shim state: render scopes, current app, web storage,
 * pending timers, focused element, document title, and document subtree.
 * Order is intentional — scopes dispose first so their teardown callbacks can
 * still touch storage / timers; then storage clears; then timers cancel; then
 * document fields reset. Each block is feature-detected so the same function
 * works wherever the runtime is loaded.
 * @returns {void}
 */
export function cleanup() {
  for (const { scope } of _renderTracker) scope.dispose();
  _renderTracker.length = 0;
  const runningApp = _getCurrentApp();
  if (runningApp && runningApp._rootScope && typeof runningApp._rootScope.dispose === "function") {
    runningApp._rootScope.dispose();
  }
  _setCurrentApp(null);

  if (typeof globalThis.localStorage !== "undefined" && typeof globalThis.localStorage.clear === "function") {
    globalThis.localStorage.clear();
  }
  if (typeof globalThis.sessionStorage !== "undefined" && typeof globalThis.sessionStorage.clear === "function") {
    globalThis.sessionStorage.clear();
  }

  if (typeof globalThis.__clearAllTimers__ === "function") {
    globalThis.__clearAllTimers__();
  }

  if (typeof globalThis.__resetFetch__ === "function") {
    globalThis.__resetFetch__();
  }

  if (typeof globalThis.document !== "undefined") {
    const doc = globalThis.document;
    if ("_activeElement" in doc) doc._activeElement = null;
    if ("_title" in doc) doc._title = "";
    // Empty body and head; documentElement itself is left in place so that
    // `document.body` / `document.head` remain attached children.
    for (const root of [doc.body, doc.head]) {
      if (root && Array.isArray(root.childNodes)) {
        for (const c of [...root.childNodes]) {
          if (typeof root.removeChild === "function") root.removeChild(c);
        }
      }
    }
  }
}

/** @internal */
const _SPY = Symbol("zero/test:spy");

/**
 * Create a spy function. Records every call (args, result, thrown error, this-binding)
 * and optionally forwards to `impl`.
 * @template {(...args: any[]) => any} T
 * @param {T} [impl]
 * @returns {T & { calls: any[][], callCount: number, results: Array<{type: "return"|"throw", value: unknown}>, instances: unknown[], mockReturnValue(v: unknown): any, mockResolvedValue(v: unknown): any, mockRejectedValue(e: unknown): any, mockImplementation(fn: Function): any, reset(): any }}
 */
export function spy(impl) {
  let _impl = impl;
  function fn(...args) {
    fn.calls.push(args);
    fn.instances.push(this);
    if (_impl == null) {
      fn.results.push({ type: "return", value: undefined });
      return undefined;
    }
    try {
      const value = _impl.apply(this, args);
      fn.results.push({ type: "return", value });
      return value;
    } catch (e) {
      fn.results.push({ type: "throw", value: e });
      throw e;
    }
  }
  fn.calls = [];
  fn.results = [];
  fn.instances = [];
  Object.defineProperty(fn, "callCount", {
    get() { return fn.calls.length; },
    enumerable: true,
  });
  Object.defineProperty(fn, _SPY, { value: true });
  fn.mockReturnValue = v => { _impl = () => v; return fn; };
  fn.mockResolvedValue = v => { _impl = () => Promise.resolve(v); return fn; };
  fn.mockRejectedValue = e => { _impl = () => Promise.reject(e); return fn; };
  fn.mockImplementation = newImpl => { _impl = newImpl; return fn; };
  fn.reset = () => {
    fn.calls.length = 0;
    fn.results.length = 0;
    fn.instances.length = 0;
    return fn;
  };
  return fn;
}

// ---------------------------------------------------------------------------
// Assertions
// ---------------------------------------------------------------------------

/** @internal */
const _FRAMEWORK_INTERNAL_BASENAMES = new Set([
  "test.js",
  "template.js",
  "reactivity.js",
  "app.js",
  "router.js",
  "dom-shim.js",
  "http.js",
]);

/**
 * Walk a fresh stack and return the first frame outside framework-internal
 * runtime modules, formatted as `"<path>:<line>:<column>"`. Returns `null`
 * when no user frame can be identified.
 * @internal
 * @returns {string|null}
 */
function _captureUserFrame() {
  const stack = (new Error().stack) || "";
  for (const line of stack.split("\n")) {
    // V8: "    at fn (path:L:C)" or "    at path:L:C"
    // SpiderMonkey: "fn@path:L:C"
    // Plain "path:L:C"
    const m = line.match(/(?:\(|@|\s)([^\s()@]+):(\d+):(\d+)\)?\s*$/);
    if (!m) continue;
    const path = m[1];
    if (path.startsWith("node:")) continue;
    const slash = path.lastIndexOf("/");
    const base = slash >= 0 ? path.slice(slash + 1) : path;
    if (_FRAMEWORK_INTERNAL_BASENAMES.has(base)) continue;
    return `${path}:${m[2]}:${m[3]}`;
  }
  return null;
}

/**
 * Throw a fresh `Error` decorated with `_userFrame` (the call-site frame
 * outside `runtime/*.js`). Used by every matcher in `expect()`.
 * @internal
 * @param {string} msg
 * @returns {never}
 */
function _fail(msg) {
  const err = new Error(msg);
  err._userFrame = _captureUserFrame();
  throw err;
}

/**
 * Pretty-print a value for use in assertion error messages.
 * @internal
 * @param {unknown} v
 * @param {Set} [seen]
 * @returns {string}
 */
function _pretty(v, seen = new Set()) {
  if (v === null) return "null";
  if (v === undefined) return "undefined";
  if (typeof v === "string") return `"${v}"`;
  if (typeof v === "function") return "[Function]";
  if (typeof v !== "object") return String(v);
  if (seen.has(v)) return "[Circular]";
  seen.add(v);
  const desc = Object.getOwnPropertyDescriptor(v, "val");
  if (desc && typeof desc.get === "function") return `signal(${_pretty(v.val, seen)})`;
  if (Array.isArray(v)) return `[${v.map(item => _pretty(item, seen)).join(", ")}]`;
  const proto = Object.getPrototypeOf(v);
  if (proto === Object.prototype || proto === null) {
    const entries = Object.keys(v).map(k => `${k}: ${_pretty(v[k], seen)}`);
    return `{${entries.join(", ")}}`;
  }
  return String(v);
}

/**
 * Deep equality used by `.toEqual`. Handles primitives, arrays, plain objects, and signal-shaped objects.
 * @internal
 * @param {unknown} a
 * @param {unknown} b
 * @returns {boolean}
 */
function _deepEqual(a, b) {
  if (a === b) return true;
  if (a == null || b == null) return false;
  if (typeof a !== "object" || typeof b !== "object") return false;
  const aDesc = Object.getOwnPropertyDescriptor(a, "val");
  const bDesc = Object.getOwnPropertyDescriptor(b, "val");
  if (aDesc && typeof aDesc.get === "function" && bDesc && typeof bDesc.get === "function") {
    return _deepEqual(a.val, b.val);
  }
  if (Array.isArray(a) && Array.isArray(b)) {
    if (a.length !== b.length) return false;
    return a.every((item, i) => _deepEqual(item, b[i]));
  }
  if (Array.isArray(a) !== Array.isArray(b)) return false;
  const aProto = Object.getPrototypeOf(a);
  const bProto = Object.getPrototypeOf(b);
  if ((aProto === Object.prototype || aProto === null) && (bProto === Object.prototype || bProto === null)) {
    const aKeys = Object.keys(a);
    const bKeys = Object.keys(b);
    if (aKeys.length !== bKeys.length) return false;
    return aKeys.every(k => Object.prototype.hasOwnProperty.call(b, k) && _deepEqual(a[k], b[k]));
  }
  return false;
}

/**
 * Create a matcher object for `actual`.
 * @param {unknown} actual
 * @returns {{ toBe(expected: unknown): void, toEqual(expected: unknown): void, toBeTruthy(): void, toBeFalsy(): void, toBeNull(): void, toContain(item: unknown): void, toThrow(message?: string): void, toBeTemplateResult(): void, toMatchSnapshot(): void, toHaveBeenCalled(): void, toHaveBeenCalledTimes(n: number): void, toHaveBeenCalledWith(...args: unknown[]): void, toHaveBeenLastCalledWith(...args: unknown[]): void }}
 */
export function expect(actual) {
  const _isSpy = v => v != null && typeof v === "function" && Array.isArray(v.calls);
  return {
    toBe(expected) {
      if (actual !== expected) {
        _fail(
          `expect(${_pretty(actual)}).toBe(${_pretty(expected)}): values are not strictly equal`,
        );
      }
    },
    toEqual(expected) {
      if (!_deepEqual(actual, expected)) {
        _fail(
          `expect(${_pretty(actual)}).toEqual(${_pretty(expected)}): values are not deeply equal`,
        );
      }
    },
    toBeTruthy() {
      if (!Boolean(actual)) {
        _fail(`expect(${_pretty(actual)}).toBeTruthy(): value is falsy`);
      }
    },
    toBeFalsy() {
      if (Boolean(actual)) {
        _fail(`expect(${_pretty(actual)}).toBeFalsy(): value is truthy`);
      }
    },
    toBeNull() {
      if (actual !== null) {
        _fail(`expect(${_pretty(actual)}).toBeNull(): value is not null`);
      }
    },
    toContain(item) {
      if (typeof actual === "string") {
        if (!actual.includes(item)) {
          _fail(
            `expect(${_pretty(actual)}).toContain(${_pretty(item)}): string does not include substring`,
          );
        }
      } else if (Array.isArray(actual)) {
        if (actual.indexOf(item) < 0) {
          _fail(
            `expect(${_pretty(actual)}).toContain(${_pretty(item)}): array does not contain item`,
          );
        }
      } else {
        _fail(`expect(...).toContain: value is not a string or array`);
      }
    },
    toThrow(message) {
      if (typeof actual !== "function") {
        _fail(`expect(...).toThrow: value must be a function`);
      }
      let threw = false;
      let thrownError;
      try {
        actual();
      } catch (e) {
        threw = true;
        thrownError = e;
      }
      if (!threw) {
        _fail(`expect(...).toThrow: function did not throw`);
      }
      if (typeof message === "string") {
        const errMsg = thrownError instanceof Error ? thrownError.message : String(thrownError);
        if (!errMsg.includes(message)) {
          _fail(
            `expect(...).toThrow(${_pretty(message)}): threw "${errMsg}" which does not contain "${message}"`,
          );
        }
      }
    },
    toBeTemplateResult() {
      if (
        actual == null ||
        typeof actual !== "object" ||
        actual._template == null ||
        !Array.isArray(actual._values)
      ) {
        _fail(
          `expect(${_pretty(actual)}).toBeTemplateResult(): value is not a TemplateResult`,
        );
      }
    },
    toMatchSnapshot() {
      _fail("toMatchSnapshot: snapshot testing is not in this slice yet");
    },
    toHaveBeenCalled() {
      if (!_isSpy(actual)) {
        _fail(`expect(...).toHaveBeenCalled: value is not a spy`);
      }
      if (actual.callCount === 0) {
        _fail(`expect(spy).toHaveBeenCalled(): spy was not called`);
      }
    },
    toHaveBeenCalledTimes(n) {
      if (!_isSpy(actual)) {
        _fail(`expect(...).toHaveBeenCalledTimes: value is not a spy`);
      }
      if (actual.callCount !== n) {
        _fail(
          `expect(spy).toHaveBeenCalledTimes(${n}): spy was called ${actual.callCount} time(s)\n` +
          `  calls: ${_pretty(actual.calls)}`,
        );
      }
    },
    toHaveBeenCalledWith(...expectedArgs) {
      if (!_isSpy(actual)) {
        _fail(`expect(...).toHaveBeenCalledWith: value is not a spy`);
      }
      const hit = actual.calls.some(args => _deepEqual(args, expectedArgs));
      if (!hit) {
        _fail(
          `expect(spy).toHaveBeenCalledWith(${expectedArgs.map(a => _pretty(a)).join(", ")}): no recorded call matched\n` +
          `  recorded calls (${actual.callCount}): ${_pretty(actual.calls)}`,
        );
      }
    },
    toHaveBeenLastCalledWith(...expectedArgs) {
      if (!_isSpy(actual)) {
        _fail(`expect(...).toHaveBeenLastCalledWith: value is not a spy`);
      }
      if (actual.callCount === 0) {
        _fail(`expect(spy).toHaveBeenLastCalledWith(...): spy was never called`);
      }
      const lastArgs = actual.calls[actual.callCount - 1];
      if (!_deepEqual(lastArgs, expectedArgs)) {
        _fail(
          `expect(spy).toHaveBeenLastCalledWith(${expectedArgs.map(a => _pretty(a)).join(", ")}): last call did not match\n` +
          `  last call args: ${_pretty(lastArgs)}`,
        );
      }
    },
  };
}
