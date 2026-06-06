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

export type SelectSize = "sm" | "md" | "lg";

export type SelectOption = {
  value: string;
  label: string;
};

export type SelectProps = {
  value: Signal<string>;
  options: SelectOption[];
  size?: SelectSize;
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
  /**
   * Optional error message signal; when non-null the control renders the
   * message below itself, sets `aria-invalid`, and links the message via
   * `aria-describedby`.
   */
  error?: Signal<string | null>;
  /**
   * Focus the underlying `<select>` after mount (e.g. the first field of a
   * drawer/dialog form).
   */
  autofocus?: boolean;
  /**
   * Additional native attributes applied to the underlying `<select>` after
   * mount. Additive-only: attributes the component renders itself (`class`,
   * …) win and the colliding key is skipped. `true` sets an empty
   * attribute, `false` skips the key, numbers are stringified.
   */
  attrs?: NativeAttrs;
};

/**
 * Select — native `<select>` wired to a signal. The native arrow is not
 * masked. Options are rendered from a `{value, label}[]` prop.
 *
 * @param props
 * @returns
 */
export default function Select(props: SelectProps): TemplateResult {
  const size: SelectSize = props.size ?? "md";
  const cls = `select select-${size}`;
  const onChange = (e: Event) => {
    const target = e.target as HTMLSelectElement;
    props.value.set(target.value);
    props.onChange?.(target.value);
  };
  const handler = debounce(onChange, props.debounceMs ?? 0);
  const labelNode: TemplateResult | null = props.label
    ? html`<label class="select-label">${props.label}</label>`
    : null;
  const options = props.options.map(
    (o) =>
      html`<option value=${o.value} selected=${() => props.value.val === o.value}>${o.label}</option>`,
  );
  const controlRef: Ref<HTMLSelectElement> = nativeRef(
    props.attrs,
    props.autofocus,
  );
  const errId = uniqueId("select-error");
  return html`${labelNode}<select ref=${controlRef} class=${cls} disabled=${props.disabled ?? false} aria-invalid=${ariaInvalid(props.error)} aria-describedby=${ariaDescribedBy(props.error, errId)} @change=${handler}>${options}</select>${errorNode(props.error, errId)}`;
}
