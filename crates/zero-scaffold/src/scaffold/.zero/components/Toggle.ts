import { html } from "zero";
import type { Signal, TemplateResult } from "zero";

export type ToggleProps = {
  checked: Signal<boolean>;
  label?: string;
  disabled?: boolean;
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
  // `<input ... />` (self-closing) is required: the template parser does
  // not have a list of void elements, so without the `/>` the following
  // `<span>` siblings would be appended as children of the input in the
  // DOM tree and the browser would refuse to render them.
  return html`<label class="toggle"><input type="checkbox" class="toggle-input" role="switch" checked=${() => checked.val} aria-checked=${() => String(checked.val)} disabled=${props.disabled ?? false} @change=${onChange} /><span class="toggle-track"><span class="toggle-thumb"></span></span><span class="toggle-label">${props.label ?? ""}</span></label>`;
}
