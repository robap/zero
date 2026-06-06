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

export type RadioProps = {
  selected: Signal<string>;
  name: string;
  value: string;
  label?: string;
  disabled?: boolean;
  /**
   * Optional debounce window in milliseconds for the `selected` signal
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
   * Focus the inner `<input type="radio">` (not the wrapper label) after
   * mount.
   */
  autofocus?: boolean;
  /**
   * Additional native attributes applied to the inner
   * `<input type="radio">` after mount. Additive-only: attributes the
   * component renders itself win and the colliding key is skipped — note
   * `name` and `value` are first-class props here, so their `attrs` keys
   * are always skipped. `true` sets an empty attribute, `false` skips the
   * key, numbers are stringified.
   */
  attrs?: NativeAttrs;
};

/**
 * Radio — single radio button in a named group. All radios sharing a
 * `selected` signal form one logical group; selecting one writes its
 * `value` to the signal.
 *
 * @param props
 * @returns
 */
export default function Radio(props: RadioProps): TemplateResult {
  const onChange = () => props.selected.set(props.value);
  const handler = debounce(onChange, props.debounceMs ?? 0);
  const controlRef: Ref<HTMLInputElement> = nativeRef(
    props.attrs,
    props.autofocus,
  );
  const errId = uniqueId("radio-error");
  // `<input ... />` (self-closing) keeps the following `<span>` as a
  // sibling rather than a child of the input — see the note in Toggle.ts.
  // The error node sits after the closing </label>: inside the label,
  // clicking the message would select the control.
  return html`<label class="radio"><input ref=${controlRef} type="radio" name=${props.name} value=${props.value} checked=${() => props.selected.val === props.value} disabled=${props.disabled ?? false} aria-invalid=${ariaInvalid(props.error)} aria-describedby=${ariaDescribedBy(props.error, errId)} @change=${handler} /><span class="radio-label">${props.label ?? ""}</span></label>${errorNode(props.error, errId)}`;
}
