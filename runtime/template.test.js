import { describe, it, expect, render, find, findAll, text, fire, cleanup, afterEach, spy } from 'zero/test';
import { html, ref, each, signal, effect } from 'zero';

describe('html tagged template — static structure', () => {
  afterEach(cleanup);

  it('static html with no placeholders renders correctly', () => {
    const el = render(html`<div>Hello</div>`);
    const div = find(el, 'div');
    expect(div).toBeTruthy();
    expect(text(div)).toBe('Hello');
  });

  it('static attribute value is preserved (<a href="/bar">)', () => {
    const el = render(html`<a href="/bar">bar</a>`);
    const a = find(el, 'a');
    expect(a.getAttribute('href')).toBe('/bar');
    expect(text(a)).toBe('bar');
  });

  it('multiple static attributes on one element', () => {
    const el = render(html`<a href="/x" class="link" data-y="z">x</a>`);
    const a = find(el, 'a');
    expect(a.getAttribute('href')).toBe('/x');
    expect(a.getAttribute('class')).toBe('link');
    expect(a.getAttribute('data-y')).toBe('z');
  });

  it('static attribute mixed with dynamic attribute on same element', () => {
    const el = render(html`<a href="/x" class=${'c'}>x</a>`);
    const a = find(el, 'a');
    expect(a.getAttribute('href')).toBe('/x');
    expect(a.getAttribute('class')).toBe('c');
  });

  it('single-quoted static attribute value is preserved', () => {
    const el = render(html`<a href='/sq'>x</a>`);
    expect(find(el, 'a').getAttribute('href')).toBe('/sq');
  });

  it('boolean static attribute is set to empty string', () => {
    const el = render(html`<input disabled>`);
    expect(find(el, 'input').getAttribute('disabled')).toBe('');
  });

  it('dynamic attr value next to static attr does not clobber dynamic', () => {
    const el = render(html`<a class=${'dyn'} href="/static">x</a>`);
    const a = find(el, 'a');
    expect(a.getAttribute('class')).toBe('dyn');
    expect(a.getAttribute('href')).toBe('/static');
  });

  it('renders SVG element when nested inside HTML element', () => {
    const el = render(html`<div><svg></svg><span></span></div>`);
    expect(find(el, 'svg')).toBeTruthy();
    expect(find(el, 'span')).toBeTruthy();
  });
});

describe('commit() — attr parts', () => {
  afterEach(cleanup);

  it('static string attribute is set on committed element', () => {
    const el = render(html`<div class=${'a'}></div>`);
    expect(find(el, 'div').getAttribute('class')).toBe('a');
  });

  it('false removes the attribute', () => {
    const el = render(html`<div hidden=${false}></div>`);
    expect(find(el, 'div').hasAttribute('hidden')).toBeFalsy();
  });

  it('true sets attribute to empty string', () => {
    const el = render(html`<div hidden=${true}></div>`);
    expect(find(el, 'div').getAttribute('hidden')).toBe('');
  });

  it('null removes the attribute', () => {
    const el = render(html`<div class=${null}></div>`);
    expect(find(el, 'div').hasAttribute('class')).toBeFalsy();
  });

  it('undefined removes the attribute', () => {
    const el = render(html`<div class=${undefined}></div>`);
    expect(find(el, 'div').hasAttribute('class')).toBeFalsy();
  });

  it('signal: attribute updates when signal changes', () => {
    const c = signal('a');
    const el = render(html`<div class=${c}></div>`);
    const div = find(el, 'div');
    expect(div.getAttribute('class')).toBe('a');
    c.set('b');
    expect(div.getAttribute('class')).toBe('b');
  });

  it('reactive function: attribute reflects formula and updates', () => {
    const n = signal(1);
    const el = render(html`<div data-x=${() => n.val * 2}></div>`);
    const div = find(el, 'div');
    expect(div.getAttribute('data-x')).toBe('2');
    n.set(5);
    expect(div.getAttribute('data-x')).toBe('10');
  });

  it('cleanup stops signal updates to DOM', () => {
    const c = signal('a');
    const el = render(html`<div class=${c}></div>`);
    const div = find(el, 'div');
    cleanup();
    c.set('b');
    expect(div.getAttribute('class')).toBe('a');
  });
});

