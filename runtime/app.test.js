import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { App, inject, _setCurrentApp, _getCurrentApp } from './app.js';
import { window, document } from './dom-shim.js';
import { html } from './template.js';

function freshMount(id = 'app') {
  const el = document.createElement('div');
  el.setAttribute('id', id);
  document.childNodes.push(el);
  return el;
}

function resetEnv() {
  document.childNodes.length = 0;
  document._listeners.clear();
  window.history._entries = [{ state: null, url: '/' }];
  window.history._index = 0;
  window.location._set('/');
  window._listeners.clear();
  _setCurrentApp(null);
}

describe('App (Step 3: scaffolding)', () => {
  it('new App() does not throw', () => {
    assert.doesNotThrow(() => new App());
  });

  it('builder methods return the App instance (chainable)', () => {
    const app = new App();
    const result = app.state('a', 1).route('/', () => null);
    assert.ok(result instanceof App);
  });

  it('state() throws on duplicate key', () => {
    const app = new App();
    app.state('x', 1);
    assert.throws(() => app.state('x', 2), /already registered/);
  });

  it('layout() throws on second call', () => {
    const app = new App();
    app.layout(() => null);
    assert.throws(() => app.layout(() => null), /already set/);
  });

  it('layout() throws if component is not a function', () => {
    assert.throws(() => new App().layout('string'), /must be a function/);
  });

  it('route() throws if handler is not a function', () => {
    assert.throws(() => new App().route('/', 42), /handler must be a function/);
  });

  it('match() returns route entry with params on hit', () => {
    const app = new App().route('/users/:id', () => null);
    const m = app.match('/users/42');
    assert.ok(m);
    assert.deepEqual(m.params, { id: '42' });
    assert.deepEqual(m.query, {});
    assert.equal(m.pathname, '/users/42');
    assert.equal(m.search, '');
  });

  it('match() first-match wins; falls through to wildcard', () => {
    const specific = () => null;
    const wildcard = () => null;
    const app = new App().route('/about', specific).route('*', wildcard);
    assert.equal(app.match('/about').route.loader, specific);
    assert.equal(app.match('/other').route.loader, wildcard);
  });

  it('inject outside running app throws', () => {
    _setCurrentApp(null);
    assert.throws(() => inject('anything'), /no app is running/);
  });

  it('inject with _setCurrentApp resolves registered value', () => {
    const app = new App().state('color', 'blue');
    _setCurrentApp(app);
    assert.equal(inject('color'), 'blue');
    _setCurrentApp(null);
  });

  it('inject with unknown key throws', () => {
    const app = new App();
    _setCurrentApp(app);
    assert.throws(() => inject('nope'), /is not registered/);
    _setCurrentApp(null);
  });
});

