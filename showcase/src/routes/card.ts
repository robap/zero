import { html } from "zero";
import type { TemplateResult } from "zero";
import { Card } from "zero/components";

/**
 * @returns
 */
export default function CardRoute(): TemplateResult {
  return html`
    <main class="showcase-page stack pad-xl">
      <h1 class="text-h1">Card</h1>
      <section class="stack gap-md">
        ${Card({ title: "Surface card", children: "Default filled surface." })}
        ${Card({ variant: "outlined", title: "Outlined card", children: "Transparent with a border." })}
        ${Card({ children: "Body-only card (no title)." })}
      </section>
      <a class="showcase-nav-link" href="/">Back</a>
    </main>
  `;
}
