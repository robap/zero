# Plan: Table component

## Summary

Add a single `Table` to the shipped component library: sticky-header,
scrollable-body, declarative `columns` + reactive `rows`, with per-column
alignment/width, density, optional row-click, and empty/loading slots.
Sort, selection, pagination, filtering, virtualization, and resizable
columns are out of scope.

The build order is bottom-up: extend the runtime's `each()` to accept an
optional key function and reconcile rows by identity (Step 1, a
prerequisite); add the Table source + SCSS + tests in the scaffold
(Steps 2–4); register them in the manifest and the type-declaration
surface (Step 5); wire up the showcase (Step 6); refresh the
documentation (Step 7). Every step compiles and tests pass at the seam,
so the branch can be paused after any step.

## Prerequisites

The runtime's `each(signal, renderFn)` only takes two arguments. The
spec calls for `each(rows, row => ..., rowKey)` and the user has
confirmed that the prerequisite is to extend `each` first. This is
**Step 1** of this plan rather than a separate issue.

No other open questions block the work. Resolutions to the spec's open
questions (confirmed during refine):

- Generic threading: `Table<T>(props: TableProps<T>)`; `T` is inferred
  from `rows: Signal<T[]>`; `TableColumn<T>['key']` is narrowed to
  `keyof T & string`.
- Hover background: reuse `--color-surface` over the table's
  `--color-bg`. No new token.
- Sticky-header `z-index`: literal `1`, matching the rest of the
  roster.
- Empty-state default copy: ship a default ("No data") wrapped in a
  muted span. `empty?` overrides.
- Loading visual: keep both — overlay spinner *and* row dim (`opacity:
  0.5; pointer-events: none`). Cheap; mirrors what users would
  hand-roll.
- `table-fixed` class location: on the `<table>` element (spec
  Requirement 10 wins; Requirement 21 selector becomes
  `table.table-fixed { table-layout: fixed; }`).
- Showcase shape: three separate Table instances (main, empty,
  loading), matching every other showcase route's
  "show-all-variants" pattern.
- Body row `data-row-index`: yes; trivial cost, useful for tests and
  any per-row hooks a future issue needs.
- `TableProps` / `TableColumn` exports from `index.ts`: yes; matches
  the existing roster convention.

## Steps

- [x] **Step 1: Extend `each()` with an optional key function**
- [x] **Step 2: Add `.zero/components/Table.ts`**
- [x] **Step 3: Add `.zero/styles/components/_table.scss`**
- [x] **Step 4: Add `.zero/components/Table.test.ts`**
- [x] **Step 5: Register Table in the framework manifest and type surface**
- [x] **Step 6: Wire Table into the showcase**
- [x] **Step 7: Refresh documentation**

---

## Step Details

### Step 1: Extend `each()` with an optional key function

**Goal:** Give the runtime keyed list reconciliation so Table's `rowKey`
becomes a real reconciler hint, not future-proofing. Backward-compatible:
the two-argument form continues to do a from-scratch re-render.

**Files:**

- `runtime/template.js` — modify `each()` and `_commitEach()`.
- `runtime/template.test.js` — add keyed-reconciliation tests.
- `runtime/zero.d.ts` — broaden the exported signature.
- `crates/zero-scaffold/src/scaffold/AGENTS.md` — update the
  "each re-renders the whole list" paragraph in the `### Lists with
  each` section.
- `zero-framework-spec.md` — update the §11 reference to `each` and
  the Phase 2 implementation-priority bullet.

**Changes:**

- `each<T, K extends string | number = string | number>(source, render,
  keyFn?)` returns
  `{ _isEach: true, signal: source, renderFn: render, keyFn }`. The
  `keyFn` field is `undefined` when omitted.
