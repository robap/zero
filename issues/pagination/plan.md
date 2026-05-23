# Plan: Pagination component

## Summary

Ship a sixteenth shipped component, `Pagination`, under `.zero/components/`.
It is a controlled, numbered pager — Prev / page buttons / ellipsis / Next —
driven by a `page: Signal<number>` and a `totalPages` that may be a plain
number or a signal. A second signal-or-plain prop, `disabled`, lets a parent
freeze the whole pager during async work. The component is decoupled from
`Table`; parents own data slicing.

The work is sequenced so each step leaves the workspace compiling and tests
green: (1) author the component + partial + tests in the scaffold and wire
them into the manifest, `_components.scss` aggregate, components index, and
`components.d.ts`; (2) bump the scaffold-test roster and the
`component_library.rs` roster; (3) add the showcase route and the two new
Table instances; (4) refresh the documentation surfaces (AGENTS.md,
docs/components.md, docs/index.md). The plan also resolves every spec open
question.

## Prerequisites

None. All decisions and open questions from the spec are resolved in **Risks
and Assumptions** below.

## Steps

- [x] **Step 1: Add Pagination scaffold templates**
- [x] **Step 2: Wire Pagination into the manifest, index, aggregate SCSS, and `components.d.ts`**
- [x] **Step 3: Bump the scaffold test roster**
- [x] **Step 4: Bump the `component_library.rs` test roster**
- [x] **Step 5: Add `Pagination.test.ts`**
- [x] **Step 6: Add the `/pagination` showcase route**
- [x] **Step 7: Extend `showcase/src/routes/table.ts` with paginated + async Table instances**
- [x] **Step 8: Refresh AGENTS.md, docs/components.md, and docs/index.md**

---

## Step Details

### Step 1: Add Pagination scaffold templates

**Goal:** Land the three Pagination files the manifest will reference so the
include_str!() additions in Step 2 don't fail to compile. After this step the
files exist but are not yet shipped to scaffolded projects (the manifest
still has 15 components).

**Files (new):**

- `crates/zero-scaffold/src/scaffold/.zero/components/Pagination.ts`
- `crates/zero-scaffold/src/scaffold/.zero/components/Pagination.test.ts`
  (an empty `describe("Pagination", () => {});` placeholder — real cases land
  in Step 5)
- `crates/zero-scaffold/src/scaffold/.zero/styles/components/_pagination.scss`

**Changes:**

- `Pagination.ts` — single default export with this exact signature:

  ```ts
  import { html } from "zero";
  import type { Signal, TemplateResult } from "zero";

  export type PaginationSize = "sm" | "md" | "lg";

  export type PaginationProps = {
    page: Signal<number>;
    totalPages: Signal<number> | number;
    size?: PaginationSize;
    siblingCount?: number;
    boundaryCount?: number;
    disabled?: Signal<boolean> | boolean;
    onChange?: (page: number) => void;
    prevLabel?: string;
    nextLabel?: string;
    summary?: (page: number, totalPages: number) => TemplateResult | string;
  };

  export default function Pagination(props: PaginationProps): TemplateResult;
  ```

- Internal `read<T>(p: Signal<T> | T): T` helper local to the file —
  `typeof p === "object" && p !== null && "val" in p && typeof (p as any).set === "function"`
  is the duck-type used to detect a `Signal`. No new public export is added
  to `"zero"` (constraint: no new runtime exports).

- Internal `pageItems(page, total, siblingCount, boundaryCount): (number | "...")[]`
  function that produces the ordered, dedup'd union per Requirement 3:
  1. sibling window: `[max(1, page - sibling), min(total, page + sibling)]`
  2. left boundary: `[1, boundaryCount]`
  3. right boundary: `[total - boundaryCount + 1, total]`
  4. union → sort ascending → dedupe
  5. walk the resulting list; where two consecutive numbers differ by `> 1`,
     emit a `"..."` between them.

