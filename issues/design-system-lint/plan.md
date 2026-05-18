# Plan: Design-System Lint + AGENTS.md Negative Examples

## Summary

Ship a new `zero lint` subcommand that flags raw values (font weights, sizes,
spacing, radius, color literals, borders, gaps, margins) and raw `display:
flex/grid` layouts in user SCSS, naming the design-system token / utility /
primitive that replaces each. Pair the lint with a negative-examples section in
the scaffolded `AGENTS.md` so the diagnostic and the fix sit next to each
other.

Implementation lives in a new `crates/zero-lint` crate (a sibling to
`zero-sass`) that consumes raw SCSS source via a small property/value scanner
(no full SCSS AST — `grass` does not expose one, and the rules only need
declarations grouped by selector). Rules are pure functions over a `Decl {
property, value, line, col }` stream plus a `RuleBody { decls,
child_selectors }` view for L11. The `crates/zero/src/cmd/lint.rs` driver
discovers user SCSS via the `ignore` crate (gitignore-aware), runs every rule,
and prints diagnostics in `zero test` shape.

The spec's R4 (token additions) is reconciled against today's already-richer
`_tokens.scss`: the radius scale is renamed to the unsuffixed naming the spec
mandates (`--radius-pill` → `--radius-3xl`, plus a new `--radius-xs/xl/2xl`),
and the tracking family is left as-is (the spec's "new" tokens already shipped
in v0.5 with a wider scale).

## Prerequisites

Resolved open questions from the spec, by plan-phase decision:

1. **Lint crate location.** Stand up a new `crates/zero-lint`. `zero-sass` is
   intentionally a narrow `grass` wrapper; bolting a rule engine onto it
   couples compilation and linting, and future JS rules need a home that
   isn't the SCSS compiler.
2. **Machine-readable output.** Not in v1. The driver renders to stderr in
   `zero test` shape; structured output (`lint/lint.json`) is deferred until a
   CI/LSP consumer materializes. Design the `Diagnostic` struct so a `--json`
   flag is a print-formatter swap, not a refactor.
3. **Nearest-token logic.** Convert both candidate value and scale entries to
   a base unit (px for lengths, unitless for weights / line-heights, `em` for
   tracking). Pick the entry with the smallest absolute difference; on an
   exact midpoint, pick the *smaller* step. If `|value - nearest| > step_size`
   where `step_size = max(|nearest - second_nearest|, 1px)`, append "outside
   the scale" to the suggestion. Worked examples are listed in Step 3.
4. **Examples audit.** Step 11 runs the lint against `examples/tracker/web`
   and `showcase/` and lands fixes for any diagnostics so the integration test
   in Step 12 can assert zero diagnostics.
5. **L11 `flank` heuristic.** Implement it (`display: flex` + child rule
   setting `flex: 0 0 auto` on `> :first-child`). If Step 11's audit shows
   false positives, drop L11.flank from v1 in the same step; the rule table
   already calls this out as the noise-prone case.
6. **`--tracking-*` values.** No-op. The codebase already ships
   `--tracking-{tight,snug,normal,wide,caps}` with picked values. L04 uses
   the existing scale.

Note: today's `_tokens.scss` is richer than the spec's Background section
implies (six font sizes, four weights including `semi`, three line heights,
five tracking steps, focus-ring + motion tokens). The lint rules name the
*actually shipped* token surface, not the spec's outdated list. Step 1
reconciles only the radius family (the one place the spec's R4 conflicts with
what shipped).

## Steps

- [x] **Step 1: Reconcile the radius scale**
- [x] **Step 2: Scaffold the `zero-lint` crate**
- [x] **Step 3: Token model + nearest-match resolver**
- [x] **Step 4: Implement L01–L04 (typography rules)**
- [x] **Step 5: Implement L05 (color literals)**
- [x] **Step 6: Implement L06–L10 (radius/border/spacing/margin/gap)**
- [x] **Step 7: Implement L11 (layout-primitive detection)**
- [x] **Step 8: Wire the `zero lint` subcommand**
- [x] **Step 9: AGENTS.md — negative examples + primitive intent**
- [x] **Step 10: Framework spec §7.1 updates**
- [x] **Step 11: Audit + fix `examples/tracker/web` and `showcase/`**
- [x] **Step 12: Integration tests**

