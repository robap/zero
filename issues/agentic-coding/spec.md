# Spec: `docs/agentic-coding.md` — onboarding page for the agent workflow zero assumes

## Problem Statement

zero's docs cover the API surface (`AGENTS.md` in-tree, `docs/components.md`, `docs/routing.md`, etc.) and the user guide explains *what* each piece does. Nothing documents the **workflow rhythm** that the framework's tooling is designed to support: `zero lint` after every code-touching step, `zero test --coverage` to find under-tested lines, `zero mutate --threads N` before declaring done, and a friction-log pattern to surface framework gaps as they're hit.

Without this, every adopter rediscovers the rhythm by accumulating bugs. The demo's `FRAMEWORK_NOTES.md` is the existence proof: every entry in it was caught in a single session of one agent doing exactly that — running tools when needed, recording surprises in a consistent format, feeding the result back into the framework. That feedback loop is reproducible only if it's documented.

Caught in the demo's friction log (`~/Documents/code/zero_demo/FRAMEWORK_NOTES.md:56`, severity 🟡). The friction-log entry names three concrete deliverables: (a) per-step verification commands and the *why* behind each, (b) copy-paste skill snippets, (c) a `FRAMEWORK_NOTES.md` template with the severity-emoji + one-line format.

## Background

### What the docs look like today

`docs/index.md` has three sections:

- **Start here** — Getting Started, Reactivity. The 5-minute on-ramp + the one new concept.
- **Reference** — Templates, Components, Routing, HTTP, Testing, Theming, Building and Deploying, Config and CLI, Linting, API. The exhaustive surface.
- **After your first app** — Best Practices, Examples Tour, Why zero.

There is no chapter that says: "Here is the rhythm you should run each command in. Here is why each one matters." Agents reading `AGENTS.md` learn what commands exist; they don't learn when to invoke them in what order.

`AGENTS.md` itself will soon gain a terse "When to run what" block per `issues/agents-quickstart/`. That block is 4-5 bullets — the right size for an in-tree reference. The deeper rationale (why mutation testing complements coverage, why lint runs every step, what `FRAMEWORK_NOTES.md` is for) needs more room than AGENTS.md is willing to give. The onboarding page is the place.

### The rhythm the framework already assumes

Tools that exist today, in the order an agentic loop reaches for them:

1. **`zero lint`** — sub-second. Catches the rule-table footguns (R01-R03, T01-T04, C01-C02, I01-I02, S01, L01-L13). Designed to be cheap enough to run after every edit.
2. **`zero test [pattern]`** — fast. Tests live next to source as `*.test.{ts,js}`. The runner has its own in-memory DOM and matchers.
3. **`zero test --coverage`** — names the lines no test reaches. Cheap pre-check before paying mutation cost.
4. **`zero mutate [--operators ID,…] [--threads N]`** — slower; runs the mutation-testing campaign across `src/`. The framework's correctness signal beyond "tests pass." `issues/mutate-operators/` and `issues/mutate-equivalence/` made it precise enough to trust.
5. **`zero fmt`** — idempotent format.
6. **`zero build`** — production output, used for smoke tests via `zero preview` (per `issues/preview/`).

The reason these were built this way — fast lint, cheap coverage, precise mutation, terse mutate output — is to make a "verify-as-you-go" loop *cheap enough that an agent will actually run them*. If running tests is slow, agents skip them. If mutation testing has noisy output, agents stop reading it. zero's tooling decisions encode the rhythm; documenting the rhythm makes the design intent visible.

### What `FRAMEWORK_NOTES.md` does

The demo's `~/Documents/code/zero_demo/FRAMEWORK_NOTES.md` is a friction log: an append-only list of zero-framework bugs, gaps, and footguns, one line each, prefixed with severity (🔴 broken/misleading, 🟡 missing, 🟢 papercut), area (templates / zero/test / zero/components / etc.), and a `[ ]` / `[x]` checkbox. Fixed entries get annotated in place with the commit SHA and a one-sentence note on what changed; nothing is deleted. The file's value compounds because the format is consistent — every entry reads the same way, so it's grep-able, scannable, and shippable as a punch list to the framework maintainers.

Without a template, every adopter invents a different format, half the entries lack severity, half lack area, and the file devolves into a notes dump that nobody can aggregate.

### The skill snippets

