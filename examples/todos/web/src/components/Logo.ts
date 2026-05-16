import { html } from "zero";
import type { TemplateResult } from "zero";

/**
 * Logo — todos' brand mark. A checkmark inside a rounded square (24×24);
 * stroke uses `currentColor` so the mark follows text color.
 *
 * @returns SVG template.
 */
export default function Logo(): TemplateResult {
  return html`<svg class="app-logo" viewBox="0 0 24 24" width="24" height="24" aria-hidden="true" focusable="false"><rect x="3" y="3" width="18" height="18" rx="4" fill="none" stroke="currentColor" stroke-width="2"></rect><path d="M7 12 L11 16 L17 9" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"></path></svg>`;
}
