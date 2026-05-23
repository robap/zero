# Spec: Pagination component

## Problem Statement

The component library now ships fifteen components, including a `Table` that
deliberately defers pagination. Every real zero app that surfaces lists of
records — search results, user directories, paginated APIs — currently
hand-rolls the same Prev/Next markup, the same "page X of Y" labels, the same
ellipsis math when there are more than a handful of pages. Each reinvention
drifts off the design-system token palette and re-litigates the same
keyboard and ARIA decisions.

This issue adds `Pagination` to the shipped component library. Scope is
deliberately tight: a controlled, numbered pager with prev/next buttons and
ellipsis for crowded ranges, driven by a `page` signal and a plain
`totalPages`. It is **decoupled from Table** — the parent owns slicing data
into a page and feeds whichever rendering surface (Table, card list, grid)
they like. The goal is the smallest component that makes the 80% "jump to
page N of M" case obvious — not a generic data-grid pager.

## Background

### What exists today

- **Component library (Phase 9 + Table extension).** Fifteen components ship
  under `.zero/components/`, re-exported from `.zero/components/index.ts`,
  importable via the bare specifier `"zero/components"`. Each component:
  - Is a plain function: `Component<P> = (props?: P) => TemplateResult`.
  - Reads stateful props as signals; never holds internal state for
    `value`/`open`/`active`/`checked`/etc.
  - Has a per-component SCSS partial under
    `.zero/styles/components/_{name}.scss`, every rule inside
    `@layer components { ... }`.
  - Has a `*.test.ts` neighbor that ships in the manifest and runs with
    `zero test`.
- **Scaffold / manifest plumbing.** `framework_manifest()` in
  `crates/zero-scaffold/src/lib.rs` lists every framework-owned file under
  `.zero/`. Each entry is a `TPL_*` constant pointing at
  `include_str!("scaffold/.zero/...")`. The `_components.scss` aggregate
  `@use`s every per-component partial in alphabetical order; `zero.scss`
  `@use`s `'components'` last.
- **Module resolution.** The dev-server transpile pipeline and the
  production bundler both resolve `"zero/components"` to
  `.zero/components/index.ts`. Adding a new component is one line in that
  index file plus the file itself — no resolver change.
- **Design-system tokens (Phases 7–8).** All values components are allowed
  to consume are `var(--*)` custom properties on `:root`. Spacing:
  `--space-{xs,sm,md,lg,xl}`. Colors: `--color-{bg,surface,text,text-muted,border,border-strong,primary,primary-fg,...}`.
  Radii: `--radius-{sm,md,lg}`. Font sizes/weights/leadings, shadows, border
  widths — all present. Dark theme overrides only the semantic `--color-*`
  set.
- **Showcase project.** `showcase/` is a full zero project at the repo
  root; every component has a route under `showcase/src/routes/`. Built by
  `zero build` in CI via `tests/showcase_build.rs`; served by `zero dev` in
  CI via `tests/showcase_dev.rs`. Component tests run via
  `tests/component_library.rs`. Adding a component means adding one route
  file and updating the home route's navigation cluster.
- **Reactivity primitives.** `signal`, `computed`, `effect`. Reactive
  bindings in templates auto-update when their dependent signals change.
- **Existing precedents the new component echoes:**
  - `Button` — variants/sizes pattern, `disabled` as a plain boolean,
    `onClick` callback, `loading` flag. Pagination's per-page buttons are
    rendered with the same `<button>` markup and visual treatment, with the
    active page highlighted via a state class.
  - `Tabs` — declarative list driven by an items array plus a single
    "active" `Signal<string>`. Pagination follows the same shape: an
    internally-computed page-button list plus an "active" `Signal<number>`.
  - `Table` — `Pagination` is decoupled from it. Parent computes
    `totalPages = ceil(rows.length / pageSize)`, slices its data into the
    visible window via a derived signal, and passes that slice to
    `Table.rows`. Pagination updates `page`; the derived slice signal
    re-evaluates; Table re-renders. No internal coupling.

### Decisions made during refine

The user confirmed each of the following:

- **Packaging: standalone component.** A new `Pagination` under
  `.zero/components/Pagination.ts`. Not a Table prop. Decoupled from any
  particular data surface so it works for lists, cards, grids, anything.
