# Spec: `zero mutate` — per-mutant timeout so infinite-loop mutants can't hang the run

## Problem Statement

`zero mutate` has no per-mutant execution deadline. When a mutation turns a
bounded loop into an unbounded one, the mutant runs forever and the whole
`zero mutate` invocation hangs with no output, no progress, and no way to
recover short of killing the process. Mutation testing's entire value is
running *hostile* variants of the code — infinite-loop mutants are not an
edge case, they are an expected product of the operators the tool already
ships (arithmetic, comparison, and update-operator mutations). A tool that
deadlocks on its own normal output is unusable on any codebase that contains
a loop.

Concrete reproduction (external project `../s3dev/web`): `src/lib/format.ts`
exports `humanBytes`, which scales a byte count through units with a `while`
loop:

```ts
let v = n / 1024;
let i = 0;
while (v >= 1024 && i < units.length - 1) {
  v /= 1024;
  i += 1;
}
```

`src/lib/format.test.ts` exercises it end to end (`humanBytes(5 * 1024 ** 3)`
→ `"5.0 GB"`, etc.). Several mutants of this loop never terminate:

- `i += 1` → `i -= 1` (update operator): `i` never reaches
  `units.length - 1`.
- `v /= 1024` → `v *= 1024` (arithmetic): `v` grows without bound, stays
  `>= 1024`.
- `v >= 1024` → `v <= 1024`, or `i < units.length - 1` →
  `i > units.length - 1` (comparison): condition can stay true forever for
  the tested inputs.

Any one of these produces a mutant whose test run never returns. Because
mutant execution has no timeout, `zero mutate` on `format.ts` hangs
indefinitely.

## Background

### Where the hang happens (confirmed in the current code)

The CLI entry point `cmd::mutate::run` (`crates/zero/src/cmd/mutate.rs:1450`)
hard-codes `Isolation::Subprocess`. Dispatch (`dispatch_sequential` /
`dispatch_parallel`) hands each mutant to `run_one_mutant_subprocess`
(`crates/zero/src/cmd/mutate.rs:1315`), which shells out to the hidden
`zero mutate-worker` subcommand via:

```rust
let output = Command::new(exe).arg("mutate-worker") … .output();
```

`Command::output()` **blocks until the child exits** — there is no timeout.
The child (`worker_main` → `run_one_mutant_inproc`, line 1388) runs the
mutated module's tests in-process on the QuickJS (rquickjs) engine
(`crates/zero-test-runner/src/harness.rs`, `Runtime::new()` at line 195),
which has **no execution deadline / interrupt handler installed**. So:

1. The child's QuickJS loop spins forever inside `run_file_with_loader`.
2. The parent's `.output()` waits forever on that child.
3. The dispatch thread (and the whole `zero mutate` command) never completes.

The in-process isolation mode (`Isolation::InProcess`,
`run_one_mutant_inproc` called directly) has the *same* exposure and is used
throughout the unit tests, so a timeout-triggering fixture would hang the
Rust test suite too if run in-process without a guard.

### Current mutant accounting (must be respected)

`MutantStatus` (`crates/zero/src/cmd/mutate.rs:33`) has three variants that
double as worker exit codes: `Survived = 0`, `Killed = 1`, `Errored = 2`.
The subprocess parent maps `status.code()`:
`Some(0) → Survived`, `Some(1) → Killed`, everything else → `Errored`.
`MutationSummary` tracks `killed / survived / errored`;
`score = killed / (killed + survived + errored)`. Verdicts round-trip
through `as_str` / `from_str` for the mutate cache (`mutate_cache.rs`) and
`mutation.json`. The `matched / unreachable / equivalent-byte /
equivalent-static / killed / survived / errored` accounting model and the
`mutation.json` schema (v2) were frozen by `issues/mutate-equivalence` and
`issues/mutate-operators`; this spec must not break that model.

### Chosen behavior (decided during refinement)

