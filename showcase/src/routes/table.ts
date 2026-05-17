import { html, signal } from "zero";
import type { TemplateResult } from "zero";
import { Table, Badge, Button } from "zero/components";
import type { TableColumn } from "zero/components";

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

/**
 * Showcase route for Table — three instances exercise the main path,
 * the empty state, and the loading overlay.
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

      <a class="showcase-nav-link" href="/">Back</a>
    </main>
  `;
}