The demo's `.claude/skills/{plan,refine,implement}/SKILL.md` files are Claude Code skill definitions: YAML frontmatter (`name`, `description`) plus a Markdown body describing the workflow phase. The `/refine` skill is the entry point ("turn a feature idea into a written spec"); `/plan` reads the spec and produces an execution plan; `/implement` reads the plan and writes code. The triad encodes a deliberate handoff so that no one phase tries to do too much.

These are Claude Code-specific in *form* (the `---` frontmatter, the `description` triggering rules) but the *workflow* (refine → plan → implement, with disk hand-offs at each boundary) is agent-agnostic. The onboarding page should describe the workflow in agent-neutral terms and provide the Claude Code shape as a concrete example.

### Adjacent surfaces

- **New: `docs/agentic-coding.md`** — the page itself.
- **`docs/index.md`** — add a link to the new chapter in "Start here" (alongside Getting Started and Reactivity).
- **`docs/_config.yml`** — Jekyll config; the new file's `nav_order` is set in its own frontmatter, but `_config.yml` may need an entry if the site lists pages explicitly. Verify during planning.
- **`crates/zero-scaffold/src/scaffold/AGENTS.md`** — the "When to run what" subsection (per `issues/agents-quickstart/`) cross-references this page. If both specs land, the AGENTS.md link target is the published URL of this page. If this lands first, AGENTS.md gets the link directly.
- **`docs/getting-started.md`** — may want a one-line nod ("Once you've shipped your first edit, read [Agentic coding with zero](./agentic-coding.html) for the verification rhythm we recommend."). Not required; planner's choice.
- **Scaffold output** — no change. Per the decision in refine, the scaffold doesn't ship `.claude/skills/` or `FRAMEWORK_NOTES.md`; the page is the canonical source for users to copy.

## Requirements

### R1 — `docs/agentic-coding.md` exists with consistent frontmatter and nav placement

Create `docs/agentic-coding.md` with Jekyll frontmatter matching the rest of the docs:

```markdown
---
title: Agentic coding with zero
nav_order: 3
---
```

`nav_order: 3` slots it between Getting Started (2) and Reactivity (4) in the Start here section — verify other pages' `nav_order` values and shift if collisions arise. The intent: a new reader sees Getting Started → Agentic coding → Reactivity in that order.

`docs/index.md` gains a bullet under "Start here":

```
- **[Agentic coding](./agentic-coding.html)** — the verification rhythm
  zero's tooling is designed around. Read this before your second feature.
```

### R2 — The verification rhythm section

A section titled `## The verification rhythm` (or equivalent) covers the per-step commands and the WHY for each. Structure as a table or stepped list — not free prose — so an agent skimming the page can pattern-match.

Each command gets:
- **What it does** in one sentence.
- **When to run it** — the trigger.
- **Why it matters** — the cost of skipping it. This is the load-bearing content; it's what AGENTS.md can't fit.

Cover at minimum:
- `zero lint` — every edit. Cheap; catches the rule-table footguns before they reach tests.
- `zero test [pattern]` — every logic change. Behavior verification.
- `zero test --coverage` — before declaring done, OR before deciding whether to run mutate. Names the under-tested lines so you know what to write a test for.
- `zero mutate [--threads N] [--operators ID,…]` — before declaring done on correctness-critical code. *Tests can pass vacuously.* Mutation testing forces the question "would this test actually fail if I broke the code?" — the rationale for the whole tool.
- `zero fmt` — before commit.
- `zero preview` — before pushing UI changes; smoke-test the production bundle.

The "*tests can pass vacuously*" framing must appear verbatim somewhere in the mutate bullet. It's the one-sentence pitch that turns mutation testing from "academic curiosity" into "the thing that catches the bugs your tests don't."

End the section with a one-line transition into the workflow shape: "*This rhythm is most useful when you also pair it with a deliberate refine → plan → implement workflow — see below.*"

### R3 — The workflow shape section (`## Refine → Plan → Implement`)

A section describing the three-phase agentic workflow the framework's own `issues/` directory uses:

- **Refine** — turn a roadmap item or rough idea into a written spec (`spec.md`). Phase ends with a file on disk that a planner can act on without asking the user new questions.
- **Plan** — read the spec, produce an execution plan (`plan.md`) — files to touch, order of operations, tests to add. Phase ends with a plan a coder can execute mechanically.
- **Implement** — read the plan, write the code, run the verification rhythm, mark items done.

