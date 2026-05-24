# Plan: Pagination accepts `Computed<number>` for `totalPages`

## Summary

`Pagination.ts` and `Combobox.ts` each carry a copy-pasted `isSignal` duck-type
that requires both `.val` *and* `.set`, which silently rejects `Computed<T>`.
The fix extracts a shared `_internal.ts` helper module exposing `Reactive<T>`,
`isReactive`, and `read` — the duck-type checks `.val` only — then deletes the
two duplicates and widens the affected prop types (`Pagination.totalPages`,
`Pagination.disabled`, `Combobox.disabled`) to accept `Computed<T>`. The new
file is wired into the scaffold manifest so `zero init` / `zero update` ship it,
and the public-facing `.zero/components.d.ts` is updated so TS users can
actually pass a `Computed`. Each component file is its own refactor step so
intermediate commits stay green.

## Prerequisites

None. Resolutions to the spec's open questions:

- **Filename.** Use `_internal.ts` as recommended. Modern TS/swc/Node ESM
  resolve underscore-prefixed files without issue; the leading underscore
  matches the SCSS-partial naming convention already used in the scaffold
  and clearly signals "not a public export."
- **`index.ts` regeneration.** `crates/zero-scaffold/src/scaffold/.zero/components/index.ts`
  is hand-maintained (read at planning time: per-line `export { default as X } from "./X.ts";`
  entries). No change needed — leave `_internal.ts` out of the re-exports.
- **`Reactive<T>` placement.** Keep inside `_internal.ts`. Not added to
  `runtime/zero.d.ts`. Not re-exported through `"zero/components"`.
- **`components.d.ts` ambient declarations.** *Do* need editing: the prop
  type unions for `PaginationProps.totalPages`, `PaginationProps.disabled`,
  and `ComboboxProps.disabled` are duplicated in the ambient module
  declaration. Widening only the source files leaves TS users with the old
  narrow type. The ambient file inlines `Signal<T> | Computed<T> | T`
  rather than importing `Reactive` — `_internal` is not a public surface.

## Steps

- [ ] **Step 1: Create `_internal.ts` with `Reactive`/`isReactive`/`read` and its test file**
- [ ] **Step 2: Ship `_internal.ts` via the scaffold manifest**
- [ ] **Step 3: Refactor `Pagination.ts` to use shared helpers and widen prop types**
- [ ] **Step 4: Refactor `Combobox.ts` to use shared helpers and widen prop type**
- [ ] **Step 5: Widen `components.d.ts` ambient prop types to accept `Computed`**
- [ ] **Step 6: Document `Computed` acceptance in `docs/components.md`**

---

## Step Details

### Step 1: Create `_internal.ts` with `Reactive`/`isReactive`/`read` and its test file

**Goal:** Land the shared helper module and its direct unit tests before any
component starts importing from it, so subsequent steps can rely on a tested
helper. Runs in isolation — no other file changes — and keeps the repo
compilable because nothing imports it yet.

**Files:**
- Create `crates/zero-scaffold/src/scaffold/.zero/components/_internal.ts`
- Create `crates/zero-scaffold/src/scaffold/.zero/components/_internal.test.ts`

**Changes:**

`_internal.ts` exports three names. Full content:

```ts
import type { Signal, Computed } from "zero";

/**
 * Either a writable signal or a read-only computed of the same value.
 * Component props that say "reactive or plain" should accept this plus `T`.
 *
 * @template T
 * @internal
 */
export type Reactive<T> = Signal<T> | Computed<T>;

/**
 * Duck-types a prop value as a `Reactive<T>` (anything with a `.val`
 * getter). Crucially does NOT require a `.set` — that would exclude
 * `Computed`, which is the bug this helper exists to fix.
 *
 * @template T
 * @param p Prop value, either reactive-wrapped or plain.
 * @returns
 * @internal
 */
export function isReactive<T>(p: Reactive<T> | T): p is Reactive<T> {
  return typeof p === "object" && p !== null && "val" in p;
}

/**
 * Read a reactive-or-plain prop, returning the underlying value.
 *
 * @template T
 * @param p
 * @returns
 * @internal
 */
export function read<T>(p: Reactive<T> | T): T {
  return isReactive(p) ? p.val : p;
}
```

`_internal.test.ts` covers the contract directly:

```ts
import { describe, it, expect } from "zero/test";
import { signal, computed } from "zero";
import { isReactive, read } from "./_internal.ts";

describe("_internal", () => {
  describe("isReactive", () => {
    it("returns false for plain primitives", () => {
      expect(isReactive(5)).toBe(false);
      expect(isReactive("hi")).toBe(false);
      expect(isReactive(true)).toBe(false);
    });
    it("returns false for null and undefined", () => {
      expect(isReactive(null as unknown as number)).toBe(false);
      expect(isReactive(undefined as unknown as number)).toBe(false);
    });
    it("returns false for plain objects without .val", () => {
      expect(isReactive({ x: 1 } as unknown as number)).toBe(false);
    });
    it("returns true for signals", () => {
      expect(isReactive(signal(5))).toBe(true);
    });
    it("returns true for computeds", () => {
      expect(isReactive(computed(() => 7))).toBe(true);
    });
  });
  describe("read", () => {
    it("returns plain primitives unchanged", () => {
      expect(read(5)).toBe(5);
      expect(read("hi")).toBe("hi");
      expect(read(false)).toBe(false);
    });
    it("returns null/undefined unchanged without crashing", () => {
      expect(read(null as unknown as number)).toBe(null);
      expect(read(undefined as unknown as number)).toBe(undefined);
    });
    it("returns signal.val", () => {
      expect(read(signal(5))).toBe(5);
    });
    it("returns computed.val", () => {
      expect(read(computed(() => 7))).toBe(7);
    });
  });
});
```

Both helpers are one-liners — well under the 80-line guideline. JSDoc is
present on every export per CLAUDE.md, with `@internal` markers.

**Tests:**
- The new `_internal.test.ts` cases above.
- This step also must keep the existing Rust `framework_manifest_matches_expected_path_set`
  test passing — but at this point `_internal.ts` is **not** yet in the
  manifest, so it's not on disk after `zero init` runs in the test, and
  the path-set assertion still matches. (Wiring lands in Step 2.) The new
  files exist only as authoring source; they aren't picked up by anything
  yet, so `cargo test --workspace` stays green.

Run `cargo run -p zero -- test _internal.test.js` to exercise the new
helpers — but note tests live under the scaffold tree, which the runtime
test discovery may or may not reach directly. Realistically, validation
of the new tests happens in Step 2's end-to-end check once the scaffold
ships the file. For now, the file is authored and asserted by inspection.

### Step 2: Ship `_internal.ts` via the scaffold manifest

**Goal:** Make `_internal.ts` ride along with every `zero init` and `zero
update` so user projects actually get the file. Until this step runs, the
file is only in the source tree of `zero-scaffold` and won't be present in
any materialized project. After this step, the file is on disk in every
fresh scaffold and the manifest's path-set test asserts it.

**Files:**
- `crates/zero-scaffold/src/lib.rs`

**Changes:**

1. Add a template constant alongside the per-component `TPL_*` block (around
   line 87, just after `TPL_TOGGLE_SCSS`), keeping alphabetical-by-name order
   within "non-component framework files." Place it with the small group of
   non-component framework constants near line 31 — between
   `TPL_COMPONENTS_INDEX_TS` and `TPL_COMPONENTS_DTS` reads naturally:

   ```rust
   const TPL_COMPONENTS_INTERNAL_TS: &str =
       include_str!("scaffold/.zero/components/_internal.ts");
   ```

2. Add an entry in `framework_manifest()` (around line 145, next to the
   `.zero/components/index.ts` and `.zero/components.d.ts` entries):

   ```rust
   (".zero/components/_internal.ts", TPL_COMPONENTS_INTERNAL_TS),
   ```

3. Extend the `framework_manifest_matches_expected_path_set` test (lines
   957-1042). Add `".zero/components/_internal.ts"` to the `expected`
   set. This is the test that prevents path-set drift and *will* fail until
   the entry is added, so update it in the same step.

4. Do **not** add `_internal` to the `COMPONENT_NAMES` array (lines 319-337).
   That array drives several iterating tests that assert each name has a
   matching `.ts`, `.test.ts`, and `_*.scss` partial — `_internal` has the
   first two but no SCSS partial, no per-name re-export in `index.ts`, and
   no `components.d.ts` declaration. Treating it as a "component" would
   break those tests.

5. Do **not** modify `.zero/components/index.ts` — leading-underscore
   files are deliberately not re-exported.

