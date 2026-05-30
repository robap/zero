---
title: API
nav_order: 14
---

# API

A flat, search-in-page reference of every public export. The
table is hand-maintained against the `.zero/*.d.ts` files in your
project; **when this page disagrees with the type files, trust
the type files.** They are the source of truth; this page mirrors
them for human reading and may drift between releases.

For tutorial coverage of any export, follow the link in the
"Teaching" column.

## `"zero"`

Source of truth: `.zero/zero.d.ts`.

### Reactivity

| Name        | Signature                                                                         | Teaching                                           |
|-------------|-----------------------------------------------------------------------------------|----------------------------------------------------|
| `signal`    | `signal<T>(initial: T): Signal<T>`                                                | [Reactivity](./reactivity.html#what-a-signal-is)   |
| `computed`  | `computed<T>(fn: () => T): Computed<T>`                                           | [Reactivity](./reactivity.html#what-a-computed-is) |
| `effect`    | `effect(fn: () => void \| (() => void)): () => void`                              | [Reactivity](./reactivity.html#what-an-effect-is)  |
| `Signal<T>` | `interface { readonly val: T; set(v: T): void; update(fn: (c: T) => T): void; }`  | [Reactivity](./reactivity.html#what-a-signal-is)   |
| `Computed<T>` | `interface { readonly val: T; }`                                                | [Reactivity](./reactivity.html#what-a-computed-is) |

### Templates

| Name             | Signature                                                                                                          | Teaching                                          |
|------------------|--------------------------------------------------------------------------------------------------------------------|---------------------------------------------------|
| `html`           | `html(strings: TemplateStringsArray, ...values: unknown[]): TemplateResult`                                        | [Templates](./templates.html#what-html-does)      |
| `each`           | `each<T>(src: Signal<T[]> \| Computed<T[]>, render: (item: T, i: number) => TemplateResult, key?: (item, i) => string \| number): TemplateResult` | [Templates](./templates.html#each--keyed-lists)   |
| `ref`            | `ref<T = unknown>(): Ref<T>`                                                                                       | [Templates](./templates.html#ref--element-handles)|
| `commit`         | `commit(result: TemplateResult, container: Element): void`                                                         | (rare — used by tests and integrations)           |
| `Ref<T>`         | `interface { el: T \| null; }`                                                                                     | [Templates](./templates.html#ref--element-handles)|
| `TemplateResult` | opaque marker interface returned from `html`                                                                       | [Templates](./templates.html#what-html-does)      |

### State and DI

| Name      | Signature                                                                                            | Teaching                                          |
|-----------|------------------------------------------------------------------------------------------------------|---------------------------------------------------|
| `inject`  | `inject<K extends keyof StateTypes>(key: K): StateTypes[K]` / `inject<T = unknown>(key: string): T`  | [Getting Started](./getting-started.html)         |
| `StateTypes` | `interface {}` — augment with `declare module "zero" { interface StateTypes { ... } }` for typed inject | (TypeScript ergonomics)                       |

### Navigation

| Name        | Signature                                                                          | Teaching                                     |
|-------------|------------------------------------------------------------------------------------|----------------------------------------------|
| `navigate`  | `navigate(to: string, opts?: { replace?: boolean; state?: unknown }): void`        | [Routing](./routing.html#navigation)         |
| `back`      | `back(): void`                                                                     | [Routing](./routing.html#navigation)         |
| `forward`   | `forward(): void`                                                                  | [Routing](./routing.html#navigation)         |
| `route`     | `route(): RouteView`                                                               | [Routing](./routing.html#navigation)         |
| `RouteView` | `interface { path: string; params: Record<string, string>; query: Record<string, string>; }` | [Routing](./routing.html#navigation) |

### `App`

| Method                | Signature                                                                                                                          | Teaching                                            |
|-----------------------|------------------------------------------------------------------------------------------------------------------------------------|-----------------------------------------------------|
| `new App()`           | `new App()`                                                                                                                        | [Getting Started](./getting-started.html#anatomy-of-srcapptts) |
| `state`               | `state(key: string, value: unknown): this`                                                                                         | [Getting Started](./getting-started.html#anatomy-of-srcapptts) |
| `use`                 | `use(mw: (ctx: MiddlewareContext) => void \| Promise<void>): this`                                                                 | [Routing](./routing.html)                           |
| `route`               | `route(pattern: string, loaderOrComponent: (...args) => unknown, opts?: RouteOptions): this`                                       | [Routing](./routing.html#defining-routes)           |
| `layout`              | `layout(component: (props: { outlet: unknown }) => TemplateResult): this`                                                          | [Routing](./routing.html#nested-routes)             |
| `loading`             | `loading(component: () => TemplateResult): this`                                                                                   | [Routing](./routing.html#navigation-lifecycle)      |
| `error`               | `error(component: (props: { error: unknown; retry: () => void }) => TemplateResult): this`                                          | [Routing](./routing.html#navigation-lifecycle)      |
| `run`                 | `run(selector: string): void`                                                                                                      | [Getting Started](./getting-started.html#anatomy-of-srcapptts) |
| `match`               | `match(input: string): { route, params, query, pathname, search } \| null`                                                          | (rare — used by tests)                              |

Supporting types: `RouteOptions`, `RouteChildEntry`,
`MiddlewareContext` — see the `.d.ts` for fields.

## `"zero/test"`

Source of truth: `.zero/zero-test.d.ts`.

| Name             | Signature                                                                                                            | Teaching                                          |
|------------------|----------------------------------------------------------------------------------------------------------------------|---------------------------------------------------|
| `describe`       | `describe(name: string, fn: () => void): void`                                                                       | [Testing](./testing.html#structure-api)           |
| `it`             | `it(name: string, fn: TestFn): void`                                                                                 | [Testing](./testing.html#structure-api)           |
| `beforeEach`     | `beforeEach(fn: HookFn): void`                                                                                       | [Testing](./testing.html#structure-api)           |
| `afterEach`      | `afterEach(fn: HookFn): void`                                                                                        | [Testing](./testing.html#structure-api)           |
| `beforeAll`      | `beforeAll(fn: HookFn): void`                                                                                        | [Testing](./testing.html#structure-api)           |
| `afterAll`       | `afterAll(fn: HookFn): void`                                                                                         | [Testing](./testing.html#structure-api)           |
| `expect`         | `expect(actual: unknown): Matcher`                                                                                   | [Testing](./testing.html#assertions)              |
| `render`         | `render(result: TemplateResult, opts?: RenderOptions): Element`                                                      | [Testing](./testing.html#dom-helpers)             |
| `find`           | `find(el: Element, selector: string): Element \| null`                                                               | [Testing](./testing.html#dom-helpers)             |
| `findAll`        | `findAll(el: Element, selector: string): Element[]`                                                                  | [Testing](./testing.html#dom-helpers)             |
| `text`           | `text(el: Element, selector?: string): string`                                                                       | [Testing](./testing.html#dom-helpers)             |
| `fire`           | `fire(el: Element, type: string, data?: Record<string, unknown>): void`                                              | [Testing](./testing.html#dom-helpers)             |
| `cleanup`        | `cleanup(): void`                                                                                                    | [Testing](./testing.html#dom-helpers)             |
| `spy`            | `spy(): SpyFn` / `spy<T extends (...a) => any>(impl: T): SpyFn<T>`                                                   | [Testing](./testing.html#spies)                   |
| `Matcher`        | interface; see [Testing § Assertions](./testing.html#assertions) for the matcher table                               | [Testing](./testing.html#assertions)              |
| `SpyFn`          | callable with `calls`, `callCount`, `results`, `instances`, plus `mockReturnValue`/`mockResolvedValue`/`mockRejectedValue`/`mockImplementation`/`reset` | [Testing](./testing.html#spies) |

## `"zero/http"`

Source of truth: `.zero/zero-http.d.ts`.

| Name              | Signature                                                                                                         | Teaching                                                            |
|-------------------|-------------------------------------------------------------------------------------------------------------------|---------------------------------------------------------------------|
| `createHttp`      | `createHttp(opts?: { fetch?: typeof fetch }): HttpClient`                                                          | [HTTP § Constructing a client](./http.html#constructing-a-client)   |
| `HttpClient`      | `interface { use, get, post, put, patch, delete, request }`                                                       | [HTTP § Constructing a client](./http.html#constructing-a-client)   |
| `HttpClient.use`  | `use(mw: Middleware): HttpClient`                                                                                  | [HTTP § Middleware](./http.html#middleware)                         |
| `HttpClient.get`  | `get<T = unknown>(url: string, init?: HttpInit): Promise<T>`                                                       | [HTTP](./http.html#json-io)                                         |
| `HttpClient.post` | `post<T = unknown>(url: string, body?: unknown, init?: HttpInit): Promise<T>`                                      | [HTTP](./http.html#json-io)                                         |
| `HttpClient.put`  | `put<T = unknown>(url: string, body?: unknown, init?: HttpInit): Promise<T>`                                       | [HTTP](./http.html#json-io)                                         |
| `HttpClient.patch`| `patch<T = unknown>(url: string, body?: unknown, init?: HttpInit): Promise<T>`                                     | [HTTP](./http.html#json-io)                                         |
| `HttpClient.delete`| `delete<T = unknown>(url: string, init?: HttpInit): Promise<T>`                                                   | [HTTP](./http.html#json-io)                                         |
| `HttpClient.request`| `request<T = unknown>(input: Request \| URL \| string, init?: HttpInit): Promise<T>`                             | [HTTP § Constructing a client](./http.html#constructing-a-client)   |
| `HttpInit`        | `interface HttpInit extends RequestInit { fetch?: typeof fetch; }`                                                | [HTTP § Route-scoped fetch threading](./http.html#route-scoped-fetch-threading) |
| `Middleware`      | `(req: Request, next: (req: Request) => Promise<Response>) => Promise<Response>`                                   | [HTTP § Middleware](./http.html#middleware)                         |
| `HttpError`       | `class HttpError extends Error { readonly status, readonly statusText, readonly body; }`                           | [HTTP § Errors](./http.html#errors)                                 |

## `"zero/components"`

Source of truth:
`.zero/components.d.ts`.

| Component  | Signature                                                                | Teaching                                                                  |
|------------|--------------------------------------------------------------------------|---------------------------------------------------------------------------|
| `Avatar`   | `Avatar(props: AvatarProps): TemplateResult`                             | [Components § Component library reference](./components.html#component-library-reference) |
| `Badge`    | `Badge(props?: BadgeProps): TemplateResult`                              | [Components § Component library reference](./components.html#component-library-reference) |
| `Button`   | `Button(props?: ButtonProps): TemplateResult`                            | [Components § Component library reference](./components.html#component-library-reference) |
| `Card`     | `Card(props?: CardProps): TemplateResult`                                | [Components § Component library reference](./components.html#component-library-reference) |
| `Checkbox` | `Checkbox(props: CheckboxProps): TemplateResult`                         | [Components § Component library reference](./components.html#component-library-reference) |
| `Dialog`   | `Dialog(props: DialogProps): TemplateResult`                             | [Components § Component library reference](./components.html#component-library-reference) |
| `Input`    | `Input(props: InputProps): TemplateResult`                               | [Components § Component library reference](./components.html#component-library-reference) |
| `Radio`    | `Radio(props: RadioProps): TemplateResult`                               | [Components § Component library reference](./components.html#component-library-reference) |
| `Select`   | `Select(props: SelectProps): TemplateResult`                             | [Components § Component library reference](./components.html#component-library-reference) |
| `Spinner`  | `Spinner(props?: SpinnerProps): TemplateResult`                          | [Components § Component library reference](./components.html#component-library-reference) |
| `Table`    | `Table<T>(props: TableProps<T>): TemplateResult`                         | [Components § Component library reference](./components.html#component-library-reference) |
| `Tabs`     | `Tabs(props: TabsProps): TemplateResult`                                 | [Components § Component library reference](./components.html#component-library-reference) |
| `TextArea` | `TextArea(props: TextAreaProps): TemplateResult`                         | [Components § Component library reference](./components.html#component-library-reference) |
| `Toast`    | `Toast(props: ToastProps): TemplateResult`                               | [Components § Component library reference](./components.html#component-library-reference) |
| `Toggle`   | `Toggle(props: ToggleProps): TemplateResult`                             | [Components § Component library reference](./components.html#component-library-reference) |

Each component's `Props` type and any associated literal-type
aliases (`ButtonVariant`, `ButtonType`, `InputType`, `BadgeSize`, `SelectOption`,
`TableColumn<T>`, `TabsTab`, etc.) live in
`.zero/components.d.ts` — that file is the canonical, typed
reference.

## `"zero/wc"`

Reserved for the web-components escape hatch. Not yet exposed at
runtime; the `C02` lint rule directs users here when they try
`customElements.define(...)` directly.
