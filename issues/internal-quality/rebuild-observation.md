# Rebuild observation (Phase 10, post-split)

Captured on `2026-05-16` after the R2 crate split completed.

## Method

1. `cargo clean` to clear `target/`
2. `/usr/bin/time -f "%e" cargo build --workspace` for the cold build
3. `touch <file>` then `cargo build --workspace` for each incremental case

## Numbers

| Scenario | Wall time | Notes |
|---|---|---|
| Cold build of full workspace | 61.77s | one-time cost; ~22.9k files written |
| Incremental after `touch crates/zero-dev/src/server.rs` | 3.34s | only `zero` (the binary) recompiles |
| Incremental after `touch crates/zero-test-runner/src/harness.rs` | 3.79s | only `zero` (the binary) recompiles |
| Incremental after `touch crates/zero-transpile/src/lib.rs` | 4.91s | only `zero` (the binary) recompiles |

## Before (pre-split, qualitative)

Previously, every change to any `src/` file forced cargo to invalidate the
single `zero` crate, which transitively pulls in `swc_core`, `boa_engine`, and
`axum`/`hyper`/`tower`. Cold incremental compiles after touching a single
source file typically required re-running the swc and boa codegen passes —
substantially longer than the post-split numbers above.

Quantitative before/after numbers are not captured here (we did not revert
the workspace to measure them); the qualitative observation that
"swc + boa + the axum chain stopped rebuilding on every touch" is what the
plan committed to verify and what the structural goals targeted.

## What this confirms

- The Tier-2 boundaries (`zero-dev`, `zero-test-runner`, `zero-bundler`)
  successfully isolate the heavy dependencies.
- Touching a leaf-crate file (`zero-transpile`) only forces the leaf
  crate and its direct downstream (the binary) to recompile; sibling
  Tier-2 crates stay cached.
- The binary recompile dominates incremental latency now, not the heavy
  third-party crates. Further wins would come from splitting `cmd::*`
  itself — out of scope for Phase 10.
