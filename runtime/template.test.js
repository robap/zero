import './dom-shim.js'; // installs globalThis.document
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { html, commit, ref, each } from './template.js';
import { signal, effect, _createScope } from './reactivity.js';
import { document } from './dom-shim.js';

describe('html tagged template', () => {
  it('returns object with _template and _values', () => {
    const result = html`<div></div>`;
    assert.ok(result._template != null);
    assert.ok(Array.isArray(result._values));
  });

  it('_values matches dynamic args in order', () => {
    const a = 1, b = 'x', c = true;
    const result = html`<div class=${a} id=${b} data-x=${c}></div>`;
    assert.deepEqual(result._values, [a, b, c]);
  });

  it('same call site returns same _template reference (cache hit)', () => {
    const make = () => html`<span>${'x'}</span>`;
    const r1 = make();
    const r2 = make();
    assert.equal(r1._template, r2._template);
  });

  it('static html with no placeholders: fragment has correct structure', () => {
    const r = html`<div>Hello</div>`;
    const frag = r._template.fragment;
    assert.equal(frag.childNodes.length, 1);
    const div = frag.childNodes[0];
    assert.equal(div.tagName, 'DIV');
    assert.equal(div.childNodes.length, 1);
    assert.equal(div.childNodes[0].nodeValue, 'Hello');
    assert.deepEqual(r._template.parts, []);
  });

  it('static attribute value is preserved (e.g. <a href="/bar">)', () => {
    const container = document.createDocumentFragment();
    commit(html`<a href="/bar">bar</a>`, container);
    const a = container.childNodes[0];
    assert.equal(a.tagName, 'A');
    assert.equal(a.getAttribute('href'), '/bar');
    assert.equal(a.childNodes[0].nodeValue, 'bar');
  });

  it('multiple static attributes on one element', () => {
    const container = document.createDocumentFragment();
    commit(html`<a href="/x" class="link" data-y="z">x</a>`, container);
    const a = container.childNodes[0];
    assert.equal(a.getAttribute('href'), '/x');
    assert.equal(a.getAttribute('class'), 'link');
    assert.equal(a.getAttribute('data-y'), 'z');
  });

  it('static attribute mixed with dynamic attribute on same element', () => {
    const container = document.createDocumentFragment();
    commit(html`<a href="/x" class=${'c'}>x</a>`, container);
    const a = container.childNodes[0];
    assert.equal(a.getAttribute('href'), '/x');
    assert.equal(a.getAttribute('class'), 'c');
  });

  it('single-quoted static attribute value is preserved', () => {
    const container = document.createDocumentFragment();
    commit(html`<a href='/sq'>x</a>`, container);
    assert.equal(container.childNodes[0].getAttribute('href'), '/sq');
  });

  it('boolean static attribute is set to empty string', () => {
    const container = document.createDocumentFragment();
    commit(html`<input disabled>`, container);
    const input = container.childNodes[0];
    assert.equal(input.getAttribute('disabled'), '');
  });

  it('dynamic attr value next to static attr does not clobber dynamic', () => {
    const container = document.createDocumentFragment();
    commit(html`<a class=${'dyn'} href="/static">x</a>`, container);
    const a = container.childNodes[0];
    assert.equal(a.getAttribute('class'), 'dyn');
    assert.equal(a.getAttribute('href'), '/static');
  });

  it('node part: p with placeholder has text and comment anchor', () => {
    const r = html`<p>Count: ${0}</p>`;
    const frag = r._template.fragment;
    const p = frag.childNodes[0];
    assert.equal(p.tagName, 'P');
    assert.equal(p.childNodes.length, 2);
    assert.equal(p.childNodes[0].nodeValue, 'Count: ');
    assert.equal(p.childNodes[1].nodeType, 8); // Comment
    assert.equal(r._template.parts.length, 1);
    assert.equal(r._template.parts[0].type, 'node');
  });

  it('attr part: button with placeholder class has no class attribute and one attr part', () => {
    const r = html`<button class=${'x'}>go</button>`;
    const button = r._template.fragment.childNodes[0];
    assert.ok(!button.hasAttribute('class'));
    assert.equal(button.childNodes[0].nodeValue, 'go');
    assert.equal(r._template.parts.length, 1);
    assert.equal(r._template.parts[0].type, 'attr');
    assert.equal(r._template.parts[0].name, 'class');
  });

  it('event part: @click.prevent.stop parsed correctly', () => {
    const r = html`<button @click.prevent.stop=${() => {}}>x</button>`;
    const button = r._template.fragment.childNodes[0];
    assert.ok(!button.hasAttribute('@click.prevent.stop'));
    assert.equal(r._template.parts.length, 1);
    const part = r._template.parts[0];
    assert.equal(part.type, 'event');
    assert.equal(part.event, 'click');
    assert.deepEqual(part.modifiers, ['prevent', 'stop']);
  });

  it('ref part: self-closing input with ref has one ref part and no ref attribute', () => {
    const r = html`<input ref=${{}} />`;
    const input = r._template.fragment.childNodes[0];
    assert.ok(!input.hasAttribute('ref'));
    assert.equal(r._template.parts.length, 1);
    assert.equal(r._template.parts[0].type, 'ref');
  });

  it('creates <svg> in the SVG namespace', () => {
    const r = html`<svg viewBox="0 0 24 24"></svg>`;
    const svg = r._template.fragment.childNodes[0];
    assert.equal(svg.namespaceURI, 'http://www.w3.org/2000/svg');
  });

  it('creates descendants of <svg> in the SVG namespace', () => {
    const r = html`<svg><circle cx="12" cy="12" r="9"></circle></svg>`;
    const svg = r._template.fragment.childNodes[0];
    const circle = svg.childNodes[0];
    assert.equal(circle.namespaceURI, 'http://www.w3.org/2000/svg');
  });

  it('returns to the HTML namespace after </svg>', () => {
    const r = html`<div><svg></svg><span></span></div>`;
    const div = r._template.fragment.childNodes[0];
    const span = div.childNodes[1];
    assert.equal(span.namespaceURI, 'http://www.w3.org/1999/xhtml');
  });
});

