# Plan: Replace the Boa JS engine with QuickJS (rquickjs)

## Summary
`zero test` is interpreter-bound — `issues/test-perf/measurements.md` shows ~98%
of wall time is per-file Boa interpretation (test-exec 70%, dom-shim eval 16%,
runtime-eval 12%), and the project has hit repeated Boa GC/finalizer bugs (see
the MapLock leak workaround in `harness.rs`). A feature-parity smoke test
(`crates/zero-test-runner/examples/qjs_smoke.rs`) already ran all 23 `zero_demo`
test files / 139 `it()` bodies through QuickJS-ng (rquickjs 0.12) with no
crashes. This plan swaps the test runner's execution engine from `boa_engine`
to `rquickjs`, additively at first (both engines coexist behind a cargo
feature + `ZERO_ENGINE` switch) so we can get a real `ZERO_TEST_TIMING`
before/after on `zero_demo`, then — gated on that result — cuts over fully and
removes Boa.

Approach: the engine touches only `crates/zero-test-runner` (`harness.rs`,
`loader.rs`, plus test-only helpers in `coverage.rs`/`bundler.rs`). We first
extract the engine-agnostic *resolution policy* out of the Boa loader, then
build a parallel rquickjs loader + harness that reuse it and produce the
identical `FileResult`/`RunOutcome` the public API already returns, keeping
caller signatures (`crates/zero/src/cmd/{test,mutate}.rs`) untouched.

## Prerequisites
- Spec: `issues/test-engine/spec.md` (backfilled); item is a 🟡 row in
  `ROADMAP.md` under **Planned**.
- **Cutover gate (Steps 5–6).** Steps 1–4 are unconditional — they build the
  engine additively and produce the comparison. Steps 5–6 execute only on a go
  decision, and the bar is **no test-outcome regression AND a measurable
  single-threaded speedup on `zero_demo`, with the user making the final call**
  (no automated numeric threshold). The Step 4 measurement is the evidence for
  that call.
- Licensing: workspace is MIT; QuickJS-ng and rquickjs are MIT/BSD-compatible.
  Assumed acceptable.

## Steps

- [x] **Step 1: Extract engine-agnostic module resolution from the Boa loader**
- [x] **Step 2: Add `engine-quickjs` feature + rquickjs loader**
- [x] **Step 3: Implement the rquickjs harness (full outcome parity)**
- [x] **Step 4: Wire engine selection + capture the timing comparison**
- [x] **Step 5: (Conditional on Step 4) Cut over to QuickJS and remove Boa**
- [x] **Step 6: (Conditional) Docs, memory, and roadmap cleanup**

---

## Step Details

### Step 1: Extract engine-agnostic module resolution from the Boa loader
**Goal:** Pull all *resolution policy* (specifier matching, transpile, coverage
instrument, mutate overlay, path-escape check, `path_map` tracking) out of the
Boa-specific `ModuleLoader` impl so both engines can share it. Pure refactor —
Boa remains the only engine and behavior is unchanged. This is the seam every
later step depends on.
**Files:**
- `crates/zero-test-runner/src/loader.rs` (modify)
**Changes:**
- Add a struct `ResolvedModule { name: String, source: String, canonical: PathBuf }`
  and a method on `ZeroModuleLoader`:
  `fn resolve_source(&self, spec: &str, referrer_dir: &Path) -> Result<ResolvedModule, String>`.
  Move into it the policy currently spread across `load_imported_module`,
  `resolve_relative`, and `resolve_components_index`:
  - `"zero" | "zero/test" | "zero/http"` → return the in-memory `runtime_src` /
    `test_src` / `http_src` with `name = spec`, `canonical = spec`.
  - `"zero/components"` → canonical path of `<root>/.zero/components/index.ts`,
    read + `transpile_typescript`.
  - `"./" | "../"` → join against `referrer_dir`, `canonicalize`, enforce
    `starts_with(self.root)` (path escape), apply `overlay` short-circuit, else
    read + (coverage-`instrument` | `.ts`-transpile | raw). Record into
    `path_map` (for `loaded_paths`) exactly as today.
  - The `name` for file modules is the canonical path string (so the engine
    uses it as both cache key and child-import base).
- Reduce the existing `impl ModuleLoader for ZeroModuleLoader` to a thin
  wrapper: call `resolve_source`, then `Module::parse(Source::from_bytes(...).
  with_path(canonical), None, ctx)` and insert into the Boa `module_cache`.
  Keep the `module_cache`/`get_cached` Boa fields as-is for now.