---

## Step Details

### Step 1: Reconcile the radius scale

**Goal:** Resolve the only token gap that conflicts with what already ships:
replace the semantic alias `--radius-pill` with the unsuffixed scale that
spec R4 mandates, and add the missing scale steps before any lint rule
references them. Done first so L06's "suggest `--radius-3xl` for fully-round"
branch has the token to name.

**Files:**
- `crates/zero-scaffold/src/scaffold/.zero/styles/_tokens.scss` (modify)
- `crates/zero-scaffold/src/scaffold/.zero/styles/components/_*.scss`
  (any partial that reads `var(--radius-pill)` — search the tree)
- `examples/tracker/web/styles/app.scss`,
  `showcase/styles/app.scss` (any consumers — re-search)

**Changes:**
- In `_tokens.scss`, replace the four-entry radius block with seven entries:
  `--radius-xs: 2px`, `--radius-sm: 4px` (unchanged), `--radius-md: 6px`
  (unchanged), `--radius-lg: 10px` (unchanged), `--radius-xl: 14px`,
  `--radius-2xl: 20px`, `--radius-3xl: 9999px`. The numeric steps follow a
  geometric-ish ramp that lines up with the existing `4 → 6 → 10` cadence;
  `3xl` keeps the previous pill value so callers that wanted "fully rounded"
  get a numerically identical result by renaming.
- Remove `--radius-pill`. Grep the scaffold tree (`crates/zero-scaffold/src/`,
  `examples/`, `showcase/`) for `var(--radius-pill)` / `--radius-pill` and
  rewrite each to `var(--radius-3xl)` / `--radius-3xl`. The shipped component
  partials (Badge in particular) are the likely callsite.

**Tests:**
- Update the existing `tokens_and_themes_split_correctly` test in
  `crates/zero-scaffold/src/lib.rs` to assert the new tokens are present and
  `--radius-pill` is absent.
- Add a `radius_scale_has_seven_steps` test that checks all seven names.
- Run `cargo test -p zero-scaffold` and confirm the scaffold compiles via
  `grass` after the rename (the `compiled_zero_css_*` tests exercise this).

---

### Step 2: Scaffold the `zero-lint` crate

**Goal:** Land the empty crate, wire it into the workspace, and stand up the
two foundations every rule needs: a gitignore-aware file walker and an SCSS
declaration scanner. Nothing tested at the binary level yet — the crate is
library-only.

**Files:**
- `Cargo.toml` (add `crates/zero-lint` to `members`; add `ignore = "0.4"` to
  workspace dependencies)
- `crates/zero-lint/Cargo.toml` (new)
- `crates/zero-lint/src/lib.rs` (new)
- `crates/zero-lint/src/walk.rs` (new)
- `crates/zero-lint/src/scan.rs` (new)

**Changes:**
- `Cargo.toml` for the new crate: depend on `ignore`, `regex`,
  `serde`/`serde_json` (for forward-compat with the JSON output path), and
  the workspace's `anyhow`. No async runtime.
- `lib.rs` exports the public API. Stub types:
  ```rust
  pub struct Diagnostic {
      pub rule: &'static str,        // "L01" .. "L11"
      pub file: PathBuf,
      pub line: u32,
      pub column: u32,
      pub property: String,
      pub value: String,
      pub message: String,           // "use var(--weight-semi)"
  }
  pub fn lint_project(root: &Path) -> anyhow::Result<Vec<Diagnostic>>;
  ```
- `walk.rs` exports `pub fn user_scss_files(root: &Path) -> impl Iterator<Item
  = PathBuf>` using `ignore::WalkBuilder` with `add_custom_ignore_filename(".gitignore")`
  and explicit `.add_ignore_pattern("**/.zero/**")` / `"**/dist/**"` /
  `"**/node_modules/**"`. Filters to `.scss` and `.css` extensions.
- `scan.rs` exports `pub struct Decl { property: String, value: String, line:
  u32, column: u32, selector_path: Vec<String> }` and `pub struct RuleBody {
  selector: String, decls: Vec<Decl>, child_decls: HashMap<String, Vec<Decl>>
  // keyed by combinator selector like "> :first-child" }`. Implementation:
  a small hand-written tokenizer that tracks `{` / `}` depth, splits the
  current block on `;`, and trims comments. Sass nesting flattens by
  concatenating selectors. The scanner is intentionally not a full SCSS
  parser — it ignores `@use`, `@mixin`, function calls, and treats `&`
  selectors literally; rules only need property/value text plus a way to
  group declarations inside one rule body.

