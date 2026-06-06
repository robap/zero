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

  it("applies attrs to the inner input and focuses it with autofocus", async () => {
    const checked = signal(false);
    const el = render(
      Checkbox({
        checked,
        label: "Sub",
        autofocus: true,
        attrs: { name: "subscribe", "data-test": "cb", type: "nope" },
      }),
    );
    const input = find(el, "input")!;
    await Promise.resolve();
    expect(input.getAttribute("name")).toBe("subscribe");
    expect(input.getAttribute("data-test")).toBe("cb");
    expect(input.getAttribute("type")).toBe("checkbox");
    expect(document.activeElement).toBe(input);
  });

  it("flips its signal on change", () => {
    const checked = signal(false);
    const el = render(Checkbox({ checked, label: "Subscribe" }));
    fire(find(el, "input")!, "change");
    expect(checked.val).toBe(true);
  });

  it("flips synchronously with debounceMs: 0", () => {
    const checked = signal(false);
    const el = render(Checkbox({ checked, label: "Subscribe", debounceMs: 0 }));
    fire(find(el, "input")!, "change");
    expect(checked.val).toBe(true);
  });

  it("renders no error node and aria-invalid 'false' without an error prop", () => {
    const checked = signal(false);
    const el = render(Checkbox({ checked, label: "Sub" }));
    expect(find(el, "[data-field-error]")).toBe(null);
    expect(find(el, "input")!.getAttribute("aria-invalid")).toBe("false");
  });

  it("renders the error message with aria wiring when errored", () => {
    const checked = signal(false);
    const error = signal<string | null>("Must accept.");
    const el = render(Checkbox({ checked, label: "Sub", error }));
    const node = find(el, "[data-field-error]")!;
    expect(node).toBeTruthy();
    expect((node.textContent ?? "").trim()).toBe("Must accept.");
    const input = find(el, "input")!;
    expect(input.getAttribute("aria-invalid")).toBe("true");
    expect(input.getAttribute("aria-describedby")).toBe(
      node.getAttribute("id"),
    );
  });

  it("renders the error node outside the label", () => {
    const checked = signal(false);
    const error = signal<string | null>("Must accept.");
    const el = render(Checkbox({ checked, label: "Sub", error }));
    expect(find(el, "[data-field-error]")).toBeTruthy();
    expect(find(el, "label [data-field-error]")).toBe(null);
  });

  it("clears the error node and aria-invalid when the signal goes null", () => {
    const checked = signal(false);
    const error = signal<string | null>("Must accept.");
    const el = render(Checkbox({ checked, label: "Sub", error }));
    expect(find(el, "[data-field-error]")).toBeTruthy();
    error.set(null);
    expect(find(el, "[data-field-error]")).toBe(null);
    expect(find(el, "input")!.getAttribute("aria-invalid")).toBe("false");
  });

  it("honours debounceMs", async () => {
    const checked = signal(false);
    const el = render(Checkbox({ checked, label: "Subscribe", debounceMs: 50 }));
    fire(find(el, "input")!, "change");
    expect(checked.val).toBe(false);
    await new Promise((r) => setTimeout(r, 80));
    expect(checked.val).toBe(true);
  });
});
