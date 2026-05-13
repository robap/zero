import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { document, window } from './dom-shim.js';

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

  it('querySelector("#x") finds nested descendant by id; null when absent', () => {
    const div = document.createElement('div');
    const inner = document.createElement('span');
    inner.setAttribute('id', 'x');
    div.appendChild(inner);
    assert.equal(div.querySelector('#x'), inner);
    assert.equal(div.querySelector('#missing'), null);
  });

  it('querySelectorAll("a") returns all anchors in document order', () => {
    const div = document.createElement('div');
    const a1 = document.createElement('a');
    const p = document.createElement('p');
    const a2 = document.createElement('a');
    div.appendChild(a1);
    div.appendChild(p);
    p.appendChild(a2);
    const result = div.querySelectorAll('a');
    assert.deepEqual(result, [a1, a2]);
  });

  it('closest("a") matches element itself, then ancestors; null if no match', () => {
    const nav = document.createElement('nav');
    const a = document.createElement('a');
    const span = document.createElement('span');
    nav.appendChild(a);
    a.appendChild(span);
    assert.equal(span.closest('a'), a);
    assert.equal(a.closest('a'), a);
    assert.equal(span.closest('nav'), nav);
    assert.equal(span.closest('section'), null);
  });

  describe('window history', () => {
    beforeEach(() => {
      window.history._entries = [{ state: null, url: '/' }];
      window.history._index = 0;
      window.location._set('/');
    });

    it('pushState advances index, appends entry, updates location', () => {
      window.history.pushState(null, '', '/about?x=1');
      assert.equal(window.history._index, 1);
      assert.equal(window.history.length, 2);
      assert.equal(window.location.pathname, '/about');
      assert.equal(window.location.search, '?x=1');
    });

    it('replaceState does not advance index and rewrites top entry', () => {
      window.history.pushState(null, '', '/page1');
      window.history.replaceState(null, '', '/page2');
      assert.equal(window.history._index, 1);
      assert.equal(window.history.length, 2);
      assert.equal(window.location.pathname, '/page2');
    });

    it('back() after two pushes dispatches popstate and rolls location back', () => {
      window.history.pushState(null, '', '/page1');
      window.history.pushState(null, '', '/page2');
      let popstateFired = false;
      window.addEventListener('popstate', e => { popstateFired = true; });
      window.history.back();
      assert.ok(popstateFired);
      assert.equal(window.location.pathname, '/page1');
    });

    it('pushState after back() truncates forward history', () => {
      window.history.pushState(null, '', '/page1');
      window.history.pushState(null, '', '/page2');
      assert.equal(window.history.length, 3);
      window.history.back();
      // back() does not remove forward entries; length is still 3
      assert.equal(window.history.length, 3);
      // pushState truncates /page2 and adds /page3
      window.history.pushState(null, '', '/page3');
      assert.equal(window.history.length, 3);
      assert.equal(window.history._entries[2].url, '/page3');
      assert.equal(window.location.pathname, '/page3');
    });
  });

  it('document.addEventListener/dispatchEvent/removeEventListener/once', () => {
    let count = 0;
    const fn = () => count++;
    document.addEventListener('click', fn);
    document.dispatchEvent({ type: 'click' });
    assert.equal(count, 1);
    document.removeEventListener('click', fn);
    document.dispatchEvent({ type: 'click' });
    assert.equal(count, 1);

    let onceCount = 0;
    document.addEventListener('click', () => onceCount++, { once: true });
    document.dispatchEvent({ type: 'click' });
    document.dispatchEvent({ type: 'click' });
    assert.equal(onceCount, 1);
  });
});
