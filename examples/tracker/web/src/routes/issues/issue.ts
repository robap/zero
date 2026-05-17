import { html, signal, inject, computed } from "zero";
import type { TemplateResult } from "zero";
import { Card, Button, TextArea } from "zero/components";
import { Keys } from "../../state.ts";
import { api } from "../../lib/api.ts";
import { setIssues, addComment, updateStatus } from "../../stores/issues.ts";
import type { Issue } from "../../stores/issues.ts";
import CommentThread from "../../components/CommentThread.ts";
import { statusLabel } from "../../lib/format.ts";

export const meta = { protected: true, title: "Issue" } as const;

/**
 * Route data loader. Ensures the issues store is hydrated; the detail page
 * reads its target issue from the store rather than from a load-return
 * value (the framework does not yet pipe `load()` returns into the
 * component — Phase 12 keeps loaders side-effect-only).
 *
 * @param ctx Loader context from the router.
 * @returns Resolves when the issues store is hydrated.
 */
export async function load(ctx: { fetch: typeof fetch }): Promise<void> {
  if (inject(Keys.Issues).val.loaded) return;
  const data = await api.get<{ issues: Issue[] }>("/public/data.json", { fetch: ctx.fetch });
  setIssues(data.issues);
}

export interface IssuePageProps {
  params: { id: string };
}

/**
 * IssuePage — detail view. Reads the target issue via `inject` so the
 * page reacts to mutations (status toggle, comment add) without a
 * re-route. Shows a graceful "not found" state when the id is unknown.
 *
 * @param props Route props.
 * @returns Template.
 */
export default function IssuePage(props: IssuePageProps): TemplateResult {
  const issue = computed<Issue | undefined>(() =>
    inject(Keys.Issues).val.items.find((it) => it.id === props.params.id),
  );
  const draft = signal("");
  const author = signal("you");

  const submitComment = (e: Event) => {
    e.preventDefault();
    const body = draft.val.trim();
    if (!body) return;
    addComment(props.params.id, {
      author: author.val.trim() || "you",
      body,
      createdAt: new Date().toISOString(),
    });
    draft.set("");
  };

  return html`
    <section class="stack pad-lg gap-md">
      ${() => {
        const it = issue.val;
        if (!it) {
          document.title = "Not found · tracker";
          return Card({
            children: html`<p class="text-body">Issue <code class="text-code">${props.params.id}</code> not found.</p>`,
          });
        }
        document.title = `${it.id} · tracker`;
        return Card({
          children: html`
            <div class="stack gap-md">
              <header class="cluster gap-sm">
                <h1 class="text-h1">${it.title}</h1>
                <span class="issue-detail-meta">${it.id} · ${it.assignee} · ${statusLabel(it.status)}</span>
              </header>
              ${Button({
                children: it.status === "open" ? "Close issue" : "Reopen issue",
                variant: "secondary",
                onClick: () => updateStatus(it.id, it.status === "open" ? "closed" : "open"),
              })}
              <h2 class="text-h2">Comments</h2>
              ${CommentThread({ comments: it.comments })}
              <form class="comment-form stack gap-sm" @submit=${submitComment}>
                ${TextArea({ value: draft, placeholder: "Add a comment...", rows: 3 })}
                ${Button({ children: "Post comment" })}
              </form>
            </div>
          `,
        });
      }}
    </section>
  `;
}
