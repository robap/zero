import { html } from "zero";
import type { Signal, TemplateResult } from "zero";

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
  };
  const labelNode: TemplateResult | null = props.label
    ? html`<label class="input-label">${props.label}</label>`
    : null;
  return html`${labelNode}<input class=${cls} type=${type} value=${() => props.value.val} placeholder=${props.placeholder ?? ""} disabled=${props.disabled ?? false} @input=${onInput}>`;
}
