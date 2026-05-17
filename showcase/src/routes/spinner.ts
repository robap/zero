import { html } from "zero";
import type { TemplateResult } from "zero";
import { Spinner } from "zero/components";

/**
 * @returns
 */
export default function SpinnerRoute(): TemplateResult {
  return html`
    <main class="showcase-page stack pad-xl">
      <h1 class="text-h1">Spinner</h1>
      <section class="cluster gap-md align-center">
        ${Spinner({ size: "sm" })}
        ${Spinner({ size: "md" })}
        ${Spinner({ size: "lg" })}
      </section>
      <section class="cluster gap-md align-center">
        ${Spinner({ variant: "primary" })}
        ${Spinner({ variant: "muted" })}
      </section>
      <a class="showcase-nav-link" href="/">Back</a>
    </main>
  `;
}
