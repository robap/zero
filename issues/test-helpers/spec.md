# Spec: `zero/test` selector grammar + spy primitive

## Problem Statement

Two real gaps in the existing `zero/test` API surface block the
scaffold and downstream apps from writing the tests they want to write:

1. **`find` / `findAll` / `text(el, selector)` / `closest` only support
   `#id` and bare-tag selectors.** The dom-shim's `_matchSelector` throws
   on anything else, so a test that wants to assert "the active link has
   class `nav-active`" or "find the submit button by `[type=submit]`"
   can't be expressed. Developers are forced to add `id="…"` attributes
   to production markup purely to make tests work, or fall back to
   walking `childNodes` by hand. Both options are bad enough that real
   test files in the scaffold stay shallow.

2. **No spy / mock primitive exists.** A test that wants to assert "the
   route loader called `fetch` with `/api/users/42`" or "clicking the
   button invoked the `onSelect` prop" has to hand-roll a counter
   variable and capture args via closures. The `test-runner` spec
   explicitly defers "Mocking utilities (spies, stubs, module mocks)"
   to a later slice — this is that slice (for spies; module mocks
   remain deferred).

Both gaps push test authors toward awkward workarounds that pollute
production code or produce brittle, hard-to-read tests. Fixing them is
small in scope, additive to the API surface, and unblocks the patterns
real tests in the scaffold and downstream apps already want to use.

## Background

### Where the relevant code lives

- `runtime/dom-shim.js` — owns the `_matchSelector(node, selector)`
  helper used by `querySelector`, `querySelectorAll`, and `closest`
  (lines ~6–14 today). The shim is shared between the runtime's own
  `node:test` suite and `zero test` under Boa. Any selector parser we
  add lives here so all three call sites benefit from one source of
  truth.
- `runtime/test.js` — exports `find`, `findAll`, `text`, `fire`,
  `render`, `cleanup`, `expect`. New exports (`spy`) and new matchers
  (`.toHaveBeenCalled*`) land here.
- `runtime/zero-test.d.ts` — the ambient declarations for the
  `zero/test` virtual module. Editor-side type surface must grow to
  match the new exports.
- `runtime/test.test.js` — `node:test` self-tests for the test API.
  Both new pieces grow their own coverage here.
- `src/runtime.rs` / `build.rs` — embed `runtime/test.js` into the
  binary verbatim. No changes expected; the new exports ride along.

### Current selector grammar

```js
function _matchSelector(node, selector) {
  if (selector.startsWith('#')) {
    return node.getAttribute != null && node.getAttribute('id') === selector.slice(1);
  }
  if (/^[a-z][a-z0-9]*$/.test(selector)) {
    return node.tagName != null && node.tagName.toLowerCase() === selector;
  }
  throw new Error(`dom-shim: unsupported selector "${selector}"`);
}
```

Anything beyond `#id` or `tag` throws. The error is loud, which is good
(no silent miss), but the developer has no path forward except adding
an id.

### Why a compound grammar (not full CSS)

The decision is to support tag, id, class, attribute existence, and
attribute equality — composed into a single compound selector
(e.g. `a#nav.active[href="/"]`). Combinators (descendant `a b`,
child `a > b`, sibling `+`/`~`) and pseudo-classes (`:nth-child`,
`:not`) are explicitly **out of scope** for this slice.

