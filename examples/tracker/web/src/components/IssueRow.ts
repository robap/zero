import { html } from "zero";
import type { TemplateResult } from "zero";
import { Badge } from "zero/components";
import type { Issue } from "../stores/issues.ts";
import { statusLabel } from "../lib/format.ts";

export interface IssueRowProps {
  issue: Issue;
}

/**
 * IssueRow — one row in the issue list. Linked to the detail page; status
 * pill uses the shipped `Badge` component to demonstrate the
 * primitives-only rule.
 *
 * @param props
 * @returns Template.
 */
export default function IssueRow(props: IssueRowProps): TemplateResult {
  const { issue } = props;
  return html`
    <li class="issue-row cluster gap-md pad-sm" data-id=${issue.id}>
      ${Badge({
        children: statusLabel(issue.status),
        variant: issue.status === "open" ? "primary" : "default",
      })}
      <a class="issue-row-title" href=${`/issues/${issue.id}`}>${issue.title}</a>
      <span class="issue-row-meta">${issue.id} · ${issue.assignee}</span>
    </li>
  `;
}
