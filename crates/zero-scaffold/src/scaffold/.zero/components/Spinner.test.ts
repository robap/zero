import { describe, it, expect, afterEach } from "zero/test";
import { render, find, cleanup } from "zero/test";
import Spinner from "./Spinner.ts";

describe("Spinner", () => {
  afterEach(cleanup);

  it("renders the base class with default variant and size", () => {
    const el = render(Spinner());
    expect(find(el, ".spinner.spinner-primary.spinner-md")).toBeTruthy();
  });

  it("sets role=status for screen readers", () => {
    const el = render(Spinner());
    const node = find(el, ".spinner");
    expect(node).toBeTruthy();
    expect(node!.getAttribute("role")).toBe("status");
  });
});