describe('attr parts — statics shape', () => {
  afterEach(cleanup);

  it('single placeholder: statics is ["", ""]', () => {
    const x = 'foo';
    const tr = html`<div class=${x}></div>`;
    const attrs = tr._template.parts.filter(p => p.type === 'attr');
    expect(attrs.length).toBe(1);
    expect(attrs[0].statics).toEqual(['', '']);
  });

  it('placeholder with prefix and suffix: statics captures both', () => {
    const x = 'mid';
    const tr = html`<div class="prefix ${x} suffix"></div>`;
    const attrs = tr._template.parts.filter(p => p.type === 'attr');
    expect(attrs.length).toBe(1);
    expect(attrs[0].statics).toEqual(['prefix ', ' suffix']);
  });

  it('commits prefix+placeholder+suffix as joined string', () => {
    const el = render(html`<span class="chip chip--${'active'} active"></span>`);
    const span = find(el, 'span');
    expect(span.getAttribute('class')).toBe('chip chip--active active');
  });

  it('two placeholders adjacent: statics is ["", " ", ""]', () => {
    const tr = html`<div class="${'a'} ${'b'}"></div>`;
    const attrs = tr._template.parts.filter(p => p.type === 'attr');
    expect(attrs.length).toBe(1);
    expect(attrs[0].statics).toEqual(['', ' ', '']);
  });

  it('two placeholders with no static between: statics is ["", "", ""]', () => {
    const tr = html`<div class="${'a'}${'b'}"></div>`;
    const attrs = tr._template.parts.filter(p => p.type === 'attr');
    expect(attrs.length).toBe(1);
    expect(attrs[0].statics).toEqual(['', '', '']);
  });

  it('commits two placeholders into joined attribute value', () => {
    const el = render(html`<div class="${'a'} ${'b'}"></div>`);
    expect(find(el, 'div').getAttribute('class')).toBe('a b');
  });

  it('signal in concat: setAttribute updates when signal changes', () => {
    const s = signal('y');
    const el = render(html`<div class="p ${s} s"></div>`);
    const div = find(el, 'div');
    expect(div.getAttribute('class')).toBe('p y s');
    s.set('z');
    expect(div.getAttribute('class')).toBe('p z s');
  });

  it('two signals in one attribute share a single effect', () => {
    const a = signal('A');
    const b = signal('B');
    const el = render(html`<div data-x="${a}-${b}"></div>`);
    const div = find(el, 'div');
    expect(div.getAttribute('data-x')).toBe('A-B');
    a.set('AA');
    expect(div.getAttribute('data-x')).toBe('AA-B');
    b.set('BB');
    expect(div.getAttribute('data-x')).toBe('AA-BB');
  });

  it('reactive function in concat updates on dependency change', () => {
    const mode = signal('dark');
    const el = render(html`<div class="theme-${() => mode.val}"></div>`);
    const div = find(el, 'div');
    expect(div.getAttribute('class')).toBe('theme-dark');
    mode.set('light');
    expect(div.getAttribute('class')).toBe('theme-light');
  });

  it('null in concat renders as empty string', () => {
    const el = render(html`<div class="a ${null} b"></div>`);
    expect(find(el, 'div').getAttribute('class')).toBe('a  b');
  });

  it('undefined in concat renders as empty string', () => {
    const el = render(html`<div class="a ${undefined} b"></div>`);
    expect(find(el, 'div').getAttribute('class')).toBe('a  b');
  });

  it('cleanup tears down concat effect', () => {
    const s = signal('y');
    const el = render(html`<div class="p ${s} s"></div>`);
    const div = find(el, 'div');
    cleanup();
    s.set('z');
    expect(div.getAttribute('class')).toBe('p y s');
  });

  it('style attribute with two placeholders interleaves correctly', () => {
    const c = signal('red');
    const p = signal(4);
    const el = render(html`<div style="color: ${c}; padding: ${p}px"></div>`);
    const div = find(el, 'div');
    expect(div.getAttribute('style')).toBe('color: red; padding: 4px');
    p.set(8);
    expect(div.getAttribute('style')).toBe('color: red; padding: 8px');
  });

  it('single-placeholder boolean semantics preserved: false removes', () => {
    const el = render(html`<input disabled=${false} />`);
    expect(find(el, 'input').hasAttribute('disabled')).toBeFalsy();
  });

  it('single-placeholder boolean semantics preserved: true sets ""', () => {
    const el = render(html`<input disabled=${true} />`);
    expect(find(el, 'input').getAttribute('disabled')).toBe('');
  });
});

