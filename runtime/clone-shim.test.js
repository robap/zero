/**
 * Node-side tests for structuredClone + queueMicrotask. Evaluated in a
 * fresh `node:vm` sandbox so we exercise the shim rather than Node's
 * builtins.
 */

import { describe, it, before } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';

const cloneSource = readFileSync(new URL('./clone-shim.js', import.meta.url), 'utf8');

/** @type {any} */
let env;

before(() => {
  env = /** @type {any} */ ({});
  env.globalThis = env;
  vm.createContext(env);
  vm.runInContext(cloneSource, env);
  vm.runInContext('globalThis.Uint8Array = Uint8Array;', env);
});

describe('structuredClone', () => {
  it('returns primitives unchanged', () => {
    assert.equal(env.structuredClone(42), 42);
    assert.equal(env.structuredClone('hi'), 'hi');
    assert.equal(env.structuredClone(null), null);
  });

  it('deep-clones a nested object with no shared references', () => {
    const src = { a: { b: [1, 2] } };
    const copy = env.structuredClone(src);
    assert.equal(JSON.stringify(copy), JSON.stringify(src));
    assert.notEqual(copy, src);
    assert.notEqual(copy.a, src.a);
    assert.notEqual(copy.a.b, src.a.b);
  });

  it('preserves a self-referential cycle', () => {
    const o = /** @type {any} */ ({});
    o.self = o;
    const copy = env.structuredClone(o);
    assert.equal(copy.self, copy);
  });

  it('clones a Date by constructing a new Date with the same time', () => {
    const src = vm.runInContext('new Date(123)', env);
    const copy = env.structuredClone(src);
    assert.equal(copy.getTime(), 123);
    assert.notEqual(copy, src);
  });

  it('clones a RegExp', () => {
    const src = vm.runInContext('/ab/gi', env);
    const copy = env.structuredClone(src);
    assert.equal(copy.source, 'ab');
    assert.equal(copy.flags, 'gi');
  });

  it('clones a Map with its entries', () => {
    const src = vm.runInContext('new Map([["a", 1], ["b", 2]])', env);
    const copy = env.structuredClone(src);
    assert.equal(copy.get('a'), 1);
    assert.equal(copy.get('b'), 2);
    assert.notEqual(copy, src);
  });

  it('clones a Set with its members', () => {
    const src = vm.runInContext('new Set([1, 2, 3])', env);
    const copy = env.structuredClone(src);
    assert.equal(copy.has(1), true);
    assert.equal(copy.has(3), true);
    assert.notEqual(copy, src);
  });

  it('clones a typed array', () => {
    const src = vm.runInContext('new Uint8Array([1, 2, 3])', env);
    const copy = env.structuredClone(src);
    assert.equal(copy.length, 3);
    assert.equal(copy[2], 3);
    assert.notEqual(copy.buffer, src.buffer);
  });

  it('clones an Error preserving name and message', () => {
    const src = vm.runInContext('Object.assign(new TypeError("boom"), { name: "TypeError" })', env);
    const copy = env.structuredClone(src);
    assert.equal(copy.name, 'TypeError');
    assert.equal(copy.message, 'boom');
  });

  it('throws DataCloneError on a function', () => {
    assert.throws(
      () => env.structuredClone(() => 1),
      (err) => err.name === 'DataCloneError',
    );
  });

  it('throws DataCloneError on a DOM-shaped node', () => {
    assert.throws(
      () => env.structuredClone({ nodeType: 1, tagName: 'DIV' }),
      (err) => err.name === 'DataCloneError',
    );
  });

  it('rejects transfer option with a stub message', () => {
    assert.throws(
      () => env.structuredClone({}, { transfer: [/** @type {any} */ ({})] }),
      /zero test: structuredClone transfer/,
    );
  });
});

describe('queueMicrotask', () => {
  it('runs the callback after a microtask boundary', async () => {
    let ran = false;
    vm.runInContext('globalThis._mark = () => { ran = true; };', env);
    // Pass the host-side mutator via the sandbox.
    env._setRan = () => { ran = true; };
    env.queueMicrotask(() => { env._setRan(); });
    assert.equal(ran, false);
    await Promise.resolve();
    await Promise.resolve();
    assert.equal(ran, true);
  });

  it('throws TypeError when called without a function', () => {
    assert.throws(() => env.queueMicrotask(/** @type {any} */ (123)), /TypeError|callback/);
  });
});
