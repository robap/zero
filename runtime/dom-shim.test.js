import {
  describe,
  it,
  expect,
  beforeEach,
  afterEach,
  cleanup,
  render,
  find,
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
