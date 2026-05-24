# Plan: `docs/agentic-coding.md` — onboarding page for the agent workflow zero assumes

## Summary

Add a single Jekyll page at `docs/agentic-coding.md` (`nav_order: 3`, slotted
between Getting Started and Reactivity) that documents the verification rhythm
zero's tooling is designed around, the agent-neutral Refine → Plan → Implement
workflow, three copy-paste Claude Code skill snippets, and the
`FRAMEWORK_NOTES.md` friction-log template. To make room for `nav_order: 3`
without collisions, all docs pages currently at `nav_order: 3..16` shift up by
one. Add a link to the new page from `docs/index.md` "Start here" and a
one-line nod from `docs/getting-started.md`. No code changes; pure docs.

## Prerequisites

- **`zero preview` and `zero fmt` are not in the CLI binary today.** `cargo run
  -p zero -- --help` lists only: `init`, `dev`, `build`, `test`, `mutate`,
  `update`, `lint`. Per the spec constraint ("Every command mentioned must
  exist in the CLI today … else `(forthcoming)`"), both commands appear in the
  rhythm with a `(forthcoming)` marker and no link target until they ship.
  `issues/preview/spec.md` is on disk but not implemented; there is no
  `issues/fmt/` at all. This is not a blocker — the marker is the documented
  escape hatch.
- **`issues/agents-quickstart/` has not landed.** Per spec R6 and Open
  Questions, the AGENTS.md cross-reference (TO this page) is handled on the
  *agents-quickstart* side once both land; no edit to the scaffold's
  `AGENTS.md` is required by this plan. The reverse link (from this page to
  AGENTS.md) is fine as-is because AGENTS.md already exists.
- All other adjacent docs (`linting.md`, `testing.md`, `config-and-cli.md`)
  already exist and are link targets — no preliminary work needed.

## Steps

- [x] **Step 1: Shift `nav_order` on existing docs to free slot 3**
- [x] **Step 2: Create `docs/agentic-coding.md` skeleton (frontmatter + intro + section headings)**
- [x] **Step 3: Write the "Verification rhythm" section**
- [x] **Step 4: Write the "Refine → Plan → Implement" workflow section**
- [x] **Step 5: Write the "Skill snippets (Claude Code)" section**
- [x] **Step 6: Write the "A friction log for the framework itself" section**
- [x] **Step 7: Wire cross-references (index.md "Start here" bullet, getting-started nod, internal page links)**
- [x] **Step 8: Verify — link sweep, length budget, Jekyll smoke build**

---

## Step Details

### Step 1: Shift `nav_order` on existing docs to free slot 3

**Goal:** Make `nav_order: 3` available for the new page so the Just-the-Docs
sidebar renders `Getting Started (2) → Agentic coding (3) → Reactivity (4) → …`
in the intended order. Has to come first or the new page collides with
`reactivity.md`.

**Files (all under `docs/`):**

| File                         | Current `nav_order` | New `nav_order` |
| ---------------------------- | ------------------- | --------------- |
| `reactivity.md`              | 3                   | 4               |
| `templates.md`               | 4                   | 5               |
| `components.md`              | 5                   | 6               |
| `routing.md`                 | 6                   | 7               |
| `http.md`                    | 7                   | 8               |
| `testing.md`                 | 8                   | 9               |
| `theming.md`                 | 9                   | 10              |
| `building-and-deploying.md`  | 10                  | 11              |
| `config-and-cli.md`          | 11                  | 12              |
| `linting.md`                 | 12                  | 13              |
| `api.md`                     | 13                  | 14              |
| `best-practices.md`          | 14                  | 15              |
| `examples-tour.md`           | 15                  | 16              |
| `why-zero.md`                | 16                  | 17              |

(Verified by `head -8` on each file in this directory; `docs/index.md` is
`nav_order: 1`, `docs/getting-started.md` is `nav_order: 2`, both unchanged.)

**Changes:** For each file above, edit the `nav_order: N` line in the YAML
frontmatter to `nav_order: N+1`. Nothing else in the frontmatter or body
changes. Use a per-file `Edit` rather than a bulk find/replace — every shift is
a 1-line change unique to its file.

**Tests:** None — Jekyll renders frontmatter; the smoke build in Step 8
verifies the site still assembles. A quick `grep -n "^nav_order:" docs/*.md`
after the edits confirms the new layout.

### Step 2: Create `docs/agentic-coding.md` skeleton (frontmatter + intro + section headings)

**Goal:** Land the file with frontmatter, an intro paragraph that orients a
reader landing here from "Start here," and the four H2 headings the subsequent
steps will fill. Lets later steps be independently reviewable section-by-section
and keeps the file compilable at all times.

**Files:** `docs/agentic-coding.md` (new).

**Changes:** Write the following exact frontmatter and skeleton (placeholders
are explicit "TODO" notes the later steps will replace; the page is valid
Markdown at every checkpoint):

```markdown
---
title: Agentic coding with zero
nav_order: 3
---

# Agentic coding with zero

zero's tooling is designed around a specific workflow rhythm: cheap checks run
often, expensive checks run before declaring done, and the framework's own
gaps get logged the moment you hit them. Reading the [API
reference](./index.html#reference) tells you *what* the framework provides;
this page tells you *how* to work on top of it so the verification loop is
actually short enough to run.

This page is agent-neutral. The workflow works in any tool — Claude Code,
Cursor, Aider, plain `vim` with a human in the chair. The Claude Code-specific
section is one clearly fenced block; everything else applies regardless.

Read this before your second feature. The first one is a tour; the second is
when the rhythm starts paying off.

## The verification rhythm

TODO — Step 3.

## Refine → Plan → Implement

TODO — Step 4.

## Skill snippets (Claude Code)

TODO — Step 5.

## A friction log for the framework itself

TODO — Step 6.
```

**Tests:** None. The page exists and is valid Markdown.

### Step 3: Write the "Verification rhythm" section

**Goal:** Deliver spec R2 — per-command coverage with WHAT / WHEN / WHY, in a
table the reader can pattern-match against. This is the load-bearing section
of the page (per spec: "the WHY is what AGENTS.md can't fit").

**Files:** `docs/agentic-coding.md`.

**Changes:** Replace the `TODO — Step 3.` placeholder under
`## The verification rhythm` with:

1. **Lead paragraph (2-3 sentences)** explaining that each command in the
   table below is designed to be cheap enough that you'll actually run it; if
   you stop running them, you lose the loop. Frame the table's columns:
   *What*, *When*, *Why skipping costs you*.
2. **Command table** with one row per command, in the order an agentic loop
   reaches for them:
   - `zero lint` — cheap, sub-second; *When:* after every edit touching
     `.ts` / `.js` / `.scss`; *Why:* catches the rule-table footguns (R01-R03,
     T01-T04, C01-C02, I01-I02, S01, L01-L13) before they reach tests. Link
     `zero lint` to `./linting.html`.
   - `zero test [pattern]` — *When:* after every logic change; *Why:* the
     basic behavior signal. Link to `./testing.html`.
   - `zero test --coverage` — *When:* before declaring done, or before
     deciding whether to run `zero mutate`; *Why:* names the lines no test
     reaches so you know what to write a test for; cheap pre-check before
     paying mutation cost. Link to `./testing.html`.
   - `zero mutate [--threads N] [--operators ID,…]` — *When:* before
     declaring done on correctness-critical code; *Why:* **tests can pass
     vacuously.** Mutation testing forces the question "would this test
     actually fail if I broke the code?" — the rationale for the whole tool.
     Link to `./config-and-cli.html#zero-mutate`.
   - `zero fmt` *(forthcoming)* — *When:* before commit; *Why:* idempotent
     format; cuts review noise.
   - `zero preview` *(forthcoming)* — *When:* before pushing UI changes;
     *Why:* smoke-test the production bundle. Builds work locally; the
     production bundle catches asset-pipeline regressions a `zero dev` run
     can't.
3. **The verbatim sentence** — the bullet/cell for `zero mutate` must contain
   the literal string *"tests can pass vacuously"* (italicized as in the
   spec). This is the one-sentence pitch that turns mutation testing from
   "academic curiosity" into "the thing that catches the bugs your tests
   don't." Quoted verbatim per R2.
4. **Closing transition** — after the table, a single italicized line:

   > *This rhythm is most useful when you also pair it with a deliberate
   > refine → plan → implement workflow — see below.*

**Implementation notes:**
- Use a Markdown table (`| Command | When | Why |`) for the four shipped
  commands plus the two forthcoming. The `(forthcoming)` marker goes in the
  *Command* cell so the reader sees immediately that the row is aspirational.
- Don't repeat content already in `./linting.html` or `./testing.html`; link
  to them once per command.
- Voice: second-person "you," short sentences, no marketing.

**Tests:** None content-wise. A `grep -n "tests can pass vacuously"
docs/agentic-coding.md` after the edit confirms the verbatim sentence is
present (R2 requirement). Step 8 covers link validity.

### Step 4: Write the "Refine → Plan → Implement" workflow section

**Goal:** Deliver spec R3 — describe the three-phase agentic workflow in
agent-neutral terms, with a one-line bridge to the Claude Code section
beneath.

**Files:** `docs/agentic-coding.md`.

**Changes:** Replace `TODO — Step 4.` with:

1. **Lead paragraph (2-3 sentences)** framing the shape: three phases, each
   ending with a file on disk, where the disk hand-off is what makes the
   workflow agent-agnostic. Cite the framework's own `issues/<slug>/spec.md` /
   `plan.md` layout as the lived example — point at this very repo's
   `issues/` directory as a worked example reader can browse on GitHub.
2. **Three subsections** (use H3 `### Refine`, `### Plan`, `### Implement`),
   each 3-5 lines:
   - **Refine.** Turn a roadmap item or rough idea into `spec.md`. Phase ends
     when a planner can act on the file without asking the user new
     questions. The output names *what* and *why*, not *how*.
   - **Plan.** Read `spec.md`, write `plan.md` — files to touch, order of
     operations, tests to add. Phase ends when a coder can execute the plan
     mechanically.
   - **Implement.** Read `plan.md`, write code, run the verification rhythm
     from the previous section after each step, tick checkboxes as steps
     complete. Phase ends when every box is `[x]` and the rhythm comes back
     green.
3. **Bridge to skill section** at the end:

   > **If you use Claude Code**, you can encode each phase as a
   > [skill file](https://docs.claude.com/en/docs/claude-code/skills).
   > Snippets are below.

**Implementation notes:**
- Path convention: `issues/<slug>/spec.md` and `issues/<slug>/plan.md` —
  matches the framework's own convention. Do NOT use the demo's
  `features/<slug>/` path here; the published guidance aligns with zero's
  own layout (per spec R4 reasoning).
- Frame the workflow without naming Claude Code anywhere until the bridge.

**Tests:** None. Step 8 covers the Claude Code link.

### Step 5: Write the "Skill snippets (Claude Code)" section

**Goal:** Deliver spec R4 — three complete, ready-to-copy skill files. Reader
copies each verbatim into `.claude/skills/<name>/SKILL.md`.

**Files:** `docs/agentic-coding.md`.

**Changes:** Replace `TODO — Step 5.` with:

1. **Lead paragraph (2 sentences):** explain that each snippet below is a
   complete Claude Code skill — paste verbatim into
   `.claude/skills/<name>/SKILL.md` and the loader picks it up. Link
   [skill documentation](https://docs.claude.com/en/docs/claude-code/skills).
2. **Three H3 subsections:** `### refine`, `### plan`, `### implement`. Each
   contains:
   - One sentence introducing the snippet's role and what it produces.
   - A fenced ```markdown``` code block with the full SKILL.md body, including
     YAML frontmatter (`name`, `description`).
3. **Cross-references within the snippets:**
   - `refine`'s "next step" line says: *Next, run `/plan <slug>`.*
   - `plan`'s "next step" line says: *Next, run `/implement <slug>`.*
   - `implement`'s loop references the **verification rhythm** section by
     anchor link (`#the-verification-rhythm`), and the **friction log**
     section by anchor link (`#a-friction-log-for-the-framework-itself`).

**Snippet shape (apply to all three):**

```markdown
---
name: <name>
description: <one-line; matches Claude Code's skill-loader expectation>
---

# <Name>

<one-paragraph workflow body>

## When to use
- <trigger>
- ...

## Steps
1. ...
2. ...

## Output
<the file or artifact this phase produces>

Next, run `/<next-skill> <slug>`.
```

**Source material to compress:**

- Demo's `~/Documents/code/zero_demo/.claude/skills/refine/SKILL.md` (~75
  lines) → target ~50 lines. Compress by:
  - Drop the demo-specific project-context list (`README.md`, `zero.toml`,
    `web/AGENTS.md` paths) — replace with one line: "Read `README.md` and
    AGENTS.md for project context."
  - Keep the **two-phase** structure (Explore → Spec) verbatim — that's the
    load-bearing detail.
  - Keep the spec template's section headers (`## Problem`, `## Goal`,
    `## Out of scope`, `## Acceptance criteria`, etc.); drop inline guidance
    that repeats what the headers already say.
- Demo's `plan/SKILL.md` (~75 lines) → target ~50 lines. Compress by:
  - Drop the demo-specific paths (`web/src/`, `features/<slug>/`); use
    `src/` and `issues/<slug>/`.
  - Keep the plan template's structure (Summary / Assumptions / Steps grouped
    by H3 / Verification / Open questions).
  - Drop the friction-log paragraph — it gets covered once in the page's R5
    section instead of in each skill.
