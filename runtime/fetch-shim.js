/**
 * Fetch-API shim (Headers, Request, Response, fetch default, AbortController,
 * AbortSignal).
 *
 * Concatenated into `ZERO_DOM_SHIM_BODY` by `crates/zero-runtime/build.rs`
 * and evaluated as a script by the test harness before user modules run.
 * No `import` / `export`; relies on globals installed by `dom-shim.js`.
 */

/**
 * Build a minimal `EventTarget`-shaped object used by `AbortSignal`. Kept as
 * a top-level helper (no inline branchy closures over shared state) per the
 * `boa-maplock-finalizer` workaround.
 * @returns {{ addEventListener: Function, removeEventListener: Function, dispatchEvent: Function }}
 */
function _makeAbortEventTarget() {
  const listeners = Object.create(null);
  return {
    /**
     * @param {string} type
     * @param {Function} fn
     * @returns {void}
     */
    addEventListener(type, fn) {
      let arr = listeners[type];
      if (!arr) { arr = []; listeners[type] = arr; }
      if (arr.indexOf(fn) < 0) arr.push(fn);
    },
    /**
     * @param {string} type
     * @param {Function} fn
     * @returns {void}
     */
    removeEventListener(type, fn) {
      const arr = listeners[type];
      if (!arr) return;
      const i = arr.indexOf(fn);
      if (i >= 0) arr.splice(i, 1);
    },
    /**
     * @param {{ type: string }} ev
     * @returns {boolean}
     */
    dispatchEvent(ev) {
      const arr = listeners[ev.type];
      if (!arr) return true;
      for (const fn of arr.slice()) fn.call(this, ev);
      return true;
    },
  };
}

/**
 * Construct a default `AbortError`-named Error.
 * @returns {Error}
 */
function _makeAbortError() {
  const err = new Error('signal is aborted without reason');
  err.name = 'AbortError';
  return err;
}

/**
 * Construct a default `TimeoutError`-named Error.
 * @returns {Error}
 */
function _makeTimeoutError() {
  const err = new Error('signal timed out');
  err.name = 'TimeoutError';
  return err;
}

/**
 * Mark a signal as aborted and dispatch its `'abort'` event. Shared by
 * `AbortController.abort`, `AbortSignal.abort`, `AbortSignal.timeout`, and
 * `AbortSignal.any`.
 * @param {AbortSignal} sig
 * @param {unknown} reason
 * @returns {void}
 */
function _fireAbort(sig, reason) {
  if (sig._aborted) return;
  sig._aborted = true;
  sig._reason = reason === undefined ? _makeAbortError() : reason;
  if (typeof sig.onabort === 'function') sig.onabort({ type: 'abort' });
  sig.dispatchEvent({ type: 'abort' });
}

class AbortSignal {
  constructor() {
    this._aborted = false;
    this._reason = undefined;
    Object.assign(this, _makeAbortEventTarget());
    this.onabort = null;
  }
  /** @returns {boolean} */
  get aborted() { return this._aborted; }
  /** @returns {unknown} */
  get reason() { return this._reason; }
  /**
   * Throw the stored reason if this signal is aborted; otherwise do nothing.
   * @returns {void}
   */
  throwIfAborted() {
    if (this._aborted) throw this._reason;
  }
}

/**
 * Static helper: return an already-aborted signal with `reason`.
 * @param {unknown} reason
 * @returns {AbortSignal}
 */
function _abortSignalAbort(reason) {
  const sig = new AbortSignal();
  _fireAbort(sig, reason);
  return sig;
}

/**
 * Static helper: return a signal that aborts on the next job-queue drain.
 * `ms` is accepted for API parity but ignored — the shim's `setTimeout`
 * fires on the next microtask.
 * @param {number} _ms
 * @returns {AbortSignal}
 */
function _abortSignalTimeout(_ms) {
  const sig = new AbortSignal();
  setTimeout(() => { _fireAbort(sig, _makeTimeoutError()); }, _ms);
  return sig;
}

/**
 * Static helper: return a composite signal that aborts when any input does.
 * @param {AbortSignal[]} signals
 * @returns {AbortSignal}
 */
function _abortSignalAny(signals) {
  const composite = new AbortSignal();
  for (const s of signals) {
    if (s && s.aborted) {
      _fireAbort(composite, s.reason);
      return composite;
    }
  }
  /** @param {AbortSignal} src @returns {Function} */
  const subscribe = (src) => {
    const handler = () => { _fireAbort(composite, src.reason); };
    src.addEventListener('abort', handler);
    return handler;
  };
  for (const s of signals) {
    if (s && !s.aborted) subscribe(s);
  }
  return composite;
}

AbortSignal.abort = _abortSignalAbort;
AbortSignal.timeout = _abortSignalTimeout;
AbortSignal.any = _abortSignalAny;

class AbortController {
  constructor() {
    /** @type {AbortSignal} */
    this.signal = new AbortSignal();
  }
  /**
   * @param {unknown} [reason]
   * @returns {void}
   */
  abort(reason) {
    _fireAbort(this.signal, reason);
  }
}