- `_commitEach()` branches on `keyFn`:
  - **No `keyFn`** — preserves today's behavior verbatim
    (`_disposeItemScopes` + `_clearNodeContent` + full re-render).
  - **With `keyFn`** — keeps a `state.itemsByKey` map of
    `key -> { scope, nodes: Node[] }`. On each tick:
    1. Compute the new key sequence from `items` and `keyFn(item, i)`.
       If a key appears twice, throw a clear error
       (`each: duplicate key '{key}' in row {i}`) so silent
       reuse-wrong-DOM bugs become explicit failures, matching the
       spec's stated motivation.
    2. For each new key in order: if present in the old map, reuse its
       `{scope, nodes}` and (if its DOM position is out of order)
       move its nodes via `insertBefore`. If absent, create a fresh
       `_createScope()`, run `renderFn(item, i)` inside it, collect the
       resulting nodes, and insert them at the correct position.
    3. For each old key not in the new sequence, dispose its scope and
       remove its nodes from the DOM.
    4. Replace `state.itemsByKey` with the new map and rebuild
       `state.currentNodes` so the existing anchor-walking logic in
       `_nextSiblingAfter` and `_clearNodeContent` keeps working.
  - When the underlying *items array reference* changes but the keys
    are stable, the effect re-runs; the diff produces zero DOM
    mutations.
- `runtime/zero.d.ts`:
  ```ts
  export function each<T>(
    source: Signal<T[]> | Computed<T[]>,
    render: (item: T, index: number) => TemplateResult,
    key?: (item: T, index: number) => string | number,
  ): TemplateResult;
  ```
- `AGENTS.md` `### Lists with each` paragraph: replace "there is no
  keyed reconciliation today" with a short note that passing an
  optional `keyFn` enables in-place reuse and duplicate keys throw.
  Add one line to the signature: `each(signalOfArray, (item, index) =>
  TemplateResult, keyFn?)`.
- `zero-framework-spec.md` §11 `// Components` block: update the
  `each(...)` signature line. In §12 Phase 2 add a bullet:
  `- [x] Keyed each() reconciliation via optional keyFn`.

**Tests** (added to `runtime/template.test.js` inside the existing
`describe('each()', ...)` block):

1. *No `keyFn` — behavior unchanged.* Existing tests stay green.
2. *Reuses DOM on reordering.* Render `[{id:1,...}, {id:2,...},
   {id:3,...}]` with `keyFn: r => r.id`. Capture the rendered `<li>`
   nodes. `set([{id:3}, {id:1}, {id:2}])`. Assert the *same* DOM nodes
   appear in the new order (`nodes[0] === capturedFor3` etc.).
3. *Reuses DOM on update of unrelated fields.* Same keys, same order,
   different non-key fields. Assert nodes are identical references and
   per-row content updates if the render reads a signal.
4. *Removes nodes whose keys disappear.* `set([{id:1},{id:3}])` after
   `[{id:1},{id:2},{id:3}]`. Assert `<li>` count goes from 3 to 2 and
   the kept nodes are the original `id:1` and `id:3` references.
5. *Inserts new keys at the correct position.* `set([{id:1},
   {id:4},{id:2},{id:3}])` after `[{id:1},{id:2},{id:3}]`. Assert
   length 4 and that the only new node is `id:4`.
6. *Per-row scope is disposed when its key disappears.* Inside the
   row's `renderFn`, register an `effect` that pushes to a captured
   array on dispose. Remove the row. Assert the dispose fires.
7. *Duplicate keys throw.* Render an items array where two rows produce
   the same key. Expect a thrown error whose message contains the key
   and the offending index.

No new files; no scaffold-manifest changes. After this step `cargo test
--workspace` and `node --test runtime/*.test.js` pass.

---

### Step 2: Add `.zero/components/Table.ts`

**Goal:** The component source itself, callable as `Table({ columns,
rows, rowKey, ... })`. After this step the file exists in the scaffold
but is not yet referenced by the manifest — the workspace still
compiles.

**Files:**

- `crates/zero-scaffold/src/scaffold/.zero/components/Table.ts`
  (new file).

