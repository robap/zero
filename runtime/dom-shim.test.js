import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import {
  document,
  window,
  Event,
  CustomEvent,
  KeyboardEvent,
  MouseEvent,
  localStorage,
  sessionStorage,
} from './dom-shim.js';

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

  describe('event constructors', () => {
    it('new Event(type) sets .type', () => {
      const ev = new Event('foo');
      assert.equal(ev.type, 'foo');
    });

    it('Event bubbles defaults to false; init.bubbles=true sets it', () => {
      assert.equal(new Event('x').bubbles, false);
      assert.equal(new Event('x', { bubbles: true }).bubbles, true);
    });

    it('new CustomEvent has .detail from init', () => {
      assert.equal(new CustomEvent('y', { detail: 42 }).detail, 42);
    });

    it('new KeyboardEvent copies .key from init', () => {
      assert.equal(new KeyboardEvent('keydown', { key: 'Enter' }).key, 'Enter');
    });

    it('new MouseEvent copies .clientX from init', () => {
      assert.equal(new MouseEvent('click', { clientX: 10 }).clientX, 10);
    });

    it('event bubbles: parent listener fires when child dispatches bubbling event', () => {
      const parent = document.createElement('div');
      const child = document.createElement('span');
      parent.appendChild(child);
      let parentFired = false;
      parent.addEventListener('x', () => { parentFired = true; });
      child.dispatchEvent(new Event('x', { bubbles: true }));
      assert.ok(parentFired);
    });

    it('non-bubbling event does not reach parent', () => {
      const parent = document.createElement('div');
      const child = document.createElement('span');
      parent.appendChild(child);
      let parentFired = false;
      parent.addEventListener('x', () => { parentFired = true; });
      child.dispatchEvent(new Event('x', { bubbles: false }));
      assert.ok(!parentFired);
    });

    it('stopPropagation halts further nodes during bubble', () => {
      const grand = document.createElement('div');
      const parent = document.createElement('div');
      const child = document.createElement('span');
      grand.appendChild(parent);
      parent.appendChild(child);
      const log = [];
      parent.addEventListener('x', e => { log.push('parent'); e.stopPropagation(); });
      grand.addEventListener('x', () => { log.push('grand'); });
      child.dispatchEvent(new Event('x', { bubbles: true }));
      assert.deepEqual(log, ['parent']);
    });

    it('stopImmediatePropagation halts further listeners on the same node', () => {
      const el = document.createElement('div');
      const log = [];
      el.addEventListener('x', e => { log.push('a'); e.stopImmediatePropagation(); });
      el.addEventListener('x', () => { log.push('b'); });
      el.dispatchEvent(new Event('x'));
      assert.deepEqual(log, ['a']);
    });

    it('capture phase fires before target phase', () => {
      const parent = document.createElement('div');
      const child = document.createElement('span');
      parent.appendChild(child);
      const log = [];
      parent.addEventListener('x', () => { log.push('capture-parent'); }, { capture: true });
      child.addEventListener('x', () => { log.push('target-child'); });
      child.dispatchEvent(new Event('x', { bubbles: true }));
      assert.deepEqual(log, ['capture-parent', 'target-child']);
    });

    it('preventDefault on cancelable event makes dispatchEvent return false', () => {
      const el = document.createElement('div');
      el.addEventListener('x', e => e.preventDefault());
      const result = el.dispatchEvent(new Event('x', { cancelable: true }));
      assert.equal(result, false);
    });

    it('preventDefault is ignored when event is not cancelable; dispatchEvent returns true', () => {
      const el = document.createElement('div');
      el.addEventListener('x', e => e.preventDefault());
      const result = el.dispatchEvent(new Event('x'));
      assert.equal(result, true);
    });
  });

  describe('element property surface', () => {
    it('classList.add(...names) sets the class attribute', () => {
      const el = document.createElement('div');
      el.classList.add('a', 'b');
      assert.equal(el.getAttribute('class'), 'a b');
    });

    it('classList.remove / contains / replace work', () => {
      const el = document.createElement('div');
      el.classList.add('a', 'b', 'c');
      el.classList.remove('a');
      assert.equal(el.getAttribute('class'), 'b c');
      assert.ok(el.classList.contains('b'));
      assert.ok(!el.classList.contains('a'));
      assert.equal(el.classList.replace('b', 'x'), true);
      assert.equal(el.getAttribute('class'), 'x c');
      assert.equal(el.classList.length, 2);
    });

    it('classList.toggle returns the resulting state', () => {
      const el = document.createElement('div');
      assert.equal(el.classList.toggle('z'), true);
      assert.ok(el.classList.contains('z'));
      assert.equal(el.classList.toggle('z'), false);
      assert.ok(!el.classList.contains('z'));
    });

    it('classList.remove of the last class clears the attribute', () => {
      const el = document.createElement('div');
      el.classList.add('a');
      el.classList.remove('a');
      assert.equal(el.hasAttribute('class'), false);
    });

    it('dataset.fooBar = "5" writes data-foo-bar attribute', () => {
      const el = document.createElement('div');
      el.dataset.fooBar = '5';
      assert.equal(el.getAttribute('data-foo-bar'), '5');
      assert.equal(el.dataset.fooBar, '5');
    });

    it('delete el.dataset.x removes the attribute', () => {
      const el = document.createElement('div');
      el.setAttribute('data-x', 'y');
      assert.equal(el.dataset.x, 'y');
      delete el.dataset.x;
      assert.equal(el.hasAttribute('data-x'), false);
    });

    it('el.style.color = "red" serializes into the style attribute', () => {
      const el = document.createElement('div');
      el.style.color = 'red';
      assert.ok(el.getAttribute('style').includes('color: red'));
    });

    it('el.style.setProperty supports CSS custom properties', () => {
      const el = document.createElement('div');
      el.style.setProperty('--x', '1');
      assert.ok(el.getAttribute('style').includes('--x: 1'));
      assert.equal(el.style.getPropertyValue('--x'), '1');
    });

    it('setAttribute("style", ...) populates the style map', () => {
      const el = document.createElement('div');
      el.setAttribute('style', 'color: red; --x: 1');
      assert.equal(el.style.getPropertyValue('color'), 'red');
      assert.equal(el.style.getPropertyValue('--x'), '1');
    });

    it('textContent getter walks descendant text nodes', () => {
      const div = document.createElement('div');
      div.appendChild(document.createTextNode('hi '));
      const span = document.createElement('span');
      span.appendChild(document.createTextNode('there'));
      div.appendChild(span);
      assert.equal(div.textContent, 'hi there');
    });

    it('textContent setter replaces children with a single text node', () => {
      const el = document.createElement('div');
      el.appendChild(document.createElement('span'));
      el.appendChild(document.createElement('p'));
      el.textContent = 'replaced';
      assert.equal(el.childNodes.length, 1);
      assert.equal(el.childNodes[0].nodeType, 3);
      assert.equal(el.childNodes[0].nodeValue, 'replaced');
    });

    it('el.value round-trips with the value attribute', () => {
      const el = document.createElement('input');
      el.value = 'hi';
      assert.equal(el.getAttribute('value'), 'hi');
      assert.equal(el.value, 'hi');
    });

    it('el.checked toggles the checked attribute', () => {
      const el = document.createElement('input');
      el.checked = true;
      assert.equal(el.hasAttribute('checked'), true);
      el.checked = false;
      assert.equal(el.hasAttribute('checked'), false);
    });

    it('el.className mirrors the class attribute', () => {
      const el = document.createElement('div');
      el.className = 'a b';
      assert.equal(el.getAttribute('class'), 'a b');
      el.setAttribute('class', 'c');
      assert.equal(el.className, 'c');
    });

    it('el.htmlFor mirrors the for attribute', () => {
      const el = document.createElement('label');
      el.htmlFor = 'input1';
      assert.equal(el.getAttribute('for'), 'input1');
      assert.equal(el.htmlFor, 'input1');
    });
  });

  describe('auxiliary globals', () => {
    it('window.matchMedia returns object with .media and .matches=false', () => {
      const mql = window.matchMedia('(min-width: 800px)');
      assert.equal(mql.media, '(min-width: 800px)');
      assert.equal(mql.matches, false);
    });

    it('matchMedia listener fires when the MQL dispatches a change', () => {
      const mql = window.matchMedia('q');
      let hits = 0;
      mql.addEventListener('change', () => hits++);
      mql.dispatchEvent({ type: 'change' });
      assert.equal(hits, 1);
    });

    it('navigator.userAgent is the shim default and is overrideable', () => {
      assert.equal(window.navigator.userAgent, 'zero-test-shim/1.0');
      window.navigator.userAgent = 'custom';
      assert.equal(window.navigator.userAgent, 'custom');
      window.navigator.userAgent = 'zero-test-shim/1.0';
    });

    it('crypto.randomUUID returns an RFC4122 v4-shaped string', () => {
      const id = globalThis.crypto.randomUUID();
      assert.equal(id.length, 36);
      assert.equal(id[14], '4');
      assert.match(id, /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[0-9a-f]{4}-[0-9a-f]{12}$/);
    });

    it('crypto.getRandomValues returns the input typed array', () => {
      const arr = new Uint8Array(8);
      const out = globalThis.crypto.getRandomValues(arr);
      assert.equal(out, arr);
      assert.equal(out.length, 8);
      for (const v of out) {
        assert.ok(v >= 0 && v <= 255);
      }
    });

    it('IntersectionObserver records observations and disconnect clears', () => {
      const cb = () => {};
      const io = new globalThis.IntersectionObserver(cb);
      const el = document.createElement('div');
      io.observe(el);
      assert.equal(io.observations.length, 1);
      assert.equal(io.observations[0].target, el);
      io.disconnect();
      assert.equal(io.observations.length, 0);
    });

    it('MutationObserver has takeRecords returning []', () => {
      const mo = new globalThis.MutationObserver(() => {});
      assert.deepEqual(mo.takeRecords(), []);
    });

    it('getComputedStyle(el).getPropertyValue returns empty string', () => {
      const el = document.createElement('div');
      const cs = window.getComputedStyle(el);
      assert.equal(cs.getPropertyValue('color'), '');
      assert.equal(cs.length, 0);
    });
  });

  describe('web storage', () => {
    beforeEach(() => { localStorage.clear(); sessionStorage.clear(); });

    it('setItem/getItem round-trip with string coercion', () => {
      localStorage.setItem('a', 1);
      assert.equal(localStorage.getItem('a'), '1');
    });

    it('removeItem deletes the key', () => {
      localStorage.setItem('a', 'v');
      localStorage.removeItem('a');
      assert.equal(localStorage.getItem('a'), null);
    });

    it('clear empties storage; length reflects size; key(0) returns first key', () => {
      localStorage.setItem('a', '1');
      localStorage.setItem('b', '2');
      assert.equal(localStorage.length, 2);
      assert.equal(localStorage.key(0), 'a');
      localStorage.clear();
      assert.equal(localStorage.length, 0);
    });

    it('localStorage and sessionStorage do not share state', () => {
      localStorage.setItem('shared', 'L');
      sessionStorage.setItem('shared', 'S');
      assert.equal(localStorage.getItem('shared'), 'L');
      assert.equal(sessionStorage.getItem('shared'), 'S');
    });
  });

  describe('document additions', () => {
    it('document.documentElement is an HTML element', () => {
      assert.equal(document.documentElement.tagName, 'HTML');
    });

    it('document.head and document.body live under documentElement', () => {
      assert.equal(document.head.parentNode, document.documentElement);
      assert.equal(document.body.parentNode, document.documentElement);
    });

    it('document.getElementById finds element appended under body', () => {
      const span = document.createElement('span');
      span.setAttribute('id', 'gid1');
      document.body.appendChild(span);
      assert.equal(document.getElementById('gid1'), span);
      document.body.removeChild(span);
    });

    it('focus() sets activeElement and blur() clears it', () => {
      const el = document.createElement('input');
      el.focus();
      assert.equal(document.activeElement, el);
      el.blur();
      assert.equal(document.activeElement, null);
    });

    it('focusing a second element dispatches blur on the first', () => {
      const a = document.createElement('input');
      const b = document.createElement('input');
      let blurredA = false;
      a.addEventListener('blur', () => { blurredA = true; });
      a.focus();
      b.focus();
      assert.ok(blurredA);
      assert.equal(document.activeElement, b);
      b.blur();
    });

    it('document.title round-trip', () => {
      document.title = 'hi';
      assert.equal(document.title, 'hi');
      document.title = '';
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
