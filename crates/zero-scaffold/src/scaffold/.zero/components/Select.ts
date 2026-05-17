import { html } from "zero";
import type { Signal, TemplateResult } from "zero";

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
  };
  const labelNode: TemplateResult | null = props.label
    ? html`<label class="select-label">${props.label}</label>`
    : null;
  const options = props.options.map(
    (o) =>
      html`<option value=${o.value} selected=${() => props.value.val === o.value}>${o.label}</option>`,
  );
  return html`${labelNode}<select class=${cls} disabled=${props.disabled ?? false} @change=${onChange}>${options}</select>`;
}
