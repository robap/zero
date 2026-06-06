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

  it("renders no error node and aria-invalid 'false' without an error prop", () => {
    const checked = signal(false);
    const el = render(Toggle({ checked, label: "Wifi" }));
    expect(find(el, "[data-field-error]")).toBe(null);
    expect(find(el, "input")!.getAttribute("aria-invalid")).toBe("false");
  });

  it("renders the error message outside the label with aria wiring", () => {
    const checked = signal(false);
    const error = signal<string | null>("Must be on.");
    const el = render(Toggle({ checked, label: "Wifi", error }));
    const node = find(el, "[data-field-error]")!;
    expect(node).toBeTruthy();
    expect((node.textContent ?? "").trim()).toBe("Must be on.");
    expect(find(el, "label [data-field-error]")).toBe(null);
    const input = find(el, "input")!;
    expect(input.getAttribute("aria-invalid")).toBe("true");
    expect(input.getAttribute("aria-describedby")).toBe(
      node.getAttribute("id"),
    );
  });

  it("clears the error node and aria-invalid when the signal goes null", () => {
    const checked = signal(false);
    const error = signal<string | null>("Must be on.");
    const el = render(Toggle({ checked, label: "Wifi", error }));
    expect(find(el, "[data-field-error]")).toBeTruthy();
    error.set(null);
    expect(find(el, "[data-field-error]")).toBe(null);
    expect(find(el, "input")!.getAttribute("aria-invalid")).toBe("false");
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
