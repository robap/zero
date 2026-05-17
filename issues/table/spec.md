# Spec: Table component

## Problem Statement

The Phase 9 component library shipped fourteen components — form inputs, display surfaces, overlays, feedback — but no `Table`. The original components/spec.md deferred "table" alongside date pickers, popovers, and accordions. Since then, every real zero app that needs to show a list of records has had to author the `<table>`/`<thead>`/`<tbody>` markup, the sticky-header CSS, the scroll container, and the empty-state handling from scratch. Each reinvention drifts off the design-system token palette and re-litigates the same layout decisions.

This issue adds `Table` to the shipped component library. Scope is deliberately tight: a sticky-header, scrollable-body table that consumes `columns` + a reactive `rows` signal, with per-column alignment, density, and an optional row-click handler. **Sort, selection, pagination, filtering, resizable columns, and virtualization are explicitly deferred.** The goal is the smallest component that makes the 80% "render this list of records" case obvious — not a data grid.

## Background

### What exists today

- **Component library (Phase 9).** Fourteen components ship under `.zero/components/`, re-exported from `.zero/components/index.ts`, importable via the bare specifier `"zero/components"`. Each component:
  - Is a plain function: `Component<P> = (props?: P) => TemplateResult`.
  - Reads stateful props as signals; never holds internal state for `value`/`open`/`active`/`checked`/etc.
  - Has a per-component SCSS partial under `.zero/styles/components/_{name}.scss`, every rule inside `@layer components { ... }`.
  - Has a `*.test.ts` neighbor that ships in the manifest and runs with `zero test`.
- **Scaffold / manifest plumbing.** `framework_manifest()` in `crates/zero-scaffold/src/lib.rs` lists every framework-owned file under `.zero/`. Each entry is a `TPL_*` constant pointing at `include_str!("scaffold/.zero/...")`. The `_components.scss` aggregate `@use`s every per-component partial in alphabetical order; `zero.scss` `@use`s `'components'` last.
- **Module resolution.** The dev-server transpile pipeline and the production bundler both resolve `"zero/components"` to `.zero/components/index.ts`. Adding a new component is one line in that index file plus the file itself — no resolver change.
- **Design-system tokens (Phases 7–8).** All values components are allowed to consume are `var(--*)` custom properties on `:root`. Spacing: `--space-{xs,sm,md,lg,xl}`. Colors: `--color-{bg,surface,text,text-muted,border,border-strong,primary,...}`. Radii: `--radius-{sm,md,lg}`. Font sizes/weights/leadings, shadows, border widths — all present. Dark theme overrides only the semantic `--color-*` set.
- **Showcase project.** `showcase/` is a full zero project at the repo root; every component has a route under `showcase/src/routes/`. Built by `zero build` in CI via `tests/showcase_build.rs`; served by `zero dev` in CI via `tests/showcase_dev.rs`. Component tests run via `tests/component_library.rs`. Adding a component means adding one route file and updating the home route's navigation cluster.
- **Reactivity primitives.** `signal`, `computed`, `effect`. `each(signalOfArray, renderFn, keyFn)` is the canonical pattern for keyed list rendering and is what Table's `<tbody>` will use.
- **Existing `Tabs` precedent.** `Tabs` accepts `tabs: TabsTab[]` + `panels: Record<string, TemplateResult>` + `active: Signal<string>`. Same declarative shape Table will follow: structural props plain, stateful props as signals.

### Decisions made during refine

The user confirmed each of the following:

