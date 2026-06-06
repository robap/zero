import { html } from "zero";
import type { Signal, TemplateResult } from "zero";
import {
  ariaDescribedBy,
  ariaInvalid,
  debounce,
  errorNode,
  uniqueId,
} from "./_internal.ts";

export type ToggleProps = {
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
};

/**
 * Toggle — a visual switch wired to a signal. Renders a hidden native
 * checkbox for accessibility + form participation, plus a styled track
 * and thumb. `role="switch"` is set on the input.
 *
 * @param props
 * @returns
 */
export default function Toggle(props: ToggleProps): TemplateResult {
  const checked = props.checked;
  const onChange = () => checked.set(!checked.val);
  const handler = debounce(onChange, props.debounceMs ?? 0);
  // `<input ... />` (self-closing) is required: the template parser does
  // not have a list of void elements, so without the `/>` the following
  // `<span>` siblings would be appended as children of the input in the
  // DOM tree and the browser would refuse to render them.
  // The error node sits after the closing </label>: inside the label,
  // clicking the message would flip the switch.
  const errId = uniqueId("toggle-error");
  return html`<label class="toggle"><input type="checkbox" class="toggle-input" role="switch" checked=${() => checked.val} aria-checked=${() => String(checked.val)} disabled=${props.disabled ?? false} aria-invalid=${ariaInvalid(props.error)} aria-describedby=${ariaDescribedBy(props.error, errId)} @change=${handler} /><span class="toggle-track"><span class="toggle-thumb"></span></span><span class="toggle-label">${props.label ?? ""}</span></label>${errorNode(props.error, errId)}`;
}
