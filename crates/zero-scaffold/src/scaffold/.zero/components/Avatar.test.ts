import { describe, it, expect, afterEach } from "zero/test";
import { render, find, text, cleanup } from "zero/test";
import Avatar from "./Avatar.ts";

describe("Avatar", () => {
  afterEach(cleanup);

  it("renders the base class", () => {
    const el = render(Avatar({ alt: "Ada Lovelace" }));
    expect(find(el, ".avatar")).toBeTruthy();
  });

  it("renders uppercased initials derived from alt when no src or initials given", () => {
    const el = render(Avatar({ alt: "ada lovelace" }));
    expect(text(el, ".avatar-initials")).toBe("A");
  });

  it("renders an <img> with src and alt when src is provided", () => {
    const el = render(Avatar({ src: "/me.png", alt: "Ada" }));
    const img = find(el, "img.avatar");
    expect(img).toBeTruthy();
  });
});
