# Review

Review a completed implementation against its spec and plan. Verify correctness,
completeness, and code quality, then produce a written report.

The issue slug to review is: $ARGUMENTS

---

## Step 1 — Orient

Read all of the following before forming any opinion:

- `issues/$ARGUMENTS/spec.md` — the requirements the implementation must satisfy
- `issues/$ARGUMENTS/plan.md` — the steps that were planned and should all be checked off
- `CLAUDE.md` — architecture, domain model, and conventions
- All file names under `crates/` and `runtime/` — the full current state of the implementation

---

## Step 2 — Verify mechanical completion

1. Confirm every step in the `## Steps` checklist in `plan.md` is marked `[x]`.
   If any are still `[ ]`, note them — the implementation is incomplete.

2. Run the test suite:
   ```
   cargo test
   ```
   If tests fail, note each failure. Do not proceed as if the implementation is
   correct when tests are red.

3. Run:
   ```
   cargo clippy
   ```
   Note any warnings — these should have been resolved during execution.

4. Run the example file:
   ```
   cargo run examples/honda_accord.json
   ```
   Note any errors.

---

## Step 3 — Review against the spec

For each item in the **Requirements** section of the spec, make a judgment:

- **Satisfied** — the implementation clearly meets it
- **Partial** — the implementation addresses it but incompletely
- **Missing** — no implementation found for this requirement

Also check the **Constraints** and **Out of Scope** sections:
- Were all constraints respected?
- Was anything built that was explicitly out of scope?

---

## Step 4 — Review code quality

Evaluate the implementation on:

- **Correctness** — does the logic match the domain model in `CLAUDE.md`?
- **Test quality** — do the tests verify behavior meaningfully, or are they
  tautological? Do they cover edge cases relevant to the spec? Use Coverage analysis tools to help determine.
- **Idiomatic Rust** — appropriate use of types, error handling, ownership, and
  standard library. No unnecessary clones, unwraps, or type gymnastics.
- **Design** — are types and module boundaries sensible? Would a future change
  to a neighboring area require touching this code unnecessarily?

---

## Step 5 — Write the review report

Write the report to `issues/$ARGUMENTS/review.md`:

```
# Review: <title from spec>

## Status
PASS / FAIL / PASS WITH NOTES

## Checklist Completion
All steps complete: yes / no
(list any incomplete steps)

## Test Results
All tests passing: yes / no
(list any failures)

## Requirements Coverage

| Requirement | Status | Notes |
|-------------|--------|-------|
| ...         | Satisfied / Partial / Missing | ... |

## Constraints and Scope
(note any constraint violations or out-of-scope work, or "None")

## Code Quality Notes
(specific observations — file and line references where useful)

## Issues to Address
(numbered list of actionable items, empty if none)
```

---

## Step 6 — Present and resolve

Tell the user the review is complete and give them the summary status
(PASS / FAIL / PASS WITH NOTES) and the count of issues to address, if any.

If there are issues, ask the user whether to fix them now or log them for a
follow-up. If fixing now, work through the issues list and re-run the review
cycle after each fix. Update the report when all issues are resolved.
