/**
 * Cloning + scheduling shim (structuredClone, queueMicrotask).
 *
 * Boa 0.21 already ships `Promise.withResolvers` (verified by Step 1's
 * baseline probe), so no polyfill is shipped here.
 *
 * Concatenated into `ZERO_DOM_SHIM_BODY` by `crates/zero-runtime/build.rs`
 * and evaluated as a script by the test harness before user modules run.
 * No `import` / `export`; relies on globals installed by `dom-shim.js`.
 */

/**
 * Throw a DataCloneError-shaped Error with the shim's "zero test:" prefix.
 * @param {string} reason
 * @returns {never}
 */
function _throwDataCloneError(reason) {
  const err = new Error(`zero test: structuredClone: ${reason}`);
  err.name = 'DataCloneError';
  throw err;
}

/**
 * @param {unknown[]} src
 * @param {WeakMap<object, unknown>} seen
 * @returns {unknown[]}
 */
function _cloneArray(src, seen) {
  const out = /** @type {unknown[]} */ ([]);
  seen.set(/** @type {object} */ (/** @type {unknown} */ (src)), out);
  for (let i = 0; i < src.length; i++) out[i] = _cloneAny(src[i], seen);
  return out;
}

/**
 * @param {Record<string, unknown>} src
 * @param {WeakMap<object, unknown>} seen
 * @returns {Record<string, unknown>}
 */
function _clonePlainObject(src, seen) {
  const out = /** @type {Record<string, unknown>} */ ({});
  seen.set(src, out);
  for (const k of Object.keys(src)) out[k] = _cloneAny(src[k], seen);
  return out;
}

/**
 * @param {Map<unknown, unknown>} src
 * @param {WeakMap<object, unknown>} seen
 * @returns {Map<unknown, unknown>}
 */
function _cloneMap(src, seen) {
  const out = new Map();
  seen.set(src, out);
  for (const [k, v] of src) out.set(_cloneAny(k, seen), _cloneAny(v, seen));
  return out;
}

/**
 * @param {Set<unknown>} src
 * @param {WeakMap<object, unknown>} seen
 * @returns {Set<unknown>}
 */
function _cloneSet(src, seen) {
  const out = new Set();
  seen.set(src, out);
  for (const v of src) out.add(_cloneAny(v, seen));
  return out;
}

/**
 * @param {Date} src
 * @returns {Date}
 */
function _cloneDate(src) { return new Date(src.getTime()); }

/**
 * @param {RegExp} src
 * @returns {RegExp}
 */
function _cloneRegExp(src) { return new RegExp(src.source, src.flags); }

/**
 * @param {Error} src
 * @returns {Error}
 */
function _cloneError(src) {
  const out = new Error(src.message);
  out.name = src.name;
  if (src.stack) out.stack = src.stack;
  return out;
}

/**
 * @param {ArrayBuffer} src
 * @returns {ArrayBuffer}
 */
function _cloneArrayBuffer(src) {
  const copy = new ArrayBuffer(src.byteLength);
  new Uint8Array(copy).set(new Uint8Array(src));
  return copy;
}

/**
 * Clone any `ArrayBufferView` by copying its bytes into a fresh buffer and
 * re-wrapping with the same constructor.
 * @param {ArrayBufferView} src
 * @returns {ArrayBufferView}
 */
function _cloneTypedArray(src) {
  const ctor = /** @type {any} */ (src.constructor);
  const bytes = new Uint8Array(src.buffer, src.byteOffset, src.byteLength);
  const copyBuffer = bytes.slice().buffer;
  if (src instanceof DataView) return new DataView(copyBuffer);
  const elementSize = /** @type {any} */ (ctor).BYTES_PER_ELEMENT ?? 1;
  return new ctor(copyBuffer, 0, src.byteLength / elementSize);
}

/**
 * Reject obviously-unclonable shapes (DOM nodes, Promises, functions,
 * WeakMap/WeakSet) per the spec.
 * @param {unknown} value
 * @returns {boolean}
 */
function _isUnclonable(value) {
  if (typeof value === 'function') return true;
  if (value && typeof value === 'object') {
    if (typeof /** @type {any} */ (value).nodeType === 'number') return true;
    if (value instanceof Promise) return true;
    if (value instanceof WeakMap || value instanceof WeakSet) return true;
  }
  return false;
}

/**
 * Recursive dispatcher; uses `seen` to short-circuit cycles.
 * @param {unknown} value
 * @param {WeakMap<object, unknown>} seen
 * @returns {unknown}
 */
function _cloneAny(value, seen) {
  if (value === null || typeof value !== 'object') {
    if (typeof value === 'function') _throwDataCloneError('functions cannot be cloned');
    return value;
  }
  if (seen.has(/** @type {object} */ (value))) return seen.get(/** @type {object} */ (value));
  if (_isUnclonable(value)) {
    const tag = value instanceof Promise ? 'Promise'
      : value instanceof WeakMap ? 'WeakMap'
        : value instanceof WeakSet ? 'WeakSet'
          : typeof /** @type {any} */ (value).nodeType === 'number' ? 'DOM node'
            : 'value';
    _throwDataCloneError(`${tag} cannot be cloned`);
  }
  if (Array.isArray(value)) return _cloneArray(/** @type {unknown[]} */ (value), seen);
  if (value instanceof Date) return _cloneDate(value);
  if (value instanceof RegExp) return _cloneRegExp(value);
  if (value instanceof Map) return _cloneMap(/** @type {Map<unknown, unknown>} */ (value), seen);
  if (value instanceof Set) return _cloneSet(/** @type {Set<unknown>} */ (value), seen);
  if (value instanceof Error) return _cloneError(value);
  if (value instanceof ArrayBuffer) return _cloneArrayBuffer(value);
  if (ArrayBuffer.isView(value)) return _cloneTypedArray(/** @type {ArrayBufferView} */ (value));
  return _clonePlainObject(/** @type {Record<string, unknown>} */ (value), seen);
}

/**
 * @param {unknown} value
 * @param {{ transfer?: unknown[] }} [options]
 * @returns {unknown}
 */
function structuredClone(value, options) {
  if (options && options.transfer && options.transfer.length > 0) {
    throw new Error(
      'zero test: structuredClone transfer is not supported. '
        + 'Drop the transfer list — the shim copies bytes instead of moving them.',
    );
  }
  return _cloneAny(value, new WeakMap());
}

/**
 * @param {Function} callback
 * @returns {void}
 */
function queueMicrotask(callback) {
  if (typeof callback !== 'function') {
    throw new TypeError('queueMicrotask: callback must be a function');
  }
  Promise.resolve().then(() => { callback(); });
}

if (typeof globalThis.structuredClone === 'undefined') {
  Object.defineProperty(globalThis, 'structuredClone', {
    value: structuredClone, writable: true, configurable: true,
  });
}
if (typeof globalThis.queueMicrotask === 'undefined') {
  Object.defineProperty(globalThis, 'queueMicrotask', {
    value: queueMicrotask, writable: true, configurable: true,
  });
}
