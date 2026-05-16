# Plan: Component Library

## Summary

Ship fourteen framework-owned components (Button, Input, TextArea, Checkbox,
Radio, Select, Toggle, Card, Dialog, Spinner, Badge, Avatar, Toast, Tabs) as
`.ts` modules under `.zero/components/`, importable via the bare specifier
`"zero/components"`. Each component is a plain function in zero's once-per-mount
style; stateful props accept `Signal`s directly. Per-component SCSS partials
under `.zero/styles/components/` are aggregated by a new `_components.scss`
partial wrapped in `@layer components`. A new in-repo `showcase/` project
exercises every component with a light/dark theme switcher. The work is
sequenced so the codebase stays compilable and the existing test suite stays
green at every step: first wire the bare specifier through every resolver
(build, dev, test) and the discovery walker, then introduce stub manifest
entries, then fill them with real components, then dogfood through the
showcase, then update docs.

The implementation rides entirely on the existing scaffold/manifest plumbing,
the existing `grass` SCSS pipeline, and the existing swc transpile pipeline.
No new Rust or npm dependencies.

## Prerequisites

None. All spec open questions are resolved below in the relevant step.

Resolved open-question decisions (referenced where they matter):

| Spec OQ | Decision |
| --- | --- |
| Per-component SCSS rule budget | No hard cap; aim for ≤ ~30 lines but do not force token gymnastics. |
| Dialog focus trap | Deferred to a future a11y-polish issue. v1 ships backdrop + Esc-to-close + `aria-modal`. |
| Toast stacking | Single-toast UI; no queue. |
| Tabs keyboard nav | Left/Right arrows cycle through `tabs`; Home/End jump to first/last. |
| Spinner animation | CSS-only `@keyframes` rotation; no SVG. |
| Showcase routing | One route per component + `/` overview. |
| Theme toggle location | `src/app.ts` (mount-scoped global) so the choice survives navigation. |
| Resolver shape | Single `"zero/components"` entry — no per-component subpaths. |
| Prop types location | `.zero/components.d.ts` declares the module with named exports for editors; per-component prop types are also exported from each `.ts` source for callers that want them. |
| Manifest count | 53. |
| User-project tests | Ship as-is; no flag. |
| Showcase `dist/` | Gitignored. |
| Avatar fallback | Colored circle showing `initials`; if `initials` is omitted, derive from the first character of `alt`. |
| Input `type` union | `"text" \| "email" \| "password" \| "number" \| "search" \| "url" \| "tel"`. |
| Select arrow | Leave the native arrow; do not mask it. |
| Tabs panel mounting | Conditionally render only the active panel via a reactive block. |
| Manifest length assertion | Switch from a hard-coded number to a vector-of-expected-paths length assertion. |

## Steps

- [x] **Step 1: Wire `"zero/components"` through every resolver and let test discovery enter `.zero/components/`.**
- [x] **Step 2: Add stub `.zero/components/index.ts` and `.zero/components.d.ts` to the manifest; update tsconfig include.**
- [x] **Step 3: Add the `_components.scss` aggregate and `@use 'components';` in `zero.scss`.**
- [x] **Step 4: Implement `Avatar` (`.ts` + `.test.ts` + `_avatar.scss`); extend index/d.ts/aggregate.**
- [x] **Step 5: Implement `Badge` (`.ts` + `.test.ts` + `_badge.scss`).**
- [x] **Step 6: Implement `Button` (`.ts` + `.test.ts` + `_button.scss`).**
- [x] **Step 7: Implement `Card` (`.ts` + `.test.ts` + `_card.scss`).**
- [x] **Step 8: Implement `Checkbox` (`.ts` + `.test.ts` + `_checkbox.scss`).**
- [x] **Step 9: Implement `Dialog` (`.ts` + `.test.ts` + `_dialog.scss`).**
- [x] **Step 10: Implement `Input` (`.ts` + `.test.ts` + `_input.scss`).**
- [x] **Step 11: Implement `Radio` (`.ts` + `.test.ts` + `_radio.scss`).**
- [x] **Step 12: Implement `Select` (`.ts` + `.test.ts` + `_select.scss`).**
- [x] **Step 13: Implement `Spinner` (`.ts` + `.test.ts` + `_spinner.scss`).**
- [x] **Step 14: Implement `Tabs` (`.ts` + `.test.ts` + `_tabs.scss`).**
- [x] **Step 15: Implement `TextArea` (`.ts` + `.test.ts` + `_textarea.scss`).**
- [x] **Step 16: Implement `Toast` (`.ts` + `.test.ts` + `_toast.scss`).**
- [x] **Step 17: Implement `Toggle` (`.ts` + `.test.ts` + `_toggle.scss`).**
- [x] **Step 18: Switch the manifest-size assertion to a vector-based check.**
- [x] **Step 19: Build the `showcase/` project — `zero.toml`, `index.html`, 15 routes, `app.scss`, committed `.zero/`.** (Deviation: `showcase/.zero/` is gitignored, not committed, to avoid duplicating ~53 framework files in the repo. Step 20's `showcase_drift` test is no longer needed — CI runs `zero update --yes` inside `showcase/` before building, so the showcase's `.zero/` is always derived from the current manifest.)
- [x] **Step 20: Add integration tests — `tests/showcase_dev.rs`, `tests/showcase_build.rs`, `tests/component_library.rs`, and a drift check for `showcase/.zero/`.** (Deviation: drift check dropped — `showcase/.zero/` is gitignored, materialized fresh each run by the shared `prepare_showcase` helper in `tests/common/mod.rs`.)
- [x] **Step 21: Update `AGENTS.md` (`## Components` section) and `zero-framework-spec.md` (§7.1, §11, §12, §13).** (Deviation: the new AGENTS.md section is titled `## Component library` rather than reusing `## Components`, because the existing `## Components` section already documents the conceptual function-component pattern. The sentinel test was extended to assert both headers.)

---

## Step Details

### Step 1: Wire `"zero/components"` through every resolver and discovery

**Goal:** Make the bare specifier `"zero/components"` resolvable in all three
module pipelines (dev server / bundler / Boa test runner) and make the test
discoverer walk into `.zero/components/` despite its hidden parent. After this
step, importing `"zero/components"` works at every layer — it just returns
nothing yet because no files exist. This step lays the wiring with no
behavioral change visible to a user.

**Files:**

