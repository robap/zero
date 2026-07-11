# Spec: `zero mutate` — incremental runs via fingerprint cache

## Problem Statement

`zero mutate` redoes all of its work on every invocation: a full baseline
suite run with coverage, then one test run per mutant — even when most (or
all) of the project is byte-identical to the previous run. On a project of
any size the per-mutant runs dominate wall time, and the agentic workflow
the framework is built around (`docs/agentic-coding.md`) asks users and
agents to run mutation testing *repeatedly* as a verification step. Today
each repeat pays full price.

This item adds a fingerprint cache: each source file's mutant verdicts are
recorded together with a hash of everything that could change those
verdicts. A later run reuses the recorded verdicts for any file whose
fingerprint still matches and only re-mutates what changed. When *nothing*
changed, the run skips the baseline too and replays the previous result
near-instantly. The mutation score stays exactly as trustworthy as a full
run — the fingerprint is sound by construction (see Background) — while a
typical edit-one-file iteration drops from "whole project" to "one file's
mutants plus one baseline run."

## Background

### Current pipeline (`crates/zero/src/cmd/mutate.rs`)

`run_inner` does, in order:

1. **Discovery** — `discover()` finds all test files.
2. **Baseline** — `run_baseline()` runs every test file with coverage,
   producing:
   - `passed` — gate; mutation refuses to run on a red suite,
   - `covered: HashMap<PathBuf, HashSet<u32>>` — per-source covered lines,
   - `src_to_tests: HashMap<PathBuf, Vec<PathBuf>>` — which test files
     loaded each in-scope source file (from `outcome.loaded`).
3. **Generation** — `walk_src` + `filter_src_files` (the `[pattern]`
   target), then `generate_all_sites` filtered by covered lines.
4. **Pre-apply** — byte-equivalence skip (`pre_apply_to_queue`).
5. **Dispatch** — `dispatch_sequential` / `dispatch_parallel`; each mutant
   runs the relevant tests (from `src_to_tests`, falling back to *all*
   tests) in a `zero mutate-worker` subprocess.
6. **Report** — terminal summary + `mutation/mutation.json`
   (schema_version 2, cwd-relative path keys). Exit 1 if any mutant
   survived or errored.

Step 5 is the cost center (`mutants × suite-subset` runs); step 2 is one
full suite run. Everything a sound fingerprint needs is already collected
in step 2: per-test loaded-module sets and the source→tests impact map.

### Why the fingerprint must cover the full closure

A cached verdict for `src/foo.ts` can be invalidated by changes to:

1. `foo.ts` itself (different sites, different behavior),
2. any test file that exercises it (a new assertion can kill a previous
   survivor), or
3. any *other* module those tests load (`foo.ts` calls into `bar.ts`; a
   behavior change in `bar.ts` can flip a verdict either way).

So the fingerprint for `foo.ts` is a hash over the contents of `foo.ts`
**plus** the sorted set of `(path, content-hash)` pairs for every test
file exercising it and every module those tests load (the test's full
`loaded` closure). Decided in scoping; the cheaper "source + its tests
only" and "source only" variants were rejected as unsound — a stale score
defeats the tool's purpose (the same vacuous-score concern that drove
`issues/mutate-reachability`).

**Crucially, fingerprints are validated against the *current* baseline's
closure data, not the recorded one.** On a run where anything changed, the
baseline runs first (as today), producing fresh `src_to_tests` and
loaded-sets; each file's fresh fingerprint is computed from those and
compared to the stored one. This makes closure-*membership* changes (a
brand-new test file that starts exercising `foo.ts`; a test that stops
loading it) invalidate correctly — the new closure hashes differently even
though every previously-known file is unchanged.

### The all-unchanged fast path

Before running the baseline, the command checks whether the project is
byte-identical to the cached universe: the discovered test-file set equals
the cached one, the in-scope source-file set equals the cached one, and
every file in the recorded universe (sources, tests, and every loaded
module) hashes to its cached value. If so, the previous baseline verdict
stands deterministically — no input changed — so the run skips the
baseline, replays all cached verdicts, prints the summary, rewrites
`mutation.json`, and exits with the same semantics as a fresh run. If
*anything* differs, the baseline runs as today (it is needed for fresh
coverage and impact data anyway) and reuse is decided per file.

