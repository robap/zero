/**
 * Smoke test for the Web Platform surface that `zero test` ships. One `it`
 * per audited API; if every test passes, the closed enumeration documented
 * in `docs/testing.md` (Web Platform surface) works under Boa.
 */

import { describe, it, expect } from 'zero/test';

describe('web platform surface', () => {
  it('Headers / Request / Response work directly', async () => {
    const headers = new Headers({ 'Content-Type': 'application/json' });
    expect(headers.get('content-type')).toBe('application/json');

    const req = new Request('http://api.test/x', { method: 'POST' });
    expect(req.method).toBe('POST');
    expect(req.url).toBe('http://api.test/x');

    const res = new Response(JSON.stringify({ a: 1 }), {
      status: 200,
      headers: { 'Content-Type': 'application/json' },
    });
    expect(res.status).toBe(200);
    expect(await res.text()).toBe('{"a":1}');
  });

  it('AbortController fires abort and propagates reason', () => {
    const c = new AbortController();
    c.abort('done');
    expect(c.signal.aborted).toBe(true);
    expect(c.signal.reason).toBe('done');
  });

  it('AbortSignal.any fires when one input aborts', () => {
    const a = new AbortController();
    const b = new AbortController();
    const composite = AbortSignal.any([a.signal, b.signal]);
    expect(composite.aborted).toBe(false);
    b.abort('b-reason');
    expect(composite.aborted).toBe(true);
  });

  it('URL and URLSearchParams round-trip a query string', () => {
    const u = new URL('https://x.test/path?a=1&b=2');
    expect(u.searchParams.get('a')).toBe('1');
    u.searchParams.set('a', '3');
    expect(u.search).toContain('a=3');
  });

  it('TextEncoder / TextDecoder round-trip a non-ASCII string', () => {
    const bytes = new TextEncoder().encode('héllo 😀');
    expect(new TextDecoder().decode(bytes)).toBe('héllo 😀');
  });

  it('Blob.text() returns the constructed parts', async () => {
    const b = new Blob(['hi', '!']);
    expect(await b.text()).toBe('hi!');
  });

  it('File extends Blob and adds name', () => {
    const f = new File(['p'], 'note.txt', { type: 'text/plain' });
    expect(f.name).toBe('note.txt');
    expect(f instanceof Blob).toBeTruthy();
  });

  it('FormData append/get round-trip', () => {
    const fd = new FormData();
    fd.append('a', '1');
    expect(fd.get('a')).toBe('1');
  });

  it('structuredClone deep-copies a nested object with a cycle', () => {
    const o = /** @type {any} */ ({ a: { b: 1 } });
    o.self = o;
    const copy = structuredClone(o);
    expect(copy === o).toBeFalsy();
    expect(copy.self).toBe(copy);
    expect(copy.a.b).toBe(1);
  });

  it('queueMicrotask runs the callback after a microtask boundary', async () => {
    let ran = false;
    queueMicrotask(() => { ran = true; });
    expect(ran).toBe(false);
    await Promise.resolve();
    await Promise.resolve();
    expect(ran).toBe(true);
  });

  it('Promise.withResolvers gives external resolve/reject', async () => {
    const { promise, resolve } = Promise.withResolvers();
    resolve(42);
    expect(await promise).toBe(42);
  });

  it('Intl.DateTimeFormat formats en-US dates', () => {
    const d = new Date(2024, 0, 5, 15, 7, 9);
    const out = new Intl.DateTimeFormat('en-US', {
      month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit',
    }).format(d);
    expect(out).toBe('Jan 5, 3:07 PM');
  });
});