- `src/build/resolver.rs` — extend `resolve`.
- `src/dev/inject.rs` — extend the importmap in `dev_scripts`.
- `src/dev/server.rs` — add a `/.zero/components/*path` route.
- `src/test_runner/loader.rs` — extend `load_imported_module`.
- `src/test_runner/discovery.rs` — narrow the hidden-directory skip so that
  `.zero/components/` is descended.

**Changes:**

1. **Bundler resolver** (`src/build/resolver.rs`):
   - In `resolve`, before the relative-specifier branch, add:
     ```rust
     if specifier == "zero/components" {
         return Ok(ModuleId::User(PathBuf::from("./.zero/components/index.ts")));
     }
     ```
   - The remainder of the BFS already canonicalizes against `root` and walks
     relative imports from this file's directory, which is `.zero/components/`.
   - Update `bare_specifier_is_rejected` to keep using `lodash`, and add a new
     `zero_components_resolves_to_dot_zero_path` test asserting the mapping.

2. **Dev-server importmap** (`src/dev/inject.rs::dev_scripts`):
   - Change the importmap literal to:
     ```rust
     s.push_str(r#"<script type="importmap">{"imports":{"zero":"/zero.js","zero/components":"/.zero/components/index.ts"}}</script>"#);
     ```
   - Add a unit test `dev_scripts_importmap_contains_zero_components`.

3. **Dev-server file route** (`src/dev/server.rs::serve`):
   - Add a new `.route(...)` call before the catch-all fallback:
     ```rust
     .route(
         "/.zero/components/*path",
         get(|State(s): State<Arc<AppState>>, Path(p): Path<String>| async move {
             serve_under_with_transpile(
                 s.root.join(".zero").join("components"),
                 "/.zero/components",
                 &format!("/.zero/components/{p}"),
                 s.dev_sourcemap,
             )
             .await
         }),
     )
     ```
   - The existing `serve_under_with_transpile` already handles `.ts`
     transpilation through swc. Relative imports inside `index.ts`
     (`./Button.ts`) resolve to URLs in the same `/.zero/components/` route, so
     no further wiring is needed.

4. **Test-runner loader** (`src/test_runner/loader.rs::load_imported_module`):
   - Add a new arm between `"zero/test"` and the relative-specifier arm:
     ```rust
     "zero/components" => {
         let mut ctx = context.borrow_mut();
         let path = self.root.join(".zero").join("components").join("index.ts");
         let synthetic = Referrer::Module(self.dummy_referrer_module());
         // Use the existing resolve_relative path-walk: synthesize a referrer
         // whose dir is .zero/components/ and a spec of "./index.ts".
         self.resolve_relative("./index.ts", &Referrer::Module(/* see below */), &mut ctx)
     }
     ```
   - The cleanest implementation is to add a small private helper
     `resolve_components_index` that opens
     `<root>/.zero/components/index.ts` directly (mirroring
     `resolve_relative` but with a hard-coded path), caches under
     `"zero/components"`, and returns the module. Implementing this without a
     synthetic Referrer keeps the relative-import semantics intact because Boa
     itself resolves the index file's own relative imports against the parsed
     module's path.
   - Add a unit test `resolves_zero_components_module` that writes a stub
     `<root>/.zero/components/index.ts` and parses a module that imports from
     `"zero/components"`; assert no rejection and cache presence.

5. **Discovery hidden-directory exception** (`src/test_runner/discovery.rs::walk_dir`):
   - The current rule `if name_str.starts_with('.') { continue; }` skips every
     hidden directory. Adjust so that the entry `".zero"` is *not* skipped —
     only its `components/` subtree is then walked. Implementation:
     replace the early `continue` with:
     ```rust
     if name_str.starts_with('.') && name_str != ".zero" {
         continue;
     }
     ```
     Then, inside the recursion, when descending into `.zero/`, only descend
     into `.zero/components/`:
     ```rust
     if name_str == ".zero" {
         walk_dot_zero(&path, out_dir, out)?;
         continue;
     }
     ```
     where `walk_dot_zero` enumerates only the `components/` child and recurses
     normally. Document the narrow exception in a single-line comment.
   - Add tests:
     - `walks_into_dot_zero_components` — places a `.zero/components/Foo.test.ts`
       and asserts discovery returns it.
     - `does_not_walk_into_other_dot_zero_subdirs` — places a
       `.zero/styles/extra.test.ts` and asserts discovery skips it.
     - `still_skips_other_hidden_dirs` — places a `.hidden/foo.test.ts` and
       asserts discovery skips it.

**Tests:** Five new unit tests across the four files plus the discovery tests.
All existing tests remain green: the resolver and importmap changes are
additive; the loader change only adds a new specifier; the discovery change
narrows the skip predicate but leaves the previously-skipped paths skipped
(except `.zero/components/`).

---

### Step 2: Add stub `.zero/components/index.ts` and `.zero/components.d.ts`

**Goal:** Make the manifest aware of two new files (an empty index re-export
and a module declaration) so the next steps can populate them and so existing
projects pick them up on `zero update`. After this step, a fresh `zero init`
creates `.zero/components/index.ts` and `.zero/components.d.ts`, and the
plumbing wired up in Step 1 hits real files.

**Files:**

- `src/scaffold/.zero/components/index.ts` — new, empty re-export aggregate.
- `src/scaffold/.zero/components.d.ts` — new, ambient module declaration.
- `src/scaffold/tsconfig.json` — extend the `include` array.
- `src/scaffold.rs` — register both new `TPL_*` constants and manifest entries.

**Changes:**

1. **`src/scaffold/.zero/components/index.ts`** (new file):
   ```ts
   // Components index — populated by Step 4.
   export {};
   ```

2. **`src/scaffold/.zero/components.d.ts`** (new file):
   ```ts
   // Declared so editors can resolve the bare specifier "zero/components"
   // against the source under .zero/components/. The real module surface is
   // declared at the bottom of this file once the components exist.
   declare module "zero/components" {
     // Populated by Step 4.
   }
   ```

3. **`src/scaffold/tsconfig.json`**: extend `include`:
   ```json
   "include": [
     "src",
     ".zero/zero.d.ts",
     ".zero/zero-test.d.ts",
     ".zero/components.d.ts"
   ]
   ```

