import { html } from "zero";
import type { TemplateResult } from "zero";
import Logo from "./Logo.ts";
import ThemeToggle from "./ThemeToggle.ts";

/**
 * App-level header — built from design-system layout classes (cluster,
 * pad-md, gap-md) and tokens, not from a shipped component. The
 * framework does not ship a `Header` because brand and contents are
 * inherently app-specific; this is the worked example of building your
 * own.
 *
 * @returns Template.
 */
export default function Header(): TemplateResult {
  return html`
    <header class="app-header cluster split pad-md gap-md">
      <a class="app-header-brand cluster gap-sm" href="/">
        ${Logo()}
        <span class="app-header-title">counter</span>
      </a>
      ${ThemeToggle()}
    </header>
  `;
}