- **Data interface: declarative `columns` + reactive `rows`.** `Table({ columns, rows, rowKey })`. Component owns the `<thead>`/`<tbody>` markup. Cells render via per-column `render?: (row) => TemplateResult | string` or default to `row[key]`. Matches the declarative shape used by `Select` and `Tabs`.
- **`rows: Signal<T[]>`.** Reactive. Body re-renders via `each(rows, row => ..., rowKey)` when the underlying array changes. Static `T[]` rejected — Table data is almost always loaded or filtered, and forcing a parent reactive-block wrapper for every Table use is ceremony for no payoff.
- **`rowKey: (row, i) => string | number` is required.** Forces stable keys for `each(...)`. Eliminates the silent-bug class where reordered rows reuse the wrong DOM.
- **No sort in v1.** The user's original message hedged ("Maybe sort ability"). Deferred to a future issue. Sticky header + scrolling body is the core; sort adds a meaningful API surface (sortable per column, sort comparator, controlled vs uncontrolled sort state, aria-sort wiring) that is worth its own scoped issue if and when an app actually needs it.
- **Row interaction: `onRowClick?: (row, i) => void` only.** Optional. When set, rows get `cursor: pointer` and a hover treatment. No selection (checkbox column, `selected` signal, single-vs-multi mode) in v1.
- **Display features in scope (all four):** empty state slot, loading state slot, per-column alignment, density variant.
- **Scroll container height: prop wins, fallback to fills-parent.** `maxHeight?: string` caps the scroll container with `overflow-y: auto`. If unset, the Table's outer scroll container is `height: 100%` and the parent constrains it (flex/grid). Sticky header works in both modes.
- **Column widths: per-column `width?: string`, table-layout: fixed when any set.** Each column may declare `width: '120px' | '20%' | '1fr'`. If any column sets `width`, the table flips to `table-layout: fixed`. Otherwise `table-layout: auto`. Predictable; opt-in.

### Component contract

Same conventions as the existing roster:

- Variants and sizes (where applicable) are string-typed string-union props. Table has no `variant`; `density` plays the size-like role.
- Boolean state that the parent observes is a signal. `loading?: Signal<boolean>` follows this rule. `density`, `maxHeight`, `align`, and `width` are plain values — they are configuration, not observable state.
- Children rendered into the table body are produced by the component itself from `rows` + `columns`. There is no `children` prop.
- `empty?` is a `TemplateResult` slot, not a string. Users pass `html\`…\`` for full formatting control.
- Event callbacks are plain functions: `onRowClick: (row, i) => void`.

### Props sketch

```ts
type TableColumn<T> = {
  key: keyof T & string;                              // also used as React-like key for the cell
  label: string;                                      // <th> text
  align?: "start" | "end" | "center";                 // applies to <th> and all <td> in the column
  width?: string;                                     // any CSS length; presence flips table-layout: fixed
  render?: (row: T, i: number) => TemplateResult | string | number;
                                                      // default: row[key]
};

type TableProps<T> = {
  columns: TableColumn<T>[];
  rows: Signal<T[]>;
  rowKey: (row: T, i: number) => string | number;    // required

  onRowClick?: (row: T, i: number) => void;
  density?: "compact" | "cozy";                       // default: "cozy"
  maxHeight?: string;                                 // any CSS length; presence enables internal scroll

  empty?: TemplateResult;                             // shown when rows.val.length === 0
  loading?: Signal<boolean>;                          // when true, overlay spinner + dim rows
};
```

The exact TypeScript generic threading (does `Table` infer `T` from `columns`, from `rows`, or both?) is for the plan phase. Practical answer is likely "infer from `rows`, narrow `columns[].key` to `keyof T & string`."

### DOM shape

Outer scroll container, then a `<table>`:

```
<div class="table table-{density} [table-clickable] [table-loading]"
     style="max-height: {maxHeight}">         <!-- or omitted -->
  <table>
    <thead>
      <tr>
        <th class="table-th [table-align-{align}]" style="width: {width}">...</th>
        ...
      </tr>
    </thead>
    <tbody>
      <!-- each(rows, ...) -->
      <tr class="table-row" @click=...>
        <td class="table-td [table-align-{align}]">...</td>
        ...
      </tr>
      <!-- empty state row -->
      <tr class="table-empty"><td colspan="N">{empty ?? "No data"}</td></tr>
    </tbody>
  </table>
  <!-- loading overlay (conditionally) -->
  <div class="table-loading-overlay"><Spinner /></div>
</div>
```