- **State model: controlled — `page: Signal<number>` + signal-or-plain
  `totalPages`.** Parent owns the `page` signal; the component reads `.val`
  and calls `.set()` when the user clicks a page button. `totalPages`
  accepts either `number` or `Signal<number>` — static datasets pass a
  plain number, async datasets pass a signal the parent updates after a
  fetch returns. The component reads `.val` if the prop is a signal,
  otherwise reads the value directly. Pagination itself does no fetching
  and prescribes no backend shape.
- **`disabled` is signal-or-plain too.** `disabled?: Signal<boolean> | boolean`.
  Lets a parent disable all controls during an in-flight request (or any
  other async pause) without remounting the pager. Same read pattern as
  `totalPages`.
- **UI shape: numbered pages + prev/next + ellipsis.** Renders like
  `[ ‹ ] [ 1 ] [ 2 ] [ 3 ] … [ 9 ] [ 10 ] [ › ]`. Clicking any visible
  number jumps directly to that page. Prev/Next step by one. Ellipsis is
  non-interactive (a `<span>`, not a `<button>`).
- **Disable when single page.** When `totalPages <= 1`, the component
  still renders (layout stability) but every control is disabled. No
  auto-hide.
- **Size variant.** `size?: "sm" | "md" | "lg"`, default `"md"`. Matches
  the size convention used by `Button`, `Input`, `Select`.
- **Summary as render slot, not built-in.** Optional
  `summary?: (page, totalPages) => TemplateResult | string` slot. Parent
  formats their own "Showing X–Y of Z" text using the data and page size
  they already know. Keeps the API surface tight and localization-friendly.
  No `pageSize` or `total` props on Pagination.
- **Showcase: dedicated route + Table integration.** New
  `showcase/src/routes/pagination.ts` demonstrating standalone usage. The
  existing `showcase/src/routes/table.ts` gains a fourth Table instance
  paired with `Pagination` over the 8 sample users (pageSize 3), showing
  the canonical Table-with-pagination pattern.

### Component contract

Same conventions as the existing roster:

- Variants and sizes are string-typed string-union props.
- Stateful prop the parent observes is a signal: `page: Signal<number>`.
- Configuration is plain values: `totalPages`, `size`, `siblingCount`,
  `boundaryCount`, `disabled`, `summary`, `onChange`, `prevLabel`,
  `nextLabel`.
- Event callbacks are plain functions: `onChange?: (page: number) => void`.
  Called *after* `page.set(newPage)`. Optional — typical callers just react
  to the signal.
- Pages are **1-indexed**. `page.val === 1` is the first page;
  `page.val === totalPages` is the last. Matches the displayed labels and
  matches the universal web convention.

### Props sketch

```ts
type PaginationProps = {
  page: Signal<number>;                          // 1-indexed; parent owns it
  totalPages: Signal<number> | number;           // signal for async, number for static; >= 1
  size?: "sm" | "md" | "lg";                     // default "md"
  siblingCount?: number;                          // default 1 — pages shown on each side of current
  boundaryCount?: number;                         // default 1 — pages shown at each end
  disabled?: Signal<boolean> | boolean;          // optional; auto-true when totalPages <= 1
  onChange?: (page: number) => void;              // called after page.set(); typically unused
  prevLabel?: string;                             // default "Previous" — used for aria-label
  nextLabel?: string;                             // default "Next"
  summary?: (page: number, totalPages: number) => TemplateResult | string;
};
```

The exact item-list algorithm (how many ellipses appear and where) is for
the plan to finalize, but the contract is:

- Always show the boundary pages on each end (`boundaryCount` controls the
  count; default 1 means always show page 1 and page `totalPages`).
- Always show `siblingCount` pages on each side of the current page
  (default 1 means always show current ± 1 when they exist).
- Insert an ellipsis (`…`) where the visible run is broken by hidden
  pages.
- When `totalPages` is small enough that the full range would be shown
  anyway, render every page with no ellipsis.

Example, `siblingCount=1, boundaryCount=1, totalPages=10, page=5`:
`‹ 1 … 4 5 6 … 10 ›`.

Example, `totalPages=5, page=3`: `‹ 1 2 3 4 5 ›` (no ellipsis).

### Async / server-side usage (illustrative, not prescriptive)

Pagination is presentation-only. It exposes three reactive surfaces that
let a parent drive it from whatever async source it likes — `fetch`,
`createHttp()`, a GraphQL client, an indexedDB query, anything:

- **`page: Signal<number>`** — the parent observes it (via `effect()`,
  `computed()`, or just by re-reading `.val` from other reactive code) to
  know when the user requested a new page.
- **`totalPages: Signal<number> | number`** — when the parent's async
  source returns a new total, the parent calls `.set()` on its
  `totalPages` signal and Pagination's button list updates automatically.