4. **`src/scaffold.rs`**:
   - Add:
     ```rust
     const TPL_COMPONENTS_INDEX_TS: &str =
         include_str!("scaffold/.zero/components/index.ts");
     const TPL_COMPONENTS_DTS: &str =
         include_str!("scaffold/.zero/components.d.ts");
     ```
   - Extend `framework_manifest()`:
     ```rust
     (".zero/components/index.ts", TPL_COMPONENTS_INDEX_TS),
     (".zero/components.d.ts", TPL_COMPONENTS_DTS),
     ```
   - Extend `write_initial_project` indirectly: `write_framework_files` already
     creates parent directories per entry; no change needed.

**Tests:**

- New: `write_initial_project_emits_components_stubs` — both files exist;
  `index.ts` contains `export {};`; `components.d.ts` contains
  `declare module "zero/components"`.
- New: `tsconfig_include_contains_components_dts`.
- Update `framework_manifest_lists_eight_files`: rename to
  `framework_manifest_contains_all_expected_paths` and change the assertion to
  contain both new paths. The total count is allowed to grow (it grows further
  in subsequent steps); the test continues to assert `paths.contains(...)`
  per expected entry. The final length assertion is replaced in Step 18.

---

### Step 3: Add the `_components.scss` aggregate and wire it into `zero.scss`

**Goal:** Set up the SCSS aggregate the per-component partials will hook into,
and surface it through `zero.scss`. Empty placeholder for now — Step 4 fills it.

**Files:**

- `src/scaffold/.zero/styles/_components.scss` — new, empty aggregate.
- `src/scaffold/.zero/styles/zero.scss` — append one line.
- `src/scaffold.rs` — register the new partial.

**Changes:**

1. **`src/scaffold/.zero/styles/_components.scss`** (new file):
   ```scss
   // Per-component partials. Populated by Step 4.
   // Every component partial is wrapped in `@layer components { ... }` so
   // user CSS in styles/app.scss automatically overrides component defaults
   // without specificity tricks or `!important`.
   ```

2. **`src/scaffold/.zero/styles/zero.scss`** — append:
   ```scss
   @use 'components';
   ```
   so the file becomes:
   ```scss
   @use 'tokens';
   @use 'base';
   @use 'layout';
   @use 'utilities';
   @use 'alignment';
   @use 'components';
   ```

3. **`src/scaffold.rs`**:
   - Add `TPL_COMPONENTS_AGGREGATE_SCSS` constant.
   - Add `(".zero/styles/_components.scss", TPL_COMPONENTS_AGGREGATE_SCSS)` to
     `framework_manifest()`.

**Tests:**

- Extend `zero_scss_contains_aggregate_uses` to assert `@use 'components'`
  appears.
- New: `components_aggregate_partial_exists` — asserts the file exists and is
  not empty.
- The existing `process_css_compiles_scss`/`scss_build.rs` tests will exercise
  the aggregate as part of any downstream `app.scss` build; no new build
  fixture is needed yet.

---

### Steps 4–17: Implement the components (one per step)

The next fourteen steps each implement a single component. They share a uniform
shape, captured here once instead of repeating per step.

**Files (per step, replacing `<Name>` with the component and `<name>` with
its lowercase class root):**

- `src/scaffold/.zero/components/<Name>.ts` — new component source.
- `src/scaffold/.zero/components/<Name>.test.ts` — new component test.
- `src/scaffold/.zero/styles/components/_<name>.scss` — new SCSS partial.
- `src/scaffold/.zero/components/index.ts` — append the re-export.
- `src/scaffold/.zero/components.d.ts` — append the `declare module` stanza.
- `src/scaffold/.zero/styles/_components.scss` — append `@use 'components/<name>';`.
- `src/scaffold.rs` — register three new `TPL_*` constants and three new
  manifest entries; extend the `COMPONENT_NAMES` slice (introduced in Step 4)
  so the per-step scaffold-tests pick up the new component.

**Shared component contract:**

```ts
import { html } from "zero";
import type { TemplateResult, Signal } from "zero";

export type <Name>Props = { /* see per-step spec */ };

export default function <Name>(props: <Name>Props = {} as <Name>Props): TemplateResult {
  // pick variant/size defaults; build a class string; return html`...`.
}
```

> Note on `?disabled=`: zero's `html` tagged template supports plain
> `attr=${value}` interpolation. If the runtime does **not** support a
> `?attr=${bool}` boolean shorthand, fall back to
> `${cond ? "disabled" : ""}` as a static string attribute. The first
> component step that hits a boolean attribute (Step 6, `Button`) verifies
> this against the runtime and codifies the chosen pattern; subsequent steps
> follow.

**Shared test shape:**

```ts
import { describe, it, expect, afterEach } from "zero/test";
import { render, find, fire, cleanup, spy } from "zero/test";
import { signal } from "zero";
import <Name> from "./<Name>.ts";

describe("<Name>", () => {
  afterEach(cleanup);

  it("renders the base class", () => {
    const el = render(<Name>(/* minimal props */));
    expect(find(el, ".<name>")).toBeTruthy();
  });

  // one interaction-specific assertion per per-step spec.
});
```

**Shared SCSS convention** (mandatory across all 14 partials):

```scss
@layer components {
  .<name> { /* base */ }
  .<name>-<variant> { /* variant rules */ }
  .<name>-<size> { /* size rules */ }
}
```

No `!important`. No inline hex. All values come from `var(--*)` tokens
(`opacity`, `transition-duration`, `z-index`, and `@keyframes` percentages
are the documented exceptions per spec constraint).

**Shared scaffold-test rig (introduced in Step 4, extended each step):**

A new private constant in `src/scaffold.rs`'s `tests` module:

```rust
const COMPONENT_NAMES: &[&str] = &[
    // Step 4 sets this to `&["Avatar"]`; each subsequent step appends one
    // entry alphabetically so this list is always the canonical roster of
    // landed components.
];
```

Six scaffold tests (also introduced in Step 4) iterate over `COMPONENT_NAMES`:

- `components_index_re_exports_each_listed` — for each name, assert the
  generated `index.ts` contains `export { default as <Name>`.
- `component_source_files_emitted` — for each name, `<Name>.ts` exists,
  is non-empty, and contains `class="<name>`.
- `component_test_files_emitted` — for each name, `<Name>.test.ts` exists
  and imports from `"zero/test"`.
- `component_partials_use_layer_components` — for each name,
  `_<name>.scss` contains `@layer components` and no `!important`.
- `components_aggregate_uses_each_partial` — for each name,
  `_components.scss` contains `@use 'components/<name>';`.
- `components_dts_declares_each_listed` — for each name,
  `components.d.ts` contains a `<Name>(` declaration.

