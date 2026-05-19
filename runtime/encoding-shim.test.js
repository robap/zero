/**
 * Node-side tests for the TextEncoder / TextDecoder shim. Evaluated in a
 * fresh `node:vm` sandbox to bypass Node's native classes.
 */

import { describe, it, before } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';

const shimSource = readFileSync(new URL('./encoding-shim.js', import.meta.url), 'utf8');

/** @type {any} */
let env;

before(() => {
  env = /** @type {any} */ ({});
  env.globalThis = env;
  vm.createContext(env);
  vm.runInContext(shimSource, env);
});

describe('TextEncoder', () => {
  it('encoding is "utf-8"', () => {
    assert.equal(new env.TextEncoder().encoding, 'utf-8');
  });

  it('encodes ASCII to a Uint8Array of the right bytes', () => {
    const bytes = new env.TextEncoder().encode('ab');
    assert.equal(bytes.length, 2);
    assert.equal(bytes[0], 0x61);
    assert.equal(bytes[1], 0x62);
  });

  it('encode/decode round-trips a 2-byte codepoint', () => {
    const bytes = new env.TextEncoder().encode('é');
    const back = new env.TextDecoder().decode(bytes);
    assert.equal(back, 'é');
  });

  it('encode/decode round-trips a 3-byte codepoint', () => {
    const bytes = new env.TextEncoder().encode('€');
    const back = new env.TextDecoder().decode(bytes);
    assert.equal(back, '€');
  });

  it('encode/decode round-trips a 4-byte codepoint', () => {
    const bytes = new env.TextEncoder().encode('😀');
    const back = new env.TextDecoder().decode(bytes);
    assert.equal(back, '😀');
  });
});

describe('TextDecoder', () => {
  it('decode(undefined) returns the empty string', () => {
    assert.equal(new env.TextDecoder().decode(), '');
  });

  it('decode accepts an ArrayBuffer input', () => {
    const enc = new env.TextEncoder();
    const bytes = enc.encode('hi');
    const buf = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
    assert.equal(new env.TextDecoder().decode(buf), 'hi');
  });

  it('constructor throws RangeError for non-utf8 encodings', () => {
    assert.throws(
      () => new env.TextDecoder('latin1'),
      /zero test: TextDecoder only supports utf-8/,
    );
  });
});
