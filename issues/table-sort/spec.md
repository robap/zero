# Spec: Table sortable-header affordance

## Problem Statement

`Table` (`crates/zero-scaffold/src/scaffold/.zero/components/Table.ts`) ships sticky headers, density variants, alignment, loading/empty slots, and row-click — but no sort. `TableColumn<T>` exposes only `key`, `label`, `align`, `width`, and `render` (lines 7-13). Any app that wants per-column sort drops out of Table entirely, rebuilding `<table>` / `<thead>` / `<tbody>` markup over the design-system `.table .table-cozy` classes and rolling its own sort UI.

This was deferred deliberately in the original Table spec (`issues/table/spec.md:31`): "No sort in v1. … Sticky header + scrolling body is the core; sort adds a meaningful API surface (sortable per column, sort comparator, controlled vs uncontrolled sort state, aria-sort wiring) that is worth its own scoped issue if and when an app actually needs it." The demo's friction log (`~/Documents/code/zero_demo/FRAMEWORK_NOTES.md:38`, severity 🟡) now confirms an app hit it.

The friction-log entry frames sort as "common enough to be a built-in." The demo needs both **client-side sort** (in-memory record lists) and **server-side sort** (paginated APIs where the backend reorders the page and returns it). A useful sort feature has to cover both shapes with one API.

## Background

### What Table looks like today

`Table<T>({ columns, rows, rowKey, onRowClick?, density?, maxHeight?, empty?, loading? })`:

- `columns: TableColumn<T>[]` — declarative cell descriptors, plain values (configuration, not state).
- `rows: Signal<T[]>` — reactive. `<tbody>` renders via `each(rows, renderRow, rowKey)`.
- `rowKey: (row, i) => string | number` — required for the keyed reconciler.
- Header cells (`headerCells`, lines 54-58) are plain `<th class="table-th" style=…>${c.label}</th>`. No interactive element.

The contract this spec extends:

- Stateful props are signals; structural props are plain values. Sort state must therefore be a signal.
- Components never hold internal state for what the parent might observe. `value`, `open`, `active`, `checked`, etc. all live in caller-owned signals. Sort follows the same rule.
- Per-component SCSS partial under `.zero/styles/components/_table.scss`; manifest plumbing in `crates/zero-scaffold/src/lib.rs`.
- Test neighbor: `Table.test.ts`. Showcase route: `showcase/src/routes/table/…` (verify path during planning).

### Decisions made during refine

The user confirmed each of the following:

- **Both client-side and server-side sort are required.** Client-side: Table reorders `rows` itself using `row[key]` (or a per-column `compare`). Server-side: Table emits intent and the parent re-fetches; Table does not re-order locally.
- **Always controlled.** Parent passes `sort: Signal<SortState | null>`. Table never holds private sort state. Matches every other stateful prop in the library and keeps the API one-shape for both modes.
- **Tri-state click cycle.** On the active column: asc → desc → null (clear). On a different sortable column: jump to asc on the new column. Three clicks on one header returns the table to the original order; clicking another column restarts the cycle on that column.

### Adjacent component precedents

- **`Tabs`** uses `active: Signal<string>` as the controlled-active prop. Same shape for `sort`.
- **`Pagination`** uses `onChange?: (page: number) => void` as the side-effect callback alongside the controlling signal. Same shape for `onSortChange`.
- **`Button` / `button-ghost`** styles are the right base for header buttons — composed, not duplicated. Per `MEMORY.md` "reuse design-system CSS," sortable headers should compose existing classes, not introduce a new button look.

### Two operating modes

Mode is implicit, determined by whether `onSortChange` is passed:

| Mode             | `onSortChange` set | Table's behavior                                                |
|------------------|--------------------|-----------------------------------------------------------------|
| Client-side      | No                 | Calls `sort.set(next)`. Renders `rows` reordered via the active comparator (per-column `compare` or default `row[key]`). Parent's `rows` signal is untouched. |
| Server-side      | Yes                | Calls `sort.set(next)` *then* `onSortChange(next)`. Renders `rows` in the order the parent provides — no client-side reorder. Parent typically re-fetches and updates `rows`. |

The signal is always the source of truth for "what's currently highlighted in the header." The parent updating `rows` is what changes what users see in the body.