- The Boa-specific `JsError` mapping moves to the wrapper; `resolve_source`
  returns `Result<_, String>` (engine-neutral).
**Tests:**
- All existing `loader.rs` unit tests (`resolves_zero_and_signal`,
  `resolves_zero_test_and_has_describe`, relative/escape cases) must pass
  unchanged. Add one unit test asserting `resolve_source("zero/test", root)`
  yields source containing `function describe(` and `name == "zero/test"`.
- `cargo test -p zero-test-runner` and the ignored integration suite stay green.

### Step 2: Add `engine-quickjs` feature + rquickjs loader
**Goal:** Introduce the additive cargo feature and the rquickjs module loader
that reuses Step 1's `resolve_source`. Default build stays Boa-only and green;
the feature, when enabled, compiles the qjs loader and its tests.
**Files:**
- `crates/zero-test-runner/Cargo.toml` (modify): add `[features] engine-quickjs = ["dep:rquickjs"]`;
  move `rquickjs` from dev- to a normal **optional** dependency
  (`rquickjs = { version = "0.12", features = ["loader", "macro"], optional = true }`).
- `crates/zero-test-runner/src/loader_qjs.rs` (create)
- `crates/zero-test-runner/src/lib.rs` (modify): `#[cfg(feature = "engine-quickjs")] pub mod loader_qjs;`
**Changes:**
- In `loader_qjs.rs`, define `ZeroResolver` and `ZeroLoader` implementing
  rquickjs `loader::{Resolver, Loader}`, constructed from an
  `Rc<ZeroModuleLoader>` so they share root/overlay/coverage/`path_map`.
  - `Resolver::resolve` mirrors the smoke test: bare `zero*` pass through,
    `zero/components` → abs index path, relative → canonicalized abs path,
    else `Error::new_resolving`.
  - `Loader::load` calls `loader.resolve_source(name, referrer_dir)` and wraps
    the returned source via `Module::declare(ctx, name, source)`. (QuickJS caches
    modules by name per-runtime, so no extra module cache is needed; `path_map`
    tracking already happens inside `resolve_source`.)
- There is **no `engine-boa` feature**: Boa stays the unconditional default
  build (always compiled, always green); only QuickJS sits behind the additive
  `engine-quickjs` feature. The Boa-only `module_cache`/`get_cached` fields stay
  as-is and are simply unused on the qjs path (removed in Step 5).
**Tests:**
- New unit tests in `loader_qjs.rs` (run with
  `cargo test -p zero-test-runner --features engine-quickjs`): a context with
  the qjs loader resolves+evaluates `import { signal } from 'zero'` and
  `import { describe } from 'zero/test'` without rejection (mirrors the Boa
  loader tests).
- Default `cargo test -p zero-test-runner` unaffected.

### Step 3: Implement the rquickjs harness (full outcome parity)
**Goal:** Per-file execution under rquickjs that returns the **same**
`FileResult`/`RunOutcome` as Boa — including hook semantics, error/stack
capture, sourcemap remapping, and coverage serialization — so callers and the
existing integration tests pass identically with the qjs engine selected.
**Files:**
- `crates/zero-test-runner/src/harness_qjs.rs` (create)
- `crates/zero-test-runner/src/harness.rs` (modify): make the engine-agnostic
  helpers reusable by `harness_qjs` — `remap_positions`, `FRAMEWORK_INTERNAL_BASENAMES`,
  `FRAMEWORK_REGISTRATION_NAMES`, the `CapturedError`/`Failure` shaping, and
  `prepare_source`/`read_test_source`. Mark them `pub(crate)` (do not change Boa
  behavior).
- `crates/zero-test-runner/src/lib.rs` (modify): under
  `#[cfg(feature = "engine-quickjs")]`, expose `harness_qjs::{run_file,
  run_file_with_coverage, run_file_with_loader}` with identical signatures to
  the Boa ones.
