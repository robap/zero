---
title: Agentic coding with zero
nav_order: 3
---

# Agentic coding with zero

zero's tooling is designed around a specific workflow rhythm: cheap checks run
often, expensive checks run before declaring done. Reading the [API
reference](./index.html#reference) tells you *what* the framework provides;
this page tells you *how* to work on top of it so the verification loop is
actually short enough to run.

This page is agent-neutral. The workflow works in any tool — Claude Code,
Cursor, Aider, plain `vim` with a human in the chair. The Claude Code-specific
section is one clearly fenced block; everything else applies regardless.

Read this before your second feature. The first one is a tour; the second is
when the rhythm starts paying off.

## The verification rhythm

Each command in the table below is built to be cheap enough that you'll
actually run it at the point indicated. If you stop running them, the loop
breaks — you lose the signal each one was designed to surface. The columns are
*what* the command does, *when* to reach for it, and *why skipping it costs
you*.

| Command | When | Why skipping costs you |
| --- | --- | --- |
| [`zero lint`](./linting.html) | After every edit touching `.ts` / `.js` / `.scss`. | Sub-second. Catches the rule-table footguns (R01-R03, T01-T04, C01-C02, I01-I02, S01, L01-L13) before they reach tests. Bugs the linter would have caught instead surface as failing tests with longer feedback loops. |
| [`zero test [pattern]`](./testing.html) | After every logic change. | The basic behavior signal. If you change a function and don't run its test, you're guessing. |
| [`zero test --coverage`](./testing.html) | Before declaring done; before deciding whether to run `zero mutate`. | Names the lines no test reaches, so you know what to write a test for. Cheap pre-check before paying mutation-testing cost on lines you already know are untested. |
| [`zero mutate [--threads N] [--operators ID,…]`](./config-and-cli.html#zero-mutate) | Before declaring done on correctness-critical code. | *tests can pass vacuously* — mutation testing forces the question "would this test actually fail if I broke the code?" That's the rationale for the whole tool. Without it, green tests are evidence the test ran, not evidence the code is correct. Repeat runs are cheap: unchanged files replay from the cache, so running it after every change is affordable. |
| `zero fmt` *(forthcoming)* | Before commit. | Idempotent format. Cuts review noise so reviewers can read the change, not the formatting. |
| `zero preview` *(forthcoming)* | Before pushing UI changes. | Smoke-tests the production bundle. `zero dev` runs the dev server; `zero preview` catches asset-pipeline regressions a dev run won't. |

*This rhythm is most useful when you also pair it with a deliberate refine →
plan → implement workflow — see below.*

## Refine → Plan → Implement

Three phases, each ending with a file on disk. The disk hand-off is what makes
the workflow agent-agnostic: the spec doesn't care which tool wrote it, and
neither does the plan. zero's own development uses this layout — every
in-flight change in this repo lives under `issues/<slug>/spec.md` and
`issues/<slug>/plan.md`, which you can browse for worked examples.

### Refine

Turn a roadmap item or rough idea into `issues/<slug>/spec.md`. The output
names *what* and *why*, not *how*: problem statement, requirements,
constraints, out-of-scope, open questions. Phase ends when a planner can act
on the file without asking the user new questions.

### Plan

Read `spec.md`, write `issues/<slug>/plan.md` — files to touch, order of
operations, tests to add, risks, assumptions. Phase ends when a coder can
execute the plan mechanically without making product calls.

### Implement

Read `plan.md`, write code, run the verification rhythm from the previous
section after each step, tick each checkbox in `plan.md` as the step lands.
Phase ends when every box is `[x]` and the rhythm comes back green.

> **If you use Claude Code**, you can encode each phase as a
> [skill file](https://docs.claude.com/en/docs/claude-code/skills).
> Snippets are below.

## Skill snippets (Claude Code)

Each snippet below is a complete Claude Code skill. Paste verbatim into
`.claude/skills/<name>/SKILL.md` and the
[skill loader](https://docs.claude.com/en/docs/claude-code/skills) picks it up
on the next invocation. The three together encode the Refine → Plan →
Implement workflow described above.

### refine

`/refine <rough idea>` produces `issues/<slug>/spec.md` — a precise enough
description that the Plan skill can act on it without coming back to the user.

````markdown
---
name: refine
description: This skill should be used when the user wants to refine a feature idea into a written spec — typically invoked as `/refine <rough idea>` or `/refine <feature-slug>` to resume. Runs in two phases — exploration (pressure-test, surface alternatives, ask clarifying questions) then specification (write a precise spec to `issues/<slug>/spec.md`). First step in the Refine → Plan → Implement workflow.
---

# Refine

Take a rough feature idea and turn it into a written spec the Plan skill can act on.

## Inputs

- A rough description: `/refine add a barcode scanner`
- A slug to resume: `/refine barcode-scanner`
- Nothing — ask the user what they want to refine

Pick a short kebab-case slug. Confirm before writing files.

## Project context

Read `README.md` and `AGENTS.md` for project context. If resuming, read `issues/<slug>/spec.md` too.

## Phase 1 — Explore

Stress-test the idea and reach alignment on shape and scope before writing anything to disk. Ask 2–5 batched clarifying questions covering:

- **Who and why** — what user problem, who hits it?
- **Scope** — what's explicitly out of scope this iteration?
- **Surface area** — which pages/routes; new or extending?
- **Data shape** — what state does it read or write?
- **Tradeoffs** — surface 2–3 approaches and recommend one with reasoning.
- **Edge cases** — empty, error, concurrent, offline.

Every question must include a recommended answer. Don't ask open-ended "what do you think?" — make a concrete proposal the user can confirm or redirect from.

## Phase 2 — Spec

Once the user has answered enough, write `issues/<slug>/spec.md` with these sections:

```
# <Feature Title>
## Problem
## Goal
## Out of scope
## User stories
## Acceptance criteria
## UX notes
## Data
## Approach (high level)
## Open questions
```

## Output

`issues/<slug>/spec.md`.

Next, run `/plan <slug>`.
````

### plan

`/plan <slug>` reads the spec and produces `issues/<slug>/plan.md` — a flat,
checkboxed implementation plan a coder can execute step by step.

````markdown
---
name: plan
description: This skill should be used when the user wants to turn a refined feature spec into an executable, checkboxed implementation plan — typically invoked as `/plan <feature-slug>`. Reads `issues/<slug>/spec.md` and writes `issues/<slug>/plan.md`. Second step in the Refine → Plan → Implement workflow.
---

# Plan

Turn a refined spec into a small, ordered, checkboxed implementation plan that the Implement skill can execute one step per turn.

## Inputs

`/plan <slug>`. If no slug, list available `issues/*/spec.md` and ask. If the spec doesn't exist, tell the user to run `/refine` first.

## Project context

- `issues/<slug>/spec.md` — source of truth
- `AGENTS.md` — framework API reference; read sections relevant to the surface area the spec touches
- Existing code under `src/` to understand what's already there
- Existing tests under `src/**/*.test.{ts,js}` to mirror the test style

## How to write the plan

Each step must be:

- **Concrete** — names the files and the change in one line
- **Small** — completable in a single Implement turn (one file, or one tight cross-file change)
- **Ordered** — the app stays runnable between steps where possible
- **Verifiable** — has a way to check it worked (a test, a type check, a visual check)

Group steps under H2 section headers (e.g. `## Data`, `## Route`, `## UI`, `## Tests`).

## Output

`issues/<slug>/plan.md` containing:

```
# <Feature Title> — Plan
## Summary
## Assumptions
## Steps
  ### <group> — checkboxed steps
## Verification
## Open questions
```

After writing, summarize the plan in 2–3 sentences (step count, assumptions, open questions).

Next, run `/implement <slug>`.
````

### implement

`/implement <slug>` reads the plan and executes one unchecked step per
invocation: identify, state, execute (TDD if behavioral), verify, log
friction, tick the box, report.

````markdown
---
name: implement
description: This skill should be used when the user wants to execute a planned feature step by step — typically invoked as `/implement <feature-slug>`. Reads `issues/<slug>/plan.md`, implements the next unchecked step, ticks the box, and pauses for review. Re-invoke to continue. Third step in the Refine → Plan → Implement workflow.
---

# Implement

Execute one step of a feature plan, then stop and report. The user re-invokes to advance.

## Inputs

`/implement <slug>`. If no slug, list `issues/*/plan.md` and ask.

## Context to load each turn

State lives on disk, not in memory — re-read each invocation:

1. `issues/<slug>/plan.md` — find the first unchecked `- [ ]` step
2. `issues/<slug>/spec.md` — for acceptance criteria
3. `AGENTS.md` — framework API reference; skim relevant sections
4. Any source files the current step names

## Loop

1. **Identify** the first `- [ ]` step in `plan.md`.
2. **State** in one sentence what the step does, which files it touches, and whether it's behavioral (must follow the TDD cycle below) or non-behavioral (config, styling, docs — no test required).
3. **Execute.** Behavioral steps follow the TDD cycle. Non-behavioral steps make the change directly.
4. **Verify with `zero lint`** on every `.ts` / `.js` / `.scss` / `.css` edit, plus the step's stated check. See the [verification rhythm](https://robap.github.io/zero/agentic-coding.html#the-verification-rhythm).
5. **Tick the box** in `plan.md` (`- [ ]` → `- [x]`). Do not edit other steps.
6. **Report and stop.** 2–4 sentences: what changed, how verified, the next step's title. Do not advance.

## TDD cycle (behavioral steps)

Write a failing test first, add a stub that compiles but still fails on behavior, then make it pass. Each phase produces an observable signal — don't move on without seeing it.

## End of plan

When every box is `[x]`, run `zero test --coverage` to find under-tested lines, then `zero mutate --threads 8 --quiet` on correctness-critical code to catch tests that pass vacuously. Repeat `zero mutate` runs are cheap — unchanged files replay from the cache — so there's no cost argument for skipping it between steps.

## Output

Updated `issues/<slug>/plan.md` with one more box checked, plus the diff for that step.
````

