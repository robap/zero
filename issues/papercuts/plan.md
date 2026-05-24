# Plan: Framework papercuts — L12 diagnostic, http fetch shape docs, S01 in test bodies

## Summary

Three independent, low-blast-radius improvements bundled per the spec.
R1 rewrites the L12 alignment diagnostic to name "markup" / "class" /
"element" instead of the misleading word "inline." R2 adds a "Spying on
fetch in tests" section to `docs/http.md` documenting the existing
contract that the injected `fetch` always receives a single `Request`
argument. R3 widens the existing `is_test_file` exemption to S01 so
`describe(…)` blocks in test files no longer get split for line count.

Each requirement touches a different file set; landing them as three
separate commits is feasible and recommended.

## Prerequisites

None. Open questions in the spec are resolved by this plan:

- **L12 wording** — settled on `apply class="{utility}" on the element
  instead of declaring this property on a project-local selector` (works
  for both `align-items` on parent and `align-self` on child).
- **S01 scope** — file-level exemption (the simpler precedent-aligned
  option), not the narrower describe-callback-only variant.
- **`docs/http.md` placement** — new section placed after "Route-scoped
  fetch threading," before "One client per backend."
- **AGENTS.md cross-link for R2** — skipped (would crowd the imports
  block; doc link from `docs/http.md` is sufficient).

## Steps

- [x] **Step 0: Baseline test run**
- [x] **Step 1: Update L12 diagnostic message (R1)**
- [x] **Step 2: Update `docs/linting.md` L12 row (R1)**
- [x] **Step 3: Document the fetch-receives-a-Request contract (R2)**
- [x] **Step 4: Exempt test files from S01 (R3)**
- [x] **Step 5: Update S01 documentation (R3.1)**
- [x] **Step 6: Full workspace verification**

---

## Step Details

### Step 0: Baseline test run

**Goal:** Establish a green baseline before any edit so any test failure
later is clearly attributable to the change just made.

**Files:** None modified.

**Changes:** Run `cargo test --workspace` and confirm it passes. If any
test fails on `main`, stop and report to the user — the plan assumes a
clean baseline.

**Tests:** This step *is* the test run.

---

### Step 1: Update L12 diagnostic message (R1)

**Goal:** Replace the misleading "inline" wording in the L12 diagnostic
so users know the fix lives in the markup (class list), not in the
SCSS selector.

**Files:**
- `crates/zero-lint/src/rules/alignment.rs`

**Changes:**

At line 33, change:

```rust
message: format!("use the .{utility} utility class instead of writing this inline"),
```

to:

```rust
message: format!(
    "apply class=\"{utility}\" on the element instead of declaring this property on a project-local selector"
),
```

This wording satisfies the spec's contract:

- Does **not** contain the word "inline."
- Names "element" (markup) — the locus of the fix.
- Names `class="{utility}"` — the literal markup the user types.
- Keeps `{utility}` as an interpolated parameter (no hardcoded
  utility names).

Existing tests in `crates/zero-lint/tests/alignment_rule.rs` assert
that the message **contains** `align-center`, `justify-center`,
`justify-between`, `text-center` (lines 36–37, 46, 55). The new
message contains those literal substrings inside `class="…"`, so
those assertions continue to pass without modification.

**Tests:**
- `cargo test -p zero-lint` — all existing L12 tests stay green.
- No new unit test needed; the message contract is enforced by the
  spec's wording rules, which a reviewer verifies by reading the
  format string. Adding a "must not contain 'inline'" assertion
  would be over-specification.

---

### Step 2: Update `docs/linting.md` L12 row (R1)

**Goal:** Keep the rule reference aligned with the new diagnostic and
add a one-sentence teaching note that explicitly steers readers away
from the wrong-fix path.

**Files:**
- `docs/linting.md`

**Changes:**

The current L12 row at line 51 (`Don't write` / `Use instead`) doesn't
quote the diagnostic message — it shows code patterns — so it doesn't
require regeneration. It can stay as-is.

Add a small teaching note immediately after the SCSS table (before the
"## JS/TS framework idiom rules" heading at line 54). Insert a brief
paragraph:

```markdown
L12's fix is to drop the declaration from SCSS and add the utility
class to the consuming element — not to add the utility class to the
same SCSS rule. The selector keeps its semantic name; alignment lives
in the class list.
```

