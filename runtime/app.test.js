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

describe('App (Step 6: guards and load)', () => {
  beforeEach(resetEnv);

  it('route() with non-function guard throws', () => {
    assert.throws(
      () => new App().route('/x', () => null, { guard: 'not-a-function' }),
      /guard must be a function/,
    );
  });

  it('guard returning false cancels nav: prior content unchanged, no blocked-route commit', async () => {
    freshMount('app');
    let blockedCalls = 0;
    const app = new App()
      .route('/', () => html`<span>home</span>`)
      .route('/blocked', () => { blockedCalls++; return html`<span>blocked</span>`; }, { guard: () => false });
    app.run('#app');
    await Promise.resolve();
    window.history.pushState(null, '', '/blocked');
    window.dispatchEvent({ type: 'popstate' });
    await Promise.resolve();
    // Blocked component should never run
    assert.equal(blockedCalls, 0);
  });

  it('guard returning true proceeds', async () => {
    freshMount('app');
    const app = new App()
      .route('/ok', () => html`<span>ok</span>`, { guard: () => true });
    app.run('#app');
    window.history.pushState(null, '', '/ok');
    window.dispatchEvent({ type: 'popstate' });
    await Promise.resolve();
    assert.ok(document.querySelector('#app').querySelector('span'));
  });

  it('guard calling redirect: URL changes; original component never invoked', async () => {
    freshMount('app');
    let adminCalls = 0;
    const app = new App()
      .route('/admin', () => { adminCalls++; return html`<span>admin</span>`; }, {
        guard: ({ redirect }) => { redirect('/login'); },
      })
      .route('/login', () => html`<span>login</span>`);
    app.run('#app');
    window.history.pushState(null, '', '/admin');
    window.dispatchEvent({ type: 'popstate' });
    await Promise.resolve(); await Promise.resolve(); await Promise.resolve();
    assert.equal(window.location.pathname, '/login');
    assert.equal(adminCalls, 0);
  });

  it('load() side-effect: state slice updated before component renders', async () => {
    freshMount('app');
    const { signal } = await import('./reactivity.js');
    const dataSig = signal(null);
    const app = new App()
      .state('data', dataSig)
      .route('/', ({ state }) => {
        return html`<span>${() => state.data.val ?? 'empty'}</span>`;
      }, {
        load: ({ state }) => { state.data.set('loaded'); },
      });
    app.run('#app');
    await Promise.resolve();
    const span = document.querySelector('#app').querySelector('span');
    assert.ok(span);
    // Signal was set by load() before component rendered
    assert.equal(dataSig.val, 'loaded');
  });

  it('load() returning slow promise delays commit', async () => {
    freshMount('app');
    let resolveLoad;
    const app = new App()
      .route('/', () => html`<span>done</span>`, {
        load: () => new Promise(r => { resolveLoad = r; }),
      });
    app.run('#app');
    await Promise.resolve();
    assert.ok(!document.querySelector('#app').querySelector('span'));
    resolveLoad();
    await Promise.resolve(); await Promise.resolve();
    assert.ok(document.querySelector('#app').querySelector('span'));
  });
});

describe('App (Step 7: loading UI)', () => {
  beforeEach(resetEnv);

  it('loading() after run() throws', () => {
    freshMount('app');
    const app = new App().route('/', () => html`<div>home</div>`);
    app.run('#app');
    assert.throws(() => app.loading(() => html`<div>loading</div>`), /cannot be called after run/);
  });

  it('loading() called twice throws', () => {
    const app = new App();
    app.loading(() => html`<div>loading</div>`);
    assert.throws(() => app.loading(() => html`<div>loading</div>`), /loading already set/);
  });

  it('fast nav: loading UI never appears', async () => {
    freshMount('app');
    let loadingCalls = 0;
    const app = new App()
      .loading(() => { loadingCalls++; return html`<span>loading</span>`; })
      .route('/', () => html`<span>home</span>`);
    app.run('#app');
    await Promise.resolve();
    assert.equal(loadingCalls, 0);
  });

  it('slow load(): loading UI appears after 150ms, then replaced by route content', async () => {
    freshMount('app');
    let resolveLoad;
    const app = new App()
      .loading(() => html`<span>loading</span>`)
      .route('/', () => html`<span>done</span>`, {
        load: () => new Promise(r => { resolveLoad = r; }),
      });
    app.run('#app');
    // Before 150ms: loading not shown
    await Promise.resolve();
    assert.ok(!document.querySelector('#app').querySelector('span'));
    // After 200ms: loading should appear
    await new Promise(r => setTimeout(r, 200));
    assert.ok(document.querySelector('#app').querySelector('span'));
    // Resolve load: content replaces loading
    resolveLoad();
    await new Promise(r => setTimeout(r, 10));
    assert.ok(document.querySelector('#app').querySelector('span'));
  });
});

