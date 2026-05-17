import { html, signal } from "zero";
import type { TemplateResult } from "zero";
import { Toast, Button } from "zero/components";

/**
 * @returns
 */
export default function ToastRoute(): TemplateResult {
  const open = signal(false);
  return html`
    <main class="showcase-page stack pad-xl">
      <h1 class="text-h1">Toast</h1>
      <section class="cluster gap-md">
        ${Button({ onClick: () => open.set(true), children: "Show toast" })}
        ${Button({ variant: "secondary", onClick: () => open.set(false), children: "Hide toast" })}
      </section>
      ${Toast({ open, message: "Saved successfully", variant: "success" })}
      <a class="showcase-nav-link" href="/">Back</a>
    </main>
  `;
}
