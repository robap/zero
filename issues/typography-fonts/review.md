# Review: Local Geist fonts + typography utilities

## Status
PASS WITH NOTES

## Checklist Completion
All steps complete: yes
- [x] Step 1 — Move font assets into `.zero/fonts/`
- [x] Step 2 — `woff2` / `woff` / `ttf` / `otf` MIME types
- [x] Step 3 — `/.zero/fonts/*` dev-server route
- [x] Step 4 — `binary_manifest()` + `write_framework_files` extension
- [x] Step 5 — `zero update` handles binary entries
- [x] Step 6 — `copy_public` → `copy_tree`, fonts copied to `dist/.zero/fonts/`
- [x] Step 7 — `_typography.scss` + `zero.scss` aggregator + manifest entry
- [x] Step 8 — `_base.scss` rewrite with `@font-face`, element selectors removed
- [x] Step 9 — Showcase + examples migrated to `.text-*` classes
- [x] Step 10 — Framework-side integration test on compiled CSS
- [x] Step 11 — Docs updated (spec, BEST_PRACTICES, scaffold AGENTS.md)

## Test Results
All tests passing: yes
- `cargo test --workspace`: 11 modules × `test result: ok`, 0 failures.
- `cargo clippy --workspace --all-targets`: clean, 0 warnings.
- `cargo run --bin zero -- build` from `showcase/`: succeeds (`zero build — 70822 bytes JS, 1 CSS file(s), 0 public asset(s), 0 font asset(s); output in showcase/dist/`). The `0 font asset(s)` is correct: showcase's `.zero/` is gitignored and not materialized in this working copy; the build helper conditionally copies the directory only when present.
- The review template asks to run `cargo run examples/honda_accord.json`. That's a generic-template artifact, not relevant to this project; skipped.

## Requirements Coverage

| Requirement | Status | Notes |
|-------------|--------|-------|
| Source-tree layout: 4 woff2 + OFL.txt under `.zero/fonts/` | Satisfied | `crates/zero-scaffold/src/scaffold/.zero/fonts/` contains all five files. |
| `binary_manifest()` returns 5 entries with correct paths | Satisfied | `crates/zero-scaffold/src/lib.rs:190-207`; covered by `binary_manifest_matches_expected_paths`. |
| `write_framework_files` writes binary entries | Satisfied | `lib.rs:259-265`; covered by `write_framework_files_writes_only_dot_zero`. |
| `zero update` diff/apply handles binary entries | Satisfied | `crates/zero/src/cmd/update.rs:161-212, 292-333`; covered by 3 new tests + updated empty-`.zero/` test. |
| Dev server route `/.zero/fonts/*path` | Satisfied | `crates/zero-dev/src/server.rs:240-252`; covered by `fonts_route_serves_woff2_with_correct_content_type` + 404 sibling test. |
| MIME `woff2`/`woff`/`ttf`/`otf` | Satisfied | `crates/zero-dev/src/files.rs:34-37`; four parallel one-liner tests added. |
| Build copy of `.zero/fonts/` → `dist/.zero/fonts/` | Satisfied | `crates/zero/src/cmd/build.rs:75-80`, helper renamed to `copy_tree`; covered by `build_copies_dot_zero_fonts_into_dist`. |
| Four `@font-face` blocks in `_base.scss` | Satisfied | `_base.scss:6-33`; values match spec verbatim including `format("woff2-variations")`, `font-weight: 100 900`, `font-display: swap`. |
| `_base.scss` element-selector deletion | Satisfied | Only top-level selectors remaining are `*`, `body`, `:focus`, `:where(...)`, and the four `@font-face`. Header comment rewritten to match spec. |
| `_typography.scss` with 12 classes inside `@layer components` | Satisfied | All 12 classes present with spec-exact values; `margin: 0` retained (not `margin-block: 0`); `.text-link:hover` uses `var(--color-primary-hover)`. |
| `zero.scss` aggregator gains `@use 'typography'` between `alignment` and `components` | Satisfied | `zero.scss:7-9`. |
| `framework_manifest()` gains `_typography.scss` entry | Satisfied | `lib.rs:128-129`; manifest path-set test updated. |
| Showcase + examples migrated | Satisfied | All `<h1>`/`<h2>`/`<p>`/visible-`<a>` tags now carry `.text-*` classes (verified via grep). |
| Docs: spec §7.1, BEST_PRACTICES, AGENTS.md | Satisfied | Typography + Fonts paragraphs added to `zero-framework-spec.md:925-927`, `BEST_PRACTICES.md:436-460`, `crates/zero-scaffold/src/scaffold/AGENTS.md:539,587-605`. |
| Tests: typography compiled-CSS shape assertion | Satisfied | `compiled_zero_css_has_typography_and_fonts_and_no_element_selectors` in `lib.rs:939-1005` covers (a) all 12 classes, (b) four `@font-face`, (c) no `fonts.googleapis.com`, (d) no top-level element selectors. |
| Tests: `binary_manifest_matches_expected_paths` | Satisfied | `lib.rs:828-845`. |
| Tests: `write_initial_project_emits_framework_files` extended for font bytes | Satisfied | `lib.rs:524-534`. |

