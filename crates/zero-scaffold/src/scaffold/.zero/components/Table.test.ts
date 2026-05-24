import { describe, it, expect, afterEach } from "zero/test";
import { render, find, findAll, fire, cleanup, text, spy } from "zero/test";
import { signal, html } from "zero";
import Table from "./Table.ts";
import type { SortState, TableColumn } from "./Table.ts";

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

describe("Table sort", () => {
  afterEach(cleanup);

  it("renders a sortable column as a button inside a th with aria-sort='none'", () => {
    const rows = signal<User[]>(sample);
    const sort = signal<SortState | null>(null);
    const columns: TableColumn<User>[] = [
      { key: "name", label: "Name", sortable: true },
    ];
    const el = render(Table({ columns, rows, rowKey, sort }));
    const th = find(el, ".table-th")!;
    expect(th).toBeTruthy();
    expect(th.getAttribute("aria-sort")).toBe("none");
    expect(find(th, "button.table-sort-btn")).toBeTruthy();
  });

  it("cycles asc -> desc -> null and updates aria-sort and row order on each click", () => {
    const rows = signal<User[]>(sample);
    const sort = signal<SortState | null>(null);
    const columns: TableColumn<User>[] = [
      { key: "name", label: "Name", sortable: true },
    ];
    const el = render(Table({ columns, rows, rowKey, sort }));
    const btn = find(el, "button.table-sort-btn")!;
    const th = find(el, ".table-th")!;

    fire(btn, "click");
    expect(sort.val).toEqual({ key: "name", dir: "asc" });
    expect(th.getAttribute("aria-sort")).toBe("ascending");
    expect(text(findAll(el, ".table-row")[0]!, ".table-td")).toBe("Ada");

    fire(btn, "click");
    expect(sort.val).toEqual({ key: "name", dir: "desc" });
    expect(th.getAttribute("aria-sort")).toBe("descending");
    expect(text(findAll(el, ".table-row")[0]!, ".table-td")).toBe("Marcus");

    fire(btn, "click");
    expect(sort.val).toBeNull();
    expect(th.getAttribute("aria-sort")).toBe("none");
    expect(text(findAll(el, ".table-row")[0]!, ".table-td")).toBe("Ada");
  });

  it("clicking a different sortable column resets to asc on the new column", () => {
    const rows = signal<User[]>(sample);
    const sort = signal<SortState | null>({ key: "name", dir: "desc" });
    const columns: TableColumn<User>[] = [
      { key: "name", label: "Name", sortable: true },
      { key: "score", label: "Score", sortable: true },
    ];
    const el = render(Table({ columns, rows, rowKey, sort }));
    const ths = findAll(el, ".table-th");
    const scoreBtn = find(ths[1]!, "button.table-sort-btn")!;
    fire(scoreBtn, "click");
    expect(sort.val).toEqual({ key: "score", dir: "asc" });
    expect(ths[0]!.getAttribute("aria-sort")).toBe("none");
    expect(ths[1]!.getAttribute("aria-sort")).toBe("ascending");
  });

  it("default comparator sorts numbers and strings correctly in both directions", () => {
    const rows = signal<User[]>(sample);
    const sort = signal<SortState | null>(null);
    const columns: TableColumn<User>[] = [
      { key: "name", label: "Name", sortable: true },
      { key: "score", label: "Score", sortable: true },
    ];
    const el = render(Table({ columns, rows, rowKey, sort }));

    sort.set({ key: "score", dir: "asc" });
    expect(text(findAll(el, ".table-row")[0]!, ".table-td")).toBe("Marcus");
    sort.set({ key: "score", dir: "desc" });
    expect(text(findAll(el, ".table-row")[0]!, ".table-td")).toBe("Ada");

    sort.set({ key: "name", dir: "asc" });
    expect(text(findAll(el, ".table-row")[0]!, ".table-td")).toBe("Ada");
    sort.set({ key: "name", dir: "desc" });
    expect(text(findAll(el, ".table-row")[0]!, ".table-td")).toBe("Marcus");
  });

  it("sorts nullish values last in asc, first in desc", () => {
    type Nullable = { id: number; name: string; score: number | null };
    const data: Nullable[] = [
      { id: 1, name: "Ada", score: 92 },
      { id: 2, name: "Lin", score: null },
      { id: 3, name: "Marcus", score: 64 },
    ];
    const rows = signal<Nullable[]>(data);
    const sort = signal<SortState | null>(null);
    const columns: TableColumn<Nullable>[] = [
      { key: "name", label: "Name" },
      { key: "score", label: "Score", sortable: true },
    ];
    const key = (r: Nullable): number => r.id;
    const el = render(Table({ columns, rows, rowKey: key, sort }));

    sort.set({ key: "score", dir: "asc" });
    const ascNames = findAll(el, ".table-row").map((r) => text(r, ".table-td"));
    expect(ascNames[ascNames.length - 1]).toBe("Lin");

    sort.set({ key: "score", dir: "desc" });
    const descNames = findAll(el, ".table-row").map((r) => text(r, ".table-td"));
    expect(descNames[0]).toBe("Lin");
  });

  it("uses a column's custom compare instead of the default row[key] comparison", () => {
    type WithPriority = { id: number; name: string; priority: number };
    const data: WithPriority[] = [
      { id: 1, name: "Charlie", priority: 1 },
      { id: 2, name: "Alpha", priority: 3 },
      { id: 3, name: "Bravo", priority: 2 },
    ];
    const rows = signal<WithPriority[]>(data);
    const sort = signal<SortState | null>(null);
    const columns: TableColumn<WithPriority>[] = [
      {
        key: "name",
        label: "Name",
        sortable: true,
        compare: (a, b) => a.priority - b.priority,
      },
    ];
    const key = (r: WithPriority): number => r.id;
    const el = render(Table({ columns, rows, rowKey: key, sort }));
    sort.set({ key: "name", dir: "asc" });
    const order = findAll(el, ".table-row").map((r) => text(r, ".table-td"));
    expect(order).toEqual(["Charlie", "Bravo", "Alpha"]);
  });

  it("throws when a column is sortable but no sort signal was passed", () => {
    const rows = signal<User[]>(sample);
    const columns: TableColumn<User>[] = [
      { key: "name", label: "Name", sortable: true },
    ];
    expect(() => render(Table({ columns, rows, rowKey }))).toThrow("sort");
  });

  it("fires onSortChange with the next sort state when a header is clicked", () => {
    const rows = signal<User[]>(sample);
    const sort = signal<SortState | null>(null);
    const onSortChange = spy<(s: SortState | null) => void>();
    const columns: TableColumn<User>[] = [
      { key: "name", label: "Name", sortable: true },
    ];
    const el = render(Table({ columns, rows, rowKey, sort, onSortChange }));
    fire(find(el, "button.table-sort-btn")!, "click");
    expect(onSortChange).toHaveBeenCalledTimes(1);
    expect(onSortChange).toHaveBeenCalledWith({ key: "name", dir: "asc" });
  });

  it("in server-side mode, renders rows in the parent's order regardless of sort", () => {
    const unsortedOrder: User[] = [
      { id: 3, name: "Marcus", role: "user", score: 64 },
      { id: 1, name: "Ada", role: "admin", score: 92 },
      { id: 2, name: "Lin", role: "user", score: 78 },
    ];
    const rows = signal<User[]>(unsortedOrder);
    const sort = signal<SortState | null>(null);
    const onSortChange = spy<(s: SortState | null) => void>();
    const columns: TableColumn<User>[] = [
      { key: "name", label: "Name", sortable: true },
    ];
    const el = render(Table({ columns, rows, rowKey, sort, onSortChange }));
    fire(find(el, "button.table-sort-btn")!, "click");
    const rendered = findAll(el, ".table-row").map((r) => text(r, ".table-td"));
    expect(rendered).toEqual(["Marcus", "Ada", "Lin"]);
  });

  it("in server-side mode, the sort signal still updates on click", () => {
    const rows = signal<User[]>(sample);
    const sort = signal<SortState | null>(null);
    const onSortChange = spy<(s: SortState | null) => void>();
    const columns: TableColumn<User>[] = [
      { key: "name", label: "Name", sortable: true },
    ];
    const el = render(Table({ columns, rows, rowKey, sort, onSortChange }));
    fire(find(el, "button.table-sort-btn")!, "click");
    expect(sort.val).toEqual({ key: "name", dir: "asc" });
  });

  it("renders non-sortable columns as plain th without aria-sort or a button", () => {
    const rows = signal<User[]>(sample);
    const sort = signal<SortState | null>(null);
    const columns: TableColumn<User>[] = [
      { key: "name", label: "Name", sortable: true },
      { key: "role", label: "Role" },
    ];
    const el = render(Table({ columns, rows, rowKey, sort }));
    const ths = findAll(el, ".table-th");
    expect(ths[1]!.getAttribute("aria-sort")).toBeNull();
    expect(find(ths[1]!, "button")).toBeNull();
  });
});