### Adjacent surfaces touched

- **`crates/zero-scaffold/src/scaffold/.zero/components/Table.ts`** — extend `TableColumn` and `TableProps`; add header-button rendering, sort cycle logic, client-side reorder.
- **`crates/zero-scaffold/src/scaffold/.zero/styles/components/_table.scss`** — header-button styling (compose `.button .button-ghost`), sort indicator (▲/▼ glyph), aria-sort visual treatment.
- **`crates/zero-scaffold/src/scaffold/.zero/components/Table.test.ts`** — new test cases for sort.
- **`showcase/src/routes/`** — Table's showcase route gets a sortable variant; verify path.
- **`crates/zero-scaffold/src/lib.rs`** — no new files; no manifest changes.
- **`docs/components.md`** — document the new column / props fields and the two modes.

## Requirements

### R1 — `TableColumn<T>` gains a `sortable` flag and optional `compare`

`TableColumn<T>` extends:

```ts
type TableColumn<T> = {
  key: keyof T & string;
  label: string;
  align?: "start" | "end" | "center";
  width?: string;
  render?: (row: T, i: number) => TemplateResult | string | number;
  sortable?: boolean;                     // default false
  compare?: (a: T, b: T) => number;       // default: see R5
};
```

A column with `sortable !== true` renders exactly as today: plain `<th>` with the label, no button, no aria-sort, no interactivity. Sortable columns render a `<button>` inside the `<th>` (see R3).

`compare` is only consulted in client-side mode (R6). It's ignored when `onSortChange` is set.

### R2 — `TableProps<T>` gains `sort` and `onSortChange`

`TableProps<T>` extends:

```ts
type SortState = { key: string; dir: "asc" | "desc" };

type TableProps<T> = {
  // ...existing fields...
  sort?: Signal<SortState | null>;        // required when any column is sortable
  onSortChange?: (next: SortState | null) => void;
};
```

- `sort` is optional in the type but **required when any column has `sortable: true`**. Table errors at render with a clear message if any column is sortable and `sort` is missing. (Throw, not console.error — bad config should be loud during development.)
- `onSortChange` is always optional. Its presence flips Table into server-side mode (R6).

### R3 — Sortable header renders an accessible button with sort indicator

For each column with `sortable: true`, the header cell renders:

```html
<th class="table-th [align?]" aria-sort="ascending|descending|none">
  <button class="button button-ghost button-sm table-sort-btn"
          type="button"
          @click=${cycleSort(column)}>
    [label]
    <span class="table-sort-icon" aria-hidden="true">[▲|▼|◇]</span>
  </button>
</th>
```

