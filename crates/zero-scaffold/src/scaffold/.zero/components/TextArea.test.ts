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

  it("applies attrs additively and focuses with autofocus after mount", async () => {
    const value = signal("");
    const el = render(
      TextArea({
        value,
        autofocus: true,
        attrs: { name: "notes", maxlength: 200, class: "nope" },
      }),
    );
    const area = find(el, "textarea")!;
    await Promise.resolve();
    expect(area.getAttribute("name")).toBe("notes");
    expect(area.getAttribute("maxlength")).toBe("200");
    expect(area.getAttribute("class")).toBe("textarea");
    expect(document.activeElement).toBe(area);
  });

  it("updates its signal on input events", () => {
    const value = signal("");
    const el = render(TextArea({ value }));
    fire(find(el, "textarea")!, "input", { target: { value: "hi there" } });
    expect(value.val).toBe("hi there");
  });

  it("renders no error node and aria-invalid 'false' without an error prop", () => {
    const value = signal("");
    const el = render(TextArea({ value }));
    expect(find(el, "[data-field-error]")).toBe(null);
    expect(find(el, "textarea")!.getAttribute("aria-invalid")).toBe("false");
  });

  it("renders the error message with aria wiring when errored", () => {
    const value = signal("");
    const error = signal<string | null>("Required.");
    const el = render(TextArea({ value, error }));
    const node = find(el, "[data-field-error]")!;
    expect(node).toBeTruthy();
    expect((node.textContent ?? "").trim()).toBe("Required.");
    const area = find(el, "textarea")!;
    expect(area.getAttribute("aria-invalid")).toBe("true");
    expect(area.getAttribute("aria-describedby")).toBe(
      node.getAttribute("id"),
    );
  });

  it("clears the error node and aria-invalid when the signal goes null", () => {
    const value = signal("");
    const error = signal<string | null>("Required.");
    const el = render(TextArea({ value, error }));
    expect(find(el, "[data-field-error]")).toBeTruthy();
    error.set(null);
    expect(find(el, "[data-field-error]")).toBe(null);
    expect(find(el, "textarea")!.getAttribute("aria-invalid")).toBe("false");
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
