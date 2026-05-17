import { describe, it, expect, afterEach } from "zero/test";
import { render, find, fire, cleanup } from "zero/test";
import { signal } from "zero";
import Input from "./Input.ts";

describe("Input", () => {
  afterEach(cleanup);

  it("renders the base class", () => {
    const value = signal("");
    const el = render(Input({ value }));
    expect(find(el, ".input")).toBeTruthy();
  });

  it("updates its signal on input events", () => {
    const value = signal("");
    const el = render(Input({ value }));
    fire(find(el, "input")!, "input", { target: { value: "hello" } });
    expect(value.val).toBe("hello");
  });
});
