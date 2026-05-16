# Review: Component Library

## Status
PASS WITH NOTES

Note: the `/review` arg was `showcase`, but no `issues/showcase/` directory
exists. The showcase work is the §"Showcase project" portion of the
Component Library spec at `issues/components/`, so this review covers that
issue end-to-end (component implementations, showcase, integration tests,
docs).

## Checklist Completion
All steps complete: **yes**

All 21 plan steps are marked `[x]`. Two steps record deviations in their
own checkboxes:

- Step 19 — `showcase/.zero/` is **gitignored**, not committed. Tests
  regenerate it from the manifest via `prepare_showcase()`.
- Step 20 — `tests/showcase_drift.rs` was not written; the helper in
  `tests/common/mod.rs` materializes `.zero/` fresh per test, so drift is
  structurally impossible.

Both deviations are documented in the plan body.

## Test Results
All tests passing: **yes**

`cargo test --no-fail-fast`: 181 unit tests + every integration suite green
(0 failed across `component_library`, `showcase_build`, `showcase_dev`,
`scss_build`, `update`, `e2e_init_*`, etc.). The Boa-based smoke tests in
`tests/test_runner_smoke.rs` remain `ignored`, consistent with main.

`cargo clippy --all-targets`: clean — no warnings, no errors.

The skill prompt's step "run `cargo run examples/honda_accord.json`" is
generic `/review` boilerplate; `examples/` does not exist and zero is a web
framework CLI, not a JSON processor. Skipped.

## Requirements Coverage

