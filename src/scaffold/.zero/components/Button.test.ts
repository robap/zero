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
});