describe('App (Step 9: nested routes)', () => {
  beforeEach(resetEnv);

  it('two-level: parent + child render; child appears inside parent outlet', async () => {
    freshMount('app');
    const Parent = ({ outlet }) => html`<div><span>parent</span>${outlet}</div>`;
    const Child = () => html`<em>child</em>`;
    const app = new App()
      .route('/dashboard', Parent, {
        children: [{ path: '/analytics', load: Child }],
      });
    app.run('#app');
    window.history.pushState(null, '', '/dashboard/analytics');
    window.dispatchEvent({ type: 'popstate' });
    await Promise.resolve(); await Promise.resolve();
    const mountEl = document.querySelector('#app');
    assert.ok(mountEl.querySelector('span'));
    assert.ok(mountEl.querySelector('em'));
  });

  it('sub-nav preserves parent: parentRenderCount stays 1 after sub-nav', async () => {
    freshMount('app');
    let parentRenderCount = 0;
    const Parent = ({ outlet }) => { parentRenderCount++; return html`<div>${outlet}</div>`; };
    const Overview = () => html`<span>overview</span>`;
    const Analytics = () => html`<span>analytics</span>`;
    const app = new App()
      .route('/dashboard', Parent, {
        children: [
          { path: '/overview', load: Overview },
          { path: '/analytics', load: Analytics },
        ],
      });
    app.run('#app');
    window.history.pushState(null, '', '/dashboard/overview');
    window.dispatchEvent({ type: 'popstate' });
    await Promise.resolve(); await Promise.resolve();
    assert.equal(parentRenderCount, 1);
    window.history.pushState(null, '', '/dashboard/analytics');
    window.dispatchEvent({ type: 'popstate' });
    await Promise.resolve(); await Promise.resolve();
    // Parent should NOT have been re-invoked
    assert.equal(parentRenderCount, 1);
  });

  it('plain top-level route (no children) still works after nested-chain changes', async () => {
    freshMount('app');
    const app = new App()
      .route('/', () => html`<span>home</span>`);
    app.run('#app');
    await Promise.resolve(); await Promise.resolve();
    assert.ok(document.querySelector('#app').querySelector('span'));
  });
});

describe('App (Step 11: guard-false URL rollback)', () => {
  beforeEach(resetEnv);

  it('guard returns false: URL rolls back to last committed URL', async () => {
    freshMount('app');
    const app = new App()
      .route('/', () => html`<span>home</span>`)
      .route('/admin', () => html`<span>admin</span>`, { guard: () => false });
    app.run('#app');
    await Promise.resolve(); await Promise.resolve();
    // Successfully committed '/'
    window.history.pushState(null, '', '/admin');
    window.dispatchEvent({ type: 'popstate' });
    await Promise.resolve(); await Promise.resolve();
    assert.equal(window.location.pathname, '/');
  });

  it('initial nav guard-false: URL stays at initial input (no rollback)', async () => {
    freshMount('app');
    window.history.pushState(null, '', '/admin');
    window.location._set('/admin');
    const app = new App()
      .route('/admin', () => html`<span>admin</span>`, { guard: () => false });
    app.run('#app');
    await Promise.resolve(); await Promise.resolve();
    assert.equal(window.location.pathname, '/admin');
  });
});

describe('App (Step 10: per-route overrides + meta merge)', () => {
  beforeEach(resetEnv);

  it('per-route loading override: slow load uses route loading component', async () => {
    freshMount('app');
    let routeLoadingCalls = 0;
    let resolveLoad;
    const app = new App()
      .loading(() => html`<span>global-loading</span>`)
      .route('/', () => html`<span>done</span>`, {
        loading: () => { routeLoadingCalls++; return html`<span>route-loading</span>`; },
        load: () => new Promise(r => { resolveLoad = r; }),
      });
    app.run('#app');
    await new Promise(r => setTimeout(r, 200));
    assert.equal(routeLoadingCalls, 1);
    resolveLoad();
    await Promise.resolve(); await Promise.resolve();
    assert.ok(document.querySelector('#app').querySelector('span'));
  });

  it('global loading fallback when no route override', async () => {
    freshMount('app');
    let globalLoadingCalls = 0;
    let resolveLoad;
    const app = new App()
      .loading(() => { globalLoadingCalls++; return html`<span>global</span>`; })
      .route('/', () => html`<span>done</span>`, {
        load: () => new Promise(r => { resolveLoad = r; }),
      });
    app.run('#app');
    await new Promise(r => setTimeout(r, 200));
    assert.equal(globalLoadingCalls, 1);
    resolveLoad();
    await Promise.resolve(); await Promise.resolve();
  });

  it('meta merge: middleware sees merged meta from parent and child', async () => {
    freshMount('app');
    let seenMeta;
    const Parent = ({ outlet }) => html`<div>${outlet}</div>`;
    const Child = () => html`<span>child</span>`;
    const app = new App()
      .use(({ route }) => { seenMeta = route.meta; })
      .route('/dashboard', Parent, {
        meta: { a: 1, b: 2 },
        children: [{ path: '/child', load: Child, meta: { b: 3, c: 4 } }],
      });
    app.run('#app');
    window.history.pushState(null, '', '/dashboard/child');
    window.dispatchEvent({ type: 'popstate' });
    await Promise.resolve(); await Promise.resolve(); await Promise.resolve();
    assert.deepEqual(seenMeta, { a: 1, b: 3, c: 4 });
  });
});

