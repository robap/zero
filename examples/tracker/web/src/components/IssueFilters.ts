import { html, navigate } from "zero";
import type { TemplateResult } from "zero";

export interface IssueFiltersProps {
  current: string;
}

const FILTERS: { value: string; label: string }[] = [
  { value: "all", label: "All" },
  { value: "open", label: "Open" },
  { value: "closed", label: "Closed" },
];

/**
 * IssueFilters — status filter chips. Active state is derived from the
 * route's `?status=` query param; clicks update the URL via `navigate`.
 * No local filter signal — the URL is the source of truth, so the back
 * button works as expected and the filter survives a hard reload.
 *
 * The buttons are raw `<button>`s rather than the shipped `Button`
 * component because `Button`'s `variant` prop is not reactive, and the
 * active variant needs to track `props.current` per render. Same pattern
 * as `examples/todos/src/components/FilterBar.ts`.
 *
 * @param props
 * @returns Template.
 */
export default function IssueFilters(props: IssueFiltersProps): TemplateResult {
  const apply = (value: string) => () => {
    const target = value === "all" ? "/issues" : `/issues?status=${value}`;
    navigate(target);
  };
  return html`
    <nav class="issue-filters cluster gap-sm" aria-label="Filter issues">
      ${FILTERS.map(
        (f) => html`
          <button
            class=${`button button-${props.current === f.value ? "primary" : "secondary"} button-sm`}
            aria-pressed=${props.current === f.value}
            @click=${apply(f.value)}
          >${f.label}</button>
        `,
      )}
    </nav>
  `;
}
