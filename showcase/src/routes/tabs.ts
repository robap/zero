import { html, signal } from "zero";
import type { TemplateResult } from "zero";
import { Tabs } from "zero/components";

/**
 * @returns
 */
export default function TabsRoute(): TemplateResult {
  const active = signal("a");
  return html`
    <main class="showcase-page stack pad-xl">
      <h1 class="text-h1">Tabs</h1>
      ${Tabs({
        active,
        tabs: [
          { id: "a", label: "First" },
          { id: "b", label: "Second" },
          { id: "c", label: "Third" },
        ],
        panels: {
          a: html`<p class="text-body">First panel content.</p>`,
          b: html`<p class="text-body">Second panel content.</p>`,
          c: html`<p class="text-body">Third panel content.</p>`,
        },
      })}
      <a class="showcase-nav-link" href="/">Back</a>
    </main>
  `;
}