- **`disabled: Signal<boolean> | boolean`** — flipped to `true` while a
  request is in flight to prevent the user from queuing clicks; flipped
  back to `false` (or omitted) when the response lands.

The canonical wiring (one of many — the framework does not prescribe a
specific shape):

```ts
const page = signal(1);
const totalPages = signal(1);
const rows = signal<User[]>([]);
const busy = signal(false);

effect(() => {
  const p = page.val;             // re-runs whenever the user clicks a page
  busy.set(true);
  myBackend.getUsers(p).then((res) => {
    rows.set(res.rows);
    totalPages.set(res.totalPages);
    busy.set(false);
  });
});

// in the template
${Table({ columns, rows, rowKey: r => r.id, loading: busy })}
${Pagination({ page, totalPages, disabled: busy })}
```

`myBackend.getUsers(p)` is whatever the user wrote — REST, GraphQL, a
mocked promise, a websocket round-trip. Pagination doesn't see it, and
nothing in this spec depends on the response shape.

The same reactive seam composes with other parent concerns. Syncing
the URL to the current page is just a second `effect()` on the same
signal:

```ts
effect(() => {
  navigate(`/parts/${page.val}`, { replace: true });
});
```

And the reverse direction — restoring `page` from the URL on initial
navigation — goes through the route's `load()`:

```ts
export async function load({ params }) {
  page.set(parseInt(params.page) || 1);
}
```

Fetch, URL sync, and any other parent reaction all live in separate
effects watching the same `page` signal. Pagination does not need to
know any of them exist.

The takeaway: Pagination accepting signals for `totalPages` and
`disabled` is the *only* concession to async usage. There is no
backend-aware API, no fetch helper, no "data source" abstraction, no
URL-sync helper. The component just exposes the right reactive seams.

### DOM shape

```
<nav class="pagination pagination-{size} [pagination-disabled]"
     role="navigation"
     aria-label="Pagination">
  <!-- summary, if provided -->
  <div class="pagination-summary">…</div>

  <ul class="pagination-list">
    <li>
      <button class="pagination-btn pagination-prev"
              aria-label="Previous"
              [disabled]>‹</button>
    </li>
    <li>
      <button class="pagination-btn"
              aria-label="Page 1"
              [aria-current="page"]>1</button>
    </li>
    <li>
      <span class="pagination-ellipsis" aria-hidden="true">…</span>
    </li>
    <li>
      <button class="pagination-btn pagination-active"
              aria-label="Page 5"
              aria-current="page">5</button>
    </li>
    ...
    <li>
      <button class="pagination-btn pagination-next"
              aria-label="Next"
              [disabled]>›</button>
    </li>
  </ul>
</nav>
```

- Buttons re-render via a reactive block that depends on `props.page`,
  `props.totalPages` (when a signal), and `props.disabled` (when a
  signal). The block recomputes the visible-page list whenever any of
  those values change. Plain `totalPages` / `disabled` values are read
  once at mount.
- Prev is disabled when `page.val <= 1`. Next is disabled when
  `page.val >= resolvedTotal`. Both also disabled when the whole component
  is disabled (single-page case or explicit `disabled` set to `true`,
  signal or plain).
- The active page button is the only one tagged `aria-current="page"` and
  also visually distinguished via the `.pagination-active` class.

## Requirements

### Component

1. New file `.zero/components/Pagination.ts` exports a single default
   `Pagination` function matching `Component<PaginationProps>`. The
   component runs once per mount; page-list reactivity is delivered via a
   reactive block (or `computed`) that depends on `props.page`.

2. Pages are 1-indexed. The current page used for rendering is
   `clamp(props.page.val, 1, resolvedTotal)`, where `resolvedTotal` is the
   value of `props.totalPages` (read `.val` if it's a signal, otherwise
   read directly). If a parent stores 0 or `total + 1` in the signal, the
   component renders as if the page were the nearest valid value. The
   component does **not** rewrite the signal; it only clamps for its own
   rendering.

2a. The component resolves prop values uniformly with a helper:
    `read<T>(p: Signal<T> | T): T => isSignal(p) ? p.val : p`. Applied to
    `props.totalPages` and `props.disabled` inside the reactive block, so
    re-renders happen when either signal changes. Plain values are read
    once at render time and never re-trigger.