**Tests:**
- `walk.rs`: `walker_excludes_dot_zero_and_dist` writes a tempdir with files
  under each blocklisted root and asserts they're not yielded.
- `walk.rs`: `walker_honors_gitignore` writes a `.gitignore` ignoring
  `vendor/` and asserts a file in `vendor/` is excluded.
- `scan.rs`: `scans_nested_decls_with_line_col` parses a fixture with nested
  selectors and asserts the recorded `(line, column)` of a known declaration.
- `scan.rs`: `scans_groups_child_combinator_decls_under_parent` parses
  `.flank { display: flex; & > :first-child { flex: 0 0 auto; } }` and
  asserts the parent body sees one decl while the child selector entry sees
  the `flex: 0 0 auto` decl.

---

### Step 3: Token model + nearest-match resolver

**Goal:** Land the data + algorithm every numeric rule depends on, in
isolation so the rule modules in Steps 4–6 just call `nearest(scale, value)`
and format the result.

**Files:**
- `crates/zero-lint/src/tokens.rs` (new)

**Changes:**
- `tokens.rs` defines a `Scale` struct of `(token: &'static str, value: f64,
  unit: Unit)` plus seven public statics: `WEIGHT`, `FONT_SIZE`, `LEADING`,
  `TRACKING`, `RADIUS`, `BORDER`, `SPACE`. Values mirror today's
  `_tokens.scss`; lengths normalize to px (with `1rem = 16px`, `1em = 16px`
  for the purposes of token matching).
- `pub fn nearest(scale: &Scale, value: f64, unit: Unit) -> NearestResult`.
  `NearestResult { token: &'static str, outside_scale: bool }`. Algorithm:
  convert input to base unit; find the entry with smallest `|input - entry|`;
  on a tie, choose the entry with the smaller raw value; mark `outside_scale
  = true` when the input is below the smallest entry or above the largest
  *and* its distance to the endpoint exceeds the adjacent step size.
- A small `Unit` enum (`Px`, `Rem`, `Em`, `Unitless`, `Percent`) plus
  `parse_dimension(s: &str) -> Option<(f64, Unit)>` for use by rule modules.

**Tests:**
- `nearest_picks_smaller_on_midpoint`: input halfway between `font-size-sm`
  (14px) and `font-size-md` (16px) → returns `--font-size-sm`.
- `nearest_marks_outside_scale_above_max`: 80px against `RADIUS` → returns
  `--radius-3xl` with `outside_scale = false` (3xl is 9999px, so the input
  is comfortably between scale endpoints — confirms the algorithm doesn't
  over-flag valid wide-rounded inputs). 0.5px against `RADIUS` → returns
  `--radius-xs` with `outside_scale = true`.
- `nearest_handles_rem`: `1.5rem` against `SPACE` → `--space-lg`.
- `nearest_weight_at_600_keyword`: numeric `600` against `WEIGHT` →
  `--weight-semi`; `bold` keyword via a separate `nearest_keyword` →
  `--weight-bold`.

---

### Step 4: Implement L01–L04 (typography rules)

**Goal:** Land the four lowest-risk rules first: `font-weight`, `font-size`,
`line-height`, `letter-spacing`. They share the simplest shape (single
property, single numeric value) and validate the Step 2/3 plumbing before any
multi-property rules pile on.

