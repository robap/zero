import { html } from "zero";
import type { TemplateResult } from "zero";

export interface DashboardLayoutProps {
  outlet: TemplateResult;
}

/**
 * DashboardLayout — nested-route layout for the protected dashboard. Wraps
 * `outlet` (the child route's TemplateResult) with a sidebar nav. The
 * global header lives outside this layout (in the root `app.layout`) so
 * brand + theme controls persist across login → dashboard transitions.
 *
 * @param props
 * @returns Template.
 */
export default function DashboardLayout(props: DashboardLayoutProps): TemplateResult {
  return html`
    <div class="dashboard-shell cluster">
      <aside class="dashboard-nav stack gap-sm pad-md">
        <a href="/issues">Issues</a>
      </aside>
      <section class="dashboard-content pad-lg">${props.outlet}</section>
    </div>
  `;
}