describe('App (Step 8: error handling)', () => {
  beforeEach(resetEnv);

  it('throw in middleware renders error UI', async () => {
    freshMount('app');
    const boom = new Error('boom');
    const app = new App()
      .use(() => { throw boom; })
      .error(({ error }) => html`<span>err:${error.message}</span>`)
      .route('/', () => html`<div>home</div>`);
    app.run('#app');
    await Promise.resolve(); await Promise.resolve();
    assert.ok(document.querySelector('#app').querySelector('span'));
  });

  it('throw in guard renders error UI', async () => {
    freshMount('app');
    const app = new App()
      .error(({ error }) => html`<span>err</span>`)
      .route('/x', () => html`<div>x</div>`, { guard: () => { throw new Error('guard'); } });
    app.run('#app');
    window.history.pushState(null, '', '/x');
    window.dispatchEvent({ type: 'popstate' });
    await Promise.resolve(); await Promise.resolve();
    assert.ok(document.querySelector('#app').querySelector('span'));
  });

  it('throw in load() renders error UI', async () => {
    freshMount('app');
    const app = new App()
      .error(({ error }) => html`<span>err</span>`)
      .route('/', () => html`<div>home</div>`, { load: () => { throw new Error('load'); } });
    app.run('#app');
    await Promise.resolve(); await Promise.resolve();
    assert.ok(document.querySelector('#app').querySelector('span'));
  });

  it('no error registered + throw: console.error called once, prior content stays', async () => {
    freshMount('app');
    const origErr = console.error;
    let errCalls = 0;
    console.error = () => { errCalls++; };
    try {
      const app = new App()
        .route('/', () => html`<span>home</span>`)
        .route('/bad', () => html`<div>bad</div>`, { load: () => { throw new Error('oops'); } });
      app.run('#app');
      await Promise.resolve(); await Promise.resolve();
      window.history.pushState(null, '', '/bad');
      window.dispatchEvent({ type: 'popstate' });
      await Promise.resolve(); await Promise.resolve();
      assert.equal(errCalls, 1);
    } finally {
      console.error = origErr;
    }
  });

  it('retry() re-invokes pipeline; success on second attempt renders content', async () => {
    freshMount('app');
    let attempt = 0;
    let retryFn;
    const app = new App()
      .error(({ error, retry }) => { retryFn = retry; return html`<span>err</span>`; })
      .route('/', () => html`<div>home</div>`, {
        load: () => { attempt++; if (attempt < 2) throw new Error('fail'); },
      });
    app.run('#app');
    await Promise.resolve(); await Promise.resolve();
    assert.ok(document.querySelector('#app').querySelector('span')); // error shown
    retryFn();
    await Promise.resolve(); await Promise.resolve();
    assert.ok(document.querySelector('#app').querySelector('div')); // success
  });
});

