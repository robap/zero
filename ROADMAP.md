# Roadmap

Index of work tracked under [`issues/`](issues/). Each entry is a folder with a
`spec.md` (the what/why) and a `plan.md` (the how); some carry a `review.md`.
This file is the bird's-eye view — status only. Detail lives in the folders.

Status: 🟡 planned · ⏳ in progress · ✅ shipped

**Lifecycle (kept in sync by the workflow skills — don't hand-edit status out of
band):**

1. **`refine`** writes `issues/<slug>/spec.md` and adds the item to the
   **Planned** table below as 🟡 (or, if already listed, points its link at the
   new spec).
2. **`plan`** writes `issues/<slug>/plan.md`. Status unchanged — still 🟡.
3. **`execute`** flips the item to ⏳ in the **Planned** table when it starts,
   and on a clean wrap-up (all steps done, checks green) moves the row out of
   **Planned** into the matching category table below as ✅ with the date.
4. **`review`** is optional and not part of the default loop. If you run it and
   it finds the work wanting (FAIL), it moves the row back to **Planned** as ⏳.

A `refactor` that doesn't correspond to a tracked item leaves this file alone; a
refactor that *is* a tracked item follows the same ⏳ → ✅ lifecycle.

## Planned

| Item | Status | Notes |
|------|--------|-------|
| [test-parallel](issues/test-parallel/spec.md) | 🟡 | Speed up `zero test` via parallel file execution (thread-local QuickJS contexts, like `zero mutate --threads`). Successor to test-perf, whose measurement showed ~98% of the run is irreducible single-threaded per-file/per-test work. |
| [examples-decision](issues/examples-decision/spec.md) | 🟡 | Decide keep/cut/slim for `examples/`; load-bearing for `examples_*` tests and three docs. Sequence with any `examples_*` test changes. |
| [coverage-output](issues/coverage-output/spec.md) | 🟡 | Make `coverage.json` opt-in via `--coverage-output <path>`; `--coverage` becomes table-only. Breaking change to `--coverage` auto-write. |

## Core & reactivity

| Item | Status | Shipped |
|------|--------|---------|
| [core-reactivity](issues/core-reactivity/spec.md) | ✅ | 2026-05-11 |
| [template-system](issues/template-system/spec.md) | ✅ | 2026-05-12 |
| [template-fixes](issues/template-fixes/spec.md) | ✅ | 2026-05-23 |
| [app-router](issues/app-router/spec.md) | ✅ | 2026-05-13 |
| [app-router-2](issues/app-router-2/spec.md) | ✅ | 2026-05-13 |
| [pagination](issues/pagination/spec.md) | ✅ | 2026-05-23 |
| [pagination-computed](issues/pagination-computed/spec.md) | ✅ | 2026-05-24 |

## Components

| Item | Status | Shipped |
|------|--------|---------|
| [components](issues/components/spec.md) | ✅ | 2026-05-16 |
| [button](issues/button/spec.md) | ✅ | 2026-05-30 |
| [combobox](issues/combobox/spec.md) | ✅ | 2026-05-23 |
| [drawer](issues/drawer/spec.md) | ✅ | 2026-05-29 |
| [table](issues/table/spec.md) | ✅ | 2026-05-17 |
| [table-sort](issues/table-sort/spec.md) | ✅ | 2026-05-24 |
| [component-debounce](issues/component-debounce/spec.md) | ✅ | 2026-05-24 |
| [forms](issues/forms/spec.md) | ✅ | 2026-06-06 |
| [form-controls](issues/form-controls/spec.md) | ✅ | 2026-06-06 |
| [alignment](issues/alignment/spec.md) | ✅ | 2026-05-16 |

## Design system & theming