Rationale: a real CSS selector parser plus matcher is hundreds of
lines and produces a maintenance commitment ("we said we support CSS,
why doesn't `:has()` work?"). Compound selectors against a single
element cover ~95% of what tests actually need to express, while
remaining trivially implementable as "split into simple selectors,
each element must match all parts." If a follow-up needs combinators,
the parser produced here is the right place to grow them.

### Why `spy(impl?)` (not `spy()` + `stub()`)

The user picked the single-primitive model (Vitest/Jest's `vi.fn()`
shape) over sinon's split spy/stub. Rationale: one mental model, one
factory, no second concept to teach. The primitive both records calls
and lets you intercept return values. Documentation and tests don't
have to explain when to use which.

### Why dedicated `expect` matchers for spies

`expect(spy.callCount).toBe(2)` works but reads poorly compared to
`expect(spy).toHaveBeenCalledTimes(2)`. Test failure messages also
get better: a dedicated matcher can print all recorded calls when an
arg comparison fails, while `expect([…]).toEqual([…])` just shows the
two arrays. The matcher count grows by four — small, additive, and
aligned with how every mainstream JS test framework names these.

### Why no `replace(obj, key, spy)` helper

The user picked option 1 (just the spy primitive) over option 3 (spy
plus a method-replacement helper). For the scaffold and the current
runtime, passing a spy as a prop / argument is sufficient — the
codebase already uses dependency injection for `fetch` and similar
("loaders receive `fetch` as a parameter for testability" per the
framework spec §8). If a real test later needs to spy on an imported
singleton's method, a `replace(obj, key, spy)` helper that
auto-restores on `cleanup()` can be added in a follow-up without
breaking anything that ships in this slice.

## Requirements

### Selector grammar (in `runtime/dom-shim.js`)

Replace `_matchSelector` with a compound-selector matcher that
accepts a single selector string containing one or more of these
**simple selectors**, concatenated with no whitespace:

- **Type:** `tag` (case-insensitive, e.g. `a`, `button`, `div`).
- **ID:** `#id` (exact match against the `id` attribute).
- **Class:** `.cls` (matches if the `class` attribute's
  whitespace-separated token list contains `cls`). Multiple class
  selectors compose: `.a.b` requires both.
- **Attribute existence:** `[name]` (element has the attribute).
- **Attribute equality:** `[name=value]`, `[name="value"]`,
  `[name='value']` (attribute exists and equals `value` exactly).
  Values may be unquoted (no spaces, no closing-bracket characters),
  double-quoted, or single-quoted; quotes are stripped.

A compound selector is the concatenation of any number of simple
selectors in any order, with optional leading tag. Examples that
must match:

- `a`
- `#nav`
- `.btn`
- `.btn.btn-primary`
- `button.btn[type=submit]`
- `a#home.nav-link[data-active]`
- `[data-testid="row-42"]`

The matcher returns `true` iff every component of the compound
selector matches the candidate node. An empty selector string
throws `Error("dom-shim: empty selector")`. Malformed input (e.g.
unclosed bracket, double `#`, leading whitespace) throws a clear
error from the parser, **not** silently false-matches.

### Selector application sites

The new `_matchSelector` is used by every existing selector
consumer in the shim:

- `Element#querySelector`
- `Element#querySelectorAll`
- `Element#closest`
- `document.querySelector` / `document.querySelectorAll`

Because `runtime/test.js` `find`, `findAll`, and `text(el, selector)`
delegate to these, all four test helpers inherit the new grammar
automatically. **No combinators** — selectors only match a single
element; there is no walk-from-root parser. Descendant matching for
`querySelectorAll` still works because the walker visits every
descendant and applies the matcher to each in isolation.

### `spy()` primitive (in `runtime/test.js`)

Add a new export:

```js
export function spy(impl?: (...args: any[]) => any): SpyFn
```

`spy()` returns a callable function (the "spy") with the following
behavior and properties:

- **Calling the spy** records the call (args, return value, thrown
  error, this-binding) and:
  - If a current implementation is set, calls it and returns its
    result (or rethrows).
  - Otherwise, returns `undefined`.
- **Initial implementation:** if `spy(fn)` is called, `fn` becomes
  the implementation. `spy()` with no argument starts with no
  implementation (returns `undefined`).
- **Properties** (all enumerable, plain data):
  - `.calls` — array of arg arrays. `spy.calls[i]` is the args of
    the i-th call.
  - `.callCount` — `spy.calls.length`. Cached getter or plain
    number, plan's call.
  - `.results` — array of result records, one per call:
    `{ type: "return", value }` or `{ type: "throw", value }`.
    Mirrors Vitest's `mock.results`.
  - `.instances` — array of `this` bindings observed (mainly for
    constructor-spy use; nice-to-have).
- **Methods:**
  - `.mockReturnValue(v)` — replace implementation with `() => v`.
    Returns the spy for chaining.
  - `.mockResolvedValue(v)` — replace implementation with
    `() => Promise.resolve(v)`. Returns the spy.
  - `.mockRejectedValue(e)` — replace implementation with
    `() => Promise.reject(e)`. Returns the spy.
  - `.mockImplementation(fn)` — replace implementation. Returns
    the spy.
  - `.reset()` — clear `calls`, `results`, `instances`. Does **not**
    reset the implementation (matches Vitest's `mockClear`; if a
    `mockReset`-style "also clear implementation" is needed later
    it can be added separately).

The spy is a function (typeof `spy()` === "function"), so it can be
passed anywhere a function is expected. Inspect properties land on
the function object directly.

The spy is **not** tied to `cleanup()` — it's a local value. If a
test stores it in a module-level `let`, the developer is responsible
for resetting it across tests (typically via `beforeEach`).

### Spy matchers (in `runtime/test.js`'s `expect()`)

Add four matchers, all of which throw if `actual` is not a spy
(detected via duck-typing on `.calls` being an array — same shape
as `toBeTemplateResult`'s check):

- `.toHaveBeenCalled()` — passes if `spy.callCount > 0`.
- `.toHaveBeenCalledTimes(n)` — passes if `spy.callCount === n`.
- `.toHaveBeenCalledWith(...args)` — passes if **any** recorded
  call's args deep-equal the provided args. Deep equality uses the
  same `_deepEqual` helper that backs `.toEqual`.
- `.toHaveBeenLastCalledWith(...args)` — passes if the **last**
  recorded call's args deep-equal the provided args.

Failure messages must include:

- The matcher name.
- The expected args (for `*CalledWith`).
- All recorded calls (or the relevant call), pretty-printed via the
  existing `_pretty` helper.
- The recorded `callCount` (for `*CalledTimes` mismatches).

### `zero/test` ambient types (`runtime/zero-test.d.ts`)

Extend the declarations:

- New export: `spy<T extends (...a: any[]) => any>(impl?: T): SpyFn<T>`.
- New interface `SpyFn<T>` that is callable (matches `T`'s signature
  if provided, else `(...args: any[]) => any`) and has `calls`,
  `callCount`, `results`, `instances`, plus the `mock*` and `reset`
  methods.
- Matcher interface grows `toHaveBeenCalled`, `toHaveBeenCalledTimes`,
  `toHaveBeenCalledWith`, `toHaveBeenLastCalledWith`.

The plan picks exact type-parameter shapes; the goal is reasonable
editor autocomplete, not perfect inference of variadic args.

### Self-tests (`runtime/test.test.js`)

Grow the existing `node:test` suite to cover:

**Selector grammar:**

- `find(el, ".foo")` matches an element with `class="foo"`.
- Multi-class: `find(el, ".foo.bar")` requires both classes; missing
  one returns null.
- Attribute existence: `find(el, "[data-x]")`.
- Attribute equality: `find(el, "[data-x=y]")`,
  `find(el, '[data-x="y z"]')`, `find(el, "[data-x='y z']")`.
- Compound: `find(el, "button.btn[type=submit]")`.
- Order independence within a compound: `.btn.btn-primary` and
  `.btn-primary.btn` both match.
- `findAll` returns all matches in document order.
- `closest("div.box")` walks up the tree honoring the compound.
- Empty / malformed selectors throw a clear error.
- Tag-only and `#id` continue to work (regression).

**Spy primitive:**

- `spy()` returns a function; calling it returns `undefined`;
  `.calls` and `.callCount` track args.
- `spy(fn)` calls through to `fn` and records its return value.
- `spy(fn)` rethrows errors from `fn` and records a `throw` result.
- `.mockReturnValue(v)` overrides; subsequent calls return `v`.
- `.mockResolvedValue` / `.mockRejectedValue` return promises that
  settle as expected.
- `.mockImplementation(fn2)` replaces the impl; `.calls` continues
  to accumulate across impl swaps.
- `.reset()` clears `calls`/`results` but keeps implementation.
- `this`-binding is recorded in `.instances`.

**Spy matchers:**

- `toHaveBeenCalled` passes after one call, fails on a fresh spy.
- `toHaveBeenCalledTimes(n)` passes exact match, fails off-by-one
  with both numbers in the message.
- `toHaveBeenCalledWith(a, b)` deep-matches a single call; passes
  if any of multiple calls match; fails with all calls printed.
- `toHaveBeenLastCalledWith` checks the last call only; an earlier
  matching call does not satisfy it.
- All matchers throw a clear error if `actual` is not a spy.

### Spec text amendments

The `test-runner` spec
(`issues/test-runner/spec.md`) currently lists "Mocking utilities
(spies, stubs, module mocks)" under Out of Scope. This slice
amends that line to "Mocking utilities (module mocks, deep stubs)"
and adds a one-line forward reference to the new spec. The same
file also notes the dom-shim's selector limitations; that note is
updated to reflect the new grammar.

The framework spec
(`zero-framework-spec.md` §8 and §11) lists the test API exports.
Update both lists to include `spy` and the four new matchers.

## Constraints

- **No combinators in the selector grammar.** Descendant
  (`a b`), child (`a > b`), sibling (`+`/`~`) are out. Future spec.
- **No pseudo-classes** (`:nth-child`, `:not`, `:has`, etc.).
  Future spec.
- **Attribute operators are `=` only.** No `~=`, `|=`, `^=`, `$=`,
  `*=`. If a real test needs one, the parser is the right place to
  grow it — but defer until that test exists.
- **No `replace(obj, key, spy)` helper.** Pass spies through props /
  arguments / DI. Method-replacement is a future slice if needed.
- **No module mocking.** Spies don't intercept `import`s. The test
  must own the call-site (e.g. by passing a `fetch` spy into a
  loader).
- **No `expect(spy).not.toHaveBeenCalled*` negation.** The existing
  `expect` API has no `.not` chain; this slice does not add one.
  Users compose with `toBe(false)` against a derived predicate if
  they really need negation, or wait for a future `.not` slice.
- **Selector matching is per-element only.** The walker handles
  descendant traversal; the parser/matcher never sees more than one
  candidate at a time.
- **`spy()` is not auto-cleaned by `cleanup()`.** Spies are values,
  not registered resources. Developers reset them in `beforeEach`
  if they share spies across tests.
- **No async spy assertions** (`.toHaveBeenCalledAfter`, etc.). Out
  of scope.
- **`.toEqual`'s existing deep-equality algorithm is the single
  source of truth** for `toHaveBeenCalledWith` arg matching. No
  separate matcher engine.

## Out of Scope

- CSS combinators (descendant, child, sibling).
- Pseudo-classes and pseudo-elements.
- Attribute operators other than `=`.
- Negation matchers (`expect(...).not.toBe*`).
- Module mocking (`jest.mock("./foo")` style).
- Stubbing methods on imported singletons via a `replace` helper.
- Timer mocks (`vi.useFakeTimers()` equivalent).
- Snapshot testing — unchanged, still deferred per the test-runner
  spec.
- Auto-restoring spies via `cleanup()`.
- Spies that proxy through to ESM module bindings.
- A `vi`-like global namespace.
- Coverage instrumentation for spied code paths.

## Open Questions

- **Selector parser implementation strategy.** Two viable shapes:
  (a) a small hand-written tokenizer that splits a compound into
  simple-selector descriptors; (b) a regex-based extractor that
  pulls `#…`, `.…`, `[…]`, leading tag out of the string. (a) is
  more maintainable if combinators are added later; (b) is shorter.
  Plan picks; lean (a).
- **Error message format for malformed selectors.** Plan-level
  decision. Recommendation: include the offending substring and
  the position. Example: `dom-shim: malformed selector ".foo[" at
  position 4 (unclosed attribute bracket)`.
- **Case sensitivity for attribute names.** HTML attributes are
  ASCII-case-insensitive in real DOM; the shim stores them as set
  by `setAttribute` (which `String()`'s the value but keeps the
  name's case). Recommendation: compare attribute names
  case-insensitively in the matcher, since that matches real-DOM
  behavior and avoids `data-FOO` vs `data-foo` surprises.
- **Should `.toHaveBeenCalledWith` support partial-arg matching
  via a sentinel** (e.g. `expect.anything()`)? Vitest/Jest do this.
  Recommendation: defer; not needed for the patterns the scaffold
  uses. If added later, it's purely additive.
- **Should `spy()` be re-exported from `"zero"` as well as
  `"zero/test"`?** Recommendation: `zero/test` only. Spies are a
  test concern; keeping them out of the production runtime bundle
  preserves the framework's small-runtime promise.
- **`SpyFn<T>` type ergonomics in `.d.ts`.** Strict typing of
  `.mockReturnValue(v)` to require `v` match `T`'s return type
  costs complexity. Recommendation: type the surface loosely
  (`unknown`/`any`) in the ambient declarations; revisit when a
  real test reports friction.
- **Deep-equality of spy args containing functions.** A test might
  do `spy(handler); el.dispatchEvent(...); expect(spy).toHaveBeenCalledWith(eventObj)`.
  Functions compared via `_deepEqual` currently fall back to
  identity. Document this behavior; no change needed.
- **What does `find(el, "*")` do?** Universal selector isn't in the
  grammar above. Recommendation: not supported in this slice;
  throws "unsupported selector". Add in a follow-up if a real test
  needs it.
