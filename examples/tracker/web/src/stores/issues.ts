// stores/issues.ts — issues domain state and mutators. Pure store
// semantics: nothing here knows about HTTP, redirects, or the wire
// format. The list is hydrated by route `load()`s calling
// `setIssues(...)`; mutators handle in-memory updates.

import { signal } from "zero";
import type { Signal } from "zero";

export type IssueStatus = "open" | "closed";

export interface Comment {
  author: string;
  body: string;
  createdAt: string;
}

export interface Issue {
  id: string;
  title: string;
  status: IssueStatus;
  assignee: string;
  comments: Comment[];
}

export interface IssuesState {
  items: Issue[];
  loaded: boolean;
}

export const issues: Signal<IssuesState> = signal<IssuesState>({ items: [], loaded: false });

/** Replace the entire list (the canonical post-fetch hydration). */
export function setIssues(items: Issue[]): void {
  issues.set({ items, loaded: true });
}

/** Append a comment to a specific issue. No-op if the id doesn't match. */
export function addComment(id: string, comment: Comment): void {
  issues.update((s) => ({
    ...s,
    items: s.items.map((it) =>
      it.id === id ? { ...it, comments: [...it.comments, comment] } : it,
    ),
  }));
}

/** Flip an issue's status. No-op if the id doesn't match. */
export function updateStatus(id: string, status: IssueStatus): void {
  issues.update((s) => ({
    ...s,
    items: s.items.map((it) => (it.id === id ? { ...it, status } : it)),
  }));
}
