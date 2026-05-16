import { html } from "zero";
import type { TemplateResult } from "zero";

export type AvatarSize = "sm" | "md" | "lg" | "xl";

export type AvatarProps = {
  src?: string;
  alt: string;
  initials?: string;
  size?: AvatarSize;
};

/**
 * Avatar — displays a user image when `src` is set, or a colored circle
 * containing initials otherwise. `initials` falls back to the first
 * character of `alt`, uppercased.
 *
 * @param props
 * @returns
 */
export default function Avatar(props: AvatarProps): TemplateResult {
  const size: AvatarSize = props.size ?? "md";
  if (props.src) {
    const cls = `avatar avatar-${size}`;
    return html`<img class=${cls} src=${props.src} alt=${props.alt}>`;
  }
  const cls = `avatar avatar-${size} avatar-initials`;
  const fallback = props.initials ?? (props.alt[0]?.toUpperCase() ?? "");
  return html`<span class=${cls} aria-label=${props.alt}>${fallback}</span>`;
}