### Decisions made in scoping

- **Fingerprint scope:** full closure (above).
- **Cache location:** a new `mutation/cache.json`, separate from the
  report. `mutation.json` keeps its documented v2 schema and stays a pure
  report; the cache uses **root-relative** path keys (the report's
  cwd-relative keys vary with invocation directory) and its own versioned
  schema. `mutation/` is already gitignored in scaffolded projects
  (`TPL_GITIGNORE` in `crates/zero-scaffold/src/lib.rs`), so the cache is
  naturally local-only. A missing, corrupt, or version-mismatched cache is
  silently treated as absent (full run, then rewritten).
- **CLI posture:** incremental is the **default**; a new `--no-cache`
  flag ignores any existing cache, runs everything fresh, and rewrites the
  cache from the results.
- **Partial runs:** `--operators` and `--max-mutants` are experimentation
  modes — they neither read nor write the cache (a narrowed run must not
  record itself as a file's complete verdicts). The `[pattern]` target
  participates normally: it reuses/refreshes entries for the files it
  covers and leaves other entries untouched. A cache entry therefore
  always means "full operator-set verdicts for this file."
- **Baseline skip:** only on the all-unchanged fast path (above);
  otherwise the baseline always runs and remains the red-suite gate.

### Hashing

No new dependencies (consistent with the prior mutate specs). Two
in-tree options: `std::hash::DefaultHasher` (SipHash — stability across
Rust releases is irrelevant because the cache records the CLI version and
one installed binary is one toolchain), or `sha2`, already in the
dependency tree transitively. The plan picks one; content hashes are over
raw file bytes.

### Code map

- `crates/zero/src/cmd/mutate.rs` — cache read/validate/write, the
  all-unchanged fast path, reuse accounting in `MutationSummary`, summary
  rendering, `--no-cache` plumbing. `run_baseline` additionally needs to
  surface the per-test loaded sets (it already iterates `outcome.loaded`;
  today it only folds them into `src_to_tests`).
- `crates/zero/src/main.rs` — `--no-cache` flag on the `mutate`
  subcommand.
- `crates/zero-test-runner/src/mutate.rs` — likely untouched (generation
  and operators are unchanged); `MutationSite` may need a stable
  serialized identity for cache entries (line/column/operator/original/
  replacement already serialize into `mutation.json`).
- `docs/config-and-cli.md` — `zero mutate` section.
- `docs/testing.md`, `docs/agentic-coding.md` — wherever they describe
  mutate cost/workflow.

### Prior, adjacent work

`issues/mutate-operators`, `issues/mutate-equivalence`, and
`issues/mutate-reachability` established the accounting model
(`matched / unreachable / equivalent-* / killed / survived / errored`),
the per-operator breakdown, and `mutation.json` schema v2. This item does
not change which mutants exist or how verdicts are decided — only whether
a verdict is recomputed or replayed.

## Requirements

### R1 — Cache file and schema

`zero mutate` writes `mutation/cache.json` after every cache-eligible run
(no `--operators`, no `--max-mutants`). Contents, at minimum:

- a cache schema version and the CLI version that wrote it (mismatch on
  either ⇒ cache treated as absent);
- the discovered test-file set and in-scope source-file set (for the
  fast-path universe check), with content hashes;
- per source file (root-relative key): its fingerprint (per Background)
  and its full list of site verdicts — enough to replay `line`, `column`,
  `operator`, `original`, `replacement`, and killed/survived/errored
  status, plus the per-file skip counts (`unreachable`,
  `equivalent_byte`, `equivalent_static`) and per-operator tallies needed
  to reconstruct the file's contribution to the summary.

Writes are atomic-ish (write temp, rename). A read that fails to parse,
fails validation, or has a version mismatch is treated as no-cache — never
an error, never a partial reuse from a corrupt file.

### R2 — Per-file reuse on a normal run

On a cache-eligible run where something changed: run the baseline as
today; compute each in-scope file's fresh fingerprint from the *current*
closure data; for files whose fingerprint matches the cached entry, skip
generation and dispatch entirely and fold the cached verdicts and skip
counts into the summary; for the rest, run the full pipeline as today.
The final summary's totals, per-operator breakdown, survived-mutants
list, mutation score, and exit code must be **identical** to what a full
run would produce, assuming the cached verdicts are honest (R6 tests
this). Cached survived/errored mutants still cause exit 1.

### R3 — All-unchanged fast path

When the universe check passes (Background), skip the baseline, replay
every cached entry, and produce the same terminal summary, refreshed
`mutation.json`, and exit code as the previous run. The fast path must
never fire if the test-file set, source-file set, or any recorded file
hash differs, or if the cache is absent/invalid/partial (e.g. written by
a `[pattern]` run that did not cover every file).

### R4 — CLI surface and narrowed runs

- New flag `--no-cache`: ignore any existing cache entirely, run
  everything fresh, rewrite the cache from the results.
- `--operators` / `--max-mutants` runs neither read nor write the cache;
  the existing cache file is left untouched.
- `[pattern]` runs read and refresh entries only for the files the
  pattern selects; other entries are preserved as-is. Such a run updates
  the per-file entries but must leave the cache marked as not covering
  the full universe unless it actually does (so R3 stays sound).

### R5 — Reporting

- The terminal summary distinguishes fresh from reused work, e.g.
  `Generated: 12 mutants across 3 files (41 reused from cache across 9
  files)` — exact wording per plan, but the reused count must be visible
  in both normal and `--quiet` output.
- Per-mutant progress lines are printed only for freshly executed
  mutants; reused files get at most a one-line note in non-quiet mode.
- The fast path prints an explicit marker that the entire result was
  replayed from cache (so a user is never confused about whether tests
  ran).
- `mutation/mutation.json` remains complete (cached + fresh verdicts
  merged) and remains schema_version 2 with no field changes; whether a
  reused-count field can be added additively is an Open Question — the
  default is terminal-only.

### R6 — Tests

End-to-end tests in `cmd/mutate.rs` (tempdir projects, `Isolation::InProcess`,
same style as the existing tests):

1. **Cold → warm equivalence:** run twice with no edits; second run hits
   the fast path (no baseline, observable via summary marker), and its
   summary totals/score/exit semantics equal the first run's.
2. **Targeted invalidation — source:** edit one of two source files;
   only the edited file's mutants re-execute, the other's verdicts are
   reused, totals match a from-scratch run of the same tree.
3. **Targeted invalidation — test file:** edit only a test that
   exercises file A (e.g. strengthen a weak assertion so a previous
   survivor dies); A re-mutates and the new verdict lands; file B
   (untouched closure) is reused.
4. **Closure-membership change:** add a brand-new test file that
   exercises file A; A's fingerprint misses (fresh-closure validation)
   and A re-mutates; the fast path does not fire.
5. **Dependency change:** A imports helper C (C also covered by A's
   tests through loading); editing C invalidates A's entry.
6. **Narrowed runs:** an `--operators` run neither reads nor writes the
   cache (file mtime/content unchanged); a `[pattern]` run refreshes only
   its files and a later full run reuses both the pattern-refreshed and
   the untouched entries; after a pattern run that covered only part of
   the universe, the fast path does not fire until a full run completes.
7. **`--no-cache`:** with a valid cache present, everything re-executes
   and the cache is rewritten.
8. **Corrupt/stale cache:** garbage JSON and a wrong-version cache both
   degrade silently to a full run.
9. **Red baseline:** a failing suite still refuses to mutate and must
   not overwrite the cache with partial state.

### R7 — Docs

This changes user-facing CLI behavior, so docs are required:

- `docs/config-and-cli.md` — `zero mutate` section: document incremental
  behavior as the default, the `--no-cache` flag row in the flags table,
  `mutation/cache.json` (what it is, that it's safe to delete, that
  `--operators`/`--max-mutants` bypass it), and the all-unchanged fast
  path's summary marker.
- `docs/testing.md` — align any description of mutate's cost/workflow
  with incremental behavior.
- `docs/agentic-coding.md` — if it advises when to run `zero mutate`,
  note that repeat runs are now cheap (this strengthens the
  verify-often rhythm; keep it to a sentence or two).

## Constraints

- **No new dependencies** (Rust or JS). Hashing via std or an
  already-in-tree crate.
- **No `mutation.json` schema change** (still v2, same accounting model);
  the cache is a separate file with its own version.
- **Soundness over hit rate:** any doubt resolves to re-execution. The
  cache must never serve a verdict whose closure cannot be proven
  unchanged. Equal-summary-to-full-run (R2) is the invariant every reuse
  path must preserve.
- **Silent degradation:** cache problems (missing, corrupt, version skew,
  unreadable files during hashing) fall back to full runs without
  warnings-as-errors or exit-code changes.
- **CLI-version keying:** a different `zero` binary version invalidates
  the whole cache (operator sets and generation logic may differ between
  releases).
- **80-line function guideline** (CLAUDE.md): cache load/validate/save
  and the fast-path check are separate helpers, not inlined into
  `run_inner`.
- The slow integration tests (`#[ignore = "slow"]`) must pass under
  `--include-ignored`; any that invoke `zero mutate` twice in one tempdir
  must still behave (they will now hit the cache — assert accordingly or
  pass `--no-cache`).
- `--threads` parallel dispatch is unchanged; cache writes happen once,
  after `consume_mutant_results`, on the main thread.

## Out of Scope

- **Watch mode / daemon** (`zero mutate --watch`) — separate item if ever.
- **Cross-machine or CI cache sharing** — the cache is a local artifact;
  path and version keying assume one machine, one binary.
- **Incremental *baseline*** (running only affected tests in the baseline
  when some files changed) — the baseline stays all-or-nothing: full run
  when anything changed, skipped only when nothing did.
- **Per-operator cache granularity** — rejected in scoping; `--operators`
  runs simply bypass.
- **New operators, equivalence detection, reachability, or scoring
  changes** — owned by the prior mutate items.
- **Caching for `zero test` / coverage runs** — this item is mutate-only;
  test-runner speed is `issues/test-parallel`.

## Open Questions

- **Loaded-set plumbing.** `run_baseline` currently folds `outcome.loaded`
  into `src_to_tests` and drops the per-test detail. The plan should
  confirm `outcome.loaded` reliably contains the *full* module closure per
  test (including non-`src/` helpers and the test file itself) and decide
  the cheapest way to retain per-test loaded sets for fingerprinting.
- **Hash choice.** `DefaultHasher` vs in-tree `sha2`; pick whichever keeps
  the code simplest given the CLI-version keying already bounds stability
  requirements.
- **Partial-universe marker (R4/R3 interplay).** Exact mechanism for "a
  pattern run refreshed entries but the cache does not cover the full
  universe" — e.g. a `complete: bool` written only by full runs, or
  storing the universe snapshot only on full runs. Plan decides.
- **Reused-count in `mutation.json`.** Whether an additive
  `totals.reused` field counts as a schema change requiring a version
  bump, or is acceptable under v2. Default: leave the JSON untouched and
  surface reuse in the terminal only.
- **Covered-lines drift on reused files.** A reused file skips
  generation, so its `unreachable` counts replay from the cache. Confirm
  replayed skip counts plus fresh per-operator `matched` accounting
  compose into a coherent per-operator block (the cached entry must store
  enough per-operator data to reconstruct its rows exactly).
- **Mutant nondeterminism.** If a test suite is flaky, cached verdicts
  freeze one observed outcome. Decide whether docs should mention that
  `--no-cache` is the remedy (probably one sentence in
  `config-and-cli.md`).