describe('commit() — node parts', () => {
  afterEach(cleanup);

  it('static TemplateResult child renders nested span', () => {
    const el = render(html`<div>${html`<span>x</span>`}</div>`);
    const span = find(el, 'span');
    expect(span).toBeTruthy();
    expect(text(span)).toBe('x');
  });

  it('static array of strings renders text nodes', () => {
    const el = render(html`<ul>${['a', 'b', 'c']}</ul>`);
    const ul = find(el, 'ul');
    const texts = ul.childNodes.filter(n => n.nodeType === 3);
    expect(texts.map(t => t.nodeValue)).toEqual(['a', 'b', 'c']);
  });

  it('primitive number renders as text node', () => {
    const el = render(html`<p>${5}</p>`);
    const p = find(el, 'p');
    const texts = p.childNodes.filter(n => n.nodeType === 3);
    expect(texts.length).toBe(1);
    expect(texts[0].nodeValue).toBe('5');
  });

  it('null renders nothing (no text nodes after anchor)', () => {
    const el = render(html`<p>${null}</p>`);
    const p = find(el, 'p');
    const nonAnchors = p.childNodes.filter(n => n.nodeType !== 8);
    expect(nonAnchors.length).toBe(0);
  });

  it('undefined renders nothing', () => {
    const el = render(html`<p>${undefined}</p>`);
    const p = find(el, 'p');
    const nonAnchors = p.childNodes.filter(n => n.nodeType !== 8);
    expect(nonAnchors.length).toBe(0);
  });

  it('signal of primitive updates text on change', () => {
    const n = signal(1);
    const el = render(html`<p>${n}</p>`);
    const p = find(el, 'p');
    const textNode = () => p.childNodes.filter(c => c.nodeType === 3)[0];
    expect(textNode().nodeValue).toBe('1');
    n.set(2);
    expect(textNode().nodeValue).toBe('2');
  });

  it('reactive function returning TR rebuilds on dependency change', () => {
    const n = signal('hello');
    const el = render(html`<div>${() => html`<span>${n.val}</span>`}</div>`);
    const div = find(el, 'div');
    const spanText = () => {
      const span = div.childNodes.find(c => c.tagName === 'SPAN');
      return span.childNodes.find(c => c.nodeType === 3)?.nodeValue;
    };
    expect(spanText()).toBe('hello');
    n.set('world');
    expect(spanText()).toBe('world');
  });

  it('transitions across types: string → null → array → TR → string', () => {
    const v = signal('a');
    const el = render(html`<p>${v}</p>`);
    const p = find(el, 'p');
    const nonAnchors = () => p.childNodes.filter(n => n.nodeType !== 8);

    expect(nonAnchors().length).toBe(1);
    expect(nonAnchors()[0].nodeValue).toBe('a');

    v.set(null);
    expect(nonAnchors().length).toBe(0);

    v.set(['x', 'y']);
    expect(nonAnchors().length).toBe(2);

    v.set(html`<i>z</i>`);
    expect(nonAnchors().length).toBe(1);
    expect(nonAnchors()[0].tagName).toBe('I');

    v.set('final');
    expect(nonAnchors().length).toBe(1);
    expect(nonAnchors()[0].nodeValue).toBe('final');
  });

  it('nested commit: TR inside TR renders correctly', () => {
    const el = render(html`<div class=${'outer'}>${html`<span class=${'inner'}>hi</span>`}</div>`);
    const div = find(el, 'div');
    expect(div.getAttribute('class')).toBe('outer');
    const span = find(div, 'span');
    expect(span).toBeTruthy();
    expect(span.getAttribute('class')).toBe('inner');
    expect(text(span)).toBe('hi');
  });
});

