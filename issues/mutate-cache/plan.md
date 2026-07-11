# Plan: `zero mutate` — incremental runs via fingerprint cache

## Summary

Add a fingerprint cache (`mutation/cache.json`) so `zero mutate` reuses
per-file mutant verdicts when a source file's full closure — the file, the
tests that exercise it, and every module those tests load — is byte-identical
to the previous run, and skips the baseline entirely when *nothing* changed.
The work lands in a new `crates/zero/src/cmd/mutate_cache.rs` module (schema,
hashing, fingerprints, load/save) plus surgical changes to `cmd/mutate.rs`
(per-test loaded-set plumbing, per-file accounting, a `CacheMode` parameter on
`run_inner`, reuse/fold/fast-path logic) and a `--no-cache` flag in `main.rs`.
Hashing uses `sha2::Sha256` — already a direct dependency (`cmd/build.rs`).
`mutation.json` stays schema v2, untouched.

## Prerequisites

None. The spec's open questions are resolved by this plan:

- **Loaded-set plumbing** — `RunOutcome.loaded` (harness.rs:22-29) is already
  per-test and lists every canonical path the loader resolved. Two caveats
  found in orientation, handled in Step 1: it contains the test file's
  *parent directory* (harness.rs:189 registers `file_abs.parent()`), and the
  test file itself is absent. The closure builder adds the test file
  explicitly and filters to `is_file()`. Pseudo-modules (`zero`, `zero/test`,
  `zero/http`) never enter `path_map`, so they can't pollute the closure;
  framework-runtime changes are covered by CLI-version keying.
- **Hash choice** — `sha2::Sha256`, already a direct dep of the `zero` crate.
  Deterministic across toolchains; cost is trivial at project scale.
- **Partial-universe marker** — no flag needed. Entries are only written when
  fresh or re-validated, so "cache is complete" is derivable as
  `entries ⊇ src_files`; the fast path requires exactly that.
