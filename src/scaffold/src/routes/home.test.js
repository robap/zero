import { describe, it, expect } from "zero/test";
import { render, find, text, fire, cleanup } from "zero/test";
import { signal } from "zero";
import Home from "./home.js";

describe("Home", () => {
  afterEach(cleanup);

  it("renders the initial count", () => {
    const el = render(Home(), {
      state: { count: signal(0) },
    });
    expect(text(el, "p")).toBe("Count: 0");
  });

  it("increments the count when the button is clicked", () => {
    const count = signal(0);
    const el = render(Home(), { state: { count } });

    fire(find(el, "button"), "click");
    expect(text(el, "p")).toBe("Count: 1");

    fire(find(el, "button"), "click");
    expect(text(el, "p")).toBe("Count: 2");
    expect(count.val).toBe(2);
  });
});