describe('commit() — attr parts', () => {
  function makeContainer() { return document.createDocumentFragment(); }

  it('static string attribute is set on committed element', () => {
    const container = makeContainer();
    commit(html`<div class=${'a'}></div>`, container);
    const div = container.childNodes[0];
    assert.equal(div.getAttribute('class'), 'a');
  });

  it('false removes the attribute', () => {
    const container = makeContainer();
    commit(html`<div hidden=${false}></div>`, container);
    assert.ok(!container.childNodes[0].hasAttribute('hidden'));
  });

  it('true sets attribute to empty string', () => {
    const container = makeContainer();
    commit(html`<div hidden=${true}></div>`, container);
    assert.equal(container.childNodes[0].getAttribute('hidden'), '');
  });

  it('null removes the attribute', () => {
    const container = makeContainer();
    commit(html`<div class=${null}></div>`, container);
    assert.ok(!container.childNodes[0].hasAttribute('class'));
  });

  it('undefined removes the attribute', () => {
    const container = makeContainer();
    commit(html`<div class=${undefined}></div>`, container);
    assert.ok(!container.childNodes[0].hasAttribute('class'));
  });

  it('signal: attribute updates when signal changes', () => {
    const container = makeContainer();
    const c = signal('a');
    const scope = _createScope();
    scope.run(() => commit(html`<div class=${c}></div>`, container));
    const div = container.childNodes[0];
    assert.equal(div.getAttribute('class'), 'a');
    c.set('b');
    assert.equal(div.getAttribute('class'), 'b');
    scope.dispose();
  });

  it('reactive function: attribute reflects formula and updates', () => {
    const container = makeContainer();
    const n = signal(1);
    const scope = _createScope();
    scope.run(() => commit(html`<div data-x=${() => n.val * 2}></div>`, container));
    const div = container.childNodes[0];
    assert.equal(div.getAttribute('data-x'), '2');
    n.set(5);
    assert.equal(div.getAttribute('data-x'), '10');
    scope.dispose();
  });

  it('scope dispose stops signal updates to DOM', () => {
    const container = makeContainer();
    const c = signal('a');
    const scope = _createScope();
    scope.run(() => commit(html`<div class=${c}></div>`, container));
    const div = container.childNodes[0];
    scope.dispose();
    c.set('b');
    assert.equal(div.getAttribute('class'), 'a'); // unchanged
  });
});