**Changes:**

- Imports:
  ```ts
  import { html, each } from "zero";
  import type { Signal, TemplateResult } from "zero";
  import Spinner from "./Spinner.ts";
  ```
- Exported types:
  ```ts
  export type TableDensity = "compact" | "cozy";

  export type TableColumn<T> = {
    key: keyof T & string;
    label: string;
    align?: "start" | "end" | "center";
    width?: string;
    render?: (row: T, i: number) => TemplateResult | string | number;
  };

  export type TableProps<T> = {
    columns: TableColumn<T>[];
    rows: Signal<T[]>;
    rowKey: (row: T, i: number) => string | number;

    onRowClick?: (row: T, i: number) => void;
    density?: TableDensity;
    maxHeight?: string;

    empty?: TemplateResult;
    loading?: Signal<boolean>;
  };
  ```
- Default export:
  ```ts
  export default function Table<T>(props: TableProps<T>): TemplateResult
  ```
- Body, in order:
  1. `const density: TableDensity = props.density ?? "cozy";`
  2. `const clickable = typeof props.onRowClick === "function";`
  3. `const hasFixedWidths = props.columns.some(c => c.width != null);`
  4. Build the outer container class string:
     ```ts
     const containerCls = ["table", `table-${density}`]
       .concat(clickable ? ["table-clickable"] : [])
       .join(" ");
     ```
     Build the loading class via a reactive class binding:
     `class=${() => clsBase + (props.loading?.val ? " table-loading" : "")}`.
     (Keep this as a single reactive expression on the
     element — the runtime supports a function value for an attribute
     binding.)
  5. Build `containerStyle`: `props.maxHeight ? \`max-height:
     ${props.maxHeight}; overflow-y: auto\` : null`. Apply via
     `style=${containerStyle}` (omitted when `null`).
  6. Build `tableCls`: `hasFixedWidths ? "table-fixed" : ""` and apply
     to the `<table>`.
  7. Build `<thead>` as a plain `.map` over `props.columns`:
     ```ts
     const headerCells = props.columns.map(c => {
       const cls = "table-th" + (c.align ? ` table-align-${c.align}` : "");
       const style = c.width ? `width: ${c.width}` : null;
       return html`<th class=${cls} style=${style}>${c.label}</th>`;
     });
     ```
  8. Build the body via `each(props.rows, renderRow, props.rowKey)` —
     this is where Step 1's keyFn pays off. `renderRow(row, i)` returns
     ```ts
     html`<tr class="table-row" data-row-index=${i}
            @click=${clickable ? () => props.onRowClick!(row, i) : null}>
       ${props.columns.map(c => {
         const cls = "table-td" + (c.align ? ` table-align-${c.align}` : "");
         const content = c.render
           ? c.render(row, i)
           : (row[c.key] as unknown as string | number);
         return html`<td class=${cls}>${content}</td>`;
       })}
     </tr>`;
     ```
  9. Empty-state row, wrapped in a reactive block so it appears and
     disappears as `rows.val.length` crosses zero:
     ```ts
     const emptyRow = () => props.rows.val.length === 0
       ? html`<tr class="table-empty"><td colspan=${props.columns.length}>${
           props.empty ?? html`<span class="text-muted">No data</span>`
         }</td></tr>`
       : null;
     ```
  10. Loading overlay, wrapped in a reactive block so the DOM is
      genuinely absent when not loading:
      ```ts
      const overlay = () => props.loading?.val
        ? html`<div class="table-loading-overlay">${Spinner({ size: "md" })}</div>`
        : null;
      ```
  11. Return:
      ```ts
      return html`<div class=${classBinding} style=${containerStyle}>
        <table class=${tableCls}>
          <thead><tr>${headerCells}</tr></thead>
          <tbody>${each(props.rows, renderRow, props.rowKey)}${emptyRow}</tbody>
        </table>
        ${overlay}
      </div>`;
      ```
