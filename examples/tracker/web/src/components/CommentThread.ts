import { html } from "zero";
import type { TemplateResult } from "zero";
import type { Comment } from "../stores/issues.ts";
import { formatDate } from "../lib/format.ts";

export interface CommentThreadProps {
  comments: Comment[];
}

/**
 * CommentThread — renders a flat list of comments. Empty list renders a
 * placeholder line; otherwise a `<ul>` of comment cards. Pure
 * presentation; no mutation paths.
 *
 * @param props
 * @returns Template.
 */
export default function CommentThread(props: CommentThreadProps): TemplateResult {
  if (props.comments.length === 0) {
    return html`<p class="comment-thread-empty">No comments yet.</p>`;
  }
  return html`
    <ul class="comment-thread stack gap-sm">
      ${props.comments.map(
        (c) => html`
          <li class="comment stack gap-xs">
            <div class="comment-head cluster gap-sm">
              <strong>${c.author}</strong>
              <span class="comment-date">${formatDate(c.createdAt)}</span>
            </div>
            <p class="comment-body">${c.body}</p>
          </li>
        `,
      )}
    </ul>
  `;
}