- The component body resolves `size` once, builds the static outer-class
  prefix, and returns:

  ```ts
  const navCls = () => `pagination pagination-${size}${isDisabled() ? " pagination-disabled" : ""}`;
  return html`<nav class=${navCls} role="navigation" aria-label="Pagination">${summaryBlock}<ul class="pagination-list">${listBlock}</ul></nav>`;
  ```

  where `summaryBlock` is `props.summary ? () => html\`<div class="pagination-summary">${props.summary!(clampedPage(), resolvedTotal())}</div>\` : null` and `listBlock` is `() => [prev, ...pageButtons, next]`. Both
  are function children — the framework's reactive substitution re-runs them
  whenever the signals they read change. `clampedPage()` and `resolvedTotal()`
  read `props.page.val` / `read(props.totalPages)` / `read(props.disabled)`
  on each call.

- `isDisabled()` returns `read(props.disabled) === true || resolvedTotal() <= 1`.

- The prev/next handlers no-op when the corresponding edge is hit or the
  pager is disabled. Page-button click handlers no-op when the clicked page
  equals the clamped current page. `onChange` is only invoked on actual
  changes — never on no-ops.

- Function body stays under ~80 lines by extracting `pageItems`,
  `read`, and a small `pageButton(n)` helper above the default export. Each
  helper carries a JSDoc block per the CLAUDE.md JS/TS style.

- `_pagination.scss` follows the spec DOM shape and Requirements 12–22. Key
  decisions (locked here, see **Risks and Assumptions**):
  - Hover bg matches `Button.ghost`: `background: var(--hover-bg)` /
    `:active { background: var(--active-bg) }`.
  - `min-width` ladder matches Button's padding rhythm — `sm: 28px`,
    `md: 36px`, `lg: 44px` on `.pagination-btn`. `min-height` equal to
    `min-width` (square hit targets keep the row visually crisp).
  - Active-page treatment uses `Button.primary` palette:
    `background: var(--color-primary); color: var(--color-primary-fg)`.
  - Summary font-size locked to `var(--font-size-sm)` regardless of size
    class (Requirement 20 — picking the constant variant for predictability).
  - All padding uses `padding-inline` / `padding-block`. No `left`/`right`.

- `Pagination.test.ts` in this step contains the bare minimum
  (`import` + `describe("Pagination", () => {});`) so the manifest entry has
  something to point at. Step 5 fills it out.

**Tests:** `cargo test -p zero-scaffold` continues to pass — the new files
exist on disk but aren't yet referenced. (The manifest-set test is bumped in
the next step.)

---

### Step 2: Wire Pagination into the manifest, index, aggregate SCSS, and `components.d.ts`

**Goal:** Make Pagination a first-class manifest entry so `zero init` and
`zero update` materialize it, and so editors and templates resolve it via
`"zero/components"`.

**Files (modified):**

- `crates/zero-scaffold/src/lib.rs`
- `crates/zero-scaffold/src/scaffold/.zero/components/index.ts`
- `crates/zero-scaffold/src/scaffold/.zero/components.d.ts`
- `crates/zero-scaffold/src/scaffold/.zero/styles/_components.scss`

**Changes:**

- `lib.rs`:
  - Add three `TPL_PAGINATION_*` constants alongside the per-component block
    (kept alphabetical — between `TPL_INPUT_*` and `TPL_RADIO_*`):

    ```rust
    const TPL_PAGINATION_TS: &str = include_str!("scaffold/.zero/components/Pagination.ts");
    const TPL_PAGINATION_TEST_TS: &str = include_str!("scaffold/.zero/components/Pagination.test.ts");
    const TPL_PAGINATION_SCSS: &str = include_str!("scaffold/.zero/styles/components/_pagination.scss");
    ```

  - Add three manifest entries in `framework_manifest()` (kept alphabetical
    — directly after the three Input entries, before the Radio entries).

- `index.ts` — add (between `Input` and `Radio`):

  ```ts
  export { default as Pagination } from "./Pagination.ts";
  export type { PaginationProps, PaginationSize } from "./Pagination.ts";
  ```

