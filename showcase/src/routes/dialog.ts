import { html, signal } from "zero";
import type { TemplateResult } from "zero";
import { Dialog, Button } from "zero/components";

/**
 * @returns
 */
export default function DialogRoute(): TemplateResult {
  const open = signal(false);
  const openTall = signal(false);
  const tallParagraphs: TemplateResult[] = [];
  for (let i = 1; i <= 30; i++) {
    tallParagraphs.push(html`<p class="text-body">Paragraph ${String(i)} — the dialog body should scroll internally instead of growing past the viewport.</p>`);
  }
  return html`
    <main class="showcase-page stack pad-xl">
      <h1 class="text-h1">Dialog</h1>
      <section class="cluster gap-md">
        ${Button({ onClick: () => open.set(true), children: "Open dialog" })}
        ${Button({ onClick: () => openTall.set(true), children: "Open tall dialog" })}
      </section>
      ${Dialog({
        open,
        title: "Hello",
        children: html`<p class="text-body">Press Escape, click the backdrop, or use the close button.</p>`,
      })}
      ${Dialog({
        open: openTall,
        title: "Tall content",
        children: html`${tallParagraphs}`,
      })}
      <a class="showcase-nav-link" href="/">Back</a>
    </main>
  `;
}
