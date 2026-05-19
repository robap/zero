/**
 * URL-API shim (URL, URLSearchParams).
 *
 * Concatenated into `ZERO_DOM_SHIM_BODY` by `crates/zero-runtime/build.rs`
 * and evaluated as a script by the test harness before user modules run.
 * No `import` / `export`; relies on globals installed by `dom-shim.js`.
 *
 * Hand-written; sized to standard `https://host[:port]/path?q#h` shapes plus
 * relative-path resolution against a base URL. Incrementally hardenable.
 */

// ---------------------------------------------------------------------------
// URLSearchParams
// ---------------------------------------------------------------------------

/**
 * Decode a name/value pair from a query string: `+` → space, then
 * percent-decode the rest. Falls back to the raw string on malformed input.
 * @param {string} s
 * @returns {string}
 */
function _decodeFormPart(s) {
  const swapped = s.replace(/\+/g, ' ');
  try { return decodeURIComponent(swapped); } catch (_) { return swapped; }
}

/**
 * Encode a name/value pair for a query string: space → `+`, then
 * percent-encode reserved characters.
 * @param {string} s
 * @returns {string}
 */
function _encodeFormPart(s) {
  return encodeURIComponent(String(s)).replace(/%20/g, '+');
}

/**
 * Parse `?a=1&b=2` (with or without the leading `?`) into an insertion-order
 * list of `[name, value]` tuples.
 * @param {string} str
 * @returns {Array<[string, string]>}
 */
function _parseQueryString(str) {
  const out = /** @type {Array<[string, string]>} */ ([]);
  const trimmed = str.charAt(0) === '?' ? str.slice(1) : str;
  if (trimmed === '') return out;
  for (const part of trimmed.split('&')) {
    if (part === '') continue;
    const eq = part.indexOf('=');
    const rawName = eq < 0 ? part : part.slice(0, eq);
    const rawValue = eq < 0 ? '' : part.slice(eq + 1);
    out.push([_decodeFormPart(rawName), _decodeFormPart(rawValue)]);
  }
  return out;
}

class URLSearchParams {
  /**
   * @param {string | URLSearchParams | Record<string, string> | Array<[string, string]>} [init]
   */
  constructor(init) {
    /** @type {Array<[string, string]>} */
    this._list = [];
    /** @type {URL | null} */
    this._owner = null;
    if (init == null) return;
    if (typeof init === 'string') {
      this._list = _parseQueryString(init);
      return;
    }
    if (init instanceof URLSearchParams) {
      for (const pair of init._list) this._list.push([pair[0], pair[1]]);
      return;
    }
    if (Array.isArray(init)) {
      for (const pair of init) this._list.push([String(pair[0]), String(pair[1])]);
      return;
    }
    if (typeof init === 'object') {
      for (const k of Object.keys(init)) this._list.push([k, String(init[k])]);
    }
  }
  /** @returns {number} */
  get size() { return this._list.length; }
  /**
   * @param {string} name
   * @returns {string | null}
   */
  get(name) {
    for (const [k, v] of this._list) if (k === name) return v;
    return null;
  }
  /**
   * @param {string} name
   * @returns {string[]}
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
   * @param {string} value
   * @returns {void}
   */
  set(name, value) {
    let replaced = false;
    const out = /** @type {Array<[string, string]>} */ ([]);
    for (const pair of this._list) {
      if (pair[0] === name) {
        if (!replaced) { out.push([name, String(value)]); replaced = true; }
        continue;
      }
      out.push(pair);
    }
    if (!replaced) out.push([name, String(value)]);
    this._list = out;
    this._writeBackToOwner();
  }
  /**
   * @param {string} name
   * @param {string} value
   * @returns {void}
   */
  append(name, value) {
    this._list.push([name, String(value)]);
    this._writeBackToOwner();
  }
  /**
   * @param {string} name
   * @returns {void}
   */
  delete(name) {
    this._list = this._list.filter(([k]) => k !== name);
    this._writeBackToOwner();
  }
  /** @returns {void} */
  sort() {
    this._list.sort((a, b) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0));
    this._writeBackToOwner();
  }
  /**
   * @param {(value: string, name: string, params: URLSearchParams) => void} cb
   * @param {unknown} [thisArg]
   * @returns {void}
   */
  forEach(cb, thisArg) {
    for (const [k, v] of this._list) cb.call(thisArg, v, k, this);
  }
  /** @returns {IterableIterator<[string, string]>} */
  *entries() {
    for (const pair of this._list) yield [pair[0], pair[1]];
  }
  /** @returns {IterableIterator<string>} */
  *keys() {
    for (const [k] of this._list) yield k;
  }
  /** @returns {IterableIterator<string>} */
  *values() {
    for (const [, v] of this._list) yield v;
  }
  /** @returns {string} */
  toString() {
    const parts = [];
    for (const [k, v] of this._list) parts.push(`${_encodeFormPart(k)}=${_encodeFormPart(v)}`);
    return parts.join('&');
  }
  /**
   * @private
   * @returns {void}
   */
  _writeBackToOwner() {
    if (this._owner) this._owner._search = this.size > 0 ? '?' + this.toString() : '';
  }
}
URLSearchParams.prototype[Symbol.iterator] = URLSearchParams.prototype.entries;

