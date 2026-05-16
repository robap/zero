import { describe, it, expect, afterEach, beforeEach } from "zero/test";
import { render, find, fire, cleanup } from "zero/test";
import ThemeToggle from "./ThemeToggle.ts";
import { theme } from "../stores/theme.ts";

describe("ThemeToggle", () => {
  beforeEach(() => {
    theme.set(null);
  });
  afterEach(cleanup);

  it("renders a toggle switch", () => {
    const el = render(ThemeToggle());
    expect(find(el, ".toggle")).toBeTruthy();
  });

  it("mounting seeds a concrete theme (exits the follow-system state)", () => {
    render(ThemeToggle());
    expect(theme.val === "light" || theme.val === "dark").toBeTruthy();
  });

  it("flipping switches the theme between light and dark", () => {
    const el = render(ThemeToggle());
    const before = theme.val;
    fire(find(el, "input")!, "change");
    const after = theme.val;
    expect(after === "light" || after === "dark").toBeTruthy();
    expect(after === before).toBeFalsy();
  });
});
