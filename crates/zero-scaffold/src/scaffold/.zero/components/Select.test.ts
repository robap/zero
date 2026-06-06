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

  it("applies attrs additively and focuses with autofocus after mount", async () => {
    const value = signal("a");
    const el = render(
      Select({
        value,
        options: [{ value: "a", label: "A" }],
        autofocus: true,
        attrs: { name: "country", class: "nope" },
      }),
    );
    const select = find(el, "select")!;
    await Promise.resolve();
    expect(select.getAttribute("name")).toBe("country");
    expect(select.getAttribute("class")).toBe("select select-md");
    expect(document.activeElement).toBe(select);
  });

  it("renders no error node and aria-invalid 'false' without an error prop", () => {
    const value = signal("a");
    const el = render(
      Select({ value, options: [{ value: "a", label: "A" }] }),
    );
    expect(find(el, "[data-field-error]")).toBe(null);
    expect(find(el, "select")!.getAttribute("aria-invalid")).toBe("false");
  });

  it("renders the error message with aria wiring when errored", () => {
    const value = signal("a");
    const error = signal<string | null>("Pick one.");
    const el = render(
      Select({ value, error, options: [{ value: "a", label: "A" }] }),
    );
    const node = find(el, "[data-field-error]")!;
    expect(node).toBeTruthy();
    expect((node.textContent ?? "").trim()).toBe("Pick one.");
    const select = find(el, "select")!;
    expect(select.getAttribute("aria-invalid")).toBe("true");
    expect(select.getAttribute("aria-describedby")).toBe(
      node.getAttribute("id"),
    );
  });

  it("clears the error node and aria-invalid when the signal goes null", () => {
    const value = signal("a");
    const error = signal<string | null>("Pick one.");
    const el = render(
      Select({ value, error, options: [{ value: "a", label: "A" }] }),
    );
    expect(find(el, "[data-field-error]")).toBeTruthy();
    error.set(null);
    expect(find(el, "[data-field-error]")).toBe(null);
    expect(find(el, "select")!.getAttribute("aria-invalid")).toBe("false");
  });

  it("renders the signal-selected option", () => {
    const value = signal("b");
    const el = render(
      Select({
        value,
        options: [
          { value: "a", label: "A" },
          { value: "b", label: "B" },
        ],
      }),
    );
    const sel = find(el, "select") as HTMLSelectElement;
    expect(sel.value).toBe("b");
    expect(sel.selectedOptions.length).toBe(1);
  });

  it("selection tracks signal changes", () => {
    const value = signal("b");
    const el = render(
      Select({
        value,
        options: [
          { value: "a", label: "A" },
          { value: "b", label: "B" },
        ],
      }),
    );
    const sel = find(el, "select") as HTMLSelectElement;
    value.set("a");
    expect(sel.value).toBe("a");
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
    const sel = find(el, "select") as HTMLSelectElement;
    sel.value = "b";
    fire(sel, "change");
    expect(value.val).toBe("b");
  });

  it("honours debounceMs", async () => {
    const value = signal("a");
    const el = render(
      Select({
        value,
        debounceMs: 50,
        options: [
          { value: "a", label: "A" },
          { value: "b", label: "B" },
        ],
      }),
    );
    const sel = find(el, "select") as HTMLSelectElement;
    sel.value = "b";
    fire(sel, "change");
    expect(value.val).toBe("a");
    await new Promise((r) => setTimeout(r, 80));
    expect(value.val).toBe("b");
  });

  it("invokes onChange with the new value after the signal write", () => {
    const value = signal("a");
    const seen: string[] = [];
    const el = render(
      Select({
        value,
        options: [
          { value: "a", label: "A" },
          { value: "b", label: "B" },
        ],
        onChange: (v) => seen.push(`${v}:${value.val}`),
      }),
    );
    const sel = find(el, "select") as HTMLSelectElement;
    sel.value = "b";
    fire(sel, "change");
    // Callback sees the new value, and the signal is already written.
    expect(seen).toEqual(["b:b"]);
  });
});
