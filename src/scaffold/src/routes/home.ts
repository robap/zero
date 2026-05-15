import { html, inject } from "zero";
import type { Signal, TemplateResult } from "zero";

function Counter(): TemplateResult {
  return html`<p>Count: ${() => inject<Signal<number>>("count").val}</p>`;
}

export default function Home(): TemplateResult {
  return html`
    <main class="stack pad-xl">
      <h1>Hello from zero</h1>
      <div class="cluster gap-md">
        <button class="pad-sm border" @click=${() => inject<Signal<number>>("count").update(n => n + 1)}>Increment</button>
        ${Counter()}
      </div>
    </main>
  `;
}
