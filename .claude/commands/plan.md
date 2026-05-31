# Plan

Turn a spec document into a detailed, step-by-step execution plan that the execute skill can follow without further clarification.

The issue slug to plan is: $ARGUMENTS

---

## Step 1 — Orient

Read these files before producing anything:

- `issues/$ARGUMENTS/spec.md` — the spec you are planning against
- `ROADMAP.md` — confirm the item is tracked, and see its current status and neighboring work
- `CLAUDE.md` — architecture, domain model, conventions
- All file names under `crates/` and `runtime/` — understand what exists and what must be extended or created

Do not write the plan until you have read all of these.

Planning does not change the item's roadmap status — it stays 🟡 in the
**Planned** table until `execute` begins.

---

## Step 2 — Produce the plan

Write the plan to `issues/$ARGUMENTS/plan.md` with the following structure:

```
# Plan: <title from spec>

## Summary
One short paragraph: what will be built, what approach is taken, and why.

## Prerequisites
Any spec open questions that must be resolved before execution, or other issues
that must be completed first. If none, write "None."

## Steps

- [ ] **Step 1: <title>**
- [ ] **Step 2: <title>**
- [ ] **Step N: <title>**

(one line per step — the execute skill will check these off as each step completes)

---

## Step Details

### Step 1: <title>
**Goal:** What this step achieves and why it comes before the next step.
**Files:** List every file to be created or modified.
**Changes:** Describe the specific changes — new types, functions, modules, or
logic. Be precise enough that a developer could execute this step without
re-reading the spec.
**Tests:** What tests cover this step and what they verify.

(repeat for each step)

## Risks and Assumptions
Things that could go wrong or assumptions baked into this plan that, if wrong,
would require replanning.
```

### Rules for writing steps

- Each step should be independently completable and leave the codebase in a
  compilable, test-passing state.
- Order steps so each one builds directly on the last — no orphaned work.
- Be specific about types, function signatures, and module layout where it
  matters. Don't leave "figure it out" gaps for the executor.
- Separate concerns: don't bundle data model changes with business logic changes
  in the same step unless they are inseparable.
- Tests are not an afterthought — include them in the step they belong to.

---

## Step 3 — Present and refine

After writing the file, tell the user the path and give a brief summary of the
steps. Ask if they want any changes. Revise until they approve.
