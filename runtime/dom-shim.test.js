import {
  describe,
  it,
  expect,
  beforeEach,
  afterEach,
  cleanup,
  render,
  find,
  findAll,
  text,
  fire,
  spy,
} from 'zero/test';
import { html } from 'zero';

describe('event bubbling and propagation', () => {
  afterEach(cleanup);

  it('parent listener fires when child dispatches bubbling event', () => {
    const handler = spy();
    const el = render(html`<div @x=${handler}><span></span></div>`);
    fire(find(el, 'span'), 'x');
    expect(handler.callCount).toBe(1);
  });

  it('stopPropagation halts the bubble before grandparent runs', () => {
    const grand = spy();
    const parent = (e) => e.stopPropagation();
    const el = render(
      html`<div @x=${grand}><div @x=${parent}><span></span></div></div>`,
    );
    fire(find(el, 'span'), 'x');
    expect(grand.callCount).toBe(0);
  });

  it('addEventListener once fires only on first dispatch', () => {
    const handler = spy();
    const el = render(html`<button></button>`);
    const btn = find(el, 'button');
    btn.addEventListener('click', handler, { once: true });
    fire(btn, 'click');
    fire(btn, 'click');
    expect(handler).toHaveBeenCalledTimes(1);
  });

  it('capture-phase listener fires before target-phase listener', () => {
    const log = [];
    const el = render(html`<div><span></span></div>`);
    const div = find(el, 'div');
    const span = find(el, 'span');
    div.addEventListener('x', () => log.push('capture-parent'), { capture: true });
    span.addEventListener('x', () => log.push('target-child'));
    fire(span, 'x');
    expect(log).toEqual(['capture-parent', 'target-child']);
  });
});

describe('window.history integration', () => {
  beforeEach(() => { window.history.pushState(null, '', '/'); });

  it('pushState advances length and updates location', () => {
    const lengthBefore = window.history.length;
    window.history.pushState(null, '', '/about?x=1');
    expect(window.history.length).toBe(lengthBefore + 1);
    expect(window.location.pathname).toBe('/about');
    expect(window.location.search).toBe('?x=1');
  });

  it('replaceState does not advance length and rewrites top entry', () => {
    window.history.pushState(null, '', '/page1');
    const lengthBefore = window.history.length;
    window.history.replaceState(null, '', '/page2');
    expect(window.history.length).toBe(lengthBefore);
    expect(window.location.pathname).toBe('/page2');
  });

  it('back() after pushes dispatches popstate and rolls location back', () => {
    window.history.pushState(null, '', '/page1');
    window.history.pushState(null, '', '/page2');
    const popHandler = spy();
    window.addEventListener('popstate', popHandler);
    window.history.back();
    expect(popHandler).toHaveBeenCalled();
    expect(window.location.pathname).toBe('/page1');
    window.removeEventListener('popstate', popHandler);
  });

  it('pushState after back() truncates forward history', () => {
    window.history.pushState(null, '', '/page1');
    window.history.pushState(null, '', '/page2');
    const lengthBeforeBack = window.history.length;
    window.history.back();
    expect(window.history.length).toBe(lengthBeforeBack);
    window.history.pushState(null, '', '/page3');
    expect(window.history.length).toBe(lengthBeforeBack);
    expect(window.location.pathname).toBe('/page3');
  });
});

describe('web storage', () => {
  beforeEach(() => { localStorage.clear(); sessionStorage.clear(); });

  it('setItem/getItem round-trip with string coercion', () => {
    localStorage.setItem('a', 1);
    expect(localStorage.getItem('a')).toBe('1');
  });

  it('removeItem deletes the key', () => {
    localStorage.setItem('a', 'v');
    localStorage.removeItem('a');
    expect(localStorage.getItem('a')).toBeNull();
  });

  it('clear empties storage; length reflects size; key(0) returns first key', () => {
    localStorage.setItem('a', '1');
    localStorage.setItem('b', '2');
    expect(localStorage.length).toBe(2);
    expect(localStorage.key(0)).toBe('a');
    localStorage.clear();
    expect(localStorage.length).toBe(0);
  });

  it('localStorage and sessionStorage do not share state', () => {
    localStorage.setItem('shared', 'L');
    sessionStorage.setItem('shared', 'S');
    expect(localStorage.getItem('shared')).toBe('L');
    expect(sessionStorage.getItem('shared')).toBe('S');
  });

  it('cleanup() clears both storages', () => {
    localStorage.setItem('a', '1');
    sessionStorage.setItem('a', '1');
    cleanup();
    expect(localStorage.length).toBe(0);
    expect(sessionStorage.length).toBe(0);
  });
});