**Tests:**
- `framework_manifest_matches_expected_path_set` — passes after the update,
  asserts `_internal.ts` is part of the framework's text-file manifest.
- The existing iterating tests (`components_index_re_exports_each_listed`,
  `component_source_files_emitted`, etc.) keep passing because they iterate
  `COMPONENT_NAMES`, which we deliberately don't extend.
- After this step, running `cargo test -p zero-scaffold` builds a fresh
  scaffold into a temp directory and `_internal.ts` is present in it. The
  JS test from Step 1 (`_internal.test.ts`) is now reachable to any user
  who runs `zero test` inside a fresh scaffold; it is *not* automatically
  run as part of `cargo test`. Validate manually:
  `cargo run -p zero -- test _internal.test.ts` from the scaffold output
  if you want end-to-end confirmation, but the Rust path-set assertion is
  the load-bearing automated check.

### Step 3: Refactor `Pagination.ts` to use shared helpers and widen prop types

**Goal:** Fix the real user-visible bug — `Pagination({ totalPages:
computed(...) })` now works. Done as its own step so the diff is small and
the test additions are scoped.

**Files:**
- `crates/zero-scaffold/src/scaffold/.zero/components/Pagination.ts`
- `crates/zero-scaffold/src/scaffold/.zero/components/Pagination.test.ts`

**Changes:**

In `Pagination.ts`:

1. Update the top-of-file import. Current:
   ```ts
   import { html } from "zero";
   import type { Signal, TemplateResult } from "zero";
   ```
   becomes:
   ```ts
   import { html } from "zero";
   import type { Signal, TemplateResult } from "zero";
   import { read, type Reactive } from "./_internal.ts";
   ```
   `isReactive` is not needed in this file (only `read` is called).

2. Widen the prop types in `PaginationProps` (lines 6-17):
   - `totalPages: Signal<number> | number;` → `totalPages: Reactive<number> | number;`
   - `disabled?: Signal<boolean> | boolean;` → `disabled?: Reactive<boolean> | boolean;`

3. Delete the local helpers — `isSignal` (lines 19-34) and `read` (lines
   36-46). The `range` helper (lines 48-59) stays; it's unrelated.

4. The call sites at line 121 (`read(props.totalPages)`) and line 128
   (`read(props.disabled)`) are unchanged. The imported `read` has the
   same signature shape.

5. Update the component-level JSDoc (lines 100-110) to mention `Computed`.
   One-line edit to the existing sentence:
   "`totalPages` and `disabled` accept a `Signal`, a `Computed`, or a plain
   value so async parents can update them without remount."

