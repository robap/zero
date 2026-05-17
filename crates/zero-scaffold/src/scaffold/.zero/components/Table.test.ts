import { describe, it, expect, afterEach } from "zero/test";
import { render, find, findAll, fire, cleanup, text, spy } from "zero/test";
import { signal, html } from "zero";
import Table from "./Table.ts";
import type { TableColumn } from "./Table.ts";

type User = { id: number; name: string; role: string; score: number };

const sample: User[] = [
  { id: 1, name: "Ada", role: "admin", score: 92 },
  { id: 2, name: "Lin", role: "user", score: 78 },
  { id: 3, name: "Marcus", role: "user", score: 64 },
];

const rowKey = (r: User): number => r.id;

describe("Table", () => {
  afterEach(cleanup);

  it("renders base markup with one .table-th per column and one .table-row per row", () => {
    const rows = signal<User[]>(sample);
    const columns: TableColumn<User>[] = [
      { key: "name", label: "Name" },
      { key: "role", label: "Role" },
    ];
    const el = render(Table({ columns, rows, rowKey }));
    expect(find(el, ".table")).toBeTruthy();
    expect(find(el, "table")).toBeTruthy();
    expect(findAll(el, ".table-th").length).toBe(2);
    expect(findAll(el, ".table-row").length).toBe(3);
    expect(find(el, ".table.table-cozy")).toBeTruthy();
  });

  it("defaults a cell's content to row[key] when no render is given", () => {
    const rows = signal<User[]>(sample);
    const columns: TableColumn<User>[] = [{ key: "name", label: "Name" }];
    const el = render(Table({ columns, rows, rowKey }));
    const firstRow = findAll(el, ".table-row")[0]!;
    expect(text(firstRow, ".table-td")).toBe("Ada");
  });

  it("renders a column's custom render() output inside its cell", () => {
    const rows = signal<User[]>(sample);
    const columns: TableColumn<User>[] = [
      { key: "name", label: "Name", render: (r) => html`<b>${r.name}</b>` },
    ];
    const el = render(Table({ columns, rows, rowKey }));
    const firstRow = findAll(el, ".table-row")[0]!;
    const bold = find(firstRow, "b")!;
    expect(bold).toBeTruthy();
    expect(text(bold)).toBe("Ada");
  });

  it("reuses the same <tr> DOM node when rows reorder (keyed reconciliation)", () => {
    const rows = signal<User[]>([sample[0]!, sample[1]!, sample[2]!]);
    const columns: TableColumn<User>[] = [{ key: "name", label: "Name" }];
    const el = render(Table({ columns, rows, rowKey }));
    const before = findAll(el, ".table-row");
    const node1 = before[0]!;
    rows.set([sample[2]!, sample[0]!, sample[1]!]);
    const after = findAll(el, ".table-row");
    expect(after.length).toBe(3);
    expect(after[1]).toBe(node1);
  });

  it("renders the default empty state when rows is empty", () => {
    const rows = signal<User[]>(sample);
    const columns: TableColumn<User>[] = [{ key: "name", label: "Name" }];
    const el = render(Table({ columns, rows, rowKey }));
    rows.set([]);
    expect(findAll(el, ".table-row").length).toBe(0);
    expect(find(el, ".table-empty")).toBeTruthy();
    expect(text(el, ".table-empty")).toContain("No data");
  });

  it("renders the custom empty slot when provided", () => {
    const rows = signal<User[]>([]);
    const columns: TableColumn<User>[] = [{ key: "name", label: "Name" }];
    const empty = html`<span class="custom-empty">Nothing here</span>`;
    const el = render(Table({ columns, rows, rowKey, empty }));
    expect(find(el, ".custom-empty")).toBeTruthy();
    expect(text(el, ".custom-empty")).toBe("Nothing here");
  });

  it("calls onRowClick with (row, index) when a row is clicked and marks the container clickable", () => {
    const rows = signal<User[]>(sample);
    const columns: TableColumn<User>[] = [{ key: "name", label: "Name" }];
    const onRowClick = spy<(row: User, i: number) => void>();
    const el = render(Table({ columns, rows, rowKey, onRowClick }));
    expect(find(el, ".table.table-clickable")).toBeTruthy();
    fire(findAll(el, ".table-row")[0]!, "click");
    expect(onRowClick).toHaveBeenCalledTimes(1);
    expect(onRowClick).toHaveBeenCalledWith(sample[0]!, 0);
  });

  it("shows the loading overlay and marks the container loading when loading flips true", () => {
    const rows = signal<User[]>(sample);
    const columns: TableColumn<User>[] = [{ key: "name", label: "Name" }];
    const loading = signal(false);
    const el = render(Table({ columns, rows, rowKey, loading }));
    expect(findAll(el, ".table-loading-overlay").length).toBe(0);
    expect(find(el, ".table.table-loading")).toBeNull();
    loading.set(true);
    expect(find(el, ".table-loading-overlay")).toBeTruthy();
    expect(find(el, ".table.table-loading")).toBeTruthy();
  });

  it("puts the alignment class on the <th> and every <td> of an aligned column", () => {
    const rows = signal<User[]>(sample);
    const columns: TableColumn<User>[] = [
      { key: "score", label: "Score", align: "end" },
    ];
    const el = render(Table({ columns, rows, rowKey }));
    expect(find(el, ".table-th.table-align-end")).toBeTruthy();
    expect(findAll(el, ".table-td.table-align-end").length).toBe(3);
  });

  it("adds table-fixed and inline width when a column declares width", () => {
    const rows = signal<User[]>(sample);
    const columns: TableColumn<User>[] = [
      { key: "name", label: "Name", width: "120px" },
    ];
    const el = render(Table({ columns, rows, rowKey }));
    expect(find(el, "table.table-fixed")).toBeTruthy();
    const th = find(el, ".table-th")!;
    expect(th.getAttribute("style")).toContain("width: 120px");
  });

  it("uses the requested density class when density is overridden", () => {
    const rows = signal<User[]>(sample);
    const columns: TableColumn<User>[] = [{ key: "name", label: "Name" }];
    const el = render(Table({ columns, rows, rowKey, density: "compact" }));
    expect(find(el, ".table.table-compact")).toBeTruthy();
    expect(find(el, ".table.table-cozy")).toBeNull();
  });

  it("throws when rowKey collides between two rows", () => {
    const rows = signal<User[]>([sample[0]!, { ...sample[1]!, id: 1 }]);
    const columns: TableColumn<User>[] = [{ key: "name", label: "Name" }];
    expect(() => render(Table({ columns, rows, rowKey }))).toThrow("duplicate key");
  });
});