- JSDoc block on the default export describing the API in one
  paragraph, matching the brevity of `Spinner` / `Tabs`. Both `@param`
  and `@returns` present. `@template T` for the generic.
- No top-level state; the function runs once per mount. The component
  never reads `props.rows.val` outside the empty-row reactive block or
  the `each()` call.

**Tests:** none in this step — the component is fully covered by
Step 4. The Rust workspace test for scaffold manifest path-set is
*intentionally not updated yet*, so this step leaves the path-set
assertion green (the file is unreferenced).

---

### Step 3: Add `.zero/styles/components/_table.scss`

**Goal:** The component's styles. Token-only, layered, no `!important`.
After this step `cargo test --workspace` continues to pass (the partial
is not yet `@use`'d).

**Files:**

- `crates/zero-scaffold/src/scaffold/.zero/styles/components/_table.scss`
  (new file).

**Changes:**

```scss
@layer components {
  .table {
    position: relative;
    height: 100%;
    overflow-y: auto;
    background: var(--color-bg);
    color: var(--color-text);
    border: var(--border-thin) solid var(--color-border);
    border-radius: var(--radius-md);
  }

  .table table {
    width: 100%;
    border-collapse: collapse;
    table-layout: auto;
  }
  .table table.table-fixed { table-layout: fixed; }

  .table thead th {
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--color-surface);
    color: var(--color-text);
    font-weight: var(--weight-semibold);
    text-align: start;
    border-block-end: var(--border-thin) solid var(--color-border);
  }

  .table .table-td {
    border-block-end: var(--border-thin) solid var(--color-border);
  }

  // Density.
  .table-cozy    .table-th, .table-cozy    .table-td { padding: var(--space-sm) var(--space-md); }
  .table-compact .table-th, .table-compact .table-td { padding: var(--space-xs) var(--space-sm); }

  // Alignment (logical only).
  .table-align-start  { text-align: start;  }
  .table-align-center { text-align: center; }
  .table-align-end    { text-align: end;    }

  // Clickable rows.
  .table-clickable .table-row { cursor: pointer; }
  .table-clickable .table-row:hover {
    background: var(--color-surface);
  }

  // Empty state.
  .table-empty td {
    text-align: center;
    color: var(--color-text-muted);
    padding: var(--space-lg);
  }

  // Loading.
  .table-loading .table-row {
    opacity: 0.5;
    pointer-events: none;
  }
  .table-loading-overlay {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--color-bg) 60%, transparent);
  }
}
```

Notes:

- `color-mix(...)` is the one non-token construct; it produces a
  consistent translucent veil from the existing `--color-bg` without
  introducing a new token. It is widely supported in current browsers
  the framework already targets (the existing Toast partial uses
  `color-mix` for the same reason — confirm during execution; if it
  does not, fall back to `background: var(--color-bg); opacity: 0.6;`
  on the overlay).
- `.table thead th` selects the inner element rather than relying on a
  `.table-th` class so the sticky positioning catches even rules that
  bypass the helper class (it complements, not replaces, `.table-th`).
- Per-density padding lives on `.table-th` and `.table-td` so the
  rule applies regardless of whether the user customizes the
  background.

**Tests:** the partial is not yet `@use`'d. `cargo test --workspace`
should pass.

---

### Step 4: Add `.zero/components/Table.test.ts`

**Goal:** Lock the component contract before it's shipped. Tests live
in the scaffold so they ride along into every project via the manifest
(consistent with the existing 14 components).

**Files:**

- `crates/zero-scaffold/src/scaffold/.zero/components/Table.test.ts`
  (new file).

**Changes:**

A single `describe("Table", ...)` block with `afterEach(cleanup)` and
one `it()` per case below. Helpers used: `render`, `find`, `findAll`,
`text`, `fire`, `spy`, `expect` from `"zero/test"`; `signal`, `html`
from `"zero"`.

A small fixture at the top of the file:

```ts
type User = { id: number; name: string; role: string; score: number };

const sample: User[] = [
  { id: 1, name: "Ada",     role: "admin", score: 92 },
  { id: 2, name: "Lin",     role: "user",  score: 78 },
  { id: 3, name: "Marcus",  role: "user",  score: 64 },
];

const rowKey = (r: User) => r.id;
```

Each `it()`:

1. *Renders base markup.* `rows = signal(sample); columns = [{key:
   "name", label:"Name"}, {key:"role", label:"Role"}]`. Assert
   `find(el, ".table")`, `find(el, "table")`, `findAll(el,
   ".table-th").length === 2`, `findAll(el, ".table-row").length ===
   3`. Default density: assert `.table` carries class `table-cozy`.
2. *Default cell content.* For a column without `render`, assert
   `text(rows[0], '.table-td')` reflects `row[key]`.
3. *Custom render.* A column with `render: r => html`<b>${r.name}</b>``
   produces a `<b>` inside the matching `<td>`. Assert via `find(el,
   ".table-row b")` and `text(el, ".table-row b")`.
4. *Row reactivity (and identity via the keyed each).* Capture the
   first `.table-row` DOM node. `rows.set([sample[2]!, sample[0]!,
   sample[1]!])`. Assert `findAll(el, ".table-row").length === 3`, and
   that the captured node is now the *second* `.table-row` (the one
   bound to `id: 1`).
5. *Empty state.* `rows.set([])`. Assert `findAll(el, ".table-row")`
   is empty, `find(el, ".table-empty")` exists, and `text(el,
   ".table-empty")` contains "No data".
6. *Custom empty slot.* Provide `empty: html`<span
   class="custom-empty">Nothing here</span>``. `rows.set([])`. Assert
   `find(el, ".custom-empty")` and text content.
7. *onRowClick.* `const onRowClick = spy();`. `fire(findAll(el,
   ".table-row")[0]!, "click")`. Assert
   `onRowClick.toHaveBeenCalledTimes(1)` and
   `onRowClick.toHaveBeenCalledWith(sample[0], 0)`. Assert `.table`
   carries `table-clickable`.
8. *Loading.* `const loading = signal(false);`. Render with `loading`.
   Assert `findAll(el, ".table-loading-overlay").length === 0` and
   `.table` does *not* carry `table-loading`. `loading.set(true)`.
   Assert overlay exists and `.table` carries `table-loading`.
9. *Alignment classes.* Column with `align: "end"`. Assert
   corresponding `.table-th` and every `.table-td` for that column
   carry `table-align-end`.
10. *Fixed layout.* Column with `width: "120px"`. Assert `find(el,
    "table").classList.contains("table-fixed")` (or — since the
    DOM-shim selector grammar lacks `.classList` checks — use a
    selector like `find(el, "table.table-fixed")` to assert presence)
    and that the corresponding `<th>` has an inline `width: 120px`
    style attribute.
11. *Density override.* Pass `density: "compact"`. Assert `.table`
    carries `table-compact` and does *not* carry `table-cozy`.
12. *Duplicate-key guard (cross-validates Step 1).* Provide a
    `rowKey` that returns the same value for two rows. Wrap the
    `render(Table(...))` in a function and assert it throws (matcher
    `.toThrow("duplicate key")`).

After this step the test file exists in the scaffold but the manifest
does not yet list it; the Rust scaffold tests pass.

---

### Step 5: Register Table in the framework manifest and type surface

**Goal:** Make the three new files part of every project's `.zero/`
output, so `zero update --yes` materializes them. This is the single
step that flips Table from "files on disk" to "shipped". After this
step, `cargo test --workspace` exercises the new manifest entries.

**Files:**

- `crates/zero-scaffold/src/lib.rs` — three new `TPL_TABLE_*`
  constants, three new `framework_manifest()` entries, three new
  paths in the `framework_manifest_matches_expected_path_set` test's
  `expected` set.
- `crates/zero-scaffold/src/scaffold/.zero/components/index.ts` — add
  Table export.
- `crates/zero-scaffold/src/scaffold/.zero/components.d.ts` — add the
  Table declaration.
- `crates/zero-scaffold/src/scaffold/.zero/styles/_components.scss` —
  add `@use 'components/table';`.

**Changes:**

In `crates/zero-scaffold/src/lib.rs`, after the `TPL_TABS_*` block (so
the alphabetical order is preserved by re-using "Ta...bs / Ta...ble"
juxtaposition — actually `Table` comes after `Tabs` alphabetically, so
inserted directly *after* `TPL_TABS_SCSS`):

```rust
const TPL_TABLE_TS: &str = include_str!("scaffold/.zero/components/Table.ts");
const TPL_TABLE_TEST_TS: &str = include_str!("scaffold/.zero/components/Table.test.ts");
const TPL_TABLE_SCSS: &str = include_str!("scaffold/.zero/styles/components/_table.scss");
```

In `framework_manifest()`, after the three `Tabs` entries:

```rust
(".zero/components/Table.ts", TPL_TABLE_TS),
(".zero/components/Table.test.ts", TPL_TABLE_TEST_TS),
(".zero/styles/components/_table.scss", TPL_TABLE_SCSS),
```

In the `framework_manifest_matches_expected_path_set` test
(`crates/zero-scaffold/src/lib.rs`, around L848), insert the three
paths after the `Tabs` entries inside the `expected` set; update the
comment that reads `// 14 components × (source, test, scss partial) =
42 entries.` to `// 15 components × (source, test, scss partial) = 45
entries.`. No other length-coupled assertion exists in the codebase —
the grep confirmed `expected.len()` is what the assertion compares
against, so adding three paths to the set is sufficient.

In `crates/zero-scaffold/src/scaffold/.zero/components/index.ts`,
after the `Tabs` block:

```ts
export { default as Table } from "./Table.ts";
export type { TableColumn, TableDensity, TableProps } from "./Table.ts";
```

In `crates/zero-scaffold/src/scaffold/.zero/components.d.ts`, after
the `TabsProps` declaration:

```ts
export type TableDensity = "compact" | "cozy";
export type TableColumn<T> = {
  key: keyof T & string;
  label: string;
  align?: "start" | "end" | "center";
  width?: string;
  render?: (row: T, i: number) => TemplateResult | string | number;
};
export type TableProps<T> = {
  columns: TableColumn<T>[];
  rows: Signal<T[]>;
  rowKey: (row: T, i: number) => string | number;
  onRowClick?: (row: T, i: number) => void;
  density?: TableDensity;
  maxHeight?: string;
  empty?: TemplateResult;
  loading?: Signal<boolean>;
};
export function Table<T>(props: TableProps<T>): TemplateResult;
```

In `crates/zero-scaffold/src/scaffold/.zero/styles/_components.scss`,
after the `tabs` line:

```scss
@use 'components/table';
```

**Tests:** `cargo test --workspace` runs the
`framework_manifest_matches_expected_path_set` test against the new
set; verifies the three paths are present and that the manifest has no
duplicate keys. `node --test runtime/*.test.js` still passes (Step 1
was backward-compatible). At this point a fresh `zero init` would
emit Table.

---

### Step 6: Wire Table into the showcase

**Goal:** Demonstrate the component in the running showcase project
and exercise the build/dev/test integration tests. The showcase's
`.zero/` is gitignored — `prepare_showcase()` runs `zero update --yes`
so the integration tests pick up Step 5's manifest automatically. The
*user-owned* showcase files (`src/app.ts`, `src/routes/`) are the only
ones we touch here.

**Files:**

- `showcase/src/routes/table.ts` (new file).
- `showcase/src/app.ts` — register the route.
- `showcase/src/routes/home.ts` — add the nav link.
- `crates/zero/tests/component_library.rs` — append `"Table"` to the
  hard-coded component-name list.

**Changes:**

- New `showcase/src/routes/table.ts`. Three Table instances inside one
  `<main class="showcase-page stack pad-xl">`:
  1. **Main instance** with `maxHeight: "320px"`. Six to ten sample
     `User`-like rows (`name`, `email`, `role`, `score`).
     - Columns: `name`, `email`, `role` (rendered as `Badge({
       variant: row.role === "admin" ? "primary" : "default",
       children: row.role })`), `score` (with `align: "end"`),
       `email` (with `width: "240px"` to demonstrate fixed layout).
     - `onRowClick: (row) => clicked.set(row.name)`; below the
       table, a `<p>` reads `${() => clicked.val ? \`Last
       clicked: ${clicked.val}\` : "Click a row"}`.
     - `rowKey: r => r.id`.
  2. **Empty-state instance.** Same columns. `rows: signal<User[]>([])`.
     Optional `empty: html`<span>No users to display yet.</span>``.
  3. **Loading instance.** `rows: signal(sample.slice(0,3))`, `loading:
     signal(false)`, plus a `Button({ children: "Toggle loading",
     onClick: () => loading.update(v => !v) })`.
- Each instance prefaced by a small heading
  (`<h2 class="text-h2">…</h2>`) so the three are skim-friendly.
- A trailing `<a class="showcase-nav-link" href="/">Back</a>` mirrors
  every other showcase route.
- `showcase/src/app.ts`: add `import TableRoute from
  "./routes/table.ts";` next to the other imports (alphabetical: after
  `TabsRoute`, before `TextAreaRoute`). Register
  `app.route("/table", TableRoute);` in the same alphabetical position.
- `showcase/src/routes/home.ts`: add `{ name: "Table", href: "/table"
  }` to the `components` array in alphabetical position (after
  `Tabs`, before `TextArea`).
- `crates/zero/tests/component_library.rs`: the hard-coded
  for-loop list (`["Avatar", ..., "Toggle"]`) gains `"Table"` after
  `"Tabs"`. After this change the test asserts the `Table.test.ts`
  output appears in `zero test`'s stdout.

**Tests:**

- `cargo test -p zero --test showcase_build` — the showcase's
  production build now bundles `Table.ts` + the SCSS partial; the
  test's existing assertions (asset emission, `@layer components` in
  CSS) cover the new file without modification.
