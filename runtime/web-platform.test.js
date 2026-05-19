/**
 * Smoke test for the Web Platform surface that `zero test` ships. One `it`
 * per audited API; if every test passes, the closed enumeration documented
 * in `zero-framework-spec.md` §8 "Web Platform surface in `zero test`"
 * works for the canonical user path under Node's native globals. Running
 * the same APIs under Boa is covered by
 * `crates/zero-test-runner/tests/web_platform_probe.rs`.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { createHttp } from './http.js';

describe('web platform surface', () => {
  it('Headers / Request / Response stub a full http.js call', async () => {
    /** @type {typeof fetch} */
    const stub = async () => new Response(JSON.stringify({ a: 1 }), {
      status: 200,
      headers: { 'Content-Type': 'application/json' },
    });
    const client = createHttp({ fetch: stub });
    const body = await client.get('http://api.test/x');
    assert.equal(JSON.stringify(body), '{"a":1}');
  });

  it('AbortController fires abort and propagates reason', () => {
    const c = new AbortController();
    c.abort('done');
    assert.equal(c.signal.aborted, true);
    assert.equal(c.signal.reason, 'done');
  });

  it('AbortSignal.any fires when the first input aborts', () => {
    const a = new AbortController();
    const b = new AbortController();
    const composite = AbortSignal.any([a.signal, b.signal]);
    assert.equal(composite.aborted, false);
    b.abort('b-reason');
    assert.equal(composite.aborted, true);
  });

  it('URL and URLSearchParams round-trip a query string', () => {
    const u = new URL('https://x.test/path?a=1&b=2');
    assert.equal(u.searchParams.get('a'), '1');
    u.searchParams.set('a', '3');
    assert.ok(u.search.includes('a=3'));
  });

  it('TextEncoder / TextDecoder round-trip a non-ASCII string', () => {
    const bytes = new TextEncoder().encode('héllo 😀');
    assert.equal(new TextDecoder().decode(bytes), 'héllo 😀');
  });

  it('Blob.text() returns the constructed parts', async () => {
    const b = new Blob(['hi', '!']);
    assert.equal(await b.text(), 'hi!');
  });

  it('File extends Blob and adds name', () => {
    const f = new File(['p'], 'note.txt', { type: 'text/plain' });
    assert.equal(f.name, 'note.txt');
    assert.ok(f instanceof Blob);
  });

  it('FormData append/get round-trip', () => {
    const fd = new FormData();
    fd.append('a', '1');
    assert.equal(fd.get('a'), '1');
  });

  it('structuredClone deep-copies a nested object with a cycle', () => {
    const o = /** @type {any} */ ({ a: { b: 1 } });
    o.self = o;
    const copy = structuredClone(o);
    assert.notEqual(copy, o);
    assert.equal(copy.self, copy);
    assert.equal(copy.a.b, 1);
  });

  it('queueMicrotask runs the callback after a microtask boundary', async () => {
    let ran = false;
    queueMicrotask(() => { ran = true; });
    assert.equal(ran, false);
    await Promise.resolve();
    await Promise.resolve();
    assert.equal(ran, true);
  });

  it('Promise.withResolvers gives external resolve/reject', async () => {
    const { promise, resolve } = Promise.withResolvers();
    resolve(42);
    assert.equal(await promise, 42);
  });
});
