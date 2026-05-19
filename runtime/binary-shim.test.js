/**
 * Node-side tests for the Blob / File / FormData shim. Evaluated in a fresh
 * `node:vm` sandbox. The shim depends on TextEncoder/TextDecoder, so we
 * concatenate the encoding shim source first.
 */

import { describe, it, before } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';

const encodingSource = readFileSync(new URL('./encoding-shim.js', import.meta.url), 'utf8');
const binarySource = readFileSync(new URL('./binary-shim.js', import.meta.url), 'utf8');

/** @type {any} */
let env;

before(() => {
  env = /** @type {any} */ ({});
  env.globalThis = env;
  vm.createContext(env);
  vm.runInContext(encodingSource, env);
  vm.runInContext(binarySource, env);
  // Surface typed-array constructors on the sandbox object so tests can
  // `new env.Uint8Array(...)`. `vm.createContext` makes these visible inside
  // the sandbox but does not bind them on the sandbox object itself.
  vm.runInContext('globalThis.Uint8Array = Uint8Array;', env);
});

describe('Blob', () => {
  it('constructs from string parts and exposes size + type', () => {
    const b = new env.Blob(['hi', '!'], { type: 'text/plain' });
    assert.equal(b.size, 3);
    assert.equal(b.type, 'text/plain');
  });

  it('text() returns the concatenated parts', async () => {
    const b = new env.Blob(['hello ', 'world']);
    assert.equal(await b.text(), 'hello world');
  });

  it('arrayBuffer() returns a fresh buffer the size of the bytes', async () => {
    const b = new env.Blob(['ab']);
    const buf = await b.arrayBuffer();
    assert.equal(buf.byteLength, 2);
  });

  it('slice returns a Blob covering the sub-range', async () => {
    const b = new env.Blob(['abcdef']);
    const part = b.slice(1, 4, 'text/x');
    assert.equal(part.size, 3);
    assert.equal(part.type, 'text/x');
    assert.equal(await part.text(), 'bcd');
  });

  it('accepts a Uint8Array part', async () => {
    const bytes = new env.Uint8Array([104, 105]);
    const b = new env.Blob([bytes]);
    assert.equal(await b.text(), 'hi');
  });

  it('accepts mixed parts (string + typed array)', async () => {
    const bytes = new env.Uint8Array([33]);
    const b = new env.Blob(['hi', bytes]);
    assert.equal(await b.text(), 'hi!');
  });
});

describe('File', () => {
  it('extends Blob with name and lastModified', () => {
    const f = new env.File(['payload'], 'note.txt', { type: 'text/plain', lastModified: 12345 });
    assert.equal(f.name, 'note.txt');
    assert.equal(f.type, 'text/plain');
    assert.equal(f.lastModified, 12345);
    assert.ok(f instanceof env.Blob);
  });

  it('lastModified defaults to a timestamp when omitted', () => {
    const f = new env.File(['p'], 'n.txt');
    assert.equal(typeof f.lastModified, 'number');
  });
});

describe('FormData', () => {
  it('append/get round-trips a string value', () => {
    const fd = new env.FormData();
    fd.append('a', '1');
    assert.equal(fd.get('a'), '1');
  });

  it('append/getAll preserves insertion order', () => {
    const fd = new env.FormData();
    fd.append('a', '1');
    fd.append('a', '2');
    assert.equal(JSON.stringify(fd.getAll('a')), '["1","2"]');
  });

  it('set replaces all entries for a name', () => {
    const fd = new env.FormData();
    fd.append('a', '1');
    fd.append('a', '2');
    fd.set('a', '3');
    assert.equal(JSON.stringify(fd.getAll('a')), '["3"]');
  });

  it('has / delete behave per spec', () => {
    const fd = new env.FormData();
    fd.append('a', '1');
    assert.equal(fd.has('a'), true);
    fd.delete('a');
    assert.equal(fd.has('a'), false);
  });

  it('appending a Blob attaches a default filename of "blob"', () => {
    const fd = new env.FormData();
    const b = new env.Blob(['x']);
    fd.append('file', b);
    const f = fd.get('file');
    assert.ok(f instanceof env.File);
    assert.equal(f.name, 'blob');
  });

  it('constructor with an htmlForm argument throws the stub message', () => {
    assert.throws(
      () => new env.FormData(/** @type {any} */ ({ nodeType: 1 })),
      /zero test: FormData constructor with form element is not supported/,
    );
  });

  it('iteration yields entries in insertion order', () => {
    const fd = new env.FormData();
    fd.append('a', '1');
    fd.append('b', '2');
    const seen = [];
    for (const [k, v] of fd) seen.push([k, v]);
    assert.equal(JSON.stringify(seen), '[["a","1"],["b","2"]]');
  });
});
