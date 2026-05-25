# Spec: `zero/test` matcher .d.ts ↔ runtime drift

## Problem Statement

Two matchers are declared on the `zero/test` public type surface but do
not exist at runtime. From `zero_demo/FRAMEWORK_NOTES.md` (2026-05-24):

> `zero-test.d.ts` lists both `toBeDefined()` and
> `NegatedMatcher.toBeUndefined()`; calling either errors with
> `TypeError: not a callable function`. Forces falling back to
> `expect(x === undefined).toBe(false)`. Likely other matchers in the
> new set (e.g. `toBeTemplateResult`, `toMatchSnapshot`) suffer the
> same drift — `.d.ts` and the JS runtime were updated out of sync as
> part of the `.not` chain landing.

The friction-log's broader guess turns out to be wrong on inspection:
`toBeTemplateResult` and `toMatchSnapshot` are both implemented in
`runtime/test.js`. The drift is narrower than feared but still
silently lies to anyone reading the type surface — TypeScript users
get green type-checks for assertions that throw at runtime, and the
documented matcher table in `docs/testing.md` advertises the same
two non-existent matchers.

Two deliverables in this slice:

1. **Fix the drift.** Implement `toBeUndefined()` and `toBeDefined()`
   in both the positive and `.not` matcher tables.
2. **Stop it recurring.** Add a self-test that parses
   `runtime/zero-test.d.ts` and asserts every method declared on the
   `Matcher` and `NegatedMatcher` interfaces is present as a function
   on the runtime objects. The next time a matcher is added to the
   `.d.ts` without a runtime implementation (or vice-versa), this
   test fails loudly.

## Background

### Where the drift lives

- `runtime/zero-test.d.ts:30-50` (`NegatedMatcher`) and `52-73`
  (`Matcher`) — both interfaces declare `toBeUndefined(): void` and
  `toBeDefined(): void`.
- `runtime/test.js:456-577` (`_buildPositive`) and `589-708`
  (`_buildNegative`) — neither table contains a `toBeUndefined` or
  `toBeDefined` method.
- `docs/testing.md:57-58` — documents the matchers as available,
  describing `.toBeNull() / .toBeUndefined()` as "Strict equality to
  `null` / `undefined`" and `.toBeDefined()` as "Not `undefined`".

### How the drift came in

`git show 35d6628` (the "added to test api" commit that landed the
`.not` chain and the numeric comparators) added the `NegatedMatcher`
interface with every matcher mirrored — including `toBeUndefined` /
`toBeDefined`, which were already declared on `Matcher` from an
earlier change. The runtime implementation in `runtime/test.js`
gained `.not` mirrors and the four numeric matchers in the same
commit, but neither `_buildPositive` nor `_buildNegative` was
extended with `toBeUndefined` / `toBeDefined`. The omission slipped
because there is no test for the two matchers and no structural
check that the `.d.ts` and the runtime stay in sync.

### Existing matcher patterns to follow

`_buildPositive` (`runtime/test.js:456`) is a plain object literal
whose methods call `_fail()` on failure. The closest analogs are
`toBeNull` (line 474) and `toBeTruthy` / `toBeFalsy` — single-argument
predicates that compare `actual` against a fixed value.

`_buildNegative` (`runtime/test.js:589`) wraps the positive matcher
via `_negate(() => positive.toX(...), <negated message>, f)` where
`f` is `_captureUserFrame()`. The pattern is uniform; the new
negated matchers should follow it.

`expect()` (`runtime/test.js:719-723`) wires the two tables together:
the positive object gains a `.not` property whose value is the
negative object.

### Drift-guard mechanism

Per the user's choice, the guard is a JS self-test in
`runtime/test.test.js` that:

1. Reads `runtime/zero-test.d.ts` (the file is in the repo; the
   test harness runs from the workspace root so a relative path
   works) using the JS-side file I/O surface already available in
   the harness — `import { readFileSync } from "fs"` is **not**
   available; the runtime is Boa with the zero web-platform shim.

   The harness already exposes a way to read files: the
   `zero-test-runner` discovery path reads files via Rust before
   handing JS to Boa. For self-test purposes the cleanest approach
   is to add a JS-callable helper backed by Rust — but that's
   heavier than the slice needs.

   The lighter, self-contained alternative: embed the `.d.ts`
   contents as a string literal in the test file. That defeats the
   point — the test must read the live file or it can't catch
   drift.

   **Recommended:** add a small Rust-backed function on the test
   harness, exposed as `__readFile__(path)` (sibling of the
   existing `__getTestTree__()` ABI), that reads a workspace-relative
   path and returns its contents as a string. Used only by the
   matcher-drift self-test; not part of the public `zero/test`
   surface. Plan picks the exact name; the constraint is that
   user-authored tests must not be able to read arbitrary
   filesystem paths through it (scope the helper to the workspace
   root or restrict to whitelisted suffixes).

2. Parses out the matcher names from `Matcher` and `NegatedMatcher`
   interface bodies. A regex over lines of the form `^\s*(\w+)\s*\(`
   inside each interface block is sufficient — the `.d.ts` is a
   small, controlled file and the lexical shape is uniform. No
   TypeScript parser is needed.

3. Asserts each matcher name is `typeof === "function"` on:
   - `expect(0)` — for `Matcher`.
   - `expect(0).not` — for `NegatedMatcher`.

4. Asserts the inverse: every function-valued key on those objects
   appears in the corresponding `.d.ts` interface. (Catches drift
   in the other direction: a matcher implemented but not declared.)

