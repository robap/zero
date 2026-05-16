import { describe, it, expect, beforeEach } from "zero/test";
import { issues, setIssues, addComment, updateStatus } from "./issues.ts";
import type { Issue } from "./issues.ts";

function seed(): Issue[] {
  return [
    {
      id: "A",
      title: "first",
      status: "open",
      assignee: "Robin",
      comments: [],
    },
    {
      id: "B",
      title: "second",
      status: "closed",
      assignee: "Sam",
      comments: [{ author: "Sam", body: "ack", createdAt: "2026-04-12T11:42:00Z" }],
    },
  ];
}

describe("issues store", () => {
  beforeEach(() => {
    issues.set({ items: [], loaded: false });
  });

  it("setIssues replaces the list and marks loaded", () => {
    setIssues(seed());
    expect(issues.val.loaded).toBeTruthy();
    expect(issues.val.items.length).toBe(2);
    expect(issues.val.items[0].id).toBe("A");
  });

  it("addComment appends to the matching issue only", () => {
    setIssues(seed());
    addComment("A", { author: "Jo", body: "hello", createdAt: "2026-04-13T10:00:00Z" });
    expect(issues.val.items[0].comments.length).toBe(1);
    expect(issues.val.items[0].comments[0].body).toBe("hello");
    expect(issues.val.items[1].comments.length).toBe(1);
  });

  it("addComment is a no-op when the id is unknown", () => {
    setIssues(seed());
    addComment("UNKNOWN", { author: "X", body: "y", createdAt: "2026-04-13T10:00:00Z" });
    expect(issues.val.items[0].comments.length).toBe(0);
    expect(issues.val.items[1].comments.length).toBe(1);
  });

  it("updateStatus flips the matching issue's status", () => {
    setIssues(seed());
    updateStatus("A", "closed");
    expect(issues.val.items[0].status).toBe("closed");
    expect(issues.val.items[1].status).toBe("closed");
  });
});
