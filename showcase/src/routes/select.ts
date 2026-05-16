import { html, signal } from "zero";
import type { TemplateResult } from "zero";
import { Select } from "zero/components";

/**
 * @returns
 */
export default function SelectRoute(): TemplateResult {
  const choice = signal("apple");
  return html`
    <main class="showcase-page stack pad-xl">
      <h1>Select</h1>
      <section class="stack gap-sm">
        ${Select({
          value: choice,
          label: "Fruit",
          options: [
            { value: "apple", label: "Apple" },
            { value: "banana", label: "Banana" },
            { value: "cherry", label: "Cherry" },
          ],
        })}
      </section>
      <p>Choice: ${() => choice.val}</p>
      <a class="showcase-nav-link" href="/">Back</a>
    </main>
  `;
}
