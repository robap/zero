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