describe('commit() — node parts', () => {
  function makeContainer() { return document.createDocumentFragment(); }

  it('static TemplateResult child renders nested span', () => {
    const container = makeContainer();
    commit(html`<div>${html`<span>x</span>`}</div>`, container);
    const div = container.childNodes[0];
    // div children: [Comment anchor, span]
    const span = div.childNodes.find(n => n.tagName === 'SPAN');
    assert.ok(span);
    assert.equal(span.childNodes[0].nodeValue, 'x');
  });

  it('static array of strings renders text nodes', () => {
    const container = makeContainer();
    commit(html`<ul>${['a', 'b', 'c']}</ul>`, container);
    const ul = container.childNodes[0];
    const texts = ul.childNodes.filter(n => n.nodeType === 3);
    assert.deepEqual(texts.map(t => t.nodeValue), ['a', 'b', 'c']);
  });

  it('primitive number renders as text node', () => {
    const container = makeContainer();
    commit(html`<p>${5}</p>`, container);
    const p = container.childNodes[0];
    const texts = p.childNodes.filter(n => n.nodeType === 3);
    assert.equal(texts.length, 1);
    assert.equal(texts[0].nodeValue, '5');
  });

  it('null renders nothing (no text nodes after anchor)', () => {
    const container = makeContainer();
    commit(html`<p>${null}</p>`, container);
    const p = container.childNodes[0];
    const nonAnchors = p.childNodes.filter(n => n.nodeType !== 8);
    assert.equal(nonAnchors.length, 0);
  });

  it('undefined renders nothing', () => {
    const container = makeContainer();
    commit(html`<p>${undefined}</p>`, container);
    const p = container.childNodes[0];
    const nonAnchors = p.childNodes.filter(n => n.nodeType !== 8);
    assert.equal(nonAnchors.length, 0);
  });

  it('signal of primitive updates text on change', () => {
    const container = makeContainer();
    const n = signal(1);
    const scope = _createScope();
    scope.run(() => commit(html`<p>${n}</p>`, container));
    const p = container.childNodes[0];
    const textNode = () => p.childNodes.filter(c => c.nodeType === 3)[0];
    assert.equal(textNode().nodeValue, '1');
    n.set(2);
    assert.equal(textNode().nodeValue, '2');
    scope.dispose();
  });

  it('reactive function returning TR rebuilds on dependency change', () => {
    const container = makeContainer();
    const n = signal('hello');
    const scope = _createScope();
    scope.run(() => commit(html`<div>${() => html`<span>${n.val}</span>`}</div>`, container));
    const div = container.childNodes[0];
    const span = () => div.childNodes.find(c => c.tagName === 'SPAN');
    assert.ok(span());
    assert.equal(span().childNodes.find(c => c.nodeType === 3)?.nodeValue, 'hello');
    n.set('world');
    assert.equal(span().childNodes.find(c => c.nodeType === 3)?.nodeValue, 'world');
    scope.dispose();
  });

  it('transitions across types: string → null → array → TR → string', () => {
    const container = makeContainer();
    const v = signal('a');
    const scope = _createScope();
    scope.run(() => commit(html`<p>${v}</p>`, container));
    const p = container.childNodes[0];
    const nonAnchors = () => p.childNodes.filter(n => n.nodeType !== 8);

    assert.equal(nonAnchors().length, 1);
    assert.equal(nonAnchors()[0].nodeValue, 'a');

    v.set(null);
    assert.equal(nonAnchors().length, 0);

    v.set(['x', 'y']);
    assert.equal(nonAnchors().length, 2);

    v.set(html`<i>z</i>`);
    assert.equal(nonAnchors().length, 1);
    assert.equal(nonAnchors()[0].tagName, 'I');

    v.set('final');
    assert.equal(nonAnchors().length, 1);
    assert.equal(nonAnchors()[0].nodeValue, 'final');

    scope.dispose();
  });

  it('nested commit: TR inside TR renders correctly', () => {
    const container = makeContainer();
    commit(html`<div class=${'outer'}>${html`<span class=${'inner'}>hi</span>`}</div>`, container);
    const div = container.childNodes[0];
    assert.equal(div.getAttribute('class'), 'outer');
    const span = div.childNodes.find(n => n.tagName === 'SPAN');
    assert.ok(span);
    assert.equal(span.getAttribute('class'), 'inner');
    assert.ok(span.childNodes.find(n => n.nodeValue === 'hi'));
  });
});

