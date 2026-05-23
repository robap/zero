// Auto-managed by `zero dev` and `zero init`. Editing this file by hand
// will lose changes the next time the CLI runs.

declare module "zero/test" {
  import type { TemplateResult } from "zero";

  export type TestFn = () => void | Promise<void>;
  export type HookFn = () => void | Promise<void>;

  export interface SpyFn<T extends (...args: any[]) => any = (...args: any[]) => any> {
    (...args: Parameters<T>): ReturnType<T>;
    calls: Array<Parameters<T>>;
    callCount: number;
    results: Array<{ type: "return" | "throw"; value: unknown }>;
    instances: unknown[];
    mockReturnValue(value: unknown): SpyFn<T>;
    mockResolvedValue(value: unknown): SpyFn<T>;
    mockRejectedValue(error: unknown): SpyFn<T>;
    mockImplementation(fn: (...args: any[]) => any): SpyFn<T>;
    reset(): SpyFn<T>;
  }

  export function describe(name: string, fn: () => void): void;
  export function it(name: string, fn: TestFn): void;
  export function beforeEach(fn: HookFn): void;
  export function afterEach(fn: HookFn): void;
  export function beforeAll(fn: HookFn): void;
  export function afterAll(fn: HookFn): void;

  export interface NegatedMatcher {
    toBe(expected: unknown): void;
    toEqual(expected: unknown): void;
    toBeTruthy(): void;
    toBeFalsy(): void;
    toBeNull(): void;
    toBeUndefined(): void;
    toBeDefined(): void;
    toContain(expected: unknown): void;
    toThrow(message?: string): void;
    toBeTemplateResult(): void;
    toMatchSnapshot(): void;
    toHaveBeenCalled(): void;
    toHaveBeenCalledTimes(n: number): void;
    toHaveBeenCalledWith(...args: unknown[]): void;
    toHaveBeenLastCalledWith(...args: unknown[]): void;
    toBeGreaterThan(n: number): void;
    toBeGreaterThanOrEqual(n: number): void;
    toBeLessThan(n: number): void;
    toBeLessThanOrEqual(n: number): void;
  }

  export interface Matcher {
    toBe(expected: unknown): void;
    toEqual(expected: unknown): void;
    toBeTruthy(): void;
    toBeFalsy(): void;
    toBeNull(): void;
    toBeUndefined(): void;
    toBeDefined(): void;
    toContain(expected: unknown): void;
    toThrow(expected?: unknown): void;
    toBeTemplateResult(): void;
    toMatchSnapshot(): void;
    toHaveBeenCalled(): void;
    toHaveBeenCalledTimes(n: number): void;
    toHaveBeenCalledWith(...args: unknown[]): void;
    toHaveBeenLastCalledWith(...args: unknown[]): void;
    toBeGreaterThan(n: number): void;
    toBeGreaterThanOrEqual(n: number): void;
    toBeLessThan(n: number): void;
    toBeLessThanOrEqual(n: number): void;
    not: NegatedMatcher;
  }

  export function expect(actual: unknown): Matcher;

  export interface RenderOptions {
    state?: Record<string, unknown>;
  }

  export function render(
    result: TemplateResult,
    opts?: RenderOptions,
  ): Element;
  export function find(el: Element, selector: string): Element | null;
  export function findAll(el: Element, selector: string): Element[];
  export function text(el: Element, selector?: string): string;
  export function fire(
    el: Element,
    type: string,
    data?: Record<string, unknown>,
  ): void;
  export function cleanup(): void;
  export function spy(): SpyFn;
  export function spy<T extends (...args: any[]) => any>(impl: T): SpyFn<T>;
}