| Requirement | Status | Notes |
|-------------|--------|-------|
| 1. `.zero/components/` ships 14 + index | Satisfied | `src/scaffold/.zero/components/` lists 14 components + `index.ts`. |
| 2. Plain functions, no class/wc | Satisfied | Every component is `export default function Name(...)`. |
| 3. Stateful props accept signals | Satisfied | Checkbox/Toggle/Radio/Select/Input/TextArea/Dialog/Toast/Tabs all read `.val` / write `.set()` on the parent-owned signal. |
| 4. Only `var(--*)` tokens for design values | Partial | Three partials (`_badge.scss`, `_toast.scss`, `_button.scss`) declare local CSS custom properties with hex values for `success`/`warning`/`danger`/`button-danger`. Plan §"Risks" explicitly accepts this until a wider token surface lands. |
| 5. `index.ts` re-exports each component | Satisfied | `src/scaffold/.zero/components/index.ts:1-30`. |
| 6. Prop types declared in `.ts` source | Satisfied | Each component exports `<Name>Props` (and where applicable `<Name>Variant`/`<Name>Size`) from its `.ts`. `index.ts` re-exports the types. |
| 7. Partials live in `_<name>.scss` inside `@layer components` | Satisfied | All 14 partials are wrapped; `component_partials_use_layer_components` enforces it. |
| 8. `_components.scss` `@use`s each partial alphabetically | Satisfied | `src/scaffold/.zero/styles/_components.scss:5-18`. |
| 9. `zero.scss` ends with `@use 'components';` | Satisfied | `src/scaffold/.zero/styles/zero.scss:1-6`. |
| 10. No `!important`, no inline hex, no magic numbers | Partial | No `!important` anywhere (test enforced). Six hex literals total — all inside `:root { --<comp>-<variant>-{bg,fg}: ... }` blocks for success/warning/danger. A few inline-size literals (`24rem`/`32rem`/`48rem` for dialog sizes, `2px` thumb offsets) exist where no token applies; this matches the spec's documented exception list. |
| 11. Flat class names, dash-suffixed variants/sizes | Satisfied | `.button-primary`, `.dialog-open`, `.tabs-tab`, etc. No BEM. |
| 12. `@layer` lets user CSS override without `!important` | Satisfied | Aggregate ordering verified by `zero_scss_contains_aggregate_uses`. |
| 13. `"zero/components"` resolves in dev + bundler | Satisfied | `src/build/resolver.rs:31-33` + `src/dev/inject.rs:12-14` + `src/dev/server.rs:135-148`. |
| 14. No new `"zero"` exports | Satisfied | Top-level `"zero"` runtime is unchanged. |
| 15. `.zero/components.d.ts` declares the module | Satisfied | `src/scaffold/.zero/components.d.ts:4-143`. |
| 16. New `TPL_*` per file | Satisfied | `src/scaffold.rs:26-73`. |
| 17. `framework_manifest()` ≈ 53 entries | Satisfied | Exactly 53 entries (`framework_manifest_matches_expected_path_set`). |
| 18. Old `framework_manifest_lists_eight_files` updated | Satisfied | Renamed to `framework_manifest_matches_expected_path_set` (`src/scaffold.rs:611`). |
| 19. Six iterating scaffold tests | Satisfied | `components_index_re_exports_each_listed`, `component_source_files_emitted`, `component_test_files_emitted`, `component_partials_use_layer_components`, `components_aggregate_uses_each_partial`, `components_dts_declares_each_listed`. |
| 20. `showcase/` is a full zero project | Satisfied | `zero.toml`, `index.html`, `src/app.ts`, 15 routes, `styles/app.scss`, `.gitignore`. |
| 21. Showcase `.zero/` committed | **Deviation** | Plan-approved deviation: `.zero/` is gitignored; `prepare_showcase()` regenerates it from the manifest in CI. Spec rationale (no drift) is preserved structurally. |
| 22. Showcase regeneratable via `zero update --yes` | Satisfied | The helper does exactly this and integration tests prove it. |
| 23. Showcase serves with `zero dev` | Satisfied | `tests/showcase_dev.rs` exercises `/` + `/.zero/components/index.ts`. |
| 24. Showcase builds with `zero build`, CSS contains `@layer components` | Satisfied | `tests/showcase_build.rs:42-50` checks both `@layer components` in CSS and the bundle's components-index define. |
| 25. Theme toggle writes `document.documentElement.dataset.theme` | Satisfied | `showcase/src/app.ts:22-29` (auto removes the attr, light/dark set it). |
| 26. One `*.test.ts` per component | Satisfied | All 14 present; `component_test_files_emitted` enforces. |
| 27. Each test exercises a key interaction | Satisfied | Spec-listed assertions are present (Button onClick spy + disabled suppression, Input/TextArea signal updates, Checkbox/Toggle flip, Radio multi-instance, Select change, Dialog `dialog-open` toggle, Toast visibility flip, Tabs click + panel render, plus display-only render+variant assertions for Card/Spinner/Badge/Avatar). |
| 28. CI integration test runs `zero test` in showcase | Satisfied | `tests/component_library.rs` asserts `0 failed` and presence of every component name. |
| 29. Component tests ship to user projects | Satisfied | `.zero/components/*.test.ts` are in `framework_manifest()`. |
| 30. `AGENTS.md` gets a Components section + table + examples + layer note + showcase pointer | Satisfied (with rename) | New section is `## Component library` (line 590) because `## Components` already existed for the conceptual function-component pattern. Sentinel test asserts both headers. Intro line at the top is updated to list three import paths. |
| 31. Spec §11 lists `"zero/components"` exports | Satisfied | `zero-framework-spec.md:1158-1184`. |
| 32. Spec §12 Phase 9 items marked `[x]` | Satisfied | Lines 1276-1282 all `[x]`. |
| 33. Spec §13 adds Component-library row | Satisfied | Line 1318. |
| 34. Spec §7.1 extended with `@layer components` pointer | Satisfied | Line 883. |
| 35. `zero update` requires no new code | Satisfied | New manifest entries surface as Add ops; existing `update.rs` tests still pass. |
| 36. `tsconfig.json` `paths` entry for `"zero/components"` | **Deviation** | Plan resolved the OQ to use the ambient `declare module "zero/components"` block in `.zero/components.d.ts` and add `.zero/components.d.ts` to `include`. No `paths` block. Editor resolution works because of the ambient declaration, so the spirit of the requirement (editor autocomplete + go-to-def) is met. `tsconfig_include_contains_components_dts` is the regression test. |
| 37. `.gitignore` unchanged | Satisfied | `.zero/` and `dist/` continue to cover everything new. |

## Constraints and Scope

Constraints all respected with the documented exception captured above
(per-variant hex literals inside `@layer components` for Badge/Toast/Button
danger). Out-of-scope items were not built:

- No theming API, no form-validation lib, no general animation utilities.
- No date/menu/tooltip/popover/etc.
- No headless variants, no SSR, no a11y audit, no snapshot tests.
- No npm publication, no rename of existing tokens.

