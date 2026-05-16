# Spec: Design system expansion — alignment, justify, self, text, and flex-direction utilities

## Problem Statement

The design system shipped in `issues/design-system/spec.md` and refined under `issues/update/spec.md` gives projects layout primitives (`cluster`, `stack`, `frame`, `split`, `flank`, `grid`) plus three small utility families (`gap-*`, `pad-*`, `border*`). That floor handles "how do children of this container space themselves" but does nothing for "where, inside the container, do those children sit." Every real application immediately reaches for `align-items`, `justify-content`, `text-align`, and the occasional `flex-direction` flip. Without utility classes, users either write per-component CSS for each occurrence or write inline `style="..."` — both of which the design system was supposed to eliminate.

Phase 8 of the framework roadmap closes that gap by adding alignment, justification, per-child self-alignment, text alignment, and flex-direction utilities to the framework-owned design-system layer under `.zero/styles/`. The classes ship as a new SCSS partial that the framework aggregate `@use`s, distributed and refreshable through the existing `zero update` plumbing. The result is the next layer of the foundation the future component library will assume exists.

## Background

### What exists today

- **`.zero/styles/_utilities.scss`** — fifteen classes today: `gap-{xs,sm,md,lg,xl}`, `pad-{xs,sm,md,lg,xl}`, `border`, `border-{t,r,b,l}`. Each is a single CSS rule. No `!important`. Override is by class-list order.
- **`.zero/styles/_layout.scss`** — six layout primitive classes. Each has a sensible default for its primary spacing slot and (where relevant) its alignment. For example `.cluster` defaults to `align-items: center; flex-wrap: wrap`. Overrides happen via utility classes that appear later in the source order.
- **`.zero/styles/zero.scss`** — the aggregate. Today reads:
  ```scss
  @use 'tokens';
  @use 'base';
  @use 'layout';
  @use 'utilities';
  ```
- **`.zero/styles/_tokens.scss`** — seven token categories declared as CSS custom properties on `:root`. No SCSS variables for tokens. Not relevant to this expansion (no new tokens).
- **Distribution model (Phase 7).** Framework-owned partials live under `.zero/`, are git-ignored, and refresh via `zero update`. Adding a new partial is a matter of:
  1. Add the source file under `src/scaffold/.zero/styles/`.
  2. Add a `TPL_*` `include_str!` constant in `src/scaffold.rs`.
  3. Add the `(path, content)` tuple to `framework_manifest()`.
  4. Add `@use 'name';` to the aggregate `zero.scss`.
  5. Update the manifest-length assertion in `src/scaffold.rs` tests.
- **Naming policy.** The design system owns flat, unprefixed class names in the global namespace. Twenty-one identifiers today across layout primitives and utility classes; this expansion adds 27 more. Documented in `issues/design-system/spec.md` as a deliberate cost the user accepts in exchange for short class names.

### Decisions already made in the refine session

- **Naming convention: long, property-mirroring.** `align-self-start`, `justify-self-end`, `flex-row`, `flex-col`, `flex-row-reverse`, `flex-col-reverse`. Mirrors the underlying CSS property names. Avoids namespace collisions on bare `row`/`col` and disambiguates the parent-vs-child versions of align/justify.
- **Audit scope.** v1 ships align-items, justify-content, align-self, justify-self, text-align, and flex-direction utility families. Wrap utilities, `place-*` shorthands, `align-content`, and grid-area helpers are explicitly out of scope.
- **Text alignment is logical-only.** `text-start`, `text-center`, `text-end` using `text-align: start | center | end` (writing-mode-aware). No `text-left` / `text-right`. No `text-justify`.
- **File organization.** One new partial `_alignment.scss`. The name covers the dominant theme; text-align and flex-direction ride along rather than getting their own partials. The aggregate `zero.scss` gains one new `@use 'alignment';` line.

### Why not `flex-start` / `flex-end` for the align-* values

CSS now accepts the directional keywords `start` and `end` on `align-items`, `justify-content`, and their `-self` variants in both grid **and** flex contexts on all evergreen browsers (Safari 16.4+, released March 2023). Using `start`/`end` everywhere keeps the class names short and consistent across families. The older `flex-start`/`flex-end` aliases are unnecessary. The constraint in `issues/design-system/spec.md` ("no vendor prefixing, evergreen browsers") authorizes this.