3. The visible page list is derived from `props.page.val`,
   `resolvedTotal`, `props.siblingCount` (default 1), and
   `props.boundaryCount` (default 1) by the following rule:
   - Compute the inclusive range `[max(1, page - sibling), min(totalPages, page + sibling)]` — the *sibling window*.
   - Compute the inclusive ranges `[1, boundaryCount]` and
     `[totalPages - boundaryCount + 1, totalPages]` — the *boundary blocks*.
   - Union these three ranges; sort ascending; deduplicate.
   - Where the resulting page list has a gap of 2 or more, insert a single
     ellipsis sentinel between the two flanking pages. (Adjacent boundary
     and sibling pages with no gap render no ellipsis.)
   - The result is an array of `number | "..."` items in left-to-right
     order, which the component maps to `<li>` elements.

4. Each page button has class `pagination-btn`, an `aria-label` of
   `"Page {n}"`, and — when `n === clamped page` — both
   `aria-current="page"` and the additional class `pagination-active`.
   `@click` calls `props.page.set(n)`; then if `props.onChange` is set,
   calls `props.onChange(n)`. No-op when the clicked page equals the
   current page.

5. The Prev button has class `pagination-btn pagination-prev`,
   `aria-label = props.prevLabel ?? "Previous"`, and is disabled when
   `clamped page <= 1` or the whole component is disabled. On click, calls
   `props.page.set(clamped page - 1)` and `props.onChange?.(clamped page - 1)`.

6. The Next button has class `pagination-btn pagination-next`,
   `aria-label = props.nextLabel ?? "Next"`, and is disabled when
   `clamped page >= totalPages` or the whole component is disabled. On
   click, calls `props.page.set(clamped page + 1)` and
   `props.onChange?.(clamped page + 1)`.

7. The outer `<nav>` carries `role="navigation"` and
   `aria-label="Pagination"`. Its class is
   `"pagination pagination-{size}"` plus `"pagination-disabled"` when the
   component is disabled. Size defaults to `"md"`.

8. The component is disabled (all buttons rendered with the native
   `disabled` attribute, outer class includes `pagination-disabled`) when
   either `read(props.disabled) === true` **or** `resolvedTotal <= 1`.
   Both terms re-evaluate inside the reactive block, so flipping a
   disabled signal mid-render updates the buttons without remount.

9. The ellipsis is rendered as a `<span class="pagination-ellipsis"
   aria-hidden="true">…</span>` wrapped in an `<li>`. It is not a button
   and has no click handler.

10. When `props.summary` is provided, the component renders a
    `<div class="pagination-summary">${props.summary(page.val, totalPages)}</div>`
    inside the reactive block (so the summary text updates as the page
    changes). When absent, no summary element is rendered.

11. Keyboard handling: each button is a native `<button>`, so Tab/Enter/Space
    work for free. No additional arrow-key handler in v1 — the plan
    confirms whether to add Left/Right arrow stepping or defer.

### CSS

12. New partial `.zero/styles/components/_pagination.scss`. Every rule sits
    inside `@layer components { ... }`.

13. Token-only values. Spacing from `--space-*`, radii from `--radius-*`,
    colors from `--color-*`, fonts from `--font-*`, borders from
    `--border-*`. No hex codes, no magic numbers except for
    `transition-duration`, `opacity`, and animation timings.

14. `.pagination` is a horizontal flex container with
    `gap: var(--space-sm)`. The `.pagination-list` is a flex row with
    `gap: var(--space-xs)`, `list-style: none`, `padding: 0`, `margin: 0`.

15. `.pagination-btn` is a button with `min-width` and `min-height` driven
    by the size class (sm/md/lg). Default `md`. Padding, radius, font-size,
    and font-weight come from tokens. Hover state uses an existing surface
    token (e.g. `--color-surface` against `--color-bg`) — the plan picks
    the exact pairing matching what `Button.ghost` already uses for hover.

16. `.pagination-active` swaps the background to `var(--color-primary)`
    and the text to `var(--color-primary-fg)`. Same treatment as
    `Button.primary`.

17. Size classes mirror Button's sizing:
    - `.pagination-sm .pagination-btn { min-width: ...; padding: var(--space-xs) var(--space-sm); font-size: var(--font-sm); }`
    - `.pagination-md .pagination-btn { ... var(--space-sm) var(--space-md); var(--font-md); }`
    - `.pagination-lg .pagination-btn { ... var(--space-md) var(--space-lg); var(--font-lg); }`
    Exact `min-width`/`min-height` numbers are picked by the plan to match
    Button's existing visual rhythm.

