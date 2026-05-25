# Spec: `zero/test` correctness — effect leaks, matchers, throw locations, Boa GC

## Problem Statement

Four bugs in `zero/test` and its Boa harness consistently waste real
debugging time when writing tests for a downstream zero app. They were
all hit in a single recent friction-log session
(`zero_demo/FRAMEWORK_NOTES.md`, entries dated `2026-05-23`):

1. **Top-level `effect()` calls leak past `cleanup()`.** An effect
   created in a route or component module's top-level body has no
   `_activeScope` to register with, so `cleanup()` cannot dispose it.
   The effect stays subscribed across tests in the same file. When a
   later test mutates a signal that effect observed — directly or
   indirectly through `render()`'s template machinery — the effect
   refires, often calls `inject()`, and throws `"inject: no app is
   running"` because `cleanup()` already cleared `_currentApp`. The
   workaround (rewrite as `computed()` so the value is pulled
   reactively only when read) is a real ergonomic regression and the
   pattern that "broke" was the natural one a user reaches for first.
2. **Non-matcher throws point at the wrong line.** When code inside a
   test body throws an error that *isn't* one of `_fail()`'s
   user-frame-decorated matcher errors — e.g. a raw `TypeError` from
   accessing a property on `undefined` (the literal symptom: chaining
   the non-existent `.not.toBeNull()` produces "cannot convert 'null'
   or 'undefined' to object") — the reporter prints the `at` line and
   caret pointing at the `it(name, fn)` signature, not the throwing
   statement. Real cost in the friction-log session: ~30 minutes
   chasing a dead-end before realizing `.not` didn't exist.
3. **Intermittent Boa `BorrowMutError` on `zero test <path>`.** The
   Boa 0.21.1 `MapLock::finalize` finalizer panics during process-exit
   garbage collection when certain JS patterns (inlined branches
   inside one dispatcher function — see project memory
   `boa-maplock-finalizer`) coexist with parsed-but-unexecuted code.
   The same tests pass under full-suite `zero test`. There is no
   newer Boa release; the project is pinned at the current latest.
   The panic crashes the process (no clean exit code, no reporter
   summary), so the developer gets no signal about whether their
   tests actually passed or failed.
4. **Matcher API gaps.** `expect(...)` has no `.not` chain and no
   numeric comparison matchers (`toBeGreaterThan`, etc.). Both are
   reflexive reaches for anyone who has used Jest, Vitest, or
   `node:test`. Users rewrite assertions as inverted positives
   (`expect(!x.includes(y)).toBe(true)`) which read poorly and lose
   the failure-message benefit dedicated matchers give.

Items 1–4 share a single domain (the test runner / `zero/test` API and
its Boa harness) and ship together in this slice. Item 4's fix
incidentally closes the symptom that produced item 2's report
(adding `.not` means `.not.toBeNull()` no longer throws a
`TypeError`), but item 2's underlying bug — non-matcher throws
attributed to the wrong line — is fixed independently so future stray
throws point at their real location.

## Background

### Where the relevant code lives

- `runtime/reactivity.js` — `signal`, `computed`, `effect`,
  `_createScope`. `effect()` registers its disposer (`stop`) on
  `_activeScope._effects` only when `_activeScope` is set; otherwise
  the effect is unowned, has no disposer reachable from any scope,
  and `cleanup()` cannot reach it. Module-level state:
  `_observerStack`, `_activeScope`.
- `runtime/test.js` — `render`, `cleanup`, `expect`, `spy`. `expect()`
  returns a plain matcher object with no chain wrappers; `_fail()`
  throws an `Error` decorated with `err._userFrame` captured via
  `_captureUserFrame()` walking `new Error().stack`. The
  framework-internal basenames are listed in
  `_FRAMEWORK_INTERNAL_BASENAMES`. `cleanup()` disposes
  `_renderTracker` scopes, clears storage, clears timers, clears
  fetch, and resets DOM state, but does not touch unowned reactivity
  effects.
- `crates/zero-test-runner/src/harness.rs` — Boa harness. Reads
  thrown error properties via `js_err_to_failure` /
  `js_val_to_failure` (functions referenced at lines 41–46 of
  `harness.rs`), prefers `err._userFrame` if present, falls back to
  parsing `err.stack` for the topmost frame outside
  `FRAMEWORK_INTERNAL_BASENAMES`. Owns the Boa `Context` per test
  file; Context drops at end of `run_file_with_coverage` — that's the
  moment the `MapLock` finalizer fires.
- `crates/zero-test-runner/src/loader.rs` — `ZeroModuleLoader`, the
  Boa module loader; resolves `zero`, `zero/test`, `zero/http`,
  `zero/components`, and relative paths. Already uses `RefCell` for
  module cache. Not directly involved in any of the four bugs.
- `crates/zero-test-runner/src/reporter.rs` — formats failures with
  the `at <path>:<line>:<col>` line and source snippet. Reads
  `Failure.location: Option<SourceLoc>` from `result.rs`.
- `crates/zero/src/cmd/test.rs` — iterates discovered files, calls
  `run_file_with_coverage` per file. Sequential. No process-level
  panic handling.
- `runtime/zero-test.d.ts` — ambient declarations for the test API
  surface; matcher additions land here too.

### Why "auto-track unowned effects" is the L33 design

The natural place to set this up is `runtime/reactivity.js` itself:
a module-level `_unownedEffects: Set<() => void>` that every
`effect()` call registers its `stop` into when `_activeScope` is null.
A new private export `_disposeUnownedEffects()` walks the set,
calling each `stop`, then clears the set. `cleanup()` in
`runtime/test.js` imports and calls it after disposing render-scope
effects.

This is the smallest correct fix and crosses only one layer (the test
layer reaches into a privately-named reactivity helper). The
alternative — having `render()` wrap module evaluation in a scope —
doesn't work because modules load once per Boa context, before any
`render()` call.

**Trade-off the user must own:** disposing unowned effects in
`cleanup()` permanently disposes them for the rest of the test file
(modules don't re-evaluate). A route module that relies on a
top-level effect for *runtime* behavior will lose that behavior after
the first `cleanup()`. The recommended pattern, documented in the
testing docs as part of this slice, is to put effects *inside* the
function called by `render()` (i.e. the component/route's exported
factory), not at module top level. Top-level effects in route bodies
are valid in production but become singletons-per-file in tests.

### Why `.not` is a chain wrapper, not per-matcher

`.not` is small to add and cleaner as a single proxy that flips every
matcher's pass/fail interpretation than as per-matcher `.notToBe`,
`.notToEqual`, etc. The implementation is a second factory inside
`expect()` that runs the same matcher body and inverts the throw
condition (and rewrites the failure message). Same `_fail()` /
`_captureUserFrame()` machinery; same `_userFrame` decoration.

### Why fix non-matcher throw location independently

L36's fix (adding `.not`) closes the *symptom* the friction-log
captured under L34, but the underlying gap remains: any thrown value
that isn't decorated with `err._userFrame` will fall through to the
`harness.rs` stack-parse fallback. The fallback is supposed to pick
the first user-frame from `err.stack`; in practice it's been
returning the `it(...)` registration frame because Boa's stack for
errors thrown from inside async test bodies sometimes lists the
`it()` call site higher than the actual throw site, and the existing
frame walk doesn't disambiguate.

This slice fixes the harness-side parser to:

- Always prefer `err._userFrame` when present (already done).
- When falling back to `err.stack`, find the *deepest* (innermost) frame
  whose path is outside `FRAMEWORK_INTERNAL_BASENAMES` and is not a
  pure registration frame (the frame whose function name matches the
  `it`/`describe` calls); use that as the throw site.

The result: a raw `TypeError`, a third-party throw, or any other
uncaught error gets the same `at <file>:<line>:<col>` + snippet
treatment matcher errors already enjoy.

### Why the Boa fix is "catch_unwind + JS audit"

There is no newer Boa to upgrade to (0.21.1 is current). Two paths:

- **Harness safety net.** `std::panic::catch_unwind` around the Boa
  `Context` drop in `run_file_with_coverage` (and any equivalent
  drop site in `run_file_with_loader` / `mutate.rs`) converts the
  finalizer panic into a structured error: the file's outcomes
  collected so far are still reported, and a synthetic
  `Failure { message: "Boa GC panic during teardown: ...", ... }`
  is appended. The process exits cleanly with non-zero status.
- **JS audit.** The memory note identifies the trigger pattern:
  inlined branches in one dispatcher function alongside
  parsed-but-unexecuted code. Sweep `runtime/*.js` for dispatcher
  functions with inline branches and extract each branch into its
  own function. Known starting point: `_commitEach` in
  `runtime/template.js` (where the memory note documents the fix
  was applied previously). Re-verify that fix is still in place and
  audit the rest of `template.js`, `reactivity.js`, `app.js`,
  `router.js`, `http.js`, `dom-shim.js`.

Both are required because neither alone is sufficient: the audit
fixes known triggers but can't prove there are no others; the
catch_unwind covers what we miss but doesn't fix the root cause.

### What "intermittent" means for L35

The friction-log entry says the panic appears on single-file
`zero test <path>` runs but not full-suite. Plausible explanation:
the full suite happens to exit before the GC pass that triggers the
panic completes, or the order of context drops differs. The audit
should not chase repro-determinism; the goal is to eliminate known
triggers and ensure the safety net catches the rest.

## Requirements

### 1. Unowned-effect cleanup (L33)

#### 1.1 Reactivity-side tracking

In `runtime/reactivity.js`:

- Add module-level `let _unownedEffects = new Set()`.
- In `effect(fn)`: after computing `_registeredScope`, if
  `_registeredScope == null`, add `stop` to `_unownedEffects`.
- In `stop()` itself: also `_unownedEffects.delete(stop)` so a
  manually-stopped unowned effect doesn't leave a stale entry.
- Add a new exported private function `_disposeUnownedEffects()`:
  iterate `[..._unownedEffects]`, call each `stop`, then
  `_unownedEffects.clear()`. The copy-to-array is required because
  each `stop` call removes itself from the set.

The export is named with a leading underscore (private convention).
It is not added to the public `zero` runtime exports; only
`runtime/test.js` imports it.

#### 1.2 `cleanup()` integration

In `runtime/test.js`:

- Import `_disposeUnownedEffects` from `./reactivity.js`.
- Call it at the start of `cleanup()`, before the existing
  `_renderTracker` disposal loop, so any unowned effects are torn
  down first. Order matters because a torn-down effect's cleanup
  callback may touch state that the render-scope disposal also
  touches.

#### 1.3 Self-tests

In `runtime/test.test.js`:

- A top-level `effect()` (no surrounding scope) is disposed by
  `cleanup()`; subsequent signal mutations do not re-fire it.
- A render-scope effect is still disposed by `cleanup()` (regression).
- A manually-stopped unowned effect doesn't error on `cleanup()`
  (no double-stop).
- The effect's cleanup callback (the `_cleanup` returned from the
  effect body) runs on `cleanup()`-driven disposal.

In `runtime/reactivity.test.js`:

- `effect()` outside any scope registers in `_unownedEffects` (white
  box; can introspect via the new helper).
- `effect()` inside `scope.run()` does *not* register in
  `_unownedEffects`.

#### 1.4 Documentation

Add a short subsection to `docs/testing.html` (and its source markdown):
"Effects in route and component bodies." States the pattern:
top-level `effect()` is disposed by `cleanup()` between tests in the
same file, which means a route relying on a singleton top-level
effect loses it after the first cleanup. Recommended pattern: place
effects inside the function called by `render()` so they live in the
render scope and re-fire each test. Cross-link from the friction-log
explanation in `best-practices.html` if one exists; otherwise omit.

### 2. Throw-location fix for non-matcher errors (L34)

#### 2.1 Harness stack parser

In `crates/zero-test-runner/src/harness.rs`:

- Locate the helper that picks a user frame from `err.stack` (the
  fallback path when `err._userFrame` is absent). Today it walks
  from the top of the stack and returns the first frame outside
  `FRAMEWORK_INTERNAL_BASENAMES`.
- Change the walk to also skip frames whose *function name*
  identifies a registration callsite — at minimum `it`, `describe`,
  `beforeEach`, `afterEach`, `beforeAll`, `afterAll`, `cleanup`,
  `render`. (These are the calls a test body sits *inside*, not
  the throw site.)
- If after filtering no frame remains, fall back to today's
  behavior: return the first non-internal frame, even if it's a
  registration frame. (Better wrong than empty.)

#### 2.2 Source-mapped location

The chosen frame is run through `remap_positions` so its `line:col`
points at the original `.ts` source. `Failure.location` is populated
from that frame. The reporter's snippet rendering is unchanged — it
already consumes `SourceLoc`.

#### 2.3 Self-tests

In `crates/zero-test-runner/src/harness.rs` tests:

- A test body that throws `null.foo` reports the throw line
  (the `null.foo` line), not the `it(...)` line.
- A test body that throws a custom `Error("boom")` reports the
  throw line.
- A matcher failure still reports the matcher call line (regression;
  the `_userFrame` path must continue to win when set).
- A `null.foo` thrown from an async test body still reports the
  throw line, not the `it()` registration.

### 3. Matcher API expansion (L36)

#### 3.1 `.not` chain

In `runtime/test.js`'s `expect(actual)`:

- Add a `.not` property on the returned object whose value is a
  second matcher object with the same matcher names. Each matcher
  on `.not` calls the same underlying check but inverts the
  pass/fail interpretation and rewrites the failure message to
  describe the negated expectation.
- The `.not` matchers throw via the same `_fail()` helper so
  `err._userFrame` is decorated identically.
- `.not.not` is **not** supported — `.not` returns a matcher object
  with no further `.not` property. (Document; no API for the
  double-negative.)
- `.not` works with every existing matcher: `toBe`, `toEqual`,
  `toBeTruthy`, `toBeFalsy`, `toBeNull`, `toContain`, `toThrow`,
  `toBeTemplateResult`, `toHaveBeenCalled`,
  `toHaveBeenCalledTimes`, `toHaveBeenCalledWith`,
  `toHaveBeenLastCalledWith`, and the new numeric matchers
  (§3.2).
- `.not.toMatchSnapshot()` keeps the current "not implemented"
  error message; snapshot testing is still out of scope.

Failure message format for negated matchers:

```
expect(<actual>).not.toBe(<expected>): values are strictly equal
expect(<actual>).not.toEqual(<expected>): values are deeply equal
expect(<actual>).not.toBeNull(): value is null
expect(<actual>).not.toContain(<item>): string contains substring
expect(spy).not.toHaveBeenCalled(): spy was called N time(s)
```

(Plan picks exact wording; the above are illustrative.)

#### 3.2 Numeric comparators

Add four new matchers to `expect(actual)` (and their `.not`
counterparts):

- `toBeGreaterThan(n)` — passes when `actual > n`.
- `toBeGreaterThanOrEqual(n)` — passes when `actual >= n`.
- `toBeLessThan(n)` — passes when `actual < n`.
- `toBeLessThanOrEqual(n)` — passes when `actual <= n`.

Each throws via `_fail()` with a message of the form:

```
expect(<actual>).toBeGreaterThan(<n>): <actual> is not greater than <n>
```

If `actual` or `n` is not a number, throw the same kind of
"value is not a number" error the existing `toContain` uses for
type mismatches.

#### 3.3 Ambient types

In `runtime/zero-test.d.ts`:

- Extend the matcher interface returned by `expect(actual)` to
  include a `.not` property whose type is a sibling interface
  containing every matcher (minus `.not` itself).
- Add `toBeGreaterThan`, `toBeGreaterThanOrEqual`, `toBeLessThan`,
  `toBeLessThanOrEqual` (and their `.not` mirrors).
- Loose typing is acceptable (numeric matchers take `number`;
  `.not.toThrow` takes the same `message?: string` as positive
  `toThrow`). Strict typing of negation results is not required.

`toBeUndefined()` and `toBeDefined()` were declared in the `Matcher` /
`NegatedMatcher` interfaces but were never implemented in `_buildPositive` /
`_buildNegative`; they were filled in by the follow-up
`issues/test-matcher-drift/` slice.

#### 3.4 Self-tests

In `runtime/test.test.js`, add coverage for each new matcher:

- `.not.toBe`, `.not.toEqual`, `.not.toBeNull`, `.not.toContain`,
  `.not.toBeTruthy`, `.not.toBeFalsy`, `.not.toThrow`,
  `.not.toBeTemplateResult`.
- `.not.toHaveBeenCalled`, `.not.toHaveBeenCalledTimes(n)`,
  `.not.toHaveBeenCalledWith(...)`, `.not.toHaveBeenLastCalledWith(...)`.
- Pass and fail cases for each, asserting on the message content.
- `toBeGreaterThan(n)` / `toBeLessThan(n)` etc. — pass / fail /
  equal-boundary cases (boundary should pass for `*OrEqual`,
  fail for the strict variant).
- Non-number argument throws clearly.

### 4. Boa GC panic safety net + JS audit (L35)

#### 4.1 Harness `catch_unwind` wrapper

In `crates/zero-test-runner/src/harness.rs`:

- Wrap the body of `run_file_with_coverage` (and any sibling
  `run_file_*` function with the same Boa-Context-drop pattern) in
  `std::panic::catch_unwind`. Use `AssertUnwindSafe` on the closure
  capture since Boa types aren't `UnwindSafe`.
- On panic, return a `RunOutcome` whose `result` contains:
  - The outcomes collected so far (preserve all `it` results that
    completed before the panic).
  - An additional synthetic outcome at the end with
    `status: Status::Failed`, name `"<Boa GC panic during teardown>"`,
    and `failure: Some(Failure { message: <captured panic
    message>, stack: None, location: None })`.
  - `load_error: None` (the file did load and run; the panic was
    teardown).
- The reporter must render this synthetic outcome like any other
  failure — no special-casing required because `Failure` already
  permits absent `location` / `stack`.

#### 4.2 Subprocess-mode mutation worker

`crates/zero-test-runner/src/mutate.rs` runs mutants in subprocesses
(per the test-improvements spec). Each subprocess invocation must
also catch the panic and exit non-zero with a structured stderr
message that the parent process recognizes and counts as
`errored` (the existing mutant-status bucket for "runner itself
crashed"). No new mutant status; reuse `errored`.

#### 4.3 Runtime JS audit

Sweep these files for the inlined-branch pattern (multiple branches
inside one dispatcher function alongside parsed-but-unexecuted
helper code):

- `runtime/template.js` — verify the `_commitEach` /
  `_commitEachKeyed` split documented in memory
  `boa-maplock-finalizer` is still in place; audit other
  dispatchers (look for functions with multiple early-return
  branches over `if (typeof X === "...")` discriminators).
- `runtime/reactivity.js` — short and audited; no known dispatchers
  but verify.
- `runtime/app.js`, `runtime/router.js`, `runtime/http.js`,
  `runtime/dom-shim.js`, `runtime/test.js` — same audit.

For any dispatcher found with inline branches, refactor to one
function per branch. The plan should keep this audit narrow: only
refactor where the pattern matches the memory note's description;
do not opportunistically restructure other code.

#### 4.4 Self-tests

- A unit test in `harness.rs` simulates a Boa-side panic (e.g. via
  a JS module that triggers the known pattern; or, if no reliable
  trigger exists, by injecting a `panic!()` in test-only code
  inside the `catch_unwind` scope) and asserts that the harness
  returns a `RunOutcome` with the synthetic GC-panic failure
  appended, not a process crash.
- The existing `cargo test -p zero --test examples_tests
  tracker_tests_pass` continues to pass after the JS audit.

#### 4.5 Memory note update

After the audit, update `boa-maplock-finalizer` memory to reflect
which files were audited and what the current trigger surface looks
like. (Memory write is part of the slice's deliverable; the agent
implementing this writes it as the final step.)

### Spec text amendments

- `issues/test-runner/spec.md` — Open Question on "Error stack
  traces" notes that Boa attributes may be wrong. Add a brief
  "delivered by `issues/test-correctness/`" line under the same
  "Shipped in a follow-up slice" section that already documents
  the test-improvements deliverables (around line 450).
- `issues/test-improvements/spec.md` — no edits needed; this slice
  doesn't touch coverage / mutation / source snippets except for
  the catch_unwind wrapping in §4.
- `runtime/zero-test.d.ts` — see §3.3.
- `docs/testing.html` (or its markdown source) — see §1.4 and
  add the new matcher names to the matcher reference list.

## Constraints

- **No new npm dependencies.** Pure JS / Rust changes inside the
  existing workspace.
- **No Boa upgrade.** 0.21.1 is the latest available; do not pin
  to a git branch or unreleased version.
- **`_disposeUnownedEffects()` stays private.** Underscore prefix,
  not added to `ZERO_RUNTIME_EXPORTS`, not reachable by user code
  from `import { ... } from "zero"`. Only `runtime/test.js`
  consumes it.
- **`.not.not` is not supported.** No double-negation chain.
- **Numeric matchers are JS-numeric only.** No `BigInt` support
  unless trivially free; if `BigInt` comparison is awkward, the
  matchers throw the same "value is not a number" error.
- **`.not.toThrow` semantics.** `.not.toThrow()` (no message)
  passes when the function doesn't throw, fails when it does.
  `.not.toThrow(msg)` passes when the function either doesn't
  throw or throws an error whose message does *not* contain `msg`.
  (Mirrors Jest/Vitest.)
- **`.not.toMatchSnapshot()` still throws the "not implemented"
  error.** Snapshot testing remains out of scope.
- **Catch_unwind wraps teardown, not the whole run.** Test-body
  panics inside Boa are surfaced as normal failures by Boa itself;
  the `catch_unwind` exists specifically to convert finalizer /
  drop-time panics into structured errors. The plan picks the
  exact wrap boundary but it should be narrow enough that genuine
  test-execution panics aren't swallowed.
- **The runtime JS audit is narrow.** Only refactor dispatchers
  matching the memory note's inlined-branch pattern; don't
  opportunistically split other functions.
- **No retry-on-panic.** A teardown panic surfaces as one synthetic
  failure; the runner does not re-execute the file.
- **No per-test isolation change.** Tests within a file still share
  state; per-test fresh contexts remain out of scope (much heavier
  spec, separate from this slice).
- **The harness stack parser still prefers `err._userFrame`.** §2
  only changes the fallback walk; matcher-decorated errors keep
  their current behavior.
- **Lint coverage is out of scope.** A lint rule warning about
  top-level `effect()` in route/component bodies would be useful
  but lives in a separate slice (`zero-lint`). This spec only
  fixes the runtime symptom.

## Out of Scope

- A lint rule for top-level `effect()` calls in route/component
  bodies.
- Per-test module re-evaluation (resetting modules between `it`
  blocks). Heavier change; this spec accepts the
  "singleton-per-file" trade-off.
- `expect.assertions(n)` and `expect.hasAssertions()` style
  utilities.
- Asymmetric matchers (`expect.anything()`, `expect.any(Number)`,
  etc.).
- `toMatchObject`, `toMatchInlineSnapshot`, snapshot testing
  generally.
- A double-negation `.not.not` chain.
- A `--bail` flag triggered by the GC panic.
- Boa engine fork / patch carrying the MapLock fix. (If a fix
  lands upstream, a separate slice will pick it up.)
- Reworking the per-file-context model into per-`it`-context.
- Color or TTY-aware output in the reporter.
- Async-quiescence helper (`settled()`).
- Adding `.not.toMatchSnapshot()` semantics; the underlying
  matcher is still a stub.

## Open Questions

- **Where exactly to wrap `catch_unwind` in `harness.rs`.** The
  Boa `Context` is owned by `run_file_with_coverage`; wrapping the
  whole function works but is broad. A tighter wrap around just
  the drop site (`drop(context)`) requires moving the context into
  a scoped block. Plan picks; recommendation: wrap the entire
  function body so any panic — load, run, or drop — surfaces
  cleanly.
- **`.not` failure message phrasing.** Two options:
  (a) literal inversion: `"expect(x).not.toBe(y): values are
  strictly equal"`; (b) descriptive: `"expected x not to be
  strictly equal to y"`. Recommendation: (a) — mirrors positive
  matcher style and is easy to scan.
- **Whether `.not.toHaveBeenCalledWith(...)` lists the matching
  call.** A negated matcher fails because *some* call matched.
  The failure message could enumerate the matching call(s) or
  just the recorded-call count. Recommendation: list the matching
  call (single-call most common) plus the total count; symmetric
  with the positive matcher's "all recorded calls" line.
- **Should the synthetic GC-panic failure include the panic message
  verbatim?** Boa panic messages can be verbose. Recommendation:
  include the full message in `Failure.message` but truncate to
  the first line (or 200 chars) when shown by the reporter; the
  JSON / programmatic surface keeps the full text.
- **Audit scope for `runtime/*.js`.** The memory note implicates
  `template.js` specifically. Should the audit look beyond the
  named pattern (inlined branches in a dispatcher) for *other*
  Boa GC pitfalls we don't yet know about? Recommendation: no —
  scope creep, and we don't have other documented triggers. If a
  new pattern surfaces, a follow-up slice handles it.
- **`_disposeUnownedEffects()` placement.** Could live as a method
  on a singleton, a free function, or be invoked indirectly via a
  scope-style API. Recommendation: free function exported with
  underscore prefix; matches `_createScope`'s existing precedent
  in the file.
- **Should the throw-location fallback also try harder to find a
  `.ts` frame over a `.js` frame?** When a project has mixed
  TypeScript and JavaScript, the user-authored `.ts` frame is
  more useful than a transpiled `.js` frame. Recommendation: no
  preference — both go through `remap_positions` and end up
  pointing at the original `.ts` if the source map exists. If a
  pure `.js` user file is on the stack, that's fine to point at.
- **Documentation cross-link for the unowned-effect trade-off.**
  The "effects in route bodies become singletons in tests" point
  is subtle. Should it live in `testing.html`, `best-practices.html`,
  or both? Recommendation: testing.html as the canonical home;
  one-line callout in best-practices.html if a "tests" section
  exists there.
- **Test for the GC-panic safety net.** If we can't construct a
  reliable JS-side trigger (the memory note calls the panic
  "intermittent"), the unit test injects a deliberate `panic!()`
  inside the Boa scope from Rust to exercise the `catch_unwind`
  path. Plan confirms whether the injection mechanism is feasible
  without leaking test-only code into the production harness.