- `components.d.ts` — add a `Pagination` block between `InputProps` /
  `Input(...)` and `RadioProps` / `Radio(...)`:

  ```ts
  export type PaginationSize = "sm" | "md" | "lg";
  export type PaginationProps = {
    page: Signal<number>;
    totalPages: Signal<number> | number;
    size?: PaginationSize;
    siblingCount?: number;
    boundaryCount?: number;
    disabled?: Signal<boolean> | boolean;
    onChange?: (page: number) => void;
    prevLabel?: string;
    nextLabel?: string;
    summary?: (page: number, totalPages: number) => TemplateResult | string;
  };
  export function Pagination(props: PaginationProps): TemplateResult;
  ```

- `_components.scss` — add `@use 'components/pagination';` in alphabetical
  position (between `input` and `radio`).

**Tests:** `cargo test -p zero-scaffold` now needs to know about Pagination
(this happens in Step 3). After this step alone the scaffold tests will fail
on the path-set assertion and the COMPONENT_NAMES iteration — that's the
signal to do Step 3.

---

### Step 3: Bump the scaffold test roster

**Goal:** Make the scaffold's own unit tests aware of Pagination so the
length-coupled assertions, the alphabetical name roster, and the manifest
path-set match the new manifest.

**Files (modified):**

- `crates/zero-scaffold/src/lib.rs` — only the `#[cfg(test)] mod tests`
  block.

**Changes:**

- `COMPONENT_NAMES` gains `"Pagination"` in alphabetical position:

  ```rust
  const COMPONENT_NAMES: &[&str] = &[
      "Avatar", "Badge", "Button", "Card", "Checkbox", "Dialog",
      "Input", "Pagination", "Radio", "Select", "Spinner", "Table",
      "Tabs", "TextArea", "Toast", "Toggle",
  ];
  ```

- `framework_manifest_matches_expected_path_set` (currently the only test
  with a hard-coded full path set) — add the three Pagination paths in
  alphabetical position. The accompanying `// 15 components × ...` comment
  bumps to `// 16 components × (source, test, scss partial) = 48 entries.`
  No `manifest.len()` literal exists — the test compares to
  `expected.len()`, so it's self-bumping once the BTreeSet gains the three
  entries.

- The iterating tests (`components_index_re_exports_each_listed`,
  `component_source_files_emitted`, `component_test_files_emitted`,
  `component_partials_use_layer_components`,
  `components_aggregate_uses_each_partial`,
  `components_dts_declares_each_listed`) all derive their coverage from
  `COMPONENT_NAMES` — the single edit above is sufficient for them.

**Tests:** `cargo test -p zero-scaffold` passes. Every per-component
existence assertion now covers Pagination.

---

### Step 4: Bump the `component_library.rs` test roster

**Goal:** The showcase integration test that runs every component's
`*.test.ts` already hard-codes the 15-name list. Bump to 16 so a Pagination
test that produces zero matches is a clear failure.

**Files (modified):**

- `crates/zero/tests/component_library.rs`

**Changes:**

- The inline string array in `showcase_test_runs_all_component_tests` gains
  `"Pagination"` in alphabetical position:

  ```rust
  for name in [
      "Avatar", "Badge", "Button", "Card", "Checkbox", "Dialog", "Input",
      "Pagination", "Radio", "Select", "Spinner", "Table", "Tabs",
      "TextArea", "Toast", "Toggle",
  ] { ... }
  ```

- No other integration test (`showcase_build.rs`, `showcase_dev.rs`,
  `design_system.rs`, `update.rs`) carries a length-coupled assertion against
  the component count — verified by reading them. They continue to pass
  unchanged.

**Tests:** `cargo test -p zero --test component_library` passes (after Step
5 fills out the test file; until then it would fail because no Pagination
test name appears in stdout).

---

### Step 5: Add `Pagination.test.ts`

**Goal:** Replace the placeholder with the full test suite enumerated in
Requirement 30.

**Files (modified):**

- `crates/zero-scaffold/src/scaffold/.zero/components/Pagination.test.ts`

**Changes:**

A single `describe("Pagination", () => { ... })` block with an
`afterEach(cleanup);` and these `it()` cases, one per bullet in
Requirement 30:

