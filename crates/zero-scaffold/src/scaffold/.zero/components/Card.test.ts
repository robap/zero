import { describe, it, expect, afterEach } from "zero/test";
import { render, find, cleanup } from "zero/test";
import Card from "./Card.ts";

describe("Card", () => {
  afterEach(cleanup);

  it("renders the base class", () => {
    const el = render(Card({ children: "Body" }));
    expect(find(el, ".card")).toBeTruthy();
  });

  it("renders a title when provided", () => {
    const el = render(Card({ title: "Heading", children: "Body" }));
    expect(find(el, ".card-title")).toBeTruthy();
  });

  it("omits the title element when not provided", () => {
    const el = render(Card({ children: "Body only" }));
    expect(find(el, ".card-title")).toBeNull();
  });
});