18. `.pagination-btn[disabled]` and `.pagination-disabled .pagination-btn`
    set `opacity: 0.5; cursor: not-allowed; pointer-events: none`.

19. `.pagination-ellipsis` has `padding: var(--space-xs) var(--space-sm)`,
    `color: var(--color-text-muted)`, `user-select: none`.

20. `.pagination-summary` uses `var(--color-text-muted)` and
    `var(--font-sm)` (or matches the active size class's font-size — the
    plan picks).

21. Logical properties only — no `left`/`right` in CSS rules that affect
    text direction (use `padding-inline-*`, `margin-inline-*`, etc. where
    relevant).

22. No `!important`. Override via the unlayered cascade per the existing
    `@layer components` convention.

### Scaffold registration

23. New `TPL_*` constants for:
    - `.zero/components/Pagination.ts`
    - `.zero/components/Pagination.test.ts`
    - `.zero/styles/components/_pagination.scss`

24. `framework_manifest()` gains three entries. Existing length-coupled
    assertions in `crates/zero-scaffold/src/lib.rs` tests and any
    `tests/update*.rs` paths that hard-code the manifest length are bumped
    accordingly. The plan enumerates the exact assertions.

25. `.zero/components/index.ts` gains one line:
    `export { default as Pagination } from "./Pagination.ts"`. Ordering
    follows the file's existing alphabetical-by-component convention
    (after `Input`, before `Radio`).

26. `.zero/components/index.ts` also re-exports the types:
    `export type { PaginationProps } from "./Pagination.ts"` (matches
    `Table`'s precedent of exporting `TableColumn`).

27. `.zero/styles/_components.scss` gains one line:
    `@use 'components/pagination';` inserted in alphabetical position
    (between `input` and `radio`).

28. `.zero/components.d.ts` (the module declaration for
    `"zero/components"`) gains a `Pagination` entry and a `PaginationProps`
    type export. Exact shape is for the plan.

29. The editor `tsconfig.json` emitted by `zero init` requires no changes —
    `"zero/components"` already resolves to `.zero/components/index.ts`,
    and Pagination rides on that resolution.

### Tests

30. New file `.zero/components/Pagination.test.ts` exercising:
    - **Renders the base markup.** With `page: signal(1), totalPages: 5`,
      `find(el, "nav.pagination")` succeeds; `find(el, ".pagination-prev")`
      and `find(el, ".pagination-next")` succeed; the rendered page-button
      labels (collected from `findAll(el, ".pagination-btn:not(.pagination-prev):not(.pagination-next)")`)
      form the array `["1","2","3","4","5"]`.
    - **Active page.** The button labeled `"1"` carries
      `aria-current="page"` and class `pagination-active`. After
      `page.set(3)`, the active class moves to the `"3"` button.
    - **Prev/Next clicks.** Clicking Next increments `page.val`. Clicking
      Prev decrements it. Both invoke an attached `spy()` `onChange`
      callback with the new page.
    - **Page-number click.** Clicking the `"4"` button sets `page.val` to
      4 and calls `onChange` with 4.
    - **Prev disabled at start, Next disabled at end.** With `page=1`,
      `.pagination-prev[disabled]` is in the DOM and no-ops on click. With
      `page=totalPages`, same for Next.
    - **Ellipsis.** With `totalPages=20, page=10, siblingCount=1, boundaryCount=1`,
      `findAll(el, ".pagination-ellipsis").length === 2`. The visible
      page-button labels are `["1","9","10","11","20"]`.
    - **No ellipsis when not needed.** With `totalPages=5`, no
      `.pagination-ellipsis` is rendered.
    - **Disabled when totalPages <= 1.** With `totalPages: 1`, the outer
      `nav` carries `pagination-disabled`; every `.pagination-btn` has the
      `disabled` attribute; clicking Next is a no-op.
    - **Manual `disabled: true` (plain).** With `totalPages: 10, disabled: true`,
      same disabled treatment as the single-page case.
    - **Reactive `disabled: Signal<boolean>`.** With
      `totalPages: 10, disabled: signal(false)`, the pager is enabled.
      After `disabled.set(true)`, the outer `nav` carries
      `pagination-disabled` and all buttons gain the `disabled` attribute
      without remount.
    - **Reactive `totalPages: Signal<number>`.** With
      `totalPages: signal(3), page: signal(1)`, the rendered page-button
      labels are `["1","2","3"]`. After `totalPages.set(5)`, the labels
      become `["1","2","3","4","5"]` without remount. Active-page,
      Prev/Next disabled states, and ellipsis math all re-evaluate
      against the new total.
    - **Out-of-range page clamps for rendering.** With `page: signal(0)`,
      rendering proceeds as if page were 1 (active button is `"1"`); the
      signal value is **not** rewritten by the component.
    - **Size variant.** With `size: "sm"`, outer `nav` carries
      `pagination-sm`. Default carries `pagination-md`.
    - **Summary slot.** Passing `summary: (p, t) => \`Page ${p} of ${t}\``
      renders `.pagination-summary` with the formatted text. After
      `page.set(2)`, the summary text updates.
    - `afterEach(cleanup)`.

31. The new test ships in the manifest (lives in `.zero/components/`) and
    runs:
    - Inside the framework's `showcase/` via `tests/component_library.rs`.
    - Inside every user project's `zero test` (consistent with the
      existing component-test ship-along policy from Phase 9).

