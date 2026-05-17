import { html } from "zero";
import type { TemplateResult } from "zero";

export type CardVariant = "surface" | "outlined";

export type CardProps = {
  variant?: CardVariant;
  title?: string;
  children?: TemplateResult | string;
};

/**
 * Card — a container with optional title. Two variants: `surface`
 * (default, filled) and `outlined` (transparent with a border).
 *
 * @param props
 * @returns
 */
export default function Card(props: CardProps = {}): TemplateResult {
  const variant: CardVariant = props.variant ?? "surface";
  const cls = `card card-${variant}`;
  const header: TemplateResult | null = props.title
    ? html`<h3 class="card-title">${props.title}</h3>`
    : null;
  return html`<section class=${cls}>${header}<div class="card-body">${props.children ?? ""}</div></section>`;
}