// ---------------------------------------------------------------------------
// URL parser helpers (each is its own named top-level function — keep Boa GC happy)
// ---------------------------------------------------------------------------

/** @typedef {{ protocol: string, username: string, password: string, hostname: string, port: string, pathname: string, search: string, hash: string }} _ParsedUrl */

/**
 * Extract the scheme prefix (`http:`, `https:`, etc.) from a URL string.
 * @param {string} s
 * @returns {{ protocol: string, rest: string } | null}
 */
function _extractProtocol(s) {
  const m = /^([a-zA-Z][a-zA-Z0-9+\-.]*):/.exec(s);
  if (!m) return null;
  return { protocol: m[1].toLowerCase() + ':', rest: s.slice(m[0].length) };
}

/**
 * Split a `[user[:pass]@]host[:port]` authority into its parts.
 * @param {string} authority
 * @returns {{ username: string, password: string, hostname: string, port: string }}
 */
function _parseAuthority(authority) {
  let username = '';
  let password = '';
  let host = authority;
  const at = authority.lastIndexOf('@');
  if (at >= 0) {
    const userinfo = authority.slice(0, at);
    host = authority.slice(at + 1);
    const colon = userinfo.indexOf(':');
    if (colon < 0) { username = userinfo; }
    else { username = userinfo.slice(0, colon); password = userinfo.slice(colon + 1); }
  }
  const portColon = host.lastIndexOf(':');
  let hostname = host;
  let port = '';
  if (portColon >= 0 && /^[0-9]+$/.test(host.slice(portColon + 1))) {
    hostname = host.slice(0, portColon);
    port = host.slice(portColon + 1);
  }
  return { username, password, hostname: hostname.toLowerCase(), port };
}

/**
 * Split the path-query-fragment tail from an authority-less remainder.
 * @param {string} tail
 * @returns {{ pathname: string, search: string, hash: string }}
 */
function _splitTail(tail) {
  let hash = '';
  let search = '';
  let pathname = tail;
  const hashIdx = pathname.indexOf('#');
  if (hashIdx >= 0) {
    hash = pathname.slice(hashIdx);
    pathname = pathname.slice(0, hashIdx);
  }
  const queryIdx = pathname.indexOf('?');
  if (queryIdx >= 0) {
    search = pathname.slice(queryIdx);
    pathname = pathname.slice(0, queryIdx);
  }
  if (pathname === '') pathname = '/';
  return { pathname, search, hash };
}

/**
 * Parse an absolute URL string into its components. Returns `null` on
 * malformed input.
 * @param {string} str
 * @returns {_ParsedUrl | null}
 */
function _parseAbsolute(str) {
  const proto = _extractProtocol(str);
  if (!proto) return null;
  if (proto.rest.startsWith('//')) {
    const afterSlashes = proto.rest.slice(2);
    let stopAt = afterSlashes.length;
    for (const ch of '/?#') {
      const i = afterSlashes.indexOf(ch);
      if (i >= 0 && i < stopAt) stopAt = i;
    }
    const authority = afterSlashes.slice(0, stopAt);
    const tail = afterSlashes.slice(stopAt);
    const auth = _parseAuthority(authority);
    const { pathname, search, hash } = _splitTail(tail);
    return {
      protocol: proto.protocol,
      username: auth.username,
      password: auth.password,
      hostname: auth.hostname,
      port: auth.port,
      pathname,
      search,
      hash,
    };
  }
  return null;
}

/**
 * Resolve `input` against `base` for the common cases: absolute input wins;
 * otherwise treat `input` as a path replacement (relative or absolute path).
 * @param {string} input
 * @param {string | undefined} base
 * @returns {_ParsedUrl | null}
 */
