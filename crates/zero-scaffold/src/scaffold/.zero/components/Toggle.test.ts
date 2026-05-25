import { describe, it, expect, afterEach } from "zero/test";
import { render, find, fire, cleanup } from "zero/test";
import { signal } from "zero";
import Toggle from "./Toggle.ts";

describe("Toggle", () => {
  afterEach(cleanup);

  it("renders the base class", () => {
    const checked = signal(false);
    const el = render(Toggle({ checked, label: "Wifi" }));
    expect(find(el, ".toggle")).toBeTruthy();
  });

  it("flips its signal on change", () => {
    const checked = signal(false);
    const el = render(Toggle({ checked, label: "Wifi" }));
    fire(find(el, "input")!, "change");
    expect(checked.val).toBe(true);
  });

  it("honours debounceMs", async () => {
    const checked = signal(false);
    const el = render(Toggle({ checked, label: "Wifi", debounceMs: 50 }));
    fire(find(el, "input")!, "change");
    expect(checked.val).toBe(false);
    await new Promise((r) => setTimeout(r, 80));
    expect(checked.val).toBe(true);
  });
});
