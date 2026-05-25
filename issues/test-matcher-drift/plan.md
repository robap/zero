# Plan: `zero/test` matcher .d.ts ↔ runtime drift

## Summary

Two matchers (`toBeUndefined`, `toBeDefined`) are declared on the `Matcher`
and `NegatedMatcher` interfaces in `runtime/zero-test.d.ts` but were never
implemented in `runtime/test.js`, so calling them throws `TypeError: not a
callable function` despite green TypeScript type-checks. This slice (1)
implements both matchers in the positive and negated tables following the
existing `toBeNull`/`_negate` patterns, and (2) adds a runtime self-test that
parses the live `.d.ts` and asserts bidirectional parity between the declared
interface methods and the implemented matcher functions, so the next drift
fails loudly. To let the self-test read the `.d.ts` from inside Boa, a
workspace-scoped native file-read helper (`__readWorkspaceFile__`) is installed
on the test harness's global object.

## Prerequisites

None. All open questions in the spec carry recommendations, which this plan
adopts (see "Resolved open questions" below).

### Resolved open questions

- `__readWorkspaceFile__` is available but undocumented (no hard runtime gate);
  named with a framework-internal `__…__` identifier and documented as internal
  in the harness source.
- Path validation: canonicalise the joined path and verify it is still inside
  the canonicalised workspace root (option (a) — robust against symlinks and
  `..` escapes).
- Parity-check `describe` block goes at the bottom of `runtime/test.test.js`,
  after `describe("spy matchers", …)`.
- Drift-guard failure messages: one summary `_fail` per direction (declared but
  missing; implemented but undeclared), each listing all mismatched names.
- The regex assumes the tidy one-method-per-line `.d.ts` format; a comment above
  the regex documents that this format is contract.
- The negated-matcher loop begins with an explicit `typeof expect(0).not ===
  "object"` assertion so removing the `.not` wiring gives a clear error.

## Steps

- [x] **Step 1: Implement `toBeUndefined` / `toBeDefined` in both matcher tables**
- [x] **Step 2: Behaviour self-tests for the two new matchers**
- [x] **Step 3: Add the `__readWorkspaceFile__` harness helper**
- [x] **Step 4: Drift-guard parity self-test**
- [x] **Step 5: Spec-text amendments**

---

## Step Details

### Step 1: Implement `toBeUndefined` / `toBeDefined` in both matcher tables
**Goal:** Close the actual drift so the two declared matchers exist at runtime.
This is the foundational change — the behaviour tests (Step 2) and the parity
guard (Step 4) both depend on the matchers existing.
**Files:** `runtime/test.js`
**Changes:**
- In `_buildPositive` (object literal returned at line 456), add two methods
  next to `toBeNull` (after line 476):
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
- In `_buildNegative` (object literal returned at line 589), add two methods
  next to the negated `toBeNull` (after line 615), following the `_negate` +
  `_captureUserFrame()` pattern:
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
- Semantics: `toBeUndefined` passes iff `actual === undefined`; `toBeDefined`
  passes iff `actual !== undefined` (so `null` passes `toBeDefined`, matching
  Jest/Vitest and the `docs/testing.md` table). No JSDoc additions needed —
  these are inline object-literal methods like their siblings.
**Tests:** None in this step beyond the existing suite continuing to pass.
Behaviour coverage is Step 2; this step just makes the matchers callable. Run
`cargo run -p zero -- test runtime/test.test.js` to confirm no regression.

### Step 2: Behaviour self-tests for the two new matchers
**Goal:** Pin the pass/fail semantics and message wording of both matchers in
positive and negated form.
**Files:** `runtime/test.test.js`
**Changes:**
- Under `describe("expect matchers", …)` (closes at line 85), add `it` blocks:
  - `toBeUndefined()` passes when actual is `undefined`.
  - `toBeUndefined()` throws when actual is `null` (regression guard) — assert
    `caught.message` contains `"toBeUndefined"` and `"is not undefined"`.
  - `toBeUndefined()` throws for `0`, `""`, `false` (loop or three asserts).
  - `toBeDefined()` passes for `null`, `0`, `""`, `false`, `{}`, `1`.
  - `toBeDefined()` throws when actual is `undefined` — assert message contains
    `"toBeDefined"` and `"is undefined"`.