- **Classification.** A timed-out mutant counts as **killed** for the
  mutation score (an infinite loop is a real, test-detectable divergence —
  the suite would never pass), but is surfaced as a distinct **`timed-out`
  sub-count** in the terminal breakdown so users can see how many kills were
  timeouts. This is additive: the `killed / survived / errored` buckets and
  the score formula are unchanged; timed-out mutants are a labeled subset of
  killed. (Matches Stryker's convention: timeout ⊂ killed.)

- **Budget.** Derived from the measured baseline suite wall-time:
  `budget = max(floor_ms, baseline_ms * factor)` (e.g. `max(2000, baseline
  * 5)`), overridable with a `--timeout <dur>` flag. Adapts to slow suites
  and slow machines; the flag is the escape hatch.

- **Guard placement (defense-in-depth).**
  - **Primary — engine deadline.** Install a QuickJS execution deadline via
    rquickjs's interrupt handler on the harness `Runtime` so a JS infinite
    loop self-aborts at the budget and the worker reports a precise
    `timed-out` verdict. This protects `Isolation::InProcess` and the unit
    tests, and makes the timeout path testable without relying on OS process
    kills.
  - **Backstop — parent kill.** The subprocess parent spawns the child and
    waits with a timeout; if the child overruns (a wedge outside JS the
    interrupt can't reach — a native shim hang, a GC-teardown deadlock, cf.
    the #63-class teardown aborts already handled as `Errored`), it kills
    the child and records the mutant as timed-out.

### Why both guards

The engine deadline is precise and cheap and covers the reported JS-loop
case completely. But the mutate subsystem already treats the child process
as potentially untrustworthy at teardown (the `mutate-worker` subcommand
exists specifically "to keep engine-internal aborts from killing the
parent," `main.rs:85`). A parent-side hard timeout guarantees the run always
makes progress even if a child wedges for a reason the JS interrupt never
observes. Neither alone is sufficient for both "precise + testable" and
"can't-hang-under-any-circumstance"; together they are.

### Code map (files this touches)

- `crates/zero-test-runner/src/harness.rs` — install an interrupt handler on
  `Runtime::new()` that trips at a deadline; thread the deadline/budget into
  the harness entry (`run_file_with_loader` / its callers). A tripped
  deadline must surface as a distinct, detectable outcome (not an ordinary
  test failure and not a generic load error).
- `crates/zero-test-runner/src/loader.rs` — the second `Runtime::new()`
  (line 423) path, if it can execute user loops, needs the same guard or an
  explicit note that it can't hang.
- `crates/zero/src/cmd/mutate.rs` —
  - `MutantStatus`: add a `TimedOut` variant; `as_str`/`from_str` round-trip
    it (`"timed-out"`); it folds into `killed` for the score and the exit
    code, and is tracked as a distinct sub-count in `MutationSummary`.
  - `run_one_mutant_subprocess`: replace `.output()` with spawn +
    wait-with-timeout + kill; map the new worker exit code and a
    killed-by-signal child onto `TimedOut`.
  - `run_one_mutant_inproc`: honor the engine deadline; return `TimedOut`
    when the harness reports a deadline trip.
  - the baseline path: surface `baseline_ms` (measured wall-time) so the
    budget can be derived; compute the budget once and thread it into
    dispatch.
  - `write_terminal_summary` / per-operator breakdown: show the `timed-out`
    sub-count.
- `crates/zero/src/main.rs` — add the `--timeout <dur>` arg to the `Mutate`
  command; pass a new timeout code from `mutate-worker` through. Update the
  `worker_main` exit-code mapping.
- `crates/zero-config/src/*` — **only if** a `zero.toml` key is added (see
  Open Questions); otherwise untouched.
- `docs/config-and-cli.md` — document the `--timeout` flag, the
  timeout-→killed classification with the distinct `timed-out` sub-count, and
  note the default budget derivation.

### Reference: prior, adjacent work

`issues/mutate-operators` and `issues/mutate-equivalence` established the
verdict accounting and `mutation.json` schema v2; `issues/mutate-cache`
established verdict caching and replay. This spec is additive to all three:
a new labeled subset of `killed`, no schema-version bump, no change to the
equivalence/unreachable partition.

## Requirements

### R1 — Diagnose-then-fix: failing fixture first

Before adding the guard, add a test (in `cmd/mutate.rs` tests) that builds a
synthetic project mirroring `format.ts`'s `humanBytes` shape: an exported
function with a `while` loop whose exit depends on an update/arithmetic
operator, plus a sibling test that calls it. Assert that **with a mutant that
inverts the loop's progress** (e.g. `i += 1` → `i -= 1`), running the mutant
under a **short** timeout budget yields a `TimedOut` verdict rather than
hanging. The test must complete in bounded wall-time and must fail (hang or
mis-classify) against today's un-guarded code, proving the guard is what
fixes it. This test is also the regression guard.

### R2 — Engine execution deadline (primary guard)

Install an rquickjs interrupt handler on the harness `Runtime` that aborts
execution once a per-mutant deadline elapses. A tripped deadline must be
distinguishable at the harness boundary from (a) a normal test failure and
(b) a module load error, so the mutate layer can map it to `TimedOut`.

- The deadline is off / effectively-infinite for ordinary `zero test` runs;
  it is only armed for mutant execution (baseline and normal test runs must
  be unaffected).
- The handler must be cheap enough not to materially slow mutant execution
  (interrupt callbacks fire frequently; the check must be O(1)).
- JS and TS inputs both work; the guard must not change the verdict of any
  mutant that already terminates.

### R3 — Parent-side wait-with-timeout + kill (backstop)

`run_one_mutant_subprocess` no longer blocks unboundedly. It spawns the
`mutate-worker` child and waits at most `budget` (plus a small grace margin
over the engine deadline, so the child's own self-report wins the common
case). If the child overruns, the parent kills it (and any grandchildren it
owns) and records `TimedOut`.

- A child that exits with the new timeout code → `TimedOut`.
- A child killed by the parent (no exit code / terminating signal) →
  `TimedOut`, **not** `Errored` — the backstop fired precisely because the
  budget was exceeded.
- Genuine engine aborts unrelated to time (exit outside the known code set,
  child died before the budget) continue to map to `Errored` as today.
- Temp files (`{uniq}.js`, `{uniq}.tests`) are still cleaned up on the
  timeout path — no leak when a child is killed.
- Prefer a std-only implementation (spawn + `try_wait` poll loop, or a timer
  thread that kills on expiry). A new crate dependency requires explicit
  justification (see Constraints).

### R4 — Timeout budget derivation and `--timeout` flag

- Default budget: `max(floor_ms, baseline_ms * factor)`, where `baseline_ms`
  is the measured baseline suite wall-time. The plan picks concrete
  `floor_ms` and `factor` defaults (starting points: `floor_ms = 2000`,
  `factor = 5`) and justifies them against the demo + `examples/` baseline
  times so legitimate slow mutants are not false-timed-out.
- `zero mutate --timeout <dur>` overrides the derived budget. Accept a
  human-friendly duration (e.g. `10s`, `500ms`); define and document the
  accepted grammar. The flag composes with `--threads`, `--operators`,
  `--max-mutants`, `--no-cache`, and `--quiet`.
- The budget is computed once per run and threaded into dispatch; it is the
  same for every mutant in the run (not re-derived per mutant).

### R5 — `TimedOut` verdict: accounting, caching, exit code

- Add `MutantStatus::TimedOut`. It contributes to `killed` in the score and
  to the process exit status exactly as `Killed` does (a run with only
  killed/timed-out mutants still exits `0`).
- `MutationSummary` gains a distinct `timed_out` sub-count (a subset of
  `killed`); the terminal summary and per-operator breakdown display it
  (e.g. `Killed: 9 (incl. 2 timed-out)`).
- `as_str`/`from_str` round-trip `"timed-out"` so the mutate cache and any
  replay path (`mutate_cache.rs`) persist and restore the verdict.
- **No `mutation.json` schema-version bump.** If the timed-out count is
  emitted to `mutation.json`, it is an additive, optional field that older
  readers ignore; the frozen v2 buckets and their meanings are unchanged.
  Call this out explicitly so a reader does not expect a schema change.

### R6 — Acceptance

End-to-end tests in `cmd/mutate.rs` (in-memory tempdir, no external deps),
each bounded in wall-time:

1. **Infinite-loop mutant is killed, not hung** (the `humanBytes`
   reproduction from R1): a mutant that makes the loop non-terminating is
   recorded `TimedOut`, folds into `killed`, and the run **completes** with a
   non-vacuous score. Assert the run's wall-time is bounded (proving no
   hang).
2. **Backstop path** (`Isolation::Subprocess`): a child that would overrun is
   killed by the parent and recorded `TimedOut`; temp files are cleaned up;
   the parent survives.
3. **No regression on terminating mutants**: a normal killed/survived mutant
   run produces identical verdicts and score with the guard installed
   (budget generous). The default budget does not false-time-out any mutant
   in the demo / `examples/` mutate runs.
4. **`--timeout` override**: an explicit short `--timeout` forces a
   `TimedOut` on a mutant that would otherwise pass under the derived budget,
   confirming the flag is wired end to end.

### R7 — Docs

`docs/config-and-cli.md`, `zero mutate` section:

- Document `--timeout <dur>`: syntax, default (baseline-derived +
  floor), and when to raise it.
- Explain that a mutant which does not terminate within the budget is
  classified **killed** and reported as a `timed-out` sub-count, and why
  (an infinite loop is a detected divergence, not a survivor).
- Note explicitly that `mutation.json` remains schema v2 (no version bump).

If `docs/testing.md` characterizes mutate verdicts, align it. If a
`zero.toml` key is added (Open Questions), document it in the `zero.toml`
schema section too.

## Constraints

- **Additive to the frozen accounting.** No change to the
  `matched / unreachable / equivalent-byte / equivalent-static / killed /
  survived / errored` model or the `mutation.json` schema version. `TimedOut`
  is a labeled subset of `killed`.
- **Must not hang under any circumstance.** The parent-side backstop is the
  hard guarantee: after the fix, no single mutant can stall the run past
  `budget + grace`, regardless of what the child does.
- **No false timeouts on the happy path.** The default budget must be
  generous enough that no legitimately-terminating mutant in the demo /
  `examples/` is mis-classified. Verify against real baseline times before
  fixing the defaults.
- **`zero test` unaffected.** The engine deadline is armed only for mutant
  execution; ordinary `zero test` and the mutate *baseline* run keep their
  current (un-deadlined) behavior.
- **Prefer std-only.** Implement wait-with-timeout without a new crate if
  practical. Any new dependency (e.g. `wait-timeout`) must be justified in
  the plan against a std alternative; the framework's dependency-frugality
  norm applies.
- **80-line function guideline** (CLAUDE.md): the deadline/interrupt wiring
  and the spawn/wait/kill logic should be factored into helpers rather than
  inlined into the existing large dispatch/worker functions.
- **Behavior-preserving otherwise.** The interrupt handler must not alter the
  result of any mutant that already terminates, and must not perturb
  coverage instrumentation or the equivalence/unreachable classification.
- **Cross-platform kill.** The parent's child-kill path must work on the
  supported platforms (Linux primary); killing must not leave orphaned
  grandchildren or temp files.
- The slow integration tests (`#[ignore = "slow"]`) that exercise mutate
  must pass under `--include-ignored` after the change.

## Out of Scope

- **A general `zero test` per-test timeout.** `zero test` on honest test
  files does not hang (only *mutation* manufactures the infinite loop). A
  user-facing test timeout is a separate feature; this spec arms the deadline
  only for mutant execution. (If the engine-deadline primitive is built
  reusably, a future item may expose it to `zero test` — noted, not required
  here.)
- **New mutation operators or changes to operator selection / equivalence
  detection** (owned by `issues/mutate-operators`, `issues/mutate-equivalence`).
- **Changing the reachability/unreachable classification** (owned by
  `issues/mutate-reachability`).
- **`mutation.json` schema/version changes** or new top-level status buckets.
- **Detecting *which* operator caused the timeout beyond the existing
  per-operator breakdown** — the per-operator `timed-out` sub-count is
  sufficient; no new diagnostic tooling.
- **Static infinite-loop detection** (proving a mutant loops without running
  it) — the timeout is the mechanism; no static analysis.

## Open Questions

- **Exact `floor_ms` and `factor` defaults.** The plan should measure the
  demo + `examples/` baseline wall-times and pick values that leave generous
  headroom over the slowest legitimate mutant while still killing a true
  infinite loop quickly. (Starting points: `floor_ms = 2000`, `factor = 5`.)
- **`zero.toml` config key.** Should the budget also be settable in
  `zero.toml` (e.g. `[mutate] timeout = "10s"`), or is the `--timeout` flag
  sufficient? A flag-only design keeps `zero-config` untouched; a config key
  matches how other per-command knobs are exposed. Recommend flag-only for
  v1 unless a config key is trivial.
- **rquickjs interrupt-handler mechanics (0.12).** Confirm the exact API
  (`Runtime::set_interrupt_handler` or equivalent), that the callback can
  read a shared deadline cheaply (e.g. an `Instant` captured at arm time, or
  an `AtomicBool` flipped by a timer thread), and how an interrupt-aborted
  execution surfaces at the `Ctx::eval` / promise-drive boundary in
  `harness.rs` so it can be mapped to `TimedOut` and not conflated with a
  thrown JS error.
- **Grace margin between engine deadline and parent kill.** How much longer
  than the engine deadline the parent should wait before the hard kill, so
  the child's precise self-report normally wins and the OS kill is a true
  last resort. (A small fixed margin, e.g. +1–2s, is likely enough.)
- **Baseline wall-time plumbing.** Confirm the cleanest place to capture
  `baseline_ms` (the baseline run already executes every test file) and
  thread it to budget computation without disturbing the coverage/reachability
  baseline logic.
- **Interaction with the mutate cache.** A `TimedOut` verdict is cached and
  replayed like any other; confirm a cached `timed-out` replays without
  re-running (and that a later `--timeout` change does not silently replay a
  stale timeout verdict — or document that narrowed/changed-timeout runs
  bypass the cache, consistent with `cache_mode_for`).
