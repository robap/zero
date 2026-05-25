import { describe, it, expect, afterEach } from "zero/test";
import { render, find, fire, cleanup } from "zero/test";
import { signal } from "zero";
import TextArea from "./TextArea.ts";

describe("TextArea", () => {
  afterEach(cleanup);

  it("renders the base class", () => {
    const value = signal("");
    const el = render(TextArea({ value }));
    expect(find(el, ".textarea")).toBeTruthy();
  });

  it("updates its signal on input events", () => {
    const value = signal("");
    const el = render(TextArea({ value }));
    fire(find(el, "textarea")!, "input", { target: { value: "hi there" } });
    expect(value.val).toBe("hi there");
  });

  it("honours debounceMs", async () => {
    const value = signal("");
    const el = render(TextArea({ value, debounceMs: 50 }));
    fire(find(el, "textarea")!, "input", { target: { value: "hi there" } });
    expect(value.val).toBe("");
    await new Promise((r) => setTimeout(r, 80));
    expect(value.val).toBe("hi there");
  });
});
