# Spec: Framework papercuts — L12 diagnostic, http fetch shape docs, S01 in test bodies

## Problem Statement

Three independent 🟢 papercuts from the demo's friction log (`~/Documents/code/zero_demo/FRAMEWORK_NOTES.md`). Each is small enough that a dedicated spec would be overhead; bundling lets one planner sequence them.

- **L41 — L12 diagnostic doesn't say where the fix lives.** The SCSS lint rule "L12" flags `align-items: center` (and similar) in a project-local class and tells the user `use the .{utility} utility class instead of writing this inline`. The natural reading of "inline" is `style="…"`, not "move the class to the markup." Users hit the rule, get confused, and try to add the utility class to the same SCSS selector — wrong fix. The message needs to name the actual remediation path: drop the property from SCSS and add the utility class to the markup. Friction-log entry: `~/Documents/code/zero_demo/FRAMEWORK_NOTES.md:41`.
- **L42 — `http.get`/`http.post`/etc. pass a `Request` to `fetch`, not a `(url, init)` pair.** `runtime/http.js:111` always constructs `new Request(url, requestInit)` and dispatches that. The injected `fetch` (via `createHttp({ fetch })`) receives a single `Request` argument. Tests that spy on `fetch` and assert on the URL must read `(arg as Request).url`, not `arg`. This is correct behavior, but undocumented in `docs/http.md` — every author of a fetch spy trips over it the first time. Friction-log entry: `~/Documents/code/zero_demo/FRAMEWORK_NOTES.md:42`.
- **L55 — S01 (function body ≤ 80 lines) fires on `describe(…)` callbacks in test files.** Test files are exempt from the T-rules and R03 (`crates/zero-lint/src/js/rules/t01_event_listener.rs:17`, etc.) but not from S01 (`crates/zero-lint/src/js/rules/s01_function_size.rs` — no `is_test_file` check). A `describe` block wraps a list of `it` calls; growing past 80 lines is a natural function of test count. The rule forces artificial splits with duplicated `beforeEach` plumbing. Friction-log entry: `~/Documents/code/zero_demo/FRAMEWORK_NOTES.md:55`.

## Background

### L12's current message and selector-path skip logic

`crates/zero-lint/src/rules/alignment.rs:33` formats the diagnostic as:

```rust
message: format!("use the .{utility} utility class instead of writing this inline"),
```

`selector_path_references_utility` (lines 37-50) already suppresses the rule when the SCSS selector itself references an alignment utility class — that's the "override site" escape hatch. The rule's design is correct: `_alignment.scss` ships utilities (`align-center`, `justify-end`, etc.) and the framework's stance is that alignment lives in the class list, not in component-local SCSS.