### Why no `stretch` on `justify-content`

`justify-content: stretch` is valid CSS but has no flex/grid effect for the parent — `stretch` on the main axis is what grid `1fr` columns already do, and flex has no equivalent main-axis stretch. The four `justify-self-*` classes do include `stretch` because `justify-self: stretch` is the default for grid items and is occasionally useful to *restore* after another rule has overridden it.

### Why no `baseline` on `justify-*` / `justify-self-*`

`baseline` is a cross-axis-only concept. `justify-content: baseline` is invalid; `justify-self: baseline` resolves to `start`. The class would be dead weight.

## Requirements

### Class surface

1. A new partial `src/scaffold/.zero/styles/_alignment.scss` declares the following 27 classes. Each class is a single CSS rule with one property declaration. Comments at the top of each family group explain the family's purpose.

   **align-items (5):**
   - `.align-start    { align-items: start; }`
   - `.align-center   { align-items: center; }`
   - `.align-end      { align-items: end; }`
   - `.align-stretch  { align-items: stretch; }`
   - `.align-baseline { align-items: baseline; }`

   **justify-content (6):**
   - `.justify-start   { justify-content: start; }`
   - `.justify-center  { justify-content: center; }`
   - `.justify-end     { justify-content: end; }`
   - `.justify-between { justify-content: space-between; }`
   - `.justify-around  { justify-content: space-around; }`
   - `.justify-evenly  { justify-content: space-evenly; }`

   **align-self (5):**
   - `.align-self-start    { align-self: start; }`
   - `.align-self-center   { align-self: center; }`
   - `.align-self-end      { align-self: end; }`
   - `.align-self-stretch  { align-self: stretch; }`
   - `.align-self-baseline { align-self: baseline; }`

   **justify-self (4):**
   - `.justify-self-start   { justify-self: start; }`
   - `.justify-self-center  { justify-self: center; }`
   - `.justify-self-end     { justify-self: end; }`
   - `.justify-self-stretch { justify-self: stretch; }`

   **text-align (3):**
   - `.text-start  { text-align: start; }`
   - `.text-center { text-align: center; }`
   - `.text-end    { text-align: end; }`

   **flex-direction (4):**
   - `.flex-row         { flex-direction: row; }`
   - `.flex-row-reverse { flex-direction: row-reverse; }`
   - `.flex-col         { flex-direction: column; }`
   - `.flex-col-reverse { flex-direction: column-reverse; }`

2. Family groups are separated by a single blank line and a single-line comment introducing the group (e.g. `// Cross-axis alignment of children (align-items)`).
3. No `!important` anywhere in the partial. Override is by source order — `_alignment.scss` is `@use`d after `_layout.scss`, so a class like `.align-stretch` correctly overrides `.cluster`'s default `align-items: center` when used together (`class="cluster align-stretch"`).
4. No additional CSS custom properties are introduced. No new tokens. The utilities consume layout primitives' existing defaults via override.

### Aggregate `zero.scss`

5. `src/scaffold/.zero/styles/zero.scss` gains exactly one new line, in this order:
   ```scss
   @use 'tokens';
   @use 'base';
   @use 'layout';
   @use 'utilities';
   @use 'alignment';
   ```
   Order matters: `alignment` is last so its rules win over any same-property defaults in `layout` or `utilities`. (`_utilities.scss` does not set `align-*` or `justify-*`, but ordering after `layout` is what enables override of primitive defaults.)

### Scaffold registration (`src/scaffold.rs`)

6. A new constant `TPL_ALIGNMENT_SCSS` is added next to the other `TPL_*_SCSS` constants:
   ```rust
   const TPL_ALIGNMENT_SCSS: &str = include_str!("scaffold/.zero/styles/_alignment.scss");
   ```
7. `framework_manifest()` gains one new entry, inserted alphabetically before `zero.scss` (after `_utilities.scss`):
   ```rust
   (".zero/styles/_alignment.scss", TPL_ALIGNMENT_SCSS),
   ```
   The total manifest length becomes 8.