- **Reused-count in `mutation.json`** — terminal-only (the spec's default).
  `MutationSummary.generated` keeps counting all verdicts (fresh + reused) so
  the JSON totals stay byte-for-byte what a full run produces.

## Steps

- [x] **Step 1: Plumb per-test loaded sets out of `run_baseline`**
- [x] **Step 2: New `mutate_cache` module — schema, hashing, fingerprints, load/save**
- [x] **Step 3: Per-file accounting in the mutate pipeline**
- [x] **Step 4: `CacheMode` on `run_inner` + cache write path**
- [x] **Step 5: Per-file reuse (read path) + reused-count reporting**
- [x] **Step 6: All-unchanged fast path (baseline skip + replay)**
- [x] **Step 7: CLI `--no-cache` flag**
- [x] **Step 8: Docs — config-and-cli, testing, agentic-coding**
- [x] **Step 9: Full verification sweep**

---

## Step Details

### Step 1: Plumb per-test loaded sets out of `run_baseline`

**Goal:** Fingerprints need, per test file, the set of files that test loads.
`run_baseline` already iterates `outcome.loaded` per test but only folds it
into `src_to_tests`, dropping the per-test detail. Retain it. Everything
downstream (Steps 2, 4, 5, 6) consumes this map.

**Files:** `crates/zero/src/cmd/mutate.rs`

**Changes:**

- `BaselineRun` gains a field:
  ```rust
  /// For each test file, every existing file it loaded (the test file
  /// itself included; directories and pseudo-modules excluded).
  test_loaded: HashMap<PathBuf, Vec<PathBuf>>,
  ```
- In `run_baseline`'s per-test loop, build the entry from `outcome.loaded`:
  filter `p.is_file()` (drops the parent-directory entry the harness
  registers at harness.rs:189), push the test file's own path, sort, dedup.
  Note: do **not** filter by `scope.covers` — out-of-scope helpers (e.g.
  `.zero/components/*`, test-utility modules) are part of the behavioral
  closure and must be hashed.
- Keep the existing `src_to_tests` logic unchanged.

**Tests:** In the existing `cmd/mutate.rs` test module (it can call the
private `run_baseline`): scaffold a tempdir project with `src/a.ts`
importing `src/helper.ts`, plus `a.test.ts` importing `src/a.ts`. Assert
`test_loaded[a.test.ts]` contains `a.test.ts`, `src/a.ts`, and
`src/helper.ts`, and contains no directories.

### Step 2: New `mutate_cache` module — schema, hashing, fingerprints, load/save

**Goal:** Self-contained cache data model and IO, unit-testable before any
pipeline wiring. (`lib.rs` has `pub mod cmd`, so `pub` items in a new
`pub mod mutate_cache` compile warning-free before consumers exist.)

**Files:** `crates/zero/src/cmd/mutate_cache.rs` (new),
`crates/zero/src/cmd/mod.rs` (add `pub mod mutate_cache;`)

**Changes:**

Types (all `pub`):

```rust
pub const CACHE_SCHEMA_VERSION: u64 = 1;

/// How `run_inner` interacts with `mutation/cache.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheMode {
    /// Read (reuse + fast path) and write. Default `zero mutate`.
    ReadWrite,
    /// Ignore any existing cache, run everything, rewrite. `--no-cache`.
    Fresh,
    /// Neither read nor write. `--operators` / `--max-mutants` runs.
    Bypass,
}

/// In-memory form of `mutation/cache.json`. All path keys root-relative,
/// `/`-separated, sorted.
pub struct MutateCache {
    pub test_files: Vec<String>,
    pub src_files: Vec<String>,
    /// Every file in any entry's closure (∪ tests ∪ srcs) → sha256 hex.
    pub files: BTreeMap<String, String>,
    pub entries: BTreeMap<String, CacheEntry>,
}

pub struct CacheEntry {
    pub fingerprint: String,
    /// Executed verdicts, in recorded order (includes apply-errored sites).
    pub sites: Vec<CachedSite>,
    /// This file's contribution to the per-operator summary.
    pub per_operator: PerOperatorSummary,   // imported from cmd::mutate
}

pub struct CachedSite {
    pub line: u32,
    pub column: u32,
    pub operator: String,    // Operator::id()
    pub original: String,
    pub replacement: String,
    pub status: String,      // "killed" | "survived" | "errored"
}
```

Functions:

- `pub fn hash_file(path: &Path, memo: &mut HashMap<PathBuf, Option<String>>) -> Option<String>`
  — `Sha256::digest` over raw bytes, lowercase hex, memoized per run.
  Unreadable file ⇒ `None` (callers treat as fingerprint miss).
- `pub fn rel_key(root: &Path, p: &Path) -> String` — `strip_prefix(root)`
  (root canonicalized once by the caller), `\` → `/`; falls back to the
  lossy absolute string if the strip fails.
- `pub fn closure_for(src: &Path, src_to_tests: &HashMap<PathBuf, Vec<PathBuf>>, test_loaded: &HashMap<PathBuf, Vec<PathBuf>>, all_tests: &[PathBuf]) -> BTreeSet<PathBuf>`
  — relevant tests are `src_to_tests.get(src)` **or the all-tests fallback,
  mirroring dispatch** (`dispatch_*`'s `unwrap_or(test_files)`); closure =
  `{src}` ∪ relevant tests ∪ their `test_loaded` sets.
- `pub fn fingerprint(root: &Path, closure: &BTreeSet<PathBuf>, memo: &mut …) -> Option<String>`
  — Sha256 over the sorted `rel_key\0hash\n` concatenation; `None` if any
  member fails to hash.
- `pub fn load(root: &Path, cli_version: &str) -> Option<MutateCache>` —
  reads `root/mutation/cache.json`; any parse error, missing field,
  `schema_version != CACHE_SCHEMA_VERSION`, or `cli_version` mismatch ⇒
  `None`. Never errors, never logs.
- `pub fn save(root: &Path, cli_version: &str, cache: &MutateCache) -> std::io::Result<()>`
  — serialize (top level: `schema_version`, `cli_version`, `test_files`,
  `src_files`, `files`, `entries`; entry `per_operator` keyed by operator id
  with the seven count fields), write `mutation/cache.json.tmp`, rename over
  `mutation/cache.json`. `create_dir_all` first.

Serialization uses `serde_json::json!` + manual `Value` extraction, matching
`write_mutation_json` / `merge_covered_lines` style — no serde-derive dep.
CLI version constant: `env!("CARGO_PKG_VERSION")`, passed in (not read
inside) so tests can simulate version skew.

**Tests:** Unit tests in the module: save→load roundtrip preserves all
fields; corrupt JSON ⇒ `None`; wrong `schema_version` ⇒ `None`; wrong
`cli_version` ⇒ `None`; `hash_file` deterministic and memo-hit counted via
map size; `fingerprint` changes when any closure member's content changes
and when closure membership changes; `closure_for` uses the all-tests
fallback when `src_to_tests` lacks the file.

### Step 3: Per-file accounting in the mutate pipeline

**Goal:** Cache entries need each file's *own* per-operator tallies; today
generation, pre-apply, and consume accumulate only global arrays. Track
per-file without changing any observable totals.

**Files:** `crates/zero/src/cmd/mutate.rs`

**Changes:**

- `MutationSummary` gains:
  ```rust
  /// Per-source-file operator tallies (same arrays as `per_operator`,
  /// restricted to one file). Feeds cache entries.
  pub per_file_operator: BTreeMap<PathBuf, PerOperatorSummary>,
  ```
- Accumulate at the three existing points, alongside the global arrays:
  - `generate_all_sites` — copy `result.per_operator`'s `matched`,
    `unreachable`, `equivalent_static` arrays into the file's entry. Insert
    an (all-zero) entry for **every** walked file, even with zero matches —
    an entry must exist so the file can replay as a zero-contribution
    member and count toward fast-path completeness.
  - `pre_apply_to_queue` — `equivalent_byte`, and the apply-error `errored`
    increment, keyed by `src_path`.
  - `consume_mutant_results` — `killed` / `survived` / `errored` per
    `src_path`.
- Keep every function under the ~80-line guideline: if `generate_all_sites`
  grows past it, extract a `fold_generate_result(summary, src, &result)`
  helper.

**Tests:** New unit test: run `run_inner` on a two-file project (one tested
file, one unreached file) and assert (a) summing `per_file_operator` values
over files reproduces `per_operator` exactly (all seven arrays), and (b)
the unreached file has an entry with `matched > 0`. All existing tests
still pass unchanged.

### Step 4: `CacheMode` on `run_inner` + cache write path

**Goal:** Thread the mode through, and write a correct cache after every
eligible run — write before read so Step 5's reuse tests have real caches
to read.

**Files:** `crates/zero/src/cmd/mutate.rs`

**Changes:**

- `run_inner` gains a `cache_mode: CacheMode` parameter (after `threads`).
  Update all ~15 existing test call sites to pass `CacheMode::Bypass`
  (exact behavior preservation); `run()` passes a computed mode (Step 7).
- Canonicalize root once at the top: `let canon_root =
  root.canonicalize().unwrap_or_else(|_| root.to_path_buf());` — loader
  paths are canonical, `rel_key` must agree.
- Retain the unfiltered walk: `let all_src = walk_src(&scope);` then
  `let src_files = filter_src_files(all_src.clone(), …)`.
- After `consume_mutant_results`, when `cache_mode != Bypass` **and**
  `baseline.passed`, call a new `build_cache(...) -> Option<MutateCache>`
  helper and `mutate_cache::save` (ignore the io::Result with `let _ =` —
  silent degradation). `build_cache`:
  - one shared `memo: HashMap<PathBuf, Option<String>>` for the whole pass;
  - for each `f` in `all_src`: compute `closure_for` + `fingerprint`;
    - if `f` was selected this run (in `src_files`): build a fresh
      `CacheEntry` — fingerprint, `sites` from `summary.outcomes` (empty
      vec if absent), `per_operator` from `summary.per_file_operator`;
    - else (`ReadWrite` only): retain the old cache's entry iff its
      `fingerprint` equals the fresh one; under `Fresh`, never retain;
    - fingerprint `None` (unreadable member) ⇒ no entry;
  - `files` = every (rel, hash) pair from every computed closure, plus all
    test files and all `all_src` files;
  - `test_files` / `src_files` = sorted rel lists of the discovered tests
    and `all_src`.
- The early `!baseline.passed` return stays **before** any cache write
  (R9): a red run never touches the file.

**Tests:** End-to-end in the existing test module (tempdir,
`Isolation::InProcess`, `threads: 1`, style of `make_project`):

1. Cold `ReadWrite` run on a green two-file project writes
   `mutation/cache.json`; parse it raw with `serde_json` and assert:
   `schema_version == 1`, both src files have entries, the tested file's
   entry has nonzero `sites`, the universe `files` map contains the test
   file and the helper, and every hash is 64 lowercase hex chars.
2. `Bypass` run (the mode all existing tests use) creates no cache file;
   with a pre-seeded cache file present, a `Bypass` run leaves its bytes
   unchanged.
3. Red baseline (failing test) with `ReadWrite`: no cache file created;
   with a pre-existing cache, bytes unchanged.
4. `[pattern]` run (target = one file) with `ReadWrite` after editing the
   *other* (non-selected) file: the non-selected file's stale entry is
   dropped, the selected file's entry is fresh.

### Step 5: Per-file reuse (read path) + reused-count reporting

**Goal:** The core R2 behavior — fold cached verdicts for fingerprint-matched
files, run the pipeline only for the rest, and surface reuse in the summary.

**Files:** `crates/zero/src/cmd/mutate.rs`

**Changes:**

- `MutationSummary` gains `pub reused_mutants: usize`,
  `pub reused_files: usize`, `pub baseline_skipped: bool` (the last consumed
  in Step 6; add now so the struct changes once).
- In `run_inner`, `ReadWrite` only, after the baseline and `filter_src_files`:
  load the cache (skip if `None`), then partition via a helper:
  ```rust
  /// Split the selected files into (reused, to_run). A file is reused
  /// iff the cache has an entry whose fingerprint matches the one
  /// computed from THIS run's baseline closures.
  fn partition_reusable(
      src_files: Vec<PathBuf>, cache: &MutateCache, canon_root: &Path,
      baseline: &BaselineRun, all_tests: &[PathBuf],
      memo: &mut HashMap<PathBuf, Option<String>>,
  ) -> (Vec<(PathBuf, CacheEntry)>, Vec<PathBuf>)
  ```
  Pass the same `memo` on to `build_cache` so files hash once per run.
- `fold_cached_entry(summary: &mut MutationSummary, src: &Path, entry: &CacheEntry)`:
  - add the entry's `per_operator` arrays into the global
    `summary.per_operator` and into `summary.per_file_operator[src]`;
  - `skipped_unreachable` / `skipped_equivalent_byte` /
    `skipped_equivalent_static` += the entry's array sums;
  - for each `CachedSite`: reconstruct
    `MutationSite { file: src.into(), operator: Operator::parse(&s.operator)?, line, column, original, replacement }`
    and the `MutantStatus` from `status`, push into `summary.outcomes[src]`,
    increment `generated` and the matching killed/survived/errored total
    (so score, exit semantics, and `mutation.json` equal a full run's);
    a site that fails to parse (operator/status string) invalidates the
    whole entry — treat the file as a fingerprint miss instead (soundness
    over hit rate);
  - `reused_mutants += sites.len()`, `reused_files += 1`.
- `generate_all_sites` + dispatch then run only over the non-reused
  remainder. Reused files must not re-enter `per_file_operator`
  accumulation (fold already populated them).
- `build_cache` (Step 4) treats reused-selected files as "has a valid
  fresh-fingerprint entry" — the folded entry is re-emitted as-is.
- `write_terminal_summary`: the `Generated:` line becomes, when
  `summary.reused_mutants > 0`:
  `Generated: {generated} mutants across {files} files ({reused_mutants} reused from cache across {reused_files} files)`
  — printed in quiet mode too (it's part of the summary block, R5).

**Tests:** End-to-end (each scenario builds the project, runs `ReadWrite`
twice with an edit between, asserts via summary fields — `reused_mutants`,
`reused_files`, plus killed/survived totals — and via the non-quiet `sink`
when per-mutant lines matter):

1. **Source edit** (R6-2): files A and B, both tested; edit A between runs
   (change a literal so verdicts still resolve); run 2 reuses B
   (`reused_files == 1`) and re-executes A's mutants; run 2's totals equal
   a from-scratch run on the edited tree (run a third, `Fresh`, and
   compare killed/survived/errored/score).
2. **Test edit kills a survivor** (R6-3): A has a weak test (calls, no
   assertion) ⇒ run 1 has survivors; strengthen the assertion; run 2
   re-executes A (fingerprint miss via the test file) and the survivor
   count drops; untouched B is reused.
3. **Dependency edit** (R6-5): A imports helper C (C in `src/`, loaded by
   A's test); edit C only; run 2 re-executes both A and C (both closures
   contain C); an unrelated B is reused.
4. **`--no-cache`** (R6-7): valid cache present, `Fresh` mode ⇒
   `reused_mutants == 0`, everything re-executes, cache rewritten (file
   mtime/content changes).
5. **Corrupt + version-skew cache** (R6-8): overwrite `cache.json` with
   garbage ⇒ run 2 is a full run (`reused_mutants == 0`), succeeds, and
   rewrites a valid cache. Same with a wrong `cli_version` (write a valid
   file via `save` with a doctored version string).
6. **Summary line**: capture `write_terminal_summary` output on a
   reuse run; assert the parenthetical appears with `quiet == true`.

### Step 6: All-unchanged fast path (baseline skip + replay)

**Goal:** R3 — when the universe is byte-identical and entries cover every
src file, skip the baseline, replay everything, mark it visibly.

**Files:** `crates/zero/src/cmd/mutate.rs`

**Changes:**

- New helper, called in `run_inner` before `run_baseline`, only when
  `cache_mode == ReadWrite && target.is_none()`:
  ```rust
  /// True iff the cached universe matches the discovered one exactly:
  /// same rel test set, same rel src set, every recorded file rehashes
  /// to its cached value, and every src file has an entry.
  fn fast_path_applies(
      canon_root: &Path, cache: &MutateCache,
      test_files: &[PathBuf], all_src: &[PathBuf],
      memo: &mut HashMap<PathBuf, Option<String>>,
  ) -> bool
  ```
  (Requires hoisting `walk_src` above the baseline — it has no dependency
  on it. A `files` entry that fails to hash, e.g. deleted helper, ⇒ false.)
- On hit: write the marker line to `progress` **unconditionally** (quiet
  included):
  `zero mutate: no changes since last run — replaying cached result (baseline skipped)`
  then build the summary by `fold_cached_entry` over every entry, set
  `baseline_passed = true`, `baseline_skipped = true`, return before the
  baseline. Do **not** rewrite the cache on this path (nothing changed).
- `run()` needs no change: it already prints the summary, writes
  `mutation.json` (R3's "refreshed report"), and derives the exit code
  from survived/errored — all correct for a replayed summary.

**Tests:**

1. **Cold→warm equivalence** (R6-1): run twice, no edits; run 2 has
   `baseline_skipped == true`, `reused_mutants == run1.generated`, and
   identical generated/killed/survived/errored/skipped_*/score; the sink
   contains the marker line; `mutation.json` after run 2 parses and its
   totals equal run 1's.
2. **Any edit declines**: touch one source file's content ⇒
   `baseline_skipped == false` (and partial reuse from Step 5 kicks in).
3. **Test-set change declines** (R6-4 closure-membership): add a new test
   file exercising A ⇒ fast path declines (test set differs), A
   re-executes (fresh closure includes the new test), B reused.
4. **Partial cache declines**: edit A and B, run with `target` = A
   (B's entry dropped per Step 4 test 4), revert nothing; next full run:
   fast path declines (B has no entry), B re-executes fresh, A reused;
   the run *after that* hits the fast path again — proving a
   pattern-refreshed cache converges back to complete.
5. **Targeted runs never fast-path**: unchanged universe + `target`
   set ⇒ `baseline_skipped == false` even though everything is reused.

### Step 7: CLI `--no-cache` flag

**Goal:** Expose the modes on the real CLI surface.

**Files:** `crates/zero/src/main.rs`, `crates/zero/src/cmd/mutate.rs`

**Changes:**

- `Commands::Mutate` gains `#[arg(long)] no_cache: bool` (renders as
  `--no-cache`); pass through to `cmd::mutate::run`.
