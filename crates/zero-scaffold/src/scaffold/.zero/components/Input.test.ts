import { describe, it, expect, afterEach } from "zero/test";
import { render, find, fire, cleanup } from "zero/test";
import { signal } from "zero";
import Input from "./Input.ts";

describe("Input", () => {
  afterEach(cleanup);

  it("renders the base class", () => {
    const value = signal("");
    const el = render(Input({ value }));
    expect(find(el, ".input")).toBeTruthy();
  });

  it("updates its signal on input events", () => {
    const value = signal("");
    const el = render(Input({ value }));
    fire(find(el, "input")!, "input", { target: { value: "hello" } });
    expect(value.val).toBe("hello");
  });

  it("writes synchronously with debounceMs: 0", () => {
    const value = signal("");
    const el = render(Input({ value, debounceMs: 0 }));
    fire(find(el, "input")!, "input", { target: { value: "hello" } });
    expect(value.val).toBe("hello");
  });

  it("honours debounceMs", async () => {
    const value = signal("");
    const el = render(Input({ value, debounceMs: 50 }));
    fire(find(el, "input")!, "input", { target: { value: "hello" } });
    expect(value.val).toBe("");
    await new Promise((r) => setTimeout(r, 80));
    expect(value.val).toBe("hello");
  });

  it("collapses successive events within the window to one write", async () => {
    const value = signal("");
    const el = render(Input({ value, debounceMs: 50 }));
    const input = find(el, "input")!;
    fire(input, "input", { target: { value: "a" } });
    fire(input, "input", { target: { value: "ab" } });
    fire(input, "input", { target: { value: "abc" } });
    await new Promise((r) => setTimeout(r, 80));
    expect(value.val).toBe("abc");
  });

  it("renders no error node and aria-invalid 'false' without an error prop", () => {
    const value = signal("");
    const el = render(Input({ value }));
    expect(find(el, "[data-field-error]")).toBe(null);
    expect(find(el, "input")!.getAttribute("aria-invalid")).toBe("false");
  });

  it("renders the error message with aria wiring when errored", () => {
    const value = signal("");
    const error = signal<string | null>("Required.");
    const el = render(Input({ value, error }));
    const node = find(el, "[data-field-error]")!;
    expect(node).toBeTruthy();
    expect((node.textContent ?? "").trim()).toBe("Required.");
    const input = find(el, "input")!;
    expect(input.getAttribute("aria-invalid")).toBe("true");
    expect(input.getAttribute("aria-describedby")).toBe(
      node.getAttribute("id"),
    );
  });

  it("clears the error node and aria-invalid when the signal goes null", () => {
    const value = signal("");
    const error = signal<string | null>("Required.");
    const el = render(Input({ value, error }));
    expect(find(el, "[data-field-error]")).toBeTruthy();
    error.set(null);
    expect(find(el, "[data-field-error]")).toBe(null);
    expect(find(el, "input")!.getAttribute("aria-invalid")).toBe("false");
  });

  it("invokes onChange with the new value after the signal write", () => {
    const value = signal("");
    const seen: string[] = [];
    const el = render(
      Input({
        value,
        onChange: (v) => seen.push(`${v}:${value.val}`),
      }),
    );
    fire(find(el, "input")!, "input", { target: { value: "hello" } });
    // Callback sees the new value, and the signal is already written.
    expect(seen).toEqual(["hello:hello"]);
  });

  it("debounces onChange together with the signal write", async () => {
    const value = signal("");
    const seen: string[] = [];
    const el = render(
      Input({ value, debounceMs: 50, onChange: (v) => seen.push(v) }),
    );
    const input = find(el, "input")!;
    fire(input, "input", { target: { value: "a" } });
    fire(input, "input", { target: { value: "ab" } });
    expect(seen).toEqual([]);
    await new Promise((r) => setTimeout(r, 80));
    expect(seen).toEqual(["ab"]);
  });
});
