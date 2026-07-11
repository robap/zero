# Plan: `zero mutate` — per-mutant timeout so infinite-loop mutants can't hang the run

## Summary

Give `zero mutate` a per-mutant execution deadline so a mutant that turns a
bounded loop into an unbounded one (the reported `humanBytes` `while` case)
is aborted and scored instead of hanging the whole run. Two guards, built
bottom-up: (1) a **QuickJS engine deadline** installed via rquickjs's
interrupt handler so a JS infinite loop self-aborts and reports a precise
`timed-out` verdict (protects `Isolation::InProcess` and unit tests); and
(2) a **parent-side spawn + wait-with-timeout + kill** backstop on the
subprocess path so a child that wedges for any reason still can't stall the
run. A timed-out mutant counts as **killed** for the score but is tracked as
a distinct `timed-out` sub-count. The budget is derived from the measured
baseline wall-time (`max(floor, baseline_ms × factor)`), overridable with a
new `--timeout` flag. The work is additive to the frozen mutate accounting
and `mutation.json` schema (v2, unchanged).

## Prerequisites

Spec open questions, resolved here so execution does not stall:

- **rquickjs interrupt API** — Step 2 verifies `Runtime::set_interrupt_handler`
  exists in rquickjs 0.12 and its exact bound (likely
  `Option<Box<dyn FnMut() -> bool + 'static>>`; a `Send` bound is satisfiable
  with `Arc<AtomicBool>` + a copied `Instant` deadline). If the API differs,
  Step 2 adapts; the rest of the plan is unaffected.
- **`zero.toml` config key** — Out for v1. Budget is settable only via
  `--timeout`; `zero-config` is untouched. (Revisit as a follow-up if asked.)
- **Default budget constants** — `floor_ms = 2000`, `factor = 5`
  (`budget = max(2000ms, baseline_ms × 5)`). Executor sanity-checks these
  against `examples/` baseline times during verification (no legitimate
  mutant should false-time-out); constants live in one place for easy tuning.
- **Grace margin** — parent hard-wait = `budget + 2s` so the child's engine
  deadline (armed at `budget`) normally self-reports before the OS kill.

No dependency on other issues; no new crate dependency (std-only wait/kill).

## Steps

- [x] **Step 1: Add `MutantStatus::TimedOut` and its accounting (data model only)**
- [x] **Step 2: QuickJS engine execution deadline in the harness (primary guard)**
- [x] **Step 3: In-process mutant path honors the deadline + baseline-derived budget**
- [x] **Step 4: Subprocess backstop — spawn/wait-with-timeout/kill + worker deadline arg**
- [x] **Step 5: `--timeout` CLI flag, end to end**
- [x] **Step 6: Docs**

---

## Step Details

### Step 1: Add `MutantStatus::TimedOut` and its accounting (data model only)

**Goal:** Introduce the verdict and thread it through every tally, cache, and
report site so it folds into `killed` for the score while surfacing as a
distinct `timed-out` sub-count. No producer emits it yet, so the tree stays
green and behavior is unchanged — this isolates the (compiler-guided,
exhaustive-match) accounting change from the runtime guard.

**Files:**
- `crates/zero/src/cmd/mutate.rs`
- `crates/zero/src/cmd/mutate_cache.rs`

**Changes:**
- `MutantStatus` (mutate.rs:33): add `TimedOut`.
  - `as_str` → `"timed-out"`; `parse("timed-out")` → `Some(TimedOut)`.
  - `to_exit_code` → `3` (new IPC code; consumed in Step 4).
- `MutationSummary` (mutate.rs:82): add `pub timed_out: usize` (subset of
  `killed`).
- `PerOperatorSummary` (mutate.rs:115): add `pub timed_out: [usize; 8]`.
  Leave `executed()`/`score()` defined on `killed+survived+errored` — since
  TimedOut also increments `killed`, the score already accounts for it.
- `consume_mutant_results` (mutate.rs:1156, 1174 matches): add a `TimedOut`
  arm that increments `summary.killed` **and** `summary.timed_out`, and
  `per_operator.killed[i]` **and** `per_operator.timed_out[i]` (and the
  per-file arrays likewise). This keeps `killed` a superset of `timed_out`.
- `fold_cached_entry` (mutate.rs:802): in the per-operator fold loop
  (828–837) add `global.timed_out[i] += per_op.timed_out[i]`; in the
  site-status loop (843) add a `TimedOut` arm incrementing
  `summary.killed += 1` and `summary.timed_out += 1`.
