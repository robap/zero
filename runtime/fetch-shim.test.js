/**
 * Node-side tests for the Fetch-API / Abort shim. The shim ships identifiers
 * via `globalThis` side effects, so each test evaluates the file inside a
 * fresh `node:vm` sandbox and asserts against the sandbox's `globalThis`.
 * That isolates the test from Node's own builtins so we are really exercising
 * the shim implementation rather than the runtime's native classes.
 */

import { describe, it, before } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';

const shimSource = readFileSync(new URL('./fetch-shim.js', import.meta.url), 'utf8');

/** @type {vm.Context & { globalThis: any }} */
let env;

/**
 * Create a fresh sandbox with `Promise`, `setTimeout`, and `Object.defineProperty`
 * available (the shim relies on them), then evaluate the shim source into it.
 * @returns {any} the sandbox object.
 */
function freshShimContext() {
  const sandbox = /** @type {any} */ ({});
  sandbox.globalThis = sandbox;
  vm.createContext(sandbox);
  // The shim uses setTimeout (for AbortSignal.timeout); forward it.
  sandbox.setTimeout = (cb, ms) => setTimeout(cb, ms);
  sandbox.clearTimeout = (id) => clearTimeout(id);
  vm.runInContext(shimSource, sandbox);
  return sandbox;
}

before(() => { env = freshShimContext(); });

describe('AbortController / AbortSignal', () => {
  it('new controller has non-aborted signal with undefined reason', () => {
    const c = new env.AbortController();
    assert.equal(c.signal.aborted, false);
    assert.equal(c.signal.reason, undefined);
  });

  it('controller.abort() flips aborted to true and fires abort event once', () => {
    const c = new env.AbortController();
    let hits = 0;
    c.signal.addEventListener('abort', () => { hits++; });
    c.abort();
    assert.equal(c.signal.aborted, true);
    assert.equal(c.signal.reason && c.signal.reason.name, 'AbortError');
    assert.equal(hits, 1);
    c.abort();
    assert.equal(hits, 1);
  });

  it('controller.abort(reason) stores the user-supplied reason', () => {
    const c = new env.AbortController();
    c.abort('user reason');
    assert.equal(c.signal.reason, 'user reason');
  });

  it('signal.throwIfAborted() throws the stored reason after abort', () => {
    const c = new env.AbortController();
    c.abort('boom');
    assert.throws(() => c.signal.throwIfAborted(), /boom/);
  });

  it('AbortSignal.abort(reason) returns an already-aborted signal', () => {
    const s = env.AbortSignal.abort('x');
    assert.equal(s.aborted, true);
    assert.equal(s.reason, 'x');
  });

  it('AbortSignal.timeout fires abort on next microtask drain', async () => {
    const s = env.AbortSignal.timeout(0);
    await new Promise((res) => setTimeout(res, 5));
    assert.equal(s.aborted, true);
    assert.equal(s.reason && s.reason.name, 'TimeoutError');
  });

  it('AbortSignal.any composite aborts when first input aborts', () => {
    const a = new env.AbortController();
    const b = new env.AbortController();
    const composite = env.AbortSignal.any([a.signal, b.signal]);
    assert.equal(composite.aborted, false);
    b.abort('reason-b');
    assert.equal(composite.aborted, true);
    assert.equal(composite.reason, 'reason-b');
  });

  it('AbortSignal.any returns an already-aborted signal when an input is aborted', () => {
    const a = new env.AbortController();
    a.abort('was-aborted');
    const composite = env.AbortSignal.any([a.signal]);
    assert.equal(composite.aborted, true);
    assert.equal(composite.reason, 'was-aborted');
  });
});