Sticky header is `thead th { position: sticky; top: 0; background: var(--color-surface); }` — works inside any scroll container (the Table's own when `maxHeight` is set, or the parent's otherwise).

## Requirements

### Component

1. New file `.zero/components/Table.ts` exports a single default `Table` function matching `Component<TableProps<T>>`. The component runs once per mount; `rows` reactivity is delivered via `each(props.rows, ..., props.rowKey)` inside the `<tbody>`.

2. `<thead>` renders once at mount from `props.columns`. Each `<th>` carries `class="table-th"` plus an optional `table-align-{align}` and an inline `style="width: {width}"` when the column sets either.

3. `<tbody>` renders via `each(props.rows, row => <tr>...cells...</tr>, props.rowKey)`. Each cell calls `column.render(row, i)` if set, otherwise renders `row[column.key]`. Cells inherit the column's alignment class.

4. When `props.onRowClick` is set, each `<tr>` gets `@click=${() => props.onRowClick(row, i)}` and the outer container gains the class `table-clickable` (drives `cursor: pointer` + hover treatment in CSS).

5. When `props.rows.val.length === 0`, the body renders a single full-width row containing `props.empty` if provided, otherwise the literal text "No data" wrapped in a muted span. This row is conditionally rendered inside a reactive block so the empty state appears and disappears as the row count crosses zero.

6. When `props.loading` is provided and `loading.val === true`, the outer container gains the class `table-loading` (drives row dimming) and a `<div class="table-loading-overlay">` containing a `Spinner` from `"zero/components"` is rendered inside the container. The overlay is conditionally rendered via a reactive block; it does not exist in the DOM when `loading.val === false`.

7. The component imports `Spinner` from `./Spinner.ts` (relative — components do not import from `"zero/components"` to avoid the index loop).

8. `density` defaults to `"cozy"`. The outer container always carries `table-{density}` so users can target both cases without conditional classes.

9. `maxHeight` is applied via inline `style="max-height: {maxHeight}; overflow-y: auto"` on the outer container when set. When unset, the outer container has no `max-height` and the CSS layer applies `height: 100%; overflow-y: auto` as the fallback.

10. When **any** column in `props.columns` sets `width`, the `<table>` element gains `class="table-fixed"` (drives `table-layout: fixed`). When no column sets `width`, the `<table>` has no fixed-layout class and falls back to `table-layout: auto`.

11. The component is typed generically: `Table<T>(props: TableProps<T>): TemplateResult`. The plan finalizes whether `T` is inferred from `rows`, from `columns`, or both, and whether `TableColumn<T>['key']` is narrowed to `keyof T & string`.

### CSS

12. New partial `.zero/styles/components/_table.scss`. Every rule sits inside `@layer components { ... }`.

13. Token-only values. Spacing from `--space-*`, radii from `--radius-*`, colors from `--color-*`, fonts from `--font-*`, borders from `--border-*`, shadows from `--shadow-*`. No hex codes, no magic numbers except for `opacity`, `transition-duration`, `z-index`, and animation timings.

14. Sticky header: `thead th { position: sticky; top: 0; background: var(--color-surface); z-index: 1; }`. The `background` is required to mask scrolling body rows.

15. The outer `.table` is the scroll container. Default: `height: 100%; overflow-y: auto;`. When inline `max-height` is set on the element, `overflow-y: auto` is also set inline (Requirement 9), and `height: 100%` does not interfere.

16. Density classes:
    - `.table-cozy .table-td, .table-cozy .table-th { padding: var(--space-sm) var(--space-md); }`
    - `.table-compact .table-td, .table-compact .table-th { padding: var(--space-xs) var(--space-sm); }`

