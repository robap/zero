import { describe, it, expect } from 'zero/test';
import { signal, computed, effect, _createScope, _disposeUnownedEffects } from 'zero';

describe('signal', () => {
  it('returns initial value', () => {
    const s = signal(42);
    expect(s.val).toBe(42);
  });

  it('.set() updates val', () => {
    const s = signal(0);
    s.set(10);
    expect(s.val).toBe(10);
  });

  it('.update() transforms val using current value', () => {
    const s = signal(5);
    s.update(v => v * 2);
    expect(s.val).toBe(10);
  });

  it('does not notify subscribers when value is strictly equal', () => {
    const s = signal(0);
    let count = 0;
    effect(() => { s.val; count++; });
    expect(count).toBe(1);
    s.set(0);
    expect(count).toBe(1);
  });

  it('notifies subscribers on distinct value', () => {
    const s = signal(0);
    let count = 0;
    effect(() => { s.val; count++; });
    s.set(1);
    expect(count).toBe(2);
  });
});

describe('computed', () => {
  it('returns derived value', () => {
    const s = signal(3);
    const c = computed(() => s.val + 1);
    expect(c.val).toBe(4);
  });

  it('updates when dependency changes', () => {
    const s = signal(1);
    const c = computed(() => s.val * 2);
    s.set(5);
    expect(c.val).toBe(10);
  });

  it('is lazy — fn does not run until .val is read', () => {
    const s = signal(0);
    let runs = 0;
    const c = computed(() => { runs++; return s.val; });
    expect(runs).toBe(0);
    c.val;
    expect(runs).toBe(1);
    s.set(1);
    expect(runs).toBe(1); // dirty but not yet re-evaluated
    c.val;
    expect(runs).toBe(2);
  });

  it('does not re-run fn when value has not changed', () => {
    const s = signal(0);
    let runs = 0;
    const c = computed(() => { runs++; return s.val; });
    c.val;
    c.val;
    expect(runs).toBe(1);
  });

  it('clears stale deps on re-evaluation (conditional branches)', () => {
    const cond = signal(true);
    const a = signal(1);
    const b = signal(2);
    let runs = 0;
    const c = computed(() => { runs++; return cond.val ? a.val : b.val; });

    expect(c.val).toBe(1);
    expect(runs).toBe(1);

    cond.set(false);
    expect(c.val).toBe(2);
    expect(runs).toBe(2);

    // a is now stale — updating it should not trigger re-eval
    a.set(99);
    c.val;
    expect(runs).toBe(2);
  });

  it('supports computed depending on computed', () => {
    const s = signal(2);
    const c1 = computed(() => s.val * 2);
    const c2 = computed(() => c1.val + 1);
    expect(c2.val).toBe(5);
    s.set(5);
    expect(c2.val).toBe(11);
  });
});

describe('effect', () => {
  it('runs fn immediately on creation', () => {
    let ran = false;
    effect(() => { ran = true; });
    expect(ran).toBeTruthy();
  });

  it('re-runs when a signal dependency changes', () => {
    const s = signal(0);
    let count = 0;
    effect(() => { s.val; count++; });
    expect(count).toBe(1);
    s.set(1);
    expect(count).toBe(2);
  });

  it('calls cleanup before each re-run', () => {
    const s = signal(0);
    const log = [];
    effect(() => {
      s.val;
      log.push('run');
      return () => log.push('cleanup');
    });
    expect(log).toEqual(['run']);
    s.set(1);
    expect(log).toEqual(['run', 'cleanup', 'run']);
    s.set(2);
    expect(log).toEqual(['run', 'cleanup', 'run', 'cleanup', 'run']);
  });

  it('calls cleanup when stop() is invoked', () => {
    let cleaned = false;
    const stop = effect(() => () => { cleaned = true; });
    expect(cleaned).toBeFalsy();
    stop();
    expect(cleaned).toBeTruthy();
  });

  it('stop() prevents further re-runs', () => {
    const s = signal(0);
    let count = 0;
    const stop = effect(() => { s.val; count++; });
    expect(count).toBe(1);
    stop();
    s.set(1);
    expect(count).toBe(1);
  });

  it('reacts to computed dependency changes', () => {
    const s = signal(1);
    const c = computed(() => s.val * 10);
    let last;
    effect(() => { last = c.val; });
    expect(last).toBe(10);
    s.set(3);
    expect(last).toBe(30);
  });
});

describe('_disposeUnownedEffects', () => {
  it('disposes an effect created with no active scope', () => {
    let cleaned = false;
    effect(() => () => { cleaned = true; });
    _disposeUnownedEffects();
    expect(cleaned).toBeTruthy();
  });

  it('does not dispose effects created inside scope.run()', () => {
    const scope = _createScope();
    let cleaned = false;
    scope.run(() => {
      effect(() => () => { cleaned = true; });
    });
    _disposeUnownedEffects();
    expect(cleaned).toBeFalsy();
    scope.dispose();
    expect(cleaned).toBeTruthy();
  });

  it('does not double-stop a manually-stopped unowned effect', () => {
    let cleanups = 0;
    const stop = effect(() => () => { cleanups++; });
    stop();
    _disposeUnownedEffects();
    expect(cleanups).toBe(1);
  });

  it('runs each unowned effect cleanup callback exactly once', () => {
    let cleanups = 0;
    effect(() => () => { cleanups++; });
    _disposeUnownedEffects();
    expect(cleanups).toBe(1);
  });

  it('after disposal, signal mutations do not re-fire the effect', () => {
    const s = signal(0);
    let runs = 0;
    effect(() => { s.val; runs++; });
    expect(runs).toBe(1);
    _disposeUnownedEffects();
    s.set(1);
    expect(runs).toBe(1);
  });
});