Because the tests iterate over `COMPONENT_NAMES`, each per-component step
only has to append one entry to the slice — the assertions then cover the
new component without further test edits.

**Closing-out the d.ts:** `components.d.ts` is wrapped in a single
`declare module "zero/components" { ... }` block. Each per-component step
inserts a new stanza inside the block, immediately before the closing brace,
keeping the block alphabetical. Step 4 introduces the block (containing just
the `Avatar` stanza); subsequent steps insert within it.

**Closing-out the aggregate SCSS:** `_components.scss` keeps the
`@use 'components/<name>';` lines in alphabetical order. Each step inserts at
the alphabetical position; Step 4 turns the placeholder comment into the
first real `@use 'components/avatar';` line.

**Tests per step:** Each step ships its component's own `.test.ts` file (the
in-project test that runs under `zero test`). The shared scaffold-test rig
above provides the Rust-side regression coverage; each step appends one entry
to `COMPONENT_NAMES`.

The 14 per-component steps follow. Each step's body lists only what is
specific to that component: prop type, render outline, SCSS notes, and test
focus. Everything else (file paths, manifest entries, test rig) follows the
shared shape above.

---

### Step 4: Implement `Avatar`

**Goal:** First component lands. This step also introduces the `COMPONENT_NAMES`
slice, the six iterating scaffold tests, and the initial `declare module`
block in `components.d.ts`.

**Spec:**

- **`Avatar.ts`** — `AvatarProps = { src?: string; alt: string; initials?: string; size?: "sm" | "md" | "lg" | "xl" }`. Default size `"md"`. When `src` is set, render `<img class="avatar avatar-${size}" src=${src} alt=${alt}>`. Otherwise render `<span class="avatar avatar-${size} avatar-initials" aria-label=${alt}>${initials ?? alt[0]?.toUpperCase() ?? ""}</span>`.
- **`_avatar.scss`** — `display: inline-flex; align-items: center; justify-content: center; border-radius: 50%; background: var(--color-surface); color: var(--color-text);` on `.avatar`. Sizes set `inline-size` and `block-size` from `--space-md` (`sm`), `--space-lg` (`md`), `--space-xl` (`lg`), `calc(var(--space-xl) * 1.5)` (`xl`). `.avatar-initials` sets `font-weight: var(--weight-bold)`.
- **`Avatar.test.ts`** — renders `Avatar({ alt: "Ada Lovelace" })`, asserts `find(el, ".avatar")` returns truthy and `text(el)` is `"A"`.

**Files:** Shared shape, plus the new test rig in `src/scaffold.rs`
(`COMPONENT_NAMES` slice and the six iterating tests). `COMPONENT_NAMES` starts
as `&["Avatar"]`.

---

### Step 5: Implement `Badge`

**Spec:**

- **`Badge.ts`** — `BadgeProps = { variant?: "default" | "primary" | "success" | "warning" | "danger"; size?: "sm" | "md"; children?: TemplateResult | string }`. Default variant `"default"`, size `"md"`. Render `<span class="badge badge-${variant} badge-${size}">${children}</span>`.
- **`_badge.scss`** — `.badge { display: inline-flex; padding-inline: var(--space-sm); padding-block: var(--space-xs); border-radius: var(--radius-sm); font-size: var(--font-sm); font-weight: var(--weight-bold); line-height: var(--leading-tight); }`. `.badge-default` → `background: var(--color-surface); color: var(--color-text);`. `.badge-primary` → `background: var(--color-primary); color: var(--color-primary-fg);`. `success` / `warning` / `danger` declare local CSS custom properties at the top of the partial (e.g. `--badge-success-bg: #2f9e44;`) **inside `@layer components`** so token-overriding rules still cascade correctly; alternatively, reuse `--color-primary` family rotations. The first choice is preferred — explicit local tokens are easier to override.
- **`Badge.test.ts`** — render the default variant; assert `.badge.badge-default.badge-md` classes. Add one variant-class assertion (`Badge({ variant: "primary" })` → `.badge-primary` present).

**`COMPONENT_NAMES`:** append `"Badge"`.

---

### Step 6: Implement `Button`

**Goal:** Verifies the runtime's handling of boolean attributes (per the
contract note above) and locks in the pattern subsequent steps reuse.

**Spec:**

- **`Button.ts`** — `ButtonProps = { variant?: "primary" | "secondary" | "ghost" | "danger"; size?: "sm" | "md" | "lg"; disabled?: boolean; loading?: boolean; onClick?: (event: Event) => void; children?: TemplateResult | string }`. Default variant `"primary"`, size `"md"`. Render `<button class="button button-${variant} button-${size}" @click=${onClick} ...>${loading ? html`<span class="button-spinner spinner spinner-${variant} spinner-sm" role="status" aria-label="loading"></span>` : null}${children}</button>`. Boolean attribute handling matches the pattern verified in this step.
- **`_button.scss`** — `display: inline-flex; align-items: center; gap: var(--space-xs); padding-inline: var(--space-md); padding-block: var(--space-sm); border: var(--border-thin) solid transparent; border-radius: var(--radius-md); font: inherit; font-weight: var(--weight-bold); line-height: var(--leading-tight); cursor: pointer;`. Variants: primary `background: var(--color-primary); color: var(--color-primary-fg);`. Secondary `background: var(--color-surface); color: var(--color-text); border-color: var(--color-border);`. Ghost `background: transparent; color: var(--color-text);`. Danger uses a local `--button-danger-bg` token. Sizes adjust padding + font-size.
- **`Button.test.ts`** — `onClick` spy fires on click; `disabled` button's click does not fire `onClick`.

**`COMPONENT_NAMES`:** append `"Button"`. Note in the commit message which
boolean-attribute pattern was chosen (so the rest of the work follows
suit).

---

### Step 7: Implement `Card`

**Spec:**

- **`Card.ts`** — `CardProps = { variant?: "surface" | "outlined"; title?: string; children?: TemplateResult | string }`. Default variant `"surface"`. Render `<section class="card card-${variant}">${title ? html`<h3 class="card-title">${title}</h3>` : null}<div class="card-body">${children}</div></section>`.
- **`_card.scss`** — `.card { display: block; padding: var(--space-md); border-radius: var(--radius-lg); }`. `.card-surface { background: var(--color-surface); }`. `.card-outlined { background: transparent; border: var(--border-thin) solid var(--color-border); }`. `.card-title { margin: 0 0 var(--space-sm); font-size: var(--font-lg); font-weight: var(--weight-bold); }`.
- **`Card.test.ts`** — `Card({ title: "Heading", children: "Body" })` renders the `<h3>`; `Card({ children: "Only body" })` omits it.

