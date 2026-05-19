import {
  describe,
  it,
  expect,
  beforeEach,
  afterEach,
  cleanup,
  fire,
  render,
  spy,
} from 'zero/test';
import { App, inject, signal, html } from 'zero';

function freshMount() {
  cleanup();
  window.history.pushState(null, '', '/');
  const mount = document.createElement('div');
  mount.setAttribute('id', 'app');
  document.body.appendChild(mount);
  return mount;
}

describe('App (Step 3: scaffolding)', () => {
  afterEach(cleanup);

  it('new App() does not throw', () => {
    new App();
  });

  it('builder methods return the App instance (chainable)', () => {
    const app = new App();
    const result = app.state('a', 1).route('/', () => null);
    expect(result instanceof App).toBeTruthy();
  });

  it('state() throws on duplicate key', () => {
    const app = new App();
    app.state('x', 1);
    expect(() => app.state('x', 2)).toThrow('already registered');
  });

  it('layout() throws on second call', () => {
    const app = new App();
    app.layout(() => null);
    expect(() => app.layout(() => null)).toThrow('already set');
  });

  it('layout() throws if component is not a function', () => {
    expect(() => new App().layout('string')).toThrow('must be a function');
  });

  it('route() throws if handler is not a function', () => {
    expect(() => new App().route('/', 42)).toThrow('handler must be a function');
  });

  it('match() returns route entry with params on hit', () => {
    const app = new App().route('/users/:id', () => null);
    const m = app.match('/users/42');
    expect(m).toBeTruthy();
    expect(m.params).toEqual({ id: '42' });
    expect(m.query).toEqual({});
    expect(m.pathname).toBe('/users/42');
    expect(m.search).toBe('');
  });

  it('match() first-match wins; falls through to wildcard', () => {
    const specific = () => null;
    const wildcard = () => null;
    const app = new App().route('/about', specific).route('*', wildcard);
    expect(app.match('/about').route.loader).toBe(specific);
    expect(app.match('/other').route.loader).toBe(wildcard);
  });

  it('inject outside running app throws', () => {
    cleanup();
    expect(() => inject('anything')).toThrow('no app is running');
  });

  it('inject inside a running route resolves registered value', async () => {
    freshMount();
    let observed;
    new App()
      .state('color', 'blue')
      .route('/', () => { observed = inject('color'); return html`<div></div>`; })
      .run('#app');
    await Promise.resolve();
    expect(observed).toBe('blue');
  });

  it('inject with unknown key throws', () => {
    expect(() => render(html`<span>${() => inject('nope')}</span>`)).toThrow('is not registered');
  });
});

describe('App (Step 6: guards and load)', () => {
  beforeEach(freshMount);
  afterEach(cleanup);

  it('route() with non-function guard throws', () => {
    expect(() => new App().route('/x', () => null, { guard: 'not-a-function' }))
      .toThrow('guard must be a function');
  });

  it('guard returning false cancels nav: blocked component never invoked', async () => {
    let blockedCalls = 0;
    const app = new App()
      .route('/', () => html`<span>home</span>`)
      .route('/blocked', () => { blockedCalls++; return html`<span>blocked</span>`; }, { guard: () => false });
    app.run('#app');
    await Promise.resolve();
    window.history.pushState(null, '', '/blocked');
    fire(window, 'popstate');
    await Promise.resolve();
    expect(blockedCalls).toBe(0);
  });

  it('guard returning true proceeds', async () => {
    const app = new App()
      .route('/ok', () => html`<span>ok</span>`, { guard: () => true });
    app.run('#app');
    window.history.pushState(null, '', '/ok');
    fire(window, 'popstate');
    await Promise.resolve();
    expect(document.querySelector('#app').querySelector('span')).toBeTruthy();
  });

  it('guard calling redirect: URL changes; original component never invoked', async () => {
    let adminCalls = 0;
    const app = new App()
      .route('/admin', () => { adminCalls++; return html`<span>admin</span>`; }, {
        guard: ({ redirect }) => { redirect('/login'); },
      })
      .route('/login', () => html`<span>login</span>`);
    app.run('#app');
    window.history.pushState(null, '', '/admin');
    fire(window, 'popstate');
    await Promise.resolve(); await Promise.resolve(); await Promise.resolve();
    expect(window.location.pathname).toBe('/login');
    expect(adminCalls).toBe(0);
  });

  it('load() side-effect: state slice updated before component renders', async () => {
    const dataSig = signal(null);
    const app = new App()
      .state('data', dataSig)
      .route('/', ({ state }) => html`<span>${() => state.data.val ?? 'empty'}</span>`, {
        load: ({ state }) => { state.data.set('loaded'); },
      });
    app.run('#app');
    await Promise.resolve();
    expect(document.querySelector('#app').querySelector('span')).toBeTruthy();
    expect(dataSig.val).toBe('loaded');
  });

  it('load() returning slow promise delays commit', async () => {
    let resolveLoad;
    const app = new App()
      .route('/', () => html`<span>done</span>`, {
        load: () => new Promise(r => { resolveLoad = r; }),
      });
    app.run('#app');
    await Promise.resolve();
    expect(document.querySelector('#app').querySelector('span')).toBeFalsy();
    resolveLoad();
    await Promise.resolve(); await Promise.resolve();
    expect(document.querySelector('#app').querySelector('span')).toBeTruthy();
  });
});

