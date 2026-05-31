# Execute

Implement a plan step by step using strict TDD. Every behavior must be driven
by a failing test before implementation is written.

The issue slug to execute is: $ARGUMENTS

---

## Step 1 — Orient

Read these files before writing any code:

- `issues/$ARGUMENTS/plan.md` — the plan you are executing
- `issues/$ARGUMENTS/spec.md` — the requirements you are satisfying
- `ROADMAP.md` — confirm the item is tracked and see its current status
- `CLAUDE.md` — architecture, domain conventions, and commands
- All file names under `crates/` and `runtime/` — understand current state before touching anything

Do not write any code until you have read all of these.

---

## Step 2 — Check prerequisites

Review the Prerequisites section of the plan. If any unresolved open questions
or blocking dependencies are listed, stop and report them to the user before
proceeding. Do not work around prerequisites — surface them.

Once prerequisites are clear and you're starting work, set this item's status to
⏳ in the **Planned** table of `ROADMAP.md`.

---

## Step 3 — Execute each step using TDD

Announce each step number and title before starting it.

Within each step, implement behavior one assertion at a time using this cycle —
do not break the cycle or skip ahead:

### The TDD cycle (repeat for every assertion)

1. **Write one test assertion.** Just one. Not a block of tests for the whole
   step — a single assertion for the next specific behavior.

2. **Write stub code so it compiles.** Add the minimum — empty functions,
   placeholder types, `todo!()` bodies — so `cargo build` succeeds. Do not
   implement behavior yet.

3. **Run the test and confirm it fails for a behavior reason:**
   ```
   cargo test
   ```
   The test must fail because the behavior is not yet implemented, not because
   of a compile error or a wrong assertion. If it fails for the wrong reason,
   fix the test or stub before continuing.

4. **Implement the behavior** for this assertion and nothing more.

5. **Run the test and confirm it passes:**
   ```
   cargo test
   ```
   If it does not pass, fix the implementation until it does. Do not move on
   with a failing test.

6. **Move to the next assertion** and repeat from step 1.

### After completing all assertions in a step

Run in this order and do not proceed until all three are clean:

```
cargo fmt
cargo clippy
cargo test
```

Fix every issue reported by `clippy` before moving on — warnings are not
acceptable.

Then mark the step complete in `issues/$ARGUMENTS/plan.md` by changing its
checkbox from `[ ]` to `[x]`:

```
- [x] **Step N: <title>**
```

Report step completion to the user: what behavior was implemented, that
fmt/clippy/test all passed.

---

## Step 4 — Handle ambiguity

If you encounter something not covered by the plan — an ambiguity, a conflict
with existing code, or a requirement that turns out harder than the plan assumed
— stop and explain the situation to the user. Do not improvise a solution that
may contradict the spec.

---

## Step 5 — Wrap up

After all steps are complete, run one final check:

```
cargo fmt
cargo clippy
cargo test --workspace -- --include-ignored
```

Report a brief summary: steps completed, files created or modified, and anything
that deviated from the plan with an explanation.

Finally, if all steps completed and the final checks are green, mark the item
shipped in `ROADMAP.md`: remove its row from the **Planned** table and add it to
the best-fit category table as
`| [<slug>](issues/<slug>/spec.md) | ✅ | <today's date> |` (create a new
category heading only if none fits). Report that the item is now ✅ shipped. If
the run did not finish cleanly, leave it ⏳ and say why.
