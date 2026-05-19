import {
  describe,
  it,
  expect,
  beforeEach,
  afterEach,
  cleanup,
  spy,
  find,
  text,
} from 'zero/test';
import { App, navigate, back, route, html, effect } from 'zero';

function freshMount() {
  cleanup();
  window.history.pushState(null, '', '/');
  const mount = document.createElement('div');
  mount.setAttribute('id', 'app');
  document.body.appendChild(mount);
  return mount;
}

function freshApp() {
  freshMount();
  const app = new App()
    .route('/', () => html`<span>home</span>`)
    .route('/about', () => html`<span>about</span>`);
  app.run('#app');
  return app;
}

describe('router', () => {
  describe('navigate / back / forward / route()', () => {
    afterEach(cleanup);

    it('navigate("/about") updates history and renders the route', async () => {
      freshApp();
      const lengthBefore = window.history.length;
      navigate('/about');
      await Promise.resolve();
      expect(window.history.length).toBe(lengthBefore + 1);
      expect(window.location.pathname).toBe('/about');
    });

    it('navigate with replace:true does not advance history', async () => {
      freshApp();
      const lengthBefore = window.history.length;
      navigate('/about', { replace: true });
      await Promise.resolve();
      expect(window.history.length).toBe(lengthBefore);
      expect(window.location.pathname).toBe('/about');
    });

    it('back() after a push dispatches popstate and re-renders', async () => {
      freshApp();
      navigate('/about');
      await Promise.resolve();
      back();
      await Promise.resolve();
      expect(window.location.pathname).toBe('/');
    });

    it('navigate outside running app throws', () => {
      cleanup();
      expect(() => navigate('/about')).toThrow('no app is running');
    });

    it('route() outside running app throws', () => {
      cleanup();
      expect(() => route()).toThrow('no app is running');
    });

    it('route() is reactive: effect re-runs after navigate', async () => {
      freshApp();
      let last = route().path;
      effect(() => { last = route().path; });
      navigate('/about');
      await Promise.resolve();
      expect(last).toBe('/about');
    });

    it('two route() calls return distinct objects with same underlying values', () => {
      freshApp();
      const r1 = route();
      const r2 = route();
      expect(r1 === r2).toBeFalsy();
      expect(r1.path).toBe(r2.path);
      expect(r1.params).toEqual(r2.params);
    });

    it('param paths are percent-decoded into route().params', async () => {
      freshMount();
      const renderSpy = spy(() => html`<span>user</span>`);
      const app = new App().route('/users/:id', renderSpy);
      app.run('#app');
      navigate('/users/%C3%A9');
      await Promise.resolve();
      expect(route().params).toEqual({ id: 'é' });
    });

    it('multi-param patterns capture all params', async () => {
      freshMount();
      const app = new App().route(
        '/users/:id/posts/:postId',
        () => html`<span>post</span>`,
      );
      app.run('#app');
      navigate('/users/7/posts/99');
      await Promise.resolve();
      expect(route().params).toEqual({ id: '7', postId: '99' });
    });

    it('query string parses into route().query', async () => {
      freshMount();
      const app = new App().route('/users/:id', () => html`<span>x</span>`);
      app.run('#app');
      navigate('/users/42?tab=posts');
      await Promise.resolve();
      expect(route().query).toEqual({ tab: 'posts' });
    });

    it('trailing slash on input is normalized', async () => {
      freshMount();
      const app = new App().route('/about', () => html`<span>about</span>`);
      app.run('#app');
      navigate('/about/');
      await Promise.resolve();
      expect(route().path).toBe('/about');
    });
  });

  describe('nested-route flattening', () => {
    afterEach(cleanup);

    it('one-level: child mounts inside parent outlet', async () => {
      freshMount();
      const Parent = ({ outlet }) => html`<div><span>parent</span>${outlet}</div>`;
      const Analytics = () => html`<i>analytics</i>`;
      const app = new App().route('/dashboard', Parent, {
        children: [{ path: '/analytics', load: Analytics }],
      });
      app.run('#app');
      navigate('/dashboard/analytics');
      await Promise.resolve(); await Promise.resolve();
      const mount = find(document, '#app');
      expect(find(mount, 'span')).toBeTruthy();
      expect(text(mount)).toContain('analytics');
    });

    it('one-level: index child (path "/") mounts inside parent outlet', async () => {
      freshMount();
      const Parent = ({ outlet }) => html`<div><span>p</span>${outlet}</div>`;
      const Overview = () => html`<em>overview</em>`;
      const app = new App().route('/dashboard', Parent, {
        children: [{ path: '/', load: Overview }],
      });
      app.run('#app');
      navigate('/dashboard');
      await Promise.resolve(); await Promise.resolve();
      const mount = find(document, '#app');
      expect(text(mount)).toContain('overview');
    });

    it('parent is reused when navigating between sibling children', async () => {
      freshMount();
      let parentCalls = 0;
      const Parent = ({ outlet }) => {
        parentCalls++;
        return html`<div><span>p</span>${outlet}</div>`;
      };
      const A = () => html`<i>a</i>`;
      const B = () => html`<i>b</i>`;
      const app = new App().route('/d', Parent, {
        children: [
          { path: '/a', load: A },
          { path: '/b', load: B },
        ],
      });
      app.run('#app');
      navigate('/d/a');
      await Promise.resolve(); await Promise.resolve();
      const callsAfterFirst = parentCalls;
      navigate('/d/b');
      await Promise.resolve(); await Promise.resolve();
      expect(parentCalls).toBe(callsAfterFirst);
      const mount = find(document, '#app');
      expect(text(mount)).toContain('b');
    });

    it('two-level nesting renders both grandparents in the outlet chain', async () => {
      freshMount();
      const Root = ({ outlet }) => html`<div><span>root</span>${outlet}</div>`;
      const Mid = ({ outlet }) => html`<div><em>mid</em>${outlet}</div>`;
      const Leaf = () => html`<i>leaf</i>`;
      const app = new App().route('/dashboard', Root, {
        children: [
          {
            path: '/foo',
            load: Mid,
            children: [{ path: '/bar', load: Leaf }],
          },
        ],
      });
      app.run('#app');
      navigate('/dashboard/foo/bar');
      await Promise.resolve(); await Promise.resolve(); await Promise.resolve();
      const mount = find(document, '#app');
      const t = text(mount);
      expect(t).toContain('root');
      expect(t).toContain('mid');
      expect(t).toContain('leaf');
    });

    it('plain top-level route (no children) renders alone', async () => {
      freshMount();
      const Home = () => html`<div>home</div>`;
      const app = new App().route('/', Home);
      app.run('#app');
      await Promise.resolve();
      const mount = find(document, '#app');
      expect(text(mount)).toContain('home');
    });
  });

  describe('route-scoped fetch', () => {
    afterEach(cleanup);

    it('aborts pending fetch when navigation supersedes', async () => {
      freshMount();
      const origFetch = globalThis.fetch;
      const seenSignals = [];
      globalThis.fetch = (_input, init = {}) => {
        const signal = init.signal;
        if (signal) seenSignals.push(signal);
        return new Promise((_resolve, reject) => {
          if (signal) {
            signal.addEventListener('abort', () => {
              const err = new Error('aborted');
              err.name = 'AbortError';
              reject(err);
            });
          }
        });
      };
      try {
        const app = new App()
          .route('/', () => html`<span>home</span>`)
          .route('/slow', () => html`<span>slow</span>`, {
            load: ({ fetch }) => fetch('/data'),
          })
          .route('/other', () => html`<span>other</span>`);
        app.run('#app');
        await Promise.resolve();
        navigate('/slow');
        await Promise.resolve();
        expect(seenSignals.length).toBe(1);
        expect(seenSignals[0].aborted).toBe(false);
        navigate('/other');
        await Promise.resolve(); await Promise.resolve(); await Promise.resolve();
        expect(seenSignals[0].aborted).toBe(true);
      } finally {
        globalThis.fetch = origFetch;
      }
    });

    it('composes caller-supplied signal: caller abort surfaces as error', async () => {
      freshMount();
      const origFetch = globalThis.fetch;
      let abortReason = null;
      globalThis.fetch = (_input, init = {}) => {
        const signal = init.signal;
        return new Promise((_resolve, reject) => {
          if (signal) {
            signal.addEventListener('abort', () => {
              const err = new Error('aborted');
              err.name = 'AbortError';
              reject(err);
            });
          }
        });
      };
      try {
        const callerController = new AbortController();
        const errorCalls = [];
        const app = new App()
          .error(({ error }) => { errorCalls.push(error); return html`<span>err</span>`; })
          .route('/', () => html`<span>home</span>`)
          .route('/x', () => html`<span>x</span>`, {
            load: ({ fetch }) => fetch('/data', { signal: callerController.signal })
              .catch((e) => { abortReason = e; throw e; }),
          });
        app.run('#app');
        await Promise.resolve();
        navigate('/x');
        await Promise.resolve();
        callerController.abort();
        await Promise.resolve(); await Promise.resolve(); await Promise.resolve();
        expect(abortReason).toBeTruthy();
        expect(abortReason.name).toBe('AbortError');
        expect(errorCalls.length).toBe(1);
      } finally {
        globalThis.fetch = origFetch;
      }
    });

    it('post-navigation: each nav gets a fresh non-aborted signal', async () => {
      freshMount();
      const origFetch = globalThis.fetch;
      const seenSignals = [];
      globalThis.fetch = (_input, init = {}) => {
        if (init.signal) seenSignals.push(init.signal);
        return Promise.resolve({ ok: true, json: async () => ({}) });
      };
      try {
        const app = new App()
          .route('/', () => html`<span>home</span>`, {
            load: ({ fetch }) => fetch('/a'),
          })
          .route('/b', () => html`<span>b</span>`, {
            load: ({ fetch }) => fetch('/b'),
          });
        app.run('#app');
        await Promise.resolve(); await Promise.resolve(); await Promise.resolve();
        navigate('/b');
        await Promise.resolve(); await Promise.resolve(); await Promise.resolve();
        expect(seenSignals.length).toBe(2);
        expect(seenSignals[0] === seenSignals[1]).toBeFalsy();
        expect(seenSignals[1].aborted).toBe(false);
      } finally {
        globalThis.fetch = origFetch;
      }
    });
  });
});