1. `renders the base markup` — `page: signal(1), totalPages: 5`. Asserts
   `find(el, "nav.pagination")`, `find(el, ".pagination-prev")`,
   `find(el, ".pagination-next")` are non-null. Collects the labels of
   `findAll(el, ".pagination-btn:not(.pagination-prev):not(.pagination-next)")`
   and asserts the array equals `["1", "2", "3", "4", "5"]`.

2. `marks the current page active` — initial `page=1` button carries
   `aria-current="page"` and class `pagination-active`. After
   `page.set(3)`, the `"3"` button gains both attributes and the `"1"`
   button loses them. Verifies the reactive block re-renders without
   remount (the wrapping `<nav>` node identity is the same before/after).

3. `prev/next click handlers` — registers an `onChange = spy()` and asserts
   that clicking `.pagination-next` increments `page.val` and calls
   `onChange` once with the new value; clicking `.pagination-prev` similarly
   decrements it.

4. `page-number click` — clicks the `"4"` button, asserts `page.val === 4`
   and `onChange.callCount === 1` with arg `4`.

5. `prev disabled at start; next disabled at end` — at `page=1`,
   `.pagination-prev` carries the native `disabled` attribute and clicking
   it does not mutate `page.val`; at `page=totalPages` the same is true for
   `.pagination-next`.

6. `ellipsis appears at expected positions` — `totalPages=20, page=10,
   siblingCount=1, boundaryCount=1`:
   - `findAll(el, ".pagination-ellipsis").length === 2`.
   - Labels of the non-prev/next buttons are `["1", "9", "10", "11", "20"]`.

7. `no ellipsis when totalPages is small` — `totalPages=5`: zero
   `.pagination-ellipsis` elements.

8. `single-page state is disabled` — `totalPages: 1`. Asserts outer `<nav>`
   has `pagination-disabled` and every `.pagination-btn` carries `disabled`.
   Clicking `.pagination-next` is a no-op.

9. `plain disabled: true freezes the pager` — `totalPages: 10, disabled: true`.
   Same disabled treatment.

10. `reactive disabled signal toggles state without remount` — captures the
    `<nav>` node, flips `disabled.set(true)`, re-queries: same node, now
    carries `pagination-disabled`, every `.pagination-btn` has the
    `disabled` attribute.

11. `reactive totalPages signal updates the list without remount` —
    `totalPages: signal(3), page: signal(1)`. Initial labels
    `["1","2","3"]`. After `totalPages.set(5)`, labels `["1","2","3","4","5"]`.
    Asserts the prev/next disabled state updates accordingly.

12. `out-of-range page clamps for rendering only` — `page: signal(0)`. The
    active button is `"1"`. `page.val` is still `0` (component does not
    rewrite the signal).

13. `size variant class` — `size: "sm"` puts `pagination-sm` on the outer
    `<nav>`. Default puts `pagination-md`.

14. `summary slot renders and updates` — `summary: (p, t) => \`Page ${p} of
    ${t}\``. `text(el, ".pagination-summary") === "Page 1 of 5"`. After
    `page.set(2)`, the text becomes `"Page 2 of 5"`.

Imports follow the existing precedent
(`Tabs.test.ts`/`Table.test.ts`/`Button.test.ts`):

```ts
import { describe, it, expect, afterEach } from "zero/test";
import { render, find, findAll, fire, cleanup, text, spy } from "zero/test";
import { signal } from "zero";
import Pagination from "./Pagination.ts";
```

**Tests:** `cargo run -p zero -- test Pagination.test.ts` runs locally from
`runtime/` or from a scaffolded showcase. `cargo test -p zero --test
component_library` passes again (the `"Pagination"` substring appears in
the report).

---

### Step 6: Add the `/pagination` showcase route

**Goal:** Manual / visual verification surface for Pagination and CI input
to `showcase_build.rs` / `showcase_dev.rs`.

**Files (new):**

- `showcase/src/routes/pagination.ts`

**Files (modified):**

- `showcase/src/app.ts` — `import PaginationRoute from "./routes/pagination.ts";`
  in alphabetical position (between Input and Radio) and
  `app.route("/pagination", PaginationRoute);` in the same alphabetical
  position.
- `showcase/src/routes/home.ts` — `{ name: "Pagination", href: "/pagination" }`
  in the `components` array between Input and Radio.

