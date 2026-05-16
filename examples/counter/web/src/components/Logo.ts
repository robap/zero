import { html } from "zero";
import type { TemplateResult } from "zero";

/**
 * Logo — counter's brand mark. A stylized "0" glyph in a 24×24 box; the
 * stroke uses `currentColor` so it inherits text color from
 * `data-theme`. No props — the brand is fixed.
 *
 * @returns SVG template.
 */
export default function Logo(): TemplateResult {
  return html`<svg class="app-logo" viewBox="0 0 24 24" width="24" height="24" aria-hidden="true" focusable="false"><circle cx="12" cy="12" r="9" fill="none" stroke="currentColor" stroke-width="2"></circle></svg>`;
}
