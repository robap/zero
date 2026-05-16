import { html, signal } from "zero";
import type { TemplateResult } from "zero";
import { Checkbox } from "zero/components";

/**
 * @returns
 */
export default function CheckboxRoute(): TemplateResult {
  const subscribe = signal(false);
  const terms = signal(true);
  return html`
    <main class="showcase-page stack pad-xl">
      <h1>Checkbox</h1>
      <section class="stack gap-sm">
        ${Checkbox({ checked: subscribe, label: "Subscribe to updates" })}
        ${Checkbox({ checked: terms, label: "I accept the terms" })}
        ${Checkbox({ checked: signal(false), label: "Disabled", disabled: true })}
      </section>
      <p>Subscribe: ${() => String(subscribe.val)}, Terms: ${() => String(terms.val)}</p>
      <a class="showcase-nav-link" href="/">Back</a>
    </main>
  `;
}
