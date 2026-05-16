import { html, signal } from "zero";
import type { TemplateResult } from "zero";
import { Toggle } from "zero/components";

/**
 * @returns
 */
export default function ToggleRoute(): TemplateResult {
  const wifi = signal(false);
  const notifications = signal(true);
  return html`
    <main class="showcase-page stack pad-xl">
      <h1>Toggle</h1>
      <section class="stack gap-sm">
        ${Toggle({ checked: wifi, label: "Wi-Fi" })}
        ${Toggle({ checked: notifications, label: "Notifications" })}
        ${Toggle({ checked: signal(false), label: "Disabled", disabled: true })}
      </section>
      <p>Wi-Fi: ${() => String(wifi.val)}, Notifications: ${() => String(notifications.val)}</p>
      <a class="showcase-nav-link" href="/">Back</a>
    </main>
  `;
}
