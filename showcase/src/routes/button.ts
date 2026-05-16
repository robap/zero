import { html, signal } from "zero";
import type { TemplateResult } from "zero";
import { Button } from "zero/components";

/**
 * @returns
 */
export default function ButtonRoute(): TemplateResult {
  const clicks = signal(0);
  return html`
    <main class="showcase-page stack pad-xl">
      <h1>Button</h1>
      <section class="cluster gap-md">
        ${Button({ variant: "primary", children: "Primary" })}
        ${Button({ variant: "secondary", children: "Secondary" })}
        ${Button({ variant: "ghost", children: "Ghost" })}
        ${Button({ variant: "danger", children: "Danger" })}
      </section>
      <section class="cluster gap-md align-center">
        ${Button({ size: "sm", children: "Small" })}
        ${Button({ size: "md", children: "Medium" })}
        ${Button({ size: "lg", children: "Large" })}
      </section>
      <section class="cluster gap-md">
        ${Button({ disabled: true, children: "Disabled" })}
        ${Button({ loading: true, children: "Loading" })}
      </section>
      <section class="stack gap-sm">
        ${Button({ onClick: () => clicks.update((n) => n + 1), children: "Click me" })}
        <p>Clicks: ${clicks}</p>
      </section>
      <a class="showcase-nav-link" href="/">Back</a>
    </main>
  `;
}