- Demo's `implement/SKILL.md` (~150 lines) → target ~55 lines. Compress by:
  - Drop the long TDD-phase exposition — replace with: *"Behavioral steps:
    write a failing test first, add a stub that compiles but still fails on
    behavior, then make it pass."* (one line per phase, three lines total).
  - Drop the multi-paragraph "what counts as friction" — link to the friction
    log section instead.
  - Keep the **per-step loop** (1. identify; 2. state; 3. execute; 4. verify
    with `zero lint`; 5. log friction; 6. tick box; 7. report).
  - Keep the end-of-plan rhythm: `zero test --coverage` + `zero mutate
    --threads N`.

**Length check:** spec R4 caps each snippet at under 60 lines. After writing,
`awk` line counts inside each fenced block must be < 60.

**Implementation notes:**
- The `description` field in YAML frontmatter is what Claude Code's loader
  uses to decide when to trigger the skill — write it as: "This skill should
  be used when …" matching the demo's voice.
- Path convention inside snippets: `issues/<slug>/`, NOT `features/<slug>/`.
  This is a deliberate change from the demo per R4.
- The `implement` skill references commands; mark the same forthcoming
  commands (`zero fmt`, `zero preview`) with `(forthcoming)` if mentioned at
  all. The demo's `implement` references `zero dev` (which exists) and `zero
  mutate --threads 8 --quiet` (exists). Keep those; don't add `zero fmt` to
  this snippet at all — the rhythm section covers it, the skill body
  shouldn't reach for a command that doesn't exist.

**Tests:** `awk 'NR>1 && /^```$/{exit} /^```markdown$/{p=1; next} p' on each
snippet block — line count under 60. Step 8 covers the skill-doc URL.

