import { html } from "zero";
import type { TemplateResult } from "zero";

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";
export type ButtonSize = "sm" | "md" | "lg";
export type ButtonType = "button" | "submit" | "reset";

export type ButtonProps = {
  variant?: ButtonVariant;
  size?: ButtonSize;
  type?: ButtonType;
  form?: string;
  name?: string;
  value?: string;
  disabled?: boolean;
  loading?: boolean;
  onClick?: (event: Event) => void;
  children?: TemplateResult | string;
};

/**
 * Button — primary interactive element. Supports four variants, three
 * sizes, and the native submit-button surface: `type` (default
 * `"button"`, so a Button never accidentally submits an enclosing form),
 * `form` (associate a button rendered outside its `<form>` by id),
 * `name`, and `value`. A `disabled` boolean suppresses activation, and a
 * `loading` boolean renders a leading `Spinner`-shaped element **and**
 * makes the button non-interactive — it sets the native `disabled`
 * attribute and short-circuits `onClick`, so a "submitting…" button
 * cannot double-submit or re-fire while busy.
 *
 * `type` is always emitted (default `"button"`). `form`/`name`/`value`
 * are emitted only when provided — the template binder removes the
 * attribute for `undefined`. Boolean attributes (`disabled`) are passed
 * as plain `boolean` values to the runtime; the binder normalizes
 * `false`/`null`/`undefined` by removing the attribute and `true` by
 * setting an empty string. No `?attr=${...}` shorthand exists; this is
 * the canonical pattern.
 *
 * @param props
 * @returns
 */
export default function Button(props: ButtonProps = {}): TemplateResult {
  const variant: ButtonVariant = props.variant ?? "primary";
  const size: ButtonSize = props.size ?? "md";
  const type: ButtonType = props.type ?? "button";
  const cls = `button button-${variant} button-${size}`;
  const spinnerCls = `button-spinner spinner spinner-${variant} spinner-sm`;
  const spinner: TemplateResult | null = props.loading
    ? html`<span class=${spinnerCls} role="status" aria-label="loading"></span>`
    : null;
  const inactive = (props.disabled ?? false) || (props.loading ?? false);
  const handler = (e: Event) => {
    if (inactive) return;
    props.onClick?.(e);
  };
  return html`<button class=${cls} type=${type} form=${props.form} name=${props.name} value=${props.value} disabled=${inactive} @click=${handler}>${spinner}${props.children ?? ""}</button>`;
}
