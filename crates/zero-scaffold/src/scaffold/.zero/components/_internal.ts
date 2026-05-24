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