The diagnostic's failure mode is purely the word "inline." A SCSS author reads `"inline"` and reaches for `style="…"`; the real remediation is to remove the declaration from SCSS entirely and add the utility class name to the consuming element. The message also doesn't name which file to edit (`styles/app.scss` vs the component's template) — leaving the user to guess.

### What `http` actually passes to `fetch`

`runtime/http.js:78-83`:

```js
request(input, init) {
  return _sendRequestLike(input, init, middlewares, baseFetch);
},
```

`_send` (line 95): builds `requestInit`, then `const req = new Request(url, requestInit)` (line 111), then `_dispatch(req, …)`. `_sendRequestLike` (line 123) similarly normalizes its `input` to a `Request` before dispatch. `_dispatch` (line 140) walks the middleware chain and ultimately calls `baseFetch(nextReq)` — a single `Request` argument.

This means a test that does:

```ts
const fetchSpy = spy<typeof fetch>(async () => new Response("…"));
const http = createHttp({ fetch: fetchSpy });
await http.get("/users/42");
expect(fetchSpy.calls[0][0]).toBe("/users/42");  // FAILS
```

…fails because `fetchSpy.calls[0][0]` is a `Request`, not a string. The correct assertion is:

```ts
const req = fetchSpy.calls[0][0] as Request;
expect(req.url).toMatch(/\/users\/42$/);
expect(req.method).toBe("GET");
```

`docs/http.md` covers middleware, `HttpError`, and `init.fetch` threading. It does **not** document the fetch-receives-a-Request contract or show how to write a fetch spy. The doc owner (the framework) has the answer; the reader doesn't.

### S01's visitor and the test-file exemption pattern

`crates/zero-lint/src/js/rules/s01_function_size.rs:21-28`: `check(ctx: &FileCtx<'_>)` runs an `S01Visitor` against `ctx.module` and returns diagnostics. The visitor implements `visit_fn_decl`, `visit_fn_expr`, `visit_arrow_expr`, `visit_class_method`, `visit_private_method`, `visit_constructor`, `visit_method_prop`, `visit_getter_prop`, `visit_setter_prop`. Every body span > 80 lines reports.

No `is_test_file` short-circuit. Compare `crates/zero-lint/src/js/rules/t01_event_listener.rs:17`:

```rust
if !ctx.is_under_components_or_routes || ctx.is_test_file {
    return Vec::new();
}
```

The pattern for exempting test files is already established and one-line. The `FileCtx::is_test_file` field exists (`crates/zero-lint/src/js/context.rs:24`) and is set by `is_test_basename(file)` against `*.test.{ts,tsx,js,jsx}` / `*.spec.{ts,tsx,js,jsx}` (verified by reading `context.rs:50` and the `detects_test_file_by_basename` test at `context.rs:168`).

### Adjacent surfaces

- **L41 (lint):** `crates/zero-lint/src/rules/alignment.rs` — message string and possibly a docs cross-reference. Existing tests in the same file assert on the rule code + utility name, not the message wording — they may or may not break depending on the exact phrasing. `docs/linting.md` has the public rule reference; the message change ripples there if the doc reproduces the message verbatim.
- **L42 (http docs):** `docs/http.md` only. No runtime change. `runtime/http.js`'s JSDoc on `_dispatch` already documents the internal contract; surfacing it to user-facing docs is the gap. `crates/zero-scaffold/src/scaffold/AGENTS.md` already mentions the http section by link — a one-line nod in AGENTS.md is optional, planner's call.
- **L55 (lint):** `crates/zero-lint/src/js/rules/s01_function_size.rs` — one-line short-circuit (or a more targeted skip — see R3.1). Existing S01 tests in the file run via `run_at(source, "/tmp/src/app.ts")`; new tests for the test-file path go through `run_at(source, "/tmp/src/foo.test.ts")` (or similar). `docs/linting.md`'s S01 row gains a note about the test-file exemption.

## Requirements

### R1 — L12 diagnostic names the markup as the remediation path

`crates/zero-lint/src/rules/alignment.rs:33` updates the message to make the fix unambiguous. Recommended wording:

> `move this to the markup as class="{utility}" instead of declaring it on a project-local selector`

Or, accommodating the property family (alignment may not all be class-on-the-target — `align-self` lands on the child):

> `apply class="{utility}" on the element instead of declaring this property on a project-local selector`

The contract: the message must **not** contain the word "inline" (the word that misled the friction-log author). It **must** name "markup" / "class" / "element" — the actual locus of the fix. It **must** name the utility name (`{utility}`) so the user sees the exact class to use.

Exact wording is a planner choice as long as it satisfies the above. Existing tests in `crates/zero-lint/src/rules/alignment.rs` (if any assert on message text) get updated to match.

`docs/linting.md` — the L12 row's "fix" column updates to match the new message. If the row currently quotes the old message, regenerate. The teaching paragraph for L12 (if one exists) gains one sentence: *"L12's fix is to drop the declaration from SCSS and add the utility class to the consuming element — not to add the utility class to the same SCSS rule."*

### R2 — `docs/http.md` documents the fetch contract and how to spy on it

`docs/http.md` gains a section (suggested title: `## Spying on fetch in tests`, placed after the existing "Route-scoped fetch threading" section).

Content must include:

- **The fact:** every method on the client (`get`, `post`, `put`, `patch`, `delete`, `request`) builds a `Request` from its arguments and passes that single `Request` to the injected `fetch`. The injected `fetch` never receives a `(url, init)` pair.
- **Why:** middleware operates on `Request` (`docs/http.md` already mentions this); building a single canonical `Request` at the boundary keeps middleware and the underlying fetch on the same shape.
- **Test pattern:** a complete example showing how to assert on URL and method through a fetch spy.

Suggested code block (planner refines as needed):

```ts
import { createHttp } from "zero/http";
import { spy } from "zero/test";

it("requests the user record", async () => {
  const fetchSpy = spy<typeof fetch>(
    async () => new Response(JSON.stringify({ id: 42 }), {
      headers: { "Content-Type": "application/json" },
    })
  );
  const api = createHttp({ fetch: fetchSpy });

  await api.get("/users/42");

  const req = fetchSpy.calls[0][0] as Request;
  expect(req.url).toMatch(/\/users\/42$/);
  expect(req.method).toBe("GET");
});
```

If `docs/http.md` already shows a test example using a different shape (e.g. asserting on a string URL), the planner updates that example to use the `Request`-aware pattern. Do not leave both shapes in the docs.

Optional cross-link to add: `AGENTS.md` has an "Imports" line for `zero/http`; the new docs section gets one line nodding to it. Planner's call; not a contract requirement.

### R3 — S01 exempts test files

`crates/zero-lint/src/js/rules/s01_function_size.rs:21` (or earliest point in `check` before walking) adds:

```rust
if ctx.is_test_file {
    return Vec::new();
}
```

Same one-line pattern T01/T02/T03/T04/R03 use. After R3, S01 emits zero diagnostics on `*.test.{ts,tsx,js,jsx}` / `*.spec.{ts,tsx,js,jsx}` files.

#### R3.1 — Documentation

`crates/zero-scaffold/src/scaffold/AGENTS.md:252-253` currently states:

> *Tests (`*.test.{ts,js,tsx,jsx}` / `*.spec.{ts,js,tsx,jsx}`) are exempt from the T-rules and R03; everything else still applies.*

Update to: `… exempt from the T-rules, R03, and S01; everything else still applies.`

`docs/linting.md` — the S01 rule entry gains a note: *"Test files (`*.test.{ts,js,…}` and `*.spec.{ts,js,…}`) are exempt — `describe` bodies grow with test count, which is structural rather than a code smell."*

#### R3.2 — Tests

`crates/zero-lint/src/js/rules/s01_function_size.rs`'s test module gains:

- `does_not_fire_in_test_file` — analogous to the existing `does_not_fire_in_test_file` tests on T-rules. Run a 90-line function through `run_at(source, "/tmp/src/foo.test.ts")` and assert `d.is_empty()`.
- `does_not_fire_on_describe_in_test_file` — a `describe("X", () => { …90 lines… })` in a test file produces no diagnostics. Demonstrates the friction-log scenario directly.
- The existing `fires_on_oversized_function_decl` test at line 170 continues to pass (it uses the default `/tmp/src/app.ts` path which is not a test file).

### R4 — End-to-end

Run `cargo test --workspace` after each change. The three sections are independent — landing them as separate commits is fine; R1, R2, R3 do not share files.

## Constraints

- No npm dependencies; no new workspace crates.
- No public API change. R1 is a message string. R2 is docs. R3 is a one-line short-circuit guarded by the existing `is_test_file` field.
- The 80-line per-function guideline (CLAUDE.md) applies to any new function. None of the requirements add a function.
- Existing tests must keep passing without modification *except* L12 tests that assert on message text and S01 tests if any of them implicitly rely on the path being non-test. Spec note: planner runs `cargo test --workspace` once before any change to establish the baseline.
- R2 must not contradict `docs/http.md`'s existing "Route-scoped fetch threading" section — both can coexist; the new section is about spying on fetch, the existing one is about per-call fetch substitution. Cross-reference if useful.
- R3 widens the test-file exemption to S01. It does **not** change which file extensions count as "test." If `is_test_basename` ever changes (e.g. picking up `.test.tsx` glob inconsistencies), that's a separate fix.
- The L12 message change must remain interpolatable with `{utility}` and any other parameters the planner adds. Do not hardcode utility names into the format string.

## Out of Scope

- **Auto-fix for L12.** A `--fix` mode that rewrites SCSS to drop the property and emits a diagnostic to add the class on the markup side requires HTML/JSX-side AST awareness the linter doesn't have. Out of scope.
- **A general "rule message readability" audit.** Only L12 is in this spec. Other rules' wording stays as-is unless a friction-log entry surfaces them.
- **`zero http` API changes.** R2 documents existing behavior; it does not propose changing the boundary shape. If a future contributor wants to support `(url, init)` shape for the injected fetch, that's a separate spec.
- **Adding a `spy` helper specifically for HTTP** (e.g. `spyHttp(api, …)`). R2 demonstrates the existing `spy` from `zero/test` — that's the supported test surface.
- **Per-rule S01 thresholds.** R3 widens the *exemption*; the 80-line target stays as-is for non-test files. No CLI flag to raise the limit.
- **Exempting only `describe` callbacks (not the whole file).** Considered; the file-level exemption is simpler, lower-risk, and aligns with the precedent set by every T-rule. The trade-off (a long helper function in a test file slips through) is acceptable given the typical shape of test code. See Open Questions for the narrower alternative if you'd rather pursue it.
- **Updating the demo's `~/Documents/code/zero_demo/web/AGENTS.md`.** It refreshes via `zero update`; users pick up the AGENTS.md edits then.

## Open Questions

- **L55 scope: file-level or callback-level exemption?** Spec recommends file-level (R3) for simplicity and precedent alignment. Alternative: visit only function nodes that are *not* arrow expressions passed as the 2nd argument to `describe(…)` / `it(…)` / `beforeEach(…)` / `beforeAll(…)` / `afterEach(…)` / `afterAll(…)` calls; this preserves S01 enforcement on named helpers inside test files. The narrower approach catches more bugs but requires AST-call-site recognition and is more code. If you want the narrower fix, the spec needs to extend R3 with a "recognize test-runner callback site" visitor.
- **L12 message wording.** Spec proposes two candidate phrasings; planner picks one. The contract is "names the markup as the fix path, names the utility class." A user-test (read the message cold) is worth more than spec-time word-smithing.
- **`docs/http.md` placement of the new section.** Spec recommends "after Route-scoped fetch threading"; planner reads the current file's flow and places it where it reads best.
- **AGENTS.md cross-link for R2.** AGENTS.md's existing "imports" line for `zero/http` (line 94) could gain a one-line "see http.html#spying-on-fetch-in-tests" nod. Not required; planner judges whether it crowds the import block.
- **Whether to add a `Request`-typed assertion helper to `zero/test`.** E.g. `expectRequest(spy.calls[0][0]).hasUrlMatching(…)`. Out of scope per the "Out of Scope" list, but worth filing as a follow-up if R2's recommended pattern feels heavy in practice.
