import { html } from "zero";
import type { Signal, TemplateResult } from "zero";
import { debounce } from "./_internal.ts";

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
  // `<input ... />` (self-closing) keeps the following `<span>` as a
  // sibling rather than a child of the input — see the note in Toggle.ts.
  return html`<label class="radio"><input type="radio" name=${props.name} value=${props.value} checked=${() => props.selected.val === props.value} disabled=${props.disabled ?? false} @change=${handler} /><span class="radio-label">${props.label ?? ""}</span></label>`;
}
