# Plan: Design system expansion — alignment, justify, self, text, and flex-direction utilities

## Summary

Add a sixth framework-owned SCSS partial, `.zero/styles/_alignment.scss`, that ships 27 new utility classes across six families (`align-*`, `justify-*`, `align-self-*`, `justify-self-*`, `text-*`, `flex-*`). The partial is wired into the existing aggregate `zero.scss` after `_utilities.scss` so source-order overrides Just Work. Distribution rides on the Phase 7 manifest plumbing: one new `include_str!` constant in `src/scaffold.rs`, one new `framework_manifest()` entry, an updated count test, a focused content test for the new partial, an extended aggregate-uses test, a `home.ts` demo touch, an extended design-system integration test, and prose updates in `AGENTS.md` and `zero-framework-spec.md` §7.1.

## Prerequisites

None. The spec resolves its own open questions (text alignment is logical-only, file name is `_alignment.scss`, naming convention is the long property-mirroring form). The "open questions" in the spec are plan-phase decisions, all resolved below:

- **Integration test file location:** `tests/design_system.rs` exists; extend `build_emits_design_system_css` (it already asserts on the same compiled-CSS artifact).
- **Concrete `home.ts` demo edit:** extend `<main class="stack pad-xl">` to `<main class="stack pad-xl align-center">`. Minimal diff, exercises the dominant `align-*` family.
- **AGENTS.md exact prose:** drafted in Step 5 below.
- **Framework-spec §7.1 exact prose:** drafted in Step 5 below.
- **`zero update` test coverage:** the manifest-length-coupled assertion lives in `src/scaffold.rs::framework_manifest_lists_seven_files` and is renamed/updated in Step 2; the existing `update_with_empty_dot_zero_dir_proposes_only_adds` test reads the manifest length dynamically and will pick up the new file automatically with no source change.

## Steps

- [x] **Step 1: Author the `_alignment.scss` partial**
- [x] **Step 2: Register the partial in scaffold + aggregate + tests**
- [x] **Step 3: Demonstrate the expansion in the `home.ts` scaffold**
- [x] **Step 4: Extend the design-system integration test**
- [x] **Step 5: Documentation (AGENTS.md, framework spec §7.1, Phase 8 checklist)**

---

## Step Details

### Step 1: Author the `_alignment.scss` partial
**Goal:** Create the SCSS source-of-truth for the 27 new utility classes. Putting the file in place first keeps the scaffold-registration step (Step 2) a pure plumbing change.

**Files:**
- Create `src/scaffold/.zero/styles/_alignment.scss`.

**Changes:**

Write exactly 27 class rules, grouped into six family blocks separated by a single blank line, each block introduced by a single-line `//` comment. No `!important`. No CSS custom properties. Each rule is a single declaration. File body:

```scss
// Alignment, justification, per-child self-alignment, text alignment, and
// flex-direction utilities. Override is by source order — this partial is
// @use'd after _layout.scss and _utilities.scss in zero.scss, so e.g.
// `class="cluster align-stretch"` overrides .cluster's default
// `align-items: center`.

// Cross-axis alignment of children (align-items)
.align-start    { align-items: start; }
.align-center   { align-items: center; }
.align-end      { align-items: end; }
.align-stretch  { align-items: stretch; }
.align-baseline { align-items: baseline; }

// Main-axis distribution of children (justify-content)
.justify-start   { justify-content: start; }
.justify-center  { justify-content: center; }
.justify-end     { justify-content: end; }
.justify-between { justify-content: space-between; }
.justify-around  { justify-content: space-around; }
.justify-evenly  { justify-content: space-evenly; }

// Per-child cross-axis alignment (align-self)
.align-self-start    { align-self: start; }
.align-self-center   { align-self: center; }
.align-self-end      { align-self: end; }
.align-self-stretch  { align-self: stretch; }
.align-self-baseline { align-self: baseline; }

// Per-child main-axis alignment (justify-self; grid only)
.justify-self-start   { justify-self: start; }
.justify-self-center  { justify-self: center; }
.justify-self-end     { justify-self: end; }
.justify-self-stretch { justify-self: stretch; }

// Logical text alignment (writing-mode-aware)
.text-start  { text-align: start; }
.text-center { text-align: center; }
.text-end    { text-align: end; }

// Flex-direction overrides
.flex-row         { flex-direction: row; }
.flex-row-reverse { flex-direction: row-reverse; }
.flex-col         { flex-direction: column; }
.flex-col-reverse { flex-direction: column-reverse; }
```