- Under `describe(".not chain", …)` (closes at line 225), add `it` blocks:
  - `.not.toBeUndefined()` passes for non-undefined, throws when `undefined`
    (assert message contains `"is undefined"`).
  - `.not.toBeDefined()` passes for `undefined`, throws otherwise (assert
    message contains `"is defined"`).
- Use the established `try { … } catch (e) { caught = e; }` +
  `expect(caught).toBeTruthy()` + `expect(caught.message).toContain(...)`
  idiom already used throughout the file.
**Tests:** These blocks *are* the tests. Verify with
`cargo run -p zero -- test runtime/test.test.js` — all new `it`s green.

### Step 3: Add the `__readWorkspaceFile__` harness helper
**Goal:** Give Boa-hosted self-tests a way to read the live `.d.ts` from the
workspace root, scoped so it cannot read outside that root. Required before the
parity test (Step 4) can read `runtime/zero-test.d.ts`.
**Files:** `crates/zero-test-runner/src/harness.rs`
**Changes:**
- Add a function `install_workspace_file_reader(ctx: &mut Context, root: &Path)`
  modelled on `install_console` (line 825). It installs a native function on
  `globalThis` named `__readWorkspaceFile__` that:
  - Takes one string argument `relPath`.
  - Joins `root.join(relPath)`, canonicalises both `root` and the joined path,
    and verifies the canonical joined path starts with the canonical root. On
    failure (missing arg, non-string, canonicalise error, or escape outside
    root) it returns `Err(JsError…)` so JS sees a thrown error.
  - On success reads the file via `std::fs::read_to_string` and returns the
    contents as a `JsValue` string.
  - Because the Boa `NativeFunction::from_fn_ptr` closure cannot capture
    `root`, capture the canonical root by building the closure with
    `NativeFunction::from_copy_closure` (or move an owned `PathBuf` into a
    boxed closure) — pick whichever the Boa version in this crate supports;
    `from_copy_closure` taking a `move` closure capturing the `PathBuf` is the
    expected mechanism.
  - Document the function with a `///` comment stating it is framework-internal,
    not part of the public `zero/test` surface, and scoped to the workspace
    root.
- Call `install_workspace_file_reader(&mut context, project_root)` in
  `run_with_loader_inner`, immediately after the `install_console(&mut context)`
  call (line 182). `project_root` is already a parameter in scope there.
**Tests:** Add a Rust unit test in the existing `#[cfg(test)] mod tests` block
at the bottom of `harness.rs`:
- A test that writes a temp `.js` test file which calls
  `globalThis.__readWorkspaceFile__("known-file.txt")` (with a known file
  written into the temp root) and asserts via an `expect(...).toContain(...)`
  inside the JS that the contents came back — i.e. the run produces a Passed
  outcome.
- A test that asserts reading an escaping path (`"../outside"`) throws: the JS
  test body calls it inside `expect(() => __readWorkspaceFile__("../x")).toThrow()`
  and the outcome is Passed (the throw was caught), or the run fails if no throw.
Run `cargo test -p zero-test-runner`.

### Step 4: Drift-guard parity self-test
**Goal:** Make future `.d.ts` ↔ runtime drift fail loudly, in both directions.
**Files:** `runtime/test.test.js`
**Changes:**
- Append a new top-level `describe("matcher .d.ts ↔ runtime parity", …)` block
  at the end of the file (after `describe("spy matchers", …)` closes at 708).
- Shared setup inside the describe (computed once per `it` or via a small
  helper fn declared in the block):
  - `const dts = globalThis.__readWorkspaceFile__("runtime/zero-test.d.ts");`
  - A helper `interfaceBody(name)` that slices the substring between
    `interface <name> {` and its matching closing `}`. Since the interfaces
    contain no nested braces in method signatures, the first `}` after the
    `interface X {` opener is the close — slice from the opener to that `}`.
  - A helper `matcherNames(body)` that runs `/^\s*(\w+)\s*\(/gm` over the body
    and collects capture group 1 into a `Set`. The `not: NegatedMatcher;`
    property has no `(` and is naturally excluded.
  - Add a comment above the regex: the `.d.ts` one-method-per-line format is
    contract; multi-line signatures would break this parser.
