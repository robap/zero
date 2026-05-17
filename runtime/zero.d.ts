// Auto-managed by `zero dev` and `zero init`. Editing this file by hand
// will lose changes the next time the CLI runs.

declare module "zero" {
  export interface Signal<T> {
    readonly val: T;
    set(value: T): void;
    update(fn: (current: T) => T): void;
  }

  export interface Computed<T> {
    readonly val: T;
  }

  export interface Ref<T = unknown> {
    el: T | null;
  }

  export interface TemplateResult {
    readonly __zero_template_result?: never;
  }

  export interface RouteView {
    path: string;
    params: Record<string, string>;
    query: Record<string, string>;
  }

  export function signal<T>(initial: T): Signal<T>;
  export function computed<T>(fn: () => T): Computed<T>;
  export function effect(fn: () => void | (() => void)): () => void;

  export interface StateTypes {}

  export function inject<K extends keyof StateTypes>(key: K): StateTypes[K];
  export function inject<T = unknown>(key: string): T;

  export function html(
    strings: TemplateStringsArray,
    ...values: unknown[]
  ): TemplateResult;
  export function commit(result: TemplateResult, container: Element): void;
  export function each<T>(
    source: Signal<T[]> | Computed<T[]>,
    render: (item: T, index: number) => TemplateResult,
    key?: (item: T, index: number) => string | number,
  ): TemplateResult;
  export function ref<T = unknown>(): Ref<T>;

  export function navigate(
    to: string,
    opts?: { replace?: boolean; state?: unknown },
  ): void;
  export function back(): void;
  export function forward(): void;
  export function route(): RouteView;

  export interface RouteChildEntry {
    path: string;
    load: (...args: unknown[]) => unknown;
    children?: RouteChildEntry[];
    guard?: (...args: unknown[]) => unknown;
    meta?: Record<string, unknown>;
    loading?: () => TemplateResult;
    error?: (props: { error: unknown; retry: () => void }) => TemplateResult;
  }

  export interface RouteOptions {
    children?: RouteChildEntry[];
    guard?: (...args: unknown[]) => unknown;
    load?: (...args: unknown[]) => unknown;
    meta?: Record<string, unknown>;
    loading?: () => TemplateResult;
    error?: (props: { error: unknown; retry: () => void }) => TemplateResult;
  }

  export interface MiddlewareContext {
    route: RouteView;
    state: Record<string, unknown>;
    redirect: (path: string) => void;
  }

  export class App {
    constructor();
    state(key: string, value: unknown): this;
    use(mw: (ctx: MiddlewareContext) => void | Promise<void>): this;
    route(
      pattern: string,
      loaderOrComponent: (...args: unknown[]) => unknown,
      opts?: RouteOptions,
    ): this;
    layout(component: (props: { outlet: unknown }) => TemplateResult): this;
    loading(component: () => TemplateResult): this;
    error(
      component: (props: { error: unknown; retry: () => void }) => TemplateResult,
    ): this;
    run(selector: string): void;
    match(input: string): {
      route: unknown;
      params: Record<string, string>;
      query: Record<string, string>;
      pathname: string;
      search: string;
    } | null;
  }
}