describe('document additions', () => {
  afterEach(cleanup);

  it('document.getElementById finds element appended under body', () => {
    const span = document.createElement('span');
    span.setAttribute('id', 'gid1');
    document.body.appendChild(span);
    expect(document.getElementById('gid1')).toBe(span);
  });

  it('focus() sets activeElement and blur() clears it', () => {
    const el = document.createElement('input');
    el.focus();
    expect(document.activeElement).toBe(el);
    el.blur();
    expect(document.activeElement).toBeNull();
  });

  it('focusing a second element dispatches blur on the first', () => {
    const a = document.createElement('input');
    const b = document.createElement('input');
    const blurSpy = spy();
    a.addEventListener('blur', blurSpy);
    a.focus();
    b.focus();
    expect(blurSpy).toHaveBeenCalled();
    expect(document.activeElement).toBe(b);
    b.blur();
  });

  it('document.title round-trip', () => {
    document.title = 'hi';
    expect(document.title).toBe('hi');
  });

  it('cleanup() resets document.title', () => {
    document.title = 'leftover';
    cleanup();
    expect(document.title).toBe('');
  });
});

describe('document listeners', () => {
  afterEach(cleanup);

  it('addEventListener/dispatchEvent/removeEventListener', () => {
    const handler = spy();
    document.addEventListener('click', handler);
    fire(document, 'click');
    expect(handler).toHaveBeenCalledTimes(1);
    document.removeEventListener('click', handler);
    fire(document, 'click');
    expect(handler).toHaveBeenCalledTimes(1);
  });

  it('once option fires the listener only once', () => {
    const handler = spy();
    document.addEventListener('click', handler, { once: true });
    fire(document, 'click');
    fire(document, 'click');
    expect(handler).toHaveBeenCalledTimes(1);
  });
});

describe('input selection APIs', () => {
  afterEach(cleanup);

  it('setSelectionRange writes selectionStart/End', () => {
    const el = document.createElement('input');
    el.value = 'foobar';
    el.setSelectionRange(3, 6);
    expect(el.selectionStart).toBe(3);
    expect(el.selectionEnd).toBe(6);
  });

  it('defaults to 0/0', () => {
    const el = document.createElement('input');
    expect(el.selectionStart).toBe(0);
    expect(el.selectionEnd).toBe(0);
  });
});

