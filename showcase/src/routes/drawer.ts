import { html, signal, computed } from "zero";
import type { Signal, TemplateResult } from "zero";
import { Drawer, Button, Input, Select, Table } from "zero/components";
import type { TableColumn } from "zero/components";

const ROLE_OPTIONS = [
  { value: "admin", label: "Admin" },
  { value: "user", label: "User" },
];

/**
 * A small "edit user" form used as a drawer body across several demos.
 *
 * @param {Signal<string>} name
 * @param {Signal<string>} role
 * @returns {TemplateResult}
 */
function userForm(name: Signal<string>, role: Signal<string>): TemplateResult {
  return html`
    <div class="stack gap-md">
      ${Input({ value: name, label: "Name", placeholder: "Ada Lovelace" })}
      ${Select({ value: role, label: "Role", options: ROLE_OPTIONS })}
    </div>
  `;
}

/**
 * Footer actions that close the drawer by flipping its `open` signal.
 *
 * @param {Signal<boolean>} open
 * @returns {TemplateResult}
 */
function saveCancel(open: Signal<boolean>): TemplateResult {
  return html`
    ${Button({ variant: "ghost", children: "Cancel", onClick: () => open.set(false) })}
    ${Button({ variant: "primary", children: "Save", onClick: () => open.set(false) })}
  `;
}

/**
 * Right-side overlay drawer with a form body and Save/Cancel controls.
 *
 * @returns {TemplateResult}
 */
function rightOverlaySection(): TemplateResult {
  const open = signal(false);
  const name = signal("");
  const role = signal("user");
  return html`
    <section class="stack gap-sm">
      <h2 class="text-h2">Right overlay (default size)</h2>
      <p class="text-body">Floats over content with a non-interactive backdrop. Save and Cancel both close it.</p>
      ${Button({ onClick: () => open.set(true), children: "Edit user" })}
      ${Drawer({
        open,
        side: "right",
        title: "Edit user",
        body: userForm(name, role),
        controls: saveCancel(open),
      })}
    </section>
  `;
}

/**
 * Right-side push drawer mounted as a flex sibling of a `<main>`; opening
 * it reflows the page content.
 *
 * @returns {TemplateResult}
 */
function rightPushSection(): TemplateResult {
  const open = signal(false);
  return html`
    <section class="stack gap-sm">
      <h2 class="text-h2">Right push (default size)</h2>
      <p class="text-body">Lives in flow as a flex sibling. Opening it reflows the content beside it instead of covering it.</p>
      ${Button({ onClick: () => open.update((v) => !v), children: "Toggle panel" })}
      <div class="cluster">
        <main class="grow stack pad-xl">
          <p class="text-body">Resize me. When the drawer opens, this column gives up room and the layout reflows naturally — no backdrop, no overlay.</p>
          <p class="text-body">Push mode is the right fit when the user must keep interacting with the content while the panel is open.</p>
        </main>
        ${Drawer({
          open,
          side: "right",
          mode: "push",
          title: "Details",
          body: html`<p class="text-body">An in-flow panel.</p>`,
        })}
      </div>
    </section>
  `;
}

/**
 * Left-side overlay drawer — verifies the opposite slide direction.
 *
 * @returns {TemplateResult}
 */
function leftOverlaySection(): TemplateResult {
  const open = signal(false);
  const name = signal("");
  const role = signal("admin");
  return html`
    <section class="stack gap-sm">
      <h2 class="text-h2">Left overlay</h2>
      <p class="text-body">Same shape as the right overlay, anchored to the left edge.</p>
      ${Button({ onClick: () => open.set(true), children: "Edit user" })}
      ${Drawer({
        open,
        side: "left",
        title: "Edit user",
        body: userForm(name, role),
        controls: saveCancel(open),
      })}
    </section>
  `;
}

/**
 * Top-side push drawer at the large size — verifies vertical-axis reflow.
 *
 * @returns {TemplateResult}
 */
function topPushSection(): TemplateResult {
  const open = signal(false);
  return html`
    <section class="stack gap-sm">
      <h2 class="text-h2">Top push (large)</h2>
      <p class="text-body">Block-axis push inside a vertical stack: the panel grows down from the top edge and pushes the content below it.</p>
      ${Button({ onClick: () => open.update((v) => !v), children: "Toggle banner" })}
      <div class="stack">
        ${Drawer({
          open,
          side: "top",
          mode: "push",
          size: "lg",
          title: "Announcement",
          body: html`<p class="text-body">A tall top panel that pushes the page down.</p>`,
        })}
        <main class="stack pad-xl">
          <p class="text-body">This content slides down when the top panel opens.</p>
        </main>
      </div>
    </section>
  `;
}

/**
 * Bottom-side overlay drawer at the small size.
 *
 * @returns {TemplateResult}
 */
function bottomOverlaySection(): TemplateResult {
  const open = signal(false);
  return html`
    <section class="stack gap-sm">
      <h2 class="text-h2">Bottom overlay (small)</h2>
      <p class="text-body">A short sheet that slides up from the bottom edge.</p>
      ${Button({ onClick: () => open.set(true), children: "Open sheet" })}
      ${Drawer({
        open,
        side: "bottom",
        size: "sm",
        title: "Quick action",
        body: html`<p class="text-body">A compact bottom sheet.</p>`,
        controls: html`${Button({ variant: "ghost", children: "Dismiss", onClick: () => open.set(false) })}`,
      })}
    </section>
  `;
}

/**
 * Shape A — one push drawer driven by several context signals; a
 * `computed` derives `open` and reactive slot functions swap content.
 *
 * @returns {TemplateResult}
 */
