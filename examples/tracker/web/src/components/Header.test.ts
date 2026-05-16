import { describe, it, expect, afterEach, beforeEach } from "zero/test";
import { render, find, cleanup } from "zero/test";
import Header from "./Header.ts";
import { theme } from "../stores/theme.ts";

describe("Header", () => {
  beforeEach(() => {
    theme.set(null);
  });
  afterEach(cleanup);

  it("renders the brand link to root", () => {
    const el = render(Header());
    const brand = find(el, "a.app-header-brand");
    expect(brand).toBeTruthy();
    expect(brand!.getAttribute("href")).toBe("/");
  });

  it("renders the logo svg", () => {
    const el = render(Header());
    expect(find(el, "svg.app-logo")).toBeTruthy();
  });

  it("mounts the theme toggle", () => {
    const el = render(Header());
    expect(find(el, ".toggle")).toBeTruthy();
  });

  it("renders the tracker title", () => {
    const el = render(Header());
    const title = find(el, ".app-header-title");
    expect(title).toBeTruthy();
  });
});