### Showcase

32. New route file `showcase/src/routes/pagination.ts` rendering:
    - A standalone `Pagination` with `totalPages: 12` (plain) and
      `page = signal(1)`, showing the default md size and the canonical
      ellipsis behavior as the user navigates.
    - A second instance with `size: "sm"` and `totalPages: 20` (plain).
    - A third instance with `size: "lg"`, `totalPages: 5` (no ellipsis
      branch), and a `summary` slot showing `Page X of Y`.
    - A fourth instance with `totalPages: 1` to demonstrate the
      auto-disabled state.
    - A fifth "Async" instance demonstrating signal-typed `totalPages`
      and `disabled`: `totalPages = signal(1)` and `busy = signal(false)`.
      A button labeled "Simulate load" sets `busy.set(true)`, schedules a
      `setTimeout(..., 600)`, and on completion sets `totalPages` to a
      random number in `[5, 25]` and `busy.set(false)`. Demonstrates that
      both the pager's button list and the disabled state update
      reactively without remount. This instance carries no real backend
      — the comment in the source explicitly notes that a real app
      would call its own fetch logic here.
    - Each instance shows the current `page.val` below the pager via a
      reactive text binding for clarity.

33. `showcase/src/app.ts` registers
    `app.route("/pagination", () => import("./routes/pagination"))`.
    Position consistent with existing alphabetical ordering of routes.

34. `showcase/src/routes/home.ts` navigation cluster gains a `Pagination`
    link.

35. `showcase/src/routes/table.ts` gains a fourth Table instance that
    demonstrates Table paired with Pagination (client-side / static
    dataset):
    - `pageSize` is a local constant (e.g. `3`).
    - `page = signal(1)`.
    - `pagedRows = computed(() => sample.slice((page.val - 1) * pageSize, page.val * pageSize))` —
      a derived signal feeding the Table's `rows`.
    - `totalPages = Math.ceil(sample.length / pageSize)` (plain number).
    - A `<section>` titled "Paginated" renders the Table followed by the
      Pagination, with a `summary` slot showing "Showing X–Y of Z".
    - Demonstrates: (a) Table doesn't need a built-in pagination prop,
      (b) the parent-owns-derivation pattern is concise.

35a. `showcase/src/routes/table.ts` **also** gains a fifth Table instance
     demonstrating the async / server-style pattern (mocked, since the
     showcase has no backend):
     - `page = signal(1)`, `totalPages = signal(1)`, `rows = signal<User[]>([])`,
       `busy = signal(false)`.
     - An `effect(() => { ... })` reads `page.val`, sets `busy.set(true)`,
       calls a local `fakeFetch(page)` that resolves a `setTimeout(...)`-
       backed promise with `{ rows, totalPages }` derived from the local
       `sample` array, then writes the three result signals and clears
       `busy`.
     - Table receives `rows` and `loading: busy`. Pagination receives
       `page`, `totalPages`, and `disabled: busy`.
     - A comment block in the source explicitly notes: "Replace `fakeFetch`
       with whatever real backend call your app uses — fetch, createHttp,
       GraphQL, etc. Pagination doesn't care."
     - Demonstrates the canonical async wiring without committing the
       framework to any particular backend shape.

36. The showcase's committed `.zero/components/Pagination.ts` (and partial
    + test) matches the manifest. `zero update --yes` from inside
    `showcase/` produces zero drift.

### Integration tests

37. `tests/showcase_build.rs` continues to pass against the new route. The
    plan verifies whether any per-route assertion exists that needs
    widening.

38. `tests/showcase_dev.rs` continues to pass.

