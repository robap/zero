---
title: Testing
nav_order: 8
---

# Testing

`zero test` is the framework's test runner — the same binary that
runs framework internals. It runs `.test.ts` / `.test.js` /
`.spec.{ts,js}` files in a Boa-powered JS environment with an
in-memory DOM and a curated set of web platform shims. No browser
launch, no jsdom dependency, no Node.

## Running tests

```sh
zero test                       # discover and run all tests under the project root
zero test home                  # run tests whose path or name matches "home"
zero test src/routes/home       # path-prefix
zero test --watch               # re-run on file change
zero test --coverage            # write coverage to coverage/
zero test --update-snapshots    # accept current output as new snapshots
```

`zero mutate` is a sibling command that runs the same suite under
mutation testing — see [Config and CLI](./config-and-cli.html#zero-mutate).

## Structure API

`describe`, `it`, and four hooks. All of them are imported from
the bare specifier `"zero/test"`:

```ts
import { describe, it, beforeEach, afterEach, beforeAll, afterAll } from "zero/test";

describe("Counter", () => {
  beforeEach(() => { /* runs before every it() in this block */ });
  afterEach(() => { /* runs after every it() — cleanup() lives here */ });
  beforeAll(() => { /* runs once before the block's tests */ });
  afterAll(() => { /* runs once after the block's tests */ });

  it("increments", () => { /* ... */ });
});
```

`describe` may be nested. Each block has its own scope for hooks.

## Assertions

`expect(actual)` returns a matcher with these methods:

| Matcher                             | Passes when…                                            |
|-------------------------------------|---------------------------------------------------------|
| `.toBe(expected)`                   | `Object.is(actual, expected)`                           |
| `.toEqual(expected)`                | Deep structural equality                                |
| `.toBeTruthy()` / `.toBeFalsy()`    | Truthy / falsy by JS rules                              |
| `.toBeNull()` / `.toBeUndefined()`  | Strict equality to `null` / `undefined`                 |
| `.toBeDefined()`                    | Not `undefined`                                         |
| `.toContain(expected)`              | Array `.includes` or string `.includes`                 |
| `.toThrow(expected?)`               | Function throws; optional message/class match           |
| `.toBeGreaterThan(n)`               | `actual > n` (numeric)                                  |
| `.toBeGreaterThanOrEqual(n)`        | `actual >= n` (numeric)                                 |
| `.toBeLessThan(n)`                  | `actual < n` (numeric)                                  |
| `.toBeLessThanOrEqual(n)`           | `actual <= n` (numeric)                                 |
| `.toHaveBeenCalled()`               | Spy has been called at least once                       |
| `.toHaveBeenCalledTimes(n)`         | Spy has been called exactly `n` times                   |
| `.toHaveBeenCalledWith(...args)`    | Spy was called with these args at some point            |
| `.toHaveBeenLastCalledWith(...args)`| Spy's most recent call matched these args               |
| `.not.<matcher>(...)`               | The matcher would fail — inverts every check above      |

```ts
expect(2 + 2).toBe(4);
expect({ a: 1 }).toEqual({ a: 1 });
expect(() => parseInt("nope") || throwIt()).toThrow();
expect(onClick).toHaveBeenCalledTimes(1);
expect(score).toBeGreaterThan(0);
expect(badge).not.toContain("error");
```

`.not` is a single chain prefix — `.not.not` is not supported, and
`.not.toMatchSnapshot()` keeps the same "not implemented" error as
`.toMatchSnapshot()`. `.not.toThrow(msg)` mirrors Jest: passes if the
function either does not throw, or throws an error whose message does
**not** contain `msg`.

## DOM helpers

The render-side helpers all come from `"zero/test"`:

```ts
import { render, find, findAll, text, fire, cleanup } from "zero/test";

const el = render(Counter());

find(el, "button");             // first <button>, or null
findAll(el, "li");              // every <li>
text(el);                       // textContent of el
text(el, "h1");                 // textContent of the first <h1> within el
fire(button, "click");          // dispatch a click event
fire(input, "input", { target: { value: "ada" } }); // synthesised event data

cleanup();                      // tear down mounted templates
```

Wire `cleanup()` into `afterEach` so per-test mounts don't leak:

```ts
afterEach(cleanup);
```

## Testing signals

You can drive `signal` / `computed` directly, with no render:

```ts
import { signal, computed } from "zero";
import { describe, it, expect } from "zero/test";

it("recomputes on dependency change", () => {
  const price = signal(10);
  const tax   = computed(() => price.val * 0.2);

  expect(tax.val).toBe(2);
  price.set(20);
  expect(tax.val).toBe(4);
});
```

## Testing components

```ts
import { render, find, fire, text } from "zero/test";

it("increments on click", () => {
  const el = render(Counter());
  expect(text(el, "p")).toBe("Count: 0");
  fire(find(el, "button")!, "click");
  expect(text(el, "p")).toBe("Count: 1");
});
```

### Effects in route and component bodies

A top-level `effect()` in a route or component module body has no
enclosing scope, so `cleanup()` disposes it between tests to prevent
stale subscriptions from re-firing once the test app is torn down.

The trade-off: a route relying on a top-level `effect()` for runtime
behavior will lose that effect after the first `cleanup()` call within
the same test file. Put effects inside the function called by
`render()` (the component's exported factory) so they live in the
render scope and re-fire each test:

```ts
// Bad — runs once at module load, disposed by cleanup() between tests:
effect(() => syncWithServer(stateSignal.val));
export default function Home() { return html`…`; }

// Good — runs inside each render's scope, refires every test:
export default function Home() {
  effect(() => syncWithServer(stateSignal.val));
  return html`…`;
}
```

## Testing routes

`render()` doesn't invoke `load()` — `load` runs in the router's
own scope at navigation time. To test a route component, seed the
store the route reads from and render the component directly:

```ts
import { render, text } from "zero/test";
import { signal } from "zero";
import Profile from "../src/routes/profile.ts";

it("shows the user name", () => {
  const user = signal({ name: "Ada" });
  const el = render(Profile({ data: { user: user.val }, params: {}, query: {} }));
  expect(text(el, "h1")).toBe("Ada");
});
```

For the `inject()` path, pass the seeded store via `render`'s
`opts.state`:

```ts
const el = render(Profile(), { state: { user: signal({ name: "Ada" }) } });
```

## In-memory DOM

`zero test` ships its own lightweight DOM. It implements the
slice of the standard that real apps and the framework's
templates depend on:

- Real `Event` constructors (`Event`, `MouseEvent`,
  `KeyboardEvent`, `InputEvent`, `SubmitEvent`, `FocusEvent`)
  with proper bubble/capture semantics — `fire()` exercises the
  full path so `@click.stop`, `@click.prevent`, and capture-phase
  listeners behave as in a browser.
- `classList`, `dataset`, `style` (full `CSSStyleDeclaration`),
  `attributes`, full Node tree.
- `localStorage` and `sessionStorage`.
- `window.matchMedia` (returns a stub `MediaQueryList`).
- `navigator` (subset — `userAgent`, `language`, `clipboard`,
  `serviceWorker` stub).
- `crypto` (subset — `getRandomValues`, `randomUUID`,
  `subtle.digest` available with a clear-error fallback).
- `IntersectionObserver`, `ResizeObserver`, `MutationObserver`
  with manual fire APIs (`observer.fire(...)`).
- `requestAnimationFrame`, `cancelAnimationFrame`,
  `setTimeout`, `clearTimeout`, `setInterval`, `clearInterval` —
  the runner exposes `tick(ms)` to advance simulated time.

## Web Platform surface

In addition to the DOM, `zero test` ships hand-written
implementations of the Web Platform APIs that real apps and the
framework itself reach for. Anything on this list is on
`globalThis` as soon as a test file imports `"zero/test"`;
anything not on this list is outside scope and throws a
`ReferenceError`.

**Fetch API**

- `Headers` — case-insensitive,
  `get`/`set`/`has`/`delete`/`append`/`forEach`/iteration.
- `Request` / `Response` — constructors, `text()` / `json()`.
  Streaming bodies (`arrayBuffer()`, `blob()`) reject with a
  clear stub message.
- `fetch` — default rejects with: "zero test:
  globalThis.fetch is not implemented. Stub it in your test's
  beforeEach (or pass init.fetch to the call) — see
  runtime/http.test.js for the pattern." `cleanup()` restores
  the default after each test.
- `AbortController` / `AbortSignal` — full standard shape,
  including `AbortSignal.abort(reason)`,
  `AbortSignal.timeout(ms)`, `AbortSignal.any([...])`.

**URLs**

- `URL` — constructor, getters/setters, `searchParams`,
  `toString`, `URL.canParse`.
- `URLSearchParams` — constructor from string/object/array/
  instance, all standard methods.

**Encoding**

- `TextEncoder` / `TextDecoder` — UTF-8 only. No `encodeInto`,
  no streaming `decode`. `new TextDecoder('latin1')` throws a
  clear stub message.

**Binary data**

- `Blob`, `File`, `FormData` — constructors and core methods.
  `new FormData(htmlForm)` is not supported and throws.

**Cloning & scheduling**

- `structuredClone` — plain objects/arrays/Date/RegExp/Map/Set/
  Error/ArrayBuffer/typed arrays. Functions, DOM nodes, Promises
  throw a `DataCloneError`-shaped error.
- `queueMicrotask`, `Promise.withResolvers`.

**The "clear error" discipline.** Any API the shim installs but
does not implement throws an error of the form `"zero test:
<API> is not implemented. <one-sentence action the user can
take>."` That is the only way gaps surface as actionable
messages; everything else outside this list surfaces as
`ReferenceError`.

**Out of scope** (restated): streaming APIs (`ReadableStream`,
`WritableStream`, `TransformStream`), `WebSocket` /
`EventSource`, Web Workers, `IndexedDB`, `SubtleCrypto.digest`,
`Notifications`, `Geolocation`, `MediaDevices`, `WebRTC`. Reach
for them inside a test and stub them yourself.

## Spies

`spy()` returns a callable that records every invocation:

```ts
import { spy } from "zero/test";

it("calls onSelect on click", () => {
  const onSelect = spy();
  const el = render(Button({ onClick: onSelect, children: "x" }));
  fire(find(el, "button")!, "click");
  expect(onSelect).toHaveBeenCalledTimes(1);
});
```

Spies are also the right shape for asserting calls to Web APIs.
Swap the API on `globalThis`, then put it back:

```ts
import { spy } from "zero/test";

const setItem = spy();
const original = localStorage.setItem;
localStorage.setItem = setItem;
afterEach(() => { localStorage.setItem = original; });
```

A spy with an implementation passes the underlying behaviour
through while still recording:

```ts
const fn = spy((n: number) => n * 2);
fn(3);                           // returns 6
expect(fn).toHaveBeenLastCalledWith(3);
```

## E2E tests

End-to-end browser tests are out of scope for `zero test` — use
Playwright or a similar tool against a `zero build` output (or a
running `zero dev`). `zero test` is for unit and integration
tests against the in-memory DOM.

---

→ Next: [Theming](./theming.html) — design tokens, layout
primitives, and the brand-theme workflow.

## See also

→ [Best Practices §9 Testing](./best-practices.html#9-testing)
