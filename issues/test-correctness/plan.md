# Plan: `zero/test` correctness — effect leaks, matchers, throw locations, Boa GC

## Summary

Four independent fixes that share the `zero/test` + Boa-harness surface,
ordered to ship one at a time without breaking the build. JS-side fixes
land first (unowned-effect disposal in `runtime/reactivity.js`; `.not`
chain and numeric matchers in `runtime/test.js`), then Rust harness
fixes (throw-location parser improvement; `catch_unwind` safety net
around Boa GC), then a narrow audit of `runtime/*.js` for the known
Boa MapLock trigger, then doc/cross-spec amendments. Each step is
independently testable and leaves `cargo test --workspace` and the
runtime's `node --test` suite green.

## Prerequisites

None. All open questions in the spec are plan-time decisions resolved
below in the step details. No upstream issue dependencies.

## Steps

- [x] **Step 1: Unowned-effect disposal (`runtime/reactivity.js` + `cleanup()`)**
- [x] **Step 2: `.not` chain and numeric matchers (`runtime/test.js` + `.d.ts`)**
- [x] **Step 3: Throw-location parser improvement (`harness.rs`)**
- [x] **Step 4: `catch_unwind` safety net around Boa GC (`harness.rs` + `mutate.rs`)**
- [x] **Step 5: Audit `runtime/*.js` for the MapLock dispatcher pattern**
- [x] **Step 6: Docs and cross-spec amendments**

---

## Step Details

### Step 1: Unowned-effect disposal (`runtime/reactivity.js` + `cleanup()`)

**Goal:** Stop top-level `effect()` calls in route/component modules
from leaking past `cleanup()` and re-firing after `_currentApp` is
nulled. Closes L33.

**Files:**
- `runtime/reactivity.js` (modify)
- `runtime/test.js` (modify `cleanup()`)
- `runtime/reactivity.test.js` (add tests)
- `runtime/test.test.js` (add tests)

**Changes:**

1. In `runtime/reactivity.js`, add a module-level set above `signal`:
   ```js
   /** @type {Set<() => void>} */
   let _unownedEffects = new Set();
   ```
2. In `effect(fn)` (around line 105), after `const _registeredScope = _activeScope;`:
   - If `_registeredScope == null`, after declaring `function stop()`,
     add `_unownedEffects.add(stop);`. The add must happen *before*
     `_run()` so the effect is tracked even if `_run()` throws.
3. In `stop()` itself, after `_unsubscribeAll(self)` and the cleanup
   call, add `_unownedEffects.delete(stop);` (no-op if it wasn't
   registered — sets are forgiving).
4. Add a new exported function:
   ```js
   /**
    * Dispose every effect created without an active scope. Used by
    * `zero/test`'s `cleanup()` to prevent leaked top-level effects
    * from firing across tests within a single Boa context.
    * @internal
    * @returns {void}
    */
   export function _disposeUnownedEffects() {
     for (const stop of [..._unownedEffects]) stop();
     _unownedEffects.clear();
   }
   ```
   Place it next to `_createScope` at the bottom of the file. Add a
   matching line to the existing "Exported for testing only" comment.
5. In `runtime/test.js`:
   - Extend the existing reactivity import:
     `import { _createScope, _disposeUnownedEffects } from "./reactivity.js";`
   - In `cleanup()` (line 218), call `_disposeUnownedEffects()` as the
     **first** statement, before the `_renderTracker` loop. Order
     matters: an effect's cleanup callback may touch state that the
     render-scope disposal will then re-touch; doing unowned first
     means the runtime invariants are intact when render scopes drop.

**Tests:**