describe('App (Step 7: loading UI)', () => {
  beforeEach(freshMount);
  afterEach(cleanup);

  it('loading() after run() throws', () => {
    const app = new App().route('/', () => html`<div>home</div>`);
    app.run('#app');
    expect(() => app.loading(() => html`<div>loading</div>`)).toThrow('cannot be called after run');
  });

  it('loading() called twice throws', () => {
    const app = new App();
    app.loading(() => html`<div>loading</div>`);
    expect(() => app.loading(() => html`<div>loading</div>`)).toThrow('loading already set');
  });

  it('fast nav: loading UI never appears', async () => {
    let loadingCalls = 0;
    const app = new App()
      .loading(() => { loadingCalls++; return html`<span>loading</span>`; })
      .route('/', () => html`<span>home</span>`);
    app.run('#app');
    await Promise.resolve();
    expect(loadingCalls).toBe(0);
  });

  it('slow load(): loading UI appears, then replaced by route content', async () => {
    let resolveLoad;
    const app = new App()
      .loading(() => html`<span class="loading">loading</span>`)
      .route('/', () => html`<span class="done">done</span>`, {
        load: () => new Promise(r => { resolveLoad = r; }),
      });
    app.run('#app');
    await new Promise(r => setTimeout(r, 200));
    expect(document.querySelector('#app').querySelector('.loading')).toBeTruthy();
    resolveLoad();
    await new Promise(r => setTimeout(r, 10));
    expect(document.querySelector('#app').querySelector('.done')).toBeTruthy();
  });
});

describe('App (Step 9: nested routes)', () => {
  beforeEach(freshMount);
  afterEach(cleanup);

  it('two-level: parent + child render; child appears inside parent outlet', async () => {
    const Parent = ({ outlet }) => html`<div><span>parent</span>${outlet}</div>`;
    const Child = () => html`<em>child</em>`;
    const app = new App().route('/dashboard', Parent, {
      children: [{ path: '/analytics', load: Child }],
    });
    app.run('#app');
    window.history.pushState(null, '', '/dashboard/analytics');
    fire(window, 'popstate');
    await Promise.resolve(); await Promise.resolve();
    const mountEl = document.querySelector('#app');
    expect(mountEl.querySelector('span')).toBeTruthy();
    expect(mountEl.querySelector('em')).toBeTruthy();
  });

  it('sub-nav preserves parent: parentRenderCount stays 1 after sub-nav', async () => {
    let parentRenderCount = 0;
    const Parent = ({ outlet }) => { parentRenderCount++; return html`<div>${outlet}</div>`; };
    const Overview = () => html`<span>overview</span>`;
    const Analytics = () => html`<span>analytics</span>`;
    const app = new App().route('/dashboard', Parent, {
      children: [
        { path: '/overview', load: Overview },
        { path: '/analytics', load: Analytics },
      ],
    });
    app.run('#app');
    window.history.pushState(null, '', '/dashboard/overview');
    fire(window, 'popstate');
    await Promise.resolve(); await Promise.resolve();
    expect(parentRenderCount).toBe(1);
    window.history.pushState(null, '', '/dashboard/analytics');
    fire(window, 'popstate');
    await Promise.resolve(); await Promise.resolve();
    expect(parentRenderCount).toBe(1);
  });

  it('plain top-level route (no children) still works after nested-chain changes', async () => {
    const app = new App().route('/', () => html`<span>home</span>`);
    app.run('#app');
    await Promise.resolve(); await Promise.resolve();
    expect(document.querySelector('#app').querySelector('span')).toBeTruthy();
  });
});