- `run(target, operators, max_mutants, quiet, threads, no_cache)` computes:
  ```rust
  let cache_mode = if operators.is_some() || max_mutants.is_some() {
      CacheMode::Bypass            // narrowed runs: no read, no write
  } else if no_cache {
      CacheMode::Fresh             // ignore, re-run, rewrite
  } else {
      CacheMode::ReadWrite
  };
  ```
  and passes it to `run_inner`. (`target` does *not* force a mode —
  pattern runs participate per-file; Step 6 already excludes them from
  the fast path.)

**Tests:** Extend the existing clap-parse test block in `main.rs`
(`parsed_threads` style): `--no-cache` parses to `true`, default is
`false`, and `--no-cache --threads 2` compose. Unit test for the mode
computation if extracted as a helper (extract
`fn cache_mode_for(filter_set: bool, max_set: bool, no_cache: bool)` so
it's testable without async plumbing).

### Step 8: Docs — config-and-cli, testing, agentic-coding

**Goal:** R7 — the change is user-facing (default behavior + new flag);
docs are part of done.

**Files:** `docs/config-and-cli.md`, `docs/testing.md`,
`docs/agentic-coding.md`

**Changes:**

- `docs/config-and-cli.md`, `zero mutate` section:
  - flags table: add `--no-cache` row — "Ignore the incremental cache:
    re-run every mutant and rewrite `mutation/cache.json`."
  - new subsection `#### Incremental runs` (between the flags table and
    "Reading `Generated: 0`"): runs are incremental by default; a file's
    verdicts are reused when the file, the tests that exercise it, and
    every module those tests load are all byte-identical to the last run
    (any doubt ⇒ re-run); when nothing changed at all the baseline is
    skipped and the run prints the
    `no changes since last run — replaying cached result` marker;
    `mutation/cache.json` is internal, version-keyed, gitignored, and
    always safe to delete; `--operators` / `--max-mutants` runs neither
    read nor write it; `[pattern]` runs refresh only the files they
    cover; one sentence: a flaky suite freezes whichever verdict was
    observed — `--no-cache` re-runs from scratch.
  - note that `mutation.json` is unchanged (still schema v2) and reused
    verdicts are folded in, so its totals always describe the whole tree.
- `docs/testing.md` (~line 45, the `zero mutate` cross-reference): add a
  clause that repeat runs reuse cached verdicts for unchanged files.
- `docs/agentic-coding.md`: in the tool table's `zero mutate` row (line
  34) and/or the closing workflow note (line 244), add one sentence:
  repeat `zero mutate` runs are cheap — unchanged files replay from the
  cache, so running it after every change is affordable.