**Tests:**

None yet — this step adds an unreferenced source file. Existing tests continue to pass because nothing else has changed:
- `cargo test` still passes (manifest still has 7 entries; the new file is not embedded yet).
- `node --test runtime/*.test.js` is unaffected.

### Step 2: Register the partial in scaffold + aggregate + tests
**Goal:** Make the new partial part of the framework manifest so it ships with `zero init`, refreshes via `zero update`, and compiles into the user's CSS via the aggregate. Update existing tests that are length-coupled and add the focused content test.

**Files:**
- Modify `src/scaffold/.zero/styles/zero.scss`
- Modify `src/scaffold.rs`

**Changes:**

1. **`src/scaffold/.zero/styles/zero.scss`** — append one line. Final contents:

```scss
@use 'tokens';
@use 'base';
@use 'layout';
@use 'utilities';
@use 'alignment';
```

Order matters: `alignment` is last so its rules win over same-property defaults set in `_layout.scss` (e.g. `.cluster { align-items: center; }`).

2. **`src/scaffold.rs`** — three discrete edits:

a. Add a new `TPL_*` constant alongside the others (between `TPL_UTILITIES_SCSS` and `TPL_ZERO_SCSS`, to keep alphabetical-ish grouping with the other style partials):

```rust
const TPL_ALIGNMENT_SCSS: &str = include_str!("scaffold/.zero/styles/_alignment.scss");
```

b. Insert a new entry into `framework_manifest()`. Insert it immediately after `_utilities.scss` and before `zero.scss` (preserving the existing topological order: tokens → base → layout → utilities → alignment → aggregate):

```rust
(".zero/styles/_utilities.scss", TPL_UTILITIES_SCSS),
(".zero/styles/_alignment.scss", TPL_ALIGNMENT_SCSS),
(".zero/styles/zero.scss", TPL_ZERO_SCSS),
```

The vec now has 8 entries.

c. Update tests in the `#[cfg(test)] mod tests` block:

- **Rename** `framework_manifest_lists_seven_files` → `framework_manifest_lists_eight_files`. Update its body: `assert_eq!(manifest.len(), 7, …)` → `assert_eq!(manifest.len(), 8, …)`. Add `".zero/styles/_alignment.scss"` to the expected-path set.
- **Extend** `zero_scss_contains_aggregate_uses` to also assert `"@use 'alignment'"` appears in `zero.scss`. Add it to the existing array of needles.
- **Add** a new test `alignment_scss_contains_each_family`:

```rust
#[test]
fn alignment_scss_contains_each_family() {
    let (_dir, root) = fresh_scaffold();
    let alignment =
        fs::read_to_string(root.join(".zero/styles/_alignment.scss")).unwrap();
    assert!(!alignment.is_empty(), "_alignment.scss is empty");
    for needle in [
        ".align-start {",
        ".justify-between {",
        ".align-self-stretch {",
        ".justify-self-center {",
        ".text-center {",
        ".flex-col-reverse {",
    ] {
        assert!(
            alignment.contains(needle),
            "_alignment.scss missing {needle}: {alignment}"
        );
    }
    assert!(
        !alignment.contains("!important"),
        "_alignment.scss must not use !important: {alignment}"
    );
}
```

**Tests:**

