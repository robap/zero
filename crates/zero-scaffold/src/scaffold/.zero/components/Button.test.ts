import { describe, it, expect, afterEach } from "zero/test";
import { render, find, fire, cleanup, spy } from "zero/test";
import Button from "./Button.ts";

describe("Button", () => {
  afterEach(cleanup);

  it("renders with the base class", () => {
    const el = render(Button({ children: "Click me" }));
    expect(find(el, ".button")).toBeTruthy();
  });

  it("fires onClick when clicked", () => {
    const onClick = spy();
    const el = render(Button({ onClick, children: "Click me" }));
    fire(find(el, "button")!, "click");
    expect(onClick.callCount).toBe(1);
  });

  it("does not fire onClick when disabled", () => {
    const onClick = spy();
    const el = render(Button({ onClick, disabled: true, children: "Click me" }));
    fire(find(el, "button")!, "click");
    expect(onClick.callCount).toBe(0);
  });

  it("defaults type to button", () => {
    const el = render(Button({ children: "Click me" }));
    expect(find(el, "button")!.getAttribute("type")).toBe("button");
  });

  it("renders an explicit type", () => {
    const submit = render(Button({ type: "submit", children: "Save" }));
    expect(find(submit, "button")!.getAttribute("type")).toBe("submit");
    const reset = render(Button({ type: "reset", children: "Reset" }));
    expect(find(reset, "button")!.getAttribute("type")).toBe("reset");
  });

  it("renders form only when provided", () => {
    const withForm = render(Button({ form: "edit-form", children: "Save" }));
    expect(find(withForm, "button")!.getAttribute("form")).toBe("edit-form");
    const withoutForm = render(Button({ children: "Save" }));
    expect(find(withoutForm, "button")!.getAttribute("form")).toBeNull();
  });

  it("renders name and value only when provided", () => {
    const withNV = render(Button({ name: "action", value: "save", children: "Save" }));
    const btn = find(withNV, "button")!;
    expect(btn.getAttribute("name")).toBe("action");
    expect(btn.getAttribute("value")).toBe("save");
    const without = find(render(Button({ children: "Save" })), "button")!;
    expect(without.getAttribute("name")).toBeNull();
    expect(without.getAttribute("value")).toBeNull();
  });

  it("sets disabled when loading", () => {
    const el = render(Button({ loading: true, children: "Saving…" }));
    expect(find(el, "button")!.getAttribute("disabled")).not.toBeNull();
  });

  it("does not fire onClick when loading", () => {
    const onClick = spy();
    const el = render(Button({ onClick, loading: true, children: "Saving…" }));
    fire(find(el, "button")!, "click");
    expect(onClick.callCount).toBe(0);
  });

  it("still renders the spinner when loading", () => {
    const el = render(Button({ loading: true, children: "Saving…" }));
    expect(find(el, ".button-spinner")).toBeTruthy();
  });
});