17. Alignment classes:
    - `.table-align-start  { text-align: start;  }`
    - `.table-align-center { text-align: center; }`
    - `.table-align-end    { text-align: end;    }`
    Logical properties only — no `left`/`right`.

18. Row hover: when `.table-clickable .table-row:hover`, swap to a muted surface (`--color-surface` variant or a dedicated hover token — the plan picks). `cursor: pointer` only on `.table-clickable .table-row`.

19. Loading overlay: `.table-loading-overlay` is `position: absolute; inset: 0; display: grid; place-items: center;`. Its parent (`.table`) gets `position: relative` when the overlay is rendered. `.table-loading .table-row { opacity: 0.5; pointer-events: none; }`.

20. Empty-state row: `.table-empty td { text-align: center; color: var(--color-text-muted); padding: var(--space-lg); }`.

21. Fixed layout: `.table-fixed table { table-layout: fixed; }`. The plan confirms the class lives on the `<table>` or the outer `.table` (Requirement 10 puts it on `<table>`).

22. No `!important`. Override via the unlayered cascade per the existing `@layer components` convention.

### Scaffold registration

23. New `TPL_*` constants for:
    - `.zero/components/Table.ts`
    - `.zero/components/Table.test.ts`
    - `.zero/styles/components/_table.scss`

24. `framework_manifest()` gains three entries. Existing length-coupled assertions in `crates/zero-scaffold/src/lib.rs` tests and any `tests/update*.rs` paths that hard-code the manifest length are bumped accordingly. The plan enumerates the exact assertions.

25. `.zero/components/index.ts` gains one line: `export { default as Table } from "./Table.ts"`. Ordering follows the file's existing alphabetical-by-component convention.