In `Pagination.test.ts`, add two new `it()` blocks inside the existing
`describe("Pagination", …)` (insert after the "reactive totalPages signal
updates the list without remount" case at line 145):

```ts
it("accepts a computed totalPages and renders the right page list", () => {
  const page = signal(1);
  const raw = signal(5);
  const totalPages = computed(() => raw.val);
  const el = render(Pagination({ page, totalPages }));
  expect(pageBtns(el).map(btnText)).toEqual(["1", "2", "3", "4", "5"]);
});

it("recomputes when the computed totalPages dependency changes", () => {
  const page = signal(1);
  const raw = signal(3);
  const totalPages = computed(() => raw.val);
  const el = render(Pagination({ page, totalPages }));
  const nav = find(el, "nav.pagination")!;
  expect(pageBtns(el).map(btnText)).toEqual(["1", "2", "3"]);
  raw.set(5);
  expect(find(el, "nav.pagination")).toBe(nav);
  expect(pageBtns(el).map(btnText)).toEqual(["1", "2", "3", "4", "5"]);
});
```

Update the test file's imports to add `computed`:
```ts
import { signal, computed } from "zero";
```

**Tests:**
- Two new Pagination cases above directly cover the bug fix and the
  reactivity guarantee.
- All existing Pagination cases keep passing because the new `read` is a
  superset of the old one — anything that worked before still works.
- `cargo test -p zero-scaffold` continues to pass because the manifest
  test already accounts for `Pagination.ts` and `Pagination.test.ts`, and
  the iterating assertions (e.g. `class="pagination"` substring) are
  unaffected by the edit.

### Step 4: Refactor `Combobox.ts` to use shared helpers and widen prop type

**Goal:** Same bug, same fix, second file. Independent of Step 3.

**Files:**
- `crates/zero-scaffold/src/scaffold/.zero/components/Combobox.ts`
- `crates/zero-scaffold/src/scaffold/.zero/components/Combobox.test.ts`

**Changes:**

In `Combobox.ts`:

1. Update imports. Current:
   ```ts
   import { html, signal, effect, ref } from "zero";
   import type { Signal, TemplateResult, Ref } from "zero";
   ```
   becomes (add the helper import line):
   ```ts
   import { html, signal, effect, ref } from "zero";
   import type { Signal, TemplateResult, Ref } from "zero";
   import { read, type Reactive } from "./_internal.ts";
   ```

2. Widen the `disabled` prop type (line 18):
   - `disabled?: Signal<boolean> | boolean;` → `disabled?: Reactive<boolean> | boolean;`

3. Delete the local `isSignal` (lines 26-41) and `read` (lines 43-53)
   definitions.

4. Existing call sites (`read(ctx.props.disabled)` and
   `read(props.disabled)` in `wrapperCls`, `handleKey`, `handleInput`,
   `handleFocus`, `registerDisabledWatch`, `scheduleFetch`, and the
   `disabled` attribute binding on the input) are unchanged.

In `Combobox.test.ts`, add one `it()` block inside the existing
`describe("Combobox", …)`:

```ts
it("accepts a computed disabled and toggles when it flips", () => {
  const value = signal("");
  const guard = signal(false);
  const disabled = computed(() => guard.val);
  const el = render(
    Combobox({ value, loadOptions: staticLoader(ABC), disabled }),
  );
  const input = find(el, "input.combobox-input")!;
  expect(input.hasAttribute("disabled")).toBe(false);
  guard.set(true);
  expect(input.hasAttribute("disabled")).toBe(true);
});
```

Update the test file's imports to add `computed`:
```ts
import { signal, computed } from "zero";
```

**Tests:**
- The new Combobox case directly verifies the `Computed<boolean>` path.
- All existing Combobox cases continue to pass — the `read` semantics
  are a superset of the old `read`.

### Step 5: Widen `components.d.ts` ambient prop types to accept `Computed`

**Goal:** Without this step, runtime acceptance works but TS users see a
compile error: the ambient module declaration in `.zero/components.d.ts`
still types `totalPages` and `disabled` as `Signal<T> | T`. This step
widens the ambient types to match the source types in Steps 3 and 4. Kept
as its own step because it's a different file with a different shape
(ambient declarations, no implementation) and lands a focused TS-typing
change.

**Files:**
- `crates/zero-scaffold/src/scaffold/.zero/components.d.ts`

**Changes:**

1. Extend the `Signal` import (line 5) to also bring in `Computed`:
   ```ts
   import type { Signal, Computed, TemplateResult } from "zero";
   ```

2. Update `PaginationProps` (lines 100-111):
   - `totalPages: Signal<number> | number;` → `totalPages: Signal<number> | Computed<number> | number;`
   - `disabled?: Signal<boolean> | boolean;` → `disabled?: Signal<boolean> | Computed<boolean> | boolean;`

3. Update `ComboboxProps` (line 61):
   - `disabled?: Signal<boolean> | boolean;` → `disabled?: Signal<boolean> | Computed<boolean> | boolean;`

The ambient file deliberately does not import `Reactive` from
`_internal`. `_internal` is not part of the `"zero/components"` public
surface; inlining the three-way union is more honest about what callers
may pass and matches the pattern already used elsewhere in this file
(e.g. `Signal<T[]> | Computed<T[]>` is the existing convention).

**Tests:**
- `components_dts_declares_each_listed` (existing) keeps passing — it
  only checks that `Pagination(` / `Combobox(` function declarations exist.
- Add a small assertion to `crates/zero-scaffold/src/lib.rs`'s test
  module. Place near the other `*_dts_*` tests (around line 468):
  ```rust
  #[test]
  fn components_dts_accepts_computed_for_widened_props() {
      let (_dir, root) = fresh_scaffold();
      let dts = fs::read_to_string(root.join(".zero/components.d.ts")).unwrap();
      assert!(
          dts.contains("totalPages: Signal<number> | Computed<number> | number"),
          "components.d.ts: PaginationProps.totalPages must accept Computed: {dts}"
      );
      assert!(
          dts.contains("disabled?: Signal<boolean> | Computed<boolean> | boolean"),
          "components.d.ts: disabled must accept Computed: {dts}"
      );
      assert!(
          dts.contains("import type { Signal, Computed, TemplateResult } from \"zero\""),
          "components.d.ts must import Computed alongside Signal: {dts}"
      );
  }
  ```
- `cargo test -p zero-scaffold` runs the new assertion plus existing ones.

### Step 6: Document `Computed` acceptance in `docs/components.md`

**Goal:** Make the new shape discoverable. The friction-log entry that
caught this bug was driven by a user *not knowing* `Computed` was rejected;
the doc fix closes that loop.

**Files:**
- `docs/components.md`

**Changes:**

1. Update the `Combobox` and `Pagination` rows in the components table
   (lines 159 and 162):

   - `Combobox` row's required-props column: change `disabled` (currently
     listed bare in the optional list) to be explicit. The current text is:

     > `value: Signal<string>`, `loadOptions: (q) => Promise<ComboboxOption[]>`;
     > optional `initialLabel`, `size`, `placeholder`, `label`, `disabled`,
     > `debounceMs`, …

     Leave the list intact; the next bullet (point 2) handles the type
     widening narrative. The table itself stays terse.

   - `Pagination` row: change
     `` `totalPages: Signal<number> \| number` `` →
     `` `totalPages: Signal<number> \| Computed<number> \| number` ``.

