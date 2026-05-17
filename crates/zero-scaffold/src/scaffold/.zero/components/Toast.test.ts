import { describe, it, expect, afterEach } from "zero/test";
import { render, find, findAll, text, cleanup } from "zero/test";
import { signal } from "zero";
import Toast from "./Toast.ts";

describe("Toast", () => {
  afterEach(cleanup);

  it("renders nothing when open is false", () => {
    const open = signal(false);
    const el = render(Toast({ open, message: "Saved" }));
    expect(findAll(el, ".toast").length).toBe(0);
  });

  it("shows the message when open flips to true", () => {
    const open = signal(false);
    const el = render(Toast({ open, message: "Saved" }));
    open.set(true);
    expect(find(el, ".toast")).toBeTruthy();
    expect(text(el, ".toast")).toBe("Saved");
  });
});
