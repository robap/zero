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

export type CheckboxProps = {
  checked: Signal<boolean>;
  label?: string;
  disabled?: boolean;
  /**
   * Optional debounce window in milliseconds for the `checked` signal
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
   * Focus the inner `<input type="checkbox">` (not the wrapper label)
   * after mount.
   */
  autofocus?: boolean;
  /**
   * Additional native attributes applied to the inner
   * `<input type="checkbox">` after mount. Additive-only: attributes the
   * component renders itself (`type`, `checked`, …) win and the colliding
   * key is skipped. `true` sets an empty attribute, `false` skips the key,
   * numbers are stringified.
   */
  attrs?: NativeAttrs;
};

/**
 * Checkbox — a native `<input type="checkbox">` wired to a signal.
 * Parent owns the signal; the component reads `.val` and writes via
 * `.set()` on change.
 *
 * @param props
 * @returns
 */
export default function Checkbox(props: CheckboxProps): TemplateResult {
  const checked = props.checked;
  const onChange = () => checked.set(!checked.val);
  const handler = debounce(onChange, props.debounceMs ?? 0);
  const controlRef: Ref<HTMLInputElement> = nativeRef(
    props.attrs,
    props.autofocus,
  );
  const errId = uniqueId("checkbox-error");
  // `<input ... />` (self-closing) keeps the following `<span>` as a
  // sibling rather than a child of the input — see the note in Toggle.ts.
  // The error node sits after the closing </label>: inside the label,
  // clicking the message would toggle the control.
  return html`<label class="checkbox"><input ref=${controlRef} type="checkbox" checked=${() => checked.val} disabled=${props.disabled ?? false} aria-invalid=${ariaInvalid(props.error)} aria-describedby=${ariaDescribedBy(props.error, errId)} @change=${handler} /><span class="checkbox-label">${props.label ?? ""}</span></label>${errorNode(props.error, errId)}`;
}