**Changes (route body):**

Four instances per Requirement 32. The fifth "async" instance is folded
into the Table route in Step 7 instead — keeps the Pagination route a clean
component showcase, lets the async wiring sit beside the Table that
illustrates it.

Page structure mirrors `routes/tabs.ts`:

```ts
import { html, signal } from "zero";
import type { TemplateResult } from "zero";
import { Pagination } from "zero/components";

export default function PaginationRoute(): TemplateResult {
  const a = signal(1);          // default md, totalPages 12
  const b = signal(1);          // sm, totalPages 20
  const c = signal(1);          // lg with summary, totalPages 5
  const d = signal(1);          // single-page disabled state

  return html`
    <main class="showcase-page stack pad-xl">
      <h1 class="text-h1">Pagination</h1>

      <section class="stack gap-sm">
        <h2 class="text-h2">Default (md)</h2>
        ${Pagination({ page: a, totalPages: 12 })}
        <p class="text-body">Current page: ${() => a.val}</p>
      </section>

      <section class="stack gap-sm">
        <h2 class="text-h2">Small</h2>
        ${Pagination({ page: b, totalPages: 20, size: "sm" })}
        <p class="text-body">Current page: ${() => b.val}</p>
      </section>

      <section class="stack gap-sm">
        <h2 class="text-h2">Large with summary</h2>
        ${Pagination({
          page: c,
          totalPages: 5,
          size: "lg",
          summary: (p, t) => `Page ${p} of ${t}`,
        })}
      </section>

      <section class="stack gap-sm">
        <h2 class="text-h2">Single page (auto-disabled)</h2>
        ${Pagination({ page: d, totalPages: 1 })}
      </section>

      <a class="showcase-nav-link" href="/">Back</a>
    </main>
  `;
}
```

**Tests:** `cargo test -p zero --test showcase_build` and `--test
showcase_dev` continue to pass. The dev test asserts the importmap and that
the index body contains `Avatar` — Pagination rides on the same plumbing,
no widening needed.

---

### Step 7: Extend `showcase/src/routes/table.ts` with paginated + async Table instances

**Goal:** Demonstrate the canonical Table-with-Pagination patterns
(Requirements 35 and 35a).

**Files (modified):**

- `showcase/src/routes/table.ts`

**Changes:**

- Import `computed, effect, signal` from `"zero"` (signal already present),
  and `Pagination` from `"zero/components"`.

- Add a fourth `<section>` titled `"Paginated"`:

  ```ts
  const pageSize = 3;
  const staticPage = signal(1);
  const staticTotalPages = Math.ceil(sample.length / pageSize);
  const staticRows = computed(() =>
    sample.slice((staticPage.val - 1) * pageSize, staticPage.val * pageSize),
  );
  ```

  Renders:

  ```
  ${Table({ columns, rows: staticRows, rowKey })}
  ${Pagination({
    page: staticPage,
    totalPages: staticTotalPages,
    summary: (p, t) => `Showing ${(p - 1) * pageSize + 1}–${Math.min(p * pageSize, sample.length)} of ${sample.length}`,
  })}
  ```

  `Table`'s `rows` prop expects `Signal<T[]>` but `computed` returns
  `Computed<T[]>` — both expose `.val`, and the existing `each()` runtime
  treats them identically. If the type-checker complains, cast through the
  shared shape: `rows: staticRows as unknown as Signal<User[]>` (a one-line
  workaround consistent with the existing showcase, which already uses
  `signal()`-backed rows elsewhere). The alternative — re-typing `each()` —
  is out of scope here.

