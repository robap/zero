# Spec: decide the fate of `examples/`

## Problem Statement

`examples/{counter,todos,tracker}` predate having a full, separate inventory
application that now serves as the real-world reference. The instinct is to
delete them, but they are **not** a clean delete — they are load-bearing for
docs and tests. This item is a **decision**, then the follow-through, not an
unconsidered `rm -rf`.

## Background

Known couplings (verify current extent before acting):

- **Tests.** Integration tests reference the examples, e.g.
  `crates/zero/tests/examples_layout.rs` and the `examples_*` slow tests noted
  in `CLAUDE.md`. Removing the examples breaks or orphans these.
- **Docs.** At least `docs/examples-tour.md`, `docs/best-practices.md`, and
  `docs/reactivity.md` reference `examples/`. `examples-tour.md` is likely
  entirely about them. Removing the dir leaves dangling docs and dead links.
- **Showcase.** There is a separate `showcase/` dir — clarify how it relates to
  `examples/` and whether it already covers some of the same ground.

## Options

1. **Keep, repurposed as fixtures.** Treat `examples/` explicitly as
   doc-backing + test-surface fixtures. Cheapest; keeps tests and docs valid.
   Cost: they must stay maintained as the framework evolves.
2. **Cut entirely.** Delete `examples/`, remove/rewrite the `examples_*` tests,
   and rewrite the three docs (and `examples-tour.md`'s role) to point at the
   external inventory app or `showcase/`. Cleanest tree; real doc work.
3. **Slim down.** Keep one minimal example (e.g. `counter`) as the canonical
   tiny sample; cut the rest and reconcile docs/tests to the survivor.

## Scope / Non-Goals

- In scope: the `examples/` directory and everything that references it (tests +
  docs).
- Non-goal: changing `showcase/` beyond clarifying its relationship, unless the
  chosen option folds examples into it.

## Open Questions (resolve with user before plan — this is mostly a decision)

- Which option (1/2/3)?
- If cut: does the external inventory app become the linked reference in docs,
  and is it public/linkable?
- What is `showcase/` for, and does it already make `examples/` redundant?

## Done When

- A decision is recorded here, and the tree is consistent with it: no dangling
  doc links, no orphaned/broken `examples_*` tests, `cargo test --workspace --
  --include-ignored` green.
