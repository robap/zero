import { html } from "zero";
import type { Signal, TemplateResult } from "zero";
import { debounce } from "./_internal.ts";

export type CheckboxProps = {
  checked: Signal<boolean>;
  label?: string;
  disabled?: boolean;
  /**
   * Optional debounce window in milliseconds for the `checked` signal
   * write. `0` or omitted means synchronous (current behaviour).
   */
  debounceMs?: number;
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
  // `<input ... />` (self-closing) keeps the following `<span>` as a
  // sibling rather than a child of the input — see the note in Toggle.ts.
  return html`<label class="checkbox"><input type="checkbox" checked=${() => checked.val} disabled=${props.disabled ?? false} @change=${handler} /><span class="checkbox-label">${props.label ?? ""}</span></label>`;
}