if (typeof globalThis.AbortSignal === 'undefined') {
  Object.defineProperty(globalThis, 'AbortSignal', {
    value: AbortSignal, writable: true, configurable: true,
  });
}
if (typeof globalThis.AbortController === 'undefined') {
  Object.defineProperty(globalThis, 'AbortController', {
    value: AbortController, writable: true, configurable: true,
  });
}

// ---------------------------------------------------------------------------
// Headers
// ---------------------------------------------------------------------------

/**
 * Build an empty internal header store: insertion-ordered list of
 * `[lower, original, value]` tuples (mirrors `_makeStorage()` in dom-shim).
 * @returns {Array<[string, string, string]>}
 */
function _headerStore() { return []; }

/**
 * Find the index of `name` in a header store (case-insensitive).
 * @param {Array<[string, string, string]>} store
 * @param {string} name
 * @returns {number}
 */
function _headerIndex(store, name) {
  const lower = String(name).toLowerCase();
  for (let i = 0; i < store.length; i++) if (store[i][0] === lower) return i;
  return -1;
}

class Headers {
  /**
   * @param {Headers | Record<string, string> | Array<[string, string]> | undefined} init
   */
  constructor(init) {
    /** @type {Array<[string, string, string]>} */
    this._store = _headerStore();
    if (init == null) return;
    if (init instanceof Headers) {
      for (const [k, v] of init.entries()) this.append(k, v);
      return;
    }
    if (Array.isArray(init)) {
      for (const pair of init) this.append(pair[0], pair[1]);
      return;
    }
    if (typeof init === 'object') {
      for (const k of Object.keys(init)) this.append(k, init[k]);
    }
  }
  /**
   * @param {string} name
   * @returns {string | null}
   */
  get(name) {
    const i = _headerIndex(this._store, name);
    return i < 0 ? null : this._store[i][2];
  }
  /**
   * @param {string} name
   * @param {string} value
   * @returns {void}
   */
  set(name, value) {
    const i = _headerIndex(this._store, name);
    const lower = String(name).toLowerCase();
    const str = String(value);
    if (i < 0) this._store.push([lower, String(name), str]);
    else this._store[i][2] = str;
  }
  /**
   * @param {string} name
   * @returns {boolean}
   */
  has(name) { return _headerIndex(this._store, name) >= 0; }
  /**
   * @param {string} name
   * @returns {void}
   */
  delete(name) {
    const i = _headerIndex(this._store, name);
    if (i >= 0) this._store.splice(i, 1);
  }
  /**
   * @param {string} name
   * @param {string} value
   * @returns {void}
   */
  append(name, value) {
    const i = _headerIndex(this._store, name);
    if (i < 0) this.set(name, value);
    else this._store[i][2] = this._store[i][2] + ', ' + String(value);
  }
  /**
   * @param {(value: string, name: string, headers: Headers) => void} cb
   * @param {unknown} [thisArg]
   * @returns {void}
   */
  forEach(cb, thisArg) {
    for (const [lower, , value] of this._store) cb.call(thisArg, value, lower, this);
  }
  /** @returns {IterableIterator<[string, string]>} */
  *entries() {
    for (const [lower, , value] of this._store) yield [lower, value];
  }
  /** @returns {IterableIterator<string>} */
  *keys() {
    for (const [lower] of this._store) yield lower;
  }
  /** @returns {IterableIterator<string>} */
  *values() {
    for (const [, , value] of this._store) yield value;
  }
}
Headers.prototype[Symbol.iterator] = Headers.prototype.entries;

// ---------------------------------------------------------------------------
// Body machinery shared by Request and Response
// ---------------------------------------------------------------------------

/**
 * Convert a stored body value to a string (best-effort, latin1 for typed
 * arrays). Used by `text()` / `json()` consumers on Request and Response.
 * @param {unknown} body
 * @returns {Promise<string>}
 */
async function _bodyToText(body) {
  if (body == null) return '';
  if (typeof body === 'string') return body;
  if (typeof body.text === 'function') return body.text();
  if (body instanceof ArrayBuffer) return _bytesToLatin1(new Uint8Array(body));
  if (ArrayBuffer.isView(body)) {
    return _bytesToLatin1(new Uint8Array(body.buffer, body.byteOffset, body.byteLength));
  }
  return String(body);
}

/**
 * Decode a byte view as a latin1 string. Sized to the runtime tests; no UTF-8.
 * @param {Uint8Array} bytes
 * @returns {string}
 */
function _bytesToLatin1(bytes) {
  let out = '';
  for (let i = 0; i < bytes.length; i++) out += String.fromCharCode(bytes[i]);
  return out;
}

/**
 * Intentional-stub helper: reject with a "not supported" message under the
 * shim's error contract.
 * @param {string} apiName
 * @returns {Promise<never>}
 */
function _stubBodyMethod(apiName) {
  return Promise.reject(new Error(
    `zero test: ${apiName} is not implemented. Read the body with text() or json() instead.`,
  ));
}

// ---------------------------------------------------------------------------
// Request
// ---------------------------------------------------------------------------

