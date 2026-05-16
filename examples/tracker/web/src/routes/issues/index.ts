import { html, inject, computed, each } from "zero";
import type { TemplateResult } from "zero";
import { Card } from "zero/components";
import { Keys } from "../../state.ts";
import { api } from "../../lib/api.ts";
import { setIssues } from "../../stores/issues.ts";
import type { Issue } from "../../stores/issues.ts";
import IssueRow from "../../components/IssueRow.ts";
import IssueFilters from "../../components/IssueFilters.ts";

/**
 * Marks the route as requiring authentication. The `requireAuth` guard
 * (registered in `app.ts`) reads this flag from the merged meta to decide
 * whether to redirect.
 */
export const meta = { protected: true, title: "Issues" } as const;

/**
 * Route data loader. Threads the route-scoped `fetch` through `zero/http`'s
 * `init.fetch` override so the underlying request inherits the navigation
 * abort signal — navigate away mid-load and the fetch is cancelled.
 *
 * @param ctx Loader context from the router.
 * @returns Resolves when the issues store is hydrated.
 */
export async function load(ctx: { fetch: typeof fetch }): Promise<void> {
  const data = await api.get<{ issues: Issue[] }>("/public/data.json", { fetch: ctx.fetch });
  setIssues(data.issues);
}

export interface IssuesIndexProps {
  query: Record<string, string>;
}

/**
 * IssuesIndex — list view. Filter chips drive `query.status`; the visible
 * list is a computed derived from `inject(Keys.Issues)` and the current
 * query.
 *
 * @param props Route props injected by the router.
 * @returns Template.
 */
export default function IssuesIndex(props: IssuesIndexProps): TemplateResult {
  const filter = props.query.status ?? "all";
  const visible = computed<Issue[]>(() => {
    const items = inject(Keys.Issues).val.items;
    if (filter === "all") return items;
    return items.filter((it) => it.status === filter);
  });
  document.title = "Issues · tracker";
  return html`
    <section class="stack pad-lg gap-md">
      ${Card({
        children: html`
          <div class="stack gap-md">
            <h1>Issues</h1>
            ${IssueFilters({ current: filter })}
            <ul class="issues-list stack gap-xs">
              ${each(visible, (issue) => IssueRow({ issue }))}
            </ul>
          </div>
        `,
      })}
    </section>
  `;
}
