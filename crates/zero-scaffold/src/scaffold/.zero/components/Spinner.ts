import { html } from "zero";
import type { TemplateResult } from "zero";

export type SpinnerVariant = "primary" | "muted";
export type SpinnerSize = "sm" | "md" | "lg";

export type SpinnerProps = {
  variant?: SpinnerVariant;
  size?: SpinnerSize;
  label?: string;
};

/**
 * Spinner — CSS-only rotating ring. The optional `label` is rendered as
 * sr-only text so assistive tech reads a live status. Used internally by
 * `Button` when `loading` is set.
 *
 * @param props
 * @returns
 */
export default function Spinner(props: SpinnerProps = {}): TemplateResult {
  const variant: SpinnerVariant = props.variant ?? "primary";
  const size: SpinnerSize = props.size ?? "md";
  const cls = `spinner spinner-${variant} spinner-${size}`;
  const labelNode: TemplateResult | null = props.label
    ? html`<span class="visually-hidden">${props.label}</span>`
    : null;
  return html`<span class=${cls} role="status">${labelNode}</span>`;
}
