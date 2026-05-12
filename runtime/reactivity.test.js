import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { signal, computed, effect, _createScope } from './reactivity.js';

describe('signal', () => {
  it('returns initial value', () => {
    const s = signal(42);
    assert.equal(s.val, 42);
  });

  it('.set() updates val', () => {
    const s = signal(0);
    s.set(10);
    assert.equal(s.val, 10);
  });

  it('.update() transforms val using current value', () => {
    const s = signal(5);
    s.update(v => v * 2);
    assert.equal(s.val, 10);
  });

  it('does not notify subscribers when value is strictly equal', () => {
    const s = signal(0);
    let count = 0;
    effect(() => { s.val; count++; });
    assert.equal(count, 1);
    s.set(0);
    assert.equal(count, 1);
  });

  it('notifies subscribers on distinct value', () => {
    const s = signal(0);
    let count = 0;
    effect(() => { s.val; count++; });
    s.set(1);
    assert.equal(count, 2);
  });
});

describe('computed', () => {
  it('returns derived value', () => {
    const s = signal(3);
    const c = computed(() => s.val + 1);
    assert.equal(c.val, 4);
  });

  it('updates when dependency changes', () => {
    const s = signal(1);
    const c = computed(() => s.val * 2);
    s.set(5);
    assert.equal(c.val, 10);
  });

  it('is lazy — fn does not run until .val is read', () => {
    const s = signal(0);
    let runs = 0;
    const c = computed(() => { runs++; return s.val; });
    assert.equal(runs, 0);
    c.val;
    assert.equal(runs, 1);
    s.set(1);
    assert.equal(runs, 1); // dirty but not yet re-evaluated
    c.val;
    assert.equal(runs, 2);
  });

  it('does not re-run fn when value has not changed', () => {
    const s = signal(0);
    let runs = 0;
    const c = computed(() => { runs++; return s.val; });
    c.val;
    c.val;
    assert.equal(runs, 1);
  });

  it('clears stale deps on re-evaluation (conditional branches)', () => {
    const cond = signal(true);
    const a = signal(1);
    const b = signal(2);
    let runs = 0;
    const c = computed(() => { runs++; return cond.val ? a.val : b.val; });

    assert.equal(c.val, 1);
    assert.equal(runs, 1);

    cond.set(false);
    assert.equal(c.val, 2);
    assert.equal(runs, 2);

    // a is now stale — updating it should not trigger re-eval
    a.set(99);
    c.val;
    assert.equal(runs, 2);
  });

  it('supports computed depending on computed', () => {
    const s = signal(2);
    const c1 = computed(() => s.val * 2);
    const c2 = computed(() => c1.val + 1);
    assert.equal(c2.val, 5);
    s.set(5);
    assert.equal(c2.val, 11);
  });
});

describe('effect', () => {
  it('runs fn immediately on creation', () => {
    let ran = false;
    effect(() => { ran = true; });
    assert.ok(ran);
  });

  it('re-runs when a signal dependency changes', () => {
    const s = signal(0);
    let count = 0;
    effect(() => { s.val; count++; });
    assert.equal(count, 1);
    s.set(1);
    assert.equal(count, 2);
  });

  it('calls cleanup before each re-run', () => {
    const s = signal(0);
    const log = [];
    effect(() => {
      s.val;
      log.push('run');
      return () => log.push('cleanup');
    });
    assert.deepEqual(log, ['run']);
    s.set(1);
    assert.deepEqual(log, ['run', 'cleanup', 'run']);
    s.set(2);
    assert.deepEqual(log, ['run', 'cleanup', 'run', 'cleanup', 'run']);
  });

  it('calls cleanup when stop() is invoked', () => {
    let cleaned = false;
    const stop = effect(() => () => { cleaned = true; });
    assert.ok(!cleaned);
    stop();
    assert.ok(cleaned);
  });

  it('stop() prevents further re-runs', () => {
    const s = signal(0);
    let count = 0;
    const stop = effect(() => { s.val; count++; });
    assert.equal(count, 1);
    stop();
    s.set(1);
    assert.equal(count, 1);
  });

  it('reacts to computed dependency changes', () => {
    const s = signal(1);
    const c = computed(() => s.val * 10);
    let last;
    effect(() => { last = c.val; });
    assert.equal(last, 10);
    s.set(3);
    assert.equal(last, 30);
  });
});

describe('scope (internal)', () => {
  it('dispose() stops effects registered within the scope', () => {
    const s = signal(0);
    let count = 0;
    const scope = _createScope();
    scope.run(() => {
      effect(() => { s.val; count++; });
    });
    assert.equal(count, 1);
    scope.dispose();
    s.set(1);
    assert.equal(count, 1);
  });

  it('dispose() recursively disposes child scopes', () => {
    const s = signal(0);
    let count = 0;
    const parent = _createScope();
    parent.run(() => {
      const child = _createScope();
      child.run(() => {
        effect(() => { s.val; count++; });
      });
    });
    assert.equal(count, 1);
    parent.dispose();
    s.set(1);
    assert.equal(count, 1);
  });

  it('scope.run() restores active scope after completion', () => {
    const s = signal(0);
    let outerCount = 0;
    let innerCount = 0;
    let inner;

    const outer = _createScope();
    outer.run(() => {
      inner = _createScope();
      inner.run(() => {
        effect(() => { s.val; innerCount++; });
      });
      // active scope is restored to outer here
      effect(() => { s.val; outerCount++; });
    });

    assert.equal(outerCount, 1);
    assert.equal(innerCount, 1);

    s.set(1);
    assert.equal(outerCount, 2);
    assert.equal(innerCount, 2);

    inner.dispose();
    s.set(2);
    assert.equal(outerCount, 3);  // outer effect still runs
    assert.equal(innerCount, 2);  // inner effect stopped
  });
});