**Tests:** None (prose). Verify internal anchors/links still resolve.

### Step 9: Full verification sweep

**Goal:** Confirm the workspace is green including the slow integration
tests, and the touched module respects the size guideline.

**Files:** none (verification only; fix-ups as needed)

**Changes / actions:**

- `cargo test --workspace` (fast loop), then
  `cargo test --workspace -- --include-ignored` (CLAUDE.md requires it for
  test-flow changes). Orientation found no integration test under
  `crates/zero/tests/` that invokes `zero mutate`, so no slow-test edits
  are expected — verify that holds.
- `cargo llvm-cov --workspace --summary-only` glance at `cmd/mutate.rs` and
  `cmd/mutate_cache.rs` per CLAUDE.md (signal, not gate).
- Confirm `run_inner` and the new helpers are each under ~80 lines; factor
  further if not.

## Risks and Assumptions

- **`loaded_paths` completeness.** Fingerprint soundness assumes every
  module evaluated during a test passes through `ZeroModuleLoader`
  (`path_map`). True today for static and dynamic imports (the QuickJS
  resolver routes both); if a future loading path bypasses `path_map`,
  closures silently shrink. Step 1's test pins the current contract.
- **Harness parent-dir quirk.** Step 1 filters `is_file()` to drop the
  parent-directory entry registered at harness.rs:189. If the harness
  later registers other non-file keys, the filter still holds; if it
  *stops* registering real loaded files, Step 1's test catches it.