26. `.zero/styles/_components.scss` gains one line: `@use 'components/table';` inserted in alphabetical position (after `_tabs.scss`'s entry).

27. `.zero/components.d.ts` (the module declaration for `"zero/components"`) gains a `Table` export entry. Exact shape (including how the generic prop type is declared) is for the plan.

28. The editor `tsconfig.json` emitted by `zero init` requires no changes — `"zero/components"` already resolves to `.zero/components/index.ts`, and Table rides on that resolution.

### Tests

29. New file `.zero/components/Table.test.ts` exercising:
    - **Renders the base markup.** With a minimal `columns` + `rows` signal, asserts `find(el, ".table")` and `find(el, "table")` succeed; `findAll(el, ".table-th").length === columns.length`; `findAll(el, ".table-row").length === rows.val.length`.
    - **Default cell content.** A column without `render` shows `row[key]`.
    - **Custom `render`.** A column with `render: r => html\`<b>${r.name}</b>\`` produces a `<b>` inside the corresponding `<td>`.
    - **Row reactivity.** Updating the `rows` signal (`.set(newArray)`) updates the rendered row count.
    - **Empty state.** When `rows.set([])`, a `.table-empty` row appears and the previous `.table-row`s are gone.
    - **`onRowClick`.** Pass a `spy()`. Fire a click on the first `.table-row`. Assert `spy` was called once with `(rows.val[0], 0)`. Also assert `.table` carries `table-clickable`.
    - **Loading.** Pass `loading: signal(false)`. Assert no `.table-loading-overlay` is in the DOM. Flip to `true`. Assert the overlay appears and `.table` carries `table-loading`.
    - **Alignment class.** A column with `align: "end"` produces `.table-align-end` on both its `<th>` and its `<td>`s.
    - **Fixed layout.** A column with `width: "120px"` produces `class="table-fixed"` on the `<table>` and an inline `style="width: 120px"` on the corresponding `<th>`.
    - **Density default + override.** Default container carries `.table-cozy`; passing `density: "compact"` carries `.table-compact`.
    - `afterEach(cleanup)`.

30. The new test ships in the manifest (lives in `.zero/components/`) and runs:
    - Inside the framework's `showcase/` via `tests/component_library.rs`.
    - Inside every user project's `zero test` (consistent with the existing component-test ship-along policy from Phase 9).

### Showcase

31. New route file `showcase/src/routes/table.ts` rendering a representative Table:
    - 6–10 sample rows of plausible data (e.g. users with `name`, `email`, `role`, `createdAt`).
    - One column with `render` showing custom markup (e.g. a `Badge` for `role`).
    - One column with `align: "end"` (e.g. a numeric column).
    - One column with `width: "200px"` (proves fixed-layout).
    - A second Table instance demonstrating the empty state.
    - A third Table instance with a `loading` signal toggled by a button, demonstrating the overlay.
    - `maxHeight: "320px"` on the primary instance to demonstrate sticky-header scroll behavior.
    - `onRowClick` wired to a no-op or a console-style log displayed below the table.

32. `showcase/src/app.ts` registers `app.route("/table", () => import("./routes/table"))`. Position consistent with existing alphabetical ordering of routes.

33. `showcase/src/routes/home.ts` navigation cluster gains a `Table` link.

34. The showcase's committed `.zero/components/Table.ts` (and partial + test) matches the manifest. `zero update --yes` from inside `showcase/` produces zero drift.

### Integration tests

35. `tests/showcase_build.rs` continues to pass against the new route (no test changes expected — the existing test asserts the build succeeds and renders the home route; new routes are picked up automatically). The plan verifies whether any per-route assertion exists that needs widening.

36. `tests/showcase_dev.rs` continues to pass. Same caveat.

37. `tests/component_library.rs` continues to pass and now includes `Table.test.ts` in its run. The plan verifies whether the test asserts a specific test count that needs bumping.

### Documentation

38. `crates/zero-scaffold/src/scaffold/AGENTS.md` `## Components` section gains a `Table` entry in the component-roster table. The "Display" category subsection (or a new "Data" subsection if the plan prefers) gains a one-instance usage example for Table.

39. `zero-framework-spec.md` §11 — `"zero/components"` listing gains a `Table({...})` line under a `// Data` group heading (or appended to the existing `// Display` group; plan picks).

40. `zero-framework-spec.md` §12 — Phase 9 line that enumerates the 14 shipped components is updated to 15, and the parenthetical list adds `Table`. (Optional: a new Phase-9-extension bullet for Table; plan picks whether to graft onto Phase 9 or create a new entry.)

41. `BEST_PRACTICES.md` gains a short subsection on Table usage if and only if there's a non-obvious idiom worth pinning. The plan judges. A throwaway example does not justify a section.

## Constraints

- **No new Rust dependencies.** Rides on the existing `grass` SCSS pipeline, the existing transpiler, the existing scaffold + manifest plumbing.
- **No new npm dependencies.** Framework-wide.
- **No new top-level `"zero"` runtime exports.** Table is exposed only via `"zero/components"`.
- **`@layer components` for all CSS rules.** Unlayered user CSS in `styles/app.scss` overrides without `!important`.
- **Tokens only — no magic numbers or hex codes.** Standard exceptions for `opacity`, `transition-duration`, `z-index`, animation timings.
- **Stateful props are signals.** `rows: Signal<T[]>` and `loading: Signal<boolean>`. Configuration props (`columns`, `rowKey`, `onRowClick`, `density`, `maxHeight`, `empty`) are plain values.
- **`rowKey` is required.** No silent fallback to index. The function signature signals that callers must think about identity.
- **No internal state for `rows`.** The component never holds a sorted/filtered copy. It renders `props.rows.val` as-is. (This is what makes sort a separate issue — sort would live in a derived signal owned by the parent, or in a future controlled-mode addition to the Table API.)
- **No web components, no scoped styles, no CSS-in-JS.** Framework-wide.
- **No external import cycle.** `Table.ts` imports `Spinner` relatively (`./Spinner.ts`), not via `"zero/components"`.
- **One styled form.** No headless variant, no unstyled "primitive" Table. Users needing a different look fork into `src/components/`.
- **Framework-owned.** Table lives under `.zero/`. `zero update` refreshes it.

## Out of Scope

- **Sort.** No `sortable: true` per column, no internal sort state, no aria-sort wiring. Deferred to a future issue. Parents who need sort do it externally via a derived signal (`computed(() => [...rows.val].sort(...))`) and pass that to `rows`.
- **Selection.** No checkbox column, no `selected: Signal<Set<K>>`, no single/multi-select mode, no row-shift-click range selection. Deferred.
- **Pagination.** No `pageSize`, no page controls, no `currentPage`. Parents slice `rows.val` themselves.
- **Filtering / search.** Parent's job. A derived signal feeds `rows`.
- **Resizable columns.** No drag handles, no `resizable: true` per column.
- **Reorderable columns.** No drag-and-drop, no column ordering controls.
- **Virtualization / windowed rendering.** Every row renders. For multi-thousand-row tables, parents either paginate or fork.
- **Server-side anything.** No `onSortChange`, no `onPageChange`, no `loading` skeleton-row mode. The `loading` prop just dims + overlays the spinner.
- **Frozen / pinned columns.** No horizontal sticky columns. Sticky header only.
- **Expandable rows / nested rows.** No row-expand toggle, no detail-row API.
- **Footer / totals row.** No `footer?` prop. Users wrap the Table in their own markup if they need a totals row underneath.
- **Caption / `<caption>` element.** No `title?` prop. Users wrap in a Card or `<section>` with their own heading.
- **Striped rows.** No `striped?: boolean`. If a project wants stripes they override via the unlayered cascade.
- **Snapshot tests.** `expect().toMatchSnapshot()` is not implemented in `zero/test`. Tests assert on DOM selectors and signal values.
- **A standalone Table package.** No npm publication.

## Open Questions

- **Generic threading for `T`.** Inferred from `rows`, from `columns`, or both? `TableColumn<T>['key']` narrowed to `keyof T & string` or kept as plain `string`? The plan finalizes — likely "infer from `rows`, narrow `key` to `keyof T & string`."
- **Hover background.** Use an existing token (`--color-surface` against `--color-bg`?) or introduce a new `--color-surface-hover` token? Introducing a token has scope implications; reusing has contrast risks. The plan checks the current palette.
- **Sticky-header `z-index`.** Pick a literal (`1`) or thread through a token (`--z-sticky`?). Existing components currently use literals for `z-index`; matching that is the cheap path.
- **Empty-state default copy.** Literal "No data" vs ".table-empty contains no built-in copy and requires `empty?`". The spec proposes a default. The plan judges whether the default earns its keep or should be required.
- **Loading: row dim or overlay-only?** The spec proposes both. The plan may decide overlay-alone is enough.
- **`table-fixed` class location.** Spec says on the `<table>` (Requirement 10). Plan confirms — could also live on the outer `.table` with a child selector (`.table-fixed table { table-layout: fixed; }` already in Requirement 21). Pick one and align Requirements 10 + 21.
- **Per-column SCSS rule budget.** Existing partials run 20–60 lines. Table is the most complex display component; a 60–80 line ceiling is plausible. The plan judges from the actual rule count.
- **Manifest size assertion.** Current manifest size after Phase 9 changes is around 50–55 entries. Adding three pushes it by three. The plan confirms the exact current number and bumps the test.
- **Showcase route — single instance with toggles, or multiple instances?** Spec proposes three instances (main, empty, loading). One instance with feature toggles is denser but less skim-friendly. The plan judges.
- **Whether the body row `<tr>` carries the `row-{i}` index as a `data-` attribute.** Useful for tests; trivial cost. The plan picks.
- **`TableProps` and `TableColumn` exports from `index.ts`.** Spec leaves this to the plan (consistent with the original components/spec.md's open question for the existing roster).