## Code Quality Notes

- **Resolver wiring is symmetric across the three pipelines.**
  `src/build/resolver.rs:31-33`, `src/dev/inject.rs:12-14`, and
  `src/test_runner/loader.rs:131-192` each map `"zero/components"` to
  `<root>/.zero/components/index.ts`. The Boa loader includes a path-escape
  check (`canonical.starts_with(&self.root)`) that the build resolver also
  has — good defensive symmetry.

- **Discovery exception is narrow and well-tested.**
  `src/test_runner/discovery.rs:99-104` narrows the hidden-dir skip only for
  `.zero` and descends only into `.zero/components/`. Three new tests
  (`walks_into_dot_zero_components`, `does_not_walk_into_other_dot_zero_subdirs`,
  `still_skips_other_hidden_dirs`) lock the behavior down. The exception is
  documented inline.

- **Manifest assertion is now path-set rather than length-coupled.**
  `src/scaffold.rs:611-680` builds two `BTreeSet`s and compares them, then
  asserts no duplicate keys via the length comparison. Adding a future
  component requires one row in the expected set, not a length bump.

- **Scaffold tests iterate `COMPONENT_NAMES`.**
  `src/scaffold.rs:238-344` is the single source of truth for the roster;
  every per-component test derives its coverage from it. This is the right
  call for a 14-item list.

- **`prepare_showcase` is the structural drift guard.**
  `tests/common/mod.rs:51-71` regenerates `.zero/` from the manifest before
  each showcase test, which gives stronger guarantees than a drift diff
  could (drift is impossible by construction). The plan deviation note in
  Step 20 captures this trade-off well.

- **Dialog's `effect` correctly cleans up the keydown listener** when
  `open.val` flips back to `false` or the component unmounts
  (`src/scaffold/.zero/components/Dialog.ts:29-36`). The early-return when
  closed avoids registering a listener while closed.

- **Toast's auto-dismiss timer uses the effect-cleanup contract.**
  `src/scaffold/.zero/components/Toast.ts:25-34` schedules `setTimeout`
  inside an `effect` that returns `clearTimeout`, so rapidly flipping `open`
  cannot double-fire. The test deliberately does not exercise this path
  (the lightweight DOM has no jsdom timer harness) — that omission is
  called out in the plan.

- **Tabs keyboard nav is implemented per the OQ resolution.** ArrowLeft /
  ArrowRight wrap; Home/End jump (`Tabs.ts:29-45`). The test exercises
  click navigation but not keys — minor coverage gap, not a spec gap.

- **Local-token hex literals in three partials.** `_badge.scss:7-12`,
  `_toast.scss:2-9`, and `_button.scss:2-5` declare `:root { --<comp>-<variant>-{bg,fg}: #...; }`
  inside `@layer components`. This is the documented compromise vs.
  expanding `_tokens.scss` with new public tokens. When tokens land, these
  locals should swap out for the global names.

- **Avatar's `alt[0]` fallback is correct but lossy for grapheme clusters.**
  Multi-codepoint glyphs (emoji, ZWJ sequences) will be split. Acceptable
  given the spec contract; future polish item.

- **No `_card_body` SCSS rule defined for `.card-body`.** Present in the
  source as a marker but only `color: inherit;`. Intentional — works as a
  hook for user overrides.

- **Style sheets average ~25 lines, well within the soft 30-line aim.**
  The largest is `_button.scss` at 64 lines; the variant block accounts for
  most of it.

## Issues to Address

None blocking. The two spec deviations (req 21 `.zero/` commit; req 36
`paths` block) and one partial (req 10 hex-literals) are all explicitly
plan-documented trade-offs with passing tests and documented rationale. If
future work wants to remove the trade-offs:

1. Extend `_tokens.scss` with `--color-{success,warning,danger}-{bg,fg}`
   and swap the three partials' `:root` blocks to consume them.
2. Add a `paths` block to `tsconfig.json` if editor go-to-definition into
   `.ts` source (vs. the `.d.ts` synopsis) is desired.
3. Commit `showcase/.zero/` if reproducibility-by-snapshot is preferred
   over reproducibility-by-regeneration; would require a CI drift check.

These are improvements, not bugs.