describe('App (Step 4: run lifecycle)', () => {
  beforeEach(resetEnv);

  it('double run() throws', () => {
    freshMount('app');
    const app = new App().route('/', () => html`<div>home</div>`);
    app.run('#app');
    assert.throws(() => app.run('#app'), /already running/);
  });

  it('run() with missing selector throws', () => {
    assert.throws(() => new App().run('#nope'), /not found/);
  });

  it('after run(), calling state/layout/route throws', () => {
    freshMount('app');
    const app = new App().route('/', () => html`<div>home</div>`);
    app.run('#app');
    assert.throws(() => app.state('x', 1), /cannot be called after run/);
    assert.throws(() => app.layout(() => null), /cannot be called after run/);
    assert.throws(() => app.route('/x', () => null), /cannot be called after run/);
  });

  it('mount + initial render writes route content into mount element', async () => {
    freshMount('app');
    new App().route('/', () => html`<div>home</div>`).run('#app');
    await Promise.resolve(); // let async scope run
    const mountEl = document.querySelector('#app');
    assert.ok(mountEl.querySelector('div'));
  });

  it('layout wraps route content; without layout, route renders directly', async () => {
    freshMount('app');
    new App()
      .layout(({ children }) => html`<main>${children}</main>`)
      .route('/', () => html`<span>content</span>`)
      .run('#app');
    await Promise.resolve();
    const mountEl = document.querySelector('#app');
    assert.ok(mountEl.querySelector('main'));
    assert.ok(mountEl.querySelector('span'));
  });

  it('click interception: plain <a href="/about"> navigates', async () => {
    freshMount('app');
    new App()
      .route('/', () => html`<div>home</div>`)
      .route('/about', () => html`<div>about</div>`)
      .run('#app');
    await Promise.resolve();
    const anchor = document.createElement('a');
    anchor.setAttribute('href', '/about');
    const evt = { type: 'click', target: anchor, button: 0, defaultPrevented: false, preventDefault() { this.defaultPrevented = true; } };
    document.dispatchEvent(evt);
    await Promise.resolve();
    assert.equal(window.location.pathname, '/about');
  });

  it('click interception: span inside anchor navigates (ancestor walk)', async () => {
    freshMount('app');
    new App()
      .route('/', () => html`<div>home</div>`)
      .route('/about', () => html`<div>about</div>`)
      .run('#app');
    await Promise.resolve();
    const anchor = document.createElement('a');
    anchor.setAttribute('href', '/about');
    const span = document.createElement('span');
    anchor.appendChild(span);
    const evt = { type: 'click', target: span, button: 0, defaultPrevented: false, preventDefault() { this.defaultPrevented = true; } };
    document.dispatchEvent(evt);
    await Promise.resolve();
    assert.equal(window.location.pathname, '/about');
  });

  it('click interception: target="_blank" does not navigate', async () => {
    freshMount('app');
    new App().route('/', () => html`<div>home</div>`).run('#app');
    await Promise.resolve();
    const anchor = document.createElement('a');
    anchor.setAttribute('href', '/about');
    anchor.setAttribute('target', '_blank');
    let prevented = false;
    const evt = { type: 'click', target: anchor, button: 0, defaultPrevented: false, preventDefault() { prevented = true; } };
    document.dispatchEvent(evt);
    assert.ok(!prevented);
    assert.equal(window.location.pathname, '/');
  });

  it('click interception: download / data-external / external href do not navigate', async () => {
    freshMount('app');
    new App().route('/', () => html`<div>home</div>`).run('#app');
    await Promise.resolve();
    const cases = [
      () => { const a = document.createElement('a'); a.setAttribute('href', '/f'); a.setAttribute('download', ''); return a; },
      () => { const a = document.createElement('a'); a.setAttribute('href', '/f'); a.setAttribute('data-external', ''); return a; },
      () => { const a = document.createElement('a'); a.setAttribute('href', 'https://example.com/page'); return a; },
    ];
    for (const makeAnchor of cases) {
      const anchor = makeAnchor();
      let prevented = false;
      const evt = { type: 'click', target: anchor, button: 0, defaultPrevented: false, preventDefault() { prevented = true; } };
      document.dispatchEvent(evt);
      assert.ok(!prevented, `expected no preventDefault for href=${anchor.getAttribute('href')}`);
    }
  });

  it('click interception: metaKey/button:1 do not navigate', async () => {
    freshMount('app');
    new App().route('/', () => html`<div>home</div>`).run('#app');
    await Promise.resolve();
    const anchor = document.createElement('a');
    anchor.setAttribute('href', '/about');
    let prevented = false;
    const metaEvt = { type: 'click', target: anchor, button: 0, metaKey: true, defaultPrevented: false, preventDefault() { prevented = true; } };
    document.dispatchEvent(metaEvt);
    assert.ok(!prevented);
    const middleEvt = { type: 'click', target: anchor, button: 1, defaultPrevented: false, preventDefault() { prevented = true; } };
    document.dispatchEvent(middleEvt);
    assert.ok(!prevented);
  });

  it('popstate re-renders the matching route', async () => {
    freshMount('app');
    new App()
      .route('/', () => html`<div>home</div>`)
      .route('/about', () => html`<div>about</div>`)
      .run('#app');
    await Promise.resolve();
    window.history.pushState(null, '', '/about');
    window.dispatchEvent({ type: 'popstate' });
    await Promise.resolve();
    assert.equal(window.location.pathname, '/about');
  });

  it('eager component: first call reuses TR; second navigation calls component again', async () => {
    freshMount('app');
    let counter = 0;
    const app = new App()
      .route('/', () => { counter++; return html`<div>home</div>`; })
      .route('/other', () => html`<div>other</div>`);
    app.run('#app');
    await Promise.resolve();
    assert.equal(counter, 1);
    // navigate away then back
    window.history.pushState(null, '', '/other');
    window.dispatchEvent({ type: 'popstate' });
    await Promise.resolve();
    window.history.pushState(null, '', '/');
    window.dispatchEvent({ type: 'popstate' });
    await Promise.resolve();
    assert.equal(counter, 2);
  });

  it('lazy loader: called once; cached on second navigation', async () => {
    freshMount('app');
    let loaderCalls = 0;
    const Component = () => html`<div>lazy</div>`;
    const app = new App()
      .route('/', () => { loaderCalls++; return Promise.resolve({ default: Component }); })
      .route('/other', () => html`<div>other</div>`);
    app.run('#app');
    await new Promise(r => setTimeout(r, 0));
    assert.equal(loaderCalls, 1);
    // navigate away then back
    window.history.pushState(null, '', '/other');
    window.dispatchEvent({ type: 'popstate' });
    await new Promise(r => setTimeout(r, 0));
    window.history.pushState(null, '', '/');
    window.dispatchEvent({ type: 'popstate' });
    await new Promise(r => setTimeout(r, 0));
    assert.equal(loaderCalls, 1); // cache hit
  });

  it('data-active and data-active-exact on matching anchors', async () => {
    freshMount('app');
    // Template system only applies dynamic (${...}) attribute parts, not static ones.
    // Use dynamic hrefs so the anchors get real href attributes after commit.
    const NavLinks = () => html`<a href=${'/'}>home</a><a href=${'/about'}>about</a>`;
    const app = new App()
      .route('/', NavLinks)
      .route('/about', NavLinks);
    app.run('#app');
    await Promise.resolve();
    const mountEl = document.querySelector('#app');
    const anchors = mountEl.querySelectorAll('a');
    const homeAnchor = anchors[0];
    const aboutAnchor = anchors[1];
    // At '/', home has data-active-exact, about has neither
    assert.ok(homeAnchor.hasAttribute('data-active-exact'));
    assert.ok(homeAnchor.hasAttribute('data-active'));
    assert.ok(!aboutAnchor.hasAttribute('data-active-exact'));
    assert.ok(!aboutAnchor.hasAttribute('data-active'));
  });

  it('route-change disposal: abandoned scope effects do not fire', async () => {
    freshMount('app');
    const { signal } = await import('./reactivity.js');
    const sig = signal('initial');
    let updateCount = 0;
    const app = new App()
      .route('/', () => {
        // This effect is inside the route component's render
        return html`<div>${() => { updateCount++; return sig.val; }}</div>`;
      })
      .route('/other', () => html`<span>other</span>`);
    app.run('#app');
    await Promise.resolve();
    const initialCount = updateCount;
    // Navigate away — disposes the route scope
    window.history.pushState(null, '', '/other');
    window.dispatchEvent({ type: 'popstate' });
    await Promise.resolve();
    // Update the signal — disposed effects should not fire
    sig.set('changed');
    assert.equal(updateCount, initialCount);
  });
});
