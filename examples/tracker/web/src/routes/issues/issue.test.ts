import { describe, it, expect, afterEach, beforeEach } from "zero/test";
import { render, find, fire, cleanup, text } from "zero/test";
import IssuePage from "./issue.ts";
import { issues } from "../../stores/issues.ts";
import { Keys } from "../../state.ts";
import type { Issue } from "../../stores/issues.ts";

function seedOne(): Issue {
  return {
    id: "A",
    title: "first",
    status: "open",
    assignee: "Robin",
    comments: [
      { author: "Sam", body: "ack", createdAt: "2026-04-12T11:42:00Z" },
    ],
  };
}

describe("IssuePage", () => {
  beforeEach(() => {
    issues.set({ items: [seedOne()], loaded: true });
  });
  afterEach(() => {
    cleanup();
    issues.set({ items: [], loaded: false });
  });

  it("renders the issue title and comment thread when found", () => {
    const el = render(IssuePage({ params: { id: "A" } }), {
      state: { [Keys.Issues]: issues },
    });
    expect(text(el, "h1")).toBe("first");
    expect(find(el, ".comment-thread")).toBeTruthy();
  });

  it("renders a not-found message when the id is unknown", () => {
    const el = render(IssuePage({ params: { id: "MISSING" } }), {
      state: { [Keys.Issues]: issues },
    });
    expect(text(el, "code")).toBe("MISSING");
  });

  it("posting a comment appends to the store via addComment", () => {
    const el = render(IssuePage({ params: { id: "A" } }), {
      state: { [Keys.Issues]: issues },
    });
    fire(find(el, "textarea")!, "input", { target: { value: "looks good" } });
    fire(find(el, "form.comment-form")!, "submit");
    expect(issues.val.items[0].comments.length).toBe(2);
    expect(issues.val.items[0].comments[1].body).toBe("looks good");
  });

  it("toggling status flips between open and closed via updateStatus", () => {
    const el = render(IssuePage({ params: { id: "A" } }), {
      state: { [Keys.Issues]: issues },
    });
    // The status-toggle button is the first button inside the card. Its
    // label tracks the current status, so we click it once and assert the
    // store updated.
    const buttons = el.querySelectorAll("button");
    // Two buttons inside the card: status toggle, post comment. Status
    // toggle is rendered first.
    fire(buttons[0], "click");
    expect(issues.val.items[0].status).toBe("closed");
  });
});