- `cargo test -p zero --test showcase_dev` — the dev-server importmap
  test still passes; the importmap entry for `"zero/components"` is
  unchanged.
- `cargo test -p zero --test component_library` — picks up
  `Table.test.ts` from the manifest and the new `"Table"` name from
  the for-loop.
- After this step a developer running `cd showcase && zero update
  --yes && zero dev` sees the `/table` route alongside the others.

---

### Step 7: Refresh documentation

**Goal:** Bring the three authoritative documents in line with what
shipped, so the next reader of the repo (human or agent) finds Table
documented identically to its peers.

**Files:**

- `crates/zero-scaffold/src/scaffold/AGENTS.md` — `## Component
  library` table + a one-instance usage example in the relevant
  category.
- `zero-framework-spec.md` — §11 component listing + §12 Phase 9
  bullet.
- `BEST_PRACTICES.md` — *no change* (no non-obvious Table idiom
  worth pinning yet).

**Changes:**

- `AGENTS.md`:
  - The component-roster table in `## Component library` gains a row:
    ```
    | `Table`    | Sticky-header data table.               | `rows: Signal<T[]>`, `loading?: Signal<boolean>` |
    ```
    inserted alphabetically after `Tabs`.
  - Under `### Display` (or a new `### Data` subsection — choose
    `### Data` to avoid bloating the existing example), add a short
    Table example:
    ```ts
    import { html, signal } from "zero";
    import { Table } from "zero/components";

    const rows = signal([{ id: 1, name: "Ada", role: "admin" }]);
    Table({
      columns: [
        { key: "name", label: "Name" },
        { key: "role", label: "Role" },
      ],
      rows,
      rowKey: r => r.id,
    });
    ```
  - In the `## The .zero/ directory` shipped-files table, bump the
    parenthetical "(14 total)" counts on the two `<Name>` rows to
    "(15 total)".
  - In `## Best practices` `### Use zero/components for every
    interactive primitive` list, insert `Table` after `Tabs`.

