import { html, each } from "zero";
import type { Signal, TemplateResult } from "zero";
import Spinner from "./Spinner.ts";

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

/**
 * Table — sticky-header data table over a reactive `rows` signal.
 * `columns` declares cells, `rowKey` makes row identity stable so the
 * keyed `each()` reconciler reuses DOM across reorders. Optional
 * `onRowClick`, `density`, `maxHeight`, `empty`, and `loading` slots
 * cover the 80% case without bringing in sort, selection, pagination,
 * or virtualization.
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

  const headerCells = props.columns.map((c) => {
    const cls = "table-th" + (c.align ? ` table-align-${c.align}` : "");
    const style = c.width ? `width: ${c.width}` : null;
    return html`<th class=${cls} style=${style}>${c.label}</th>`;
  });

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
    if (props.rows.val.length !== 0) return null;
    const slot = props.empty ?? html`<span class="text-muted">No data</span>`;
    return html`<tr class="table-empty"><td colspan=${props.columns.length}>${slot}</td></tr>`;
  };

  const overlay = (): TemplateResult | null => {
    if (!loadingSig || !loadingSig.val) return null;
    return html`<div class="table-loading-overlay">${Spinner({ size: "md" })}</div>`;
  };

  return html`<div class=${containerCls} style=${containerStyle}><table class=${tableCls}><thead><tr>${headerCells}</tr></thead><tbody>${each(props.rows, renderRow, props.rowKey)}${emptyRow}</tbody></table>${overlay}</div>`;
}
