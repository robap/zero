/**
 * Binary-data shim (Blob, File, FormData).
 *
 * Concatenated into `ZERO_DOM_SHIM_BODY` by `crates/zero-runtime/build.rs`
 * and evaluated as a script by the test harness before user modules run.
 * No `import` / `export`; relies on `TextEncoder` / `TextDecoder` from
 * `encoding-shim.js` (concatenated earlier in the shim body).
 */

/**
 * Coerce a single Blob constructor part into UTF-8 bytes.
 * @param {unknown} part
 * @returns {Uint8Array}
 */
function _partToBytes(part) {
  if (part == null) return new Uint8Array(0);
  if (typeof part === 'string') return new TextEncoder().encode(part);
  if (part instanceof Uint8Array) return part;
  if (part instanceof ArrayBuffer) return new Uint8Array(part);
  if (ArrayBuffer.isView(part)) {
    const view = /** @type {ArrayBufferView} */ (part);
    return new Uint8Array(view.buffer.slice(view.byteOffset, view.byteOffset + view.byteLength));
  }
  if (part && typeof part === 'object' && part._bytes instanceof Uint8Array) {
    return part._bytes;
  }
  return new TextEncoder().encode(String(part));
}

/**
 * Concatenate Blob constructor parts into a single `Uint8Array`.
 * @param {unknown[]} parts
 * @returns {Uint8Array}
 */
function _concatParts(parts) {
  if (!Array.isArray(parts)) return new Uint8Array(0);
  let total = 0;
  const arrays = /** @type {Uint8Array[]} */ ([]);
  for (const p of parts) {
    const bytes = _partToBytes(p);
    arrays.push(bytes);
    total += bytes.length;
  }
  const out = new Uint8Array(total);
  let offset = 0;
  for (const a of arrays) { out.set(a, offset); offset += a.length; }
  return out;
}

class Blob {
  /**
   * @param {unknown[]} [parts]
   * @param {{ type?: string }} [options]
   */
  constructor(parts, options) {
    /** @type {Uint8Array} */
    this._bytes = _concatParts(parts ?? []);
    /** @type {string} */
    this.type = String((options && options.type) ?? '');
  }
  /** @returns {number} */
  get size() { return this._bytes.byteLength; }
  /** @returns {Promise<string>} */
  text() {
    return Promise.resolve(new TextDecoder().decode(this._bytes));
  }
  /** @returns {Promise<ArrayBuffer>} */
  arrayBuffer() {
    return Promise.resolve(this._bytes.buffer.slice(
      this._bytes.byteOffset,
      this._bytes.byteOffset + this._bytes.byteLength,
    ));
  }
  /**
   * @param {number} [start]
   * @param {number} [end]
   * @param {string} [contentType]
   * @returns {Blob}
   */
  slice(start, end, contentType) {
    const s = start == null ? 0 : start;
    const e = end == null ? this._bytes.byteLength : end;
    const sub = this._bytes.subarray(s, e);
    const out = new Blob([], { type: contentType });
    out._bytes = new Uint8Array(sub);
    return out;
  }
}

class File extends Blob {
  /**
   * @param {unknown[]} parts
   * @param {string} name
   * @param {{ type?: string, lastModified?: number }} [options]
   */
  constructor(parts, name, options) {
    super(parts, options);
    /** @type {string} */
    this.name = String(name);
    /** @type {number} */
    this.lastModified = (options && typeof options.lastModified === 'number')
      ? options.lastModified
      : Date.now();
  }
}

/**
 * Normalize a FormData value: if it's a `Blob` (not a `File`), wrap it as a
 * `File` with the resolved filename so iteration semantics match the spec.
 * @param {unknown} value
 * @param {string | undefined} filename
 * @returns {unknown}
 */
function _normalizeFormDataValue(value, filename) {
  if (value instanceof File) {
    if (filename != null) {
      return new File([value._bytes], String(filename), { type: value.type, lastModified: value.lastModified });
    }
    return value;
  }
  if (value instanceof Blob) {
    return new File([value._bytes], filename != null ? String(filename) : 'blob', { type: value.type });
  }
  return String(value);
}

class FormData {
  /**
   * @param {unknown} [form]
   */
  constructor(form) {
    if (form != null) {
      throw new TypeError(
        'zero test: FormData constructor with form element is not supported. '
          + 'Build one with append() or use a plain object.',
      );
    }
    /** @type {Array<[string, unknown]>} */
    this._list = [];
  }
  /**
   * @param {string} name
   * @param {unknown} value
   * @param {string} [filename]
   * @returns {void}
   */
  append(name, value, filename) {
    this._list.push([String(name), _normalizeFormDataValue(value, filename)]);
  }
  /**
   * @param {string} name
   * @param {unknown} value
   * @param {string} [filename]
   * @returns {void}
   */
  set(name, value, filename) {
    const key = String(name);
    const normalized = _normalizeFormDataValue(value, filename);
    let replaced = false;
    const out = /** @type {Array<[string, unknown]>} */ ([]);
    for (const pair of this._list) {
      if (pair[0] === key) {
        if (!replaced) { out.push([key, normalized]); replaced = true; }
        continue;
      }
      out.push(pair);
    }
    if (!replaced) out.push([key, normalized]);
    this._list = out;
  }
  /**
   * @param {string} name
   * @returns {unknown}
   */
  get(name) {
    for (const [k, v] of this._list) if (k === name) return v;
    return null;
  }
  /**
   * @param {string} name
   * @returns {unknown[]}
   */
  getAll(name) {
    const out = [];
    for (const [k, v] of this._list) if (k === name) out.push(v);
    return out;
  }
  /**
   * @param {string} name
   * @returns {boolean}
   */
  has(name) {
    for (const [k] of this._list) if (k === name) return true;
    return false;
  }
  /**
   * @param {string} name
   * @returns {void}
   */
  delete(name) {
    this._list = this._list.filter(([k]) => k !== name);
  }
  /**
   * @param {(value: unknown, name: string, fd: FormData) => void} cb
   * @param {unknown} [thisArg]
   * @returns {void}
   */
  forEach(cb, thisArg) {
    for (const [k, v] of this._list) cb.call(thisArg, v, k, this);
  }
  /** @returns {IterableIterator<[string, unknown]>} */
  *entries() { for (const pair of this._list) yield [pair[0], pair[1]]; }
  /** @returns {IterableIterator<string>} */
  *keys() { for (const [k] of this._list) yield k; }
  /** @returns {IterableIterator<unknown>} */
  *values() { for (const [, v] of this._list) yield v; }
}
FormData.prototype[Symbol.iterator] = FormData.prototype.entries;

if (typeof globalThis.Blob === 'undefined') {
  Object.defineProperty(globalThis, 'Blob', {
    value: Blob, writable: true, configurable: true,
  });
}
if (typeof globalThis.File === 'undefined') {
  Object.defineProperty(globalThis, 'File', {
    value: File, writable: true, configurable: true,
  });
}
if (typeof globalThis.FormData === 'undefined') {
  Object.defineProperty(globalThis, 'FormData', {
    value: FormData, writable: true, configurable: true,
  });
}