- `write_terminal_summary` (mutate.rs:293): render the killed line as
  `Killed:    {n}  ({pct}%)` and, when `summary.timed_out > 0`, append
  `  (incl. {timed_out} timed-out)`. In the per-operator row
  (`write_per_operator_row`, ~386) include the timed-out sub-count when
  non-zero.
- `write_mutation_json` (mutate.rs:420): additive fields only —
  per-operator object gains `"timed_out": per_operator.timed_out[i]`; the
  `totals` object gains `"timed_out": summary.timed_out`. **`schema_version`
  stays `2`.**
- `mutate_cache.rs`: `per_operator_from_json` (193) reads `timed_out`
  tolerantly (`out.timed_out[i] = count("timed_out").unwrap_or(0)`); `save`
  (256–262) emits `"timed_out": e.per_operator.timed_out[i]`. Bump
  `CACHE_SCHEMA_VERSION` so any stale cache is cleanly re-run rather than
  mis-read. (This is the internal mutate cache schema, **not** the
  `mutation.json` schema.)

**Tests:**
- mutate.rs unit: `MutantStatus::parse("timed-out")` round-trips through
  `as_str`; `to_exit_code() == 3`.
- Tally test: a `MutationSummary` with one `TimedOut` outcome reports
  `killed == 1`, `timed_out == 1`, `score() == 1.0`.
- mutate_cache.rs: extend the existing round-trip test so a `"timed-out"`
  site and a non-zero `per_operator.timed_out` survive `save`→`load`.
- Terminal-summary test: output contains `incl. 1 timed-out` when present and
  omits it when zero.

### Step 2: QuickJS engine execution deadline in the harness (primary guard)

**Goal:** Let JS execution self-abort at a deadline and report that fact, so
the in-process path (Step 3) and the worker (Step 4) have a precise,
testable timeout primitive. This is the load-bearing runtime change.

**Files:**
- `crates/zero-test-runner/src/harness.rs`
- `crates/zero-test-runner/src/result.rs`

**Changes:**
- `result.rs`: add `pub timed_out: bool` to `FileResult` (default `false`;
  update every constructor / struct-literal site — `load_error_outcome`,
  the `Ok`/panic paths in `run_with_loader`, `synthesize_panic_outcome`).
- `harness.rs`: thread an `Option<Instant>` deadline through
  `run_with_loader` → `run_with_loader_inner`. New public entry:
  `pub fn run_file_with_loader_deadline(project_root, file_abs, loader, deadline: Option<Instant>) -> FileResult`.
  Keep `run_file_with_loader` as `…_deadline(.., None)` (no behavior change
  for `zero test` / baseline).
- In `run_with_loader_inner`, after `Runtime::new()` (line 195): when
  `deadline` is `Some(d)`, create `let tripped = Arc::new(AtomicBool::new(false));`
  and install
  `rt.set_interrupt_handler(Some(Box::new({ let tripped = tripped.clone(); move || { if Instant::now() >= d { tripped.store(true, Ordering::Relaxed); true } else { false } } })));`.
  (Verify the exact rquickjs 0.12 signature/bound first; `Arc<AtomicBool>` +
  copied `Instant` keeps the closure `Send + 'static` if required.)
- After `context.with(...)` returns, read `tripped`; set
  `result.timed_out = tripped.load(...)`. A tripped deadline typically
  surfaces the aborted execution as an `Err`/exception inside `inner` — the
  `tripped` flag is the source of truth regardless of how it surfaces, so the
  outcome is classified timed-out even if it also produced a load error or a
  synthetic failure. The flag lives outside the `catch_unwind` closure, so a
  teardown panic path still reports it.
- Keep the interrupt check O(1): a single `Instant::now()` compare per
  callback (QuickJS calls the handler periodically, not per-op).

**Tests:**
- test-runner unit test: a temp `.js` file with `while (true) {}` run via
  `run_file_with_loader_deadline(.., Some(now + 200ms))` returns
  `timed_out == true` and completes well under, say, 5s (asserts no hang).
- A normal passing test file with a generous deadline returns
  `timed_out == false` and identical outcomes to the no-deadline path
  (no-regression).

### Step 3: In-process mutant path honors the deadline + baseline-derived budget

**Goal:** Make `Isolation::InProcess` timeout-safe and compute the per-run
budget from the baseline wall-time. After this step the reported bug is
fixed for the in-process path, and the R1 failing-fixture test passes.

**Files:**
- `crates/zero/src/cmd/mutate.rs`

**Changes:**
- Add `fn parse_timeout(s: &str) -> anyhow::Result<Duration>`: accepts
  `"<n>ms"`, `"<n>s"`, or a bare integer (seconds); errors on anything else.
  (Used by the flag in Step 5; introduced here with its unit tests.)
