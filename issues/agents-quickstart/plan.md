# Plan: AGENTS.md Quick Start — surface `mutate`, `--coverage`, key flags, and when to run them

## Summary

Edit one file (`crates/zero-scaffold/src/scaffold/AGENTS.md`) so that the Quick
Start block lists every subcommand the CLI actually exposes today *with* its
1–3 key flags, and add a terse "When to run what" subsection directly below.
Then extend the existing scaffold-output tests in
`crates/zero-scaffold/src/lib.rs` with three regression assertions
(`zero mutate`, `--coverage`, and a section sentinel for the new subsection)
so the new content can't silently disappear in a future refresh. No new files,
no new dependencies, no CLI behavior change.

## Prerequisites

None. The spec is implementable against today's CLI surface. Two spec-mentioned
subcommands (`preview`, `fmt`) and one flag (`init --no-fmt`) do not exist in
the current binary, so per R4 they are dropped from the new Quick Start and
recorded in **Risks and Assumptions** for downstream specs to pick up.

## Steps

- [ ] **Step 1: Rewrite the AGENTS.md Quick Start block to include flags and `mutate`**
- [ ] **Step 2: Add the "When to run what" subsection immediately below**
- [ ] **Step 3: Extend the scaffold regression tests in `crates/zero-scaffold/src/lib.rs`**
- [ ] **Step 4: Cross-check `docs/config-and-cli.md` and record any flag-coverage gaps as follow-up**

---

## Step Details

### Step 1: Rewrite the AGENTS.md Quick Start block to include flags and `mutate`

**Goal:** Replace the flag-less 7-row Quick Start block with one that names
every subcommand the CLI actually exposes today, each with its 1–3 most useful
flags. After this step, an agent reading AGENTS.md top-to-bottom sees
`zero mutate` and `zero test --coverage` for the first time.

**Files:**
- `crates/zero-scaffold/src/scaffold/AGENTS.md` (lines 18–28: the fenced
  `bash` block plus the trailing CLI-reference link)

**Changes:**

Replace lines 18–26 (the existing block) with a single fenced ` ```bash ` block
containing exactly the following seven rows, in phase-of-work order
(`init` → `update` → `dev` → `test` → `mutate` → `build` → `lint`). Comments
are right-padded with spaces so the `#` column lines up; comment text stays
≤ 60 chars. The exact text the executor writes:

```bash
zero init [--yes]                   # scaffold a project
zero update [--yes]                 # refresh framework-owned files in .zero/
zero dev                            # dev server (file watch + full-page reload)
zero test [pattern] [--coverage]    # run *.test.{ts,js}; --coverage to coverage/coverage.json
zero mutate [pattern] [--threads N] [--operators ID,…] [--max-mutants N] [--quiet]
                                    # mutation testing across src/
zero build [--sourcemap|--no-sourcemap]   # production build
zero lint [--quiet]                 # SCSS + JS/TS idiom checks
```

The `mutate` row deliberately wraps to a second line for its comment — the
flag list is the longest of any row, and forcing it onto one line breaks the
visual scan. The continuation comment is indented to land in the same column
as the others.

Replace the trailing line (currently line 28:
`Full CLI reference: <https://robap.github.io/zero/config-and-cli.html>.`)
with:

```
Full CLI reference: <https://robap.github.io/zero/config-and-cli.html> —
every flag the CLI accepts is documented there.
```

Rationale for the chosen flag set, row by row (the executor does **not** need
to add these as code comments — they are here so the executor can defend the
choice if the user asks):

- `init`, `update` — `--yes` is the only flag either exposes (verified against
  `zero init --help` / `zero update --help`); use it.
- `dev` — no flags exist today. Row stays.
- `test` — `[pattern]` positional + `--coverage`. These are the only two
  surface bits and both are routinely reached for.
- `mutate` — all four flags listed. Each is something an agent will reach for:
  `--threads` for speed, `--operators` to narrow, `--max-mutants` to cap a big
  run, `--quiet` for CI logs.
- `build` — both forms of the sourcemap flag, since the meaningful choice is
  "force on / force off" relative to `zero.toml`.
- `lint` — `--quiet` is the only flag.

**Subcommands intentionally NOT listed:** `preview`, `fmt`, `gen`, `upgrade`.
None exist in `zero --help` today. Note: the *current* AGENTS.md already
advertises `preview` as if it worked (line 24); this step removes that line,
which is a small regression in surface area but an honest reflection of
shipped behavior. `issues/preview/` is the spec that re-adds it.

**Tests:** No test changes in this step. The existing sentinel test
(`write_initial_project_agents_md_has_section_sentinels`,
`crates/zero-scaffold/src/lib.rs:611`) still passes because it only checks
that `## Quick start` exists — which it does. After this step,
`cargo test -p zero-scaffold` should be green with no edits.

---

### Step 2: Add the "When to run what" subsection immediately below

