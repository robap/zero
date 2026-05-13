import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import {
  _normalizePath,
  _parseQuery,
  _parsePathAndQuery,
  _compileRoutePattern,
  _matchAgainst,
  _matchRoutes,
  _joinPaths,
  navigate,
  back,
  forward,
  route,
} from './router.js';
import { App, _setCurrentApp } from './app.js';
import { window, document } from './dom-shim.js';
import { html } from './template.js';
import { effect } from './reactivity.js';

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

function freshApp() {
  freshMount('app');
  const app = new App()
    .route('/', () => html`<span>home</span>`)
    .route('/about', () => html`<span>about</span>`);
  app.run('#app');
  return app;
}

describe('router', () => {
  describe('_normalizePath', () => {
    it('strips trailing slash unless root', () => {
      assert.equal(_normalizePath('/about/'), '/about');
      assert.equal(_normalizePath('/'), '/');
      assert.equal(_normalizePath('/users/42/'), '/users/42');
      assert.equal(_normalizePath('/about'), '/about');
    });
  });

  describe('navigate / back / forward / route()', () => {
    beforeEach(resetEnv);

    it('navigate("/about") updates history and renders the route', async () => {
      freshApp();
      navigate('/about');
      await Promise.resolve();
      assert.equal(window.history.length, 2);
      assert.equal(window.location.pathname, '/about');
    });

    it('navigate with replace:true does not advance history', async () => {
      freshApp();
      navigate('/about', { replace: true });
      await Promise.resolve();
      assert.equal(window.history.length, 1);
      assert.equal(window.location.pathname, '/about');
    });

    it('navigate with state attaches state to history entry', async () => {
      freshApp();
      navigate('/about', { state: { from: 'x' } });
      assert.deepEqual(window.history._entries[window.history._index].state, { from: 'x' });
    });

    it('back() after two pushes dispatches popstate and re-renders', async () => {
      freshApp();
      navigate('/about');
      await Promise.resolve();
      back();
      await Promise.resolve();
      assert.equal(window.location.pathname, '/');
    });

    it('navigate outside running app throws', () => {
      _setCurrentApp(null);
      assert.throws(() => navigate('/about'), /no app is running/);
    });

    it('route() outside running app throws', () => {
      _setCurrentApp(null);
      assert.throws(() => route(), /no app is running/);
    });

    it('route() is reactive: effect re-runs after navigate', async () => {
      freshApp();
      let last = route().path;
      effect(() => { last = route().path; });
      navigate('/about');
      await Promise.resolve();
      assert.equal(last, '/about');
    });

    it('two route() calls return distinct objects with same underlying values', async () => {
      freshApp();
      const r1 = route();
      const r2 = route();
      assert.notEqual(r1, r2);
      assert.equal(r1.path, r2.path);
      assert.deepEqual(r1.params, r2.params);
    });
  });

  describe('_matchRoutes', () => {
    function makeEntry(pattern) {
      const compiled = _compileRoutePattern(pattern);
      return { pattern, compiled, loader: null, resolvedComponent: null };
    }

    it('first match wins in registration order', () => {
      const about = makeEntry('/about');
      const wildcard = makeEntry('*');
      const m = _matchRoutes([about, wildcard], '/about');
      assert.equal(m.route, about);
      assert.deepEqual(m.params, {});
    });

    it('wildcard catches unmatched routes when registered last', () => {
      const about = makeEntry('/about');
      const wildcard = makeEntry('*');
      const m = _matchRoutes([about, wildcard], '/other');
      assert.equal(m.route, wildcard);
    });

    it('returns null when nothing matches', () => {
      const about = makeEntry('/about');
      assert.equal(_matchRoutes([about], '/nope'), null);
    });

    it('normalizes trailing slash on input', () => {
      const about = makeEntry('/about');
      const m = _matchRoutes([about], '/about/');
      assert.ok(m);
      assert.equal(m.pathname, '/about');
    });

    it('drops hash from input', () => {
      const about = makeEntry('/about');
      const m = _matchRoutes([about], '/about#section');
      assert.ok(m);
      assert.equal(m.search, '');
    });

    it('parses query into result', () => {
      const users = makeEntry('/users/:id');
      const m = _matchRoutes([users], '/users/42?tab=posts');
      assert.ok(m);
      assert.deepEqual(m.params, { id: '42' });
      assert.deepEqual(m.query, { tab: 'posts' });
    });
  });

  describe('_parsePathAndQuery', () => {
    it('splits path, search, and drops hash', () => {
      assert.deepEqual(_parsePathAndQuery('/about?x=1#y'), { pathname: '/about', search: '?x=1' });
    });

    it('returns empty search when no query', () => {
      assert.deepEqual(_parsePathAndQuery('/about'), { pathname: '/about', search: '' });
    });

    it('returns empty search for hash-only suffix', () => {
      assert.deepEqual(_parsePathAndQuery('/about#section'), { pathname: '/about', search: '' });
    });
  });

  describe('_compileRoutePattern / _matchAgainst', () => {
    it('single param pattern matches and captures', () => {
      const compiled = _compileRoutePattern('/users/:id');
      assert.deepEqual(_matchAgainst(compiled, '/users/42'), { params: { id: '42' } });
      assert.equal(_matchAgainst(compiled, '/users/42/posts'), null);
    });

    it('multi-param pattern captures all params', () => {
      const compiled = _compileRoutePattern('/users/:id/posts/:postId');
      assert.deepEqual(
        _matchAgainst(compiled, '/users/7/posts/99'),
        { params: { id: '7', postId: '99' } },
      );
    });

    it('wildcard matches anything with empty params', () => {
      const compiled = _compileRoutePattern('*');
      assert.deepEqual(_matchAgainst(compiled, '/anything/here'), { params: {} });
    });

    it('decoded params: percent-encoded segment', () => {
      const compiled = _compileRoutePattern('/users/:id');
      assert.deepEqual(_matchAgainst(compiled, '/users/%C3%A9'), { params: { id: 'é' } });
    });
  });

  describe('_joinPaths', () => {
    it('child "/" returns parent', () => {
      assert.equal(_joinPaths('/dashboard', '/'), '/dashboard');
    });

    it('joins parent + child path', () => {
      assert.equal(_joinPaths('/dashboard', '/analytics'), '/dashboard/analytics');
    });

    it('parent "/" returns child', () => {
      assert.equal(_joinPaths('/', '/about'), '/about');
    });

    it('parent "/" and child "/" returns "/"', () => {
      assert.equal(_joinPaths('/', '/'), '/');
    });

    it('normalizes trailing slash on parent before join', () => {
      assert.equal(_joinPaths('/dashboard/', '/stats'), '/dashboard/stats');
    });
  });

  describe('nested-route flattening', () => {
    beforeEach(resetEnv);

    it('one-level: children produce two entries with correct normalized paths', () => {
      const P = () => html`<div>parent</div>`;
      const O = () => html`<div>overview</div>`;
      const A = () => html`<div>analytics</div>`;
      const app = new App().route('/dashboard', P, {
        children: [
          { path: '/', load: O },
          { path: '/analytics', load: A },
        ],
      });
      assert.equal(app._routes.length, 2);
      assert.equal(app._routes[0].normalized, '/dashboard');
      assert.equal(app._routes[1].normalized, '/dashboard/analytics');
    });

    it('one-level: both entries have chain.length === 2', () => {
      const P = () => html`<div>parent</div>`;
      const O = () => html`<div>overview</div>`;
      const A = () => html`<div>analytics</div>`;
      const app = new App().route('/dashboard', P, {
        children: [
          { path: '/', load: O },
          { path: '/analytics', load: A },
        ],
      });
      assert.equal(app._routes[0].chain.length, 2);
      assert.equal(app._routes[1].chain.length, 2);
    });

    it('sibling parent-descriptor reuse: chain[0] is identical by reference in both children', () => {
      const P = () => html`<div>parent</div>`;
      const O = () => html`<div>overview</div>`;
      const A = () => html`<div>analytics</div>`;
      const app = new App().route('/dashboard', P, {
        children: [
          { path: '/', load: O },
          { path: '/analytics', load: A },
        ],
      });
      assert.strictEqual(app._routes[0].chain[0], app._routes[1].chain[0]);
    });

    it('child descriptors carry correct loaderOrLoad', () => {
      const P = () => html`<div>parent</div>`;
      const O = () => html`<div>overview</div>`;
      const A = () => html`<div>analytics</div>`;
      const app = new App().route('/dashboard', P, {
        children: [
          { path: '/', load: O },
          { path: '/analytics', load: A },
        ],
      });
      assert.strictEqual(app._routes[0].chain[1].loaderOrLoad, O);
      assert.strictEqual(app._routes[1].chain[1].loaderOrLoad, A);
    });

    it('two-level nesting produces entry at /dashboard/foo/bar with chain.length === 3', () => {
      const P = () => html`<div>parent</div>`;
      const Leaf = () => html`<div>leaf</div>`;
      const app = new App().route('/dashboard', P, {
        children: [
          {
            path: '/foo',
            load: () => null,
            children: [
              { path: '/bar', load: Leaf },
            ],
          },
        ],
      });
      assert.equal(app._routes.length, 1);
      assert.equal(app._routes[0].normalized, '/dashboard/foo/bar');
      assert.equal(app._routes[0].chain.length, 3);
    });

    it('plain top-level route (no children) has chain: [self]', () => {
      const Home = () => html`<div>home</div>`;
      const app = new App().route('/', Home);
      assert.equal(app._routes[0].chain.length, 1);
      assert.strictEqual(app._routes[0].chain[0].loaderOrLoad, Home);
    });
  });

  describe('_parseQuery', () => {
    it('empty string returns {}', () => {
      assert.deepEqual(_parseQuery(''), {});
    });

    it('parses key=value pairs', () => {
      assert.deepEqual(_parseQuery('?a=1&b=2'), { a: '1', b: '2' });
    });

    it('decodes percent-encoded values', () => {
      assert.deepEqual(_parseQuery('?c=hello%20world'), { c: 'hello world' });
    });

    it('last value wins for duplicate keys', () => {
      assert.deepEqual(_parseQuery('?k=first&k=second'), { k: 'second' });
    });

    it('empty value is empty string', () => {
      assert.deepEqual(_parseQuery('?k='), { k: '' });
    });
  });
});
