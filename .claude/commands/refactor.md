# Refactor

Restructure existing code — split large functions, reorganize modules, improve
naming — without changing behavior. The existing test suite is the safety net:
every individual move must keep all tests green.

What to refactor: $ARGUMENTS

---

## Step 1 — Orient

Read these files before planning anything:

- `CLAUDE.md` — architecture, domain model, conventions
- `ROADMAP.md` — see whether this refactor corresponds to a tracked item
- All file names under `crates/` and `runtime/` — understand the current structure in full

If `$ARGUMENTS` matches an issue slug and `issues/$ARGUMENTS/review.md` exists,
read it — it may contain specific structural findings that motivate this
refactor.

**Roadmap interaction:** a refactor changes structure, not behavior, so it does
not ship or unship a feature.

- If `$ARGUMENTS` matches a *feature* item tracked in `ROADMAP.md`, leave that
  item's status untouched.
- If the refactor is itself a tracked item in the **Planned** table (e.g. an
  internal-quality cleanup), follow the normal lifecycle: set it to ⏳ when you
  start executing (Step 4), and after Step 5's checks pass, move it to the
  matching category table as ✅ with today's date.
- Incidental cleanup that isn't tracked stays off the roadmap — don't add a row
  for it.

---

## Step 2 — Confirm the tests are green before starting

```
cargo test
```

If tests are failing before you touch anything, stop and report it. Do not
refactor a broken codebase — fix the failures first or ask the user how to
proceed.

---

## Step 3 — Plan the moves

Produce a short plan as a checklist of discrete moves. Each move should be
small enough to complete and verify in one step. Write the plan to:

- `issues/$ARGUMENTS/refactor-plan.md` if `$ARGUMENTS` is an existing issue slug
- `issues/refactor-<slug>/refactor-plan.md` for standalone work, where `<slug>`
  is a one-or-two word kebab-case label derived from `$ARGUMENTS`

Format:

```
# Refactor Plan: <description>

## Goal
One or two sentences on what structural problem this fixes and why it matters.

## Moves

- [ ] **Move 1: <title>** — <one line description>
- [ ] **Move 2: <title>** — <one line description>
- [ ] **Move N: <title>** — <one line description>
```

Present the plan to the user and ask for approval before executing. Adjust if
they request changes.

---

## Step 4 — Execute each move

For each move in the checklist:

1. **Announce** the move before starting.
2. **Make only the change described** — do not bundle adjacent cleanup into the
   same move.
3. **Run the tests immediately after:**
   ```
   cargo test
   ```
   Tests must stay green. If any test fails, the move introduced a behavioral
   change — revert or fix it before continuing. Do not proceed with a red suite.
4. **Mark the move complete** in the plan file: `[ ]` → `[x]`.
5. Report the move is done and tests are green.

---

## Step 5 — Final cleanup and verification

After all moves are complete:

```
cargo fmt
cargo clippy
cargo test
```

Fix any clippy warnings before finishing. Report a summary: what was
restructured, that all tests remain green, and the final file/module layout.
