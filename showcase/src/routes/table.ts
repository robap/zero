import { html, signal, computed, effect } from "zero";
import type { Signal, TemplateResult } from "zero";
import { Table, Badge, Button, Pagination } from "zero/components";
import type { SortState, TableColumn } from "zero/components";

type User = {
  id: number;
  name: string;
  email: string;
  role: "admin" | "user";
  score: number;
};

const sample: User[] = [
  { id: 1, name: "Ada Lovelace", email: "ada@example.com", role: "admin", score: 92 },
  { id: 2, name: "Lin Chen", email: "lin@example.com", role: "user", score: 78 },
  { id: 3, name: "Marcus Reid", email: "marcus@example.com", role: "user", score: 64 },
  { id: 4, name: "Priya Shah", email: "priya@example.com", role: "admin", score: 88 },
  { id: 5, name: "Hugo Park", email: "hugo@example.com", role: "user", score: 71 },
  { id: 6, name: "Yuki Tanaka", email: "yuki@example.com", role: "user", score: 55 },
  { id: 7, name: "Sam Diaz", email: "sam@example.com", role: "user", score: 83 },
  { id: 8, name: "Reza Khan", email: "reza@example.com", role: "admin", score: 95 },
];

const columns: TableColumn<User>[] = [
  { key: "name", label: "Name" },
  { key: "email", label: "Email", width: "240px" },
  {
    key: "role",
    label: "Role",
    render: (r) =>
      Badge({
        variant: r.role === "admin" ? "primary" : "default",
        children: r.role,
      }),
  },
  { key: "score", label: "Score", align: "end" },
];

const rowKey = (r: User): number => r.id;

const PAGE_SIZE = 3;

/**
 * Static / client-side paginated Table — the parent owns `page`, derives
 * a sliced view via `computed`, and feeds it to `Table.rows`.
 *
 * @returns
 */
function paginatedSection(): TemplateResult {
  const page = signal(1);
  const totalPages = Math.ceil(sample.length / PAGE_SIZE);
  const rows = computed(() =>
    sample.slice((page.val - 1) * PAGE_SIZE, page.val * PAGE_SIZE),
  );
  const summary = (p: number, t: number): string => {
    const start = (p - 1) * PAGE_SIZE + 1;
    const end = Math.min(p * PAGE_SIZE, sample.length);
    return `Showing ${start}–${end} of ${sample.length} (page ${p} of ${t})`;
  };
  return html`
    <section class="stack gap-sm">
      <h2 class="text-h2">Paginated</h2>
      ${Table({ columns, rows: rows as unknown as Signal<User[]>, rowKey })}
      ${Pagination({ page, totalPages, summary })}
    </section>
  `;
}

/**
 * Mocked async-paginated Table — `page` change triggers a fake fetch
 * that resolves with `{ rows, totalPages }` after a delay. The `busy`
 * signal drives both `Table.loading` and `Pagination.disabled`.
 * Replace `fakeFetch` with whatever real backend call your app uses —
 * fetch, createHttp, GraphQL, etc. Pagination doesn't care.
 *
 * @returns
 */
function asyncSection(): TemplateResult {
  const page = signal(1);
  const totalPages = signal(1);
  const rows = signal<User[]>([]);
  const busy = signal(false);
  const fakeFetch = (p: number): Promise<{ rows: User[]; totalPages: number }> =>
    new Promise((resolve) =>
      setTimeout(
        () =>
          resolve({
            rows: sample.slice((p - 1) * PAGE_SIZE, p * PAGE_SIZE),
            totalPages: Math.ceil(sample.length / PAGE_SIZE),
          }),
        250,
      ),
    );
  effect(() => {
    const p = page.val;
    busy.set(true);
    fakeFetch(p).then((res) => {
      rows.set(res.rows);
      totalPages.set(res.totalPages);
      busy.set(false);
    });
  });
  return html`
    <section class="stack gap-sm">
      <h2 class="text-h2">Async (mocked)</h2>
      ${Table({ columns, rows, rowKey, loading: busy })}
      ${Pagination({ page, totalPages, disabled: busy })}
    </section>
  `;
}

/**
 * Client-side sortable Table — every sortable column owns its sort
 * cycle locally. The `score` column includes a deliberate `null` so
 * the default comparator's null-handling is visible. To switch into
 * server-side mode, pass `onSortChange` and re-fetch `rows` from the
 * parent — Table emits intent only and never reorders locally.
 *
 * @returns
 */
function sortableSection(): TemplateResult {
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

/**
 * Showcase route for Table — six instances exercise the main path,
 * the empty state, the loading overlay, sortable headers, static-
 * paginated, and async-paginated Table-with-Pagination wiring.
 *
 * @returns
 */
export default function TableRoute(): TemplateResult {
  const mainRows = signal<User[]>(sample);
  const clicked = signal<string | null>(null);
  const emptyRows = signal<User[]>([]);
  const loadingRows = signal<User[]>(sample.slice(0, 3));
  const loading = signal(false);
  return html`
    <main class="showcase-page stack pad-xl">
      <h1 class="text-h1">Table</h1>

      <section class="stack gap-sm">
        <h2 class="text-h2">Main</h2>
        ${Table({
          columns,
          rows: mainRows,
          rowKey,
          onRowClick: (row) => clicked.set(row.name),
          maxHeight: "320px",
        })}
        <p class="text-body">${() => (clicked.val ? `Last clicked: ${clicked.val}` : "Click a row")}</p>
      </section>

      <section class="stack gap-sm">
        <h2 class="text-h2">Empty</h2>
        ${Table({
          columns,
          rows: emptyRows,
          rowKey,
          empty: html`<span>No users to display yet.</span>`,
        })}
      </section>

      <section class="stack gap-sm">
        <h2 class="text-h2">Loading</h2>
        ${Table({ columns, rows: loadingRows, rowKey, loading })}
        ${Button({
          children: "Toggle loading",
          onClick: () => loading.update((v) => !v),
        })}
      </section>

      ${sortableSection()}
      ${paginatedSection()}
      ${asyncSection()}

      <a class="showcase-nav-link" href="/">Back</a>
    </main>
  `;
}