function shapeASection(): TemplateResult {
  const editingUser = signal<string | null>(null);
  const addingProduct = signal(false);
  const open = computed(() => editingUser.val !== null || addingProduct.val);
  const clear = (): void => {
    editingUser.set(null);
    addingProduct.set(false);
  };
  return html`
    <section class="stack gap-sm">
      <h2 class="text-h2">Shape A — context-driven forms (push)</h2>
      <p class="text-body">One drawer, several triggers. A <code>computed</code> over the context signals drives <code>open</code>; reactive slot functions swap the title/body/controls by active context.</p>
      <div class="cluster gap-md">
        ${Button({ onClick: () => editingUser.set("A"), children: "Edit user A" })}
        ${Button({ onClick: () => editingUser.set("B"), children: "Edit user B" })}
        ${Button({ onClick: () => addingProduct.set(true), children: "Add product" })}
      </div>
      <div class="cluster">
        <main class="grow stack pad-xl">
          <p class="text-body">The same panel stays mounted; only its contents change as you pick different actions.</p>
        </main>
        ${Drawer({
          open,
          side: "right",
          mode: "push",
          title: () =>
            editingUser.val
              ? `Edit user ${editingUser.val}`
              : addingProduct.val
                ? "Add product"
                : null,
          body: () => {
            const u = editingUser.val;
            if (u) return html`<p class="text-body">Editing user ${u}.</p>`;
            return addingProduct.val
              ? html`<p class="text-body">New product form.</p>`
              : null;
          },
          controls: () => html`${Button({ variant: "ghost", children: "Cancel", onClick: clear })}`,
        })}
      </div>
    </section>
  `;
}

type Row = { id: number; name: string; email: string };

const INSPECTOR_ROWS: Row[] = [
  { id: 1, name: "Ada Lovelace", email: "ada@example.com" },
  { id: 2, name: "Lin Chen", email: "lin@example.com" },
  { id: 3, name: "Marcus Reid", email: "marcus@example.com" },
  { id: 4, name: "Priya Shah", email: "priya@example.com" },
  { id: 5, name: "Hugo Park", email: "hugo@example.com" },
  { id: 6, name: "Yuki Tanaka", email: "yuki@example.com" },
  { id: 7, name: "Sam Diaz", email: "sam@example.com" },
  { id: 8, name: "Reza Khan", email: "reza@example.com" },
];

/**
 * Shape B — a push drawer inspecting the selected row of a Table. Push
 * mode is load-bearing: with no backdrop the rows stay clickable, so the
 * user can re-pick a different row and the body swaps without closing.
 *
 * @returns {TemplateResult}
 */
function shapeBSection(): TemplateResult {
  const rows = signal<Row[]>(INSPECTOR_ROWS);
  const selectedRow = signal<Row | null>(null);
  const open = computed(() => selectedRow.val !== null);
  const columns: TableColumn<Row>[] = [
    { key: "name", label: "Name" },
    { key: "email", label: "Email", width: "240px" },
  ];
  return html`
    <section class="stack gap-sm">
      <h2 class="text-h2">Shape B — inspector over a table (push)</h2>
      <p class="text-body">Click a row to inspect it. Because push mode renders <strong>no backdrop</strong>, the rows underneath stay clickable — pick another row and the drawer body swaps without closing. Overlay mode would intercept those clicks.</p>
      <div class="cluster">
        <section class="grow stack pad-xl">
          ${Table({
            columns,
            rows,
            rowKey: (r: Row) => r.id,
            onRowClick: (r: Row) => selectedRow.set(r),
          })}
        </section>
        ${Drawer({
          open,
          side: "right",
          mode: "push",
          title: () => (selectedRow.val ? selectedRow.val.name : null),
          body: () => {
            const r = selectedRow.val;
            return r ? html`<p class="text-body">${r.email}</p>` : null;
          },
          controls: () => html`${Button({ variant: "ghost", children: "Close", onClick: () => selectedRow.set(null) })}`,
        })}
      </div>
    </section>
  `;
}

/**
 * Slot-variation demos: one drawer with no title, one with no controls,
 * proving each region collapses independently.
 *
 * @returns {TemplateResult}
 */
function slotVariationsSection(): TemplateResult {
  const noTitle = signal(false);
  const noControls = signal(false);
  return html`
    <section class="stack gap-sm">
      <h2 class="text-h2">Slot variations</h2>
      <p class="text-body">Omitted slots collapse their wrapper via the native <code>hidden</code> attribute.</p>
      <div class="cluster gap-md">
        ${Button({ onClick: () => noTitle.set(true), children: "Open without title" })}
        ${Button({ onClick: () => noControls.set(true), children: "Open without controls" })}
      </div>
      ${Drawer({
        open: noTitle,
        side: "right",
        title: null,
        body: html`<p class="text-body">No title region here.</p>`,
        controls: html`${Button({ variant: "ghost", children: "Close", onClick: () => noTitle.set(false) })}`,
      })}
      ${Drawer({
        open: noControls,
        side: "left",
        title: "No footer",
        body: html`<p class="text-body">No controls region here.</p>`,
        controls: null,
      })}
    </section>
  `;
}

/**
 * Showcase route for Drawer — exercises every side, both modes, all
 * sizes, both canonical usage shapes (context-driven forms and an
 * inspector over a table), and the empty-slot collapse behaviour.
 *
 * @returns {TemplateResult}
 */
export default function DrawerRoute(): TemplateResult {
  return html`
    <main class="showcase-page stack pad-xl">
      <h1 class="text-h1">Drawer</h1>
      ${rightOverlaySection()}
      ${rightPushSection()}
      ${leftOverlaySection()}
      ${topPushSection()}
      ${bottomOverlaySection()}
      ${shapeASection()}
      ${shapeBSection()}
      ${slotVariationsSection()}
      <a class="showcase-nav-link" href="/">Back</a>
    </main>
  `;
}
