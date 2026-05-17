import { html, signal } from "zero";
import type { TemplateResult } from "zero";
import { Radio } from "zero/components";

/**
 * @returns
 */
export default function RadioRoute(): TemplateResult {
  const selected = signal("a");
  return html`
    <main class="showcase-page stack pad-xl">
      <h1 class="text-h1">Radio</h1>
      <section class="stack gap-sm">
        ${Radio({ selected, name: "group", value: "a", label: "Option A" })}
        ${Radio({ selected, name: "group", value: "b", label: "Option B" })}
        ${Radio({ selected, name: "group", value: "c", label: "Option C" })}
      </section>
      <p class="text-body">Selected: ${() => selected.val}</p>
      <a class="showcase-nav-link" href="/">Back</a>
    </main>
  `;
}