In `runtime/reactivity.test.js`:
- Effect created with no active scope is added to the unowned set
  (introspect by calling `_disposeUnownedEffects()` and asserting
  the effect's callback ran one final cleanup, e.g. via a flag).
- Effect created inside `scope.run()` is **not** disposed by
  `_disposeUnownedEffects()` — only by `scope.dispose()`.
- Manually-stopped unowned effect doesn't double-stop when
  `_disposeUnownedEffects()` runs.
- Effect's `_cleanup` callback (returned from `fn`) runs exactly
  once when disposed by `_disposeUnownedEffects()`.

In `runtime/test.test.js`:
- A top-level `effect()` then `cleanup()` then a `signal.set(...)`
  on its dependency does not re-fire the effect. (Regression
  surface for the friction-log symptom.)
- Render-scope effects are still disposed (regression).
- Calling `cleanup()` twice is safe (no double-stop error).

Run: `cargo run -p zero -- test runtime/reactivity.test.js` and
`cargo run -p zero -- test runtime/test.test.js`. Also
`cargo test --workspace` to ensure the harness's own integration
tests still pass.

---

### Step 2: `.not` chain and numeric matchers (`runtime/test.js` + `.d.ts`)

**Goal:** Add negation and numeric comparison matchers. Closes L36;
incidentally removes the surface symptom that triggered L34's
reporting confusion.

**Files:**
- `runtime/test.js` (modify `expect()`)
- `runtime/zero-test.d.ts` (modify matcher interface)
- `runtime/test.test.js` (add tests)
- `runtime/dom-shim.test.js` (no changes — sanity that selectors keep working)

**Changes:**

1. In `runtime/test.js`, refactor `expect()` to factor the matcher
   table out of the literal. The matcher body for each check becomes
   a function `(actual, expected, ...) => { /* throw via _fail if
   failed */ }`. The positive matcher swallows that throw inversely
   for `.not`:
   ```js
   /**
    * @internal
    * @param {() => void} check  Body that calls _fail on failure.
    * @param {string} negatedMsg  Message used when negation should fail (i.e. positive check passed).
    */
   function _negate(check, negatedMsg) {
     let threw = false;
     try { check(); } catch (_) { threw = true; }
     if (!threw) _fail(negatedMsg);
   }
   ```
2. Replace the current `expect(actual)` body with two parallel
   matcher objects:
   ```js
   const positive = {
     toBe(expected) { /* existing */ },
     ...
     toBeGreaterThan(n) {
       if (typeof actual !== "number" || typeof n !== "number") {
         _fail(`expect(...).toBeGreaterThan: value is not a number`);
       }
       if (!(actual > n)) {
         _fail(`expect(${_pretty(actual)}).toBeGreaterThan(${n}): ${actual} is not greater than ${n}`);
       }
     },
     toBeGreaterThanOrEqual(n) { /* similar with `>=` */ },
     toBeLessThan(n) { /* similar with `<` */ },
     toBeLessThanOrEqual(n) { /* similar with `<=` */ },
   };
   const negative = {
     toBe(expected) {
       _negate(() => positive.toBe(expected),
         `expect(${_pretty(actual)}).not.toBe(${_pretty(expected)}): values are strictly equal`);
     },
     ...
   };
   positive.not = negative;
   return positive;
   ```
   Every existing matcher (`toBe`, `toEqual`, `toBeTruthy`,
   `toBeFalsy`, `toBeNull`, `toContain`, `toThrow`,
   `toBeTemplateResult`, `toHaveBeenCalled`,
   `toHaveBeenCalledTimes`, `toHaveBeenCalledWith`,
   `toHaveBeenLastCalledWith`) gets a `.not.X` counterpart.
   `toMatchSnapshot` keeps its current "not implemented" body on
   both sides (calling `_fail()` in `.not.toMatchSnapshot` is fine
   — the negation message is irrelevant because the body throws
   either way).
3. `.not.toThrow(msg?)`: passes if the function either doesn't
   throw, or throws an error whose message does **not** contain
   `msg`. Mirrors Jest/Vitest.
4. `.not.toHaveBeenCalledWith(...args)`: failure message lists the
   recorded calls that matched plus the total recorded count.
5. `negative` has **no** `.not` property. Document via JSDoc.
6. In `runtime/zero-test.d.ts`, update the matcher interface:
   - Define `type Matchers` with every existing method plus the four
     numeric ones.
   - Define `type NegatedMatchers = Omit<Matchers, "toMatchSnapshot">`
     (or similar — match same shape minus `.not` itself).
   - Add `not: NegatedMatchers` to `Matchers`.
   - Add `toBeGreaterThan(n: number): void` etc.

**Tests:**

In `runtime/test.test.js`, one describe per area:
- `.not.toBe` — pass when actual !== expected, fail with the
  "values are strictly equal" message when they are.
- `.not.toEqual` — same shape against deep-equal values.
- `.not.toBeNull` / `.not.toBeTruthy` / `.not.toBeFalsy`.
- `.not.toContain(item)` for both arrays and strings.
- `.not.toThrow()` — passes for a no-op fn; fails for a throwing fn.
- `.not.toThrow("msg")` — passes when the thrown message doesn't
  match; fails when it does.
- `.not.toBeTemplateResult()` — passes for `{}`, fails for an
  actual `html\`...\`` result.
- `.not.toHaveBeenCalled` / `Times(n)` / `With(...)` / `LastCalledWith(...)`.
- Numeric: `toBeGreaterThan` pass/fail/equal-boundary,
  `toBeGreaterThanOrEqual` pass/fail/equal-boundary, same for
  `toBeLessThan` / `toBeLessThanOrEqual`. Non-number actual or
  argument throws cleanly.
- `.not.toBeGreaterThan(n)` etc. all work.
- Error decoration: `expect(1).not.toBe(1)` failure carries a
  `_userFrame` (assert by catching and inspecting `err._userFrame`).

Run: `cargo run -p zero -- test runtime/test.test.js`. Also run a
real assertion in a `.ts` test file to confirm the reporter still
shows the proper `at <path>:<line>:<col>` line (sanity check that
`_userFrame` propagation still works).

---

### Step 3: Throw-location parser improvement (`harness.rs`)

**Goal:** When a thrown error has no `_userFrame`, the harness picks
the throw site instead of the `it(...)` registration line. Closes
L34's underlying issue.

**Files:**
- `crates/zero-test-runner/src/harness.rs` (modify `compute_location`
  and `is_user_path`; add a `FRAMEWORK_REGISTRATION_NAMES` constant)

**Changes:**

1. Add a new constant near `FRAMEWORK_INTERNAL_BASENAMES`:
   ```rust
   /// Function names that identify test-framework registration call sites.
   /// A frame whose function name is one of these is never the actual throw
   /// site — it's the call that *contains* the throw. The stack walker
   /// skips these when picking the topmost user frame.
   const FRAMEWORK_REGISTRATION_NAMES: &[&str] = &[
       "it", "describe",
       "beforeEach", "afterEach", "beforeAll", "afterAll",
       "cleanup", "render",
   ];
   ```
2. Extend `parse_stack_frame` (line 585) to also return the function
   name. Today the regex captures `(path, line, col)`; widen it to
   optionally extract the function name from V8-style
   `"    at fn (path:L:C)"` frames. SpiderMonkey-style `"fn@path:L:C"`
   already starts with the name. New return type:
   `Option<(Option<String>, String, u32, u32)>`. Plain `path:L:C`
   frames produce `None` for the function name.
3. Refactor `compute_location` (line 546):
   - Keep the `user_frame` preference (no change).
   - In the stack walk, switch from "first user-frame" to "first
     user-frame that is *not* a registration call". A frame matches
     "registration" when its function name is in
     `FRAMEWORK_REGISTRATION_NAMES`. (Path-based filtering via
     `is_user_path` stays as a prerequisite.)
   - If after filtering no frame matches, fall back to today's
     behavior (first non-internal frame) so we never return `None`
     when the old code would have returned `Some`.

**Tests:**

In the existing `#[cfg(test)] mod tests` block at the bottom of
`harness.rs`:
- A `.js` test that throws `null.foo` (raw TypeError) reports a
  `location` whose line matches the `null.foo` line, not the
  `it()` line. Construct a test file with `null.foo` on a known
  line.
- A `.js` test that does `throw new Error("boom")` reports the
  `throw` line.
- A matcher failure still reports the matcher call line
  (regression — `_userFrame` path).
- An async test body that does `await Promise.reject(new Error("x"))`
  reports a location whose line is the `await` (or `Promise.reject`)
  line, not `it()`. (Best-effort: if the stack doesn't surface a
  meaningful frame, the fallback is acceptable, but the
  registration filter must at least not pick the `it` frame.)
- Unit test on `parse_stack_frame` directly: a V8 frame like
  `"    at it (zero/test:42:7)"` returns
  `Some(("it", "zero/test", 42, 7))`.

Run: `cargo test -p zero-test-runner harness::tests`.

---

### Step 4: `catch_unwind` safety net around Boa GC (`harness.rs` + `mutate.rs`)

**Goal:** A Boa MapLock finalizer panic during Context teardown
becomes a structured `Failure` appended to the file's outcomes; the
process exits cleanly with non-zero status instead of crashing.
Closes L35 (the safety-net half).

**Files:**
- `crates/zero-test-runner/src/harness.rs` (wrap `run_with_loader`)
- `crates/zero-test-runner/src/mutate.rs` (audit mutant worker for
  the same wrap pattern)
- `crates/zero-test-runner/src/result.rs` (no shape change needed —
  `Failure` already permits `stack: None`, `location: None`)

**Changes:**

1. In `harness.rs`, change `run_with_loader` (line 118) to wrap its
   body in `std::panic::catch_unwind`:
   ```rust
   fn run_with_loader(
       project_root: &Path,
       file_abs: &Path,
       loader: Rc<ZeroModuleLoader>,
       want_coverage: bool,
   ) -> RunOutcome {
       let rel_path = file_abs.strip_prefix(project_root).unwrap_or(file_abs).to_path_buf();
       let rel_for_panic = rel_path.clone();

       let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
           run_with_loader_inner(project_root, file_abs, loader, want_coverage, rel_path)
       }));

       match result {
           Ok(outcome) => outcome,
           Err(panic_payload) => synthesize_panic_outcome(rel_for_panic, panic_payload),
       }
   }
   ```
   Move the current body into `run_with_loader_inner` taking the
   `rel_path` by value.
2. Add `synthesize_panic_outcome`:
   ```rust
   fn synthesize_panic_outcome(
       rel_path: PathBuf,
       payload: Box<dyn std::any::Any + Send>,
   ) -> RunOutcome {
       let msg = panic_payload_to_string(&payload);
       RunOutcome {
           result: FileResult {
               path: rel_path,
               outcomes: vec![TestOutcome {
                   name_chain: vec!["<Boa GC panic during teardown>".to_string()],
                   status: Status::Failed,
                   duration_ms: 0,
                   failure: Some(Failure {
                       message: format!("Boa GC panic: {msg}"),
                       stack: None,
                       location: None,
                   }),
               }],
               load_error: None,
           },
           coverage: None,
           loaded: vec![],
       }
   }

   fn panic_payload_to_string(p: &(dyn std::any::Any + Send)) -> String {
       if let Some(s) = p.downcast_ref::<&'static str>() { return (*s).to_string(); }
       if let Some(s) = p.downcast_ref::<String>() { return s.clone(); }
       "<non-string panic payload>".to_string()
   }
   ```
3. **Important:** outcomes collected before the panic are lost
   because `catch_unwind` unwinds the inner stack. This is
   acceptable for the safety net — a teardown panic is rare and
   the synthetic failure makes the run loud. Document this trade-off
   in a comment above `synthesize_panic_outcome`.
4. Set the panic hook only once per process (or use the silent-hook
   trick `std::panic::set_hook` to suppress the default stderr
   spam during the catch). Plan: install a no-op hook only for the
   duration of `catch_unwind` to avoid noisy stderr:
   ```rust
   let prev_hook = std::panic::take_hook();
   std::panic::set_hook(Box::new(|_| {}));
   let result = std::panic::catch_unwind(...);
   std::panic::set_hook(prev_hook);
   ```
   Wrap the prev_hook swap in a drop guard so a panic *inside* the
   catch closure still restores the hook.
5. In `mutate.rs`, find the per-mutant subprocess invocation. The
   subprocess already runs `run_file_*` which now has the
   `catch_unwind` wrap (so panics in the worker become structured
   failures). The parent only needs to ensure that a subprocess
   exiting non-zero from the synthetic failure path counts as
   `errored` (the existing bucket). Audit the existing
   classification logic — if it already treats non-zero exit as
   `errored`, no change. Otherwise add one: a subprocess whose
   stdout contains the synthetic GC-panic marker is `errored`.

**Tests:**

In `harness.rs` tests:
- Inject a deliberate `panic!()` inside the inner function (via a
  `#[cfg(test)]`-gated env var like `ZERO_TEST_FORCE_TEARDOWN_PANIC=1`
  read at the bottom of `run_with_loader_inner`) and assert that
  `run_file` returns a `RunOutcome` with one synthetic failure,
  rather than the test process crashing. Wire the env var read
  behind `#[cfg(test)]` so it never affects production builds.
- `cargo test -p zero --test examples_tests tracker_tests_pass`
  continues to pass (regression for the known Boa pattern that
  used to crash).

Run: `cargo test --workspace`.

---

### Step 5: Audit `runtime/*.js` for the MapLock dispatcher pattern

**Goal:** Remove known MapLock-finalizer triggers from the runtime by
applying the "one function per branch" pattern documented in the
`boa-maplock-finalizer` memory. Reduces how often the safety net
from Step 4 has to fire.

**Files:**
- `runtime/template.js` (audit; likely already compliant since
  `_commitEach` → `_commitEachKeyed` split is in place at lines
  519/551)
- `runtime/reactivity.js`, `runtime/app.js`, `runtime/router.js`,
  `runtime/http.js`, `runtime/dom-shim.js`, `runtime/test.js`
  (audit)
- Memory file
  `/home/rob/.claude/projects/-home-rob-Documents-code-zero/memory/boa_maplock_finalizer.md`
  (update at end)

**Changes:**

For each file:
1. Grep for functions with two or more branches that early-return
   over a discriminator (`if (typeof x === "...") { ... return; }`
   patterns with substantial inline code).
2. For each matching dispatcher, extract each branch's body into a
   separate top-level function and have the dispatcher call into
   them.
3. If a file has no matching pattern, note it and move on. The
   audit's deliverable is a list, not a forced refactor.

Concrete starting point (from grep): `runtime/template.js`'s
`_commitEach` already follows the pattern. Verify the other
candidates:
- `_applyNodeValue` (line 630) and `_applyNodeValueLeaf` (line 506)
  — check whether the leaf has multi-branch dispatch.
- `_commitNode` (line 653) — check for inlined branches.
- `_commitAttr` (line 393) already splits to `_commitAttrSingle` /
  `_commitAttrJoined`.

Do **not** opportunistically refactor functions that don't match
the pattern. Conservative scope keeps regressions low.

**Tests:**

- `cargo test --workspace` — must remain green throughout.
- `cargo test -p zero --test examples_tests tracker_tests_pass`
  — primary canary per the memory note.
- `cargo run -p zero -- test runtime/` — exercises the runtime's
  own test suites through the Boa harness.

After the audit, update the `boa-maplock-finalizer` memory file to:
- List which files were audited and which were modified.
- Reaffirm the dispatcher pattern as the canonical fix.
- Note the new `catch_unwind` safety net as the backstop.

---

### Step 6: Docs and cross-spec amendments

**Goal:** Document the new matcher surface, the unowned-effect
trade-off, and update cross-references in prior specs.

**Files:**
- `docs/testing.md` (matcher table + new "Effects in route bodies"
  subsection)
- `docs/api.md` (if it has a matcher list, update it)
- `issues/test-runner/spec.md` (add this slice to the "Shipped in a
  follow-up slice" section, around line 450)

**Changes:**

1. In `docs/testing.md`, extend the matcher table (line 52) with:
   - `.not.<any>` row describing the negation chain.
   - `.toBeGreaterThan(n)`, `.toBeGreaterThanOrEqual(n)`,
     `.toBeLessThan(n)`, `.toBeLessThanOrEqual(n)`.
2. Add a new subsection under "Testing components" or near
   "Testing signals" titled **"Effects in route and component
   bodies"**:
   > A top-level `effect()` in a route or component module body has
   > no enclosing scope, so `cleanup()` disposes it between tests
   > to prevent stale subscriptions from re-firing once the test
   > app is torn down. The trade-off: a route relying on a
   > top-level effect for runtime behavior will lose that effect
   > after the first `cleanup()` call. Put effects inside the
   > function called by `render()` (the component's exported
   > factory) so they live in the render scope and re-fire each
   > test.
3. In `docs/api.md`, if the export list mentions `expect`, mention
   the new matcher names. If the list is auto-generated from
   `.d.ts`, no change needed.
4. In `issues/test-runner/spec.md`, append to the existing
   "Shipped in a follow-up slice (`issues/test-improvements/`)"
   section (line 450) a new entry:
   > **Test correctness fixes** (delivered by
   > `issues/test-correctness/`): unowned-effect disposal in
   > `cleanup()`, `.not` chain and numeric matchers, throw-location
   > parser fix for non-matcher errors, Boa GC panic safety net.

**Tests:**

- `cargo test --workspace` (docs aren't compiled but rule out
  accidental link breakage).
- Visual: `cat docs/testing.md` and verify the table renders;
  spot-check the new matcher row formatting matches existing rows.

---

## Risks and Assumptions

- **Per-file context isolation holds.** Each test file runs in a
  fresh Boa context, so `_unownedEffects` set state doesn't leak
  across files. If a future spec moves to context reuse, the
  unowned-effect tracker must be reset between files explicitly.
- **`_disposeUnownedEffects()` doesn't break the runtime's own
  tests.** The runtime files (`runtime/*.test.js`) run under
  `node:test`, not under the Boa harness, so they don't import
  `zero/test` and never call its `cleanup()`. The change is
  effectively transparent to the node-side suite. The Boa-side
  tests run via `zero test runtime/*.test.js` (which the
  framework supports), and those will need `cleanup()` integration
  if any of them rely on top-level effects across test cases.
- **`catch_unwind` payload format.** The Boa panic message is
  likely a `String` or `&str`; if it's something else, the
  fallback `<non-string panic payload>` lands and the operator
  has less info. The synthetic failure is still loud and
  non-crashing, which is the goal.
- **Panic hook swap is not thread-safe.** The harness runs
  sequentially in a single thread; the take/set hook trick is
  safe under that invariant. If `zero test` ever gains
  multi-threaded file execution, this must move behind a
  `Mutex` or similar. The current `cmd/test.rs` loop is
  sequential (verified line 46), so this is fine.
- **Boa panic outcomes lose pre-panic test results.** The
  `catch_unwind` unwinds the inner stack, dropping any
  in-progress `outcomes` Vec. This is the accepted trade-off:
  GC panics happen at teardown after all tests have already run
  and been reported, so the loss is usually empty. If a panic
  ever fires mid-test, the synthetic outcome is the only signal.
- **Stack-frame regex won't match every Boa format.** Boa's stack
  format may evolve. The widened regex falls back gracefully —
  unmatched frames produce no function name and route through the
  existing path-based filter.
- **Audit may surface deeper issues.** If Step 5 finds dispatcher
  patterns that don't refactor cleanly (e.g. closures that share
  state across branches), the safety net from Step 4 covers the
  residual risk. No new fix path is added — the audit is bounded.
- **No new tests fail under existing behavior.** Each step's tests
  are new additions; running `cargo test --workspace` between
  steps must remain green. If a step makes existing tests fail,
  that indicates a regression to fix before moving on.
- **`zero test` self-host coverage.** Some new tests for Step 2 use
  `.not` to verify negation; we test the matcher with itself,
  which is fine because the implementation path for `.not` is
  separate from positive matchers. If a `.not` bug masks itself,
  the explicit failure-message assertions catch it.
