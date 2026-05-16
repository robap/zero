import { html } from "zero";
import type { TemplateResult } from "zero";

/**
 * Logo — tracker's brand mark. A stylized ticket glyph (24×24); stroke uses
 * `currentColor` so the mark follows text color.
 *
 * @returns SVG template.
 */
export default function Logo(): TemplateResult {
  return html`<svg class="app-logo" viewBox="0 0 24 24" width="24" height="24" aria-hidden="true" focusable="false"><path d="M3 8a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2v2a2 2 0 0 0 0 4v2a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-2a2 2 0 0 0 0-4z" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"></path><path d="M13 7v10" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-dasharray="2 2"></path></svg>`;
}