**`COMPONENT_NAMES`:** append `"Card"`.

---

### Step 8: Implement `Checkbox`

**Spec:**

- **`Checkbox.ts`** — `CheckboxProps = { checked: Signal<boolean>; label?: string; disabled?: boolean }`. Render `<label class="checkbox"><input type="checkbox" checked=${() => checked.val} @change=${() => checked.set(!checked.val)} disabled=${...}><span class="checkbox-label">${label}</span></label>`.
- **`_checkbox.scss`** — `.checkbox { display: inline-flex; align-items: center; gap: var(--space-sm); cursor: pointer; }`. Style the native input minimally; let the OS render the checkmark.
- **`Checkbox.test.ts`** — `const checked = signal(false); render(Checkbox({ checked, label: "Subscribe" })); fire(find(el, "input"), "change");` → `expect(checked.val).toBe(true)`.

**`COMPONENT_NAMES`:** append `"Checkbox"`.

---

### Step 9: Implement `Dialog`

**Spec:**

- **`Dialog.ts`** — `DialogProps = { open: Signal<boolean>; size?: "sm" | "md" | "lg"; title?: string; children?: TemplateResult | string; onClose?: () => void }`. Default size `"md"`. Render a reactive block that returns `null` when `!open.val`, otherwise `<div class="dialog-backdrop dialog-open" @click=${() => { open.set(false); onClose?.(); }}><div class="dialog dialog-${size}" role="dialog" aria-modal="true" @click.stop=${() => {}}>${title ? html`<h2 class="dialog-title">${title}</h2>` : null}<div class="dialog-body">${children}</div></div></div>`. Wire an `effect` that adds a `keydown` listener to `document` while `open.val` is true; pressing `Escape` calls `open.set(false)` and `onClose?.()`. The listener is removed in the effect's cleanup. **No focus trap** (deferred per spec OQ).
- **`_dialog.scss`** — `.dialog-backdrop { position: fixed; inset: 0; display: flex; align-items: center; justify-content: center; background: rgba(0, 0, 0, 0.5); z-index: 1000; }`. (rgba is allowed — `opacity` is in the documented exception list.) `.dialog { background: var(--color-bg); color: var(--color-text); border-radius: var(--radius-lg); box-shadow: var(--shadow-lg); padding: var(--space-lg); max-inline-size: 90vw; }`. Sizes set `inline-size`.
- **`Dialog.test.ts`** — `const open = signal(false); const el = render(Dialog({ open, children: "Body" }));` — assert `findAll(el, ".dialog-open").length === 0`. Then `open.set(true)`; assert `find(el, ".dialog-open")` is truthy after commit.

**`COMPONENT_NAMES`:** append `"Dialog"`.

---

### Step 10: Implement `Input`

**Spec:**

- **`Input.ts`** — `InputProps = { value: Signal<string>; type?: "text" | "email" | "password" | "number" | "search" | "url" | "tel"; size?: "sm" | "md" | "lg"; placeholder?: string; disabled?: boolean; label?: string }`. Default type `"text"`, size `"md"`. Render `${label ? html`<label class="input-label">${label}</label>` : null}<input class="input input-${size}" type=${type} value=${() => value.val} placeholder=${placeholder} @input=${(e) => value.set((e.target as HTMLInputElement).value)} disabled=${...}>`. If both are present, wrap them in `<label class="input-wrap">`.
- **`_input.scss`** — `.input { display: inline-flex; padding-inline: var(--space-sm); padding-block: var(--space-xs); border: var(--border-thin) solid var(--color-border); border-radius: var(--radius-sm); background: var(--color-bg); color: var(--color-text); font: inherit; }`. Sizes change padding + font-size. `.input-label { display: block; font-size: var(--font-sm); margin-block-end: var(--space-xs); }`.
- **`Input.test.ts`** — `const value = signal(""); render(Input({ value })); fire(find(el, "input"), "input", { target: { value: "hello" } });` → `expect(value.val).toBe("hello")`.

**`COMPONENT_NAMES`:** append `"Input"`.

---

### Step 11: Implement `Radio`

**Spec:**

- **`Radio.ts`** — `RadioProps = { selected: Signal<string>; name: string; value: string; label?: string; disabled?: boolean }`. Render `<label class="radio"><input type="radio" name=${name} value=${value} checked=${() => selected.val === value} @change=${() => selected.set(value)} disabled=${...}><span class="radio-label">${label}</span></label>`.
- **`_radio.scss`** — same shape as `.checkbox` partial: flex row with gap.
- **`Radio.test.ts`** — render two `Radio`s sharing one `selected` signal; click the second; assert `selected.val === "<second value>"`.

**`COMPONENT_NAMES`:** append `"Radio"`.

---

### Step 12: Implement `Select`

**Spec:**

- **`Select.ts`** — `SelectProps = { value: Signal<string>; options: { value: string; label: string }[]; size?: "sm" | "md" | "lg"; disabled?: boolean; label?: string }`. Default size `"md"`. Render `${label ? html`<label class="select-label">${label}</label>` : null}<select class="select select-${size}" @change=${(e) => value.set((e.target as HTMLSelectElement).value)} disabled=${...}>${options.map(o => html`<option value=${o.value} selected=${() => value.val === o.value}>${o.label}</option>`)}</select>`.
- **`_select.scss`** — `.select { padding-inline: var(--space-sm); padding-block: var(--space-xs); border: var(--border-thin) solid var(--color-border); border-radius: var(--radius-sm); background: var(--color-bg); color: var(--color-text); font: inherit; }`. Sizes change padding + font-size. Native arrow not masked (per OQ).
- **`Select.test.ts`** — `const value = signal("a"); render(Select({ value, options: [{value:"a",label:"A"},{value:"b",label:"B"}] })); fire(find(el, "select"), "change", { target: { value: "b" } });` → `expect(value.val).toBe("b")`.

**`COMPONENT_NAMES`:** append `"Select"`.

---

### Step 13: Implement `Spinner`

**Spec:**

