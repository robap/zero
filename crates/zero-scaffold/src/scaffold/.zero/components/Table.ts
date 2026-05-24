import { html, each, computed } from "zero";
import type { Signal, TemplateResult } from "zero";
import Spinner from "./Spinner.ts";

export type TableDensity = "compact" | "cozy";

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

/**
 * Compute the next sort state for a click on `columnKey`. Cycle:
 * null → asc → desc → null on the active column; jump-to-asc on any
 * other column.
 *
 * @param current The current sort state, or null when nothing is sorted.
 * @param columnKey The key of the clicked column.
 * @returns
 * @internal
 */
function nextSortState(
  current: SortState | null,
  columnKey: string,
): SortState | null {
  if (current === null || current.key !== columnKey) {
    return { key: columnKey, dir: "asc" };
  }
  if (current.dir === "asc") return { key: columnKey, dir: "desc" };
  return null;
}

/**
 * Default comparator for a column with key `k`. Nullish values sort
 * last in ascending order (and therefore first in descending). Numbers
 * compare via subtraction, strings via `localeCompare`, mixed types
 * fall back to `String()`-coerced `localeCompare`.
 *
 * @template T
 * @param k The column key whose values will be compared.
 * @returns
 * @internal
 */
function defaultCompare<T>(k: keyof T & string): (a: T, b: T) => number {
  return (a, b) => {
    const av = a[k] as unknown;
    const bv = b[k] as unknown;
    const aNull = av === null || av === undefined;
    const bNull = bv === null || bv === undefined;
    if (aNull && bNull) return 0;
    if (aNull) return 1;
    if (bNull) return -1;
    if (typeof av === "number" && typeof bv === "number") return av - bv;
    if (typeof av === "string" && typeof bv === "string") return av.localeCompare(bv);
    return String(av).localeCompare(String(bv));
  };
}

/**
 * Return the visible row order for `rows` given a sort state. When
 * `state` is null, returns the same array reference (no copy) so
 * keyed reconciliation stays cheap. When `state.key` does not match
 * any column, the rows are returned unchanged.
 *
 * @template T
 * @param rows The unsorted row array.
 * @param state The active sort state or null.
 * @param columns The column descriptors, used to look up a per-column comparator.
 * @returns
 * @internal
 */
function sortedRows<T>(
  rows: T[],
  state: SortState | null,
  columns: TableColumn<T>[],
): T[] {
  if (state === null) return rows;
  const col = columns.find((c) => c.key === state.key);
  if (!col) return rows;
  const cmp = col.compare ?? defaultCompare<T>(col.key);
  const mul = state.dir === "desc" ? -1 : 1;
  return [...rows].sort((a, b) => mul * cmp(a, b));
}

/**
 * Render a single `<th>` cell. The sortable branch wires a button,
 * reactive `aria-sort`, and a reactive glyph; the non-sortable branch
 * returns a plain label. Lifted to module scope so the `Table<T>`
 * body stays under the per-function line guideline.
 *
 * @template T
 * @param c The column descriptor for this header.
 * @param sortSig The parent's sort signal, or undefined when no column is sortable.
 * @param cycleSort Click handler that advances the sort cycle for a column key.
 * @returns
 * @internal
 */
function renderHeaderCell<T>(
  c: TableColumn<T>,
  sortSig: Signal<SortState | null> | undefined,
  cycleSort: (key: string) => void,
): TemplateResult {
  const cls = "table-th" + (c.align ? ` table-align-${c.align}` : "");
  const style = c.width ? `width: ${c.width}` : null;
  if (c.sortable !== true) {
    return html`<th class=${cls} style=${style}>${c.label}</th>`;
  }
  const ariaSort = (): string => {
    const s = sortSig?.val;
    if (!s || s.key !== c.key) return "none";
    return s.dir === "asc" ? "ascending" : "descending";
  };
  const icon = (): string => {
    const s = sortSig?.val;
    if (!s || s.key !== c.key) return "↕";
    return s.dir === "asc" ? "▲" : "▼";
  };
  return html`<th class=${cls} style=${style} aria-sort=${ariaSort}><button type="button" class="button button-ghost button-sm table-sort-btn" @click=${() => cycleSort(c.key)}>${c.label}<span class="table-sort-icon" aria-hidden="true">${icon}</span></button></th>`;
}

