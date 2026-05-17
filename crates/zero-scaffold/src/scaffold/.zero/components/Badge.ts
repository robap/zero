import { html } from "zero";
import type { TemplateResult } from "zero";

export type BadgeVariant = "default" | "primary" | "success" | "warning" | "danger";
export type BadgeSize = "sm" | "md";

export type BadgeProps = {
  variant?: BadgeVariant;
  size?: BadgeSize;
  children?: TemplateResult | string;
};

/**
 * Badge — small inline label. Supports five variants and two sizes.
 *
 * @param props
 * @returns
 */
export default function Badge(props: BadgeProps = {}): TemplateResult {
  const variant: BadgeVariant = props.variant ?? "default";
  const size: BadgeSize = props.size ?? "md";
  const cls = `badge badge-${variant} badge-${size}`;
  return html`<span class=${cls}>${props.children ?? ""}</span>`;
}