describe('App (Step 11: guard-false URL rollback)', () => {
  beforeEach(freshMount);
  afterEach(cleanup);

  it('guard returns false: URL rolls back to last committed URL', async () => {
    const app = new App()
      .route('/', () => html`<span>home</span>`)
      .route('/admin', () => html`<span>admin</span>`, { guard: () => false });
    app.run('#app');
    await Promise.resolve(); await Promise.resolve();
    window.history.pushState(null, '', '/admin');
    fire(window, 'popstate');
    await Promise.resolve(); await Promise.resolve();
    expect(window.location.pathname).toBe('/');
  });

  it('initial nav guard-false: URL stays at initial input (no rollback)', async () => {
    window.history.pushState(null, '', '/admin');
    const app = new App()
      .route('/admin', () => html`<span>admin</span>`, { guard: () => false });
    app.run('#app');
    await Promise.resolve(); await Promise.resolve();
    expect(window.location.pathname).toBe('/admin');
  });
});

describe('App (Step 10: per-route overrides + meta merge)', () => {
  beforeEach(freshMount);
  afterEach(cleanup);

  it('per-route loading override: slow load uses route loading component', async () => {
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
    expect(routeLoadingCalls).toBe(1);
    resolveLoad();
    await Promise.resolve(); await Promise.resolve();
    expect(document.querySelector('#app').querySelector('span')).toBeTruthy();
  });

  it('global loading fallback when no route override', async () => {
    let globalLoadingCalls = 0;
    let resolveLoad;
    const app = new App()
      .loading(() => { globalLoadingCalls++; return html`<span>global</span>`; })
      .route('/', () => html`<span>done</span>`, {
        load: () => new Promise(r => { resolveLoad = r; }),
      });
    app.run('#app');
    await new Promise(r => setTimeout(r, 200));
    expect(globalLoadingCalls).toBe(1);
    resolveLoad();
    await Promise.resolve(); await Promise.resolve();
  });

  it('meta merge: middleware sees merged meta from parent and child', async () => {
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
    fire(window, 'popstate');
    await Promise.resolve(); await Promise.resolve(); await Promise.resolve();
    expect(seenMeta).toEqual({ a: 1, b: 3, c: 4 });
  });
});