class Request {
  /**
   * @param {string | Request | { toString(): string }} input
   * @param {RequestInit} [init]
   */
  constructor(input, init) {
    const initOpts = init ?? {};
    let baseUrl;
    let baseInit = /** @type {Record<string, unknown>} */ ({});
    if (input instanceof Request) {
      baseUrl = input.url;
      baseInit = {
        method: input.method,
        headers: input.headers,
        body: input._body,
        signal: input.signal,
        mode: input.mode,
        credentials: input.credentials,
        cache: input.cache,
        redirect: input.redirect,
        referrer: input.referrer,
        integrity: input.integrity,
      };
    } else {
      baseUrl = String(input);
    }
    /** @type {string} */
    this.url = baseUrl;
    /** @type {string} */
    this.method = String(initOpts.method ?? baseInit.method ?? 'GET').toUpperCase();
    /** @type {Headers} */
    this.headers = new Headers(
      /** @type {Headers | Record<string, string> | undefined} */ (
        initOpts.headers ?? baseInit.headers
      ),
    );
    /** @type {unknown} */
    this._body = initOpts.body !== undefined ? initOpts.body : baseInit.body;
    /** @type {AbortSignal} */
    this.signal = /** @type {AbortSignal} */ (initOpts.signal ?? baseInit.signal ?? new AbortSignal());
    this.mode = String(initOpts.mode ?? baseInit.mode ?? 'cors');
    this.credentials = String(initOpts.credentials ?? baseInit.credentials ?? 'same-origin');
    this.cache = String(initOpts.cache ?? baseInit.cache ?? 'default');
    this.redirect = String(initOpts.redirect ?? baseInit.redirect ?? 'follow');
    this.referrer = String(initOpts.referrer ?? baseInit.referrer ?? '');
    this.integrity = String(initOpts.integrity ?? baseInit.integrity ?? '');
  }
  /** @returns {Promise<string>} */
  text() { return _bodyToText(this._body); }
  /** @returns {Promise<unknown>} */
  json() { return this.text().then(JSON.parse); }
  /** @returns {Promise<ArrayBuffer>} */
  arrayBuffer() { return /** @type {Promise<ArrayBuffer>} */ (_stubBodyMethod('Request.arrayBuffer()')); }
  /** @returns {Promise<unknown>} */
  blob() { return _stubBodyMethod('Request.blob()'); }
}

// ---------------------------------------------------------------------------
// Response
// ---------------------------------------------------------------------------

class Response {
  /**
   * @param {unknown} [body]
   * @param {ResponseInit} [init]
   */
  constructor(body, init) {
    const initOpts = init ?? {};
    /** @type {unknown} */
    this._body = body;
    /** @type {number} */
    this.status = initOpts.status ?? 200;
    /** @type {string} */
    this.statusText = String(initOpts.statusText ?? '');
    /** @type {Headers} */
    this.headers = new Headers(
      /** @type {Headers | Record<string, string> | undefined} */ (initOpts.headers),
    );
    this.redirected = false;
    this.type = 'default';
    this.url = '';
  }
  /** @returns {boolean} */
  get ok() { return this.status >= 200 && this.status < 300; }
  /** @returns {Promise<string>} */
  text() { return _bodyToText(this._body); }
  /** @returns {Promise<unknown>} */
  json() { return this.text().then(JSON.parse); }
  /** @returns {Promise<ArrayBuffer>} */
  arrayBuffer() { return /** @type {Promise<ArrayBuffer>} */ (_stubBodyMethod('Response.arrayBuffer()')); }
  /** @returns {Promise<unknown>} */
  blob() { return _stubBodyMethod('Response.blob()'); }
}

// ---------------------------------------------------------------------------
// fetch default + __resetFetch__
// ---------------------------------------------------------------------------

/**
 * Default global `fetch`: rejects with an actionable message. Users override
 * `globalThis.fetch` per test; `cleanup()` calls `__resetFetch__()` to put
 * this default back.
 * @returns {Promise<never>}
 */
function _zeroDefaultFetch() {
  return Promise.reject(new Error(
    "zero test: globalThis.fetch is not implemented. "
      + "Stub it in your test's beforeEach (or pass init.fetch to the call) "
      + '— see runtime/http.test.js for the pattern.',
  ));
}

if (typeof globalThis.Headers === 'undefined') {
  Object.defineProperty(globalThis, 'Headers', {
    value: Headers, writable: true, configurable: true,
  });
}
if (typeof globalThis.Request === 'undefined') {
  Object.defineProperty(globalThis, 'Request', {
    value: Request, writable: true, configurable: true,
  });
}
if (typeof globalThis.Response === 'undefined') {
  Object.defineProperty(globalThis, 'Response', {
    value: Response, writable: true, configurable: true,
  });
}
if (typeof globalThis.fetch === 'undefined') {
  Object.defineProperty(globalThis, 'fetch', {
    value: _zeroDefaultFetch, writable: true, configurable: true,
  });
}

globalThis.__resetFetch__ = () => {
  globalThis.fetch = _zeroDefaultFetch;
};
