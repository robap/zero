import { html, signal } from "zero";
import type { TemplateResult } from "zero";
import { Dialog, Button } from "zero/components";

/**
 * @returns
 */
export default function DialogRoute(): TemplateResult {
  const open = signal(false);
  return html`
    <main class="showcase-page stack pad-xl">
      <h1 class="text-h1">Dialog</h1>
      <section class="cluster gap-md">
        ${Button({ onClick: () => open.set(true), children: "Open dialog" })}
      </section>
      ${Dialog({
        open,
        title: "Hello",
        children: html`<p class="text-body">Press Escape, click the backdrop, or use the close button.</p>`,
      })}
      <a class="showcase-nav-link" href="/">Back</a>
    </main>
  `;
}