| Item | Status | Shipped |
|------|--------|---------|
| [design-system](issues/design-system/spec.md) | ✅ | 2026-05-14 |
| [design-system-lint](issues/design-system-lint/spec.md) | ✅ | 2026-05-18 |
| [themes](issues/themes/spec.md) | ✅ | 2026-05-16 |
| [scss](issues/scss/spec.md) | ✅ | 2026-05-14 |
| [typography-fonts](issues/typography-fonts/spec.md) | ✅ | 2026-05-17 |

## Runtime & web-platform shims

| Item | Status | Shipped |
|------|--------|---------|
| [dom-shim](issues/dom-shim/spec.md) | ✅ | 2026-05-18 |
| [dom-shim-selectors](issues/dom-shim-selectors/spec.md) | ✅ | 2026-05-30 |
| [select-element](issues/select-element/spec.md) | ✅ | 2026-06-06 |
| [web-platform](issues/web-platform/spec.md) | ✅ | 2026-05-18 |
| [intl-shim](issues/intl-shim/spec.md) | ✅ | 2026-05-30 |

## Test runner & quality

| Item | Status | Shipped |
|------|--------|---------|
| [test-runner](issues/test-runner/spec.md) | ✅ | 2026-05-14 |
| [test-perf](issues/test-perf/spec.md) | ✅ | 2026-05-31 |
| [test-engine](issues/test-engine/spec.md) | ✅ | 2026-05-31 |
| [test-no-config](issues/test-no-config/spec.md) | ✅ | 2026-05-31 |
| [test-helpers](issues/test-helpers/spec.md) | ✅ | 2026-05-15 |
| [test-improvements](issues/test-improvements/spec.md) | ✅ | 2026-05-16 |
| [test-correctness](issues/test-correctness/spec.md) | ✅ | 2026-05-23 |
| [test-matcher-drift](issues/test-matcher-drift/spec.md) | ✅ | 2026-05-24 |
| [runtime-tests](issues/runtime-tests/spec.md) | ✅ | 2026-05-19 |
| [mutate-operators](issues/mutate-operators/spec.md) | ✅ | 2026-05-23 |
| [mutate-equivalence](issues/mutate-equivalence/spec.md) | ✅ | 2026-05-23 |
| [mutate-reachability](issues/mutate-reachability/spec.md) | ✅ | 2026-05-29 |
| [internal-quality](issues/internal-quality/spec.md) | ✅ | 2026-05-16 |
| [minification](issues/minification/spec.md) | ✅ | 2026-05-23 |

## CLI & dev server

| Item | Status | Shipped |
|------|--------|---------|
| [cli-bootstrap](issues/cli-bootstrap/spec.md) | ✅ | 2026-05-13 |
| [dev-watch](issues/dev-watch/spec.md) | ✅ | 2026-05-13 |
| [proxy-mode](issues/proxy-mode/spec.md) | ✅ | 2026-05-18 |
| [preview](issues/preview/spec.md) | ✅ | 2026-05-24 |
| [update](issues/update/spec.md) | ✅ | 2026-05-16 |
| [http-content-type](issues/http-content-type/spec.md) | ✅ | 2026-05-30 |
| [papercuts](issues/papercuts/spec.md) | ✅ | 2026-05-24 |

## Linting & TypeScript

| Item | Status | Shipped |
|------|--------|---------|
| [lint-js](issues/lint-js/spec.md) | ✅ | 2026-05-19 |
| [typescript](issues/typescript/spec.md) | ✅ | 2026-05-14 |

## Docs

| Item | Status | Shipped |
|------|--------|---------|
| [user-docs](issues/user-docs/spec.md) | ✅ | 2026-05-21 |
| [best-practices](issues/best-practices/spec.md) | ✅ | 2026-05-16 |
| [agentic-coding](issues/agentic-coding/spec.md) | ✅ | 2026-05-24 |
| [agents-doc](issues/agents-doc/spec.md) | ✅ | 2026-05-14 |
| [agents-quickstart](issues/agents-quickstart/spec.md) | ✅ | 2026-05-24 |
| [agents-update](issues/agents-update/spec.md) | ✅ | 2026-05-22 |
