import { html } from "zero";
import type { TemplateResult } from "zero";

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";
export type ButtonSize = "sm" | "md" | "lg";

export type ButtonProps = {
  variant?: ButtonVariant;
  size?: ButtonSize;
  disabled?: boolean;
  loading?: boolean;
  onClick?: (event: Event) => void;
  children?: TemplateResult | string;
};

/**
 * Button — primary interactive element. Supports four variants, three
 * sizes, a `disabled` boolean (suppresses click), and a `loading` boolean
 * that renders a leading `Spinner`-shaped element.
 *
 * Boolean attributes (`disabled`) are passed as plain `boolean` values to
 * the runtime; the template binder normalizes `false`/`null`/`undefined`
 * by removing the attribute and `true` by setting an empty string. No
 * `?attr=${...}` shorthand exists; this is the canonical pattern.
 *
 * @param props
 * @returns
 */
export default function Button(props: ButtonProps = {}): TemplateResult {
  const variant: ButtonVariant = props.variant ?? "primary";
  const size: ButtonSize = props.size ?? "md";
  const cls = `button button-${variant} button-${size}`;
  const spinnerCls = `button-spinner spinner spinner-${variant} spinner-sm`;
  const spinner: TemplateResult | null = props.loading
    ? html`<span class=${spinnerCls} role="status" aria-label="loading"></span>`
    : null;
  const handler = (e: Event) => {
    if (props.disabled) return;
    props.onClick?.(e);
  };
  return html`<button class=${cls} disabled=${props.disabled ?? false} @click=${handler}>${spinner}${props.children ?? ""}</button>`;
}
