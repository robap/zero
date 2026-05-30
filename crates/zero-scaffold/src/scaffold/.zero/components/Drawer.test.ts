import { describe, it, expect, afterEach } from "zero/test";
import { render, find, findAll, fire, cleanup, spy } from "zero/test";
import { signal, html } from "zero";
import Drawer from "./Drawer.ts";
import type { DrawerSide, DrawerSize } from "./Drawer.ts";

describe("Drawer", () => {
  afterEach(cleanup);

  it("renders the base markup for a closed overlay drawer", () => {
    const open = signal(false);
    const el = render(Drawer({ open, side: "right" }));
    const aside = find(el, "aside.drawer") as HTMLElement;
    expect(aside).toBeTruthy();
    expect(aside.classList.contains("drawer-overlay")).toBe(true);
    expect(aside.classList.contains("drawer-right")).toBe(true);
    expect(aside.classList.contains("drawer-md")).toBe(true);
    expect(aside.classList.contains("drawer-open")).toBe(false);
    const backdrop = find(el, ".drawer-backdrop") as HTMLElement;
    expect(backdrop).toBeTruthy();
    expect(backdrop.classList.contains("drawer-backdrop-open")).toBe(false);
  });

  it("toggles the open classes with the open signal without remounting", () => {
    const open = signal(false);
    const el = render(Drawer({ open, side: "right" }));
    const aside = find(el, "aside.drawer") as HTMLElement;
    const backdrop = find(el, ".drawer-backdrop") as HTMLElement;

    open.set(true);
    expect(aside.classList.contains("drawer-open")).toBe(true);
    expect(backdrop.classList.contains("drawer-backdrop-open")).toBe(true);

    open.set(false);
    expect(aside.classList.contains("drawer-open")).toBe(false);
    expect(backdrop.classList.contains("drawer-backdrop-open")).toBe(false);

    // Same node references survive the toggle — the DOM stays mounted.
    expect(find(el, "aside.drawer")).toBe(aside);
    expect(find(el, ".drawer-backdrop")).toBe(backdrop);
  });

  it("skips the backdrop and uses complementary role in push mode", () => {
    const open = signal(true);
    const el = render(Drawer({ open, side: "right", mode: "push" }));
    expect(findAll(el, ".drawer-backdrop").length).toBe(0);
    const aside = find(el, "aside.drawer") as HTMLElement;
    expect(aside.classList.contains("drawer-push")).toBe(true);
    expect(aside.classList.contains("drawer-overlay")).toBe(false);
    expect(aside.getAttribute("role")).toBe("complementary");
  });

  it("renders the backdrop and dialog ARIA in overlay mode", () => {
    const open = signal(true);
    const el = render(Drawer({ open, side: "right" }));
    expect(find(el, ".drawer-backdrop")).toBeTruthy();
    const aside = find(el, "aside.drawer") as HTMLElement;
    expect(aside.getAttribute("role")).toBe("dialog");
    expect(aside.getAttribute("aria-modal")).toBe("true");
  });

  it("applies the side class for each side", () => {
    const sides: DrawerSide[] = ["left", "right", "top", "bottom"];
    for (const side of sides) {
      const el = render(Drawer({ open: signal(false), side }));
      const aside = find(el, "aside.drawer") as HTMLElement;
      expect(aside.classList.contains(`drawer-${side}`)).toBe(true);
    }
  });

  it("defaults to md and applies the size class for each size", () => {
    const def = render(Drawer({ open: signal(false), side: "right" }));
    expect(
      (find(def, "aside.drawer") as HTMLElement).classList.contains("drawer-md"),
    ).toBe(true);

    const sizes: DrawerSize[] = ["sm", "md", "lg"];
    for (const size of sizes) {
      const el = render(Drawer({ open: signal(false), side: "right", size }));
      const aside = find(el, "aside.drawer") as HTMLElement;
      expect(aside.classList.contains(`drawer-${size}`)).toBe(true);
    }
  });

  it("renders string slots inside their wrappers without hidden", () => {
    const el = render(
      Drawer({
        open: signal(true),
        side: "right",
        title: "Edit user",
        body: "Form goes here",
        controls: "Buttons",
      }),
    );
    const title = find(el, ".drawer-title") as HTMLElement;
    const body = find(el, ".drawer-body") as HTMLElement;
    const controls = find(el, ".drawer-controls") as HTMLElement;
    expect(title.textContent).toContain("Edit user");
    expect(body.textContent).toContain("Form goes here");
    expect(controls.textContent).toContain("Buttons");
    expect(title.hasAttribute("hidden")).toBe(false);
    expect(body.hasAttribute("hidden")).toBe(false);
    expect(controls.hasAttribute("hidden")).toBe(false);
  });

  it("renders TemplateResult slots inside their wrappers", () => {
    const el = render(
      Drawer({
        open: signal(true),
        side: "right",
        title: html`<h2 class="text-h2">Edit user</h2>`,
        body: html`<p class="text-body">Body</p>`,
        controls: html`<button>Save</button>`,
      }),
    );
    expect(find(find(el, ".drawer-title") as Element, "h2")).toBeTruthy();
    expect(find(find(el, ".drawer-body") as Element, "p")).toBeTruthy();
    expect(find(find(el, ".drawer-controls") as Element, "button")).toBeTruthy();
  });

  it("hides each wrapper independently for null / undefined / empty slots", () => {
    const nullTitle = render(
      Drawer({ open: signal(true), side: "right", title: null, body: "b", controls: "c" }),
    );
    expect((find(nullTitle, ".drawer-title") as HTMLElement).hasAttribute("hidden")).toBe(true);
    expect((find(nullTitle, ".drawer-body") as HTMLElement).hasAttribute("hidden")).toBe(false);
    expect((find(nullTitle, ".drawer-controls") as HTMLElement).hasAttribute("hidden")).toBe(false);

    const omitted = render(Drawer({ open: signal(true), side: "right" }));
    expect((find(omitted, ".drawer-title") as HTMLElement).hasAttribute("hidden")).toBe(true);
    expect((find(omitted, ".drawer-body") as HTMLElement).hasAttribute("hidden")).toBe(true);
    expect((find(omitted, ".drawer-controls") as HTMLElement).hasAttribute("hidden")).toBe(true);

    const emptyControls = render(
      Drawer({ open: signal(true), side: "right", controls: "" }),
    );
    expect((find(emptyControls, ".drawer-controls") as HTMLElement).hasAttribute("hidden")).toBe(true);
  });

  it("updates a reactive function slot in place without remounting the panel", () => {
    const state = signal(true);
    const el = render(
      Drawer({
        open: signal(true),
        side: "right",
        body: () => (state.val ? "A" : "B"),
      }),
    );
    const aside = find(el, "aside.drawer") as HTMLElement;
    const body = find(el, ".drawer-body") as HTMLElement;
    expect(body.textContent).toContain("A");

    state.set(false);
    expect(body.textContent).toContain("B");
    // Panel node identity is preserved across the slot swap.
    expect(find(el, "aside.drawer")).toBe(aside);
  });

  it("does not close when the backdrop is clicked", () => {
    const open = signal(true);
    const el = render(Drawer({ open, side: "right" }));
    const backdrop = find(el, ".drawer-backdrop") as HTMLElement;
    fire(backdrop, "click");
    expect(open.val).toBe(true);
  });

  it("does not close when Escape is pressed", () => {
    const open = signal(true);
    render(Drawer({ open, side: "right" }));
    fire(document as unknown as Element, "keydown", { key: "Escape" });
    expect(open.val).toBe(true);
  });

  it("registers no document listeners across many mounts", () => {
    const orig = document.addEventListener.bind(document);
    const s = spy(orig);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    (document as any).addEventListener = s;
    try {
      for (let i = 0; i < 3; i++) {
        render(Drawer({ open: signal(i % 2 === 0), side: "right" }));
        cleanup();
      }
    } finally {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (document as any).addEventListener = orig;
    }
    expect(s).not.toHaveBeenCalled();
  });
});
