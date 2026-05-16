import { describe, it, expect, afterEach, beforeEach } from "zero/test";
import { render, find, findAll, cleanup } from "zero/test";
import IssuesIndex from "./index.ts";
import { issues } from "../../stores/issues.ts";
import { Keys } from "../../state.ts";
import type { Issue } from "../../stores/issues.ts";

function seed(): Issue[] {
  return [
    { id: "A", title: "first", status: "open", assignee: "Robin", comments: [] },
    { id: "B", title: "second", status: "closed", assignee: "Sam", comments: [] },
    { id: "C", title: "third", status: "open", assignee: "Jo", comments: [] },
  ];
}

describe("IssuesIndex", () => {
  beforeEach(() => {
    issues.set({ items: seed(), loaded: true });
  });
  afterEach(() => {
    cleanup();
    issues.set({ items: [], loaded: false });
  });

  it("renders every issue when no filter is active", () => {
    const el = render(IssuesIndex({ query: {} }), { state: { [Keys.Issues]: issues } });
    expect(findAll(el, "li.issue-row").length).toBe(3);
  });

  it("renders only open issues when status=open", () => {
    const el = render(IssuesIndex({ query: { status: "open" } }), {
      state: { [Keys.Issues]: issues },
    });
    const rows = findAll(el, "li.issue-row");
    expect(rows.length).toBe(2);
  });

  it("renders only closed issues when status=closed", () => {
    const el = render(IssuesIndex({ query: { status: "closed" } }), {
      state: { [Keys.Issues]: issues },
    });
    const rows = findAll(el, "li.issue-row");
    expect(rows.length).toBe(1);
  });

  it("each row links to its detail page", () => {
    const el = render(IssuesIndex({ query: {} }), { state: { [Keys.Issues]: issues } });
    const links = findAll(el, "a.issue-row-title");
    expect(links.length).toBe(3);
    expect(links[0].getAttribute("href")).toBe("/issues/A");
  });

  it("renders the filter bar", () => {
    const el = render(IssuesIndex({ query: {} }), { state: { [Keys.Issues]: issues } });
    const bar = find(el, ".issue-filters");
    expect(bar).toBeTruthy();
    expect(findAll(bar!, "button").length).toBe(3);
  });
});