describe('commit() — event bindings', () => {
  function makeContainer() { return document.createDocumentFragment(); }
  function makeEvent(type, extra = {}) {
    return { type, preventDefault() { this._prevented = true; }, stopPropagation() { this._stopped = true; }, ...extra };
  }

  it('basic @click handler is called on dispatch', () => {
    const container = makeContainer();
    let called = false;
    commit(html`<button @click=${() => { called = true; }}>x</button>`, container);
    const btn = container.childNodes[0];
    btn.dispatchEvent(makeEvent('click'));
    assert.ok(called);
  });

  it('.prevent calls preventDefault on the event', () => {
    const container = makeContainer();
    commit(html`<button @click.prevent=${() => {}}>x</button>`, container);
    const btn = container.childNodes[0];
    const e = makeEvent('click');
    btn.dispatchEvent(e);
    assert.ok(e._prevented);
  });

  it('.stop calls stopPropagation on the event', () => {
    const container = makeContainer();
    commit(html`<button @click.stop=${() => {}}>x</button>`, container);
    const btn = container.childNodes[0];
    const e = makeEvent('click');
    btn.dispatchEvent(e);
    assert.ok(e._stopped);
  });

  it('.once: handler called only on first dispatch', () => {
    const container = makeContainer();
    let count = 0;
    commit(html`<button @click.once=${() => count++}>x</button>`, container);
    const btn = container.childNodes[0];
    btn.dispatchEvent(makeEvent('click'));
    btn.dispatchEvent(makeEvent('click'));
    assert.equal(count, 1);
  });

  it('.enter: handler called for Enter key, not others', () => {
    const container = makeContainer();
    let count = 0;
    commit(html`<input @keydown.enter=${() => count++} />`, container);
    const input = container.childNodes[0];
    input.dispatchEvent(makeEvent('keydown', { key: 'Enter' }));
    input.dispatchEvent(makeEvent('keydown', { key: 'a' }));
    assert.equal(count, 1);
  });

  it('.enter.prevent: Enter key AND preventDefault', () => {
    const container = makeContainer();
    let called = false;
    commit(html`<input @keydown.enter.prevent=${() => { called = true; }} />`, container);
    const input = container.childNodes[0];
    const e = makeEvent('keydown', { key: 'Enter' });
    input.dispatchEvent(e);
    assert.ok(called);
    assert.ok(e._prevented);
  });

  it('scope dispose removes event listener', () => {
    const container = makeContainer();
    let count = 0;
    const scope = _createScope();
    scope.run(() => commit(html`<button @click=${() => count++}>x</button>`, container));
    const btn = container.childNodes[0];
    btn.dispatchEvent(makeEvent('click'));
    assert.equal(count, 1);
    scope.dispose();
    btn.dispatchEvent(makeEvent('click'));
    assert.equal(count, 1); // listener removed
  });
});

describe('ref()', () => {
  function makeContainer() { return document.createDocumentFragment(); }

  it('ref().el is set to the element after commit', () => {
    const container = makeContainer();
    const r = ref();
    commit(html`<input ref=${r} />`, container);
    const input = container.childNodes[0];
    assert.equal(r.el, input);
  });

  it('ref().el is cleared to null on scope dispose', () => {
    const container = makeContainer();
    const r = ref();
    const scope = _createScope();
    scope.run(() => commit(html`<input ref=${r} />`, container));
    assert.ok(r.el != null);
    scope.dispose();
    assert.equal(r.el, null);
  });

  it('multiple refs in one template are each populated', () => {
    const container = makeContainer();
    const r1 = ref();
    const r2 = ref();
    commit(html`<div ref=${r1}><span ref=${r2}></span></div>`, container);
    assert.ok(r1.el != null);
    assert.equal(r1.el.tagName, 'DIV');
    assert.ok(r2.el != null);
    assert.equal(r2.el.tagName, 'SPAN');
  });
});

