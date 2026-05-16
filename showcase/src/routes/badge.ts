import { html } from "zero";
import type { TemplateResult } from "zero";
import { Badge } from "zero/components";

/**
 * @returns
 */
export default function BadgeRoute(): TemplateResult {
  return html`
    <main class="showcase-page stack pad-xl">
      <h1>Badge</h1>
      <section class="cluster gap-md align-center">
        ${Badge({ children: "Default" })}
        ${Badge({ variant: "primary", children: "Primary" })}
        ${Badge({ variant: "success", children: "Success" })}
        ${Badge({ variant: "warning", children: "Warning" })}
        ${Badge({ variant: "danger", children: "Danger" })}
      </section>
      <section class="cluster gap-md align-center">
        ${Badge({ size: "sm", children: "Small" })}
        ${Badge({ size: "md", children: "Medium" })}
      </section>
      <a class="showcase-nav-link" href="/">Back</a>
    </main>
  `;
}
