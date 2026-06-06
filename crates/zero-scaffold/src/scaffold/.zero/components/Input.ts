import { html } from "zero";
import type { Signal, TemplateResult } from "zero";
import { debounce } from "./_internal.ts";

export type InputType =
  | "text"
  | "email"
  | "password"
  | "number"
  | "search"
  | "url"
  | "tel";
export type InputSize = "sm" | "md" | "lg";

export type InputProps = {
  value: Signal<string>;
  type?: InputType;
  size?: InputSize;
  placeholder?: string;
  disabled?: boolean;
  label?: string;
  /**
   * Optional debounce window in milliseconds for the `value` signal
   * write. `0` or omitted means synchronous (current behaviour).
   */
  debounceMs?: number;
  /**
   * Optional callback invoked with the new value after each user edit
   * (after the `value` signal write, inside the same debounce window).
   * Use this to react to edits directly instead of bridging the signal
   * with an `effect`.
   */
  onChange?: (value: string) => void;
};

/**
 * Input — single-line text input wired to a signal. The signal owns the
 * value; the component reads `.val` reactively and writes via `.set` on
 * every input event. Supports a labeled and unlabeled form.
 *
 * @param props
 * @returns
 */
export default function Input(props: InputProps): TemplateResult {
  const type: InputType = props.type ?? "text";
  const size: InputSize = props.size ?? "md";
  const cls = `input input-${size}`;
  const onInput = (e: Event) => {
    const target = e.target as HTMLInputElement;
    props.value.set(target.value);
    props.onChange?.(target.value);
  };
  const handler = debounce(onInput, props.debounceMs ?? 0);
  const labelNode: TemplateResult | null = props.label
    ? html`<label class="input-label">${props.label}</label>`
    : null;
  return html`${labelNode}<input class=${cls} type=${type} value=${() => props.value.val} placeholder=${props.placeholder ?? ""} disabled=${props.disabled ?? false} @input=${handler}>`;
}