- **Canonicalization.** Loader paths are canonicalized; `root` as passed
  may not be (symlinked tempdirs). `run_inner` canonicalizes root once for
  all `rel_key` calls. If `strip_prefix` still fails for some path, the
  lossy-absolute fallback keeps behavior correct (worst case: machine-
  specific keys that simply never match after a move — a full run, not a
  wrong run).
- **Verdict-order nondeterminism.** Parallel dispatch already makes
  per-file site order nondeterministic; equivalence tests compare counts
  and score, not JSON bytes.
- **CLI-version keying granularity.** Framework devs rebuilding at the
  same `CARGO_PKG_VERSION` after changing operator logic can be served
  stale verdicts; `--no-cache` is the escape. Accepted in the spec.
- **`run_inner` size.** It's the orchestration hot spot and will exceed 80
  lines if the fast path / partition / write logic is inlined — the plan
  names helpers (`fast_path_applies`, `partition_reusable`,
  `fold_cached_entry`, `build_cache`) to keep each unit small.
- **Existing-test churn.** ~15 `run_inner` call sites gain
  `CacheMode::Bypass` mechanically in Step 4; behavior is provably
  unchanged for them (Bypass touches nothing). New behavior is covered
  exclusively by new tests, so no existing assertion needs reinterpreting.
- **`zero.toml` scope changes.** Config changes that move `project.root`
  relocate the cache with the root; changes to `build.out` alter the
  discovered src/test sets when they matter and are caught by the
  universe check. No toml hash is stored; if a pathological case surfaces
  (out-dir change with identical file sets), the verdicts are identical
  anyway, so reuse stays sound.
- **Flaky suites.** Cached verdicts freeze one observed outcome; this is
  inherent to caching and documented (Step 8) rather than mitigated.
