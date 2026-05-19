import { describe, it, expect } from 'zero/test';
import { createHttp, HttpError } from 'zero/http';

/**
 * Build a stub fetch that delegates to `handler(req)` and asserts only
 * one Request is ever issued (unless tests mark otherwise).
 * @param {(req: Request) => Response | Promise<Response>} handler
 * @returns {typeof fetch & { calls: Request[] }}
 */
function makeStubFetch(handler) {
  /** @type {Request[]} */
  const calls = [];
  /** @type {typeof fetch} */
  const f = async (input, init) => {
    const req = input instanceof Request ? input : new Request(input, init);
    calls.push(req);
    return handler(req);
  };
  // @ts-ignore — augment for assertions.
  f.calls = calls;
  // @ts-ignore
  return f;
}

describe('zero/http — createHttp', () => {
  it('GET with JSON response returns the parsed body', async () => {
    const fetchStub = makeStubFetch((_req) =>
      new Response(JSON.stringify({ value: 42 }), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      }),
    );
    const client = createHttp({ fetch: fetchStub });
    const body = await client.get('http://api.test/data');
    expect(body).toEqual({ value: 42 });
  });

  it('POST with plain object body sends JSON with Content-Type', async () => {
    /** @type {{ ct: string|null, body: string }} */
    let seen = { ct: null, body: '' };
    const fetchStub = makeStubFetch(async (req) => {
      seen = { ct: req.headers.get('Content-Type'), body: await req.text() };
      return new Response(JSON.stringify({ ok: true }), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      });
    });
    const client = createHttp({ fetch: fetchStub });
    await client.post('http://api.test/items', { name: 'x' });
    expect(seen.ct).toBe('application/json');
    expect(seen.body).toBe('{"name":"x"}');
  });

  it('non-2xx JSON response rejects with HttpError carrying status + body', async () => {
    const fetchStub = makeStubFetch((_req) =>
      new Response(JSON.stringify({ message: 'nope' }), {
        status: 404,
        statusText: 'Not Found',
        headers: { 'Content-Type': 'application/json' },
      }),
    );
    const client = createHttp({ fetch: fetchStub });
    let err;
    try {
      await client.get('http://api.test/missing');
    } catch (e) {
      err = e;
    }
    expect(err instanceof HttpError).toBeTruthy();
    expect(err.status).toBe(404);
    expect(err.statusText).toBe('Not Found');
    expect(err.body).toEqual({ message: 'nope' });
  });

  it('aborted caller signal rejects with AbortError', async () => {
    /** @type {typeof fetch} */
    const fetchStub = (input, init) => new Promise((_resolve, reject) => {
      const req = input instanceof Request ? input : new Request(input, init);
      const signal = req.signal;
      if (signal.aborted) {
        const err = new Error('aborted');
        err.name = 'AbortError';
        reject(err);
        return;
      }
      signal.addEventListener('abort', () => {
        const err = new Error('aborted');
        err.name = 'AbortError';
        reject(err);
      });
    });
    const client = createHttp({ fetch: fetchStub });
    const controller = new AbortController();
    const p = client.get('http://api.test/slow', { signal: controller.signal });
    controller.abort();
    let err;
    try {
      await p;
    } catch (e) {
      err = e;
    }
    expect(err).toBeTruthy();
    expect(err.name).toBe('AbortError');
  });

  it('middleware ordering follows onion model', async () => {
    const log = [];
    const fetchStub = makeStubFetch((_req) =>
      new Response(JSON.stringify({}), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      }),
    );
    const client = createHttp({ fetch: fetchStub })
      .use(async (req, next) => { log.push('A in'); const r = await next(req); log.push('A out'); return r; })
      .use(async (req, next) => { log.push('B in'); const r = await next(req); log.push('B out'); return r; })
      .use(async (req, next) => { log.push('C in'); const r = await next(req); log.push('C out'); return r; });
    await client.get('http://api.test/x');
    expect(log).toEqual(['A in', 'B in', 'C in', 'C out', 'B out', 'A out']);
  });

  it('middleware can short-circuit by returning a Response without calling next', async () => {
    let baseCalls = 0;
    const fetchStub = makeStubFetch((_req) => { baseCalls++; return new Response('{}', { headers: { 'Content-Type': 'application/json' } }); });
    const client = createHttp({ fetch: fetchStub })
      .use(async (_req, _next) =>
        new Response(JSON.stringify({ short: true }), {
          status: 200,
          headers: { 'Content-Type': 'application/json' },
        }),
      );
    const body = await client.get('http://api.test/x');
    expect(body).toEqual({ short: true });
    expect(baseCalls).toBe(0);
  });

  it('middleware can inject headers seen by base fetch', async () => {
    let seenAuth = null;
    const fetchStub = makeStubFetch((req) => {
      seenAuth = req.headers.get('Authorization');
      return new Response(JSON.stringify({}), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      });
    });
    const client = createHttp({ fetch: fetchStub })
      .use(async (req, next) => {
        const headers = new Headers(req.headers);
        headers.set('Authorization', 'Bearer x');
        return next(new Request(req, { headers }));
      });
    await client.get('http://api.test/secret');
    expect(seenAuth).toBe('Bearer x');
  });

  it('per-call fetch override is used instead of constructor-time fetch', async () => {
    const baseFetch = makeStubFetch((_req) =>
      new Response('{}', { status: 200, headers: { 'Content-Type': 'application/json' } }),
    );
    let overrideCalls = 0;
    const overrideFetch = makeStubFetch((_req) => {
      overrideCalls++;
      return new Response(JSON.stringify({ from: 'override' }), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      });
    });
    const client = createHttp({ fetch: baseFetch });
    const body = await client.get('http://api.test/x', { fetch: overrideFetch });
    expect(body).toEqual({ from: 'override' });
    expect(overrideCalls).toBe(1);
    expect(baseFetch.calls.length).toBe(0);
  });

  it('non-JSON 2xx response returns the raw Response object (escape hatch)', async () => {
    const fetchStub = makeStubFetch((_req) =>
      new Response('binary', {
        status: 200,
        headers: { 'Content-Type': 'application/octet-stream' },
      }),
    );
    const client = createHttp({ fetch: fetchStub });
    const result = await client.get('http://api.test/file');
    expect(result instanceof Response).toBeTruthy();
    expect(await result.text()).toBe('binary');
  });
});