39. `tests/component_library.rs` continues to pass and now includes
    `Pagination.test.ts` in its run. The plan verifies whether the test
    asserts a specific test count that needs bumping.

### Documentation

40. `crates/zero-scaffold/src/scaffold/AGENTS.md` `## Components` section
    gains a `Pagination` entry in the component-roster table. The relevant
    category subsection (likely "Navigation" or a new one — the plan
    judges; could attach to the same group as `Tabs`) gains a one-instance
    usage example.

41. `docs/components.md` is updated:
    - The component-count language ("fifteen components") is bumped to
      "sixteen components".
    - The summary table gains a new row for `Pagination` with its required
      props and an example, in alphabetical position (after `Input`,
      before `Radio`).

42. `zero-framework-spec.md` §11 — `"zero/components"` listing gains a
    `Pagination({...})` line in the same group as `Tabs` (or a new
    `// Navigation` group — plan picks).

43. `zero-framework-spec.md` §12 — Phase-9 component-count line is bumped,
    and the parenthetical list adds `Pagination`.

44. `BEST_PRACTICES.md` (or `docs/best-practices.md` — plan picks the
    right surface) gains a short subsection on the Table-with-Pagination
    idiom covering **both** the static (derived-slice computed) and the
    async (effect on `page.val` calling user's own fetch logic) patterns.
    The async example explicitly uses a placeholder `myBackend.getUsers(p)`
    so readers don't mistake it for prescribed plumbing. Add the
    subsection only if the plan judges the patterns need pinning; a
    throwaway example does not justify a section.

## Constraints

- **No new Rust dependencies.** Rides on the existing `grass` SCSS
  pipeline, the existing transpiler, the existing scaffold + manifest
  plumbing.
- **No new npm dependencies.** Framework-wide.
- **No new top-level `"zero"` runtime exports.** Pagination is exposed
  only via `"zero/components"`.
- **`@layer components` for all CSS rules.** Unlayered user CSS in
  `styles/app.scss` overrides without `!important`.
- **Tokens only — no magic numbers or hex codes.** Standard exceptions
  for `opacity`, `transition-duration`, and animation timings. Sizing
  literals (`min-width`/`min-height`) on `.pagination-btn` are permitted
  if they match what `Button` already uses; the plan reuses Button's
  values rather than introducing new ones.
- **Stateful prop is a signal.** `page: Signal<number>`. Two
  configuration props accept signal-or-plain so async parents can update
  them without remount: `totalPages: Signal<number> | number` and
  `disabled?: Signal<boolean> | boolean`. The remaining props (`size`,
  `siblingCount`, `boundaryCount`, `summary`, `onChange`, `prevLabel`,
  `nextLabel`) are plain values read once at mount.
- **No backend awareness.** Pagination does not fetch, does not import
  from `"zero/http"`, does not assume a response shape, does not assume
  REST/GraphQL/anything. Parent does all I/O; Pagination only exposes
  reactive seams.
- **Component is controlled, not headless.** Pagination always renders a
  styled pager; it does not expose a headless / render-prop API for
  custom markup.
- **Pages are 1-indexed.** No `zeroIndexed?: boolean` escape hatch.
- **No internal state.** The component holds no mutable state beyond what
  it derives from `props.page.val` + `props.totalPages` at render time.
- **Decoupled from Table.** No import of `Table` from
  `Pagination.ts`. The Table route in the showcase wires them together via
  a derived signal in user-land; the framework itself does not know they
  go together.
- **No web components, no scoped styles, no CSS-in-JS.** Framework-wide.
- **One styled form.** No headless variant, no unstyled "primitive"
  Pagination. Users needing a different look fork into `src/components/`.
- **Framework-owned.** Lives under `.zero/`. `zero update` refreshes it.

## Out of Scope

- **Uncontrolled mode.** No `defaultPage?: number` for internal-state
  pagination. Always controlled via `page: Signal<number>`.
- **Page-size selector.** No `pageSize: Signal<number>` /
  `pageSizeOptions: number[]` props. Users render a `Select` next to the
  pager themselves if they want a size picker.
- **Total-items + pageSize state model.** The component does not accept
  `total` or `pageSize`. Parent computes `totalPages` and passes it in.
  Summary text is the parent's job via the `summary` slot.
- **Built-in fetch / data-source helpers.** No `createPagedSource()`, no
  `usePagedFetch()`, no `Pagination.fetch` adapter. The component never
  prescribes how the parent loads data. Documentation and showcase
  demonstrate the wiring pattern; the framework ships no helper.
- **Auto-abort on page change.** Pagination does not own an
  `AbortController` and does not abort in-flight requests when `page`
  changes. The parent's `effect()` (or whatever pattern they use) is
  responsible for cancellation if they want it. The route-scoped fetch
  from `load()` already covers the navigation-away case; mid-route
  pagination is the parent's call.
- **Jump-to-page input.** No editable page-number input. Users build that
  themselves if they need it.
- **First/Last buttons.** No `« 1` / `» Last`. Users click the boundary
  numbers (which always render via `boundaryCount`).
- **Auto-hide when single page.** Component renders disabled instead of
  hidden. Predictable layout beats clever DOM removal.
- **Arrow-key navigation.** Plan may add Left/Right arrow handling on the
  outer `<nav>` if cheap; otherwise deferred. Tab/Enter/Space work for
  free via native `<button>`.
- **`aria-live` announcements for page changes.** Out of scope; future
  a11y polish.
- **Keyboard shortcut hints in the UI (e.g. "PgDn for next").** Out of
  scope.
- **Persistence of page across navigation / reload.** Parent's job (URL
  path params, URL search params, localStorage, etc.) — Pagination just
  reads/writes its signal. The Background section shows the canonical
  URL-sync wiring (`effect()` on `page.val` calling `navigate()`, plus
  `load({ params })` restoring on initial nav) as a non-prescriptive
  illustration; the framework ships no helper for it.