describe('commit() — event bindings', () => {
  afterEach(cleanup);

  it('basic @click handler is called on dispatch', () => {
    const handler = spy();
    const el = render(html`<button @click=${handler}>x</button>`);
    fire(find(el, 'button'), 'click');
    expect(handler.callCount).toBe(1);
  });

  it('.once: handler called only on first dispatch', () => {
    const handler = spy();
    const el = render(html`<button @click.once=${handler}>x</button>`);
    const btn = find(el, 'button');
    fire(btn, 'click');
    fire(btn, 'click');
    expect(handler.callCount).toBe(1);
  });

  it('.stop: parent handler does not run when child click is .stop-modified', () => {
    const parent = spy();
    const child = spy();
    const el = render(html`<div @click=${parent}><button @click.stop=${child}>x</button></div>`);
    fire(find(el, 'button'), 'click');
    expect(child.callCount).toBe(1);
    expect(parent.callCount).toBe(0);
  });

  it('.enter: handler called for Enter key, not others', () => {
    const handler = spy();
    const el = render(html`<input @keydown.enter=${handler} />`);
    const input = find(el, 'input');
    fire(input, 'keydown', { key: 'Enter' });
    fire(input, 'keydown', { key: 'a' });
    expect(handler.callCount).toBe(1);
  });

  it('.enter.prevent: Enter key triggers handler', () => {
    const handler = spy();
    const el = render(html`<input @keydown.enter.prevent=${handler} />`);
    fire(find(el, 'input'), 'keydown', { key: 'Enter' });
    expect(handler.callCount).toBe(1);
  });

  it('cleanup removes event listener', () => {
    const handler = spy();
    const el = render(html`<button @click=${handler}>x</button>`);
    const btn = find(el, 'button');
    fire(btn, 'click');
    expect(handler.callCount).toBe(1);
    cleanup();
    fire(btn, 'click');
    expect(handler.callCount).toBe(1);
  });
});

describe('event timing modifiers — :NNN interval', () => {
  afterEach(cleanup);

  it('parses bare .debounce: modifiers is ["debounce"]', () => {
    const h = () => {};
    const tr = html`<input @input.debounce=${h} />`;
    const event = tr._template.parts.find(p => p.type === 'event');
    expect(event.modifiers).toEqual(['debounce']);
  });

  it('parses .debounce:250: modifiers is ["debounce:250"]', () => {
    const h = () => {};
    const tr = html`<input @input.debounce:250=${h} />`;
    const event = tr._template.parts.find(p => p.type === 'event');
    expect(event.modifiers).toEqual(['debounce:250']);
  });

  it('parses combined modifiers: ["prevent", "throttle:500"]', () => {
    const h = () => {};
    const tr = html`<a @click.prevent.throttle:500=${h}>x</a>`;
    const event = tr._template.parts.find(p => p.type === 'event');
    expect(event.modifiers).toEqual(['prevent', 'throttle:500']);
  });

  it('commit throws on .debounce:abc', () => {
    const h = () => {};
    expect(() => render(html`<input @input.debounce:abc=${h} />`))
      .toThrow('debounce:abc');
  });

  it('commit throws on .debounce: (empty suffix)', () => {
    const h = () => {};
    expect(() => render(html`<input @input.debounce:=${h} />`))
      .toThrow('debounce:');
  });

  it('commit throws on .debounce:0', () => {
    const h = () => {};
    expect(() => render(html`<input @input.debounce:0=${h} />`))
      .toThrow('> 0');
  });

  it('commit throws on .debounce:-5', () => {
    const h = () => {};
    expect(() => render(html`<input @input.debounce:-5=${h} />`))
      .toThrow('debounce:-5');
  });

  it('.debounce:250 passes 250 to setTimeout', () => {
    const calls = [];
    const origSetTimeout = globalThis.setTimeout;
    globalThis.setTimeout = (cb, ms) => {
      calls.push(ms);
      return origSetTimeout(cb, ms);
    };
    try {
      const handler = spy();
      const el = render(html`<input @input.debounce:250=${handler} />`);
      fire(find(el, 'input'), 'input');
      expect(calls.length).toBe(1);
      expect(calls[0]).toBe(250);
    } finally {
      globalThis.setTimeout = origSetTimeout;
    }
  });

  it('bare .debounce passes 100 to setTimeout', () => {
    const calls = [];
    const origSetTimeout = globalThis.setTimeout;
    globalThis.setTimeout = (cb, ms) => {
      calls.push(ms);
      return origSetTimeout(cb, ms);
    };
    try {
      const handler = spy();
      const el = render(html`<input @input.debounce=${handler} />`);
      fire(find(el, 'input'), 'input');
      expect(calls.length).toBe(1);
      expect(calls[0]).toBe(100);
    } finally {
      globalThis.setTimeout = origSetTimeout;
    }
  });

  it('.prevent combined with .debounce:300 commits without error', () => {
    const calls = [];
    const origSetTimeout = globalThis.setTimeout;
    globalThis.setTimeout = (cb, ms) => {
      calls.push(ms);
      return origSetTimeout(cb, ms);
    };
    try {
      const handler = spy();
      const el = render(html`<form @submit.prevent.debounce:300=${handler}></form>`);
      fire(find(el, 'form'), 'submit');
      expect(calls.length).toBe(1);
      expect(calls[0]).toBe(300);
    } finally {
      globalThis.setTimeout = origSetTimeout;
    }
  });

  it('.throttle:500 honors 500ms', () => {
    const calls = [];
    const origNow = Date.now;
    let nowVal = 1000;
    Date.now = () => nowVal;
    try {
      const handler = spy();
      const el = render(html`<div @scroll.throttle:500=${handler}></div>`);
      const div = find(el, 'div');
      fire(div, 'scroll');
      expect(handler.callCount).toBe(1);
      nowVal = 1100; // 100ms later — below 500 threshold
      fire(div, 'scroll');
      expect(handler.callCount).toBe(1);
      nowVal = 1600; // 600ms later — over 500 threshold
      fire(div, 'scroll');
      expect(handler.callCount).toBe(2);
    } finally {
      Date.now = origNow;
    }
  });
});

