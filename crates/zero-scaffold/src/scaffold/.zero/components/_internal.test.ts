import { describe, it, expect, afterEach } from "zero/test";
import { render, find, cleanup } from "zero/test";
import { signal, computed, ref } from "zero";
import {
  isReactive,
  read,
  debounce,
  uniqueId,
  errorNode,
  ariaInvalid,
  ariaDescribedBy,
  nativeRef,
} from "./_internal.ts";

describe("_internal", () => {
  describe("nativeRef", () => {
    it("applies string/number/true attrs additively and focuses after the element commits", async () => {
      const el = document.createElement("input");
      el.setAttribute("class", "input input-md");
      document.body.appendChild(el);
      const r = nativeRef<HTMLInputElement>(
        {
          name: "customer",
          tabindex: 3,
          required: true,
          skipped: false,
          class: "nope",
        },
        true,
      );
      r.el = el as unknown as HTMLInputElement;
      // Nothing happens synchronously — the apply is post-commit.
      expect(el.hasAttribute("name")).toBe(false);
      await Promise.resolve();
      expect(el.getAttribute("name")).toBe("customer");
      expect(el.getAttribute("tabindex")).toBe("3");
      expect(el.getAttribute("required")).toBe("");
      expect(el.hasAttribute("skipped")).toBe(false);
      expect(el.getAttribute("class")).toBe("input input-md");
      expect(document.activeElement).toBe(el);
      document.body.removeChild(el);
    });

    it("re-applies on a later commit — deferred dialog-open case included", async () => {
      // Simulates a Dialog: the component (and ref) exist long before the
      // element commits, and the element commits again on each re-open.
      const r = nativeRef<HTMLInputElement>({ name: "code" }, true);
      await Promise.resolve();
      expect(r.el).toBe(null); // nothing committed yet — no crash, no work
      const first = document.createElement("input");
      document.body.appendChild(first);
      r.el = first as unknown as HTMLInputElement;
      await Promise.resolve();
      expect(first.getAttribute("name")).toBe("code");
      expect(document.activeElement).toBe(first);
      // Dialog closes (disposal nulls the ref), then reopens with fresh DOM.
      r.el = null;
      const second = document.createElement("input");
      document.body.appendChild(second);
      r.el = second as unknown as HTMLInputElement;
      await Promise.resolve();
      expect(second.getAttribute("name")).toBe("code");
      expect(document.activeElement).toBe(second);
      document.body.removeChild(first);
      document.body.removeChild(second);
    });

    it("bails when the element is cleaned up before the microtask", async () => {
      const el = document.createElement("input");
      const r = nativeRef<HTMLInputElement>({ name: "x" }, true);
      r.el = el as unknown as HTMLInputElement;
      r.el = null; // disposed in the same task
      await Promise.resolve();
      expect(el.hasAttribute("name")).toBe(false);
      expect(r.el).toBe(null);
    });

    it("acts as a plain ref when no attrs and no autofocus are given", () => {
      const r = nativeRef<HTMLInputElement>(undefined, undefined);
      const el = document.createElement("input");
      r.el = el as unknown as HTMLInputElement;
      expect(r.el).toBe(el);
      r.el = null;
      expect(r.el).toBe(null);
    });
  });
  describe("isReactive", () => {
    it("returns false for plain primitives", () => {
      expect(isReactive(5)).toBe(false);
      expect(isReactive("hi")).toBe(false);
      expect(isReactive(true)).toBe(false);
    });
    it("returns false for null and undefined", () => {
      expect(isReactive(null as unknown as number)).toBe(false);
      expect(isReactive(undefined as unknown as number)).toBe(false);
    });
    it("returns false for plain objects without .val", () => {
      expect(isReactive({ x: 1 } as unknown as number)).toBe(false);
    });
    it("returns true for signals", () => {
      expect(isReactive(signal(5))).toBe(true);
    });
    it("returns true for computeds", () => {
      expect(isReactive(computed(() => 7))).toBe(true);
    });
  });
  describe("read", () => {
    it("returns plain primitives unchanged", () => {
      expect(read(5)).toBe(5);
      expect(read("hi")).toBe("hi");
      expect(read(false)).toBe(false);
    });
    it("returns null/undefined unchanged without crashing", () => {
      expect(read(null as unknown as number)).toBe(null);
      expect(read(undefined as unknown as number)).toBe(undefined);
    });
    it("returns signal.val", () => {
      expect(read(signal(5))).toBe(5);
    });
    it("returns computed.val", () => {
      expect(read(computed(() => 7))).toBe(7);
    });
  });
  describe("uniqueId", () => {
    it("returns ids carrying the prefix", () => {
      expect(uniqueId("err").startsWith("err-")).toBe(true);
    });
    it("returns distinct ids on successive calls", () => {
      expect(uniqueId("err")).not.toBe(uniqueId("err"));
    });
  });
  describe("errorNode", () => {
    afterEach(cleanup);

    it("renders a [data-field-error] node when the signal is non-null", () => {
      const error = signal<string | null>("Required.");
      const el = render(errorNode(error, "err-a"));
      expect(find(el, "[data-field-error]")).toBeTruthy();
    });
    it("renders the error message text", () => {
      const error = signal<string | null>("Required.");
      const el = render(errorNode(error, "err-b"));
      const node = find(el, "[data-field-error]")!;
      expect((node.textContent ?? "").trim()).toBe("Required.");
    });
    it("carries the given id and the text-muted utility class", () => {
      const error = signal<string | null>("Required.");
      const el = render(errorNode(error, "err-c"));
      expect(find(el, "small#err-c.text-muted")).toBeTruthy();
    });
    it("renders nothing when the signal is null", () => {
      const error = signal<string | null>(null);
      const el = render(errorNode(error, "err-d"));
      expect(find(el, "[data-field-error]")).toBe(null);
    });
    it("renders nothing when no signal is passed", () => {
      const el = render(errorNode(undefined, "err-e"));
      expect(find(el, "[data-field-error]")).toBe(null);
    });
    it("updates reactively when the signal flips", () => {
      const error = signal<string | null>(null);
      const el = render(errorNode(error, "err-f"));
      expect(find(el, "[data-field-error]")).toBe(null);
      error.set("Now broken.");
      const node = find(el, "[data-field-error]")!;
      expect(node).toBeTruthy();
      expect((node.textContent ?? "").trim()).toBe("Now broken.");
      error.set(null);
      expect(find(el, "[data-field-error]")).toBe(null);
    });
  });
  describe("ariaInvalid", () => {
    it("returns 'true' while the signal holds a message", () => {
      const error = signal<string | null>("Broken.");
      expect(ariaInvalid(error)()).toBe("true");
    });
    it("returns 'false' for a null signal and for no signal", () => {
      expect(ariaInvalid(signal<string | null>(null))()).toBe("false");
      expect(ariaInvalid(undefined)()).toBe("false");
    });
    it("tracks the signal as it flips", () => {
      const error = signal<string | null>(null);
      const value = ariaInvalid(error);
      expect(value()).toBe("false");
      error.set("Broken.");
      expect(value()).toBe("true");
    });
  });
  describe("ariaDescribedBy", () => {
    it("returns the id while the signal holds a message", () => {
      const error = signal<string | null>("Broken.");
      expect(ariaDescribedBy(error, "err-1")()).toBe("err-1");
    });
    it("returns the empty string for a null signal and for no signal", () => {
      expect(ariaDescribedBy(signal<string | null>(null), "err-1")()).toBe("");
      expect(ariaDescribedBy(undefined, "err-1")()).toBe("");
    });
  });
  describe("debounce", () => {
    it("returns the same function reference when ms is 0", () => {
      const fn = () => {};
      expect(debounce(fn, 0)).toBe(fn);
    });
    it("returns the same function reference when ms is negative", () => {
      const fn = () => {};
      expect(debounce(fn, -5)).toBe(fn);
    });
    it("delays the call until the window elapses", async () => {
      let calls: unknown[][] = [];
      const wrapped = debounce((...args: unknown[]) => calls.push(args), 50);
      wrapped("x");
      expect(calls.length).toBe(0);
      await new Promise((r) => setTimeout(r, 80));
      expect(calls.length).toBe(1);
    });
    it("collapses calls within the window to one trailing call with the last args", async () => {
      let calls: unknown[][] = [];
      const wrapped = debounce((...args: unknown[]) => calls.push(args), 50);
      wrapped("a");
      wrapped("b");
      wrapped("c");
      await new Promise((r) => setTimeout(r, 80));
      expect(calls.length).toBe(1);
      expect(calls[0]).toEqual(["c"]);
    });
  });
});
