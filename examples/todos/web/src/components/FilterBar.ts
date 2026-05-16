import { html, inject } from "zero";
import type { TemplateResult } from "zero";
import { Keys } from "../state.ts";
import { setFilter } from "../stores/todos.ts";
import type { Filter } from "../stores/todos.ts";

const ALL: Filter[] = ["all", "active", "done"];

// FilterBar uses the design-system button classes directly (rather than the
// `Button` component) so the active variant tracks the filter signal
// reactively. This mirrors the pattern Tabs uses internally — its tab
// buttons are inline `<button>`s with reactive `aria-selected` bindings.
export default function FilterBar(): TemplateResult {
  return html`
    <nav class="filter-bar cluster gap-sm">
      ${ALL.map(
        (f) => html`
          <button
            class=${() => {
              const active = inject(Keys.Todos).val.filter === f;
              return `button button-${active ? "primary" : "secondary"} button-sm`;
            }}
            aria-pressed=${() => inject(Keys.Todos).val.filter === f}
            @click=${() => setFilter(f)}
          >${f}</button>
        `,
      )}
    </nav>
  `;
}
