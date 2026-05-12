import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { document } from './dom-shim.js';

describe('dom-shim', () => {
  it('createElement returns element with uppercase tagName', () => {
    const el = document.createElement('div');
    assert.equal(el.tagName, 'DIV');
    assert.equal(el.nodeName, 'DIV');
    assert.equal(el.nodeType, 1);
  });

  it('setAttribute / getAttribute / hasAttribute / removeAttribute', () => {
    const el = document.createElement('div');
    el.setAttribute('class', 'foo');
    assert.equal(el.getAttribute('class'), 'foo');
    assert.ok(el.hasAttribute('class'));
    el.removeAttribute('class');
    assert.ok(!el.hasAttribute('class'));
    assert.equal(el.getAttribute('class'), null);
  });

  it('createTextNode and createComment carry data', () => {
    const t = document.createTextNode('hello');
    assert.equal(t.nodeValue, 'hello');
    assert.equal(t.data, 'hello');
    assert.equal(t.nodeType, 3);
    const c = document.createComment('note');
    assert.equal(c.data, 'note');
    assert.equal(c.nodeValue, 'note');
    assert.equal(c.nodeType, 8);
  });

  it('appendChild wires parentNode', () => {
    const parent = document.createElement('div');
    const child = document.createElement('span');
    parent.appendChild(child);
    assert.equal(child.parentNode, parent);
    assert.equal(parent.childNodes[0], child);
    assert.equal(parent.childNodes.length, 1);
  });

  it('insertBefore places child at correct index', () => {
    const parent = document.createElement('div');
    const a = document.createElement('a');
    const b = document.createElement('b');
    const c = document.createElement('c');
    parent.appendChild(a);
    parent.appendChild(c);
    parent.insertBefore(b, c);
    assert.deepEqual(parent.childNodes, [a, b, c]);
    assert.equal(b.parentNode, parent);
  });

  it('cloneNode(deep) copies children but not listeners', () => {
    const el = document.createElement('div');
    el.setAttribute('id', '1');
    const child = document.createElement('span');
    el.appendChild(child);
    let fired = false;
    el.addEventListener('click', () => { fired = true; });
    const clone = el.cloneNode(true);
    assert.equal(clone.getAttribute('id'), '1');
    assert.equal(clone.childNodes.length, 1);
    assert.equal(clone.childNodes[0].tagName, 'SPAN');
    clone.dispatchEvent({ type: 'click', preventDefault() {}, stopPropagation() {} });
    assert.ok(!fired);
  });

  it('dispatchEvent fires listeners and respects once', () => {
    const el = document.createElement('button');
    let count = 0;
    el.addEventListener('click', () => count++, { once: true });
    el.dispatchEvent({ type: 'click' });
    el.dispatchEvent({ type: 'click' });
    assert.equal(count, 1);
  });
});
