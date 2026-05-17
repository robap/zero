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
    // The test DOM shim has no `matchMedia`, so the toggle seeds to "off"
    // and the theme must be exactly "light" — not just *some* concrete
    // value. Flipping once must drive the theme to exactly "dark".
    expect(theme.val).toBe("light");
    fire(find(el, "input")!, "change");
    expect(theme.val).toBe("dark");
  });
});