/**
 * Table — sticky-header data table over a reactive `rows` signal.
 * `columns` declares cells, `rowKey` makes row identity stable so the
 * keyed `each()` reconciler reuses DOM across reorders. Optional
 * `onRowClick`, `density`, `maxHeight`, `empty`, and `loading` slots
 * cover the 80% case. Mark a column with `sortable: true` and pass
 * `sort: Signal<SortState | null>` to enable per-column sort; pass
 * `onSortChange` to opt into server-side mode where the parent owns
 * the row order.
 *
 * @template T Row record type, inferred from `props.rows`.
 * @param props
 * @returns
 */
export default function Table<T>(props: TableProps<T>): TemplateResult {
  const density: TableDensity = props.density ?? "cozy";
  const clickable = typeof props.onRowClick === "function";
  const hasFixedWidths = props.columns.some((c) => c.width != null);
  const baseCls = ["table", `table-${density}`]
    .concat(clickable ? ["table-clickable"] : [])
    .join(" ");
  const loadingSig = props.loading;
  const containerCls: string | (() => string) = loadingSig
    ? () => baseCls + (loadingSig.val ? " table-loading" : "")
    : baseCls;
  const containerStyle: string | null = props.maxHeight
    ? `max-height: ${props.maxHeight}; overflow-y: auto`
    : null;
  const tableCls = hasFixedWidths ? "table-fixed" : "";

  const anySortable = props.columns.some((c) => c.sortable === true);
  if (anySortable && props.sort == null) {
    throw new Error(
      "Table: at least one column has sortable: true but no sort prop was passed. " +
        "Pass sort: Signal<SortState | null> from the parent.",
    );
  }
  const sortSig = props.sort;
  const cycleSort = (key: string): void => {
    if (!sortSig) return;
    const next = nextSortState(sortSig.val, key);
    sortSig.set(next);
    props.onSortChange?.(next);
  };
  const headerCells = props.columns.map((c) =>
    renderHeaderCell(c, sortSig, cycleSort),
  );

  const viewRows: Signal<T[]> =
    props.onSortChange == null && sortSig != null
      ? (computed(() =>
          sortedRows(props.rows.val, sortSig.val, props.columns),
        ) as unknown as Signal<T[]>)
      : props.rows;

  const renderRow = (row: T, i: number): TemplateResult => {
    const onClick = clickable ? () => props.onRowClick!(row, i) : null;
    const cells = props.columns.map((c) => {
      const cls = "table-td" + (c.align ? ` table-align-${c.align}` : "");
      const content = c.render ? c.render(row, i) : (row[c.key] as unknown as string | number);
      return html`<td class=${cls}>${content}</td>`;
    });
    return html`<tr class="table-row" data-row-index=${i} @click=${onClick}>${cells}</tr>`;
  };

  const emptyRow = (): TemplateResult | null => {
    if (viewRows.val.length !== 0) return null;
    const slot = props.empty ?? html`<span class="text-muted">No data</span>`;
    return html`<tr class="table-empty"><td colspan=${props.columns.length}>${slot}</td></tr>`;
  };

  const overlay = (): TemplateResult | null => {
    if (!loadingSig || !loadingSig.val) return null;
    return html`<div class="table-loading-overlay">${Spinner({ size: "md" })}</div>`;
  };

  return html`<div class=${containerCls} style=${containerStyle}><table class=${tableCls}><thead><tr>${headerCells}</tr></thead><tbody>${each(viewRows, renderRow, props.rowKey)}${emptyRow}</tbody></table>${overlay}</div>`;
}
