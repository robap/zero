import { html } from "zero";
import type { Ref, Signal, TemplateResult } from "zero";
import {
  ariaDescribedBy,
  ariaInvalid,
  debounce,
  errorNode,
  nativeRef,
  uniqueId,
  type NativeAttrs,
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
  /**
   * Focus the underlying `<textarea>` after mount (e.g. the first field of
   * a drawer/dialog form).
   */
  autofocus?: boolean;
  /**
   * Additional native attributes applied to the underlying `<textarea>`
   * after mount. Additive-only: attributes the component renders itself
   * (`class`, `rows`, …) win and the colliding key is skipped. `true` sets
   * an empty attribute, `false` skips the key, numbers are stringified.
   */
  attrs?: NativeAttrs;
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
  const controlRef: Ref<HTMLTextAreaElement> = nativeRef(
    props.attrs,
    props.autofocus,
  );
  const labelNode: TemplateResult | null = props.label
    ? html`<label class="textarea-label">${props.label}</label>`
    : null;
  const errId = uniqueId("textarea-error");
  return html`${labelNode}<textarea ref=${controlRef} class="textarea" rows=${props.rows ?? 4} placeholder=${props.placeholder ?? ""} disabled=${props.disabled ?? false} aria-invalid=${ariaInvalid(props.error)} aria-describedby=${ariaDescribedBy(props.error, errId)} @input=${handler}>${() => props.value.val}</textarea>${errorNode(props.error, errId)}`;
}