Frame it as agent-neutral: the workflow works in any tool; the deliverable at each phase is a file on disk; the disk hand-off is what makes it agent-agnostic. Then:

> **If you use Claude Code**, you can encode each phase as a [skill file](https://docs.claude.com/en/docs/claude-code/skills). Examples are below.

### R4 — Copy-paste Claude Code skill snippets

A section titled `## Skill snippets (Claude Code)` containing three complete, ready-to-copy skill files. The shape is the YAML-frontmatter Markdown that Claude Code's skill loader reads:

```markdown
---
name: refine
description: This skill should be used when …
---

# Refine

…workflow body…
```

The bodies should be condensed versions of the demo's `.claude/skills/{plan,refine,implement}/SKILL.md` (which the planner reads as source material), adapted so they reference *zero project layout* — i.e. `issues/<slug>/spec.md` and `issues/<slug>/plan.md` per the framework's own convention, not the demo's `features/<slug>/`. The path convention matters: the framework's own `issues/` directory uses `spec.md` + `plan.md`; the published skills should match so users following the docs end up with a directory layout that mirrors zero's own.

Each snippet should be:
- Self-contained — works pasted into `.claude/skills/<name>/SKILL.md` with no further edits.
- Short — under 60 lines each. Long bodies discourage adoption.
- Cross-referencing — `refine`'s "next step" is `/plan <slug>`; `plan`'s is `/implement <slug>`.
- Aligned with R2's rhythm — `implement`'s workflow should include the `zero lint` / `zero test --coverage` / `zero mutate` cadence.

If page length becomes an issue, the planner may move full snippets to `docs/agentic-coding-skills.md` and link from the main page. Recommended: keep them inline (one page is easier to skim than two).

### R5 — `FRAMEWORK_NOTES.md` template section

A section titled `## A friction log for the framework itself` describing the `FRAMEWORK_NOTES.md` pattern and providing a copy-paste template.

Content must include:

- **What it is.** An append-only log of framework bugs, gaps, and footguns surfaced during real work on top of zero. The point: every adopter is also a tester of the framework's surface; without a log, that signal is lost.
- **The format.** One line per item, with a checkbox, ISO date, severity emoji, short name, one-sentence description, and area:

  ```
  - [ ] `YYYY-MM-DD` 🔴/🟡/🟢 **short name** — what happens; the workaround if any. Area: <templates | zero/test | zero/components | …>
  ```

- **The severity legend.** 🔴 broken or misleading (silent wrong behavior, confusing error, footgun likely to bite repeatedly); 🟡 missing (something you reach for, can work around, but ergonomically poor); 🟢 papercut (minor annoyance with an obvious workaround).
- **How to mark fixed.** Flip `[ ]` to `[x]` and append a fix annotation on the same line: `**FIXED YYYY-MM-DD** (#PR / SHA): one-sentence note`. Don't delete.
- **Where to file the actual issue.** A friction-log entry is not a substitute for filing a real issue against the framework — link the entry to the issue.
- **The full template** as a fenced code block the reader can copy verbatim:

  ```markdown
  # <Project> framework — friction log

  Append-only log of bugs, gaps, and footguns discovered while building
  this app on top of `zero`. Keep ground-truth feedback in one place so
  framework work has a real-world bug list to pull from.

  ## How to add an entry
  One line per item, prefixed with an open checkbox …

  ## How to mark an entry fixed
  Flip `- [ ]` to `- [x]` …

  ## Entries
  - [ ] `YYYY-MM-DD` 🟢 **first entry** — describe what happened. Area: …
  ```

The template content should be lifted directly from the demo's `FRAMEWORK_NOTES.md` header (lines 1-30) since it's already battle-tested and proven to produce consistent entries. Planner reads the demo as source material.

### R6 — Cross-references both ways

- **From AGENTS.md** (after `issues/agents-quickstart/` lands): the "When to run what" subsection ends with a one-line link to this page for the deeper why. If `agents-quickstart` lands first, that link target is `agentic-coding.html#the-verification-rhythm`.
- **From `docs/getting-started.md`** (optional, planner's call): a one-line nod at the end pointing to this page as the recommended next read for agentic workflows.
- **From this page** to:
  - `linting.html` — the full rule reference behind `zero lint`.
  - `testing.html` — the full test-runner reference.
  - `config-and-cli.html` — the full flag reference (mutate operators, coverage output path, etc.).
  - `https://docs.claude.com/en/docs/claude-code/skills` — Claude Code's skill documentation.

Links inside the docs site use relative `.html` paths matching the rest of the site (e.g. `[Linting](./linting.html)`).

### R7 — Tests / acceptance

- `docs/agentic-coding.md` exists and renders cleanly under Jekyll's GitHub Pages build. If the docs site has a build check (smoke test, link checker, etc.), the new page must pass it.
- `docs/index.md` lists the new page under "Start here."
- All inline links resolve. The planner uses a link checker (`htmlproofer` or grep + curl) and surfaces any 404s before merging.
- The page renders correctly on the GitHub Pages preview before merging. (Planner runs a local Jekyll preview or pushes to a preview branch.)

No automated test asserts the *content* of the page — that's a doc, not a contract. The above are sanity checks.

### R8 — Length budget

Target ~400-600 lines of Markdown. If the page grows past 800 lines, split the skill snippets into `docs/agentic-coding-skills.md` and link. The page must be readable in one sitting; if it isn't, it'll go unread.

## Constraints

- No new code in `crates/` or `runtime/` — pure docs.
- No new scaffolded files. Per the refine decision, the scaffold doesn't ship `.claude/skills/` or `FRAMEWORK_NOTES.md`. Reader copies from this page.
- The page must read coherently in agent-neutral terms; the Claude Code-specific section is one block, clearly fenced.
- Do not introduce framework primitives or APIs in this page. It documents *how to work*, not *what the framework provides* (that's the rest of the docs).
- Voice and style match the existing docs: second-person "you," short paragraphs, generous code blocks, no marketing tone.
- Every command mentioned must exist in the CLI today. If the spec mentions `zero preview`, the planner confirms `issues/preview/` has landed; if not, either the link or the command line gets a `(forthcoming)` marker rather than a broken reference.
- The page's published URL (`https://robap.github.io/zero/agentic-coding.html`) is the cross-reference target from AGENTS.md per R6 — the slug `agentic-coding` is therefore part of this spec's contract.

## Out of Scope

- **Scaffolding skill files into new projects.** Decided against in refine.
- **Scaffolding `FRAMEWORK_NOTES.md` into new projects.** Same.
- **A separate "developer workflow" chapter for non-agentic users.** This page is about the agentic loop; users running a non-agentic workflow can still read the verification-rhythm section, but the page is not pitched at them.
- **A new top-level "Workflow" section in `docs/index.md`.** The chapter slots into "Start here" — adding a new top-level section reshuffles the IA more than this work warrants.
- **Skill snippets for tools other than Claude Code** (Cursor, Aider, Continue, Cline, etc.). Each has its own configuration shape; the workflow is agent-neutral and translates. Adding per-tool snippets would balloon the page beyond R8's budget.
- **Authoring tooling for `FRAMEWORK_NOTES.md`** (a `zero fricton-log add` CLI, a VS Code snippet, etc.). The template is the deliverable; tooling is a follow-up only if friction is observed.
- **An issue tracker integration** (auto-creating GitHub issues from friction-log entries). The page tells readers to file separately; how is out of scope.
- **A worked example of using the workflow on a specific feature.** Tempting; bloats the page. The framework's own `issues/<slug>/spec.md` directories serve as worked examples already and are linkable.

## Open Questions

- **`nav_order` value.** Spec recommends `3` (between Getting Started=2 and Reactivity=4). Planner verifies other pages' values during implementation and renumbers if needed.
- **Inline vs split skill snippets.** Spec recommends inline; if the page approaches 800 lines, split per R8.
- **Whether to include a "what a friction-log entry looks like" example list.** A handful of real entries from the demo's `FRAMEWORK_NOTES.md` (anonymized if necessary) make the format concrete. Recommended: yes, include 3-5 real-looking examples spanning each severity. Planner decides exact entries.
- **AGENTS.md cross-reference handling if `issues/agents-quickstart/` lands first.** That spec's R2 has a TODO for the link target; once this page exists, the planner files a small follow-up to update AGENTS.md. Not a blocker for either spec.
- **Whether to demonstrate the workflow on a sample task.** Tempting. Out of scope per the "Out of Scope" list above; reconsider only if reader feedback says the page is too abstract.
- **Skill file Markdown body length.** The demo's `refine/SKILL.md` is ~100 lines; spec says "under 60." Planner judges what's compressible without losing the workflow's load-bearing detail. The demo files are source material — published versions can compress.