**Files:**
- `crates/zero-lint/src/rules/mod.rs` (new) — re-exports each rule's `fn
  check(decl: &Decl) -> Option<Diagnostic>`.
- `crates/zero-lint/src/rules/typography.rs` (new) — L01–L04.
- `crates/zero-lint/src/lib.rs` — `lint_project` iterates files → scans →
  pipes each `Decl` through every rule in `rules::ALL`.
- `crates/zero-lint/tests/fixtures/typography/*.scss` (new) — one
  `pass_*.scss` and one `fail_*.scss` per rule.
- `crates/zero-lint/tests/typography_rules.rs` (new).

**Changes:**
- Whitelist handling shared across all rules: a `is_whitelisted_value(s:
  &str) -> bool` helper that returns true for `0`, `0%`, `auto`, `none`,
  `inherit`, `initial`, `unset`, `currentColor`, `transparent`, anything
  matching `^var\(--[\w-]+\)$`, and `calc(...)` expressions whose tokens
  are *all* either `var(--…)` references or `0`. Calc expressions with a
  raw number do NOT pass — spec R2's "calc to mix raw values back in is the
  same failure mode."
- L01 (`font-weight`): split value into tokens; first token is either a
  keyword (`normal` → `--weight-normal`, `bold` → `--weight-bold`) or a
  numeric weight (`nearest(WEIGHT, n, Unitless)`).
- L02 (`font-size`): parse dimension, call `nearest(FONT_SIZE, ...)`. Append
  `", or consider a text-* utility for body/heading text"` to the message.
- L03 (`line-height`): unitless number → `nearest(LEADING, n, Unitless)`;
  dimension form → convert to ratio against 16px before matching.
- L04 (`letter-spacing`): parse dimension as `em`-equivalent (px values
  divided by the implied 16px to coerce to em-space), call
  `nearest(TRACKING, ...)`.

**Tests:**
- `typography_rules.rs` integration test: load each fixture, lint, assert
  exact `(rule, line, column, suggested-token)` for the `fail_*` cases and
  empty diagnostics for `pass_*`.
- Fixtures cover: numeric and keyword weights; px and rem font sizes;
  unitless and dimensional line-heights; px and em letter-spacing; the
  midpoint behavior; the "outside the scale" message.

---

### Step 5: Implement L05 (color literals)

**Goal:** Catch the only non-numeric rule. Color matching is its own beast —
the resolver works in Lab/sRGB space, not on a scalar axis — so isolate it
in one step.

**Files:**
- `crates/zero-lint/src/rules/color.rs` (new) — L05.
- `crates/zero-lint/src/tokens.rs` — add the `COLOR` table: thirteen
  semantic tokens with their *current-theme* sRGB values. Light-theme values
  are the source of truth for matching; the lint suggests the semantic name
  (e.g. `--color-primary`), which is theme-aware at runtime.
- `crates/zero-lint/tests/fixtures/color/*.scss`, plus
  `crates/zero-lint/tests/color_rule.rs`.

**Changes:**
- Properties matched: `color`, `background`, `background-color`,
  `border-color`, `fill`, `stroke`, `outline-color`. Match on exact property
  name (lowercased).
- Value parser handles: hex (`#rgb`, `#rrggbb`, `#rrggbbaa`),
  `rgb()`/`rgba()` (decimal or % channels), `hsl()`/`hsla()`, and CSS named
  colors (a small lookup table — top ~30 names covers what an agent would
  type: `white`, `black`, `red`, `blue`, …). For `background` shorthand,
  scan tokens and flag the first color-shaped token.
- Distance metric: convert both candidate and tokens to sRGB tuples and
  match on Euclidean distance over (R, G, B). Adequate for "did the agent
  type something semantically close to `--color-danger`?"; no perceptual
  color science needed.
- Whitelist override: never fire when the value contains `var(--color-…)`.

**Tests:**
- `flags_hex_background_to_color_primary`: a fixture with `background:
  #2563eb` lints to a `--color-primary` suggestion at the right line/col.
- `passes_var_color_reference`: `background: var(--color-bg)` produces no
  diagnostic.
- `passes_currentcolor_and_transparent`: each whitelist sentinel is silent.
- `flags_rgb_function_and_named_color`: `color: rgb(255, 0, 0)` and `color:
  red` both suggest `--color-danger` (or whichever is nearest in the table).

---

### Step 6: Implement L06–L10 (radius / border / spacing / margin / gap)

**Goal:** Land the remaining single-property numeric rules. They reuse
Step 3's resolver against the `RADIUS`, `BORDER`, `SPACE` scales and, for
two rules, also name a utility class as the secondary suggestion.

**Files:**
- `crates/zero-lint/src/rules/box_model.rs` (new) — L06–L10.
- `crates/zero-lint/tests/fixtures/box_model/*.scss`,
  `crates/zero-lint/tests/box_model_rules.rs`.

**Changes:**
- L06 (`border-radius`): parse dimension; if the value is `50%` or numeric
  ≥ 999px, suggest `--radius-3xl` (the dedicated branch); else
  `nearest(RADIUS, ...)`.
- L07 (`border-width` and the `border` / `border-{side}` shorthands):
  scan value tokens for the first dimension; resolve against `BORDER`.
  Message names both the token (`var(--border-thin)`) and the utility
  shorthand (`use the .border / .border-{t,r,b,l} utility instead of
  writing the shorthand inline`).
- L08 (`padding`, `padding-{top,right,bottom,left}`, `padding-{block,
  inline}`, and the two-axis `padding-{block,inline}-{start,end}` longhands):
  flag each numeric value. For one-value shorthands the suggestion names
  both `var(--space-md)` and `pad-md`. For multi-value forms, the message
  names tokens only.
- L09 (`margin` and longhands): same as L08 but no utility class to suggest.
- L10 (`gap`, `row-gap`, `column-gap`): nearest `SPACE` token + `gap-md`
  utility.

**Tests:**
- One pass/one fail fixture per rule. The shorthand cases need their own
  fixtures (`fail_border_shorthand.scss`, `fail_padding_two_value.scss`).
- A `calc_expression_with_raw_value` fixture covers the calc-injection case
  flagged by the whitelist helper.

---

### Step 7: Implement L11 (layout-primitive detection)

**Goal:** The one rule that operates on a *rule body* rather than a single
declaration. Implementation is conservative: every pattern must match all
its required declarations, and the rule fires once per matching selector
(not per declaration).

**Files:**
- `crates/zero-lint/src/rules/layout.rs` (new) — L11.
- `crates/zero-lint/src/lib.rs` — extend `lint_project` to feed `RuleBody`
  instances to body-shaped rules.
- `crates/zero-lint/tests/fixtures/layout/*.scss`,
  `crates/zero-lint/tests/layout_rule.rs`.

**Changes:**
- Six pattern matchers, each taking a `RuleBody` and returning
  `Option<&'static str>` (the suggested primitive class). Patterns mirror
  spec R3's table exactly:
  - `cluster`: `display: flex` + `flex-wrap: wrap` + any `gap`.
  - `stack`: `display: flex` + `flex-direction: column` + any `gap`.
  - `split`: `display: flex` + `justify-content: space-between`.
  - `flank`: `display: flex` + the body has a child entry whose declarations
    include `flex: 0 0 auto`.
  - `grid`: `display: grid` + a `grid-template-columns` value containing
    `repeat(` and `auto-fit` and `minmax`.
  - `frame`: any `aspect-ratio` value + `overflow: hidden`.
- Diagnostic message names the class and references AGENTS.md's "When to
  reach for which primitive" section by name (so the lint, the AGENTS.md
  table, and the framework spec all point at one phrasing).
- Each pattern is short-circuited by a `selector` filter: rules whose
  selector already contains `.cluster`, `.stack`, `.split`, etc. (which is
  how an override site looks) are skipped. This protects framework-internal
  partials and user overrides from being flagged.

**Tests:**
- One pass + one fail fixture per primitive. The `pass_*` fixtures use the
  primitive class directly; the `fail_*` fixtures replicate the body shape
  on a different selector.
- A `flank_misses_when_no_child_rule` test confirms the conservative
  matcher does not fire on `display: flex` alone.
- A `selector_with_primitive_class_is_skipped` test confirms `.cluster
  .my-item { display: flex; flex-wrap: wrap; gap: 1rem; }` does not fire.

---

### Step 8: Wire the `zero lint` subcommand

**Goal:** Make the lint runnable from the CLI: `zero lint` walks the
project, invokes `zero_lint::lint_project`, prints diagnostics in `zero
test`-shaped output, exits non-zero on any diagnostic.

**Files:**
- `crates/zero/Cargo.toml` (add `zero-lint = { path = "../zero-lint" }`)
- `crates/zero/src/main.rs` (add `Commands::Lint` variant + arm)
- `crates/zero/src/cmd/mod.rs` (add `pub mod lint;`)
- `crates/zero/src/cmd/lint.rs` (new)
- `crates/zero/tests/lint_smoke.rs` (new — minimal CLI invocation test)

**Changes:**
- `Commands::Lint { #[arg(long, short = 'q')] quiet: bool }`. No `--verbose`
  because verbose is the default; flag inversion keeps the surface small.
- `cmd::lint::run(quiet)` loads `Config::load_from_cwd`, resolves
  `config.project_root_path()`, calls `zero_lint::lint_project(&root)`,
  writes diagnostics to stderr, and exits 1 if any fired. Output shape per
  diagnostic:
  ```
  styles/app.scss:14:5  L01  font-weight: 600 — use var(--weight-semi)
        font-weight: 600;
        ^
  ```
  `--quiet` drops the source-snippet line and the caret. Implementation
  reads each file once and indexes line offsets to render the caret.
- The smoke test writes a minimal project under a tempdir, invokes the
  binary via `assert_cmd`, and asserts a known diagnostic appears.

**Tests:**
- `lint_smoke.rs`: write `web/styles/app.scss` with a single `font-weight:
  600` rule, assert exit code 1 and that stderr contains `L01` and
  `--weight-semi`.
- A second test asserts `--quiet` suppresses the snippet line.

---

### Step 9: AGENTS.md — negative examples + primitive intent

**Goal:** Land the documentation half of the feedback loop in the scaffold
source of truth, so newly initialized projects get it and existing projects
refresh via `zero update`.

**Files:**
- `crates/zero-scaffold/src/scaffold/AGENTS.md` (modify)
- `crates/zero-scaffold/src/lib.rs` (update section-sentinel test)

**Changes:**
- Insert `### When to reach for which primitive` immediately after the
  `#### Layout primitives` table inside `## Styles → Design system`. Body
  is the six-row table from spec R5a, with the wording preserved.
- Insert `## Common mistakes (the lint will catch these)` after `## Styles
  → Design system` ends and before `## Component library` begins.
  Structure:
  - One framing paragraph ("These are the patterns the design system
    replaces. `zero lint` flags them; this section is the answer.").
  - A "Don't write / Use" table with one row per rule L01–L11.
  - A pointer line: "See `issues/design-system-lint/spec.md` for the rule
    catalog. Reach for layout primitives by name — see "When to reach for
    which primitive" above for canonical intent."
- Update the `#### Spacing scale` and the `_tokens.scss` row in the partials
  table to mention the new radius scale entries (xs/xl/2xl/3xl).

**Tests:**
- Extend the existing `write_initial_project_agents_md_has_section_sentinels`
  test in `crates/zero-scaffold/src/lib.rs` with two new sentinels:
  `"## Common mistakes"` and `"### When to reach for which primitive"`.
- Add `agents_md_lists_all_lint_rules`: load AGENTS.md, assert every
  `L01`..`L11` string appears in the negative-examples table.

---

### Step 10: Framework spec §7.1 updates

**Goal:** Keep the framework spec in sync with the scaffold so both
documents describe the same surface.

**Files:**
- `zero-framework-spec.md` (modify §7.1)

**Changes:**
- In the §7.1 token table, expand the Radius row to list all seven steps.
- Add a new paragraph after the §7.1 "Layout primitives" line, titled
  "When to reach for which primitive," reproducing the same six-row table
  added to AGENTS.md in Step 9 (verbatim — the spec calls out that the
  table is mirrored so both audiences see one canonical phrasing).
- In §12 Phase 6, change the `zero lint` checkbox from `[ ]` to `[x]` with
  a sub-bullet listing the eleven shipped SCSS rules.

**Tests:**
- No automated tests for spec prose; a manual grep confirms the new
  sentinels (`--radius-3xl`, `When to reach for which primitive`) appear.

---

### Step 11: Audit + fix `examples/tracker/web` and `showcase/`

**Goal:** Run the freshly built `zero lint` against the in-repo example
apps, fix every diagnostic in place (or document why a flag is correct),
so the integration test in Step 12 can assert zero diagnostics. Also the
moment to decide L11.flank's fate per the spec's open question.

**Files:**
- `examples/tracker/web/styles/app.scss` (modify as needed)
- `showcase/styles/app.scss` (modify as needed)
- Possibly `crates/zero-lint/src/rules/layout.rs` if L11.flank produces
  noise and gets dropped.

**Changes:**
- Run `cargo run -p zero -- lint` from each project root. Expected initial
  hits based on a manual read of the two files:
  - `examples/tracker/web/styles/app.scss` uses `padding: var(--pad-sm)`
    and `margin-top: var(--gap-md)` — those reference *utility class
    names*, not tokens. L08/L09 will not flag them (they're
    `var(--…)` references) but the values are wrong CSS regardless.
    Replace with `var(--space-sm)` / `var(--space-md)` while we're here.
  - `showcase/styles/app.scss` looks clean on inspection.
- If L11.flank emits ≥1 false positive across the two projects, remove the
  flank pattern from `rules/layout.rs` and update the per-rule fixture +
  AGENTS.md table accordingly. Spec R3 explicitly authorizes this drop.

**Tests:**
- Step 12 codifies the zero-diagnostic assertion; this step's verification
  is interactive (`cargo run -p zero -- lint`) plus the existing
  example/showcase build tests staying green.

---

### Step 12: Integration tests

**Goal:** Lock in two end-to-end guarantees: the example projects lint
clean, and the patterns from `improved_agent_usage.md` reliably fire.

**Files:**
- `crates/zero/tests/lint_examples.rs` (new)
- `crates/zero/tests/fixtures/agent_failures/styles/app.scss` (new)
- `crates/zero/tests/lint_agent_failures.rs` (new)

**Changes:**
- `lint_examples.rs`: two tests — one per project — that copy
  `examples/tracker/web` / `showcase/` into a tempdir (the existing test
  helpers under `crates/zero/tests/common/mod.rs` already do this for
  build tests), run the binary via `assert_cmd`, and assert exit code 0
  + empty stderr.
- `lint_agent_failures.rs`: a single test that points the binary at a
  hand-crafted fixture containing every pattern from
  `improved_agent_usage.md` (`font-weight: 600`, `border-radius: 999px`,
  `font-size: 0.75rem`, raw `padding`, inline `display: flex; gap: …`)
  and asserts that the diagnostics include at least one of each rule
  `L01`, `L02`, `L06`, `L08`, and `L11.cluster`.

**Tests:**
- The two integration tests above are themselves the verification. They
  run under `cargo test -p zero`.

---

## Risks and Assumptions

- **The SCSS scanner is not a real parser.** It works for the rule shapes
  the spec catalogs and the way users actually author SCSS (selectors,
  declarations, nesting). Edge cases that could trip it: deeply nested
  `@if` / `@for` blocks, SCSS interpolation inside property names
  (`#{$prefix}-color: …`), or multi-line declarations split across
  comments. Assumption: none of these appear in user-authored SCSS in the
  audited projects; if Step 11 surfaces a parser miss, fix the scanner
  there. A full SCSS AST is out of scope.
- **Color distance is sRGB-Euclidean.** Adequate for "agent typed something
  close to a semantic token"; will misrank perceptually-distant-but-numerically-close
  pairs. If L05 produces visibly-wrong suggestions in Step 11's audit,
  drop to a "did the value match *any* var(--color-*) in the table"
  heuristic (no nearest-match, just "use a semantic color token").
- **Theme-aware token suggestions for color.** L05 suggests semantic names
  based on the *light* theme palette. The semantic surface is identical
  across themes (same thirteen tokens), so the recommendation is
  theme-correct even if the dark-theme value differs.
- **Radius renaming is a breaking change for users.** `--radius-pill` is
  publicly documented today; renaming to `--radius-3xl` will break any
  user SCSS that reads the old name. Acceptable because (a) the project
  is pre-1.0, (b) `zero update` rewrites `.zero/` (so user files keep
  compiling but reference a missing token until the user fixes it), and
  (c) the spec explicitly forbids the semantic alias. The audit in
  Step 11 catches the in-tree consumers; document the breaking change in
  the commit message for Step 1.
- **L11.flank may be too noisy to ship.** If Step 11 confirms this,
  dropping it doesn't reduce the value of L11 materially — the other five
  patterns cover the failure mode the agent demonstrated (raw `display:
  flex; gap` for clusters and stacks).
- **`improved_agent_usage.md` test fixture must stay in sync with the
  rules.** If a future rule revision changes the suggested token, the
  fixture's expected suggestions update too. The assertion in Step 12
  checks rule IDs, not exact suggestion strings, to minimize this churn.
