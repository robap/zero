/**
 * Encoding-API shim (TextEncoder, TextDecoder — UTF-8 only).
 *
 * Concatenated into `ZERO_DOM_SHIM_BODY` by `crates/zero-runtime/build.rs`
 * and evaluated as a script by the test harness before user modules run.
 * No `import` / `export`; relies on globals installed by `dom-shim.js`.
 */

/**
 * Append the UTF-8 bytes for a single codepoint to `out`. Split into one
 * function per branch length (per `boa-maplock-finalizer`).
 * @param {number} cp
 * @param {number[]} out
 * @returns {void}
 */
function _encodeCodepoint(cp, out) {
  if (cp < 0x80) out.push(cp);
  else if (cp < 0x800) _encode2byte(cp, out);
  else if (cp < 0x10000) _encode3byte(cp, out);
  else _encode4byte(cp, out);
}

/**
 * @param {number} cp
 * @param {number[]} out
 * @returns {void}
 */
function _encode2byte(cp, out) {
  out.push(0xc0 | (cp >> 6));
  out.push(0x80 | (cp & 0x3f));
}

/**
 * @param {number} cp
 * @param {number[]} out
 * @returns {void}
 */
function _encode3byte(cp, out) {
  out.push(0xe0 | (cp >> 12));
  out.push(0x80 | ((cp >> 6) & 0x3f));
  out.push(0x80 | (cp & 0x3f));
}

/**
 * @param {number} cp
 * @param {number[]} out
 * @returns {void}
 */
function _encode4byte(cp, out) {
  out.push(0xf0 | (cp >> 18));
  out.push(0x80 | ((cp >> 12) & 0x3f));
  out.push(0x80 | ((cp >> 6) & 0x3f));
  out.push(0x80 | (cp & 0x3f));
}

/**
 * Encode a JS string to UTF-8 bytes via `String#codePointAt`.
 * @param {string} str
 * @param {number[]} out
 * @returns {void}
 */
function _encodeUtf8Into(str, out) {
  let i = 0;
  while (i < str.length) {
    const cp = /** @type {number} */ (str.codePointAt(i));
    _encodeCodepoint(cp, out);
    i += cp > 0xffff ? 2 : 1;
  }
}

class TextEncoder {
  constructor() {
    /** @type {string} */
    this.encoding = 'utf-8';
  }
  /**
   * @param {string} [str]
   * @returns {Uint8Array}
   */
  encode(str) {
    const s = String(str ?? '');
    const out = /** @type {number[]} */ ([]);
    _encodeUtf8Into(s, out);
    return new Uint8Array(out);
  }
}

/**
 * Decode UTF-8 bytes back to a JS string. Surrogate pairs are emitted via
 * `String.fromCodePoint` so codepoints > U+FFFF round-trip correctly.
 * @param {Uint8Array} bytes
 * @returns {string}
 */
function _decodeUtf8(bytes) {
  let out = '';
  let i = 0;
  while (i < bytes.length) {
    const b0 = bytes[i];
    if (b0 < 0x80) { out += String.fromCharCode(b0); i += 1; continue; }
    if ((b0 & 0xe0) === 0xc0) { out += _decode2byte(bytes, i); i += 2; continue; }
    if ((b0 & 0xf0) === 0xe0) { out += _decode3byte(bytes, i); i += 3; continue; }
    if ((b0 & 0xf8) === 0xf0) { out += _decode4byte(bytes, i); i += 4; continue; }
    out += '�'; i += 1;
  }
  return out;
}

/**
 * @param {Uint8Array} bytes
 * @param {number} i
 * @returns {string}
 */
function _decode2byte(bytes, i) {
  const cp = ((bytes[i] & 0x1f) << 6) | (bytes[i + 1] & 0x3f);
  return String.fromCodePoint(cp);
}

/**
 * @param {Uint8Array} bytes
 * @param {number} i
 * @returns {string}
 */
function _decode3byte(bytes, i) {
  const cp = ((bytes[i] & 0x0f) << 12) | ((bytes[i + 1] & 0x3f) << 6) | (bytes[i + 2] & 0x3f);
  return String.fromCodePoint(cp);
}

/**
 * @param {Uint8Array} bytes
 * @param {number} i
 * @returns {string}
 */
function _decode4byte(bytes, i) {
  const cp = ((bytes[i] & 0x07) << 18)
    | ((bytes[i + 1] & 0x3f) << 12)
    | ((bytes[i + 2] & 0x3f) << 6)
    | (bytes[i + 3] & 0x3f);
  return String.fromCodePoint(cp);
}

class TextDecoder {
  /**
   * @param {string} [encoding]
   */
  constructor(encoding) {
    const label = String(encoding ?? 'utf-8').toLowerCase();
    if (label !== 'utf-8' && label !== 'utf8') {
      throw new RangeError(
        `zero test: TextDecoder only supports utf-8 (got "${encoding}")`,
      );
    }
    /** @type {string} */
    this.encoding = 'utf-8';
  }
  /**
   * @param {ArrayBuffer | ArrayBufferView | undefined | null} input
   * @returns {string}
   */
  decode(input) {
    if (input == null) return '';
    if (input instanceof Uint8Array) return _decodeUtf8(input);
    if (input instanceof ArrayBuffer) return _decodeUtf8(new Uint8Array(input));
    const view = /** @type {ArrayBufferView} */ (input);
    return _decodeUtf8(new Uint8Array(view.buffer, view.byteOffset, view.byteLength));
  }
}

if (typeof globalThis.TextEncoder === 'undefined') {
  Object.defineProperty(globalThis, 'TextEncoder', {
    value: TextEncoder, writable: true, configurable: true,
  });
}
if (typeof globalThis.TextDecoder === 'undefined') {
  Object.defineProperty(globalThis, 'TextDecoder', {
    value: TextDecoder, writable: true, configurable: true,
  });
}
