import { html } from "zero";
import type { TemplateResult } from "zero";
import { Avatar } from "zero/components";

/**
 * @returns
 */
export default function AvatarRoute(): TemplateResult {
  return html`
    <main class="showcase-page stack pad-xl">
      <h1>Avatar</h1>
      <section class="cluster gap-md align-center">
        ${Avatar({ alt: "Ada Lovelace", size: "sm" })}
        ${Avatar({ alt: "Ada Lovelace", size: "md" })}
        ${Avatar({ alt: "Ada Lovelace", size: "lg" })}
        ${Avatar({ alt: "Ada Lovelace", size: "xl" })}
      </section>
      <section class="cluster gap-md align-center">
        ${Avatar({ alt: "Grace Hopper", initials: "GH" })}
        ${Avatar({ alt: "Margaret Hamilton", initials: "MH", size: "lg" })}
      </section>
      <a class="showcase-nav-link" href="/">Back</a>
    </main>
  `;
}