8. The existing test `framework_manifest_lists_seven_files` is renamed to `framework_manifest_lists_eight_files`, the assertion `assert_eq!(manifest.len(), 7, ...)` becomes `assert_eq!(manifest.len(), 8, ...)`, and `.zero/styles/_alignment.scss` is added to the expected-path set.
9. A new test in the `tests` module of `src/scaffold.rs` asserts that the alignment partial is emitted and contains one representative class per family. Suggested name: `alignment_scss_contains_each_family`. Assertions:
   - File exists at `.zero/styles/_alignment.scss` and is non-empty.
   - Contains `.align-start {`
   - Contains `.justify-between {`
   - Contains `.align-self-stretch {`
   - Contains `.justify-self-center {`
   - Contains `.text-center {`
   - Contains `.flex-col-reverse {`
   - Does **not** contain `!important`.
10. The existing test `zero_scss_contains_aggregate_uses` is extended to also assert `@use 'alignment'` appears in `zero.scss`.

### Integration coverage

11. The end-to-end test `tests/design_system.rs` (or its current equivalent — confirm path in the plan phase) is extended so that the compiled CSS served at `/styles/app.scss` by `zero dev` (and the equivalent build output) contains at least one class from each new family: `.align-start`, `.justify-between`, `.align-self-stretch`, `.justify-self-center`, `.text-center`, `.flex-col-reverse`. The existing assertions for `.cluster`, `.gap-md`, etc. remain unchanged.
12. No new integration test file is added unless the existing one cannot be extended cleanly — the plan phase confirms.

### Scaffold demo

13a. `src/scaffold/src/routes/home.ts` is updated so the rendered template demonstrates at least one new utility class — at minimum `align-center` applied to an appropriate flex/grid primitive (e.g. extend the existing `class="stack pad-xl"` to `class="stack pad-xl align-center"`, or add a `cluster align-center` row). The existing assertion `home_ts.contains("class=\"stack pad-xl\"")` in `write_initial_project_emits_user_files` is updated to match the new class list, and a new assertion confirms the presence of `align-center` (or whichever new utility the plan picks). The change exists so first-touch users see the expansion in the scaffolded output, not just in the partial.

### Documentation

13. `src/scaffold/AGENTS.md` — the `## Styles` section's utility/primitive pointer tables gain new entries documenting the six new families with their value lists. One table row per family (not per class). Suggested format mirrors the existing `gap-*` row:
    - `align-*` — sets `align-items` on a flex/grid container. Values: `start`, `center`, `end`, `stretch`, `baseline`.
    - `justify-*` — sets `justify-content`. Values: `start`, `center`, `end`, `between`, `around`, `evenly`.
    - `align-self-*` — per-child override of cross-axis alignment. Values: `start`, `center`, `end`, `stretch`, `baseline`.
    - `justify-self-*` — per-child override of main-axis alignment (grid). Values: `start`, `center`, `end`, `stretch`.
    - `text-*` — sets `text-align` using logical values. Values: `start`, `center`, `end`.
    - `flex-row` / `flex-row-reverse` / `flex-col` / `flex-col-reverse` — override the flex-direction of a flex container (e.g. flip `cluster` or `flank` to vertical or right-to-left).
14. `zero-framework-spec.md` §7.1 "Design system" — the **Utility families** paragraph is extended to mention the new families. The existing three (`gap-*`, `pad-*`, `border` / `border-{t,r,b,l}`) remain; six new family entries are appended. Total: 9 utility families, 42 utility classes (15 existing + 27 new). The exact wording is drafted in the plan phase to match the prose style of the existing section.
15. `zero-framework-spec.md` §12 — the Phase 8 checklist items are marked `[x]` after implementation lands. (Implementation, not the plan, performs this — listed here for completeness.)

### Out-of-band invariants

16. The existing test `write_initial_project_emits_framework_files` continues to assert all framework files are emitted; no new assertion is required there beyond what (9) covers.
17. `zero update` — no special handling required. The new partial appears in `framework_manifest()`, so `zero update` will offer to **add** it to existing projects on the next run. Tests in the update flow (e.g. anything under `src/cmd/update.rs` or its integration tests) should be reviewed in the plan phase to confirm no length-coupled assertions break.

## Constraints