- The renamed `framework_manifest_lists_eight_files` test verifies count + path-set.
- The new `alignment_scss_contains_each_family` test verifies one representative class per family is emitted, and that the partial does not contain `!important`.
- The extended `zero_scss_contains_aggregate_uses` test verifies the aggregate `@use`s the new partial.
- `write_initial_project_emits_framework_files` continues to pass unchanged — it walks the manifest dynamically (it asserts the existing files but doesn't enumerate by count); add one new line asserting the alignment partial is emitted and non-empty, mirroring the pattern used for the other style partials:

```rust
let alignment_scss = fs::read_to_string(root.join(".zero/styles/_alignment.scss")).unwrap();
assert!(!alignment_scss.is_empty());
```

- `write_framework_files_writes_only_dot_zero` continues to pass — it iterates `framework_manifest()` dynamically and asserts each file exists; no change needed.
- `tests/update.rs` and `src/cmd/update.rs::tests` rely only on dynamic manifest lengths; they continue to pass unchanged.

Run `cargo test` after this step. All tests must pass.

### Step 3: Demonstrate the expansion in the `home.ts` scaffold
**Goal:** First-touch users see at least one new utility class in the rendered scaffold output, not just in the partial. Per spec Requirement 13a, the chosen class is `align-center` applied to the existing `<main class="stack pad-xl">`.

**Files:**
- Modify `src/scaffold/src/routes/home.ts`
- Modify `src/scaffold.rs` (the `write_initial_project_emits_user_files` test assertion)
- Modify `tests/design_system.rs` (the `scaffold_home_uses_design_system_classes` test assertion)

**Changes:**

1. **`src/scaffold/src/routes/home.ts`** — change line 10 from

```ts
    <main class="stack pad-xl">
```

to

```ts
    <main class="stack pad-xl align-center">
```

All other lines unchanged.

2. **`src/scaffold.rs`** — update `write_initial_project_emits_user_files`. Replace the existing assertion:

```rust
assert!(
    home_ts.contains("class=\"stack pad-xl\""),
    "home.ts missing design-system classes: {home_ts}"
);
```

with:

```rust
assert!(
    home_ts.contains("class=\"stack pad-xl align-center\""),
    "home.ts missing design-system classes: {home_ts}"
);
assert!(
    home_ts.contains("align-center"),
    "home.ts missing alignment demo class: {home_ts}"
);
```

(The second assertion is intentionally a separate check per Requirement 13a, even though it is implied by the first — it documents intent and survives any future reordering of the class list.)

3. **`tests/design_system.rs`** — update `scaffold_home_uses_design_system_classes`. Replace the existing assertion:

```rust
assert!(
    home_ts.contains("class=\"stack pad-xl\""),
    "home.ts missing stack pad-xl: {home_ts}"
);
```

with:

```rust
assert!(
    home_ts.contains("class=\"stack pad-xl align-center\""),
    "home.ts missing stack pad-xl align-center: {home_ts}"
);
```

Leave the other two assertions (`cluster gap-md`, `pad-sm border`) untouched.

**Tests:**

- `write_initial_project_emits_user_files` (passes with the new assertions).
- `scaffold_home_uses_design_system_classes` (passes with the updated assertion).
- `src/scaffold/src/routes/home.test.ts` continues to pass — it asserts the count text, not the class list.

Run `cargo test` after this step. Run `node --test` is not affected; the scaffold's own `home.test.ts` runs only when a user runs `zero test` against the scaffolded project (not part of CI here).

### Step 4: Extend the design-system integration test
**Goal:** Lock in the end-to-end guarantee that the compiled CSS contains at least one class from each new family. This is the only test that exercises the full `grass` SCSS pipeline through `zero build` for the alignment partial.

**Files:**
- Modify `tests/design_system.rs`

**Changes:**

Extend the existing `build_emits_design_system_css` test. After the existing block of `.cluster`, `.stack`, `.gap-md`, `.border`, `.border-t` assertions and before the dark-mode media query assertion, add one assertion per new family. Each follows the same `class { ` / `class{` either-spacing pattern used by the existing assertions:

```rust
assert!(
    css.contains(".align-start {") || css.contains(".align-start{"),
    "compiled CSS missing .align-start: {css}"
);
assert!(
    css.contains(".justify-between {") || css.contains(".justify-between{"),
    "compiled CSS missing .justify-between: {css}"
);
assert!(
    css.contains(".align-self-stretch {") || css.contains(".align-self-stretch{"),
    "compiled CSS missing .align-self-stretch: {css}"
);
assert!(
    css.contains(".justify-self-center {") || css.contains(".justify-self-center{"),
    "compiled CSS missing .justify-self-center: {css}"
);
assert!(
    css.contains(".text-center {") || css.contains(".text-center{"),
    "compiled CSS missing .text-center: {css}"
);
assert!(
    css.contains(".flex-col-reverse {") || css.contains(".flex-col-reverse{"),
    "compiled CSS missing .flex-col-reverse: {css}"
);
```

No new test function is added — the existing test is the right home per Requirement 12.

**Tests:**

- `build_emits_design_system_css` (extended; passes when the SCSS pipeline emits all six representative classes).
- `build_design_system_passes_contrast_smoke` (unchanged).
- `scaffold_home_uses_design_system_classes` (already updated in Step 3).

Run `cargo test --test design_system` after this step.

### Step 5: Documentation (AGENTS.md, framework spec §7.1, Phase 8 checklist)
**Goal:** Document the new families in the two places users and agents will look — the scaffolded `AGENTS.md` (user-facing reference) and `zero-framework-spec.md` §7.1 (framework spec) — and mark the Phase 8 roadmap items complete.

**Files:**
- Modify `src/scaffold/AGENTS.md`
- Modify `zero-framework-spec.md`

**Changes:**

1. **`src/scaffold/AGENTS.md`** — three edits in the `## Styles` → `### Design system` subsection:

   a. **Update the partials table** (currently lines ~518–523) so the `_utilities.scss` row reflects the present utility families and the new `_alignment.scss` row is inserted before `zero.scss`. The "What it declares" cell for `_utilities.scss` stays the same (`gap-*`, `pad-*`, `border-*`). Add:

   ```markdown
   | `_alignment.scss` | Twenty-seven utility classes across six families: `align-*`, `justify-*`, `align-self-*`, `justify-self-*`, `text-*`, `flex-{row,row-reverse,col,col-reverse}`. |
   ```

   b. **Add a new subsection** after the existing `#### Border utilities` subsection and before `#### Theme switching`. Title: `#### Alignment, justification, and direction`. Body — one paragraph then six rows:

   ```markdown
   #### Alignment, justification, and direction

   Six families of single-property utilities live in `_alignment.scss`. They override the layout primitives' defaults by class-list order: `class="cluster align-stretch"` cancels `.cluster`'s default `align-items: center`.

   | Family | Property | Values |
   | --- | --- | --- |
   | `align-*` | `align-items` (on a flex/grid container) | `start`, `center`, `end`, `stretch`, `baseline` |
   | `justify-*` | `justify-content` (on a flex/grid container) | `start`, `center`, `end`, `between`, `around`, `evenly` |
   | `align-self-*` | `align-self` (on a flex/grid child) | `start`, `center`, `end`, `stretch`, `baseline` |
   | `justify-self-*` | `justify-self` (on a grid child) | `start`, `center`, `end`, `stretch` |
   | `text-*` | `text-align` (logical, writing-mode-aware) | `start`, `center`, `end` |
   | `flex-row` / `flex-row-reverse` / `flex-col` / `flex-col-reverse` | `flex-direction` (flip `cluster`, `flank`, etc.) | — |

   No `flex-left`/`flex-end`/physical-direction aliases. `text-justify`, `place-*` shorthands, `align-content`, and wrap utilities are intentionally out of v1.
   ```

   c. **Update the `## The .zero/ directory` files table** (currently lines ~582–590) to add a new row for the alignment partial. Insert between `_utilities.scss` and `zero.scss`:

   ```markdown
   | `.zero/styles/_alignment.scss` | Alignment, justify, self, text-align, and flex-direction utility classes. |
   ```

2. **`zero-framework-spec.md` §7.1** — extend the **Utility families** paragraph (line 877). Replace the existing single-paragraph sentence:

   ```markdown
   **Utility families.** Three families in `_utilities.scss`: `gap-{step}` (5 classes), `pad-{step}` (5 classes), `border` / `border-{t,r,b,l}` (5 classes). No `!important`; override is by class-list order.
   ```

   with an extended version that names the new families and updates the count:

   ```markdown
   **Utility families.** Nine families across two partials, 42 utility classes total. `_utilities.scss`: `gap-{step}` (5), `pad-{step}` (5), `border` / `border-{t,r,b,l}` (5). `_alignment.scss`: `align-{start,center,end,stretch,baseline}` (5), `justify-{start,center,end,between,around,evenly}` (6), `align-self-{start,center,end,stretch,baseline}` (5), `justify-self-{start,center,end,stretch}` (4), `text-{start,center,end}` (3, logical-only), `flex-{row,row-reverse,col,col-reverse}` (4). No `!important`; override is by class-list order, and `_alignment.scss` is `@use`d after `_utilities.scss` in the aggregate so its rules win where they touch the same property.
   ```

   Also update the surrounding sentence (line 858) that names the four partials, to name five:

   - Existing: `four partials (\`_tokens.scss\`, \`_base.scss\`, \`_layout.scss\`, \`_utilities.scss\`)`
   - New: `five partials (\`_tokens.scss\`, \`_base.scss\`, \`_layout.scss\`, \`_utilities.scss\`, \`_alignment.scss\`)`

3. **`zero-framework-spec.md` §12 — Phase 8 checklist.** Mark the first three Phase 8 items `[x]` (lines 1241–1243). The fourth item (`Distribution rides on Phase 7…`) is also `[x]` once the partial ships through the manifest:

   ```markdown
   ### Phase 8 — Design System Expansion
   - [x] Alignment utilities: `align-start`, `align-center`, `align-end`, `align-stretch`, `align-baseline` (sets `align-items`)
   - [x] Justify utilities: `justify-start`, `justify-center`, `justify-end`, `justify-between`, `justify-around`, `justify-evenly` (sets `justify-content`)
   - [x] Audit for other primitive utilities the layout primitives commonly need (text alignment, flex-direction overrides) and add only the ones with clear demand
   - [x] Distribution rides on Phase 7: new partials land under `.zero/styles/`, refresh via `zero update`
   ```

**Tests:**

The `agents_md_has_section_sentinels` test asserts specific top-level section names exist in `AGENTS.md` — none of those names change, so the test continues to pass. No new test is added for documentation prose; the existing sentinel test is sufficient.

Run `cargo test` one final time after this step to verify the whole suite is green end-to-end.

## Risks and Assumptions

- **`grass` accepts the modern `start` / `end` keywords without alias.** All values in `_alignment.scss` are plain CSS keywords; `grass` is a CSS-pass-through SCSS implementation and emits keywords unchanged. The integration test in Step 4 catches any regression.
- **Class-list source order survives the SCSS aggregate.** The aggregate `@use`s each partial in order; rules appear in compiled CSS in `@use` order, and within each `@use` in file order. Verified today by `_utilities.scss` overriding `_layout.scss` defaults; the new partial slots in after `_utilities.scss` and inherits the same guarantee.
- **The chosen demo class (`align-center` on `<main class="stack pad-xl">`) is benign.** `.stack` is `display: flex; flex-direction: column`; `align-items: center` on a column-flex parent centers children on the cross axis (horizontally). The existing `<h1>` and `<div class="cluster gap-md">` will be visually centered horizontally; nothing breaks layout for the scaffold's smoke test or `home.test.ts`. If a future user objects to this visual default, the choice is one diff away from being moved to a different primitive.
- **No length-coupled assertions outside `framework_manifest_lists_seven_files` will silently miss the new file.** Verified by reading `src/cmd/update.rs::tests` (all dynamic) and the `tests/update.rs` integration tests (also dynamic). If a future audit finds one, it will be caught by `cargo test` immediately.
- **Doc updates are user-visible but not test-asserted at the prose level.** A typo in the AGENTS.md table will not break a test; the integration tests cover behavior, not prose. This is consistent with how the existing utility-family prose is treated.
- **Phase 8's spec mentioned "audit for other primitive utilities" is treated as resolved.** The spec's refine session locked in the six families. No further audit happens in this implementation. If a downstream user identifies a missing utility, it is a separate spec.