2. After the table (the existing "The convention across the library:" list
   starts around line 175), add a short paragraph under that list — or
   append a bullet — explaining the widened shape. Something like:

   > Props typed `Signal<T> | T` accept a `Computed<T>` too where noted:
   > `Pagination.totalPages`, `Pagination.disabled`, and `Combobox.disabled`.
   > Pass a plain value when it's static, a `Signal` when the parent
   > mutates it, or a `Computed` when it's derived from other reactive
   > state (e.g. `computed(() => Math.ceil(totalCount.val / pageSize))`).

   Final wording is the author's call as long as it covers all three
   affected props and the derivation use case.

**Tests:**
- No automated test. Docs are markdown and aren't currently asserted.
- Verify manually: `grep -n "Computed<number>" docs/components.md` returns
  the updated `Pagination` row; `grep -n "computed(() => Math" docs/components.md`
  returns the new narrative paragraph.

## Risks and Assumptions

- **Assumption: leading-underscore `.ts` filenames are handled cleanly by
  the dev server transpile, the bundler graph walker, and the test
  discovery.** If any of those skip underscore-prefixed files (the way
  Sass skips underscore-prefixed `.scss` partials from direct compilation),
  the helper module will never load and Steps 3-4 will break at runtime.
  Mitigation: spot-check by running `cargo run -p zero -- test` against
  a fresh scaffold once Step 2 lands. If broken, rename `_internal.ts` →
  `reactive.ts` and update all the imports — a small mechanical change,
  contained to two source files and one manifest entry.

- **Assumption: the duck-type `typeof p === "object" && p !== null && "val"
  in p` is safe against all reasonable plain-value props.** False positive
  case: a user passes `{ val: 5 }` as a plain object expecting it to be
  treated as a value. This was already the behavior of the old `isSignal`
  (any object with `.val` that also had a `set` function would be
  unwrapped); the new check is strictly looser. Realistically no caller
  passes `{ val: ... }`-shaped plain objects to `totalPages` or `disabled`
  — they're `number` and `boolean` props. If a future "signal or POJO"
  prop appears, that prop should not use this helper.

- **Assumption: `.zero/components/index.ts` will not be regenerated by a
  tool that doesn't know to skip `_internal.ts`.** Verified during planning
  — `index.ts` is hand-maintained. If a future "regenerate component
  index" command is added, it must explicitly skip leading-underscore
  filenames; flag this in the regenerator's design.

- **Risk: `Reactive<T>` type alias becomes load-bearing for user code via
  `import { type Reactive } from "zero/components/_internal"`.** Spec says
  this is deliberately not a documented entry point. If users start
  importing it, that's a signal to promote it to a public type in a
  follow-up — at which point `runtime/zero.d.ts` is the right home.

- **Risk: the existing `each(source: Signal<T[]> | Computed<T[]>, …)`
  declaration in `runtime/zero.d.ts` is a precedent that could be
  refactored to use `Reactive`.** Out of scope per the spec; called out
  only so a future reader understands why the inline union persists.