describe('App (Step 8: error handling)', () => {
  beforeEach(freshMount);
  afterEach(cleanup);

  it('throw in middleware renders error UI', async () => {
    const boom = new Error('boom');
    const app = new App()
      .use(() => { throw boom; })
      .error(({ error }) => html`<span>err:${error.message}</span>`)
      .route('/', () => html`<div>home</div>`);
    app.run('#app');
    await Promise.resolve(); await Promise.resolve();
    expect(document.querySelector('#app').querySelector('span')).toBeTruthy();
  });

  it('throw in guard renders error UI', async () => {
    const app = new App()
      .error(({ error }) => html`<span>err</span>`)
      .route('/x', () => html`<div>x</div>`, { guard: () => { throw new Error('guard'); } });
    app.run('#app');
    window.history.pushState(null, '', '/x');
    fire(window, 'popstate');
    await Promise.resolve(); await Promise.resolve();
    expect(document.querySelector('#app').querySelector('span')).toBeTruthy();
  });

  it('throw in load() renders error UI', async () => {
    const app = new App()
      .error(({ error }) => html`<span>err</span>`)
      .route('/', () => html`<div>home</div>`, { load: () => { throw new Error('load'); } });
    app.run('#app');
    await Promise.resolve(); await Promise.resolve();
    expect(document.querySelector('#app').querySelector('span')).toBeTruthy();
  });

  it('no error registered + throw: console.error called once, prior content stays', async () => {
    const origErr = console.error;
    const errSpy = spy();
    console.error = errSpy;
    try {
      const app = new App()
        .route('/', () => html`<span>home</span>`)
        .route('/bad', () => html`<div>bad</div>`, { load: () => { throw new Error('oops'); } });
      app.run('#app');
      await Promise.resolve(); await Promise.resolve();
      window.history.pushState(null, '', '/bad');
      fire(window, 'popstate');
      await Promise.resolve(); await Promise.resolve();
      expect(errSpy).toHaveBeenCalledTimes(1);
    } finally {
      console.error = origErr;
    }
  });

  it('retry() re-invokes pipeline; success on second attempt renders content', async () => {
    let attempt = 0;
    let retryFn;
    const app = new App()
      .error(({ error, retry }) => { retryFn = retry; return html`<span>err</span>`; })
      .route('/', () => html`<div>home</div>`, {
        load: () => { attempt++; if (attempt < 2) throw new Error('fail'); },
      });
    app.run('#app');
    await Promise.resolve(); await Promise.resolve();
    expect(document.querySelector('#app').querySelector('span')).toBeTruthy();
    retryFn();
    await Promise.resolve(); await Promise.resolve();
    expect(document.querySelector('#app').querySelector('div')).toBeTruthy();
  });
});

describe('App (Step 5: middleware, guards, load)', () => {
  beforeEach(freshMount);
  afterEach(cleanup);

  it('use() after run() throws', () => {
    const app = new App().route('/', () => html`<div>home</div>`);
    app.run('#app');
    expect(() => app.use(() => {})).toThrow('cannot be called after run');
  });

  it('two use() calls execute in registration order', async () => {
    const order = [];
    const app = new App()
      .use(() => { order.push(1); })
      .use(() => { order.push(2); })
      .route('/', () => html`<div>home</div>`);
    app.run('#app');
    await Promise.resolve();
    expect(order).toEqual([1, 2]);
  });

  it('async middleware: route content commits only after middleware resolves', async () => {
    let resolve;
    const blocked = new Promise(r => { resolve = r; });
    const app = new App()
      .use(() => blocked)
      .route('/', () => html`<span>home</span>`);
    app.run('#app');
    await Promise.resolve();
    expect(document.querySelector('#app').querySelector('span')).toBeFalsy();
    resolve();
    await Promise.resolve(); await Promise.resolve();
    expect(document.querySelector('#app').querySelector('span')).toBeTruthy();
  });

  it('redirect() from middleware: URL is updated; original component never runs', async () => {
    let targetCalls = 0;
    const app = new App()
      .use(({ route, redirect }) => { if (route.path !== '/login') redirect('/login'); })
      .route('/admin', () => { targetCalls++; return html`<div>admin</div>`; })
      .route('/login', () => html`<div>login</div>`);
    app.run('#app');
    window.history.pushState(null, '', '/admin');
    fire(window, 'popstate');
    await Promise.resolve(); await Promise.resolve(); await Promise.resolve();
    expect(window.location.pathname).toBe('/login');
    expect(targetCalls).toBe(0);
  });

  it('supersede: slow first nav is abandoned when second nav starts', async () => {
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
    fire(window, 'popstate');
    window.history.pushState(null, '', '/second');
    fire(window, 'popstate');
    await Promise.resolve(); await Promise.resolve();
    expect(document.querySelector('#app').querySelector('span')).toBeTruthy();
    resolveFirst();
    await Promise.resolve(); await Promise.resolve();
    expect(firstCompCalls).toBe(0);
    expect(document.querySelector('#app').querySelector('span')).toBeTruthy();
  });
});

