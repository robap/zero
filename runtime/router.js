import { _getCurrentApp } from './app.js';

/**
 * Navigate to a new path, pushing (or replacing) a history entry.
 * Throws if no app is running.
 * @param {string} path
 * @param {{ replace?: boolean, state?: unknown }} [opts]
 */
export function navigate(path, opts = {}) {
  const app = _getCurrentApp();
  if (!app) throw new Error('navigate: no app is running');
  const state = opts.state ?? null;
  if (opts.replace) window.history.replaceState(state, '', path);
  else window.history.pushState(state, '', path);
  app._navigateTo(path);
}

/**
 * Go back one entry in the browser history.
 * The popstate listener on the running app handles the resulting navigation.
 * Throws if no app is running.
 */
export function back() {
  if (!_getCurrentApp()) throw new Error('back: no app is running');
  window.history.back();
}

/**
 * Go forward one entry in the browser history.
 * The popstate listener on the running app handles the resulting navigation.
 * Throws if no app is running.
 */
export function forward() {
  if (!_getCurrentApp()) throw new Error('forward: no app is running');
  window.history.forward();
}

/**
 * Return a reactive snapshot of the current route.
 * Reading `.path`, `.params`, or `.query` inside an effect or template reactive
 * block subscribes to the underlying signals on the running app.
 * Each call returns a new proxy object; the signals are shared.
 * Throws if no app is running.
 * @returns {{ readonly path: string, readonly params: Record<string,string>, readonly query: Record<string,string> }}
 */
export function route() {
  const app = _getCurrentApp();
  if (!app) throw new Error('route: no app is running');
  return {
    get path() { return app._pathSig.val; },
    get params() { return app._paramsSig.val; },
    get query() { return app._querySig.val; },
  };
}

/**
 * Strip a single trailing slash unless the path is the root `/`.
 * @internal
 * @param {string} p
 * @returns {string}
 */
export function _normalizePath(p) {
  if (p === '/') return p;
  return p.endsWith('/') ? p.slice(0, -1) : p;
}

/**
 * Join a parent normalized path with a child path segment.
 * - child `'/'` → parent (exact-parent-match).
 * - parent `'/'` → child (avoid double-slash).
 * - else → `parent + child`.
 * Result is normalized via `_normalizePath`.
 * @internal
 * @param {string} parent - Already-normalized parent path.
 * @param {string} child - Child path segment, begins with `/`.
 * @returns {string}
 */
export function _joinPaths(parent, child) {
  const p = _normalizePath(parent);
  if (child === '/') return p;
  if (p === '/') return _normalizePath(child);
  return _normalizePath(p + child);
}

/**
 * Parse a query string into a plain object.
 * Accepts `''`, `'?'`, or `'?k=v&k2=v2'`. Keys and values are
 * `decodeURIComponent`-decoded. Repeated keys: last wins.
 * @internal
 * @param {string} search
 * @returns {Record<string, string>}
 */
export function _parseQuery(search) {
  if (!search || search === '?') return {};
  const qs = search.startsWith('?') ? search.slice(1) : search;
  const result = {};
  for (const part of qs.split('&')) {
    const eq = part.indexOf('=');
    if (eq === -1) {
      result[decodeURIComponent(part)] = '';
    } else {
      result[decodeURIComponent(part.slice(0, eq))] = decodeURIComponent(part.slice(eq + 1));
    }
  }
  return result;
}

/**
 * Split a `pathname[?query][#hash]` string into `{ pathname, search }`.
 * Hash is dropped. `search` retains its leading `?`.
 * @internal
 * @param {string} input
 * @returns {{ pathname: string, search: string }}
 */
export function _parsePathAndQuery(input) {
  const hashIdx = input.indexOf('#');
  const noHash = hashIdx >= 0 ? input.slice(0, hashIdx) : input;
  const qIdx = noHash.indexOf('?');
  if (qIdx >= 0) {
    return { pathname: noHash.slice(0, qIdx), search: noHash.slice(qIdx) };
  }
  return { pathname: noHash, search: '' };
}

/**
 * Compile a route pattern into a regex and param-name list.
 * Supports exact paths, `:name` segments, and the bare `*` wildcard.
 * @internal
 * @param {string} pattern
 * @returns {{ pattern: string, normalized: string, paramNames: string[], regex: RegExp, isWildcard: boolean }}
 */
export function _compileRoutePattern(pattern) {
  if (pattern === '*') {
    return { pattern, normalized: '*', paramNames: [], regex: /^.*$/, isWildcard: true };
  }
  const normalized = _normalizePath(pattern);
  const paramNames = [];
  const segments = normalized.split('/').map(seg => {
    if (seg.startsWith(':')) {
      paramNames.push(seg.slice(1));
      return '([^/]+)';
    }
    return seg.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  });
  const regex = new RegExp('^' + segments.join('\\/') + '$');
  return { pattern, normalized, paramNames, regex, isWildcard: false };
}

/**
 * Test a compiled route pattern against a normalized pathname.
 * @internal
 * @param {{ regex: RegExp, paramNames: string[] }} compiled
 * @param {string} pathname
 * @returns {{ params: Record<string,string> } | null}
 */
export function _matchAgainst(compiled, pathname) {
  const m = compiled.regex.exec(pathname);
  if (!m) return null;
  const params = {};
  for (let i = 0; i < compiled.paramNames.length; i++) {
    params[compiled.paramNames[i]] = decodeURIComponent(m[i + 1]);
  }
  return { params };
}

/**
 * Match an input path-and-query string against an ordered list of route entries.
 * Returns the first match with parsed params and query, or `null`.
 * @internal
 * @param {Array<{ compiled: object, loader: Function }>} routeEntries
 * @param {string} input
 * @returns {{ route: object, params: Record<string,string>, query: Record<string,string>, pathname: string, search: string } | null}
 */
export function _matchRoutes(routeEntries, input) {
  const { pathname, search } = _parsePathAndQuery(input);
  const normalizedPath = _normalizePath(pathname);
  const query = _parseQuery(search);
  for (const route of routeEntries) {
    const hit = _matchAgainst(route.compiled, normalizedPath);
    if (hit) return { route, params: hit.params, query, pathname: normalizedPath, search };
  }
  return null;
}