**Goal:** Tell agents *when* to invoke each verification command. Without
this, listing `mutate` and `--coverage` (Step 1) doesn't convert into use —
an agent still doesn't know that lint runs every step, that coverage is the
cheap pre-check before mutation, or that mutate runs before declaring done.

**Files:**
- `crates/zero-scaffold/src/scaffold/AGENTS.md` (insert directly after the
  expanded Quick Start block produced by Step 1, before the existing
  `Generated project layout:` line — i.e., between what is currently line 28
  and line 30)

**Changes:**

Insert the following block, verbatim, after the trailing CLI-reference line
from Step 1 and before the existing `Generated project layout:` heading:

```markdown
### When to run what

- `zero lint` — after any `.ts` / `.js` / `.scss` edit. Sub-second; catches the
  L- and R- rules below before they reach tests.
- `zero test` — after any logic change. Add `--coverage` to write
  `coverage/coverage.json` and print per-file line / function coverage to the
  terminal.
- `zero mutate` — before declaring a task done on correctness-critical code.
  `--threads N` parallelizes (defaults to `min(cores, 8)`);
  `--operators arith,cmp` narrows the run; `--max-mutants N` caps it.
- Every subcommand has its own `--help`.
```

Heading level is `###` (the parent `## Quick start` is `##`, matching the
existing rhythm of `### Reach for these first` / `### When to reach for which
primitive` under Styles).

Title choice: **"When to run what"** — sentence-case, parallel construction
with the existing `### When to reach for which primitive` subsection under
Styles. The other two open-question candidates ("Verification rhythm",
"How to verify your work") were considered and rejected: the former
introduces vocabulary that's not used anywhere else in the file; the latter
is wordier without adding signal.

The fourth bullet (`Every subcommand has its own --help`) addresses spec
Open Question 4 — a one-line nudge toward the CLI's own help that doesn't
cost the file anything. Keep it.

**Deliberately omitted:** the `fmt` bullet from spec R2 (because `zero fmt`
does not exist in the current CLI) and the
`[Agentic coding with zero](TODO-link)` cross-reference from spec R2 (because
the chapter does not yet exist — `issues/agentic-coding/` has only a spec).
Both are tracked in **Risks and Assumptions** for re-introduction when their
respective specs land. No `(forthcoming)` placeholder ships — the spec's
constraint against broken links applies.

**Tests:** No test changes in this step. The new subsection is asserted in
Step 3.

After Step 2, manually verify in the editor that:

1. The Quick Start fenced block remains a single ` ```bash ` block (spec
   constraint).
2. The new `### When to run what` block sits between the Quick Start fence
   close and `Generated project layout:`.
3. AGENTS.md still renders legibly in `cat` / `less` — no
   Markdown-rendering-specific tricks.

`cargo test -p zero-scaffold` should still be green.

---

### Step 3: Extend the scaffold regression tests in `crates/zero-scaffold/src/lib.rs`

**Goal:** Pin behavior (not exact text) so a future AGENTS.md refresh can't
silently delete the things this spec added. Three new assertions: `zero mutate`
appears, `--coverage` appears, and the `When to run what` heading appears.

**Files:**
- `crates/zero-scaffold/src/lib.rs` — extend the existing
  `write_initial_project_agents_md_has_section_sentinels` test
  (currently lines 611–645).

**Changes:**

Inside `write_initial_project_agents_md_has_section_sentinels`, add
`"### When to run what"` to the existing array of sentinel strings (the array
that today contains `"## Quick start"`, `"## Imports"`, etc.). Keep the array
in the same shape — one new entry, no other reordering.

Immediately after the existing `for sentinel in [...]` loop and before the
`framework-owned just like the files under '.zero/'` assertion, add three
new assertions:

```rust
assert!(
    agents.contains("zero mutate"),
    "AGENTS.md Quick Start must mention `zero mutate`: {agents}"
);
assert!(
    agents.contains("--coverage"),
    "AGENTS.md Quick Start must mention `--coverage`: {agents}"
);
```

(The `When to run what` heading is already covered by the sentinel-array
addition above, so it does not need its own separate `assert!`.)

The choice of `agents.contains("zero mutate")` rather than e.g.
`"# mutation testing"` follows the spec's R5 directive: pin *behavior*
(this string appears somewhere), not exact text (which would re-break on
the next legitimate edit).

**Tests:** This step *is* the test changes. Run `cargo test -p zero-scaffold`
after the edit; expect:

- `write_initial_project_agents_md_has_section_sentinels` passes (the new
  sentinel `### When to run what` is satisfied because Step 2 added that exact
  heading).
- The two new `assert!` calls pass (Step 1 added `zero mutate` to the block
  and Step 1 + Step 2 both reference `--coverage`).