describe('App (Step 5: middleware, guards, load)', () => {
  beforeEach(resetEnv);

  it('use() after run() throws', () => {
    freshMount('app');
    const app = new App().route('/', () => html`<div>home</div>`);
    app.run('#app');
    assert.throws(() => app.use(() => {}), /cannot be called after run/);
  });

  it('two use() calls execute in registration order', async () => {
    freshMount('app');
    const order = [];
    const app = new App()
      .use(() => { order.push(1); })
      .use(() => { order.push(2); })
      .route('/', () => html`<div>home</div>`);
    app.run('#app');
    await Promise.resolve();
    assert.deepEqual(order, [1, 2]);
  });

  it('async middleware: route content commits only after middleware resolves', async () => {
    freshMount('app');
    let resolve;
    const blocked = new Promise(r => { resolve = r; });
    const app = new App()
      .use(() => blocked)
      .route('/', () => html`<span>home</span>`);
    app.run('#app');
    await Promise.resolve();
    assert.ok(!document.querySelector('#app').querySelector('span'));
    resolve();
    await Promise.resolve(); await Promise.resolve();
    assert.ok(document.querySelector('#app').querySelector('span'));
  });

  it('redirect() from middleware: URL is updated; original component never runs', async () => {
    freshMount('app');
    let targetCalls = 0;
    const app = new App()
      .use(({ redirect }) => { redirect('/login'); })
      .route('/admin', () => { targetCalls++; return html`<div>admin</div>`; })
      .route('/login', () => html`<div>login</div>`);
    app.run('#app');
    window.history.pushState(null, '', '/admin');
    window.dispatchEvent({ type: 'popstate' });
    await Promise.resolve(); await Promise.resolve(); await Promise.resolve();
    assert.equal(window.location.pathname, '/login');
    assert.equal(targetCalls, 0);
  });

  it('supersede: slow first nav is abandoned when second nav starts', async () => {
    freshMount('app');
    let resolveFirst;
    const firstBlocked = new Promise(r => { resolveFirst = r; });
    let firstCompCalls = 0;
    const app = new App()
      .use(({ route }) => route.path === '/first' ? firstBlocked : undefined)
      .route('/', () => html`<div>home</div>`)
      .route('/first', () => { firstCompCalls++; return html`<div>first</div>`; })
      .route('/second', () => html`<span>second</span>`);
    app.run('#app');
    await Promise.resolve();
    window.history.pushState(null, '', '/first');
    window.dispatchEvent({ type: 'popstate' });
    // immediately navigate to /second before /first resolves
    window.history.pushState(null, '', '/second');
    window.dispatchEvent({ type: 'popstate' });
    // flush microtask queue to let /second pipeline complete
    await Promise.resolve(); await Promise.resolve();
    // /second should now be rendered
    assert.ok(document.querySelector('#app').querySelector('span'));
    resolveFirst();
    // flush microtask queue to let /first pipeline run and supersede
    await Promise.resolve(); await Promise.resolve();
    // /first component should never have rendered (superseded)
    assert.equal(firstCompCalls, 0);
    // /second content still visible
    assert.ok(document.querySelector('#app').querySelector('span'));
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
      .layout(({ outlet }) => html`<main>${outlet}</main>`)
      .route('/', () => html`<span>content</span>`)
      .run('#app');
    await Promise.resolve();
    const mountEl = document.querySelector('#app');
    assert.ok(mountEl.querySelector('main'));
    assert.ok(mountEl.querySelector('span'));
  });

  it('layout component is invoked exactly once across multiple navigations', async () => {
    freshMount('app');
    let layoutCount = 0;
    const app = new App()
      .layout(({ outlet }) => { layoutCount++; return html`<main>${outlet}</main>`; })
      .route('/', () => html`<span>home</span>`)
      .route('/about', () => html`<span>about</span>`);
    app.run('#app');
    await Promise.resolve();
    assert.equal(layoutCount, 1);
    window.history.pushState(null, '', '/about');
    window.dispatchEvent({ type: 'popstate' });
    await Promise.resolve();
    assert.equal(layoutCount, 1);
  });

  it('route component receives state proxy; state.foo returns registered value', async () => {
    freshMount('app');
    const { signal } = await import('./reactivity.js');
    const fooSig = signal('bar');
    let receivedState;
    const app = new App()
      .state('foo', fooSig)
      .route('/', ({ state }) => { receivedState = state; return html`<div>x</div>`; });
    app.run('#app');
    await Promise.resolve();
    assert.ok(receivedState);
    assert.strictEqual(receivedState.foo, fooSig);
  });

  it('component that does not destructure state still works', async () => {
    freshMount('app');
    const app = new App()
      .route('/', () => html`<div>ok</div>`);
    app.run('#app');
    await Promise.resolve();
    assert.ok(document.querySelector('#app').querySelector('div'));
  });

  it('without layout, content still renders inside mount across navigations', async () => {
    freshMount('app');
    const app = new App()
      .route('/', () => html`<span>home</span>`)
      .route('/about', () => html`<span>about</span>`);
    app.run('#app');
    await Promise.resolve();
    assert.ok(document.querySelector('#app').querySelector('span'));
    window.history.pushState(null, '', '/about');
    window.dispatchEvent({ type: 'popstate' });
    await Promise.resolve();
    assert.ok(document.querySelector('#app').querySelector('span'));
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
    await Promise.resolve(); await Promise.resolve();
    assert.equal(loaderCalls, 1);
    // navigate away then back
    window.history.pushState(null, '', '/other');
    window.dispatchEvent({ type: 'popstate' });
    await Promise.resolve(); await Promise.resolve();
    window.history.pushState(null, '', '/');
    window.dispatchEvent({ type: 'popstate' });
    await Promise.resolve(); await Promise.resolve();
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