- **No new Rust dependencies.** Rides on the existing `grass` SCSS pipeline.
- **No new CSS custom properties.** The expansion consumes existing tokens and consumes the cascade for overrides; it does not introduce theme-able knobs of its own.
- **No `!important`.** Same constraint as the existing utility surface. Source order + class-list order is the override mechanism.
- **No physical-direction text alignment.** `text-start`/`text-end` only — `text-left`/`text-right` are excluded to keep RTL behavior correct.
- **No `flex-start` / `flex-end` aliases.** The class names use the modern `start`/`end` keywords throughout. Evergreen-browser support is the framework-wide assumption.
- **No vendor prefixing.** Unchanged framework-wide constraint.
- **No `place-*` shorthands.** `place-items`, `place-content`, `place-self` are deferred — users compose `align-* justify-*` (or the self/grid variants) at the use site.
- **No `align-content` family.** `align-content` controls cross-axis distribution of *lines* in multi-line flex/grid containers. Out of scope for v1; revisit when a concrete use case appears.
- **No wrap utilities (`flex-wrap`, `flex-nowrap`, `flex-wrap-reverse`).** The layout primitives that wrap (`cluster`, `flank`) do so by default; users who need to override wrap behavior write a one-off rule. Revisit if demand materializes.
- **No `flex-grow` / `flex-shrink` / `flex-basis` utilities.** Per-child flex behavior beyond `align-self` is out of scope.
- **No gap-x/gap-y axis-specific utilities.** Same constraint as the original design-system spec; restated.
- **Framework-owned, refreshable.** The partial lives under `.zero/styles/`. Users who want to customize override at the cascade level, not by editing the partial. `zero update` refreshes from the binary.
- **One new partial only.** `_alignment.scss` carries all six new families. No `_text.scss`, no `_flex.scss`.
- **No JavaScript.** Pure CSS expansion. No helpers, no class toggles.

## Out of Scope

- **`place-items`, `place-content`, `place-self` shorthand utilities.** Compose with the long forms.
- **`align-content` utilities.** Multi-line flex/grid cross-axis distribution. Future expansion if demand appears.
- **`flex-wrap` / `flex-nowrap` / `flex-wrap-reverse` utilities.** Primitives default to wrap; override is a one-off.
- **`flex-grow-*` / `flex-shrink-*` / `flex-basis-*` utilities.** Per-child sizing is out for v1.
- **`gap-x-*` / `gap-y-*` / `pad-x-*` / `pad-y-*` axis-specific utilities.** Restated from the original spec.
- **Margin utilities (`m-*`, `mx-*`, `my-*`).** Restated from the original spec; margin is not how the design system spaces things.
- **Physical-direction text alignment (`text-left`, `text-right`).** Logical-only.
- **`text-justify`.** Long-form-content concern; not in v1.
- **`vertical-align-*` utilities.** Table-cell / inline-element use case; not in v1.
- **`writing-mode-*` utilities.** Out of v1; users set on `<html>` directly if needed.
- **New layout primitives.** No `cover`, `switcher`, or right-flank variant. Same exclusion as the original spec.
- **New tokens.** No new spacing, color, radius, font, shadow, or border tokens.
- **JavaScript helpers.** No `setAlign()` or class-toggling utilities.
- **Migration tooling.** Existing projects pick up the new partial via `zero update`; no special migration step.
- **Component library.** Buttons, forms, etc. — Phase 9.

## Open Questions

- **Exact test file location for the integration assertion.** The original design-system spec proposed `tests/design_system.rs`. The plan phase should confirm the file exists at that path today and extend it; if it lives elsewhere or was rolled into a broader integration file, the plan picks the right home.
- **Exact placement of the new utility in `home.ts`.** Resolved at the spec level: the scaffold must demonstrate at least one new utility (e.g. `align-center`). The plan phase picks the concrete class-list edit and updates the corresponding assertion in `write_initial_project_emits_user_files`. See Requirement 13a.
- **AGENTS.md exact prose.** The spec lists the six new family rows but does not produce the final wording. The plan phase drafts the exact diff.
- **Framework-spec §7.1 exact prose.** Same as above — the spec names the change but does not write it.
- **Whether `zero update` test coverage needs new assertions.** The Add operation is exercised by existing tests at a generic level (any new manifest entry produces an Add op). The plan phase confirms no length-coupled assertion exists that would silently keep passing while ignoring the new file, and adds a targeted test if needed.
- **Roadmap update bookkeeping.** Marking the Phase 8 checklist items `[x]` in `zero-framework-spec.md` is part of *implementation*, not the plan. Listed in Requirements (15) for completeness but should not appear in the plan's task list.