- `zero-framework-spec.md`:
  - §11 `// Components` block: insert a new `// Data` group above
    `// Display` and add the Table signature line:
    ```ts
    // Data
    Table({ columns, rows, rowKey, ... })  // sticky-header table over a Signal<T[]>
    ```
  - §12 Phase 9 line: change `14 shipped` to `15 shipped` and add
    `Table` at the alphabetical position in the parenthetical list.
    Bump the surrounding "fourteen components" mention in the §11
    prose if present (rg flagged none — re-verify in execution).

- `BEST_PRACTICES.md`: no change. Re-evaluate only if execution
  surfaces a non-obvious idiom (e.g. an unexpected interaction
  between `loading` and `each`'s keyed reconciliation). If so, add a
  short subsection under whichever existing top-level section
  matches.

**Tests:** none specific to docs. The full Rust + Node test runs
performed after each prior step continue to pass.

---

## Risks and Assumptions

- **Keyed `each()` reconciliation is the largest single change.** Step
  1 ships a true reconciler (the existing implementation only
  re-renders from scratch). If the keyed path has a subtle bug —
  duplicate-detection corners, mid-list insertion, anchor walking
  across mixed `each`/non-`each` siblings — it can corrupt arbitrary
  list rendering across the framework. Mitigation: keep the no-`keyFn`
  branch byte-identical to today's `_commitEach`; the new code path
  only runs when callers opt in. Cover the keyed branch with the
  seven tests in Step 1 before moving on.
- **The `_themes.scss` / token surface is unchanged.** The plan
  reuses `--color-surface` over `--color-bg` for hover. If a future
  audit decides this contrast is insufficient, we add a
  `--color-surface-hover` token in its own issue; Table flips to
  reading the new token via a single SCSS edit.
- **`color-mix()` in the loading overlay is the one non-token
  fragment.** Browser support is assumed (the spec doesn't pin a
  browser baseline). If the existing `_toast.scss` uses `color-mix`,
  precedent is established; if not, the execute step swaps to
  `background: var(--color-bg); opacity: 0.6` on the overlay.
- **DOM-shim `classList` reliance in tests.** The test runner's
  lightweight DOM may not implement `Element.classList`. Step 4 plans
  around this by using selector-based assertions (e.g. `find(el,
  "table.table-fixed")`) instead of `.classList.contains`. If a
  selector form is needed that the shim's grammar does not support
  (combinators, pseudo-classes), the test falls back to reading
  `el.getAttribute("class")` and `.includes`.
- **Two manifest length assertions exist (`framework_manifest`
  path-set and the binary manifest). Only the framework one needs a
  bump.** The binary manifest is unaffected (no new fonts). Confirmed
  via grep.
- **Showcase's `.zero/` is gitignored but exists locally.** After
  Step 5 the developer must run `cd showcase && zero update --yes`
  locally to refresh their on-disk `.zero/` so `zero dev` picks up
  Table. The integration tests do this automatically; no commit needed.
- **Inferring `T` from `rows` alone leaves a soft seam.** Callers
  passing a `Signal<unknown[]>` (e.g. via `inject`) lose column-key
  narrowing. The plan accepts this — the alternative (deriving `T`
  from `columns`) makes the column array's element type drive
  inference, which produces worse error messages. If a real call site
  hits the soft seam, the user pins `T` with `Table<MyRow>({...})`
  explicitly.