- Add budget constants `TIMEOUT_FLOOR_MS: u64 = 2000`, `TIMEOUT_FACTOR: u64 = 5`
  and `fn mutant_budget(baseline_ms: u64, override_: Option<Duration>) -> Duration`
  → `override_.unwrap_or(Duration::from_millis(max(FLOOR, baseline_ms*FACTOR)))`.
- `run_inner`: add param `timeout: Option<Duration>`. Wrap the `run_baseline`
  call with `Instant`: `let t0 = Instant::now(); let baseline = run_baseline(..); let baseline_ms = t0.elapsed().as_millis() as u64;`.
  Compute `let budget = mutant_budget(baseline_ms, timeout);` and thread it
  into `dispatch_mutants`.
- `dispatch_mutants` / `dispatch_sequential` / `dispatch_parallel`: add a
  `budget: Duration` param (parallel stores it in the `Arc` bundle).
- `run_one_mutant_inproc`: add `budget: Duration`; per test file compute
  `deadline = Instant::now() + budget`, call
  `run_file_with_loader_deadline(root, tf, loader, Some(deadline))`; if
  `result.timed_out` return `MutantStatus::TimedOut` (checked before the
  `load_error`/`Failed` checks so an abort isn't miscounted as errored).
- Update the existing `run()` call site and all in-repo `run_inner(...)` test
  call sites to pass a `timeout` argument (`None`).

**Tests (mutate.rs, in-memory tempdir, `Isolation::InProcess`, bounded wall-time):**
- **R1 failing-fixture / R6 case 1:** synthetic project with an exported
  `humanBytes`-shaped function (a `while` whose exit depends on an
  update/arith operator) and a sibling test. Run with a short explicit
  budget; assert the loop-inverting mutant is recorded `TimedOut`, folds into
  `killed`, the run **completes** (assert bounded elapsed), and the score is
  non-vacuous. A companion assertion that today's code (no deadline) would
  hang is expressed as "runs under budget with the guard" (the guard is what
  makes it terminate).
- **R6 case 3 (no regression):** a normal killed/survived fixture with a
  generous budget yields identical verdicts/score to a run with the guard
  effectively disabled.
- `parse_timeout` unit tests: `"10s"`, `"500ms"`, `"2"`, and error cases.
- `mutant_budget` unit tests: floor dominates for a fast baseline; factor
  dominates for a slow one; override wins.

### Step 4: Subprocess backstop — spawn/wait-with-timeout/kill + worker deadline arg

**Goal:** Guarantee the CLI (`Isolation::Subprocess`) can never hang, even if
a child wedges outside JS. The child arms its own engine deadline (fast, precise
self-report); the parent enforces a hard `budget + grace` wait and kills on
overrun.

**Files:**
- `crates/zero/src/cmd/mutate.rs`
- `crates/zero/src/main.rs`

**Changes:**
- `MutateWorker` subcommand (main.rs:87): add `#[arg(long)] timeout_ms: Option<u64>`;
  pass it into `worker_main`.
- `worker_main` (mutate.rs:1388): add `timeout: Option<Duration>`; forward to
  `run_one_mutant_inproc` as the budget (so the child self-aborts and exits
  with code `3` on timeout via `TimedOut.to_exit_code()`).
- `run_one_mutant_subprocess` (mutate.rs:1315): add `budget: Duration`.
  - Pass `--timeout-ms <budget_ms>` to the child.
  - Replace `.output()` with `.spawn()` + a std-only wait-with-timeout:
    `let hard = Instant::now() + budget + GRACE;` loop on `child.try_wait()`,
    sleeping a short interval (e.g. 10ms); if `Instant::now() >= hard` and the
    child is still running, `child.kill()` + `child.wait()` and return
    `MutantStatus::TimedOut`. (`GRACE = Duration::from_secs(2)`.)
  - Exit-code mapping: `Some(0)`→Survived, `Some(1)`→Killed, `Some(3)`→TimedOut,
    everything else → Errored. A child the parent killed → TimedOut directly
    (we know why it died).
  - Keep temp-file cleanup (`{uniq}.js`, `{uniq}.tests`) on **every** path,
    including the kill path (no leak).
- Thread `budget` through `dispatch_parallel`'s worker loop into
  `run_one_mutant_subprocess`.

**Tests:**
- **R6 case 2 (backstop):** an `Isolation::Subprocess` end-to-end run of a
  fixture that hangs; assert the mutant is `TimedOut`, the run completes in
  bounded wall-time, and no `zero-mutate-*` temp files linger. To exercise
  the *parent kill* specifically (not just the child's self-report), use a
  fixture whose hang the engine deadline can't observe — e.g. force the
  parent path with a tiny `GRACE`/injected knob, or assert kill-path cleanup
  via a unit test around the wait/kill helper. Factor the wait/kill logic
  into a small helper so it is unit-testable without a real hang.
- Worker-arg parse test in main.rs (`--timeout-ms` reaches `worker_main`).

### Step 5: `--timeout` CLI flag, end to end

**Goal:** Expose the user-facing override and confirm the whole path is wired.

**Files:**
- `crates/zero/src/main.rs`
- `crates/zero/src/cmd/mutate.rs`

**Changes:**
- `Mutate` command (main.rs:50): add
  `/// Per-mutant timeout (e.g. 10s, 500ms). Default: max(2s, baseline×5).`
  `#[arg(long)] timeout: Option<String>`; pass through to `cmd::mutate::run`.
- `cmd::mutate::run` (mutate.rs:1425): add `timeout: Option<String>` param;
  parse with `parse_timeout` (error out with a clear message on bad input);
  pass the resulting `Option<Duration>` to `run_inner`.
- Update the `main.rs` dispatch arm and any other `run(...)` callers.

**Tests:**
- main.rs clap tests (mirroring `parsed_threads`): `--timeout 10s` parses and
  reaches the command; absent → `None`; composes with `--threads`/`--no-cache`.
- **R6 case 4:** end-to-end mutate run where an explicit short `--timeout`
  (via `run_inner`) forces a `TimedOut` on a mutant that would pass under the
  derived budget — proves the override is honored.

### Step 6: Docs

**Goal:** Document the new flag and the timeout semantics; state the schema is
unchanged.

**Files:**
- `docs/config-and-cli.md`
- `docs/testing.md` (only if it characterizes mutate verdicts)

**Changes:**
- `docs/config-and-cli.md`, `### zero mutate [pattern]` (~181): document
  `--timeout <dur>` — syntax (`10s`, `500ms`, bare seconds), default
  (`max(2s, baseline × 5)`), and when to raise it. Add a short "Timeouts"
  note: a mutant that does not terminate within the budget is classified
  **killed** and reported as a `timed-out` sub-count (an infinite loop is a
  detected divergence, not a survivor). Reference the parent-kill backstop in
  one sentence.
- In the same section, note explicitly that `mutation.json` remains
  **schema v2** — `timed_out` is an additive field, no version bump.
- The `#### Reading Generated: 0` block (~229) is unaffected by this change;
  leave it, but ensure the new "Timeouts" note sits nearby so the two
  skip/kill concepts aren't conflated.
- `docs/testing.md`: if it lists mutate verdicts, add `timed-out` (⊂ killed).

**Tests:** none (docs). Executor spot-checks the rendered section.

## Risks and Assumptions

- **rquickjs interrupt API/bound.** Assumes `Runtime::set_interrupt_handler`
  is available in 0.12 with a `FnMut() -> bool` handler. If the bound is
  `Send`, `Arc<AtomicBool>` + a copied `Instant` satisfies it. If the API is
  absent/different, Step 2 must adapt (worst case: rely on the parent-kill
  backstop for the CLI and mark `Isolation::InProcess` as best-effort). This
  is the main technical unknown; Step 2 verifies it before Steps 3–4 build on
  it.
- **Interrupt granularity.** QuickJS invokes the handler periodically, not on
  every bytecode op; a pathological tight native loop with no interrupt point
  could evade the engine deadline. The parent-side kill (Step 4) is the
  backstop precisely for this; the in-process path (tests) assumes ordinary
  JS loops, which do hit interrupt points.
- **Default budget tuning.** `floor=2s, factor=5` are starting values. A slow
  `examples/` suite on a slow CI box could in principle false-time-out a
  legitimately slow mutant; the executor verifies against real baseline times
  and the constants are centralized for easy adjustment.
- **`FileResult` field addition** touches several constructors across the
  test-runner; the compiler enumerates them, but any external assertions on
  `FileResult` shape may need a trivial update.
- **Cache invalidation.** Bumping `CACHE_SCHEMA_VERSION` invalidates existing
  `mutation/cache.json` once, causing one full re-run per project — expected,
  harmless (coverage/verdicts are a signal, not a gate).
- **Sleep-poll overhead.** The parent's 10ms `try_wait` poll adds negligible
  latency versus mutant run times; if it ever matters, a timer thread is a
  drop-in replacement (still std-only).
- **Assumption:** timed-out ⊂ killed is the desired scoring (confirmed in
  refinement). If a future consumer needs timed-out excluded from `killed`,
  the `timed_out` field already carries the sub-count to subtract.
