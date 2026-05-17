import { describe, it, expect, afterEach } from "zero/test";
import { render, find, findAll, cleanup } from "zero/test";
import { signal } from "zero";
import Dialog from "./Dialog.ts";

describe("Dialog", () => {
  afterEach(cleanup);

  it("renders nothing when open is false", () => {
    const open = signal(false);
    const el = render(Dialog({ open, children: "Body" }));
    expect(findAll(el, ".dialog-open").length).toBe(0);
  });

  it("renders the backdrop when open flips to true", () => {
    const open = signal(false);
    const el = render(Dialog({ open, children: "Body" }));
    open.set(true);
    expect(find(el, ".dialog-open")).toBeTruthy();
  });
});