Skipped names: the `not: NegatedMatcher` property on `Matcher` is
declared as a property (not a method) and is filtered out by the
regex naturally (it doesn't have a `(...)` signature).

## Requirements

### 1. Implement `toBeUndefined()` and `toBeDefined()`

#### 1.1 Positive matchers in `_buildPositive`

Add to the object literal in `runtime/test.js`:

```js
toBeUndefined() {
  if (actual !== undefined) {
    _fail(`expect(${_pretty(actual)}).toBeUndefined(): value is not undefined`);
  }
},
toBeDefined() {
  if (actual === undefined) {
    _fail(`expect(${_pretty(actual)}).toBeDefined(): value is undefined`);
  }
},
```

Semantics: `toBeUndefined` passes when `actual === undefined`;
`toBeDefined` passes when `actual !== undefined`. `null` passes
`toBeDefined` — this mirrors Jest/Vitest and matches the
`docs/testing.md` table entry "Not `undefined`".

#### 1.2 Negated matchers in `_buildNegative`

Add to the object literal:

```js
toBeUndefined() {
  const f = _captureUserFrame();
  _negate(() => positive.toBeUndefined(),
    `expect(${_pretty(actual)}).not.toBeUndefined(): value is undefined`, f);
},
toBeDefined() {
  const f = _captureUserFrame();
  _negate(() => positive.toBeDefined(),
    `expect(${_pretty(actual)}).not.toBeDefined(): value is defined`, f);
},
```

The negated failure messages should be symmetric with the positive
ones and follow the existing wording style.

#### 1.3 Self-tests for behaviour

Add coverage in `runtime/test.test.js` under the existing
`describe("expect matchers", ...)` and `describe(".not chain", ...)`
blocks:

- `toBeUndefined()` passes when actual is `undefined`.
- `toBeUndefined()` throws when actual is `null` (regression: `null`
  is not `undefined`).
- `toBeUndefined()` throws when actual is `0`, `""`, `false` — all
  non-undefined values fail.
- `toBeDefined()` passes when actual is `null`, `0`, `""`, `false`,
  an object, a number.
- `toBeDefined()` throws when actual is `undefined`.
- `.not.toBeUndefined()` mirrors: passes for non-undefined, throws
  when undefined.
- `.not.toBeDefined()` mirrors: passes for undefined, throws
  otherwise.
- Each throw message contains both the matcher name and a hint
  (`is undefined` / `is not undefined` / `is defined` / `is not
  defined`), consistent with the existing matcher message
  style — assert with `expect(caught.message).toContain(...)`.

### 2. Drift-guard self-test

#### 2.1 Harness file-read helper

In `crates/zero-test-runner/src/harness.rs`:

- Expose a JS-callable function (sibling of `__getTestTree__` /
  `__resetTestTree__`) named `__readWorkspaceFile__(relPath: string)
  -> string`. It reads `<workspace_root>/<relPath>` and returns the
  contents. Errors (path not found, escaping `../` outside the
  workspace root) throw a JS error.
- The workspace root is the directory passed to the harness as the
  test run's root (the same one used to discover test files).
- The function is documented as **internal**, not part of the public
  `zero/test` API. Plan decides whether to gate it behind a feature
  flag or a build-mode check; for a self-test that only the
  framework ships, the simplest gate is "available but undocumented."

#### 2.2 Drift-guard self-test in `runtime/test.test.js`

Add a new `describe("matcher .d.ts ↔ runtime parity", ...)` block:

- Read `runtime/zero-test.d.ts` via `__readWorkspaceFile__`.
- Parse the body of the `Matcher` interface (substring between
  `interface Matcher {` and the matching closing `}`).
- Parse the body of the `NegatedMatcher` interface similarly.
- For each, extract method names with the regex
  `/^\s*(\w+)\s*\(/gm`. Exclude the `not` property (no `(`).
- For every declared matcher name on `Matcher`, assert
  `typeof expect(0)[name] === "function"`.
- For every declared matcher name on `NegatedMatcher`, assert
  `typeof expect(0).not[name] === "function"`.
- For every function-valued key on `expect(0)` (excluding `not`),
  assert it appears in the parsed `Matcher` name set.
- For every function-valued key on `expect(0).not`, assert it
  appears in the parsed `NegatedMatcher` name set.

The two-way check catches both "declared but not implemented" and
"implemented but not declared" drift. Failure messages should name
the matcher(s) that are mismatched.

#### 2.3 Documentation reference

The docs already list `toBeUndefined` / `toBeDefined` in
`docs/testing.md:57-58`. No doc changes needed — the docs were
already correct; the runtime was the lagging surface. After this
slice lands, the docs and runtime and `.d.ts` are aligned.

### 3. Spec text amendments

- `issues/test-correctness/spec.md` — the matcher API section (§3)
  lists every shipped matcher. The "Add four new matchers" wording
  for numeric comparators is fine, but `.d.ts` §3.3 doesn't mention
  `toBeUndefined` / `toBeDefined`. Append a one-line note to §3.3:
  "`toBeUndefined()` and `toBeDefined()` were declared in the
  `Matcher` / `NegatedMatcher` interfaces but were never
  implemented in `_buildPositive` / `_buildNegative`; they were
  filled in by the follow-up `issues/test-matcher-drift/` slice."
  Append a corresponding fix-annotation to the friction-log entry
  itself once the implementation lands.
- `runtime/zero-test.d.ts` — no edits required; the declarations
  already exist.
- `docs/testing.md` — no edits required.

## Constraints

- **No new npm dependencies.** Pure JS / Rust changes inside the
  existing workspace.
- **No TypeScript parser.** The `.d.ts` is a small, controlled file;
  a line-oriented regex is sufficient and avoids introducing a TS
  parser dependency into a Boa-hosted test.
- **`__readWorkspaceFile__` is internal.** It must not be exported
  from `zero/test`. Its path argument must be scoped to the
  workspace root (no `..` escapes); the workspace root is the
  directory the harness already knows about.
- **`toBeDefined` mirrors Jest semantics.** Passes for `null`, `0`,
  `""`, `false`, objects, numbers. Fails only for `undefined`.
- **No new failure-message format.** The new matchers follow the
  existing pattern: `expect(<pretty>).<name>(): <reason>`.
- **`.not.not` remains unsupported.** No change to existing
  guarantee.
- **The drift guard runs as a normal self-test.** No new test
  classification or runner flag — it's an `it(...)` block under a
  new `describe(...)`, executed by `cargo run -p zero -- test
  runtime/test.test.js` like every other self-test.
- **The parity check is a runtime test, not a build-time check.**
  No bundler-time or lint-time hook is added in this slice.
- **Pre-existing matcher behaviour is unchanged.** `toBe`, `toEqual`,
  `toBeTruthy`, `toBeFalsy`, `toBeNull`, `toContain`, `toThrow`,
  `toBeTemplateResult`, `toMatchSnapshot`, the spy matchers, and
  the numeric comparators keep their current implementations and
  messages.

## Out of Scope

- A `zero lint` rule enforcing `.d.ts` ↔ runtime parity. The
  user picked the JS self-test approach; a lint rule is a follow-up
  if drift extends beyond the matcher surface.
- Audit of other `.d.ts` ↔ runtime parity outside `Matcher` /
  `NegatedMatcher` (e.g. `SpyFn` methods, `render` / `find` /
  `findAll` / `text` / `fire` / `cleanup` signatures). This slice
  scopes to the matcher surface only, because that's where the
  observed drift lives. A general-purpose ambient-vs-runtime
  parity checker is a separate slice.
- Asymmetric matchers (`expect.anything()`, `expect.any(Number)`).
- `toMatchObject`, snapshot testing, `toMatchInlineSnapshot`.
- Doc rewrites to `docs/testing.md` beyond confirming alignment.
- `.not.not` support.
- Color or TTY-aware reporter output for parity-check failures.
- Backporting the new matchers into older spec docs as additions
  to their requirement lists (they were already declared in those
  surfaces; the spec was right, the implementation lagged).

## Open Questions

- **Should `__readWorkspaceFile__` be available to user-authored
  tests, or hidden behind a framework-only gate?** Recommendation:
  hidden but not strictly gated — name it with a
  framework-internal-looking identifier (`__readWorkspaceFile__`,
  matching the `__getTestTree__` precedent) and document it as
  internal in the harness source, but don't add a runtime flag to
  refuse calls from user code. User-authored tests already run in
  the same Boa context with full filesystem access via other shims
  (where available); a hard gate buys little and complicates the
  implementation.
- **Path validation for `__readWorkspaceFile__`.** The minimum
  guarantee is "no path traversal outside the workspace root."
  Implementation options: (a) canonicalise the joined path and
  check the prefix equals the workspace root; (b) reject any
  segment equal to `..`. Recommendation: (a) — more correct, robust
  against symlinks.
- **Where to put the parity-check `describe` block.** Adjacent to
  `describe(".not chain", ...)` reads naturally as a structural
  follow-up to the matcher tests. Recommendation: at the bottom of
  `runtime/test.test.js`, after `describe("spy matchers", ...)`,
  so the file's top-down reading order is positive matchers →
  negated matchers → numeric matchers → DOM helpers → spy
  primitive → spy matchers → drift-guard. Plan can decide.
- **What does the drift-guard failure message look like?** A
  single `_fail` with a list (`"matcher(s) declared but not
  implemented: toBeUndefined, toBeDefined"` plus the inverse
  list) is more informative than per-matcher individual failures.
  Recommendation: one summary failure per direction (declared but
  missing; implemented but undeclared), each listing all
  mismatches.
- **Does the parity check handle whitespace/comment variations in
  the `.d.ts`?** The current file is tidy: one method per line, no
  inline comments inside the interface bodies. The regex assumes
  this. If the `.d.ts` is ever reformatted (e.g. multi-line method
  signatures), the parity check will need to be updated.
  Recommendation: keep the regex simple, accept the fragility,
  document in a comment above the regex that the `.d.ts` format
  is contract.
- **Should the drift guard also walk the `.not` property on
  `expect(0)` to confirm it exists (as opposed to only checking
  its methods)?** The current spec implies yes (the guard tests
  call `expect(0).not[name]`, which throws if `.not` itself is
  missing). Explicit assertion that `typeof expect(0).not ===
  "object"` would give a clearer error if someone removes the
  `.not` wiring entirely. Recommendation: add the explicit check
  at the start of the negated-matcher loop.
