# Plan: `zero/test` selector grammar + spy primitive

## Summary

Two additive changes to the test-API surface. First, replace the
dom-shim's two-clause selector matcher (`#id` or bare-tag only) with a
compound matcher that accepts any concatenation of tag, `#id`, `.class`,
`[attr]`, and `[attr=value]` simple selectors against a single element.
Second, grow `runtime/test.js` with a `spy(impl?)` primitive (Vitest-
shaped: `.calls` / `.callCount` / `.results` / `.instances` plus
`.mockReturnValue` / `.mockResolvedValue` / `.mockRejectedValue` /
`.mockImplementation` / `.reset`) and four matchers that consume it
(`toHaveBeenCalled`, `toHaveBeenCalledTimes`, `toHaveBeenCalledWith`,
`toHaveBeenLastCalledWith`).

The implementation uses a small hand-written tokenizer for the selector
parser (option (a) from the open questions) so a future combinators
slice can grow from the same code, and reuses the existing `_deepEqual`
and `_pretty` helpers for spy-arg matching and failure messages. New
exports register in `src/runtime.rs::ZERO_TEST_EXPORTS` so the
embedded test-API module surfaces them; ambient types in
`runtime/zero-test.d.ts` grow to match.

## Prerequisites

None. The spec's open questions are resolved in-spec (recommendations
accepted): hand-written tokenizer, error messages with offending
substring + position, case-insensitive attribute-name compare, defer
partial-arg sentinels, `zero/test`-only export of `spy`, loose
`SpyFn<T>` typing, document function-arg identity behavior, no `*`
selector in this slice.

## Steps

- [x] **Step 1: Compound selector parser + matcher in `dom-shim.js`**
- [x] **Step 2: `spy()` primitive in `test.js`**
- [x] **Step 3: Spy matchers in `expect()`**
- [x] **Step 4: Ambient types for `spy` and matchers in `zero-test.d.ts`**
- [x] **Step 5: Spec text amendments (test-runner spec + framework spec)**

---

## Step Details

### Step 1: Compound selector parser + matcher in `dom-shim.js`

**Goal:** Make `find` / `findAll` / `closest` / `text(el, selector)`
accept compound selectors so tests can target by class, attribute, and
combinations without resorting to `id=` attributes on production
markup. Leaves the dom-shim's API contract identical (still
`_matchSelector(node, selector) → boolean`), only the accepted grammar
grows.

**Files:**
- `runtime/dom-shim.js` (modify: replace `_matchSelector`)
- `runtime/test.test.js` (add: a `describe("selector grammar", …)` block
  that exercises `find` / `findAll` / `closest` / `text(el, selector)`
  with every grammar element)
- `runtime/dom-shim.test.js` (no functional change required — the
  existing `#id` and tag-name tests must continue to pass as regression
  coverage)

**Changes:**