- **`Spinner.ts`** — `SpinnerProps = { variant?: "primary" | "muted"; size?: "sm" | "md" | "lg"; label?: string }`. Default variant `"primary"`, size `"md"`. Render `<span class="spinner spinner-${variant} spinner-${size}" role="status">${label ? html`<span class="visually-hidden">${label}</span>` : null}</span>`.
- **`_spinner.scss`** — Define `@keyframes spinner-rotate { to { transform: rotate(360deg); } }`. `.spinner { display: inline-block; border-radius: 50%; border: var(--border-md) solid var(--color-border); border-top-color: var(--color-primary); animation: spinner-rotate 0.8s linear infinite; }`. Sizes set `inline-size`/`block-size`. `.spinner-muted { border-top-color: var(--color-text-muted); }`. `.visually-hidden` rule (a small a11y helper) lives here too — `position: absolute; clip-path: inset(50%); inline-size: 1px; block-size: 1px;`.
- Note: `Button`'s loading state (Step 6) renders `<span class="button-spinner spinner spinner-{variant} spinner-sm">`, so this partial's `.spinner` rules apply to it automatically.
- **`Spinner.test.ts`** — render `Spinner()`; assert `.spinner.spinner-primary.spinner-md` classes and `role="status"`.

**`COMPONENT_NAMES`:** append `"Spinner"`.

---

### Step 14: Implement `Tabs`

**Spec:**

- **`Tabs.ts`** — `TabsProps = { active: Signal<string>; tabs: { id: string; label: string }[]; panels: Record<string, TemplateResult> }`. Render `<div class="tabs"><div class="tabs-list" role="tablist" @keydown=${onKeyDown}>${tabs.map(t => html`<button class="tabs-tab" role="tab" aria-selected=${() => active.val === t.id} @click=${() => active.set(t.id)}>${t.label}</button>`)}</div><div class="tabs-panel" role="tabpanel">${() => panels[active.val] ?? null}</div></div>`. `onKeyDown` reads `event.key`: `ArrowLeft`/`ArrowRight` rotates `active` through `tabs` (wraps); `Home` → first; `End` → last.
- **`_tabs.scss`** — `.tabs-list { display: inline-flex; gap: var(--space-xs); border-block-end: var(--border-thin) solid var(--color-border); }`. `.tabs-tab { padding-inline: var(--space-md); padding-block: var(--space-sm); border: none; background: transparent; color: var(--color-text-muted); font: inherit; cursor: pointer; }`. `.tabs-tab[aria-selected="true"] { color: var(--color-text); border-block-end: var(--border-md) solid var(--color-primary); }`. `.tabs-panel { padding-block: var(--space-md); }`.
- **`Tabs.test.ts`** — render with `tabs: [{id:"a",label:"A"},{id:"b",label:"B"}]`, `panels: { a: html\`<p>A</p>\`, b: html\`<p>B</p>\` }`, `active: signal("a")`. Assert `text(el, ".tabs-panel")` is `"A"`. Click the second `<button role="tab">`; assert `active.val === "b"` and the panel updates to `"B"`.

**`COMPONENT_NAMES`:** append `"Tabs"`.

---

### Step 15: Implement `TextArea`

**Spec:**

- **`TextArea.ts`** — `TextAreaProps = { value: Signal<string>; rows?: number; placeholder?: string; disabled?: boolean; label?: string }`. Render `${label ? html`<label class="textarea-label">${label}</label>` : null}<textarea class="textarea" rows=${rows ?? 4} placeholder=${placeholder} @input=${(e) => value.set((e.target as HTMLTextAreaElement).value)} disabled=${...}>${() => value.val}</textarea>`.
- **`_textarea.scss`** — same look as `.input`; add `min-block-size: calc(var(--space-xl) * 2);`.
- **`TextArea.test.ts`** — same shape as `Input.test.ts`; assert input event updates the signal.

**`COMPONENT_NAMES`:** append `"TextArea"`.

---

### Step 16: Implement `Toast`

**Spec:**