describe('ref()', () => {
  afterEach(cleanup);

  it('ref().el is set to the element after commit', () => {
    const r = ref();
    const el = render(html`<input ref=${r} />`);
    const input = find(el, 'input');
    expect(r.el).toBe(input);
  });

  it('ref().el is cleared to null on cleanup', () => {
    const r = ref();
    render(html`<input ref=${r} />`);
    expect(r.el != null).toBeTruthy();
    cleanup();
    expect(r.el).toBeNull();
  });

  it('multiple refs in one template are each populated', () => {
    const r1 = ref();
    const r2 = ref();
    render(html`<div ref=${r1}><span ref=${r2}></span></div>`);
    expect(r1.el != null).toBeTruthy();
    expect(r1.el.tagName).toBe('DIV');
    expect(r2.el != null).toBeTruthy();
    expect(r2.el.tagName).toBe('SPAN');
  });
});

describe('each()', () => {
  afterEach(cleanup);

  it('static list renders one li per item', () => {
    const items = signal(['a', 'b']);
    const el = render(html`<ul>${each(items, (it) => html`<li>${it}</li>`)}</ul>`);
    const lis = findAll(el, 'li');
    expect(lis.length).toBe(2);
    expect(text(lis[0])).toBe('a');
    expect(text(lis[1])).toBe('b');
  });

  it('array signal change re-renders the list', () => {
    const items = signal(['a', 'b']);
    const el = render(html`<ul>${each(items, (it) => html`<li>${it}</li>`)}</ul>`);
    items.set(['x', 'y', 'z']);
    const lis = findAll(el, 'li');
    expect(lis.length).toBe(3);
    expect(text(lis[0])).toBe('x');
  });

  it('empty array renders nothing (only anchor)', () => {
    const items = signal([]);
    const el = render(html`<ul>${each(items, (it) => html`<li>${it}</li>`)}</ul>`);
    const ul = find(el, 'ul');
    const nonAnchors = ul.childNodes.filter(n => n.nodeType !== 8);
    expect(nonAnchors.length).toBe(0);
  });

  it('list shrinks correctly: 3 items → 1 item', () => {
    const items = signal(['a', 'b', 'c']);
    const el = render(html`<ul>${each(items, (it) => html`<li>${it}</li>`)}</ul>`);
    items.set(['a']);
    expect(findAll(el, 'li').length).toBe(1);
  });

  it('index is passed correctly to renderFn', () => {
    const items = signal(['a', 'b']);
    const el = render(html`<ul>${each(items, (it, i) => html`<li>${i}: ${it}</li>`)}</ul>`);
    const lis = findAll(el, 'li');
    expect(text(lis[0])).toBe('0: a');
    expect(text(lis[1])).toBe('1: b');
  });

  it('per-item effects are torn down on cleanup', () => {
    let effectRunCount = 0;
    const itemSig = signal('hello');
    const items = signal([itemSig]);
    render(html`<ul>${each(items, (sigItem) => html`<li>${sigItem}</li>`)}</ul>`);
    cleanup();
    const before = effectRunCount;
    itemSig.set('world');
    expect(effectRunCount).toBe(before);
  });

  it('keyed: reuses DOM nodes when same keys re-emitted in same order', () => {
    const items = signal([
      { id: 1, name: 'a' },
      { id: 2, name: 'b' },
    ]);
    const el = render(
      html`<ul>${each(items, (it) => html`<li>${it.name}</li>`, (it) => it.id)}</ul>`,
    );
    const before = findAll(el, 'li');
    items.set([
      { id: 1, name: 'a2' },
      { id: 2, name: 'b2' },
    ]);
    const after = findAll(el, 'li');
    expect(after[0]).toBe(before[0]);
    expect(after[1]).toBe(before[1]);
  });

  it('keyed: removes nodes whose keys disappear', () => {
    const items = signal([
      { id: 1, name: 'a' },
      { id: 2, name: 'b' },
      { id: 3, name: 'c' },
    ]);
    const el = render(
      html`<ul>${each(items, (it) => html`<li>${it.name}</li>`, (it) => it.id)}</ul>`,
    );
    const before = findAll(el, 'li');
    const node1 = before[0];
    const node3 = before[2];
    items.set([
      { id: 1, name: 'a' },
      { id: 3, name: 'c' },
    ]);
    const after = findAll(el, 'li');
    expect(after.length).toBe(2);
    expect(after[0]).toBe(node1);
    expect(after[1]).toBe(node3);
  });

  it('keyed: inserts new keys at the correct position', () => {
    const items = signal([
      { id: 1, name: 'a' },
      { id: 2, name: 'b' },
      { id: 3, name: 'c' },
    ]);
    const el = render(
      html`<ul>${each(items, (it) => html`<li>${it.name}</li>`, (it) => it.id)}</ul>`,
    );
    const before = findAll(el, 'li');
    const node1 = before[0];
    const node2 = before[1];
    const node3 = before[2];
    items.set([
      { id: 1, name: 'a' },
      { id: 4, name: 'd' },
      { id: 2, name: 'b' },
      { id: 3, name: 'c' },
    ]);
    const after = findAll(el, 'li');
    expect(after.length).toBe(4);
    expect(after[0]).toBe(node1);
    expect(after[2]).toBe(node2);
    expect(after[3]).toBe(node3);
    expect(after[1] === node1).toBeFalsy();
    expect(after[1] === node2).toBeFalsy();
    expect(after[1] === node3).toBeFalsy();
  });

  it('keyed: per-row scope is disposed when its key disappears', () => {
    const disposed = [];
    const items = signal([
      { id: 1, name: 'a' },
      { id: 2, name: 'b' },
    ]);
    render(
      html`<ul>${each(items, (it) => {
        effect(() => {
          return () => { disposed.push(it.id); };
        });
        return html`<li>${it.name}</li>`;
      }, (it) => it.id)}</ul>`,
    );
    items.set([{ id: 1, name: 'a' }]);
    expect(disposed).toEqual([2]);
  });

  it('keyed: duplicate keys throw', () => {
    const items = signal([
      { id: 1, name: 'a' },
      { id: 1, name: 'b' },
    ]);
    expect(() => render(
      html`<ul>${each(items, (it) => html`<li>${it.name}</li>`, (it) => it.id)}</ul>`,
    )).toThrow("duplicate key '1' in row 1");
  });

  it('keyed: reuses DOM nodes when items reorder', () => {
    const items = signal([
      { id: 1, name: 'a' },
      { id: 2, name: 'b' },
      { id: 3, name: 'c' },
    ]);
    const el = render(
      html`<ul>${each(items, (it) => html`<li>${it.name}</li>`, (it) => it.id)}</ul>`,
    );
    const before = findAll(el, 'li');
    const node1 = before[0];
    const node2 = before[1];
    const node3 = before[2];
    items.set([
      { id: 3, name: 'c' },
      { id: 1, name: 'a' },
      { id: 2, name: 'b' },
    ]);
    const after = findAll(el, 'li');
    expect(after.length).toBe(3);
    expect(after[0]).toBe(node3);
    expect(after[1]).toBe(node1);
    expect(after[2]).toBe(node2);
  });
});

