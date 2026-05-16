/**
 * `"zero/http"` — a thin, middleware-aware HTTP client over `fetch`.
 */

/**
 * Error thrown when a request resolves with a non-2xx response.
 */
export class HttpError extends Error {
  /**
   * @param {number} status
   * @param {string} statusText
   * @param {unknown} body
   */
  constructor(status, statusText, body) {
    super(`HTTP ${status} ${statusText}`);
    this.name = "HttpError";
    /** @type {number} */
    this.status = status;
    /** @type {string} */
    this.statusText = statusText;
    /** @type {unknown} */
    this.body = body;
  }
}

/**
 * @typedef {(req: Request, next: (req: Request) => Promise<Response>) => Promise<Response>} Middleware
 */

/**
 * @typedef {RequestInit & { fetch?: typeof fetch }} HttpInit
 */

/**
 * Construct a new HTTP client.
 * @param {{ fetch?: typeof fetch }} [opts]
 * @returns {object}
 */
export function createHttp(opts = {}) {
  const baseFetch = opts.fetch ?? globalThis.fetch;
  /** @type {Middleware[]} */
  const middlewares = [];

  /**
   * @param {string} method
   * @param {string} url
   * @param {unknown} body
   * @param {HttpInit | undefined} init
   * @returns {Promise<unknown>}
   */
  function request(method, url, body, init) {
    return _send(method, url, body, init, middlewares, baseFetch);
  }

  const client = {
    use(mw) {
      if (typeof mw !== "function") {
        throw new TypeError("HttpClient.use: middleware must be a function");
      }
      middlewares.push(mw);
      return client;
    },
    get(url, init) {
      return request("GET", url, undefined, init);
    },
    post(url, body, init) {
      return request("POST", url, body, init);
    },
    put(url, body, init) {
      return request("PUT", url, body, init);
    },
    patch(url, body, init) {
      return request("PATCH", url, body, init);
    },
    delete(url, init) {
      return request("DELETE", url, undefined, init);
    },
    request(input, init) {
      return _sendRequestLike(input, init, middlewares, baseFetch);
    },
  };
  return client;
}

/**
 * @internal
 * @param {string} method
 * @param {string} url
 * @param {unknown} body
 * @param {HttpInit | undefined} init
 * @param {Middleware[]} middlewares
 * @param {typeof fetch} baseFetch
 * @returns {Promise<unknown>}
 */
function _send(method, url, body, init, middlewares, baseFetch) {
  const { fetch: perCallFetch, ...rest } = init ?? {};
  /** @type {RequestInit} */
  const requestInit = { ...rest, method };
  const headers = new Headers(requestInit.headers || {});
  if (body !== undefined) {
    if (_isPlainObject(body) || Array.isArray(body)) {
      if (!headers.has("Content-Type")) {
        headers.set("Content-Type", "application/json");
      }
      requestInit.body = JSON.stringify(body);
    } else {
      requestInit.body = /** @type {BodyInit} */ (body);
    }
  }
  requestInit.headers = headers;
  const req = new Request(url, requestInit);
  return _dispatch(req, middlewares, perCallFetch ?? baseFetch);
}

/**
 * @internal
 * @param {Request | URL | string} input
 * @param {HttpInit | undefined} init
 * @param {Middleware[]} middlewares
 * @param {typeof fetch} baseFetch
 * @returns {Promise<unknown>}
 */
function _sendRequestLike(input, init, middlewares, baseFetch) {
  const { fetch: perCallFetch, ...rest } = init ?? {};
  const req =
    input instanceof Request && Object.keys(rest).length === 0
      ? input
      : new Request(input, rest);
  return _dispatch(req, middlewares, perCallFetch ?? baseFetch);
}

/**
 * Onion-walk the middleware chain. Innermost layer calls `baseFetch(req)`.
 * @internal
 * @param {Request} req
 * @param {Middleware[]} middlewares
 * @param {typeof fetch} baseFetch
 * @returns {Promise<unknown>}
 */
async function _dispatch(req, middlewares, baseFetch) {
  /** @param {number} i @returns {(req: Request) => Promise<Response>} */
  const make = (i) => async (nextReq) => {
    if (i >= middlewares.length) {
      return baseFetch(nextReq);
    }
    return middlewares[i](nextReq, make(i + 1));
  };
  const response = await make(0)(req);
  return _readResponse(response);
}

/**
 * @internal
 * @param {Response} response
 * @returns {Promise<unknown>}
 */
async function _readResponse(response) {
  const contentType = response.headers.get("Content-Type") || "";
  const isJson = /\bjson\b/i.test(contentType);
  if (!response.ok) {
    let body;
    if (isJson) {
      try { body = await response.json(); } catch (_) { body = undefined; }
    } else {
      try { body = await response.text(); } catch (_) { body = undefined; }
    }
    throw new HttpError(response.status, response.statusText, body);
  }
  if (isJson) {
    return response.json();
  }
  return response;
}

/**
 * @internal
 * @param {unknown} value
 * @returns {boolean}
 */
function _isPlainObject(value) {
  if (value === null || typeof value !== "object") return false;
  if (typeof FormData !== "undefined" && value instanceof FormData) return false;
  if (typeof Blob !== "undefined" && value instanceof Blob) return false;
  if (value instanceof ArrayBuffer) return false;
  if (ArrayBuffer.isView(value)) return false;
  if (typeof URLSearchParams !== "undefined" && value instanceof URLSearchParams) return false;
  if (typeof ReadableStream !== "undefined" && value instanceof ReadableStream) return false;
  const proto = Object.getPrototypeOf(value);
  return proto === Object.prototype || proto === null;
}