1. **New helpers in `dom-shim.js`** (above `_matchSelector`):

   - `_parseSelector(selector)` — pure function, no DOM access.
     Tokenizes a compound selector into an array of simple-selector
     descriptors. Returns:
     ```js
     {
       tag: string | null,            // lowercase, or null if no tag part
       id: string | null,             // exact id, or null
       classes: string[],             // collected `.cls` tokens
       attrs: Array<{ name: string, value: string | null }>,
                                       // value === null means existence-only
     }
     ```
     Empty input throws
     `Error('dom-shim: empty selector')`. Any other malformed input
     throws `Error('dom-shim: malformed selector "<input>" at position
     <i> (<reason>)')`.

     Tokenizer algorithm (single left-to-right pass over the string
     with an index `i`):
     - At `i === 0`, if the first char is `[a-zA-Z]`, read a tag run
       matching `[a-zA-Z][a-zA-Z0-9]*` and lowercase it; store as
       `tag`. (Leading whitespace is malformed.)
     - Loop:
       - `#` → read an identifier run `[a-zA-Z0-9_-]+`, store as
         `id`. Empty run after `#` → malformed (reason: "expected id
         after #"). A second `#` → malformed (reason: "duplicate
         id").
       - `.` → read an identifier run `[a-zA-Z0-9_-]+`, push onto
         `classes`. Empty run → malformed (reason: "expected class
         name after .").
       - `[` → read attribute clause until the matching `]`.
         - Read attribute-name run `[a-zA-Z][a-zA-Z0-9_:-]*`,
           lowercased (spec recommendation: case-insensitive
           attribute names).
         - If next char is `]`, push `{ name, value: null }`.
         - If next char is `=`:
           - If next-next char is `"`, read up to the closing `"`
             (no escape handling — simple slice); value is the
             enclosed substring. Then expect `]`.
           - Else if next-next char is `'`, mirror with single
             quotes.
           - Else read unquoted value as `[^\]]+` (no `]` chars);
             that value is the slice.
           - Push `{ name, value }`. Expect `]`.
         - Any deviation from this shape (unclosed bracket, missing
           `=`/`]`, empty name) → malformed with reason string.
       - End-of-string → done.
       - Any other character → malformed (reason: "unexpected
         character '<c>'").

     The parser deliberately does **not** support whitespace,
     combinators (` `, `>`, `+`, `~`), pseudo-classes (`:foo`),
     pseudo-elements (`::foo`), attribute operators other than `=`
     (`~=`, `^=`, etc.), or the universal selector (`*`). Each of
     these triggers the "unexpected character" path; the spec is
     explicit that they're out of scope.

   - `_matchSelector(node, selector)` — keep the same signature.
     Implementation:
     ```js
     function _matchSelector(node, selector) {
       const parsed = _parseSelector(selector);
       if (node.nodeType !== ELEMENT_NODE) return false;
       if (parsed.tag != null) {
         if (node.tagName == null || node.tagName.toLowerCase() !== parsed.tag) return false;
       }
       if (parsed.id != null) {
         if (!node.getAttribute || node.getAttribute('id') !== parsed.id) return false;
       }
       if (parsed.classes.length > 0) {
         const cls = node.getAttribute ? node.getAttribute('class') : null;
         if (cls == null) return false;
         const tokens = cls.split(/\s+/).filter(Boolean);
         for (const c of parsed.classes) if (!tokens.includes(c)) return false;
       }
       for (const { name, value } of parsed.attrs) {
         if (!node.hasAttribute || !node.hasAttribute(name)) return false;
         if (value != null && node.getAttribute(name) !== value) return false;
       }
       return true;
     }
     ```

   Per-call parsing is fine for the test workloads we run (cache is a
   micro-optimization for a follow-up).

2. **No edits to call sites.** `Element#querySelector`,
   `Element#querySelectorAll`, `Element#closest`, and the
   `document.querySelector(All)` pair all still call `_matchSelector`
   unchanged; they automatically inherit the new grammar.

3. **Error messages.** Each parser error includes the original input,
   the position (0-based index into the string), and a reason
   substring. Existing call sites already let the error propagate.

**Tests** (added to `runtime/test.test.js` under a new
`describe("selector grammar", () => { … })`):

For each, build the markup with `render(html`…`)` and assert via
`find` / `findAll` / `closest` / `text(el, selector)`. Use
`afterEach(cleanup)` so render scopes don't leak.

- Class match: `find(el, ".foo")` matches an element with
  `class="foo"` and `class="foo bar"`; returns `null` for none.
- Multi-class: `find(el, ".foo.bar")` requires both; missing one
  returns `null`. Order independence: `.foo.bar` and `.bar.foo`
  both match a `class="foo bar"` element.
- Attribute existence: `find(el, "[data-x]")` matches when the
  attribute is set to any value (including empty).
- Attribute equality unquoted: `find(el, "[data-x=y]")`.
- Attribute equality double-quoted: `find(el, '[data-x="y z"]')`
  matches `data-x="y z"` (value contains a space).
- Attribute equality single-quoted: `find(el, "[data-x='y z']")`
  matches the same.
- Compound with tag + class + attr:
  `find(el, "button.btn[type=submit]")` matches
  `<button class="btn" type="submit">` and not `<button class="btn">`.
- Compound with tag + id + class + attr existence:
  `find(el, "a#home.nav-link[data-active]")`.
- Order independence within a compound: `.btn.btn-primary` and
  `.btn-primary.btn` both match `class="btn btn-primary"`.
- `findAll` returns all matches in document order for a class
  selector.
- `closest("div.box")` walks up the tree and honors the compound;
  returns the nearest ancestor element matching every part.
- Empty selector throws `/empty selector/`.
- Malformed selector — at least three cases — throw with the input
  and position in the message:
  - `find(el, ".foo[")` (unclosed bracket).
  - `find(el, "##id")` (duplicate id).
  - `find(el, " a")` (leading whitespace).
- Tag-only (`find(el, "a")`) and `#id` (`find(el, "#nav")`)
  regressions still pass.
- Attribute-name case-insensitivity:
  `find(el, "[DATA-X=y]")` matches an element with
  `data-x="y"` (lowercased internally; matches real-DOM behavior).

`runtime/dom-shim.test.js` does **not** need new tests in this step —
its existing `#id`, tag, and `closest` cases cover the
back-compat path. (If something there breaks, that's a regression
signal worth chasing.)

---

### Step 2: `spy()` primitive in `test.js`

**Goal:** Give tests a first-class way to record calls and stub return
values without hand-rolling closures or counters. The primitive is
exported from `"zero/test"` only; nothing about it touches the
production runtime bundle.

**Files:**
- `runtime/test.js` (modify: add `spy` export)
- `src/runtime.rs` (modify: append `"spy"` to `ZERO_TEST_EXPORTS`)
- `runtime/test.test.js` (modify: new `describe("spy primitive", …)`
  block)

**Changes:**

1. **New export in `runtime/test.js`** (placed below `cleanup`, above
   the `// --- Assertions ---` divider so the assertion section
   stays cleanly separated):

   ```js
   /**
    * Mark used by spy matchers to duck-type spies in `expect()`.
    * @internal
    */
   const _SPY = Symbol("zero/test:spy");

   /**
    * Create a spy function. Records every call (args, result, thrown
    * error, `this`-binding) and optionally forwards to `impl`.
    * @template {(...args: any[]) => any} T
    * @param {T} [impl]
    * @returns {T & { calls: any[][], callCount: number, results: Array<{ type: "return" | "throw", value: unknown }>, instances: unknown[], mockReturnValue(v: unknown): any, mockResolvedValue(v: unknown): any, mockRejectedValue(e: unknown): any, mockImplementation(fn: (...a: any[]) => any): any, reset(): any }}
    */
   export function spy(impl) {
     let _impl = impl;
     function fn(...args) {
       fn.calls.push(args);
       fn.instances.push(this);
       if (_impl == null) {
         fn.results.push({ type: "return", value: undefined });
         return undefined;
       }
       try {
         const value = _impl.apply(this, args);
         fn.results.push({ type: "return", value });
         return value;
       } catch (e) {
         fn.results.push({ type: "throw", value: e });
         throw e;
       }
     }
     fn.calls = [];
     fn.results = [];
     fn.instances = [];
     Object.defineProperty(fn, "callCount", {
       get() { return fn.calls.length; },
       enumerable: true,
     });
     Object.defineProperty(fn, _SPY, { value: true });
     fn.mockReturnValue = v => { _impl = () => v; return fn; };
     fn.mockResolvedValue = v => { _impl = () => Promise.resolve(v); return fn; };
     fn.mockRejectedValue = e => { _impl = () => Promise.reject(e); return fn; };
     fn.mockImplementation = newImpl => { _impl = newImpl; return fn; };
     fn.reset = () => {
       fn.calls.length = 0;
       fn.results.length = 0;
       fn.instances.length = 0;
       return fn;
     };
     return fn;
   }
   ```

   Notes that justify the shape:
   - `callCount` is a getter (not a frozen number) so it always
     mirrors `calls.length`; cheaper than re-syncing on every push.
   - `_impl` is captured in a closure so swapping it via
     `mockImplementation` / `mockReturnValue` doesn't break the
     spy's identity (the function reference stays stable — important
     when the spy is already passed around).
   - `.reset()` matches Vitest's `mockClear` (clears history, keeps
     implementation). A stricter `mockReset` that also drops the
     implementation is not in this slice; the spec defers it.
   - The `_SPY` brand symbol is not exported; matchers detect spies
     by checking `Array.isArray(actual?.calls)` (the duck-type the
     spec calls out, identical in style to `toBeTemplateResult`'s
     `_template`/`_values` check), and the brand is a redundant
     internal signal that doesn't pollute the public surface.

2. **`src/runtime.rs::ZERO_TEST_EXPORTS`** — append `"spy"`. The
   trailing `export { … }` block in `test_module()` then includes
   `spy`; the `test_module_ends_with_aggregate_export_block_for_test_exports`
   test (already present in that file) covers the regression.

**Tests** (in `runtime/test.test.js`, new
`describe("spy primitive", () => { … })`):

- `spy()` returns a function whose `typeof` is `"function"`. Calling
  it returns `undefined`; subsequent `.calls` / `.callCount`
  reflect the invocation.
- `spy(fn)` calls through to `fn`, returns its value, and records
  the args in `.calls[0]` and a `{ type: "return", value }` entry
  in `.results[0]`.
- `spy(fn)` rethrows when `fn` throws and records a
  `{ type: "throw", value: <error> }` in `.results`.
- `.mockReturnValue(v)`: subsequent calls return `v`; the original
  impl (if any) no longer runs. `.calls` still accumulates.
- `.mockResolvedValue(v)`: subsequent call returns a Promise that
  resolves to `v` (assert via `await`).
- `.mockRejectedValue(e)`: subsequent call returns a Promise that
  rejects with `e` (assert via `await assert.rejects`).
- `.mockImplementation(fn2)`: subsequent calls use `fn2`. `.calls`
  accumulates across impl swaps (assert by calling once before swap
  and once after).
- `.reset()` empties `.calls`, `.results`, `.instances`; the
  current implementation persists (call after reset still goes
  through the post-swap impl).
- `this`-binding: invoke the spy via `obj.method = spy(); obj.method()`
  and assert `spy.instances[0] === obj`.

---

### Step 3: Spy matchers in `expect()`

**Goal:** Make assertions about spies read naturally
(`expect(spy).toHaveBeenCalledWith(...)`) and produce useful failure
messages that print all recorded calls. The matchers reuse
`_deepEqual` for arg comparison, so semantics align exactly with
`.toEqual`.

**Files:**
- `runtime/test.js` (modify: extend the object returned by `expect()`)
- `runtime/test.test.js` (modify: new `describe("spy matchers", …)`
  block)

**Changes:**

1. **Inside `expect()` in `runtime/test.js`**, add four matchers to
   the returned object. The implementation pattern is consistent with
   the existing matchers — synchronous, throws `Error` with a
   pretty-printed message on failure.

   Add a small private helper at the top of `expect()` (before the
   return) for the duck-type:

   ```js
   const _isSpy = v => v != null && typeof v === "function" && Array.isArray(v.calls);
   ```

   New matchers (added inside the returned object literal):

   ```js
   toHaveBeenCalled() {
     if (!_isSpy(actual)) {
       throw new Error(`expect(...).toHaveBeenCalled: value is not a spy`);
     }
     if (actual.callCount === 0) {
       throw new Error(
         `expect(spy).toHaveBeenCalled(): spy was not called`,
       );
     }
   },
   toHaveBeenCalledTimes(n) {
     if (!_isSpy(actual)) {
       throw new Error(`expect(...).toHaveBeenCalledTimes: value is not a spy`);
     }
     if (actual.callCount !== n) {
       throw new Error(
         `expect(spy).toHaveBeenCalledTimes(${n}): spy was called ${actual.callCount} time(s)\n` +
         `  calls: ${_pretty(actual.calls)}`,
       );
     }
   },
   toHaveBeenCalledWith(...expectedArgs) {
     if (!_isSpy(actual)) {
       throw new Error(`expect(...).toHaveBeenCalledWith: value is not a spy`);
     }
     const hit = actual.calls.some(args => _deepEqual(args, expectedArgs));
     if (!hit) {
       throw new Error(
         `expect(spy).toHaveBeenCalledWith(${expectedArgs.map(a => _pretty(a)).join(", ")}): no recorded call matched\n` +
         `  recorded calls (${actual.callCount}): ${_pretty(actual.calls)}`,
       );
     }
   },
   toHaveBeenLastCalledWith(...expectedArgs) {
     if (!_isSpy(actual)) {
       throw new Error(`expect(...).toHaveBeenLastCalledWith: value is not a spy`);
     }
     if (actual.callCount === 0) {
       throw new Error(
         `expect(spy).toHaveBeenLastCalledWith(...): spy was never called`,
       );
     }
     const lastArgs = actual.calls[actual.callCount - 1];
     if (!_deepEqual(lastArgs, expectedArgs)) {
       throw new Error(
         `expect(spy).toHaveBeenLastCalledWith(${expectedArgs.map(a => _pretty(a)).join(", ")}): last call did not match\n` +
         `  last call args: ${_pretty(lastArgs)}`,
       );
     }
   },
   ```

   The duck-type check (`_isSpy`) deliberately requires `actual.calls`
   to be an array — that's the contract documented in the spec. A
   plain function with a manually-attached `.calls = []` would pass;
   that's intentional and matches how `toBeTemplateResult` accepts
   any object with the right shape.

2. **Update the `expect()` JSDoc return type** so it lists the four
   new matcher names. Match the style of the existing inline
   `@returns` annotation.

**Tests** (in `runtime/test.test.js`, new
`describe("spy matchers", () => { … })`):

- `toHaveBeenCalled` passes after a single invocation; throws on a
  fresh spy with `/spy was not called/`.
- `toHaveBeenCalledTimes(2)` passes after exactly two calls; throws
  off-by-one with the expected number (`2`) and the actual
  (`callCount`) both in the message.
- `toHaveBeenCalledWith(a, b)` passes when exactly one recorded call
  matches; passes when any of multiple recorded calls match (call
  the spy three times, assert against the args of the middle
  call); fails when no recorded call matches and the failure message
  contains the recorded calls (assert via substring on
  `_pretty([...])`-shaped output).
- `toHaveBeenLastCalledWith(...)` passes against the last call only.
  Earlier matching args do **not** satisfy it (call with `[1]`, then
  `[2]`, then assert `toHaveBeenLastCalledWith(1)` throws).
- All four throw `/value is not a spy/` (or the matcher-specific
  equivalent error from `_isSpy`) when given a non-spy actual.
- Deep-equality semantics inherit `.toEqual`: assert
  `toHaveBeenCalledWith({ a: 1 })` against a recorded
  `[{ a: 1 }]` call. (One sanity test is enough — full deep-equal
  coverage is `_deepEqual`'s job.)

---

### Step 4: Ambient types for `spy` and matchers in `zero-test.d.ts`

**Goal:** Make the new exports surface in editor autocomplete and
type-check cleanly for downstream TS code. Loose typing per the open-
question recommendation: prioritize "the property is there" over
"this `.mockReturnValue(v)` enforces `v` matches `T`'s return type."

**Files:**
- `runtime/zero-test.d.ts` (modify)

**Changes:**

1. **Add `SpyFn<T>` interface** after the existing `HookFn` type
   alias:

   ```ts
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
   ```

   The default type-parameter (`= (...args: any[]) => any`) keeps
   `const s: SpyFn = spy()` working without explicit generics.

2. **Add `spy` export** near `cleanup`:

   ```ts
   export function spy(): SpyFn;
   export function spy<T extends (...args: any[]) => any>(impl: T): SpyFn<T>;
   ```

   The overload pair lets `spy()` (no arg) resolve to the default
   `SpyFn` while `spy(fn)` infers `Parameters<typeof fn>` /
   `ReturnType<typeof fn>` for `.calls` and the call signature.

3. **Extend `Matcher`** with the four new methods. Args are loose
   (`unknown[]`) for the same reason — strict typing of variadic args
   buys little here and complicates downstream call sites:

   ```ts
   export interface Matcher {
     // …existing members…
     toHaveBeenCalled(): void;
     toHaveBeenCalledTimes(n: number): void;
     toHaveBeenCalledWith(...args: unknown[]): void;
     toHaveBeenLastCalledWith(...args: unknown[]): void;
   }
   ```

4. **No changes to `runtime/zero.d.ts`.** The spec is explicit that
   `spy` lives in `"zero/test"` only; importing `spy` from `"zero"`
   should keep failing.

**Tests:** No dedicated runtime tests for this step (the `.d.ts` file
is editor-side only and has no JS-side behavior). The existing
Rust-side test
`zero_test_types_body_declares_every_public_test_export` in
`src/runtime.rs` will fail if `spy` is added to `ZERO_TEST_EXPORTS`
(Step 2) but not declared in this `.d.ts` — so this step's edits keep
that test green.

---

### Step 5: Spec + agent-reference text amendments

**Goal:** Keep the spec docs and the user-facing scaffold reference
honest about what has shipped. Three files need surgical edits — no
rewrites.

**Files:**
- `issues/test-runner/spec.md` (modify)
- `zero-framework-spec.md` (modify)
- `src/scaffold/AGENTS.md` (modify — this is the authoritative API
  reference written into every newly-scaffolded project by `zero init`;
  downstream developers and their LLM assistants read this file, not
  the internal specs, so it must list `spy` and the new matchers or
  they're effectively invisible to users)

**Changes:**

1. **`issues/test-runner/spec.md`** — two edits:

   - In the "Out of Scope" section, replace
     `Mocking utilities (spies, stubs, module mocks).`
     with
     `Mocking utilities (module mocks, deep stubs). Spies ship in
     the `zero/test` selector-grammar + spy slice; see
     `issues/test-helpers/spec.md`.`
   - In the DOM-helpers section that currently says
     `The dom-shim currently supports tag-name and \`#id\` selectors;
     anything else throws from the shim.` (around the description of
     `find`), replace with a sentence noting the slice now supports
     compound selectors (tag, id, class, attribute existence,
     attribute equality) and that combinators / pseudo-classes /
     other attribute operators are still deferred, with a pointer to
     `issues/test-helpers/spec.md`.

   Both edits are content-only; no schema/structure changes.

2. **`zero-framework-spec.md`** — two edits:

   - Section 8 ("Testing"), under the `import` example for
     `"z/test"`, append `spy` to both the structure-API import and
     the matcher list. (The current file uses `z/test` as the
     module path — keep that; the slice does not rename the
     module.) The relevant lines today list
     `describe, it, expect, beforeEach, afterEach, beforeAll,
     afterAll` for the structure API and don't enumerate matchers
     by name; add a short paragraph (~3 lines) after "Testing
     Components" that calls out `spy` plus the four spy matchers
     with a one-line usage example. The example mirrors the
     "Testing Components" style:

     ```ts
     it("calls onSelect on click", () => {
       const onSelect = spy()
       const el = render(Button({ onSelect }))
       fire(find(el, "button"), "click")
       expect(onSelect).toHaveBeenCalledTimes(1)
     })
     ```

   - Section 11 ("Phase 5 — Test Runner") — under the existing
     checklist, add two unchecked items right after the
     `render()` / `find()` / `text()` / `fire()` / `cleanup()` line:
     `- [x] Compound selector grammar in dom-shim` and
     `- [x] `spy()` primitive + spy matchers (`toHaveBeenCalled`,
     `toHaveBeenCalledTimes`, `toHaveBeenCalledWith`,
     `toHaveBeenLastCalledWith`)`. (Both ticked because this slice
     ships them.)

3. **`src/scaffold/AGENTS.md`** — four edits to keep the user-facing
   API reference in sync. The file is loaded into every scaffolded
   project, so omissions show up as "the docs say this doesn't
   exist" in downstream PRs.

   - **Imports block** (around lines 65–69): append `spy` to the
     `"zero/test"` import example:
     ```ts
     import {
       describe, it, expect,
       beforeAll, afterAll, beforeEach, afterEach,
       render, find, findAll, text, fire, cleanup, spy,
     } from "zero/test";
     ```

   - **`### DOM helpers`** (the bullet list around lines 672–676):
     extend the `find` / `findAll` bullets to call out the supported
     selector grammar so readers don't have to guess. Replace the
     two lines with:
     ```
     - `find(el, selector)` / `findAll(el, selector)` —
       `querySelector` / `querySelectorAll` on the lightweight DOM.
       Selectors compose tag, `#id`, `.class`, `[attr]`, and
       `[attr=value]` (quoted or unquoted) parts against a single
       element (e.g. `button.btn[type=submit]`). Combinators
       (descendant, child, sibling), pseudo-classes, and attribute
       operators beyond `=` are not supported.
     ```

   - **`### Assertions`** (the matcher list around lines 682–689):
     after the `.toBeTemplateResult()` bullet and before the
     `.toMatchSnapshot()` paragraph, insert a new sub-block for spy
     matchers:
     ```
     - `.toHaveBeenCalled()` — `actual` must be a spy. Passes if the
       spy recorded at least one call.
     - `.toHaveBeenCalledTimes(n)` — passes if the spy was called
       exactly `n` times. Failure message includes recorded
       `callCount` and the full call log.
     - `.toHaveBeenCalledWith(...args)` — passes if any recorded
       call's args deep-equal `args` (same algorithm as `.toEqual`).
     - `.toHaveBeenLastCalledWith(...args)` — passes if only the
       most recent call's args deep-equal `args`.
     ```

   - **New subsection `### Spies`** between
     `### Testing components` and `### Testing reactivity directly`.
     The body explains the primitive, lists properties / methods,
     and gives one usage example aligned with the AGENTS.md style:
     ```
     ### Spies

     `spy(impl?)` returns a callable that records every invocation.
     Pass it as a prop, callback, or argument anywhere a function is
     expected; assertions about how it was called use the
     `toHaveBeenCalled*` matchers above.

     ```js
     import { it, expect, spy, render, find, fire, cleanup, afterEach } from "zero/test";
     import Button from "./Button.ts";

     afterEach(cleanup);

     it("calls onSelect on click", () => {
       const onSelect = spy();
       const el = render(Button({ label: "Go", onSelect }));
       fire(find(el, "button"), "click");
       expect(onSelect).toHaveBeenCalledTimes(1);
       expect(onSelect).toHaveBeenLastCalledWith();
     });
     ```

     Properties on a spy (all live, read every call):

     - `.calls` — array of argument-arrays, one per invocation.
     - `.callCount` — `calls.length`.
     - `.results` — array of `{ type: "return" | "throw", value }`,
       one per invocation.
     - `.instances` — array of `this`-bindings observed.

     Methods (all return the spy for chaining):

     - `.mockReturnValue(v)` — subsequent calls return `v`.
     - `.mockResolvedValue(v)` — subsequent calls return
       `Promise.resolve(v)`.
     - `.mockRejectedValue(e)` — subsequent calls return
       `Promise.reject(e)`.
     - `.mockImplementation(fn)` — replace the underlying impl.
     - `.reset()` — clear `.calls`, `.results`, `.instances`. The
       implementation is preserved; if you need a fresh impl too,
       construct a new spy.

     Spies are plain values, not registered resources. `cleanup()`
     does **not** reset them — wire a `beforeEach` if a spy is
     shared across tests in a `describe`.
     ```

   The four edits are content-only; no structural changes to the
   surrounding sections.

**Tests:** No automated tests for these markdown edits — the
verification is a `git diff` review plus a `grep -n "spy\|toHaveBeenCalled"`
across all three files to confirm coverage. The Rust-side
`zero_test_types_body_declares_every_public_test_export` test
(unchanged by this step) doubles as a tripwire: if Step 2 adds `spy`
to `ZERO_TEST_EXPORTS` and the `.d.ts` lacks it, that test fails — a
separate signal from the markdown drift problem.

---

## Risks and Assumptions

- **Parser robustness.** The hand-written tokenizer is small and the
  spec's grammar is intentionally tight. If a real test in the
  scaffold lands on a selector shape that's syntactically odd but
  reasonable (e.g., a class name with a digit prefix the regex
  rejects), the parser's "malformed" error mode is the right place to
  loosen — the change is local and additive. Risk: the
  identifier-run regex (`[a-zA-Z0-9_-]+`) doesn't accept Unicode
  identifiers. Accepted: real-world CSS identifiers in test code are
  ASCII; if Unicode is needed later, widen the run.
- **Attribute-name case folding.** The matcher lowercases attribute
  names on both sides of the compare. This matches real-DOM behavior
  but does not match the dom-shim's storage layer (which stores the
  name as passed to `setAttribute`). A test that sets
  `el.setAttribute("DATA-X", "y")` would write `DATA-X` into the map
  and `_matchSelector` would call `hasAttribute("data-x")`, which
  returns `false`. Accepted: real test code uses lowercase
  attribute names, and this is consistent with how browsers behave
  in HTML mode. If it bites, the fix is to lowercase in
  `setAttribute` / `getAttribute` / `hasAttribute` — but that's a
  dom-shim change beyond this slice.
- **Spy identity stability.** Tests sometimes capture the spy
  function reference and pass it around (e.g.,
  `el.addEventListener("click", spy)`). `mockImplementation` swaps
  the internal `_impl` closure variable without replacing the
  function object, so the captured reference stays valid. If a
  future change moves implementation onto the function object
  itself, this property must be preserved.
- **`_deepEqual` with function args.** A test that does
  `expect(spy).toHaveBeenCalledWith(handler)` where `handler` is a
  function will only pass if the recorded arg is the **same**
  function reference (`_deepEqual` falls through to `===` for
  non-objects-with-`val`-getters, and functions are not plain
  objects). Accepted per the spec's open-question
  recommendation; if real tests hit friction, widen `_deepEqual`'s
  function handling or add a `expect.any(Function)` sentinel in a
  follow-up.
- **`runtime/zero-test.d.ts` is "auto-managed".** The file's banner
  says `Auto-managed by zero dev and zero init`. Hand-editing it
  works for the source-tree copy, but if the CLI overwrites it on
  scaffold, downstream apps would temporarily see a stale ambient
  surface. Accepted: the file is regenerated from the very source
  we're editing (`runtime/zero-test.d.ts`); the regeneration would
  re-emit the new content. Verify after Step 4 by spot-reading
  `src/scaffold` / wherever the CLI writes the file to confirm it
  uses `runtime/zero-test.d.ts` as the source-of-truth (it does —
  `build.rs` embeds it as `ZERO_TEST_TYPES_BODY`).
- **No new Rust dependencies, no `Cargo.toml` changes.** All Rust-
  side movement is one string-array push in `src/runtime.rs`. If
  this slice grows scope (e.g., a `--spy-trace` flag), revisit; it
  shouldn't.
