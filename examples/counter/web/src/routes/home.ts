import { html, inject } from "zero";
import type { Signal, TemplateResult } from "zero";
import { Button } from "zero/components";

export default function Home(): TemplateResult {
  return html`
    <section class="stack pad-xl align-center">
      <p class="text-body">Count: ${() => inject<Signal<number>>("count").val}</p>
      ${Button({
        children: "Increment",
        onClick: () => inject<Signal<number>>("count").update((n) => n + 1),
      })}
    </section>
  `;
}