### Step 6: Write the "A friction log for the framework itself" section

**Goal:** Deliver spec R5 — explain what `FRAMEWORK_NOTES.md` is, the format,
severity legend, fix-annotation pattern, and a copy-paste template. 3-5
example entries spanning each severity make the format concrete.

**Files:** `docs/agentic-coding.md`.

**Changes:** Replace `TODO — Step 6.` with:

1. **What it is.** 2-3 sentences: append-only log of zero-framework bugs,
   gaps, and footguns surfaced while building on top of the framework. Every
   adopter is also a tester of the surface; without a log, that signal is
   lost.
2. **The format.** A single fenced block showing the entry shape:

   ```
   - [ ] `YYYY-MM-DD` 🔴/🟡/🟢 **short name** — what happens; the workaround if any. Area: <templates | zero/test | zero/components | …>
   ```

3. **Severity legend.** Three-line bullet list:
   - 🔴 **Broken or misleading** — silent wrong behavior, confusing error,
     footgun likely to bite repeatedly.
   - 🟡 **Missing** — something you reach for; you can work around it but
     ergonomically poor.
   - 🟢 **Papercut** — minor annoyance with an obvious workaround.

4. **How to mark fixed.** One paragraph + example: flip `- [ ]` to `- [x]`
   and append `**FIXED YYYY-MM-DD** (#PR / SHA): one-sentence note` on the
   same line. Don't delete. Partial fixes use `**PARTIAL YYYY-MM-DD:** …`.
