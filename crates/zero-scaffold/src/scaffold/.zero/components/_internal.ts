import type { Signal, Computed } from "zero";

/**
 * Either a writable signal or a read-only computed of the same value.
 * Component props that say "reactive or plain" should accept this plus `T`.
 *
 * @template T
 * @internal
 */
export type Reactive<T> = Signal<T> | Computed<T>;

/**
 * Duck-types a prop value as a `Reactive<T>` (anything with a `.val`
 * getter). Crucially does NOT require a `.set` — that would exclude
 * `Computed`, which is the bug this helper exists to fix.
 *
 * @template T
 * @param p Prop value, either reactive-wrapped or plain.
 * @returns
 * @internal
 */
export function isReactive<T>(p: Reactive<T> | T): p is Reactive<T> {
  return typeof p === "object" && p !== null && "val" in p;
}

/**
 * Read a reactive-or-plain prop, returning the underlying value.
 *
 * @template T
 * @param p
 * @returns
 * @internal
 */
export function read<T>(p: Reactive<T> | T): T {
  return isReactive(p) ? p.val : p;
}

/**
 * Wrap `fn` so that successive invocations within `ms` reset a
 * trailing-edge timer. Same shape as the framework's
 * `@event.debounce:<ms>` template modifier, lifted into a
 * components-only helper so component implementations can apply
 * it from JS when the modifier route isn't available.
 *
 * Returns the wrapped function unchanged when `ms <= 0` so a
 * caller can pass `props.debounceMs ?? 0` without branching.
 *
 * @template T
 * @param fn Handler to wrap.
 * @param ms Trailing-edge debounce window in milliseconds; `<= 0` is a no-op.
 * @returns The wrapped handler, or `fn` itself when `ms <= 0`.
 * @internal
 */
export function debounce<T extends (...args: any[]) => void>(
  fn: T,
  ms: number,
): T {
  if (!(ms > 0)) return fn;
  let timer: ReturnType<typeof setTimeout> | null = null;
  return ((...args: Parameters<T>) => {
    if (timer != null) clearTimeout(timer);
    timer = setTimeout(() => fn(...args), ms);
  }) as T;
}