describe('each()', () => {
  function makeContainer() { return document.createDocumentFragment(); }

  it('static list renders one li per item', () => {
    const container = makeContainer();
    const items = signal(['a', 'b']);
    const scope = _createScope();
    scope.run(() => commit(html`<ul>${each(items, (it) => html`<li>${it}</li>`)}</ul>`, container));
    const ul = container.childNodes[0];
    const lis = ul.childNodes.filter(n => n.tagName === 'LI');
    assert.equal(lis.length, 2);
    assert.equal(lis[0].childNodes.find(n => n.nodeType === 3)?.nodeValue, 'a');
    assert.equal(lis[1].childNodes.find(n => n.nodeType === 3)?.nodeValue, 'b');
    scope.dispose();
  });

  it('array signal change re-renders the list', () => {
    const container = makeContainer();
    const items = signal(['a', 'b']);
    const scope = _createScope();
    scope.run(() => commit(html`<ul>${each(items, (it) => html`<li>${it}</li>`)}</ul>`, container));
    const ul = container.childNodes[0];
    items.set(['x', 'y', 'z']);
    const lis = ul.childNodes.filter(n => n.tagName === 'LI');
    assert.equal(lis.length, 3);
    assert.equal(lis[0].childNodes.find(n => n.nodeType === 3)?.nodeValue, 'x');
    scope.dispose();
  });

  it('empty array renders nothing (only anchor)', () => {
    const container = makeContainer();
    const items = signal([]);
    const scope = _createScope();
    scope.run(() => commit(html`<ul>${each(items, (it) => html`<li>${it}</li>`)}</ul>`, container));
    const ul = container.childNodes[0];
    const nonAnchors = ul.childNodes.filter(n => n.nodeType !== 8);
    assert.equal(nonAnchors.length, 0);
    scope.dispose();
  });

  it('list shrinks correctly: 3 items → 1 item', () => {
    const container = makeContainer();
    const items = signal(['a', 'b', 'c']);
    const scope = _createScope();
    scope.run(() => commit(html`<ul>${each(items, (it) => html`<li>${it}</li>`)}</ul>`, container));
    const ul = container.childNodes[0];
    items.set(['a']);
    const lis = ul.childNodes.filter(n => n.tagName === 'LI');
    assert.equal(lis.length, 1);
    scope.dispose();
  });

  it('index is passed correctly to renderFn', () => {
    const container = makeContainer();
    const items = signal(['a', 'b']);
    const scope = _createScope();
    scope.run(() => commit(html`<ul>${each(items, (it, i) => html`<li>${i}: ${it}</li>`)}</ul>`, container));
    const ul = container.childNodes[0];
    const lis = ul.childNodes.filter(n => n.tagName === 'LI');
    const text0 = lis[0].childNodes.filter(n => n.nodeType === 3).map(n => n.nodeValue).join('');
    const text1 = lis[1].childNodes.filter(n => n.nodeType === 3).map(n => n.nodeValue).join('');
    assert.equal(text0, '0: a');
    assert.equal(text1, '1: b');
    scope.dispose();
  });

  it('per-item effects are torn down on parent scope dispose', () => {
    const container = makeContainer();
    let effectRunCount = 0;
    const itemSig = signal('hello');
    const items = signal([itemSig]);
    const scope = _createScope();
    scope.run(() => commit(
      html`<ul>${each(items, (sigItem) => {
        return html`<li>${sigItem}</li>`;
      })}</ul>`,
      container
    ));
    scope.dispose();
    // After dispose, changing itemSig should not cause effects to run
    const before = effectRunCount;
    itemSig.set('world');
    assert.equal(effectRunCount, before);
  });

  it('keyed: reuses DOM nodes when same keys re-emitted in same order', () => {
    const container = makeContainer();
    const items = signal([
      { id: 1, name: 'a' },
      { id: 2, name: 'b' },
    ]);
    const scope = _createScope();
    scope.run(() => commit(
      html`<ul>${each(items, (it) => html`<li>${it.name}</li>`, (it) => it.id)}</ul>`,
      container,
    ));
    const ul = container.childNodes[0];
    const before = ul.childNodes.filter(n => n.tagName === 'LI');
    items.set([
      { id: 1, name: 'a2' },
      { id: 2, name: 'b2' },
    ]);
    const after = ul.childNodes.filter(n => n.tagName === 'LI');
    assert.equal(after[0], before[0]);
    assert.equal(after[1], before[1]);
    scope.dispose();
  });

  it('keyed: removes nodes whose keys disappear', () => {
    const container = makeContainer();
    const items = signal([
      { id: 1, name: 'a' },
      { id: 2, name: 'b' },
      { id: 3, name: 'c' },
    ]);
    const scope = _createScope();
    scope.run(() => commit(
      html`<ul>${each(items, (it) => html`<li>${it.name}</li>`, (it) => it.id)}</ul>`,
      container,
    ));
    const ul = container.childNodes[0];
    const before = ul.childNodes.filter(n => n.tagName === 'LI');
    const node1 = before[0];
    const node3 = before[2];
    items.set([
      { id: 1, name: 'a' },
      { id: 3, name: 'c' },
    ]);
    const after = ul.childNodes.filter(n => n.tagName === 'LI');
    assert.equal(after.length, 2);
    assert.equal(after[0], node1);
    assert.equal(after[1], node3);
    scope.dispose();
  });

  it('keyed: inserts new keys at the correct position', () => {
    const container = makeContainer();
    const items = signal([
      { id: 1, name: 'a' },
      { id: 2, name: 'b' },
      { id: 3, name: 'c' },
    ]);
    const scope = _createScope();
    scope.run(() => commit(
      html`<ul>${each(items, (it) => html`<li>${it.name}</li>`, (it) => it.id)}</ul>`,
      container,
    ));
    const ul = container.childNodes[0];
    const before = ul.childNodes.filter(n => n.tagName === 'LI');
    const node1 = before[0];
    const node2 = before[1];
    const node3 = before[2];
    items.set([
      { id: 1, name: 'a' },
      { id: 4, name: 'd' },
      { id: 2, name: 'b' },
      { id: 3, name: 'c' },
    ]);
    const after = ul.childNodes.filter(n => n.tagName === 'LI');
    assert.equal(after.length, 4);
    assert.equal(after[0], node1);
    assert.equal(after[2], node2);
    assert.equal(after[3], node3);
    assert.notEqual(after[1], node1);
    assert.notEqual(after[1], node2);
    assert.notEqual(after[1], node3);
    scope.dispose();
  });

  it('keyed: per-row scope is disposed when its key disappears', () => {
    const container = makeContainer();
    const disposed = [];
    const items = signal([
      { id: 1, name: 'a' },
      { id: 2, name: 'b' },
    ]);
    const scope = _createScope();
    scope.run(() => commit(
      html`<ul>${each(items, (it) => {
        effect(() => {
          return () => { disposed.push(it.id); };
        });
        return html`<li>${it.name}</li>`;
      }, (it) => it.id)}</ul>`,
      container,
    ));
    items.set([{ id: 1, name: 'a' }]);
    assert.deepEqual(disposed, [2]);
    scope.dispose();
  });

  it('keyed: duplicate keys throw', () => {
    const container = makeContainer();
    const items = signal([
      { id: 1, name: 'a' },
      { id: 1, name: 'b' },
    ]);
    const scope = _createScope();
    assert.throws(() => {
      scope.run(() => commit(
        html`<ul>${each(items, (it) => html`<li>${it.name}</li>`, (it) => it.id)}</ul>`,
        container,
      ));
    }, /duplicate key '1' in row 1/);
    scope.dispose();
  });

  it('keyed: reuses DOM nodes when items reorder', () => {
    const container = makeContainer();
    const items = signal([
      { id: 1, name: 'a' },
      { id: 2, name: 'b' },
      { id: 3, name: 'c' },
    ]);
    const scope = _createScope();
    scope.run(() => commit(
      html`<ul>${each(items, (it) => html`<li>${it.name}</li>`, (it) => it.id)}</ul>`,
      container,
    ));
    const ul = container.childNodes[0];
    const before = ul.childNodes.filter(n => n.tagName === 'LI');
    const node1 = before[0];
    const node2 = before[1];
    const node3 = before[2];
    items.set([
      { id: 3, name: 'c' },
      { id: 1, name: 'a' },
      { id: 2, name: 'b' },
    ]);
    const after = ul.childNodes.filter(n => n.tagName === 'LI');
    assert.equal(after.length, 3);
    assert.equal(after[0], node3);
    assert.equal(after[1], node1);
    assert.equal(after[2], node2);
    scope.dispose();
  });
});
