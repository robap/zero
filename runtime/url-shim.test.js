/**
 * Node-side tests for the URL / URLSearchParams shim. Evaluated in a fresh
 * `node:vm` sandbox to bypass Node's native URL / URLSearchParams classes.
 */

import { describe, it, before } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';

const shimSource = readFileSync(new URL('./url-shim.js', import.meta.url), 'utf8');

/** @type {any} */
let env;

before(() => {
  env = /** @type {any} */ ({});
  env.globalThis = env;
  vm.createContext(env);
  vm.runInContext(shimSource, env);
});

describe('URLSearchParams', () => {
  it('constructor from a string drops the leading "?"', () => {
    const p = new env.URLSearchParams('?a=1&b=2');
    assert.equal(p.get('a'), '1');
    assert.equal(p.get('b'), '2');
  });

  it('constructor from a plain object', () => {
    const p = new env.URLSearchParams({ a: '1', b: '2' });
    assert.equal(p.get('a'), '1');
    assert.equal(p.get('b'), '2');
  });

  it('constructor from an array of pairs', () => {
    const p = new env.URLSearchParams([['a', '1'], ['a', '2']]);
    assert.equal(JSON.stringify(p.getAll('a')), '["1","2"]');
  });

  it('copy-construct from another URLSearchParams', () => {
    const src = new env.URLSearchParams('a=1');
    const dst = new env.URLSearchParams(src);
    assert.equal(dst.get('a'), '1');
  });

  it('decodes "+" as space and percent-decoding works', () => {
    const p = new env.URLSearchParams('a=hello+world&b=%2Fpath');
    assert.equal(p.get('a'), 'hello world');
    assert.equal(p.get('b'), '/path');
  });

  it('set replaces all entries with a single one', () => {
    const p = new env.URLSearchParams('a=1&a=2');
    p.set('a', '3');
    assert.equal(JSON.stringify(p.getAll('a')), '["3"]');
  });

  it('append adds; getAll returns insertion order', () => {
    const p = new env.URLSearchParams();
    p.append('a', '1');
    p.append('a', '2');
    assert.equal(JSON.stringify(p.getAll('a')), '["1","2"]');
  });

  it('delete removes every entry for the name', () => {
    const p = new env.URLSearchParams('a=1&a=2&b=3');
    p.delete('a');
    assert.equal(p.has('a'), false);
    assert.equal(p.get('b'), '3');
  });

  it('toString encodes spaces as "+" and reserved characters', () => {
    const p = new env.URLSearchParams();
    p.set('q', 'hello world');
    p.set('z', '/path');
    assert.equal(p.toString(), 'q=hello+world&z=%2Fpath');
  });

  it('iteration yields pairs in insertion order', () => {
    const p = new env.URLSearchParams('a=1&b=2');
    const seen = [];
    for (const [k, v] of p) seen.push([k, v]);
    assert.equal(JSON.stringify(seen), '[["a","1"],["b","2"]]');
  });

  it('size reflects total pair count', () => {
    const p = new env.URLSearchParams('a=1&a=2&b=3');
    assert.equal(p.size, 3);
  });
});

describe('URL', () => {
  it('parses a simple absolute URL', () => {
    const u = new env.URL('https://example.com/a/b?x=1#h');
    assert.equal(u.protocol, 'https:');
    assert.equal(u.hostname, 'example.com');
    assert.equal(u.pathname, '/a/b');
    assert.equal(u.search, '?x=1');
    assert.equal(u.hash, '#h');
  });

  it('exposes port when present', () => {
    const u = new env.URL('http://example.com:8080/');
    assert.equal(u.port, '8080');
    assert.equal(u.host, 'example.com:8080');
  });

  it('resolves a relative path against a base URL', () => {
    const u = new env.URL('/x', 'https://h.test/y');
    assert.equal(u.pathname, '/x');
    assert.equal(u.hostname, 'h.test');
  });

  it('searchParams reads from query', () => {
    const u = new env.URL('https://x.test/?a=1&b=2');
    assert.equal(u.searchParams.get('a'), '1');
    assert.equal(u.searchParams.get('b'), '2');
  });

  it('searchParams mutations write back to search', () => {
    const u = new env.URL('https://x.test/');
    u.searchParams.set('a', '1');
    assert.equal(u.search, '?a=1');
  });

  it('toString reassembles all parts', () => {
    const u = new env.URL('https://example.com/a/b?x=1#h');
    assert.equal(u.toString(), 'https://example.com/a/b?x=1#h');
  });

  it('URL.canParse returns true for valid URLs', () => {
    assert.equal(env.URL.canParse('https://x.test/'), true);
  });

  it('URL.canParse returns false for invalid input', () => {
    assert.equal(env.URL.canParse('not a url'), false);
  });
});
