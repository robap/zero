import { html } from "zero";
import type { Signal, Computed, TemplateResult } from "zero";

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
 * Monotonic counter backing {@link uniqueId}.
 */
let _idCounter = 0;

/**
 * Generate a document-unique id for ARIA relationships
 * (`aria-describedby` et al.). Module-level counter, collision-safe
 * within a page lifetime.
 *
 * @param prefix Human-readable id prefix, e.g. `"input-error"`.
 * @returns An id of the form `"<prefix>-<n>"`.
 * @internal
 */
export function uniqueId(prefix: string): string {
  _idCounter += 1;
  return `${prefix}-${_idCounter}`;
}

/**
 * Render a form control's error message as a reactive slot. When the
 * error signal holds a message, renders
 * `<small class="text-muted" id=<id> data-field-error>` with the message;
 * when the signal is null (or no signal was passed) renders nothing.
 * Composes the existing `.text-muted` utility — no bespoke CSS.
 *
 * @param error Optional error message signal from the control's props.
 * @param id Element id linked from the control via `aria-describedby`.
 * @returns A reactive template slot tracking the error signal.
 * @internal
 */
export function errorNode(
  error: Signal<string | null> | undefined,
  id: string,
): TemplateResult {
  return html`${() =>
    error && error.val != null
      ? html`<small class="text-muted" id=${id} data-field-error="">${error.val}</small>`
      : html``}`;
}

/**
 * Derive an `aria-invalid` attribute value binding from a control's
 * optional error signal: `"true"` while the signal holds a message,
 * `"false"` otherwise (including when no signal was passed).
 *
 * @param error Optional error message signal from the control's props.
 * @returns A reactive attribute-value function for `aria-invalid`.
 * @internal
 */
export function ariaInvalid(
  error: Signal<string | null> | undefined,
): () => string {
  return () => (error?.val != null ? "true" : "false");
}

/**
 * Derive an `aria-describedby` attribute value binding from a control's
 * optional error signal: the error node's `id` while the signal holds a
 * message, the empty string otherwise.
 *
 * @param error Optional error message signal from the control's props.
 * @param id Id of the error node rendered by {@link errorNode}.
 * @returns A reactive attribute-value function for `aria-describedby`.
 * @internal
 */
export function ariaDescribedBy(
  error: Signal<string | null> | undefined,
  id: string,
): () => string {
  return () => (error?.val != null ? id : "");
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