function _resolveUrl(input, base) {
  const absolute = _parseAbsolute(input);
  if (absolute) return absolute;
  if (base == null) return null;
  const baseParsed = _parseAbsolute(base);
  if (!baseParsed) return null;
  if (input.startsWith('//')) {
    const merged = baseParsed.protocol + input;
    return _parseAbsolute(merged) ?? null;
  }
  if (input.startsWith('/')) {
    const { pathname, search, hash } = _splitTail(input);
    return { ...baseParsed, pathname, search, hash };
  }
  if (input.startsWith('?') || input.startsWith('#')) {
    const { pathname, search, hash } = _splitTail(input);
    return {
      ...baseParsed,
      pathname: pathname === '/' ? baseParsed.pathname : pathname,
      search,
      hash,
    };
  }
  const baseDir = baseParsed.pathname.replace(/[^/]*$/, '');
  const { pathname, search, hash } = _splitTail(baseDir + input);
  return { ...baseParsed, pathname, search, hash };
}

class URL {
  /**
   * @param {string | URL} input
   * @param {string | URL} [base]
   */
  constructor(input, base) {
    const parsed = _resolveUrl(String(input), base != null ? String(base) : undefined);
    if (!parsed) throw new TypeError(`zero test: invalid URL "${input}"`);
    this._protocol = parsed.protocol;
    this._username = parsed.username;
    this._password = parsed.password;
    this._hostname = parsed.hostname;
    this._port = parsed.port;
    this._pathname = parsed.pathname;
    this._search = parsed.search;
    this._hash = parsed.hash;
    /** @type {URLSearchParams | null} */
    this._searchParams = null;
  }
  /** @returns {string} */
  get protocol() { return this._protocol; }
  /** @param {string} v */ set protocol(v) {
    this._protocol = String(v).endsWith(':') ? String(v) : String(v) + ':';
  }
  /** @returns {string} */
  get username() { return this._username; }
  /** @param {string} v */ set username(v) { this._username = String(v); }
  /** @returns {string} */
  get password() { return this._password; }
  /** @param {string} v */ set password(v) { this._password = String(v); }
  /** @returns {string} */
  get hostname() { return this._hostname; }
  /** @param {string} v */ set hostname(v) { this._hostname = String(v).toLowerCase(); }
  /** @returns {string} */
  get port() { return this._port; }
  /** @param {string} v */ set port(v) { this._port = String(v); }
  /** @returns {string} */
  get host() { return this._port ? `${this._hostname}:${this._port}` : this._hostname; }
  /** @param {string} v */ set host(v) {
    const s = String(v);
    const colon = s.lastIndexOf(':');
    if (colon >= 0) { this._hostname = s.slice(0, colon).toLowerCase(); this._port = s.slice(colon + 1); }
    else { this._hostname = s.toLowerCase(); this._port = ''; }
  }
  /** @returns {string} */
  get pathname() { return this._pathname; }
  /** @param {string} v */ set pathname(v) {
    const s = String(v);
    this._pathname = s.startsWith('/') ? s : '/' + s;
  }
  /** @returns {string} */
  get search() { return this._search; }
  /** @param {string} v */ set search(v) {
    const s = String(v);
    this._search = s === '' ? '' : (s.startsWith('?') ? s : '?' + s);
    if (this._searchParams) {
      this._searchParams._list = _parseQueryString(this._search);
    }
  }
  /** @returns {string} */
  get hash() { return this._hash; }
  /** @param {string} v */ set hash(v) {
    const s = String(v);
    this._hash = s === '' ? '' : (s.startsWith('#') ? s : '#' + s);
  }
  /** @returns {string} */
  get origin() { return `${this._protocol}//${this.host}`; }
  /** @returns {URLSearchParams} */
  get searchParams() {
    if (this._searchParams) return this._searchParams;
    const sp = new URLSearchParams(this._search);
    sp._owner = this;
    this._searchParams = sp;
    return sp;
  }
  /** @returns {string} */
  toString() {
    const userinfo = this._username
      ? this._password
        ? `${this._username}:${this._password}@`
        : `${this._username}@`
      : '';
    return `${this._protocol}//${userinfo}${this.host}${this._pathname}${this._search}${this._hash}`;
  }
  /** @returns {string} */
  toJSON() { return this.toString(); }
}

/**
 * Static helper: probe whether `new URL(input, base?)` would succeed.
 * @param {string} input
 * @param {string} [base]
 * @returns {boolean}
 */
URL.canParse = function canParse(input, base) {
  try { new URL(input, base); return true; } catch (_) { return false; }
};

if (typeof globalThis.URLSearchParams === 'undefined') {
  Object.defineProperty(globalThis, 'URLSearchParams', {
    value: URLSearchParams, writable: true, configurable: true,
  });
}
if (typeof globalThis.URL === 'undefined') {
  Object.defineProperty(globalThis, 'URL', {
    value: URL, writable: true, configurable: true,
  });
}
