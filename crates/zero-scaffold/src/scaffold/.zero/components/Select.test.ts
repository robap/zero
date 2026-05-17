import { describe, it, expect, afterEach } from "zero/test";
import { render, find, fire, cleanup } from "zero/test";
import { signal } from "zero";
import Select from "./Select.ts";

describe("Select", () => {
  afterEach(cleanup);

  it("renders the base class", () => {
    const value = signal("a");
    const el = render(
      Select({
        value,
        options: [
          { value: "a", label: "A" },
          { value: "b", label: "B" },
        ],
      }),
    );
    expect(find(el, ".select")).toBeTruthy();
  });

  it("updates its signal on change events", () => {
    const value = signal("a");
    const el = render(
      Select({
        value,
        options: [
          { value: "a", label: "A" },
          { value: "b", label: "B" },
        ],
      }),
    );
    fire(find(el, "select")!, "change", { target: { value: "b" } });
    expect(value.val).toBe("b");
  });
});
