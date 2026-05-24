# Plan: Table sortable-header affordance

## Summary

Extend the shipped `Table` component with per-column sort. The contract stays
declarative and signal-controlled: `TableColumn<T>` gains a `sortable` flag plus
optional `compare`; `TableProps<T>` gains a `sort: Signal<SortState | null>`
plus an optional `onSortChange` callback. Mode is implicit — without
`onSortChange`, Table sorts a copy of `rows` client-side; with it, Table emits
intent and renders whatever the parent supplies. Click cycle is asc → desc →
null on the active column, jump-to-asc on any other sortable column. Header
buttons compose `.button .button-ghost .button-sm` (reuse design-system CSS),
with `aria-sort` on the `<th>` and a glyph indicator inside the button.

## Prerequisites

None. The spec's open questions are all planner choices, resolved below:

- `SortState` lives in `Table.ts` and is re-exported from `components/index.ts`
  (matches every other component's type-export pattern).
- Validation `throw`s at the top of `Table<T>(props)` (cheap, single entry
  point — fine to re-check per call).
- `sort.set(next)` runs before `onSortChange?.(next)` (matches `Pagination`
  precedent at `Pagination.ts:135-136`).
- Sort glyphs: `▲` (asc), `▼` (desc), `↕` (inactive). Inactive glyph is
  dimmed via `opacity: 0.4` in SCSS so it reads as "sortable but not active."
- `sort` stays `Signal<SortState | null>` — never widened to `Reactive<...>`,
  since server-side mode requires `.set` on the parent's signal.

## Steps

- [x] **Step 1: Extend public types for sort**
- [x] **Step 2: Implement sort behavior, header rendering, and tests**
- [x] **Step 3: Style the sortable header button and indicator**
- [x] **Step 4: Add a sortable variant to the Table showcase route**
- [x] **Step 5: Document the sort API in `docs/components.md`**

---

## Step Details

### Step 1: Extend public types for sort

**Goal:** Land the API shape — new column fields, new props, and the
`SortState` export — without changing any runtime behavior. Existing call sites
keep compiling because every new field is optional. This step is type-only so
the surface can be reviewed before implementation lands.

**Files:**
- `crates/zero-scaffold/src/scaffold/.zero/components/Table.ts`
- `crates/zero-scaffold/src/scaffold/.zero/components/index.ts`

**Changes:**

In `Table.ts`, add the `SortState` type and extend the two existing types.
Order: `TableDensity`, then `SortState`, then `TableColumn`, then `TableProps`.

```ts
export type SortState = { key: string; dir: "asc" | "desc" };

export type TableColumn<T> = {
  key: keyof T & string;
  label: string;
  align?: "start" | "end" | "center";
  width?: string;
  render?: (row: T, i: number) => TemplateResult | string | number;
  sortable?: boolean;
  compare?: (a: T, b: T) => number;
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
  sort?: Signal<SortState | null>;
  onSortChange?: (next: SortState | null) => void;
};
```

In `components/index.ts`, add `SortState` to the existing
`export type { TableColumn, TableDensity, TableProps } from "./Table.ts";` line:

```ts
export type { SortState, TableColumn, TableDensity, TableProps } from "./Table.ts";
```

No JSDoc additions yet — the implementation comments land in Step 2 alongside
the helpers.

**Tests:** None. This is a type-only change; the existing `Table.test.ts`
suite must still pass unchanged. The validation throw and behavioral tests
arrive in Step 2.

---

### Step 2: Implement sort behavior, header rendering, and tests

**Goal:** Wire the actual sort feature — sortable headers, click cycle, both
modes, default comparator, and the validation throw. All sort-related tests
land in this step so the implementation and its contract are reviewed together.

**Files:**
- `crates/zero-scaffold/src/scaffold/.zero/components/Table.ts`
- `crates/zero-scaffold/src/scaffold/.zero/components/Table.test.ts`

**Changes (Table.ts):**

Add `computed` to the imports from `"zero"`:

```ts
import { html, each, computed } from "zero";
```

Add three module-level helpers above `Table<T>`. Each gets full JSDoc per the
project's TS style rules.

1. `nextSortState(current, columnKey)` — pure function implementing the R4
   cycle table. Signature:
   ```ts
   function nextSortState(
     current: SortState | null,
     columnKey: string,
   ): SortState | null
   ```
   - `current == null` → `{ key: columnKey, dir: "asc" }`
   - `current.key !== columnKey` → `{ key: columnKey, dir: "asc" }`
   - `current.key === columnKey && current.dir === "asc"` → `{ key, dir: "desc" }`
   - `current.key === columnKey && current.dir === "desc"` → `null`

2. `defaultCompare<T>(key)` — returns a `(a: T, b: T) => number` comparator
   implementing R5 (nullish → last in asc; numbers via subtraction; strings via
   `localeCompare`; fallback `String()` coercion).

3. `sortedRows<T>(rows, state, columns)` — pure function that returns the
   visible row order. Signature:
   ```ts
   function sortedRows<T>(
     rows: T[],
     state: SortState | null,
     columns: TableColumn<T>[],
   ): T[]
   ```
   - `state == null` → returns `rows` (same reference, no copy — keeps
     keyed `each(...)` identity stable when there's no active sort).
   - Otherwise finds the column by `state.key`. If none found (stale state),
     returns `rows` unchanged.
   - Otherwise: `const cmp = col.compare ?? defaultCompare(col.key); const mul
     = state.dir === "desc" ? -1 : 1; return [...rows].sort((a, b) => mul *
     cmp(a, b));`

Refactor `Table<T>(props)`:

- Near the top of the function (after the existing `density` / `clickable` /
  etc. lines), insert the validation guard and signal capture:
  ```ts
  const anySortable = props.columns.some((c) => c.sortable === true);
  if (anySortable && props.sort == null) {
    throw new Error(
      "Table: at least one column has sortable: true but no sort prop was passed. " +
      "Pass sort: Signal<SortState | null> from the parent.",
    );
  }
  const sortSig = props.sort;
  ```

- Replace the existing `headerCells` block (lines 54–58) with a per-column
  `headerCell(c)` helper that branches on `c.sortable`. Keep
  `headerCells = props.columns.map(headerCell)` afterward. The non-sortable
  branch returns the exact current markup; the sortable branch returns:
  ```html
  <th class="${cls}" style="${style}" aria-sort=${ariaSortFor(c.key)}>
    <button
      type="button"
      class="button button-ghost button-sm table-sort-btn"
      @click=${() => cycleSort(c.key)}
    >${c.label}<span class="table-sort-icon" aria-hidden="true">${iconFor(c.key)}</span></button>
  </th>
  ```
  where `ariaSortFor(key)` and `iconFor(key)` are local reactive getters:
  ```ts
  const ariaSortFor = (key: string) => () => {
    const s = sortSig?.val;
    if (!s || s.key !== key) return "none";
    return s.dir === "asc" ? "ascending" : "descending";
  };
  const iconFor = (key: string) => () => {
    const s = sortSig?.val;
    if (!s || s.key !== key) return "↕";
    return s.dir === "asc" ? "▲" : "▼";
  };
  ```
  Both return a function so the template binding stays reactive. The non-
  sortable branch passes `null` for `aria-sort` (omitted attribute) and renders
  the bare `${c.label}` exactly as today.

- Add `cycleSort`:
  ```ts
  const cycleSort = (key: string): void => {
    if (!sortSig) return;
    const next = nextSortState(sortSig.val, key);
    sortSig.set(next);
    props.onSortChange?.(next);
  };
  ```

- Derive the rows source. In client-side mode (`onSortChange === undefined`),
  use `computed` to fold sort into a sorted-copy view; in server-side mode
  (`onSortChange != null`), just reuse `props.rows`:
  ```ts
  const viewRows: Signal<T[]> =
    props.onSortChange == null && sortSig != null
      ? (computed(() => sortedRows(props.rows.val, sortSig.val, props.columns)) as unknown as Signal<T[]>)
      : props.rows;
  ```
  The `as unknown as Signal<T[]>` cast matches the existing precedent at
  `showcase/src/routes/table.ts:64`. If `sortSig` is missing (no sortable
  columns), `viewRows` falls through to `props.rows` and the table behaves
  exactly as today.

- Change the `<tbody>` template binding from
  `each(props.rows, renderRow, props.rowKey)` to
  `each(viewRows, renderRow, props.rowKey)`. Also update `emptyRow` to read
  from `viewRows.val` instead of `props.rows.val` so an empty client-side view
  still triggers the empty state correctly (it will, since sorting an empty
  array yields an empty array, but the read should be of the view).

- Keep the existing `headerCells` / `renderRow` / `overlay` shape. The
  function should still fit under ~80 lines after the refactor; if it
  pushes past, lift `headerCell` and `cycleSort` to module scope (they only
  close over `props` / `sortSig`, both passable as args).

**Changes (Table.test.ts):**

Add a `describe("Table sort", () => { ... })` block (or nested under the
existing `Table` describe) with the R7 cases. Use the same imports already in
the file. Patterns to follow:

- `sortable_column_renders_button_with_aria_sort` — render with one column
  `{ key: 'name', label: 'Name', sortable: true }` and `sort = signal(null)`;
  assert `find(el, '.table-th')` has attribute `aria-sort` equal to `"none"`
  and `find(el, '.table-th button.table-sort-btn')` is truthy.

- `click_cycles_asc_desc_clear` — same setup; `fire(find(el, '.table-sort-btn'),
  'click')` three times and assert `sort.val` after each click is
  `{key: 'name', dir: 'asc'}`, `{key: 'name', dir: 'desc'}`, then `null`.
  Between clicks, assert `aria-sort` is `"ascending"`, `"descending"`, `"none"`
  and rendered first-row text reorders per R6.

- `clicking_other_column_resets_to_asc` — two sortable columns; set
  `sort.set({key: 'a', dir: 'desc'})`; click the second column's button;
  assert `sort.val` is `{key: 'b', dir: 'asc'}` and the first column's
  `<th>` has `aria-sort="none"`.

- `default_comparator_sorts_numbers_and_strings` — two separate sub-cases
  inside the test: a numeric `score` column and a string `name` column. After
  setting `sort` to asc/desc, assert the first row in the DOM matches the
  expected smallest/largest.

- `nulls_sort_last_asc_first_desc` — rows with mixed `null` and non-null
  values on the sorted column; verify asc order ends with null, desc order
  starts with null.

- `custom_compare_overrides_default` — column with
  `compare: (a, b) => a.priority - b.priority` even though the column key is
  `name`; verify rows order follows `priority`, not `name`.

- `sortable_without_sort_signal_throws` — expect `render(Table({ columns:
  [{ key: 'name', label: 'Name', sortable: true }], rows, rowKey }))` to throw
  with a message that includes `"sort"`.

- `onSortChange_fires_with_next_state` — server-side mode with
  `const onSortChange = spy<(s: SortState | null) => void>();`. Click;
  `expect(onSortChange).toHaveBeenCalledWith({key: 'name', dir: 'asc'})`.

- `server_side_mode_does_not_reorder_rows` — server-side mode with
  out-of-order rows in `rows`. Click sort header; assert the rendered row
  order matches `rows.val`'s original order verbatim.

- `sort_signal_still_updates_in_server_side_mode` — server-side mode; click;
  assert `sort.val` reflects the click even though the DOM did not reorder.

- `non_sortable_columns_render_as_plain_th` — column without `sortable: true`;
  assert `find(<that th>, 'button')` is null and `<th>.getAttribute('aria-
  sort')` is null. Also assert *no* `.table-sort-btn` in a column-free
  default-table render (regression for backwards compatibility).

**Tests pass criteria:** `cargo run -p zero -- test Table.test.ts` runs green;
existing Table tests remain unchanged in behavior.

---

### Step 3: Style the sortable header button and indicator

**Goal:** Compose the existing button-ghost utility into the header so the
sortable header reads as "click target the whole cell" and the indicator glyph
has the right rhythm. Reuse design-system CSS per `MEMORY.md` —
no duplicated `.button` rules.

**Files:**
- `crates/zero-scaffold/src/scaffold/.zero/styles/components/_table.scss`

**Changes:**

Inside the existing `@layer components { ... }` block, append:

```scss
.table-sort-btn {
  // Fill the cell so users don't have to aim at the label.
  width: 100%;
  justify-content: flex-start;
  padding-inline: 0;
  padding-block: 0;
  background: transparent;
  border: none;
  font: inherit;
  font-weight: var(--weight-semibold);
  color: var(--color-text);
  // Hover/active treatment is inherited from .button-ghost — do not redefine.
}

.table-align-end .table-sort-btn    { justify-content: flex-end; }
.table-align-center .table-sort-btn { justify-content: center; }

.table-sort-icon {
  margin-inline-start: var(--space-xs);
  font-size: var(--font-size-sm);
  line-height: 1;
}

// Inactive (showing ↕) reads as a hint, not a status.
.table-th[aria-sort="none"] .table-sort-icon { opacity: 0.4; }
```

Notes:
- The `padding-inline: 0; padding-block: 0;` override is the only "remove"
  rule needed; everything else comes from `.button .button-ghost .button-sm`.
- The `[aria-sort="none"]` selector relies on Step 2 placing `aria-sort` on
  the `<th>`, not the `<button>`.
- No new tokens introduced. All `var(--*)` calls reference existing tokens
  used elsewhere in `_table.scss` or `_button.scss`.

**Tests:** None directly — SCSS does not have unit tests in this project. The
existing `tests/showcase_build.rs` exercises the scaffold compile path and
will catch a SCSS syntax error. Visual review happens in Step 4 when the
showcase route renders a sortable Table.

---

### Step 4: Add a sortable variant to the Table showcase route

**Goal:** Demonstrate client-side sort end-to-end in the showcase so
`tests/showcase_build.rs` and `tests/showcase_dev.rs` exercise the new API
surface. A short comment in the same route documents the server-side mode
contract — a separate "Server-side" section isn't worth a fake-fetch
re-implementation when the async-paginated section already shows the
re-fetch pattern.

**Files:**
- `showcase/src/routes/table.ts`

**Changes:**

Add a new top-level function `sortableSection()` near the other section
helpers (after `asyncSection`, before `TableRoute`). Mount it in
`TableRoute` between the `Loading` section and `paginatedSection()`.

```ts
function sortableSection(): TemplateResult {
  // Sample rows include a deliberate `null` score (Yuki) so the
  // default-comparator's null-handling behavior is visible.
  const data: User[] = sample.map((u) =>
    u.name === "Yuki Tanaka" ? { ...u, score: null as unknown as number } : u,
  );
  const rows = signal<User[]>(data);
  const sort = signal<SortState | null>(null);
  const sortableColumns: TableColumn<User>[] = [
    { key: "name", label: "Name", sortable: true },
    { key: "email", label: "Email", width: "240px" },
    { key: "role", label: "Role", sortable: true },
    { key: "score", label: "Score", align: "end", sortable: true },
  ];
  return html`
    <section class="stack gap-sm">
      <h2 class="text-h2">Sortable (client-side)</h2>
      <p class="text-small text-muted">
        Click a sortable header to cycle asc → desc → unsorted. Pass
        <code>onSortChange</code> to switch into server-side mode — Table
        emits intent and renders whatever the parent's <code>rows</code>
        signal contains, instead of sorting locally.
      </p>
      ${Table({ columns: sortableColumns, rows, rowKey, sort })}
    </section>
  `;
}
```

Imports at the top of the file add `SortState`:
```ts
import type { TableColumn, SortState } from "zero/components";
```

`TableRoute` adds `${sortableSection()}` between the existing `Loading`
section and `${paginatedSection()}`.

**Tests:**
- `cargo test -p zero-scaffold` and `cargo test --workspace` cover the
  showcase build/dev integration tests
  (`tests/showcase_build.rs`, `tests/showcase_dev.rs`,
  `tests/component_library.rs`). The new section must compile and render
  without runtime errors. No new dedicated test file is added.

---

### Step 5: Document the sort API in `docs/components.md`

**Goal:** Make the new API discoverable. The existing Table row in the
component table at `docs/components.md:167` does not enumerate every prop —
the doc relies on the source-of-truth `.d.ts`. Add a short subsection
*after* the main table that covers Table-specific sort details and link
back from the table row.

**Files:**
- `docs/components.md`

**Changes:**

1. Update the `Table` row in the component table (line 167) to surface the
   new optional props:
   ```
   | `Table`    | `columns`, `rows: Signal<T[]>`, `rowKey`; optional `density`, `loading`, `sort`, `onSortChange` | `Table({ columns, rows, rowKey: r => r.id })`                                    |
   ```

2. After the component table and before the `→ Next:` line, insert a new
   `## Table sort` subsection at the same heading depth as the existing
   "Components are functions" / "Props" sections (so it lands inside the
   page's flow, not as a sibling page). Content:
   ```markdown
   ## Table sort

   `Table` supports per-column sort. Mark a column with `sortable: true`
   and pass a `sort` signal that the parent owns:

   ```ts
   import { signal } from "zero";
   import { Table } from "zero/components";
   import type { SortState, TableColumn } from "zero/components";

   const sort = signal<SortState | null>(null);
   const columns: TableColumn<User>[] = [
     { key: "name",  label: "Name",  sortable: true },
     { key: "score", label: "Score", sortable: true, align: "end" },
   ];

   // Client-side: Table sorts a copy of `rows` itself.
   Table({ columns, rows, rowKey, sort });

   // Server-side: pass onSortChange. Table emits intent; the parent
   // re-fetches and updates `rows`. Table does not reorder locally.
   Table({ columns, rows, rowKey, sort, onSortChange: (next) => refetch(next) });
   ```

   | Mode         | `onSortChange` set | Behavior                                                                |
   |--------------|--------------------|-------------------------------------------------------------------------|
   | Client-side  | No                 | `sort.set(next)`; renders `rows` reordered via column `compare` or default. |
   | Server-side  | Yes                | `sort.set(next)` then `onSortChange(next)`; renders `rows` as the parent provides. |

   Clicking a sortable header cycles **asc → desc → unsorted** on the active
   column. Clicking a different sortable column resets to asc on the new
   column.

   The default comparator handles numbers (subtraction), strings
   (`localeCompare`), and nullish values (sorted last in asc, first in
   desc). For mixed-type columns or custom orderings, pass
   `compare: (a, b) => number` on the column.
   ```

**Tests:** None. Doc files don't ship with tests beyond Jekyll's build
(out-of-scope here). Verify by eyeballing the rendered Markdown.

---

## Risks and Assumptions

- **`each(...)` accepts a `Computed` via the existing duck-type/`Signal` cast
  pattern.** Verified by `showcase/src/routes/table.ts:64`, which already
  passes `rows: rows as unknown as Signal<User[]>` where `rows` is a
  `computed(...)`. If the runtime tightens this later, Step 2 needs to
  switch to a manually-mirrored signal driven by an `effect`.

- **The validation throw runs at render time.** A parent that conditionally
  flips a column's `sortable` after first render won't re-trigger the throw,
  but Table also won't be re-invoked — `columns` is a structural plain-value
  prop, not reactive, so the parent re-creating the column set means re-
  rendering Table, which re-runs the guard. No silent failure mode.

- **Sort glyphs are ASCII Unicode triangles/up-down arrow, not SVG.** Renders
  consistently across the design-system font stack (verified in adjacent
  components — Pagination uses `‹` / `›` / `…` similarly). If a downstream
  user wants custom icons, that's the "Sort indicator customization" item
  explicitly out of scope per the spec.

- **`sort` may legitimately reference a column that's been removed.** R6
  says Table renders unsorted in that case and clears no state. The
  `sortedRows` helper handles this by returning rows unchanged if no column
  matches. No test for this edge — the `default_comparator_sorts_numbers_and_strings`
  test already exercises the happy path, and an explicit "stale state"
  test would lock in a behavior the spec calls the parent's responsibility.

- **80-line function guideline.** After the Step 2 refactor, `Table<T>` adds
  the validation guard, `cycleSort`, the icon/aria reactive getters, and the
  `viewRows` derivation. If the function pushes past ~80 lines, the planned
  lift is `headerCell(col, sortSig, cycleSort)` to module scope. The helpers
  `nextSortState`, `defaultCompare`, and `sortedRows` are already module-
  scope so they don't count toward `Table<T>`'s length.

- **Backwards compatibility.** Every existing test in `Table.test.ts` must
  pass unchanged in Steps 1–3. If any does not (e.g., a test that asserts
  `find(el, 'th button')` is null and now matches a sortable header from
  some unintended cross-test signal), the regression points to behavior
  leaking outside the new code paths and needs to be fixed before moving
  on.
