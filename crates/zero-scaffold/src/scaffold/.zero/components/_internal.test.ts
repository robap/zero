import { describe, it, expect } from "zero/test";
import { signal, computed } from "zero";
import { isReactive, read, debounce } from "./_internal.ts";

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