describe('App (Step 4: run lifecycle)', () => {
  beforeEach(freshMount);
  afterEach(cleanup);

  it('double run() throws', () => {
    const app = new App().route('/', () => html`<div>home</div>`);
    app.run('#app');
    expect(() => app.run('#app')).toThrow('already running');
  });

  it('run() with missing selector throws', () => {
    expect(() => new App().run('#nope')).toThrow('not found');
  });

  it('after run(), calling state/layout/route throws', () => {
    const app = new App().route('/', () => html`<div>home</div>`);
    app.run('#app');
    expect(() => app.state('x', 1)).toThrow('cannot be called after run');
    expect(() => app.layout(() => null)).toThrow('cannot be called after run');
    expect(() => app.route('/x', () => null)).toThrow('cannot be called after run');
  });

  it('mount + initial render writes route content into mount element', async () => {
    new App().route('/', () => html`<div>home</div>`).run('#app');
    await Promise.resolve();
    expect(document.querySelector('#app').querySelector('div')).toBeTruthy();
  });

  it('layout wraps route content; without layout, route renders directly', async () => {
    new App()
      .layout(({ outlet }) => html`<main>${outlet}</main>`)
      .route('/', () => html`<span>content</span>`)
      .run('#app');
    await Promise.resolve();
    const mountEl = document.querySelector('#app');
    expect(mountEl.querySelector('main')).toBeTruthy();
    expect(mountEl.querySelector('span')).toBeTruthy();
  });

  it('layout component is invoked exactly once across multiple navigations', async () => {
    let layoutCount = 0;
    const app = new App()
      .layout(({ outlet }) => { layoutCount++; return html`<main>${outlet}</main>`; })
      .route('/', () => html`<span>home</span>`)
      .route('/about', () => html`<span>about</span>`);
    app.run('#app');
    await Promise.resolve();
    expect(layoutCount).toBe(1);
    window.history.pushState(null, '', '/about');
    fire(window, 'popstate');
    await Promise.resolve();
    expect(layoutCount).toBe(1);
  });

  it('route component receives state proxy; state.foo returns registered value', async () => {
    const fooSig = signal('bar');
    let receivedState;
    const app = new App()
      .state('foo', fooSig)
      .route('/', ({ state }) => { receivedState = state; return html`<div>x</div>`; });
    app.run('#app');
    await Promise.resolve();
    expect(receivedState).toBeTruthy();
    expect(receivedState.foo).toBe(fooSig);
  });

  it('component that does not destructure state still works', async () => {
    const app = new App().route('/', () => html`<div>ok</div>`);
    app.run('#app');
    await Promise.resolve();
    expect(document.querySelector('#app').querySelector('div')).toBeTruthy();
  });

  it('without layout, content still renders inside mount across navigations', async () => {
    const app = new App()
      .route('/', () => html`<span>home</span>`)
      .route('/about', () => html`<span>about</span>`);
    app.run('#app');
    await Promise.resolve();
    expect(document.querySelector('#app').querySelector('span')).toBeTruthy();
    window.history.pushState(null, '', '/about');
    fire(window, 'popstate');
    await Promise.resolve();
    expect(document.querySelector('#app').querySelector('span')).toBeTruthy();
  });

  it('click interception: plain <a href="/about"> navigates', async () => {
    new App()
      .route('/', () => html`<div>home</div>`)
      .route('/about', () => html`<div>about</div>`)
      .run('#app');
    await Promise.resolve();
    const anchor = document.createElement('a');
    anchor.setAttribute('href', '/about');
    fire(document, 'click', { target: anchor, button: 0 });
    await Promise.resolve();
    expect(window.location.pathname).toBe('/about');
  });

  it('click interception: span inside anchor navigates (ancestor walk)', async () => {
    new App()
      .route('/', () => html`<div>home</div>`)
      .route('/about', () => html`<div>about</div>`)
      .run('#app');
    await Promise.resolve();
    const anchor = document.createElement('a');
    anchor.setAttribute('href', '/about');
    const span = document.createElement('span');
    anchor.appendChild(span);
    fire(document, 'click', { target: span, button: 0 });
    await Promise.resolve();
    expect(window.location.pathname).toBe('/about');
  });

  it('click interception: target="_blank" does not navigate', async () => {
    new App().route('/', () => html`<div>home</div>`).run('#app');
    await Promise.resolve();
    const anchor = document.createElement('a');
    anchor.setAttribute('href', '/about');
    anchor.setAttribute('target', '_blank');
    fire(document, 'click', { target: anchor, button: 0 });
    expect(window.location.pathname).toBe('/');
  });

  it('click interception: download / data-external / external href do not navigate', async () => {
    new App().route('/', () => html`<div>home</div>`).run('#app');
    await Promise.resolve();
    const cases = [
      () => { const a = document.createElement('a'); a.setAttribute('href', '/f'); a.setAttribute('download', ''); return a; },
      () => { const a = document.createElement('a'); a.setAttribute('href', '/f'); a.setAttribute('data-external', ''); return a; },
      () => { const a = document.createElement('a'); a.setAttribute('href', 'https://example.com/page'); return a; },
    ];
    for (const makeAnchor of cases) {
      const anchor = makeAnchor();
      fire(document, 'click', { target: anchor, button: 0 });
      expect(window.location.pathname).toBe('/');
    }
  });

  it('click interception: metaKey/button:1 do not navigate', async () => {
    new App().route('/', () => html`<div>home</div>`).run('#app');
    await Promise.resolve();
    const anchor = document.createElement('a');
    anchor.setAttribute('href', '/about');
    fire(document, 'click', { target: anchor, button: 0, metaKey: true });
    expect(window.location.pathname).toBe('/');
    fire(document, 'click', { target: anchor, button: 1 });
    expect(window.location.pathname).toBe('/');
  });

  it('popstate re-renders the matching route', async () => {
    new App()
      .route('/', () => html`<div>home</div>`)
      .route('/about', () => html`<div>about</div>`)
      .run('#app');
    await Promise.resolve();
    window.history.pushState(null, '', '/about');
    fire(window, 'popstate');
    await Promise.resolve();
    expect(window.location.pathname).toBe('/about');
  });

  it('eager component: first call reuses TR; second navigation calls component again', async () => {
    let counter = 0;
    const app = new App()
      .route('/', () => { counter++; return html`<div>home</div>`; })
      .route('/other', () => html`<div>other</div>`);
    app.run('#app');
    await Promise.resolve();
    expect(counter).toBe(1);
    window.history.pushState(null, '', '/other');
    fire(window, 'popstate');
    await Promise.resolve();
    window.history.pushState(null, '', '/');
    fire(window, 'popstate');
    await Promise.resolve();
    expect(counter).toBe(2);
  });

  it('lazy loader: called once; cached on second navigation', async () => {
    let loaderCalls = 0;
    const Component = () => html`<div>lazy</div>`;
    const app = new App()
      .route('/', () => { loaderCalls++; return Promise.resolve({ default: Component }); })
      .route('/other', () => html`<div>other</div>`);
    app.run('#app');
    await Promise.resolve(); await Promise.resolve();
    expect(loaderCalls).toBe(1);
    window.history.pushState(null, '', '/other');
    fire(window, 'popstate');
    await Promise.resolve(); await Promise.resolve();
    window.history.pushState(null, '', '/');
    fire(window, 'popstate');
    await Promise.resolve(); await Promise.resolve();
    expect(loaderCalls).toBe(1);
  });

  it('data-active and data-active-exact on matching anchors', async () => {
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
    expect(homeAnchor.hasAttribute('data-active-exact')).toBeTruthy();
    expect(homeAnchor.hasAttribute('data-active')).toBeTruthy();
    expect(aboutAnchor.hasAttribute('data-active-exact')).toBeFalsy();
    expect(aboutAnchor.hasAttribute('data-active')).toBeFalsy();
  });

  it('route-change disposal: abandoned scope effects do not fire', async () => {
    const sig = signal('initial');
    let updateCount = 0;
    const app = new App()
      .route('/', () => html`<div>${() => { updateCount++; return sig.val; }}</div>`)
      .route('/other', () => html`<span>other</span>`);
    app.run('#app');
    await Promise.resolve();
    const initialCount = updateCount;
    window.history.pushState(null, '', '/other');
    fire(window, 'popstate');
    await Promise.resolve();
    sig.set('changed');
    expect(updateCount).toBe(initialCount);
  });
});
