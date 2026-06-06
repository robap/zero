import { describe, it, expect, afterEach } from "zero/test";
import { render, find, cleanup } from "zero/test";
import { signal, computed } from "zero";
import {
  isReactive,
  read,
  debounce,
  uniqueId,
  errorNode,
  ariaInvalid,
  ariaDescribedBy,
} from "./_internal.ts";

describe("_internal", () => {
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