describe('selector engine', () => {
  afterEach(cleanup);

  it('descendant matches nested elements at any depth', () => {
    const root = render(html`
      <div>
        <ul><li class="a">x</li><li class="b"><span><em>deep</em></span></li></ul>
        <p><em>not-in-ul</em></p>
      </div>
    `);
    const items = findAll(root, 'ul li');
    expect(items.length).toBe(2);
  });

  it('child matches only direct children, excluding nested grandchildren', () => {
    const root = render(html`
      <ul id="outer">
        <li class="direct">a</li>
        <li class="direct">b<ul><li class="nested">c</li></ul></li>
      </ul>
    `);
    const direct = findAll(root, 'ul#outer > li');
    expect(direct.length).toBe(2);
    expect(findAll(root, 'ul li').length).toBe(3);
  });

  it('selector list returns all branches in document order', () => {
    const root = render(html`
      <table><thead><tr><th>H</th></tr></thead>
      <tbody><tr><td>D1</td><td>D2</td></tr></tbody></table>
    `);
    const cells = findAll(root, 'th, td');
    expect(cells.length).toBe(3);
    expect(text(cells[0])).toBe('H');
    expect(text(cells[1])).toBe('D1');
    expect(text(cells[2])).toBe('D2');
  });

  it('selector list yields each node at most once', () => {
    const root = render(html`<div class="x" id="dup">hi</div>`);
    const matches = findAll(root, 'div, .x, #dup');
    expect(matches.length).toBe(1);
  });

  it('mixed descendant and child combinators in one branch', () => {
    const root = render(html`
      <table><tbody><tr><td><span class="hit">A</span></td></tr></tbody></table>
    `);
    const hits = findAll(root, 'table tbody > tr td span');
    expect(hits.length).toBe(1);
    expect(text(hits[0])).toBe('A');
  });

  it('left-hand compound outside the query root does not match', () => {
    const container = render(html`
      <div class="outer"><section><span>x</span></section></div>
    `);
    const section = find(container, 'section');
    expect(findAll(section, '.outer span').length).toBe(0);
    expect(findAll(section, 'span').length).toBe(1);
  });

  it('closest resolves self-or-ancestor, including list and combinator forms', () => {
    const container = render(html`
      <div class="card"><section><span class="leaf">x</span></section></div>
    `);
    const span = find(container, 'span');
    expect(span.closest('div')).toBe(find(container, 'div'));
    expect(span.closest('span')).toBe(span);
    expect(span.closest('article, .card')).toBe(find(container, 'div'));
    expect(span.closest('div span')).toBe(span);
    expect(span.closest('table td')).toBeNull();
  });

  it('tolerates surrounding and collapsed whitespace around combinators', () => {
    const root = render(html`
      <table><tbody><tr><td>c</td></tr></tbody></table>
      <ul><li>x</li></ul>
    `);
    expect(findAll(root, ' tbody tr ').length).toBe(1);
    expect(findAll(root, 'ul  >  li').length).toBe(1);
    expect(findAll(root, 'tr , li').length).toBe(2);
  });

  it('whitespace and combinator chars inside attribute values do not split', () => {
    const root = render(html`
      <div data-label="a b">one</div>
      <div data-expr="x>y">two</div>
    `);
    expect(findAll(root, '[data-label="a b"]').length).toBe(1);
    expect(findAll(root, '[data-expr="x>y"]').length).toBe(1);
  });

  it('preserves malformed-selector errors for bad combinators and lists', () => {
    const root = render(html`<div></div>`);
    for (const sel of ['a > > b', 'a >', '> a', 'a + b', 'a ~ b', 'a,', '']) {
      expect(() => findAll(root, sel)).toThrow('dom-shim:');
    }
  });

  it('reports malformed positions against the original full selector', () => {
    const root = render(html`<div></div>`);
    expect(() => findAll(root, '> a')).toThrow(
      'dom-shim: malformed selector "> a" at position 0 (expected selector before >)',
    );
    expect(() => findAll(root, 'a > > b')).toThrow(
      'dom-shim: malformed selector "a > > b" at position 4 (expected selector after >)',
    );
    expect(() => findAll(root, 'div, .#bad')).toThrow(
      'dom-shim: malformed selector "div, .#bad" at position 5 (expected class name after .)',
    );
    expect(() => findAll(root, 'a + b')).toThrow(
      "dom-shim: malformed selector \"a + b\" at position 2 (unexpected character '+')",
    );
  });

  it('single-compound selectors behave exactly as before', () => {
    const root = render(html`
      <section id="sec" class="box panel" data-role="main">
        <span class="box">s</span>
      </section>
    `);
    expect(findAll(root, 'section').length).toBe(1);
    expect(find(root, '#sec')).toBe(find(root, 'section'));
    expect(findAll(root, '.box').length).toBe(2);
    expect(findAll(root, '[data-role]').length).toBe(1);
    expect(findAll(root, '[data-role=main]').length).toBe(1);
    expect(findAll(root, 'span.box').length).toBe(1);
    expect(() => findAll(root, '.#bad')).toThrow('dom-shim: malformed selector');
  });
});
