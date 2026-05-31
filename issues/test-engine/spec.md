# Spec: Replace the Boa JS engine with QuickJS (rquickjs)

## Problem Statement

The `zero test` runner executes user test files in an embedded JavaScript
engine, [Boa](https://github.com/boa-dev/boa) (`boa_engine` 0.21). Boa has been
a recurring source of friction during this project:

- **Correctness/stability bugs.** The harness carries a `std::mem::forget`
  workaround to dodge Boa 0.21's `MapLock::finalize` GC panic — a `BorrowMutError`
  during finalization that can abort the entire process when a test file uses
  `Map` (FRAMEWORK_NOTES #63; the [[boa_maplock_finalizer]] memory documents a
  second workaround keeping code-path-variant branches in their own functions to
  avoid a GC panic on exit).
- **Performance.** `issues/test-perf/measurements.md` established that
  `zero test` is interpreter-bound: ~98% of `zero_demo`'s 5.4 s run is per-file
  Boa interpretation (test-exec 70%, dom-shim eval 16%, runtime-eval 12%);
  transpile (SWC) is only 1.5%. The single-threaded optimizations originally
  planned could not reach the target because the cost *is* the interpreter.

QuickJS (via the `rquickjs` crate, bundling QuickJS-ng) is a mature, fast,
bytecode-interpreted engine with a small, well-matched embedding API. A
feature-parity smoke test (`crates/zero-test-runner/examples/qjs_smoke.rs`)
already ran **all 23 `zero_demo` `web/src` test files / 139 `it()` bodies**
through QuickJS — full module graph plus dom-shim — with no crashes and no `Map`
finalizer issue. Replacing Boa removes the GC-bug class outright and is expected
to cut interpreter time; this item validates that with a real measurement and,
on confirmation, completes the migration so **no `boa_engine` dependency remains
anywhere in the workspace.**

## Background

### Where Boa lives

The engine is confined to `crates/zero-test-runner`:

- `loader.rs` — a Boa `ModuleLoader` resolving `"zero"`, `"zero/test"`,
  `"zero/http"`, `"zero/components"`, and relative `./`/`../` specifiers
  (transpiling `.ts` via SWC, applying coverage instrumentation and the
  `zero mutate` overlay, tracking resolved paths for test-impact analysis).
- `harness.rs` — boots a Boa `Context` per file, installs `console` +
  `__readWorkspaceFile__`, evaluates `ZERO_DOM_SHIM_BODY`, evaluates the entry
  module, drains the promise-job queue, reads the test tree from `zero/test`'s
  namespace, walks `describe`/`it` running `beforeAll`/`afterAll`/`beforeEach`/
  `afterEach` hooks, captures thrown errors (message, stack, `_userFrame`),
  remaps stack positions to the original `.ts` via source maps, and returns
  `FileResult`/`RunOutcome`.
- `coverage.rs` / `bundler.rs` — Boa appears only in `#[cfg(test)]` helpers
  (`run_in_boa`, `bundle_evaluates_under_boa`).

Everything else in the crate — `discovery`, `reporter`, `result`, `timing`,
`mutate`, and the coverage *instrument* transform (SWC-based) — is
engine-agnostic.

### Public surface callers depend on

`crates/zero/src/cmd/test.rs` and `cmd/mutate.rs` consume only stable signatures:
`run_file`, `run_file_with_coverage`, `run_file_with_loader`, and the
`ZeroModuleLoader` / `CoverageContext` handles (e.g.
`ZeroModuleLoader::new(root).with_overlay(...)`). These signatures must not
change, so the swap is internal to the test-runner crate.

### Why oxc / "roll our own" were rejected

oxc is a parser/transformer with no JS execution — it cannot run tests, and the
1.5% transpile slice is already cheap. A hand-written engine is a multi-year
effort that would almost certainly be slower than Boa. rquickjs is the realistic
embeddable engine; deno_core/V8 is the heavier max-speed alternative, out of
scope here.

### Relationship to other items

- Successor in spirit to [`test-perf`](../test-perf/spec.md) (shipped as
  measurement-only), which proved the cost is the interpreter.
- Orthogonal to [`test-parallel`](../test-parallel/spec.md): parallel file
  execution remains a separate lever and composes with a faster per-thread
  engine. This item is single-threaded.

## Requirements

### Engine implementation (unconditional)

- Implement a QuickJS (rquickjs) module loader and per-file harness in
  `crates/zero-test-runner` that produce **byte-for-byte equivalent**
  `FileResult`/`RunOutcome` to the Boa path: identical pass/fail/skip status,
  test ordering within a file, failure messages, and source-mapped failure
  locations.
- Preserve the full hook semantics (`beforeAll`/`afterAll` per suite;
  `beforeEach`/`afterEach` collected up the parent describe chain), error capture
  (message, stack, `_userFrame`), source-map remapping, coverage serialization of
  `globalThis.__zero_coverage__`, and the `zero mutate` overlay + resolved-path
  tracking.
- During development the two engines coexist: QuickJS lives behind an additive
  cargo feature so the default build stays Boa and green, and the suite can be
  run on either engine to compare.
- The `catch_unwind` panic safety net is retained; the Boa-specific
  `std::mem::forget` MapLock workaround is **not** ported (QuickJS drops cleanly).

### Comparison (unconditional — the decision input)

- Produce a real before/after measurement on `zero_demo`: release builds,
  identical transpiled input, single-threaded, `ZERO_TEST_TIMING=1`, median of
  ≥3 runs. Record the per-phase table (Boa vs QuickJS) and the wall-clock delta
  in `issues/test-engine/measurements-qjs.md`.

### Correctness / no-regression (hard requirement)

- `cargo test --workspace` and `cargo test --workspace -- --include-ignored`
  (e2e_init_*, showcase_*, examples_*, mutate, coverage suites) pass identically
  under QuickJS — same outcomes as Boa.
- `zero test` on `zero_demo` reports the same pass/fail/skip counts as before.
- `--coverage` output (table + `coverage.json`) is unchanged.

### Cutover and Boa removal (gated on the measurement)

- The cutover decision bar is **no test-outcome regression AND a measurable
  single-threaded wall-clock speedup on `zero_demo`** — with the **user making
  the final call** (no automated numeric threshold). The comparison above is the
  evidence for that decision.
- On a go decision, QuickJS becomes the sole engine and **`boa_engine` is removed
  from every manifest and source file in the workspace** (both `zero test` and
  `zero mutate` run on QuickJS; the test-only `run_in_boa` / `bundle_evaluates_
  under_boa` helpers are ported to rquickjs or dropped; the additive feature
  flag, the `ZERO_ENGINE` switch, and the smoke-test example are removed). The
  end state has no Boa, no exceptions.

### Documentation

- The engine is an **internal implementation detail**; `zero test` behavior,
  CLI, flags, and `zero.toml` are unchanged, so **no user-facing `docs/*.md`
  content change is required.** This is called out so the plan records that docs
  were considered, not forgotten. Two cleanups still apply on cutover: (1) update
  `CLAUDE.md` comments that name "the Boa interpreter"; (2) grep `docs/testing.md`
  (and the rest of `docs/`) for any incidental "Boa" mention and correct it if
  present. If the plan ends up exposing a user-visible surface (it should not),
  it must update `docs/config-and-cli.md`.

## Constraints

- **No public-API change.** `run_file*`, `ZeroModuleLoader`, and
  `CoverageContext` signatures consumed by `crates/zero` stay identical.
- **No test-semantics change.** Outcomes, ordering, messages, and source-mapped
  locations are preserved exactly.
- **Single-threaded.** This item does not introduce parallel file execution.
- **Per-file isolation preserved.** A fresh QuickJS runtime/context per file;
  tests across files never share state.
- **Additive during development.** The default build remains Boa-backed and green
  at every step until the gated cutover.
- **Licensing.** rquickjs / QuickJS-ng are MIT/BSD-compatible with the workspace
  MIT license.

## Out of Scope

- **Parallel file execution** — the separate [`test-parallel`](../test-parallel/spec.md)
  item; composes with this one later.
- **deno_core / V8** — heavier max-speed alternative; not pursued here.
- **oxc / rolling a custom engine** — rejected (see Background).
- **Changing test APIs, dom-shim behavior, discovery, reporter format, or
  coverage/mutation semantics** beyond what the engine swap requires to stay
  equivalent.
- **Reworking `zero mutate` performance** — it inherits the engine but its
  parallelism story is unchanged.

## Open Questions

- **Error/stack-frame fidelity.** QuickJS stack formatting differs from Boa's;
  the `remap_positions` regex (`file.ts:LINE:COL`) and `_userFrame` extraction
  may need format tweaks to keep failure locations identical. Validated by the
  outcome-parity integration tests during planning/execution.
- **Coverage JSON shape.** `globalThis.__zero_coverage__` must serialize to the
  exact JSON the aggregator expects; confirm via the coverage integration tests
  under the QuickJS feature.
- **Final cutover decision.** Conditional on the recorded measurement meeting the
  no-regression-plus-measurable-speedup bar; the user decides. If the speedup is
  not measurable, the additive QuickJS path can remain in place (Boa default) or
  the work can pause at the gate without breaking the tree.