Also run the whole workspace (`cargo test --workspace`) to confirm no
downstream snapshot or `include_str!`-equality test pinned the old AGENTS.md
contents byte-for-byte. (Searched at plan time — none found — but verify.)

---

### Step 4: Cross-check `docs/config-and-cli.md` and record any flag-coverage gaps as follow-up

**Goal:** Per spec R6, verify every flag the new Quick Start names is also
documented in the long-form CLI reference. Spec is explicit that this does
**not** block the spec — gaps are filed as follow-ups, not fixed here.

**Files:**
- `docs/config-and-cli.md` — read-only inspection.

**Changes:**

None to source. The executor:

1. Greps `docs/config-and-cli.md` for each flag listed in the new Quick Start:
   `--yes`, `--coverage`, `--sourcemap`, `--no-sourcemap`, `--threads`,
   `--operators`, `--max-mutants`, `--quiet`.
2. Records the result in this `plan.md` (or directly in the conversation if
   the file has already been handed off to a reviewer) under a short
   **Follow-up notes** trailing section. Plan-time spot-check (executor
   should re-verify, since CLI/docs may have moved):

   - `--yes` — documented (lines 93, 107 of `config-and-cli.md`).
   - `--coverage` — documented (line 156).
   - `--sourcemap` — documented (line 141). `--no-sourcemap` should be
     verified at execution time; spec R6 says file a follow-up if missing.
   - `--threads`, `--operators`, `--max-mutants`, `-q/--quiet` for `mutate`
     — all documented in the `zero mutate` subsection (lines 170–173 at plan
     time).
   - `-q/--quiet` for `lint` — documented (line 266 at plan time).

   If any flag listed in the new Quick Start is *not* present in
   `config-and-cli.md`, the executor opens a follow-up issue (separate from
   this spec) and notes it here. Plan does **not** treat that as a
   step-failure condition.

**Tests:** None. This step is documentation cross-check, not a code change.

---

## Risks and Assumptions

- **`preview` is intentionally absent.** The current AGENTS.md already
  advertises `zero preview` despite the subcommand not existing
  (`zero preview` errors with `unrecognized subcommand`). Step 1 removes
  the row. When `issues/preview/` lands, that spec is responsible for
  re-adding the row (and the corresponding `When to run` line if relevant).
  If `issues/preview/` lands *before* this one, the executor should re-add
  a `preview` row in `init → update → … → build → preview → lint` order
  during Step 1.

- **`fmt` is intentionally absent.** Spec R1's skeleton lists
  `zero fmt # idempotent formatter` and spec R2 lists a corresponding
  bullet ("`zero fmt` — before commit"). The subcommand does not exist —
  `zero fmt` errors with `unrecognized subcommand` — and no `issues/fmt/`
  spec is open at plan time. Both the row and the bullet are dropped. If
  `fmt` ships later, that spec re-adds them.

- **`gen` and `upgrade` are absent.** Spec R1's background paragraph
  (line 56) names them; neither exists. Drop without ceremony.

- **`init --no-fmt` does not exist.** Spec R1's skeleton mentions
  `zero init [--yes] [--no-fmt]`; `init --help` shows only `--yes`. Row
  uses `[--yes]` only.

- **Cross-reference to `docs/agentic-coding.md` is omitted.** Spec R2
  asks for a "Why this rhythm matters" link to the agentic-coding chapter.
  That chapter does not exist (`issues/agentic-coding/` has spec but no
  `docs/agentic-coding.md`). Per spec R6 ("Spec must not introduce a broken
  link") the link is omitted entirely — no `(forthcoming)` placeholder.
  When `docs/agentic-coding.md` ships, that spec's plan adds the link
  back into this subsection.

- **Demo's `web/AGENTS.md`** at `~/Documents/code/zero_demo/web/AGENTS.md`
  is **out of scope** per spec line 153. It refreshes via `zero update`
  after this lands. The plan does not run `zero update` on the demo.

- **The `mutate` row wraps to two lines.** This is the only row whose
  flag list runs long. If the executor finds the wrap visually ugly in the
  rendered Markdown (it should be fine in plain-text viewers per the spec
  constraint), an acceptable alternative is to compress the comment to
  `# mutation testing` and drop `[--max-mutants N]` from the row — but
  spec R3 wants `mutate` visibly weighty, so the executor should prefer
  the wrap over dropping flags.

- **No `cargo install` step is needed.** `cargo test -p zero-scaffold`
  builds against the scaffold crate directly; the binary installed at
  `~/.cargo/bin/zero` is not touched and does not need to be reinstalled
  for this work to verify. The executor should not run `cargo install`.

- **No `zero update` is needed in this repo.** The repo *itself* is not a
  scaffolded project — there is no `AGENTS.md` at the repo root. The only
  AGENTS.md edited is the embedded template at
  `crates/zero-scaffold/src/scaffold/AGENTS.md`, which ships via
  `include_str!` to every scaffolded user project on next `zero update`.