- Add a fifth `<section>` titled `"Async (mocked)"`:

  ```ts
  const asyncPage = signal(1);
  const asyncTotalPages = signal(1);
  const asyncRows = signal<User[]>([]);
  const busy = signal(false);

  // Replace `fakeFetch` with whatever real backend call your app uses —
  // fetch, createHttp, GraphQL, etc. Pagination doesn't care.
  const fakeFetch = (p: number) =>
    new Promise<{ rows: User[]; totalPages: number }>((resolve) =>
      setTimeout(() => {
        const ps = 3;
        const total = Math.ceil(sample.length / ps);
        resolve({
          rows: sample.slice((p - 1) * ps, p * ps),
          totalPages: total,
        });
      }, 250),
    );

  effect(() => {
    const p = asyncPage.val;
    busy.set(true);
    fakeFetch(p).then((res) => {
      asyncRows.set(res.rows);
      asyncTotalPages.set(res.totalPages);
      busy.set(false);
    });
  });
  ```

  Renders:

  ```
  ${Table({ columns, rows: asyncRows, rowKey, loading: busy })}
  ${Pagination({
    page: asyncPage,
    totalPages: asyncTotalPages,
    disabled: busy,
  })}
  ```

- The two new sections sit at the end of the existing `<main>` content,
  after the `Loading` section, before the back link. Use the same
  `<section class="stack gap-sm">` shell the existing sections use.

**Tests:** `cargo test -p zero --test showcase_build` and `--test
showcase_dev` continue to pass — no per-route assertion checks the section
count.

---

### Step 8: Refresh AGENTS.md, docs/components.md, and docs/index.md

**Goal:** Documentation surfaces all agree the library has sixteen
components and Pagination is one of them.

**Files (modified):**

- `crates/zero-scaffold/src/scaffold/AGENTS.md`
- `docs/components.md`
- `docs/index.md`

**Files explicitly NOT modified:**

- `zero-framework-spec.md` — does not exist in this repo (verified with
  `find`). Requirements 42 and 43 are documentation surfaces that landed in
  the spec by analogy with another project; there is nothing to update.
- `docs/best-practices.md` — Requirement 44 leaves this to plan judgment;
  the showcase Table route already demonstrates both static and async
  patterns end-to-end and `§7 Component usage` already covers the
  "prefer shipped components" stance. Adding another subsection just for
  Pagination would not pay for its bytes. Deferred.

**Changes:**

- `crates/zero-scaffold/src/scaffold/AGENTS.md`:
  - In the `## Component library` table (between `Input` and `Radio` rows):
    `` | `Pagination` | `page: Signal<number>`, `totalPages: Signal<number> | number` | ``
  - In the `## The .zero/ directory` table, bump both
    `(15 total)` strings to `(16 total)` on the
    `.zero/components/<Name>.ts` and `.zero/components/<Name>.test.ts` rows
    and the `.zero/styles/components/_<name>.scss` row.

- `docs/components.md`:
  - Line 147: `fifteen production-ready components` → `sixteen production-ready components`.
  - Insert a new row in the summary table between `Input` and `Radio`:
    ```
    | `Pagination` | `page: Signal<number>`, `totalPages: Signal<number> \| number`; optional `size`, `siblingCount`, `boundaryCount`, `disabled`, `onChange`, `summary` | `Pagination({ page, totalPages: 10 })` |
    ```

- `docs/index.md`:
  - Line 40: `fifteen shipped components` → `sixteen shipped components`.

**Tests:** None of these are exercised by integration tests. Visual
verification via `cargo test -p zero-scaffold`'s `agents_md_has_section_sentinels`
(unchanged — sentinels still present) and a manual `cargo build --workspace`
to confirm `include_str!` resolves.

---

## Risks and Assumptions

**Resolved open questions (from spec §"Open Questions"):**

- **Default `siblingCount=1, boundaryCount=1`.** Confirmed. Wider defaults
  (`2/1`) trade a wider pager for marginal information density; staying at
  `1/1` matches the example renderings already in the spec and is the most
  common default in MUI/AntD-style pagers.
- **Keep `siblingCount` and `boundaryCount` as v1 props.** Confirmed. They
  cost a few lines each, the algorithm already needs them as inputs, and
  removing them now would be a v1 → v2 breaking change.