**Changes:**
- Port the `run_with_loader_inner` flow into a single `ctx.with(|ctx| { ... })`
  closure (rquickjs `Value<'js>` cannot escape it):
  1. `install_host` — `console.{log,warn,error}` (via a native `__print`) and
     `__readWorkspaceFile__` rooted at `project_root` (port from the smoke test;
     mirror the path-escape check in the Boa `install_workspace_file_reader`).
  2. Eval `ZERO_DOM_SHIM_BODY` as a script; on error → `load_error_outcome`.
  3. `prepare_source` (transpile + capture `sourcemap::SourceMap`), then
     `Module::evaluate(ctx, abs_name, src)`, `drain` jobs, inspect
     `PromiseState` → `load_error_outcome` on reject (with `caught` message).
  4. Read the test tree: evaluate a probe module importing `__getTestTree__`
     from `zero/test` and stash the tree on `globalThis`, then read it back as an
     `Object` (replaces Boa's `loader.get_cached("zero/test").namespace()`).
  5. `walk_describe` ported to rquickjs `Object`/`Function`: same control flow as
     `harness.rs` — `beforeAll` (skip subtree on failure), recurse children,
     `beforeEach`/`afterEach` collected up the `parent` chain, time the `it`
     body, `afterAll`. Each hook/body call is `fn.call(())` followed by `drain`.
  6. Error capture: on a thrown body/hook, build `CapturedError { message, stack,
     user_frame }` from the rquickjs exception (`message`/`stack`, plus the
     `_userFrame` property the test API attaches), then `build_failure` +
     `remap_positions` (reused from `harness.rs`).
  7. Coverage: when requested, JSON-serialize `globalThis.__zero_coverage__`
     (via `ctx.json_stringify` → `serde_json::Value`) into `RunOutcome.coverage`.
  8. `loaded` = `loader.loaded_paths()`. Keep the `catch_unwind` safety net
     (`run_with_loader`); **drop** the Boa `std::mem::forget(context)` MapLock
     workaround — QuickJS drops cleanly.
- Reuse `timing::record_since` for `ContextBuild`/`DomShim`/`RuntimeEval`/
  `TestExec` phases so the qjs path emits the same `ZERO_TEST_TIMING` table.
**Tests:**
- Run the existing test-runner integration tests with the feature on:
  `cargo test -p zero-test-runner --features engine-quickjs` and
  `cargo test --workspace --features zero-test-runner/engine-quickjs -- --include-ignored`
  (e2e_init_*, showcase_*, examples_*) — assert identical pass/fail/skip outcomes
  to the Boa run, including a file that uses `Map` (the Boa MapLock case) and
  the async files.
- Add a focused test that runs a known `zero_demo` file under qjs and asserts the
  outcome list matches the Boa harness's outcome list for the same file.

### Step 4: Wire engine selection + capture the timing comparison
**Goal:** Let `zero test` run on either engine so we can produce the real
before/after number — the deliverable that gates Step 5.
**Files:**
- `crates/zero/Cargo.toml` (modify): add passthrough feature
  `engine-quickjs = ["zero-test-runner/engine-quickjs"]`.
- `crates/zero/src/cmd/test.rs` (modify): when built with the feature and
  `ZERO_ENGINE=quickjs` is set, route `run_file_with_coverage` to the
  `harness_qjs` variant; otherwise Boa. (A small `#[cfg]`-guarded dispatch fn.)
- `issues/test-engine/measurements-qjs.md` (create): the comparison record.

Note: `cmd/mutate.rs` is intentionally *not* wired to the `ZERO_ENGINE` switch.
That switch is transitional, throwaway scaffolding for the `zero test` timing
comparison only; building a parallel one into mutate would be wasted work.
`zero mutate` parity on QuickJS is instead validated by running the mutate
integration tests under `--features engine-quickjs` in Step 3, and mutate moves
onto QuickJS unconditionally in Step 5 when Boa is removed (both `cmd/test.rs`
and `cmd/mutate.rs` collapse to the single qjs harness).
**Changes:**
- Implement the dispatch helper; default (no env / no feature) is unchanged Boa.
- Methodology (record in `measurements-qjs.md`): release builds
  (`cargo build -p zero --release` and `... --release --features engine-quickjs`),
  identical `zero_demo`, single-threaded, `ZERO_TEST_TIMING=1`, median of 3 runs.
  Capture the per-phase table for Boa vs QuickJS and the wall-clock delta.
**Tests:**
- `cargo build -p zero --features engine-quickjs` compiles; `zero test` with and
  without `ZERO_ENGINE=quickjs` both pass on `zero_demo`.
- The comparison is data collection, not an assertion — the gate is the recorded
  numbers + zero outcome regressions.

### Step 5: (Conditional on Step 4) Cut over to QuickJS and remove Boa
**Goal:** If the comparison justifies it, make QuickJS the sole engine and delete
Boa entirely — **no `boa_engine` dependency anywhere in the workspace** when this
step is done. Both `zero test` and `zero mutate` run on QuickJS; the GC-bug class
and the dual-engine maintenance cost are gone.
**Files:**
- `crates/zero-test-runner/src/harness.rs` → replace with the qjs implementation
  (delete Boa `harness_qjs.rs` shim by promoting it to `harness.rs`).
- `crates/zero-test-runner/src/loader.rs` → promote the qjs loader; remove the
  Boa `ModuleLoader` impl, `module_cache`/`get_cached`, `Module` fields.
- `crates/zero-test-runner/src/coverage.rs` (modify): port the test-only
  `run_in_boa` helper to rquickjs (or drop it).
- `crates/zero-bundler/src/bundler.rs` (modify): port the `#[cfg(test)]`
  `bundle_evaluates_under_boa` to rquickjs.
- `Cargo.toml` (workspace) + `crates/zero-test-runner/Cargo.toml` +
  `crates/zero/Cargo.toml`: remove `boa_engine`; drop the `engine-quickjs`
  feature flags and the `ZERO_ENGINE` switch; make rquickjs a non-optional
  normal dependency.
**Changes:**
- Remove the `ZERO_ENGINE` dispatch from `cmd/test.rs` and point both
  `cmd/test.rs` and `cmd/mutate.rs` at the now-sole qjs harness directly (mutate
  uses the same `run_file_with_loader`/`run_file_with_coverage`, now qjs-backed).
- Delete the MapLock leak comment/workaround and the smoke-test example
  (`examples/qjs_smoke.rs`).
**Tests:**
- Full suite green with no feature flags: `cargo test --workspace -- --include-ignored`
  (covers both the `zero test` and `zero mutate` integration tests on qjs).
- **Boa-removal invariant:** `grep -ri "boa" crates/ Cargo.toml` returns nothing
  (no `boa_engine` in any manifest or source); `cargo tree -i boa_engine` errors
  with "package not found."
- `cargo build --workspace --release` clean; `cargo install --path crates/zero --locked`
  succeeds.

### Step 6: (Conditional) Docs, memory, and roadmap cleanup
**Goal:** Reflect the engine change everywhere it's documented; close the loop.
**Files:**
- `CLAUDE.md` (modify): the test-command comments mention "Boa interpreter" —
  update to QuickJS; note the engine in the Commands section if relevant.
- `docs/*.md` (modify): grep for any "Boa"/engine references in user docs and
  update.
- `crates/zero-test-runner` module docs / comments referencing Boa.
- Auto-memory: update `boa_to_quickjs_exploration.md` → mark migrated; revise the
  `boa_maplock_finalizer.md` note (the workaround is gone — keep it only as
  historical context or delete) and the MEMORY.md pointers.
- `ROADMAP.md` (modify): move the `test-engine` row to **Test runner & quality**
  as ✅ with the ship date (per the lifecycle, `execute` handles this on clean
  wrap-up).
**Tests:**
- Docs-only; `cargo test --workspace` remains green. Sanity-read the changed
  docs (no heavy build needed).

## Risks and Assumptions
- **`Value<'js>` lifetime restructuring.** The Boa harness threads `&mut Context`
  through free functions; rquickjs confines values to a `ctx.with` closure, so
  `walk_describe` and helpers must be reorganized as nested closures/methods.
  Mitigated by the smoke test already proving the pattern works; risk is
  mechanical volume, not feasibility.
- **Error/stack fidelity.** QuickJS stack-frame formatting differs from Boa's;
  `remap_positions` (regex on `file.ts:LINE:COL`) and the `_userFrame` extraction
  may need format tweaks to keep failure locations accurate. Covered by the
  Step 3 outcome-parity tests; if frames differ, adjust the regex/extraction.
- **Coverage serialization shape.** `globalThis.__zero_coverage__` must serialize
  to the same JSON the aggregator expects; verified by running the coverage
  integration tests under the qjs feature in Step 3.
- **Decision dependency.** Steps 5–6 are conditional; if the measured win is
  marginal, the additive feature can stay in place (Boa default) or the work can
  pause at the gate without leaving the tree broken — Steps 1–4 keep the default
  Boa path fully intact.
- **Assumption:** the smoke test's clean run across all 23 files generalizes to
  full outcome parity (assertions, async timing, error messages). The Step 3
  integration-test gate is what actually validates this; if a feature gap
  surfaces there, it is contained to the qjs path.
- **Out of scope:** parallel file execution remains the separate `test-parallel`
  item; this plan is single-threaded and composes with it later.