- `aria-sort` set on the `<th>` per W3C ARIA: `"ascending"`, `"descending"`, or `"none"` (when this column isn't the active sort key).
- Button is keyboard-focusable; Enter / Space trigger click natively. No custom keydown handler.
- Sort icon is a single character glyph that swaps based on state. Recommended: `▲` for asc on the active column, `▼` for desc, `◇` (or a neutral up-down stack) for an inactive sortable column. Final glyph choice is a planner detail; the requirement is *some visible indicator that differentiates inactive / asc / desc*.
- Header styling composes existing utilities (`.button .button-ghost .button-sm`) — no bespoke `.table-sort-button` ruleset that duplicates `.button`. Reuse design-system CSS per `MEMORY.md`.
- Header click target fills the cell so users don't have to aim at the label specifically (`width: 100%`, `justify-content: flex-start` or per `c.align`).

### R4 — Click cycle and column-switch behavior

`cycleSort(column)` computes the next `SortState | null`:

| Current `sort.val`               | Click on this column → next state               |
|----------------------------------|-------------------------------------------------|
| `null`                           | `{ key: column.key, dir: "asc" }`               |
| `{ key: <other>, dir: any }`     | `{ key: column.key, dir: "asc" }`               |
| `{ key: column.key, dir: "asc" }`| `{ key: column.key, dir: "desc" }`              |
| `{ key: column.key, dir: "desc" }`| `null`                                         |

Then:
1. `sort.set(next)`.
2. `onSortChange?.(next)`.

Order matters: `sort.set` runs first so any reactive readers (including Table's own client-side reorder in R6) see the new state before the side-effect callback fires.

### R5 — Default comparator handles strings, numbers, and nullish

When client-side mode is active and a column has no `compare`, the default comparator on column with key `k` is:

- `a[k] == null && b[k] == null` → `0`.
- `a[k] == null` → `1` (nulls sort last in asc; in desc they end up first because the result is negated).
- `b[k] == null` → `-1`.
- Both `number`: `(a[k] as number) - (b[k] as number)`.
- Both `string`: `(a[k] as string).localeCompare(b[k] as string)`.
- Otherwise: `String(a[k]).localeCompare(String(b[k]))` as fallback.

Direction `"desc"` negates the result of the asc comparison. Planner is free to refine (e.g. number-stringly coerced like `"10"` < `"9"` is acceptable — users with mixed types should pass a `compare`).

### R6 — Mode selection: client-side reorders, server-side passes through

In `Table<T>(props)`:

- Compute a derived rows source. Recommended shape: a `computed` (or equivalent reactive read inside the template) that returns:
  - **Client-side mode** (`onSortChange` is `undefined`): when `sort.val` is `null`, return `rows.val` as-is. Otherwise, return a sorted *copy* (`[...rows.val].sort(activeComparator)`). Never mutate the parent's `rows.val` array.
  - **Server-side mode** (`onSortChange` is set): always return `rows.val` as-is. The parent owns ordering.
- `<tbody>` renders from the derived rows via the existing `each(...)` pattern. Row identity is preserved by `rowKey`, so reorder reuses DOM nodes.

The active comparator is `column.compare ?? defaultCompare(column.key)`; `dir === "desc"` negates the result. If `sort.val.key` doesn't match any column (stale state), Table renders rows unsorted and clears no state — the parent is responsible.

### R7 — Tests

`crates/zero-scaffold/src/scaffold/.zero/components/Table.test.ts` gains:

Client-side sort:
- `sortable_column_renders_button_with_aria_sort` — render with one sortable column, no sort active; `<th>` has `aria-sort="none"`, contains a `<button>`.
- `click_cycles_asc_desc_clear` — three clicks on the same header produce `sort.val = {asc} → {desc} → null`; rows reorder accordingly in mode 1; `aria-sort` updates each click.
- `clicking_other_column_resets_to_asc` — sort by A desc; click B; `sort.val = {key: 'b', dir: 'asc'}`; A's `aria-sort` flips to `"none"`.
- `default_comparator_sorts_numbers_and_strings` — verify both data types reorder correctly.
- `nulls_sort_last_asc_first_desc` — column with mixed null and non-null values: asc puts nulls at the end; desc puts nulls at the start.
- `custom_compare_overrides_default` — column with `compare: (a, b) => a.priority - b.priority`; verify ordering uses the comparator, not `row[key]`.
- `sortable_without_sort_signal_throws` — render with `columns[0].sortable = true` and no `props.sort`: throws (or render-time error) with a message mentioning `sort` prop.

Server-side mode:
- `onSortChange_fires_with_next_state` — register a spy on `onSortChange`; click; spy called once with `{key, dir: 'asc'}`.
- `server_side_mode_does_not_reorder_rows` — `onSortChange` set; click header; assert the rendered row order is exactly `rows.val`'s order, even though the column has values that would sort differently client-side.
- `sort_signal_still_updates_in_server_side_mode` — `sort.val` reflects the click even though Table doesn't reorder.

Non-sortable behavior unchanged:
- `non_sortable_columns_render_as_plain_th` — existing test pattern; assert no `<button>` inside `<th>` for columns without `sortable: true`, no `aria-sort` attribute.

### R8 — Showcase

The Table showcase route gains a sortable variant demonstrating client-side mode (numbers + strings + nulls). If a separate showcase route makes sense for server-side mode, the planner adds it; otherwise a comment in the existing route's source documenting the two-mode contract is enough. The showcase exists primarily to keep `tests/showcase_build.rs` and `tests/showcase_dev.rs` honest about new APIs — both routes must compile and render without error.

### R9 — Docs

`docs/components.md` — Table section gains:

- The new column fields (`sortable`, `compare`) and props (`sort`, `onSortChange`) with one-line descriptions.
- The two-mode table (`client-side` vs `server-side`) — same shape as the table in this spec's Background.
- The click cycle (asc → desc → null; column-switch resets).
- A one-line example for each mode:

```ts
// Client-side
const sort = signal<SortState | null>(null);
Table({ columns: [{ key: "name", label: "Name", sortable: true }], rows, rowKey, sort });

// Server-side
Table({ columns: [...], rows, rowKey, sort, onSortChange: (next) => refetch(next) });
```

- A note on the default comparator's behavior with strings/numbers/nulls and when to pass a custom `compare`.

## Constraints

- No npm dependencies; same workspace dependencies as the rest of `zero`.
- The 80-line per-function guideline (CLAUDE.md) applies. The current `Table<T>` function is already long; the sort additions almost certainly require factoring `headerCell`, `cycleSort`, and the derived-rows computation into helpers.
- Backwards compatibility: every existing `Table({ columns, rows, rowKey, ... })` call site without `sortable` columns must continue to render identically. No required prop is added (sort is conditionally required only if a column is sortable; the type stays optional).
- Client-side mode never mutates `rows.val`. Always sort a copy.
- The default comparator must not throw on any input shape — it falls back to `String()` coercion for unknown types. Render errors are reserved for actual config bugs (missing `sort` when sortable columns exist).
- Sort icon and `aria-sort` must stay in sync; the planner picks the implementation (a single derived expression covering both, ideally).
- CSS additions live in `_table.scss` and compose existing utilities. No new base button class.

## Out of Scope

- **Multi-column sort** (shift-click for secondary key). Single-column only in v1.
- **Sort indicator customization** (custom glyphs per column, icon library). Single glyph, design-system-styled.
- **Drag-to-reorder columns.** Separate feature.
- **Selection** (checkbox column, `selected` signal). Already deferred by `issues/table/spec.md`.
- **Virtualization, filtering, resizable columns.** Same.
- **Persisting sort across reloads** (URL query sync, localStorage). Parent's job if needed.
- **Sort presets / initial sort.** Just pass `signal({key, dir})` instead of `signal(null)` — no new API surface needed.
- **Per-column "sortable but with no client-side fallback" mode.** If a column is sortable and `onSortChange` is unset, default comparator handles it. Users who want "sort only via the parent's hook" can branch in their own `compare` or just always pass `onSortChange`.

## Open Questions

- **Sort icon glyph.** Spec recommends `▲` / `▼` / neutral indicator (e.g. `◇` or a stacked up-down). Planner picks the exact characters and any SCSS treatment (color, opacity for inactive). Final UI choice — not a contract decision.
- **`SortState` type export.** Where does the `SortState = { key: string; dir: "asc" | "desc" }` type live? Recommended: export from `Table.ts` as a named type so callers can declare `signal<SortState | null>(null)` without redefining the shape. Alternative: export from `_internal.ts` (the helper module introduced by `issues/pagination-computed/`). Spec recommends `Table.ts` since it's Table-specific; planner confirms.
- **Render-time vs construct-time validation.** R2 says Table errors if a column is sortable and `sort` is missing. Should that be a `throw` at the top of the component function (runs every render — duplicate noise), or a one-time check via a guard the planner figures out? Spec defaults to throwing at the top of `Table<T>` because the function is the only entry point and the check is cheap; planner refines if needed.
- **Order of `sort.set(...)` and `onSortChange(...)`.** R4 says `set` first. If a parent's `onSortChange` reads `sort.val` to decide what to do, they'd see the updated value — this is intentional and matches `Pagination`'s `props.page.set(target); props.onChange?.(target)` shape (`Pagination.ts:135-136`). Planner confirms by analogy.
- **Empty-state interaction with sort.** If `rows.val` is empty in client-side mode after the user clicks a sort header, does anything change? Recommended: no — the empty row renders as today (`emptyRow` in `Table.ts:70-74`). Sort state and empty state are independent. Planner confirms during implementation.
- **Whether to fold `sort` into a `Reactive<SortState | null>` per `issues/pagination-computed/`.** Since the spec requires the parent to own sort state and update it (server-side mode), the parent needs `.set` — `Computed<SortState | null>` would never work. So `sort` stays `Signal<...>` specifically, not `Reactive<...>`. No cross-spec coupling; noting here so the planner doesn't try to generalize.