- **`Toast.ts`** — `ToastProps = { open: Signal<boolean>; variant?: "info" | "success" | "warning" | "danger"; message: string; duration?: number; onDismiss?: () => void }`. Default variant `"info"`. Render `${() => open.val ? html`<div class="toast toast-${variant}" role="status" aria-live="polite">${message}</div>` : null}`. If `duration` is set, set up an `effect` that — whenever `open.val` flips to `true` — schedules `setTimeout(() => { open.set(false); onDismiss?.(); }, duration)`. Clear the timer in the effect's cleanup so flipping `open` rapidly does not double-fire. Single-toast UI; no queue (per OQ).
- **`_toast.scss`** — `.toast { position: fixed; inset-block-end: var(--space-lg); inset-inline-end: var(--space-lg); padding: var(--space-sm) var(--space-md); border-radius: var(--radius-md); box-shadow: var(--shadow-md); z-index: 900; }`. Variants set background/color (`info` uses `--color-surface`/`--color-text`; others use local `--toast-*-bg` tokens declared at the top of the partial inside `@layer components`).
- **`Toast.test.ts`** — `const open = signal(false); const el = render(Toast({ open, message: "Saved" }));` — assert `findAll(el, ".toast").length === 0`. `open.set(true)` → assert `find(el, ".toast")` truthy and `text(el, ".toast") === "Saved"`. Do not test the `setTimeout` path (timers in the lightweight DOM may not match real browser behavior; the spec doesn't list a timer harness).

**`COMPONENT_NAMES`:** append `"Toast"`.

---

### Step 17: Implement `Toggle`

**Spec:**

- **`Toggle.ts`** — `ToggleProps = { checked: Signal<boolean>; label?: string; disabled?: boolean }`. Render `<label class="toggle"><input type="checkbox" class="toggle-input" checked=${() => checked.val} @change=${() => checked.set(!checked.val)} disabled=${...} role="switch" aria-checked=${() => String(checked.val)}><span class="toggle-track"><span class="toggle-thumb"></span></span><span class="toggle-label">${label}</span></label>`.
- **`_toggle.scss`** — `.toggle-input { position: absolute; opacity: 0; pointer-events: none; }`. `.toggle-track { display: inline-block; inline-size: var(--space-xl); block-size: var(--space-md); border-radius: var(--radius-lg); background: var(--color-border); position: relative; transition: background-color 150ms; }`. `.toggle-thumb { position: absolute; inset-block: 2px; inset-inline-start: 2px; inline-size: calc(var(--space-md) - 4px); aspect-ratio: 1; border-radius: 50%; background: var(--color-bg); transition: inset-inline-start 150ms; }`. `.toggle-input:checked ~ .toggle-track { background: var(--color-primary); }`. `.toggle-input:checked ~ .toggle-track .toggle-thumb { inset-inline-start: calc(100% - var(--space-md) + 2px); }`.
- **`Toggle.test.ts`** — `const checked = signal(false); render(Toggle({ checked, label: "Wifi" })); fire(find(el, "input"), "change");` → `expect(checked.val).toBe(true)`.

**`COMPONENT_NAMES`:** append `"Toggle"`. After this step, the slice is the full
14-component roster.

---

### Step 18: Switch manifest-size assertion to a vector-based check

**Goal:** Replace the brittle hard-coded length with an explicit list of
expected paths, and assert the manifest matches that set exactly. The list is
the single source of truth — adding or removing a file requires updating one
named list, not a count.

**Files:**

- `src/scaffold.rs` — modify the existing
  `framework_manifest_lists_eight_files` test.

**Changes:**

Rename to `framework_manifest_matches_expected_path_set` and refactor:

```rust
#[test]
fn framework_manifest_matches_expected_path_set() {
    let manifest = framework_manifest();
    let actual: BTreeSet<&str> = manifest.iter().map(|(p, _)| *p).collect();
    let expected: BTreeSet<&str> = [
        ".zero/zero.d.ts",
        ".zero/zero-test.d.ts",
        ".zero/styles/_tokens.scss",
        ".zero/styles/_base.scss",
        ".zero/styles/_layout.scss",
        ".zero/styles/_utilities.scss",
        ".zero/styles/_alignment.scss",
        ".zero/styles/_components.scss",
        ".zero/styles/zero.scss",
        ".zero/components/index.ts",
        ".zero/components.d.ts",
        ".zero/components/Avatar.ts",
        ".zero/components/Avatar.test.ts",
        // … list every component + test + partial …
        ".zero/styles/components/_avatar.scss",
        // …
    ].into_iter().collect();
    assert_eq!(actual, expected, "manifest path set drift");
    assert_eq!(manifest.len(), expected.len(),
        "manifest has duplicate keys");
}
```

**Tests:** The test itself is the assertion. Run the existing suite; it must
stay green.

---

### Step 19: Build the `showcase/` project

**Goal:** Add a complete in-repo zero project at `showcase/` that exercises
every component on a per-route basis with a theme switcher, builds with
`zero build`, and serves with `zero dev`. The showcase doubles as a manual
QA harness and as a CI integration-test target.

**Files (all under `showcase/`):**

- `showcase/zero.toml`
- `showcase/index.html`
- `showcase/src/app.ts`
- `showcase/src/routes/home.ts`
- `showcase/src/routes/{avatar,badge,button,card,checkbox,dialog,input,radio,select,spinner,tabs,textarea,toast,toggle}.ts` (14 files)
- `showcase/styles/app.scss`
- `showcase/.gitignore` — `dist/` only.
- `showcase/.zero/` (committed) — populated by `zero update --yes` inside
  `showcase/`.

**Changes:**

1. **`showcase/zero.toml`:**
   ```toml
   [project]
   root = "."

   [dev]
   port = 5174

   [build]
   out = "dist"
   ```

2. **`showcase/index.html`** — copy the scaffold's `index.html`,
   substituting `<title>zero showcase</title>`. Leave `<html lang="en">`;
   the theme attribute is set by `app.ts` at mount time.

3. **`showcase/src/app.ts`** — registers a `theme` signal and one route per
   component:
   ```ts
   import { App, signal, effect } from "zero";
   import Home from "./routes/home.ts";
   import Button from "./routes/button.ts";
   // … one import per component route ...

   const theme = signal<"auto" | "light" | "dark">("auto");

   effect(() => {
     const t = theme.val;
     if (t === "auto") {
       document.documentElement.removeAttribute("data-theme");
     } else {
       document.documentElement.dataset.theme = t;
     }
   });

   const app = new App();
   app.state("theme", theme);
   app.route("/", Home);
   app.route("/button", Button);
   app.route("/input", Input);
   // … one route per component …
   app.run("#app");
   ```
   Theme effect lives at app scope (per OQ resolution) so the choice survives
   navigation.

4. **`showcase/src/routes/home.ts`** — landing page:
   - A "Theme: auto/light/dark" cluster of buttons. Each button calls
     `theme.set(<value>)`. Active button gets `data-active`.
   - A `<nav>` cluster with `<a>` links to every component route. Active link
     styled via the existing `[data-active]` mechanism.
   - A short intro paragraph quoting the spec's motivation.

5. **`showcase/src/routes/<component>.ts`** (14 files) — each route imports
   its component from `"zero/components"` and renders it in every variant +
   size. At least one instance is wired to a per-route signal so the
   reactive behavior is visible. Example for `button.ts`:
   ```ts
   import { html, signal } from "zero";
   import type { TemplateResult } from "zero";
   import { Button } from "zero/components";

   export default function ButtonRoute(): TemplateResult {
     const clicks = signal(0);
     return html`
       <main class="stack pad-xl">
         <h1>Button</h1>
         <section class="cluster gap-md">
           ${Button({ variant: "primary", children: "Primary" })}
           ${Button({ variant: "secondary", children: "Secondary" })}
           ${Button({ variant: "ghost", children: "Ghost" })}
           ${Button({ variant: "danger", children: "Danger" })}
         </section>
         <section class="cluster gap-md align-center">
           ${Button({ size: "sm", children: "Small" })}
           ${Button({ size: "md", children: "Medium" })}
           ${Button({ size: "lg", children: "Large" })}
         </section>
         <section class="stack gap-sm">
           ${Button({ onClick: () => clicks.update(n => n + 1), children: "Click me" })}
           <p>Clicks: ${clicks}</p>
         </section>
       </main>
     `;
   }
   ```

6. **`showcase/styles/app.scss`:**
   ```scss
   @use '../.zero/styles/zero';

   // Showcase-only layout helpers.
   main { max-inline-size: 56rem; margin-inline: auto; }
   nav.cluster a { padding-block: var(--space-xs); padding-inline: var(--space-sm); }
   ```

7. **`showcase/.gitignore`:**
   ```gitignore
   dist/
   ```

8. **`showcase/.zero/`** — populated by running `cargo run -- update --yes`
   from within `showcase/` after Steps 1–5 land. The resulting tree is
   committed to git (req 21). Subsequent `zero update` runs converge to the
   same content.

**Tests:** No new tests in this step itself — Step 20 adds the integration
tests that exercise the showcase. The framework's existing test suite must
remain green (the showcase is a leaf directory and not picked up by
`cargo test`).

---

### Step 20: Add integration tests for the showcase and the component library

**Goal:** Wire the showcase into framework CI so regressions in either the
component implementations or the build/dev plumbing are caught. Three new
integration test files (plus a drift check).

**Files:**

- `tests/showcase_dev.rs` — new.
- `tests/showcase_build.rs` — new.
- `tests/component_library.rs` — new.
- `tests/showcase_drift.rs` — new.

**Changes:**

1. **`tests/showcase_dev.rs`** — start `zero dev` inside `showcase/`,
   request `GET /`, assert the body contains the home-route H1 and that the
   importmap mentions `"zero/components"`. Mirrors the existing
   `dev_serves_files.rs` pattern: bind to an ephemeral port via
   `[dev].port`, spawn the binary, hit the local URL, kill the process.

2. **`tests/showcase_build.rs`** — run `zero build` inside `showcase/` (use a
   `tempdir` copy of `showcase/` so the repo's `showcase/dist/` is not
   touched). Assert `dist/index.html` exists, that the hashed CSS in
   `dist/assets/` contains the literal string `@layer components`, and that
   the bundled JS contains `__zero_define('./.zero/components/index.ts'`.

3. **`tests/component_library.rs`** — run `zero test` inside the same
   `tempdir` copy. Assert the stdout reports all 14 component tests as
   passing (substring check on each component name plus a single
   `passed, 0 failed` end-of-report assertion).

4. **`tests/showcase_drift.rs`** — copy `showcase/` to a `tempdir`,
   delete the `.zero/` directory there, run `zero update --yes`, diff
   the regenerated tree against the committed `showcase/.zero/`. Fail
   if they differ. This is the CI-side enforcement of req 22.

**Tests:** Each file is itself a test; once they pass, the showcase is
locked in.

---

### Step 21: Documentation

**Goal:** Update the user-facing reference (`AGENTS.md`) and the
framework-internal spec so the new surface is documented and the Phase 9
checklist reflects landing.

**Files:**

- `src/scaffold/AGENTS.md`
- `zero-framework-spec.md`

**Changes:**

1. **`src/scaffold/AGENTS.md`:**
   - Insert a new section `## Components` after `## Styles` (and before
     `## The .zero/ directory`). Content per spec req 30:
     - Intro paragraph (one paragraph).
     - Import example: `import { Button, Input, Dialog } from "zero/components";`.
     - Table of all 14 components: name, one-line purpose, primary stateful
       prop type or `—`.
     - Four subsections (Form Inputs, Display, Overlay, Feedback), each with
       one usage example for the most representative component in that
       category.
     - Short paragraph on `@layer components` and how `styles/app.scss` rules
       automatically win.
     - Pointer to `showcase/` as the canonical live example.
   - Extend the `## The .zero/ directory` table with rows for every new path:
     `.zero/components/index.ts`, `.zero/components.d.ts`,
     `.zero/components/<Name>.ts`, `.zero/components/<Name>.test.ts`,
     `.zero/styles/_components.scss`, `.zero/styles/components/_<name>.scss`.
   - Update the import-paths intro at the top: change "two import paths" to
     "three import paths" and add `"zero/components"` to the bullet list.

   Add a new sentinel-presence test in `src/scaffold.rs` (extends the
   existing `write_initial_project_agents_md_has_section_sentinels`):
   include `"## Components"` in the sentinels list.

2. **`zero-framework-spec.md`:**
   - **§7.1 (Design system):** Append a short paragraph pointing at
     `@layer components` and the partial location
     `.zero/styles/components/`.
   - **§11 (Complete API Surface):** Add a new sub-section
     `"zero/components"` listing every named export.
   - **§12 (Roadmap checklist):** Mark Phase 9 items `[x]`.
   - **§13 (Key Design Decisions):** Append the row:
     ```
     | Component library | 14 components shipped under .zero/components/; CSS wrapped in @layer components | Real apps shouldn't rebuild the same primitives; @layer keeps user overrides predictable without prefixing |
     ```

**Tests:** The new AGENTS.md sentinel assertion. No other test changes —
documentation correctness is verified by review.

## Risks and Assumptions

- **`?attr=` boolean-attribute syntax.** Multiple components (`Button`,
  `Input`, `Dialog`, etc.) want to bind boolean attributes (`disabled`,
  `open`, `aria-checked`). The per-step contract assumes the `html` runtime
  supports `?attr=${bool}` style binding; if it doesn't, the implementer
  falls back to the conditional-string-attribute pattern noted in the
  shared contract. Step 6 (`Button`) is the first to hit this and codifies
  the chosen pattern for the rest of the component steps. Worst case
  requires touching `runtime/template.js` — out of scope for this issue and
  would need a separate plan.
- **Discovery exception for `.zero/components/`.** The narrowed
  hidden-dir skip rule is load-bearing for the per-component `.test.ts`
  files (Steps 4–17) to be picked up by `zero test`. If a future framework
  feature also wants to ship tests under `.zero/`, the rule has to broaden
  again. Documented in code with a pointer to this plan.
- **Token coverage for badge/toast variants.** Success/warning/danger color
  tokens are not defined in `_tokens.scss` today. The `Badge` (Step 5) and
  `Toast` (Step 16) partials declare per-variant locals inside
  `@layer components` rather than expanding the token surface, on the
  theory that adding tokens to `_tokens.scss` belongs in a separate,
  smaller issue. If review prefers extending tokens, that's a one-line
  spec change.
- **Boa loader synthetic referrer.** Step 1's `"zero/components"` handler
  in the loader needs to load `<root>/.zero/components/index.ts` with the
  loader's existing path-relative semantics intact for the index file's own
  internal imports. The implementation note proposes a small helper rather
  than building a synthetic Referrer; if Boa's `Module::parse` does not honor
  the `.with_path(...)` value for relative-import resolution from cached
  modules, fall back to writing a small wrapper module string that
  side-effect-imports `./index.ts` from a synthetic referrer dir.
- **`showcase/.zero/` drift.** Committing the showcase's `.zero/` means
  every change to a manifest entry must be followed by a `zero update --yes`
  inside `showcase/` and a commit of the result. Step 20's `showcase_drift`
  test makes this a hard CI gate.
- **`framework_manifest()` length explosion.** Going from 8 to 53 entries
  is a 6× growth. Step 18's vector-of-paths assertion keeps the maintenance
  burden linear instead of relying on an opaque count. Subsequent additions
  (a 15th component, a new partial) require adding one line to the expected
  set.