describe('select reactive selection', () => {
  afterEach(cleanup);

  /**
   * Render the Select-component shape: per-option reactive `selected`
   * bindings driven by a signal, inside a `<select @change=...>`.
   * @param {object} sig - value signal
   * @param {Function} [handler] - optional @change handler
   * @returns {object} the rendered root
   */
  function renderSelectShape(sig, handler = () => {}) {
    const options = [
      { value: 'a', label: 'A' },
      { value: 'b', label: 'B' },
      { value: 'c', label: 'C' },
    ].map(
      (o) =>
        html`<option value=${o.value} selected=${() => sig.val === o.value}>${o.label}</option>`,
    );
    return render(html`<select @change=${handler}>${options}</select>`);
  }

  it('signal-driven bindings produce observable selection', () => {
    const sig = signal('b');
    const el = renderSelectShape(sig);
    const sel = find(el, 'select');
    expect(sel.value).toBe('b');
    expect(sel.selectedOptions.length).toBe(1);
    sig.set('a');
    expect(sel.value).toBe('a');
    expect(findAll(el, 'option')[1].hasAttribute('selected')).toBe(false);
  });

  it('hand-written selection and signal re-assertion stay consistent', () => {
    const sig = signal('a');
    const el = renderSelectShape(sig);
    const sel = find(el, 'select');
    sel.value = 'b';
    const [a, b, c] = findAll(el, 'option');
    expect(sel.value).toBe('b');
    expect(a.hasAttribute('selected')).toBe(false);
    sig.set('c');
    expect(sel.value).toBe('c');
    expect(c.selected).toBe(true);
    expect(a.hasAttribute('selected')).toBe(false);
    expect(b.hasAttribute('selected')).toBe(false);
  });

  it('change handler reads the derived value from the real element', () => {
    const seen = [];
    const sig = signal('a');
    const el = renderSelectShape(sig, (e) => seen.push(e.target.value));
    const sel = find(el, 'select');
    sel.value = 'b';
    fire(sel, 'change');
    expect(seen).toEqual(['b']);
  });

  it('programmatic writes dispatch no change events', () => {
    const handler = spy();
    const sig = signal('a');
    const el = renderSelectShape(sig, handler);
    const sel = find(el, 'select');
    sel.value = 'b';
    sel.selectedIndex = 0;
    findAll(el, 'option')[1].selected = true;
    expect(handler.callCount).toBe(0);
  });
});

