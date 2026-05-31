# Refine

Turn a roadmap item into a spec document by orienting yourself in the codebase, asking the user targeted questions one at a time, and writing a spec that gives the plan skill enough to work from.

The roadmap item to refine is: $ARGUMENTS

---

## Step 1 — Orient

Read these files before asking any questions:

- `ROADMAP.md` — the index of all items and their status. Find where this item sits (it may already be a 🟡 row in the **Planned** table) or note that it's new.
- `docs/index.md` — overview of every shipped capability and where the user-facing reference lives.
- `issues/` — browse the spec/plan of any past or in-flight item adjacent to the new one.
- Skim `crates/` (Rust workspace) and `runtime/` (JS runtime) to understand the shape of the code touched by the item.

Do not ask any questions until you have read all of these.

---

## Step 2 — Ask questions one at a time

Ask the user questions **one at a time**. Wait for each answer before asking the next. Do not present a list of questions. Do present your recommendation.

There is no fixed question set — use your judgment based on what you learned in Step 1 and what the roadmap item needs. Good areas to probe:

- Why this item matters and what problem it solves
- Acceptance criteria / definition of done
- Known edge cases or complexity the user has already thought through
- Technical constraints or dependencies on other items
- What is explicitly out of scope

Stop asking when you judge you have enough to write a spec that the plan skill can use to produce a detailed execution plan. Don't over-ask.

---

## Step 3 — Write the spec

Tell the user you have enough information and are writing the spec.

Derive a `<slug>`: a one-or-two word kebab-case string capturing the essence of the item (e.g. `core`, `test-runner`, `router`).

Create the file at `issues/<slug>/spec.md` with this structure:

```
# Spec: <title>

## Problem Statement
What problem does this solve and why does it matter at this point in the project.

## Background
Relevant context from the codebase and domain that a planner needs to know.

## Requirements
Specific, testable statements of what the implementation must do.

## Constraints
Technical, business, or design boundaries that must be respected.

## Out of Scope
Explicit exclusions — things that might seem related but are not part of this item.

## Open Questions
Anything unresolved that the plan phase should address before execution begins.
```

**Documentation.** If the item changes anything user-facing — CLI commands or
flags, `zero.toml` config, the public API, scaffold output, or observable
behavior a user could rely on — the **Requirements** section must include an
explicit docs requirement naming the affected `docs/*.md` file(s) (the user
reference lives there; see `docs/index.md`). If the change is purely internal
with no user-facing effect, say so, so the plan phase knows docs were considered
rather than forgotten.

After writing the file, register the item in `ROADMAP.md`:

- If it isn't already in the **Planned** table, add a row:
  `| [<slug>](issues/<slug>/spec.md) | 🟡 | <one-line note> |`
- If it's already listed, make sure its link points at the new spec and the note
  still reads true.

Then tell the user the spec path and ask them to review it. Make any changes
they request — in both the spec and, if the title or scope shifted, the roadmap
row.