- **Arrow-key navigation deferred.** Tab/Enter/Space already work via
  native `<button>`. Capturing Left/Right on the outer `<nav>` introduces
  focus-management questions (only when a pager button has focus? how does
  that interact with the test runner's in-memory DOM?) that don't pay back
  in v1. Filed as out-of-scope.
- **Prev/Next labels are `aria-label` only.** Visible text stays `‹` / `›`.
  Cheap variant per the spec.
- **Summary placement.** Inside the `<nav>`, before the `<ul>`, per the
  spec sketch. Simplest layout and matches the test's
  `text(el, ".pagination-summary")` selector.
- **Hover token.** `var(--hover-bg)` / `var(--active-bg)` — reuses
  exactly what `Button.ghost` uses today (verified in `_button.scss`).
- **`.pagination-btn` sizing.** `sm: 28px`, `md: 36px`, `lg: 44px` square.
  Matches the height of Button at each size (Button uses padding to define
  height; Pagination uses `min-width`/`min-height` because its content is a
  single number that would otherwise produce uneven widths).
- **Page-button `aria-label`.** Hardcoded English `"Page {n}"`. An optional
  `pageLabel: (n) => string` prop is appealing for i18n but inconsistent
  with the rest of the library (none of the other components expose i18n
  hooks today). Defer to a future a11y/i18n pass.
- **Render-only clamp.** Component does not rewrite `page.val`. Confirmed —
  per Requirement 2 and test case #12.
- **`onChange` not called on no-ops.** Confirmed.
- **`PaginationProps` is re-exported from `index.ts`.** Confirmed — matches
  `TableColumn` precedent.
- **AGENTS.md grouping.** No subgrouping in the AGENTS.md component table;
  it is alphabetical. Pagination sits between Input and Radio.
- **Manifest size.** Current manifest has 47 text entries (15 components × 3
  + 2 type declarations + 1 components index + 1 components.d.ts + 7 styles
  partials + 1 styles aggregate + 1 zero.scss + 4 zero/zero-test/zero-http
  + 1 AGENTS.md). After Pagination it has 50. No literal check on this
  number — the manifest path-set assertion is the only length-coupled
  test, and it self-bumps via `expected.len()`.
- **Showcase route shape.** Four instances on `/pagination` (default, sm,
  lg+summary, single-page). The async instance lives on `/table` instead —
  it composes the canonical Table+Pagination pattern there and avoids
  duplicating wiring on two routes.

**Assumptions worth flagging:**

- **`Signal<T>` vs `Computed<T>` for `Table.rows`.** The `Table` type signature
  is `rows: Signal<T[]>` but the static-paginated example uses a
  `Computed<User[]>` (derived slice). The runtime treats both interchangeably
  (both expose `.val` and re-subscribe via `each()`), but TypeScript may
  complain. The plan uses a one-line cast (`as unknown as Signal<User[]>`)
  in the showcase route — the same pragmatic workaround in
  `examples/todos/web/src/...` patterns. If a clean fix is required,
  widening `Table.rows` to `Signal<T[]> | Computed<T[]>` is a separate
  trivial PR.

- **Reactive substitution semantics.** The plan relies on the fact that a
  function-valued substitution (e.g. `${listBlock}` where
  `listBlock = () => [prev, ...pageButtons, next]`) re-runs whenever the
  signals it reads (`props.page.val`, `read(props.totalPages)`,
  `read(props.disabled)`) change. This is the same pattern used by
  `Dialog.ts:42–47` (`body = (): TemplateResult | null => ... ${body}`) and
  by Tabs's panel slot (`${() => props.panels[props.active.val] ?? null}`).
  If for any reason the page-button list does not re-render on signal
  change, the fallback is to wrap the list in `each()` with a numeric key —
  but the spec specifically calls out "reactive block (or `computed`)" and
  the existing precedent confirms function-substitution is sufficient.

- **`isSignal` is not a public runtime export.** The component duck-types
  signals via `typeof p === "object" && p !== null && "val" in p && typeof
  (p as any).set === "function"`. If the runtime gains a public `isSignal`
  later, the local helper collapses to one line.

- **Geometry of the page-button row.** The spec leaves `min-width` /
  `min-height` to "match Button's existing visual rhythm" — Button uses
  padding only, so its visual heights are roughly 28/36/44px. The
  Pagination `min-*` values aim to match those heights so a Pagination row
  reads at the same density as a button cluster. If the visual review
  rejects these specifically, they are the only magic numbers in the
  partial and easy to tune.