## Constraints and Scope
- All constraints respected: no Google Fonts URL, `zero update` writes only inside `.zero/`, `@layer components`, no `!important` (asserted), no new JS runtime work scoped to this change.
- Out-of-scope work present (not blocking, but worth noting):
  - **`crates/zero-test-runner/src/harness.rs`** has an unrelated edit adding `boa_engine::gc::force_collect()` after dropping the context. This is a JS test-runner change with no connection to the typography/fonts spec; it appears to be incidental work bundled into the same branch.

## Code Quality Notes
- `crates/zero-scaffold/src/lib.rs:87-94` — `include_bytes!` constants follow the convention of `include_str!` neighbors; names (`FONT_GEIST`, etc.) are clean and grouped under a comment that explains the embed.
- `crates/zero-scaffold/src/lib.rs:190-207` — `binary_manifest()` is a faithful parallel of `framework_manifest()`. Spec accepted either parallel or merged-enum shape; planner picked parallel, plan documents the trade-off.
- `crates/zero/src/cmd/update.rs:162-193` — text + binary loops are duplicated rather than abstracted; this is acceptable at N=2 manifests and matches the parallel-manifest decision.
- `crates/zero/src/cmd/update.rs:309-321` — `apply()` looks up by linear scan through both manifests for each operation (O(n·m) total). At ~70 entries this is invisible; if the manifest grows by another order of magnitude this would deserve a `HashMap` precompute, but the current shape is correct.
- `crates/zero-dev/src/server.rs:240-252` — the new route uses plain `serve_under`, consistent with `/public/*` and unlike the TS/SCSS routes. Correct.
- `crates/zero/src/cmd/build.rs:75-80` — uses the spec-suggested `if fonts_src.is_dir()` guard, so projects without bootstrapped fonts (`.zero/` gitignored, no `zero init` run) don't fail the build.
- `crates/zero-scaffold/src/scaffold/.zero/styles/_typography.scss` — 12 classes implement the spec's property table verbatim. The optional `%heading-base` placeholder is intentionally not used per the plan.
- The compiled-CSS test at `lib.rs:939-1005` is well-targeted: it asserts the four invariants that must not regress (utility class presence, four `@font-face`, no Google Fonts URL, no top-level element selectors) and uses byte-substring matching that is robust to compressed vs. expanded output.
- The `font-family: "Geist"` and `font-family: "Geist Mono"` counts in the test correctly disambiguate (the substring `"Geist"` does not appear inside `"Geist Mono"` because of the trailing space).

## Issues to Address
1. `crates/zero-test-runner/src/harness.rs` change is out of scope for this issue. Either move it onto its own branch/commit with its own justification, or document it in the commit/PR as a co-landed unrelated fix.