5. **Where to file the actual issue.** One sentence: a friction-log entry is
   *not* a substitute for filing an issue against the framework — link the
   entry to the issue (GitHub URL, ticket number, whatever).
6. **The full copy-paste template** as a fenced ```markdown``` block. Lift
   verbatim from `~/Documents/code/zero_demo/FRAMEWORK_NOTES.md` lines
   1-30 (already battle-tested per spec R5), with one substitution: replace
   the demo-specific opening sentence ("…while building this app on top of
   the `zero` framework. The point: keep ground-truth feedback…") with a
   generic placeholder phrasing (`<Project>` instead of "this app").
7. **3-5 example entries** in their own fenced block, spanning each severity.
   Pick from the demo's `FRAMEWORK_NOTES.md` `## Entries` section,
   prioritizing ones whose subject matter is already public (i.e. closed
   friction-log items already cited in this repo's own `issues/` directory).
   Suggested set (planner's recommendation, executor confirms):
   - 🔴 from line 31 of demo notes ("partial-string `class=` interpolation
     silently broken" — fixed, has FIXED annotation).
   - 🟡 from demo notes ("`zero mutate` missing from AGENTS.md Quick Start"
     — open, no fix annotation; shows the open-entry shape).
   - 🟢 from demo notes ("S01 lint fires on test `describe` bodies" — open).
   - 🔴 fixed example with a `(SHA)` annotation to model the citation
     format.
   Quote them verbatim (date and emoji included). If an entry references
   `~/Documents/code/zero_demo` paths, trim that prefix — the examples are
   shown as illustrations, not as a back-reference to the demo specifically.

**Implementation notes:**
- The template's `## How to add an entry` and `## How to mark an entry
  fixed` sections in the demo are already at the right granularity — lift
  verbatim.
- The full template lives inside one fenced ```markdown``` block so a reader
  can triple-click + copy.

**Tests:** None. Step 8 covers length and link validity.

### Step 7: Wire cross-references (index.md "Start here" bullet, getting-started nod, internal page links)

**Goal:** Deliver spec R1 (index.md bullet) and R6 (cross-links). After this
step the new page is discoverable from "Start here," nudged from Getting
Started, and links out to the four reference pages the rhythm cites.

**Files:**

- `docs/index.md` — add "Start here" bullet.
- `docs/getting-started.md` — add one-line nod at the end (optional but
  spec-recommended).
- `docs/agentic-coding.md` — confirm internal links (placed in earlier
  steps) all resolve to existing files.

**Changes:**

1. **`docs/index.md`.** Under `## Start here`, between the Getting Started
   bullet and the Reactivity bullet, insert:

   ```markdown
   - **[Agentic coding](./agentic-coding.html)** — the verification rhythm
     zero's tooling is designed around. Read this before your second feature.
   ```

   Match the indentation and prose style of the existing two bullets.

2. **`docs/getting-started.md`.** At the very end of the file (after the
   last existing section), append a single concluding paragraph (planner's
   call per spec R6: yes, include it):

   ```markdown
   Once you've shipped your first edit, read
   [Agentic coding with zero](./agentic-coding.html) for the verification
   rhythm we recommend.
   ```

   If `getting-started.md`'s last existing section already has a "next
   steps" pointer, the executor inserts this bullet inside that block
   instead of appending a new paragraph. Read the last ~20 lines before
   editing.

3. **`docs/agentic-coding.md` internal links** (added in steps 3-5; this
   step is the consolidated audit). The four outbound links the page must
   contain — all relative `.html` paths matching the rest of the site:
   - `./linting.html` — from the `zero lint` row of the rhythm table.
   - `./testing.html` — from the `zero test` and `zero test --coverage`
     rows.
   - `./config-and-cli.html#zero-mutate` — from the `zero mutate` row.
   - `https://docs.claude.com/en/docs/claude-code/skills` — from the
     bridge sentence in Step 4 and the lead paragraph in Step 5.

   Verify each appears at least once by `grep -n` on the file.

**Tests:** `grep -nE "linting\.html|testing\.html|config-and-cli\.html|docs\.claude\.com" docs/agentic-coding.md`
should hit all four. `grep -n "agentic-coding\.html" docs/index.md` should
return one match.

### Step 8: Verify — link sweep, length budget, Jekyll smoke build

**Goal:** Confirm spec R7 (acceptance) and R8 (length). No automated test
asserts page content; these are the sanity checks.

**Files:** No edits. Verifications only.

**Changes:** Run, in order:

1. **Length budget (R8):** `wc -l docs/agentic-coding.md` — expect 400–600
   lines, hard cap 800. If over 800, the executor revisits the page and
   either compresses prose or splits the skill snippets out per R8's
   fallback (`docs/agentic-coding-skills.md` with a link).
2. **Outbound link sweep:** for each `[`*`text`*`](`*`URL`*`)` in
   `docs/agentic-coding.md`:
   - Relative `./*.html` links: confirm the corresponding `.md` file exists
     in `docs/`. (e.g. `./linting.html` ↔ `docs/linting.md`.)
   - Absolute `https://` links: spot-check by reading the URL — no curl
     required for `docs.claude.com` (the URL is the canonical published
     skill doc and is stable; the executor flags it only if it looks
     malformed).
3. **`nav_order` collision check:** `grep -nE "^nav_order: [0-9]+" docs/*.md
   | sort -t: -k3,3n` — every value 1..17 should appear exactly once.
4. **Jekyll smoke build (best-effort):** if `bundle exec jekyll build` runs
   cleanly in `docs/` (the executor checks for `Gemfile`/`_config.yml`
   tooling), run it and confirm the page renders into `_site/`. If no
   Jekyll toolchain is locally available, fall back to: confirm
   `docs/_config.yml` does not exclude `agentic-coding.md` (the existing
   `exclude:` list excludes only `README.md`, `BEST_PRACTICES.md`, the
   issues tree, etc. — verified at planning time, no edit needed).
5. **Verbatim-sentence check:** `grep -n "tests can pass vacuously"
   docs/agentic-coding.md` must return at least one match (R2 requirement).

**Tests:** Same as the steps above. If any check fails, the executor stops
and reports — does not tick this box.

## Risks and Assumptions

- **Just-the-Docs nav order is contiguous.** Renumbering everything from
  3→17 assumes the sidebar honors the new order without further config. If
  `_config.yml` pins page order explicitly somewhere not noticed at
  planning time, Step 8's smoke build catches it. Verified that
  `_config.yml` has no `pages:` or `order:` list — only `exclude:`.
- **The demo's skill files compress cleanly to under 60 lines.** Spec R4's
  budget. If compression loses load-bearing detail, R4 allows splitting
  into `docs/agentic-coding-skills.md`. Step 8's length check is the
  trigger.
- **`(forthcoming)` is acceptable for `zero preview` and `zero fmt` in the
  rhythm table.** Spec constraint explicitly allows this. If the user
  prefers omitting `zero fmt` entirely (since it's not even spec'd
  anywhere), drop the row — replanning required only if the user wants
  omission to be the default.
- **No AGENTS.md edit required by this plan.** Spec R6 states the
  AGENTS.md cross-reference belongs to `issues/agents-quickstart/` and is
  filed as a follow-up if this lands first. Confirmed at planning time
  that `agents-quickstart` hasn't shipped yet.
- **Hand-edits to 14 frontmatter files in Step 1 are mechanical.** Each
  change is `nav_order: N` → `nav_order: N+1`. Risk of typo is low; risk
  of missing a file is low because the table in Step 1 enumerates all of
  them. A `grep -nE "^nav_order:" docs/*.md` after Step 1 is the
  cheapest possible sanity check.
- **The example friction-log entries are non-sensitive.** Source is the
  demo's `FRAMEWORK_NOTES.md`, which the repo's own `issues/*/spec.md`
  already cites publicly (preview, mutate-operators, mutate-equivalence,
  agents-quickstart all quote line numbers from it). Lifting 3-5 lines for
  illustrative purposes is consistent with that precedent.