This satisfies the spec's R1 docs requirement without restructuring
the table.

**Tests:** Docs change; no automated test. Manual review of the
rendered markdown.

---

### Step 3: Document the fetch-receives-a-Request contract (R2)

**Goal:** Make it discoverable that `client.get`/`post`/etc. always
pass a single `Request` to the injected `fetch`, so authors of fetch
spies write the correct assertion shape on the first try.

**Files:**
- `docs/http.md`

**Changes:**

Insert a new section between "Route-scoped fetch threading" (ends at
line 175) and "One client per backend" (starts at line 177).

The new section:

````markdown
## Spying on fetch in tests

Every client method (`get`, `post`, `put`, `patch`, `delete`,
`request`) normalises its arguments into a single `Request` and
passes **that one `Request`** to the injected `fetch`. The injected
`fetch` never receives a `(url, init)` pair — only `(req)`.

This is the same shape middleware sees (middleware operates on
`Request` — see [Middleware](#middleware) above), so the boundary
stays uniform from middleware all the way down to the network call.

When you spy on `fetch` in a test, read `(arg as Request).url` /
`.method` / `.headers` — not `arg` itself:

```ts
import { createHttp } from "zero/http";
import { spy } from "zero/test";

it("requests the user record", async () => {
  const fetchSpy = spy<typeof fetch>(
    async () => new Response(JSON.stringify({ id: 42 }), {
      headers: { "Content-Type": "application/json" },
    }),
  );
  const api = createHttp({ fetch: fetchSpy });

  await api.get("/users/42");

  const req = fetchSpy.calls[0][0] as Request;
  expect(req.url).toMatch(/\/users\/42$/);
  expect(req.method).toBe("GET");
});
```

`req.url` is the fully resolved URL (relative URLs are resolved
against the document base). Use a regex or `endsWith` rather than
strict equality if you care about the path but not the origin.
````

This new section satisfies R2's contract:

- **The fact** — first paragraph: every method builds a `Request`,
  injected fetch receives only `(req)`.
- **Why** — second paragraph: keeps the boundary shape uniform with
  middleware.
- **Test pattern** — code block with a complete, runnable example
  asserting URL and method through a fetch spy.

No existing test example in `docs/http.md` uses the
"assert-on-string-URL" shape that the spec warned about, so there's no
shape contradiction to clean up. (Verified by reading the current file
top-to-bottom.)

**Tests:** Docs change; no automated test. Manual review of the
rendered markdown, ideally pasted into the in-tree showcase to
confirm code formatting.

---

### Step 4: Exempt test files from S01 (R3)

**Goal:** Stop S01 from firing on `describe(…)` callbacks (and
anything else) in test files, mirroring the existing T-rules / R03
test-file exemption.

**Files:**
- `crates/zero-lint/src/js/rules/s01_function_size.rs`

**Changes:**

In `check` (currently at lines 21–28), add a single short-circuit
before constructing the visitor:

```rust
pub fn check(ctx: &FileCtx<'_>) -> Vec<Diagnostic> {
    if ctx.is_test_file {
        return Vec::new();
    }
    let mut v = S01Visitor {
        ctx,
        diags: Vec::new(),
    };
    ctx.module.visit_with(&mut v);
    v.diags
}
```

Identical pattern to `crates/zero-lint/src/js/rules/t01_event_listener.rs:17`.
The `FileCtx::is_test_file` field is already populated by
`is_test_basename` in `crates/zero-lint/src/js/context.rs:50` (verified
in this plan's orientation step).

**Tests:**

Add two new tests to the existing `tests` module in
`s01_function_size.rs` (after line 225, before the closing `}`):

```rust
#[test]
fn does_not_fire_in_test_file() {
    let body = big_body(90);
    let src = format!("function f() {body}");
    let d = run_at(&src, "/tmp/src/foo.test.ts");
    assert!(d.is_empty(), "expected none, got {d:?}");
}

#[test]
fn does_not_fire_on_describe_in_test_file() {
    // A `describe("X", () => { ... })` block that exceeds 80 lines
    // is the friction-log scenario: tests accrete `it` calls and
    // S01 should not push authors into artificial splits.
    let body = big_body(90);
    let src = format!("describe(\"X\", () => {body});");
    let d = run_at(&src, "/tmp/src/foo.test.ts");
    assert!(d.is_empty(), "expected none, got {d:?}");
}
```

The existing `fires_on_oversized_function_decl` at line 170 keeps
passing because `run(&src)` uses `/tmp/src/app.ts` (non-test path).
No other existing S01 test uses a test-file path.

Run `cargo test -p zero-lint` to confirm both new tests pass and no
regressions.

---

### Step 5: Update S01 documentation (R3.1)

**Goal:** Surface the new exemption in the two places that catalog
test-file exemptions, so a future contributor sees the rule's actual
behaviour.

**Files:**
- `crates/zero-scaffold/src/scaffold/AGENTS.md`
- `docs/linting.md`

**Changes:**

**`crates/zero-scaffold/src/scaffold/AGENTS.md`** — at lines 252–253,
replace:

```markdown
Tests (`*.test.{ts,js,tsx,jsx}` / `*.spec.{ts,js,tsx,jsx}`) are exempt
from the T-rules and R03; everything else still applies.
```

with:

```markdown
Tests (`*.test.{ts,js,tsx,jsx}` / `*.spec.{ts,js,tsx,jsx}`) are exempt
from the T-rules, R03, and S01; everything else still applies.
```

**`docs/linting.md`** — at the "Test-file exemptions" section
(lines 76–86), update the two paragraphs:

Replace:

```markdown
`*.test.{ts,js,tsx,jsx}` and `*.spec.{ts,js,tsx,jsx}` files are
exempt from the **T-rules** and **R03**. Tests legitimately
reach into the DOM (`querySelector`-style assertions, custom
event dispatch helpers) and legitimately declare module-level
signals as test fixtures.

`R02`, `C01`, `C02`, `I01`, `I02`, and `S01` still apply in
tests — they're about correctness or code health, not about
framework-idiomatic UI code.
```

with:

```markdown
`*.test.{ts,js,tsx,jsx}` and `*.spec.{ts,js,tsx,jsx}` files are
exempt from the **T-rules**, **R03**, and **S01**. Tests legitimately
reach into the DOM (`querySelector`-style assertions, custom event
dispatch helpers), legitimately declare module-level signals as test
fixtures, and `describe` bodies grow with test count — structural,
not a code smell.

`R02`, `C01`, `C02`, `I01`, and `I02` still apply in tests —
they're about correctness, not framework-idiomatic UI code.
```

**Tests:** Docs change; no automated test.

---

### Step 6: Full workspace verification

**Goal:** Confirm the bundle of changes is green end-to-end and no
unanticipated test relied on the old L12 message or on S01 firing in
a test file.

**Files:** None modified.

**Changes:** Run:

```bash
cargo test --workspace
```

If anything fails, the failure points to a missed assumption in one of
the prior steps — fix in place rather than amending earlier steps.

Optionally also run `cargo run -p zero -- test` to confirm the JS test
runner is unaffected (it should be; R3 is the only rule change and it
only loosens S01, never tightens it).

**Tests:** The workspace test suite is the test for this step.

---

## Risks and Assumptions

- **L12 wording is purely a planner judgment call.** If the new wording
  reads worse than the original to the user, R1 needs a re-spin. The
  contract (no "inline," names "class"/"element," names utility) is
  met; the exact phrasing is replaceable.
- **R2 assumes `docs/http.md` is the right surface.** If the framework
  later grows a dedicated "Testing HTTP" doc, this section should
  migrate (or cross-link). Not a blocker.
- **R3 widens — not narrows — S01.** Files that previously failed will
  now pass; no file that previously passed can now fail. So no risk of
  surprising regressions in user projects on `zero update`.
- **The narrower alternative for S01** (only exempting `describe` /
  `it` / `before*` / `after*` callback bodies) is rejected by this
  plan per the spec's recommendation. If that turns out to be the
  wrong call (e.g. a 200-line helper in a test file slips through and
  causes pain), the narrower fix is a follow-up issue, not a revision
  of this plan.
- **No new dependencies, no new crates, no public API surface
  changes.** The only behaviour change is the L12 message text and the
  S01 exemption gate — both forward-compatible.
