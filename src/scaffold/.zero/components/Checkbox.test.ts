import { describe, it, expect, afterEach } from "zero/test";
import { render, find, fire, cleanup } from "zero/test";
import { signal } from "zero";
import Checkbox from "./Checkbox.ts";

describe("Checkbox", () => {
  afterEach(cleanup);

  it("renders the base class", () => {
    const checked = signal(false);
    const el = render(Checkbox({ checked, label: "Sub" }));
    expect(find(el, ".checkbox")).toBeTruthy();
  });

  it("flips its signal on change", () => {
    const checked = signal(false);
    const el = render(Checkbox({ checked, label: "Subscribe" }));
    fire(find(el, "input")!, "change");
    expect(checked.val).toBe(true);
  });
});