describe('live form property bindings', () => {
  afterEach(cleanup);

  it('select value=${sig} selects the matching option and updates on change', () => {
    const sig = signal('b');
    const el = render(html`
      <select value=${sig}>
        <option value="a">A</option>
        <option value="b">B</option>
        <option value="c">C</option>
      </select>
    `);
    const sel = find(el, 'select');
    expect(sel.value).toBe('b');
    sig.set('c');
    expect(sel.value).toBe('c');
  });

  it('input value=${sig} reflects the signal and updates programmatically', () => {
    const sig = signal('hello');
    const el = render(html`<input value=${sig} />`);
    const input = find(el, 'input');
    expect(input.value).toBe('hello');
    sig.set('world');
    expect(input.value).toBe('world');
  });

  it('input checked=${sig} reflects the boolean signal', () => {
    const sig = signal(true);
    const el = render(html`<input type="checkbox" checked=${sig} />`);
    const input = find(el, 'input');
    expect(input.checked).toBe(true);
    sig.set(false);
    expect(input.checked).toBe(false);
  });

  it('option selected=${sig} reflects the boolean signal', () => {
    // A `multiple` select lets an option be independently (de)selected — a
    // single select keeps its first option selected (the default-first rule).
    const sig = signal(true);
    const el = render(html`
      <select multiple>
        <option value="a" selected=${() => sig.val}>A</option>
        <option value="b">B</option>
      </select>
    `);
    const [a] = findAll(el, 'option');
    expect(a.selected).toBe(true);
    sig.set(false);
    expect(a.selected).toBe(false);
  });

  it('joined value="draft-${id}" sets the property and updates', () => {
    const id = signal(1);
    const el = render(html`<input value="draft-${id}" />`);
    const input = find(el, 'input');
    expect(input.value).toBe('draft-1');
    id.set(2);
    expect(input.value).toBe('draft-2');
  });

  it('does not reset the caret when the bound value equals the current value', () => {
    const sig = signal('');
    const el = render(html`<input value=${sig} />`);
    const input = find(el, 'input');
    // Simulate native typing: the DOM value is 'ab' with the caret mid-string.
    input.value = 'ab';
    input.setSelectionRange(1, 1);
    // The binding effect re-runs with a value equal to what is already shown.
    sig.set('ab');
    // Guard skipped the assignment, so the caret is untouched (would be 2
    // without the guard, since setting .value moves the caret to the end).
    expect(input.selectionStart).toBe(1);
  });
});
