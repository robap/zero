// Auto-managed by `zero dev` and `zero init`. Editing this file by hand
// will lose changes the next time the CLI runs.

declare module "zero/test" {
  import type { TemplateResult } from "zero";

  export type TestFn = () => void | Promise<void>;
  export type HookFn = () => void | Promise<void>;

  export function describe(name: string, fn: () => void): void;
  export function it(name: string, fn: TestFn): void;
  export function beforeEach(fn: HookFn): void;
  export function afterEach(fn: HookFn): void;
  export function beforeAll(fn: HookFn): void;
  export function afterAll(fn: HookFn): void;

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
}
