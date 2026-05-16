import { describe, it, expect, afterEach } from "zero/test";
import { render, find, cleanup } from "zero/test";
import Badge from "./Badge.ts";

describe("Badge", () => {
  afterEach(cleanup);

  it("renders the default variant and size", () => {
    const el = render(Badge({ children: "Hello" }));
    expect(find(el, ".badge.badge-default.badge-md")).toBeTruthy();
  });

  it("renders the primary variant when requested", () => {
    const el = render(Badge({ variant: "primary", children: "P" }));
    expect(find(el, ".badge-primary")).toBeTruthy();
  });
});