- `it` blocks:
  1. **Matcher declared ⊆ implemented:** for each name in
     `matcherNames(interfaceBody("Matcher"))`, collect any where
     `typeof expect(0)[name] !== "function"`; if the list is non-empty, build
     one failure message `"matcher(s) declared on Matcher but not implemented:
     <names>"` and fail (use `expect([]).toEqual(missing)` or a thrown Error;
     prefer asserting `expect(missing).toEqual([])` so the message lists them).
  2. **NegatedMatcher declared ⊆ implemented:** first assert
     `expect(typeof expect(0).not).toBe("object")`; then the same loop against
     `expect(0).not[name]`.
  3. **Implemented ⊆ declared (positive):** for every own key of `expect(0)`
     where `typeof expect(0)[key] === "function"` and `key !== "not"`, assert
     it appears in the `Matcher` name set; collect violations and fail with
     `"matcher(s) implemented on expect() but not declared in Matcher: <names>"`.
  4. **Implemented ⊆ declared (negated):** same for `expect(0).not` keys
     against the `NegatedMatcher` name set.
- Enumerate keys with `Object.keys(expect(0))` — the positive table is a plain
  object literal so own enumerable keys are the matcher methods plus `not`.
**Tests:** The four `it` blocks are the tests. Verify with
`cargo run -p zero -- test runtime/test.test.js` — all parity `it`s green now
that Step 1 landed the two matchers. Sanity-check the guard actually bites by
temporarily removing one matcher from `_buildPositive` and confirming the
parity test fails with the matcher name in the message (revert after).

### Step 5: Spec-text amendments
**Goal:** Record that the two matchers were back-filled, per spec §3.
**Files:** `issues/test-correctness/spec.md`; the friction-log entry in
`zero_demo/FRAMEWORK_NOTES.md` (if present in this checkout).
**Changes:**
- Append one line to `issues/test-correctness/spec.md` §3.3 (after line 358):
  > `toBeUndefined()` and `toBeDefined()` were declared in the `Matcher` /
  > `NegatedMatcher` interfaces but were never implemented in `_buildPositive`
  > / `_buildNegative`; they were filled in by the follow-up
  > `issues/test-matcher-drift/` slice.
- If `zero_demo/FRAMEWORK_NOTES.md` exists in this checkout, append a
  fix-annotation to the 2026-05-24 entry noting the drift is resolved by
  `issues/test-matcher-drift/`. If the file is not present, note that in the
  step output and skip — do not create it.
- No edits to `runtime/zero-test.d.ts` (declarations already correct) or
  `docs/testing.md` (already correct).
**Tests:** None — documentation only.

## Risks and Assumptions

- **Boa native-closure capture API.** Step 3 assumes the Boa version in
  `zero-test-runner` exposes a way to build a native function that captures an
  owned `PathBuf` (e.g. `NativeFunction::from_copy_closure`). If only
  `from_fn_ptr` (no capture) is available, the workspace root must be threaded
  another way — e.g. stash the canonical root in a `globalThis` string the
  closure reads, or use a thread-local. Resolve by checking the Boa API at
  implementation time; the rest of the plan is unaffected.
- **`.d.ts` parser fragility.** The regex assumes one method per line and no
  nested braces inside interface bodies (true today). A future reformat to
  multi-line signatures would break the parity test — accepted and documented
  in-code as contract.
- **Interface-body slicing.** Assumes the first `}` after `interface X {` is the
  closing brace. Valid because method signatures here are single-line with no
  `{`. If a method signature ever gains an inline object-type brace, the slice
  would truncate — same fragility class as the regex, same mitigation.
- **`Object.keys(expect(0))` enumerability.** Assumes the positive table's
  matcher methods are own enumerable properties (true for object-literal
  methods) and that `.not` is the only non-matcher key. If `expect()` later
  attaches other own function properties, the implemented⊆declared check would
  flag them; that is the intended behaviour (forces a declaration).
- **Path canonicalisation requires the file to exist.** `std::fs::canonicalize`
  errors on missing paths, so a read of a non-existent file throws — acceptable,
  since the self-test only reads a file known to exist.
