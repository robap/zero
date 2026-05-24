# Spec: AGENTS.md Quick Start — surface `mutate`, `--coverage`, key flags, and when to run them

## Problem Statement

The scaffolded `AGENTS.md` (`crates/zero-scaffold/src/scaffold/AGENTS.md`) is the canonical in-tree reference an agent reads when working inside a zero project. Its Quick Start block (lines 16-28) lists seven subcommands by name. Four friction-log entries against that block:

- **L44 🟡 `zero mutate` is missing entirely.** `zero --help` lists it, but agents reading AGENTS.md as authoritative never learn it exists. A major correctness tool stays unused.
- **L45 🟢 Quick Start doesn't say *when* to run `zero lint` or `zero mutate`.** They're listed (well, `lint` is — `mutate` isn't yet), but nothing tells an agent "lint after every code change, mutate before declaring done." Easy to drift into never running them.
- **L49 🟡 `zero test --coverage` is missing.** The flag exists (`zero test --help`: "Emit line + function coverage from src/ to terminal and coverage/coverage.json"), but AGENTS.md mentions `zero test` with no flag information. Coverage data is the cheap pre-check before paying the mutation-testing cost — agents who don't know about it skip the leverage.
- **L52 🟡 No flag information per command.** `zero build` has `--sourcemap` / `--no-sourcemap`; `zero mutate` has `--operators` / `--max-mutants` / `--threads` / `--quiet`; `zero init` / `zero update` have `--yes`; `zero lint` has `--quiet`. None appear in AGENTS.md. Agents either run commands without the flag they needed or stumble into `zero --help` second-hand.

All four are AGENTS.md edits in or adjacent to the Quick Start. They share a file, an audience, and a shape. Bundling them is cheaper than four edits to the same paragraph.

Caught in the demo's friction log (`~/Documents/code/zero_demo/FRAMEWORK_NOTES.md:44, 45, 49, 52`).

## Background

### What AGENTS.md is and is not

From the file itself (line 3): *"a condensed in-tree reference for agents working inside a scaffolded project. The full user guide lives at <https://robap.github.io/zero/> — every section below links to its long-form chapter."* The house style is:

- **Terse.** One-line bullets, short tables.
- **Link out for detail.** Every section ends with a link to the corresponding user-guide chapter.
- **Reach-for table** pattern (see lines 188-208 for Styles): a small table whose rows are "the agent's situation" and whose cells are "what to use."
- **Self-contained for the 80% case.** An agent should be able to write code without leaving AGENTS.md for the common path.

The Quick Start is currently a code block (lines 18-26), then a link to `config-and-cli.html`. Adding per-command flag detail belongs in that same code block (one-line `# comment` per row), not in a new heading-rich subsection that breaks the file's rhythm.

### What "when to run" means in this codebase

The verification commands an agentic workflow leans on are:

| Command | Why it matters | When to run |
|---|---|---|
| `zero lint` | Cheap, fast; catches the rule-table footguns from "Common mistakes". | After any code edit that touches `.ts` / `.js` / `.scss`. Sub-second. |
| `zero test` | Behavior verification. | After any code change that could break a test. |
| `zero test --coverage` | Names lines that have no test reaching them. | Before declaring a task done, OR before deciding whether `mutate` is worth running. |
| `zero mutate` | Tests can pass vacuously; mutation testing forces the question "would the test actually fail if I broke this?" | Before declaring a task done on correctness-critical code, especially after `--coverage` shows the lines are reached. |
| `zero fmt` | Idempotent format. | Before commit. |

This table is the L45 content. Spec keeps it tight (3-5 bullets in AGENTS.md) and points the deeper why ("tests can pass vacuously…") at the agentic-coding onboarding page (separate spec, `issues/agentic-coding/`).

### What `zero --help` actually shows today

The Quick Start in AGENTS.md needs to match the real CLI surface. From a fresh `zero --help` run:

- `zero init [--yes] [--no-fmt]`
- `zero update [--yes]`
- `zero dev`
- `zero test [pattern] [--coverage]`
- `zero build [--sourcemap | --no-sourcemap]`
- `zero preview` *(this spec assumes `issues/preview/` lands; if it hasn't, the row notes "see `issues/preview/`")*
- `zero lint [--quiet]`
- `zero mutate [pattern] [--operators ID,…] [--max-mutants N] [--threads N] [--quiet]`
- `zero fmt`
- `zero gen component <name>` / `zero gen route <path>`
- `zero upgrade`

The Quick Start today lists 7 subcommands (init, update, dev, test, build, preview, lint) without flags. After this spec, the block lists each with its 1-3 most useful flags and includes `mutate`.

### Adjacent surfaces

- **`crates/zero-scaffold/src/scaffold/AGENTS.md`** — the only file edited by this spec.
- **`crates/zero/tests/`** — any fixture or assertion that pins specific Quick Start lines must be updated. (Spot check during planning: grep for `"zero dev"` / `"Quick start"` in tests.)
- **`docs/config-and-cli.md`** — the link target. Verify that every flag listed in the new AGENTS.md Quick Start is documented in `config-and-cli.md`. If a flag is in `--help` but not in the docs, that's a documentation bug outside this spec's scope — file separately; do not block on it.
- **The demo's `web/AGENTS.md`** (`~/Documents/code/zero_demo/web/AGENTS.md`) — out of scope. It's a scaffold-generated copy that refreshes via `zero update`. After this spec lands and the demo runs `zero update`, the changes appear there.
- **`crates/zero-scaffold/src/lib.rs`** — `TPL_AGENTS_MD` (or equivalent) already includes the file via `include_str!`. No manifest change; the edit ships through the existing pipeline.

### What this spec is NOT

This spec covers AGENTS.md *Quick Start* edits only. The friction-log entry L56 ("no agentic-coding onboarding page") is a separate spec (`issues/agentic-coding/`) because it adds a new docs page (not an in-tree reference edit) and a workflow narrative that doesn't fit AGENTS.md's terse style. The two specs cross-reference: AGENTS.md's "when to run" bullets link to the onboarding page for the deeper rationale.

## Requirements

### R1 — Quick Start block lists every subcommand with its key flags

Replace the existing Quick Start code block (`crates/zero-scaffold/src/scaffold/AGENTS.md:18-26`) with an expanded code block. Each row is a single line: `zero <subcommand> [<key flag> …]    # purpose`. The block must:

- Include every subcommand the user-facing CLI exposes (currently: `init`, `update`, `dev`, `test`, `build`, `preview`, `lint`, `mutate`, `fmt`, plus `gen` and `upgrade` — planner confirms the current `--help` list).
- Show 1-3 key flags per command in `[brackets]`. "Key" = flags an agent will reach for in normal work, not every flag the binary accepts.
- Keep the comment column terse (≤ 60 chars).
- Keep the table visually scannable — column-align the comments using spaces if it fits the file's existing rhythm.

The closing line under the block stays: a link to `config-and-cli.html` for the full reference. Add a parenthetical: *"every flag the CLI accepts is documented there."*

Exact text is a planner choice. Skeleton for shape (not the literal output):

```bash
zero init [--yes] [--no-fmt]      # scaffold a project
zero update [--yes]               # refresh framework-owned files
zero dev                          # dev server (file watching + reload)
zero test [pattern] [--coverage]  # run *.test.{ts,js}; --coverage writes coverage/coverage.json
zero build [--sourcemap]          # production build
zero preview                      # serve dist/ locally (auto-runs build)
zero lint [--quiet]               # SCSS + JS/TS idiom checks
zero mutate [pattern] [--threads N] [--operators ID,…] [--max-mutants N] [--quiet]
                                  # mutation testing across src/
zero fmt                          # idempotent formatter
```

### R2 — A short "When to run what" subsection lives directly under Quick Start

Add a new subsection after the Quick Start block (between current lines 28 and 30) titled `### When to run what` (or equivalent — title is a planner choice; "Verification rhythm" works too). The subsection is a bulleted list, one bullet per command-or-flag, each ≤ 2 lines:

- `zero lint` — after any `.ts` / `.js` / `.scss` edit. Cheap; catches the L- and R- rules above before they reach tests.
- `zero test` — after any logic change. `--coverage` writes `coverage/coverage.json` and prints per-file line / function coverage to the terminal.
- `zero mutate` — before declaring a task done on correctness-critical code. Pass `--threads N` to parallelize (defaults to a reasonable CPU-aware value once `issues/mutate-operators/` lands; planner verifies). `--operators arith,boundary` narrows the run.
- `zero fmt` — before commit.

The bulleted form keeps it scannable without elevating it to a full chapter (which is the onboarding page's job).

End the subsection with a one-line cross-reference: *"Why this rhythm matters: see the [Agentic coding with zero](TODO-link) chapter."* If the onboarding spec (`issues/agentic-coding/`) has not landed yet, the planner replaces the link with `(forthcoming)` or omits the cross-reference until the page exists. Spec must not introduce a broken link.

### R3 — `zero mutate` and `zero test --coverage` are visible

The Quick Start block (R1) lists `zero mutate` as a top-level row. The "When to run what" subsection (R2) mentions both `--coverage` and `mutate` with one-line rationale. After this spec, an agent reading AGENTS.md top-to-bottom encounters mutation testing and coverage *before* writing their first test.

### R4 — The flag list stays in sync with the CLI

The new flag information must reflect what `zero <subcommand> --help` actually accepts as of the implementation date. The planner runs `zero --help` and each `zero <subcommand> --help` during implementation to verify; the spec does not pin specific flag names beyond the skeleton in R1, because the CLI surface may evolve between spec and plan.

If a flag mentioned in this spec's skeleton doesn't exist in `--help` at plan time, the planner drops it (and surfaces the doc-vs-code divergence as a follow-up). Spec does not gate on the existence of any specific flag.

### R5 — Tests

`crates/zero/tests/` (or wherever scaffold-output assertions live):

- If a test pins exact AGENTS.md content (`include_str!` byte-equal or similar), update it to match the new block.
- Add a regression assertion: the AGENTS.md scaffold output contains the literal string `zero mutate` and `--coverage`. Single grep-style assertion is enough — pin behavior, not exact text.
- Add a regression assertion: the AGENTS.md scaffold output contains a "When to run" (or whatever the planner titles R2) heading. Same shape.

Planner picks file location; this assertion belongs alongside whichever existing tests cover scaffold AGENTS.md correctness. If none exist, the planner adds one in a sensible spot — `crates/zero-scaffold/tests/`.

### R6 — Docs ripple

- **`docs/config-and-cli.md`** — verify every flag named in the new Quick Start exists in the long-form reference. If gaps exist (a flag in `--help` but not in `config-and-cli.md`), file a follow-up — do not block this spec. (Spec note for planner: this is a likely small punch-list; record it in `plan.md`.)
- **`docs/index.md`** — no change. AGENTS.md is in the scaffold, not the user-guide doc tree.
- **Cross-link target for R2.** If `issues/agentic-coding/` lands first, R2's "Why this rhythm matters" link points at the published URL. If this spec lands first, R2 omits the link or marks `(forthcoming)`.

## Constraints

- No new dependencies or files; pure edits to one Markdown file plus a test or two.
- AGENTS.md must continue to render legibly in plain-text viewers (cat / grep). No Markdown-rendering-specific tricks.
- The Quick Start code block stays a single fenced ` ```bash ` block. Splitting it into multiple blocks fragments the visual scan.
- Adding the "When to run what" subsection adds at most ~15 lines to the file. AGENTS.md is already long (~488 lines); the addition must earn its place by surfacing previously-invisible signal — not by expanding for its own sake.
- The 80-line-per-function guideline does not apply (this is docs). The terseness guideline does: every added bullet must be a sentence or fragment, not a paragraph.
- This spec does not edit any flag's behavior in the CLI; if a flag described here doesn't exist, the spec defers to the actual `--help` output (R4).

## Out of Scope

- **Replacing the Quick Start with a workflow narrative.** That's the agentic-coding onboarding page's job (`issues/agentic-coding/`).
- **Every flag for every subcommand.** R1 says "1-3 key flags." A user wanting the exhaustive list follows the link to `config-and-cli.html`.
- **Editing the demo's `web/AGENTS.md` directly.** It refreshes via `zero update`. After this spec lands, anyone in the demo runs `zero update` and picks up the changes.
- **Adding a "Workflow" or "How to develop" chapter to the user guide.** Same as above — that's the onboarding page.
- **Reformatting the rest of AGENTS.md.** Touch only the Quick Start block and the new subsection. Leave the rest alone.
- **Tooling that auto-syncs `--help` output into AGENTS.md.** Nice in theory; out of scope for a doc edit. R4 makes it the planner's job at implementation time, manually.

## Open Questions

- **Quick Start row order.** Today (line 19-26) the order is: `init`, `update`, `dev`, `test`, `build`, `preview`, `lint`. Adding `mutate` and `fmt`: should they slot in alphabetically, by "phase of work" (init → dev → test → mutate → build → preview → lint → fmt), or some other order? Spec recommends phase-of-work since the file's audience reads top-down. Planner confirms.
- **Title of the new subsection.** "When to run what" / "Verification rhythm" / "How to verify your work" all work. Planner picks the one that reads best alongside the rest of AGENTS.md's heading style.
- **Link to the onboarding page.** R2's last line cross-references `issues/agentic-coding/`. If that spec hasn't landed when this one is implemented, planner omits the link (don't ship a broken anchor) and adds a TODO in `plan.md` to revisit when the page exists.
- **Whether to mention `zero --help` itself.** AGENTS.md doesn't currently nudge readers toward the CLI's own help. A one-line *"Every subcommand has `--help`."* in the new subsection might be worth it. Planner decides; not a contract requirement.
- **Coverage-output shape.** R2 describes `--coverage` as "writes `coverage/coverage.json` and prints per-file line / function coverage to the terminal." Planner confirms the exact output (path + console format) against the actual CLI behavior at plan time and adjusts if needed.
