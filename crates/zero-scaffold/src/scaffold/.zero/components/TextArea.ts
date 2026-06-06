import { html } from "zero";
import type { Signal, TemplateResult } from "zero";
import {
  ariaDescribedBy,
  ariaInvalid,
  debounce,
  errorNode,
  uniqueId,
} from "./_internal.ts";

export type TextAreaProps = {
  value: Signal<string>;
  rows?: number;
  placeholder?: string;
  disabled?: boolean;
  label?: string;
  /**
   * Optional debounce window in milliseconds for the `value` signal
   * write. `0` or omitted means synchronous (current behaviour).
   */
  debounceMs?: number;
  /**
   * Optional error message signal; when non-null the control renders the
   * message below itself, sets `aria-invalid`, and links the message via
   * `aria-describedby`.
   */
  error?: Signal<string | null>;
};

/**
 * TextArea — multi-line text input wired to a signal. Same shape as
 * `Input` but renders a `<textarea>`.
 *
 * @param props
 * @returns
 */
export default function TextArea(props: TextAreaProps): TemplateResult {
  const onInput = (e: Event) => {
    const target = e.target as HTMLTextAreaElement;
    props.value.set(target.value);
  };
  const handler = debounce(onInput, props.debounceMs ?? 0);
  const labelNode: TemplateResult | null = props.label
    ? html`<label class="textarea-label">${props.label}</label>`
    : null;
  const errId = uniqueId("textarea-error");
  return html`${labelNode}<textarea class="textarea" rows=${props.rows ?? 4} placeholder=${props.placeholder ?? ""} disabled=${props.disabled ?? false} aria-invalid=${ariaInvalid(props.error)} aria-describedby=${ariaDescribedBy(props.error, errId)} @input=${handler}>${() => props.value.val}</textarea>${errorNode(props.error, errId)}`;
}