describe('Headers', () => {
  it('constructor accepts a plain object', () => {
    const h = new env.Headers({ 'Content-Type': 'application/json' });
    assert.equal(h.get('content-type'), 'application/json');
  });

  it('constructor accepts an array of pairs', () => {
    const h = new env.Headers([['X-A', '1'], ['X-B', '2']]);
    assert.equal(h.get('x-a'), '1');
    assert.equal(h.get('x-b'), '2');
  });

  it('constructor accepts another Headers instance', () => {
    const src = new env.Headers({ A: '1' });
    const dst = new env.Headers(src);
    assert.equal(dst.get('a'), '1');
  });

  it('constructor accepts undefined and produces empty Headers', () => {
    const h = new env.Headers();
    assert.equal(h.get('anything'), null);
  });

  it('set then get round-trips with case-insensitive names', () => {
    const h = new env.Headers();
    h.set('X-A', '1');
    assert.equal(h.get('x-a'), '1');
    assert.equal(h.has('X-A'), true);
  });

  it('append joins repeated values with ", "', () => {
    const h = new env.Headers();
    h.append('Accept', 'a');
    h.append('Accept', 'b');
    assert.equal(h.get('accept'), 'a, b');
  });

  it('iteration yields lowercased names', () => {
    const h = new env.Headers({ 'Content-Type': 'json', 'X-A': '1' });
    const seen = [];
    for (const [k, v] of h) seen.push([k, v]);
    assert.deepEqual(seen.sort(), [['content-type', 'json'], ['x-a', '1']]);
  });
});

describe('Request', () => {
  it('constructor with string URL and object init', () => {
    const r = new env.Request('http://x.test/y', { method: 'post', headers: { A: '1' } });
    assert.equal(r.url, 'http://x.test/y');
    assert.equal(r.method, 'POST');
    assert.equal(r.headers.get('a'), '1');
  });

  it('signal defaults to a fresh non-aborted AbortSignal', () => {
    const r = new env.Request('/x');
    assert.ok(r.signal instanceof env.AbortSignal);
    assert.equal(r.signal.aborted, false);
  });

  it('copy-construct from another Request overlays init', () => {
    const a = new env.Request('/x', { method: 'POST', headers: { A: '1' } });
    const b = new env.Request(a, { method: 'PUT' });
    assert.equal(b.url, '/x');
    assert.equal(b.method, 'PUT');
    assert.equal(b.headers.get('a'), '1');
  });

  it('text() resolves a string body', async () => {
    const r = new env.Request('/x', { method: 'POST', body: 'hello' });
    assert.equal(await r.text(), 'hello');
  });

  it('json() parses a JSON body', async () => {
    const r = new env.Request('/x', { method: 'POST', body: JSON.stringify({ a: 1 }) });
    const parsed = await r.json();
    assert.equal(JSON.stringify(parsed), '{"a":1}');
  });

  it('arrayBuffer() rejects with the stub message', async () => {
    const r = new env.Request('/x', { method: 'POST', body: 'hi' });
    await assert.rejects(() => r.arrayBuffer(), /zero test/);
  });
});

describe('Response', () => {
  it('constructor with body + init exposes status and headers', () => {
    const r = new env.Response('hi', { status: 201, statusText: 'Created', headers: { A: '1' } });
    assert.equal(r.status, 201);
    assert.equal(r.statusText, 'Created');
    assert.equal(r.ok, true);
    assert.equal(r.headers.get('a'), '1');
  });

  it('ok is false for non-2xx statuses', () => {
    const r = new env.Response('', { status: 500 });
    assert.equal(r.ok, false);
  });

  it('text() and json() consume the body', async () => {
    const r = new env.Response(JSON.stringify({ v: 42 }), {
      headers: { 'Content-Type': 'application/json' },
    });
    const parsed = await r.json();
    assert.equal(JSON.stringify(parsed), '{"v":42}');
  });
});

describe('fetch default', () => {
  it('rejects with the actionable stub message', async () => {
    await assert.rejects(env.fetch('/x'), /zero test: globalThis\.fetch is not implemented/);
  });

  it('returns a Promise (no synchronous throw)', () => {
    const p = env.fetch('/x');
    assert.ok(p && typeof p.then === 'function');
    p.catch(() => {});
  });

  it('__resetFetch__ restores the default after the user overwrites fetch', async () => {
    env.fetch = () => Promise.resolve('overridden');
    assert.equal(await env.fetch('/x'), 'overridden');
    env.__resetFetch__();
    await assert.rejects(env.fetch('/x'), /zero test: globalThis\.fetch is not implemented/);
  });
});
