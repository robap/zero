import { describe, it, expect, afterEach } from "zero/test";
import { render, findAll, fire, cleanup, find } from "zero/test";
import { signal, html } from "zero";
import Radio from "./Radio.ts";

describe("Radio", () => {
  afterEach(cleanup);

  it("renders the base class", () => {
    const selected = signal("a");
    const el = render(Radio({ selected, name: "g", value: "a", label: "A" }));
    expect(find(el, ".radio")).toBeTruthy();
  });

  it("applies attrs to the inner input without overriding the prop-rendered name", async () => {
    const selected = signal("a");
    const el = render(
      Radio({
        selected,
        name: "size",
        value: "a",
        label: "A",
        autofocus: true,
        attrs: { name: "smuggled", "data-test": "rd" },
      }),
    );
    const input = find(el, "input")!;
    await Promise.resolve();
    expect(input.getAttribute("name")).toBe("size");
    expect(input.getAttribute("data-test")).toBe("rd");
    expect(input.getAttribute("type")).toBe("radio");
    expect(document.activeElement).toBe(input);
  });

  it("renders no error node and aria-invalid 'false' without an error prop", () => {
    const selected = signal("a");
    const el = render(Radio({ selected, name: "g", value: "a", label: "A" }));
    expect(find(el, "[data-field-error]")).toBe(null);
    expect(find(el, "input")!.getAttribute("aria-invalid")).toBe("false");
  });

  it("renders the error message outside the label with aria wiring", () => {
    const selected = signal("a");
    const error = signal<string | null>("Pick a plan.");
    const el = render(
      Radio({ selected, name: "g", value: "a", label: "A", error }),
    );
    const node = find(el, "[data-field-error]")!;
    expect(node).toBeTruthy();
    expect((node.textContent ?? "").trim()).toBe("Pick a plan.");
    expect(find(el, "label [data-field-error]")).toBe(null);
    const input = find(el, "input")!;
    expect(input.getAttribute("aria-invalid")).toBe("true");
    expect(input.getAttribute("aria-describedby")).toBe(
      node.getAttribute("id"),
    );
  });

  it("clears the error node and aria-invalid when the signal goes null", () => {
    const selected = signal("a");
    const error = signal<string | null>("Pick a plan.");
    const el = render(
      Radio({ selected, name: "g", value: "a", label: "A", error }),
    );
    expect(find(el, "[data-field-error]")).toBeTruthy();
    error.set(null);
    expect(find(el, "[data-field-error]")).toBe(null);
    expect(find(el, "input")!.getAttribute("aria-invalid")).toBe("false");
  });

  it("updates the selected signal when a second radio is clicked", () => {
    const selected = signal("a");
    const group = html`<div>
      ${Radio({ selected, name: "g", value: "a", label: "A" })}
      ${Radio({ selected, name: "g", value: "b", label: "B" })}
    </div>`;
    const el = render(group);
    const inputs = findAll(el, "input[type=radio]");
    fire(inputs[1]!, "change");
    expect(selected.val).toBe("b");
  });

  it("honours debounceMs", async () => {
    const selected = signal("a");
    const group = html`<div>
      ${Radio({ selected, name: "g", value: "a", label: "A" })}
      ${Radio({ selected, name: "g", value: "b", label: "B", debounceMs: 50 })}
    </div>`;
    const el = render(group);
    const inputs = findAll(el, "input[type=radio]");
    fire(inputs[1]!, "change");
    expect(selected.val).toBe("a");
    await new Promise((r) => setTimeout(r, 80));
    expect(selected.val).toBe("b");
  });
});
