import { describe, it, expect, afterEach } from "zero/test";
import { render, find, findAll, fire, cleanup, text, spy } from "zero/test";
import { signal } from "zero";
import Pagination from "./Pagination.ts";

/**
 * Filter a list of `.pagination-btn` elements down to the numbered page
 * buttons (i.e. drop the prev/next/anything else that isn't a page button).
 * The in-memory DOM selector parser doesn't support `:not()`, so we filter
 * in JS instead of expressing the constraint in CSS.
 *
 * @param {Element} root
 * @returns {Element[]}
 */
function pageBtns(root: Element): Element[] {
  return Array.from(findAll(root, ".pagination-btn")).filter(
    (b) =>
      !(b as HTMLElement).classList.contains("pagination-prev") &&
      !(b as HTMLElement).classList.contains("pagination-next"),
  );
}

/**
 * Read the visible text content of an element, trimmed.
 *
 * @param {Element} el
 * @returns {string}
 */
function btnText(el: Element): string {
  return ((el as HTMLElement).textContent ?? "").trim();
}

describe("Pagination", () => {
  afterEach(cleanup);

  it("renders the base markup", () => {
    const page = signal(1);
    const el = render(Pagination({ page, totalPages: 5 }));
    expect(find(el, "nav.pagination")).toBeTruthy();
    expect(find(el, ".pagination-prev")).toBeTruthy();
    expect(find(el, ".pagination-next")).toBeTruthy();
    expect(pageBtns(el).map(btnText)).toEqual(["1", "2", "3", "4", "5"]);
  });

  it("marks the current page active", () => {
    const page = signal(1);
    const el = render(Pagination({ page, totalPages: 5 }));
    const nav = find(el, "nav.pagination");
    const findPage = (label: string): Element =>
      pageBtns(el).find((b) => btnText(b) === label)!;
    expect(findPage("1").getAttribute("aria-current")).toBe("page");
    expect((findPage("1") as HTMLElement).classList.contains("pagination-active")).toBe(true);
    page.set(3);
    expect(findPage("1").getAttribute("aria-current")).toBe(null);
    expect((findPage("1") as HTMLElement).classList.contains("pagination-active")).toBe(false);
    expect(findPage("3").getAttribute("aria-current")).toBe("page");
    expect((findPage("3") as HTMLElement).classList.contains("pagination-active")).toBe(true);
    expect(find(el, "nav.pagination")).toBe(nav);
  });

  it("prev/next click handlers", () => {
    const page = signal(2);
    const onChange = spy<(p: number) => void>();
    const el = render(Pagination({ page, totalPages: 5, onChange }));
    fire(find(el, ".pagination-next")!, "click");
    expect(page.val).toBe(3);
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith(3);
    fire(find(el, ".pagination-prev")!, "click");
    expect(page.val).toBe(2);
    expect(onChange).toHaveBeenCalledTimes(2);
    expect(onChange).toHaveBeenCalledWith(2);
  });

  it("page-number click", () => {
    const page = signal(1);
    const onChange = spy<(p: number) => void>();
    const el = render(Pagination({ page, totalPages: 5, onChange }));
    const four = pageBtns(el).find((b) => btnText(b) === "4")!;
    fire(four, "click");
    expect(page.val).toBe(4);
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith(4);
  });

  it("prev disabled at start; next disabled at end", () => {
    const page = signal(1);
    const el = render(Pagination({ page, totalPages: 3 }));
    expect(find(el, ".pagination-prev")!.hasAttribute("disabled")).toBe(true);
    fire(find(el, ".pagination-prev")!, "click");
    expect(page.val).toBe(1);
    page.set(3);
    expect(find(el, ".pagination-next")!.hasAttribute("disabled")).toBe(true);
    fire(find(el, ".pagination-next")!, "click");
    expect(page.val).toBe(3);
  });

  it("ellipsis appears at expected positions", () => {
    const page = signal(10);
    const el = render(Pagination({ page, totalPages: 20, siblingCount: 1, boundaryCount: 1 }));
    expect(findAll(el, ".pagination-ellipsis").length).toBe(2);
    expect(pageBtns(el).map(btnText)).toEqual(["1", "9", "10", "11", "20"]);
  });

  it("no ellipsis when totalPages is small", () => {
    const page = signal(1);
    const el = render(Pagination({ page, totalPages: 5 }));
    expect(findAll(el, ".pagination-ellipsis").length).toBe(0);
  });

  it("single-page state is disabled", () => {
    const page = signal(1);
    const el = render(Pagination({ page, totalPages: 1 }));
    expect((find(el, "nav.pagination") as HTMLElement).classList.contains("pagination-disabled")).toBe(true);
    for (const btn of findAll(el, ".pagination-btn")) {
      expect((btn as Element).hasAttribute("disabled")).toBe(true);
    }
    fire(find(el, ".pagination-next")!, "click");
    expect(page.val).toBe(1);
  });

  it("plain disabled: true freezes the pager", () => {
    const page = signal(1);
    const el = render(Pagination({ page, totalPages: 10, disabled: true }));
    expect((find(el, "nav.pagination") as HTMLElement).classList.contains("pagination-disabled")).toBe(true);
    for (const btn of findAll(el, ".pagination-btn")) {
      expect((btn as Element).hasAttribute("disabled")).toBe(true);
    }
  });

  it("reactive disabled signal toggles state without remount", () => {
    const page = signal(1);
    const disabled = signal(false);
    const el = render(Pagination({ page, totalPages: 10, disabled }));
    const nav = find(el, "nav.pagination")!;
    expect((nav as HTMLElement).classList.contains("pagination-disabled")).toBe(false);
    disabled.set(true);
    expect(find(el, "nav.pagination")).toBe(nav);
    expect((nav as HTMLElement).classList.contains("pagination-disabled")).toBe(true);
    for (const btn of findAll(el, ".pagination-btn")) {
      expect((btn as Element).hasAttribute("disabled")).toBe(true);
    }
  });

  it("reactive totalPages signal updates the list without remount", () => {
    const page = signal(1);
    const totalPages = signal(3);
    const el = render(Pagination({ page, totalPages }));
    const nav = find(el, "nav.pagination")!;
    expect(pageBtns(el).map(btnText)).toEqual(["1", "2", "3"]);
    expect(find(el, ".pagination-next")!.hasAttribute("disabled")).toBe(false);
    totalPages.set(5);
    expect(find(el, "nav.pagination")).toBe(nav);
    expect(pageBtns(el).map(btnText)).toEqual(["1", "2", "3", "4", "5"]);
    expect(find(el, ".pagination-prev")!.hasAttribute("disabled")).toBe(true);
  });

  it("out-of-range page clamps for rendering only", () => {
    const page = signal(0);
    const el = render(Pagination({ page, totalPages: 5 }));
    const active = pageBtns(el).find(
      (b) => (b as HTMLElement).classList.contains("pagination-active"),
    )!;
    expect(btnText(active)).toBe("1");
    expect(page.val).toBe(0);
  });

  it("size variant class", () => {
    const sm = render(Pagination({ page: signal(1), totalPages: 5, size: "sm" }));
    expect((find(sm, "nav.pagination") as HTMLElement).classList.contains("pagination-sm")).toBe(true);
    const md = render(Pagination({ page: signal(1), totalPages: 5 }));
    expect((find(md, "nav.pagination") as HTMLElement).classList.contains("pagination-md")).toBe(true);
  });

  it("summary slot renders and updates", () => {
    const page = signal(1);
    const el = render(
      Pagination({
        page,
        totalPages: 5,
        summary: (p, t) => `Page ${p} of ${t}`,
      }),
    );
    expect(text(el, ".pagination-summary")).toBe("Page 1 of 5");
    page.set(2);
    expect(text(el, ".pagination-summary")).toBe("Page 2 of 5");
  });
});