- **Snapshot tests.** `expect().toMatchSnapshot()` is not implemented in
  `zero/test`. Tests assert on DOM selectors and signal values.
- **A standalone Pagination package.** No npm publication.

## Open Questions

- **Default `siblingCount` and `boundaryCount`.** Spec proposes `1` and
  `1`. Plan confirms — `2` and `1` is also a common default (current ± 2)
  but produces wider pagers.
- **Whether `siblingCount` / `boundaryCount` should be props at all in
  v1.** Hardcoding `siblingCount=1, boundaryCount=1` reduces API surface
  by two props. Plan judges whether the configurability earns its keep or
  should be deferred.
- **Arrow-key navigation.** Add Left/Right arrow stepping on the outer
  `<nav>` (focus management, capturing keys only when a pager button has
  focus)? Or defer to a future a11y-polish issue? Plan judges from the
  cost.
- **Prev/Next visible label vs icon.** Spec renders `‹` and `›`
  characters. Should `prevLabel` / `nextLabel` also affect visible text,
  not just `aria-label`? Plan picks. (Cheap variant: always show the
  glyph, use `prevLabel`/`nextLabel` only for `aria-label`. Richer
  variant: show the label visibly at `lg` size, glyph-only at `sm`/`md`.)
- **Summary placement.** Spec puts `.pagination-summary` inside the
  `<nav>` before the `<ul>`. Alternative: render it via a slot the parent
  positions themselves. Spec position is simplest. Plan confirms.
- **Hover token reuse.** Plan picks the exact token pairing for hover
  background to match `Button.ghost`'s hover treatment without
  duplicating values.
- **`min-width` / `min-height` values for `.pagination-btn`.** Plan picks
  by inspecting `Button`'s current sizing to keep visual rhythm
  consistent.
- **Page-button `aria-label` format.** Spec uses `"Page {n}"`. Locale?
  Plan can defer to existing locale conventions (English) or expose an
  optional `pageLabel?: (n: number) => string` slot. Spec proposes the
  hardcoded English form; plan judges.
- **Whether the component clamps `page.val` for rendering but does NOT
  rewrite the signal (Requirement 2), or whether it should call
  `page.set(clamped)` to repair invalid input.** Spec proposes
  render-only clamp to avoid surprising the parent. Plan confirms.
- **`onChange` semantics.** Spec calls `onChange` after `page.set()`.
  Should it ever be called when the click is a no-op (e.g. clicking the
  active page)? Spec says no. Plan confirms.
- **Whether `Pagination` exports its `PaginationProps` from
  `index.ts`.** Spec says yes (matches `TableColumn`'s precedent). Plan
  confirms.
- **Manifest size assertion.** Plan confirms the exact current number
  after the Table addition and bumps the test by three.
- **Showcase route — four instances or fewer.** Spec proposes four
  (default, sm, lg+summary, single-page). Plan can simplify if the
  default + summary + single-page combo is enough.
- **AGENTS.md / docs grouping.** Where Pagination sits in the
  component-category taxonomy (Navigation? Forms? Display?). Plan picks;
  Navigation alongside `Tabs` is the natural fit.
