# Spec: Pagination accepts `Computed<number>` for `totalPages`

## Problem Statement

`Pagination`'s `totalPages` prop is typed `Signal<number> | number` (`crates/zero-scaffold/src/scaffold/.zero/components/Pagination.ts:8`). The internal `isSignal` duck-type (lines 27-34) requires the value to have **both** a `.val` getter **and** a `.set` function:

```ts
function isSignal<T>(p: Signal<T> | T): p is Signal<T> {
  return (
    typeof p === "object" &&
    p !== null &&
    "val" in p &&
    typeof (p as { set?: unknown }).set === "function"
  );
}
```

`Computed<T>` has `.val` but no `.set` (see `runtime/zero.d.ts:11-13`). So passing a `Computed<number>` to `totalPages` — the natural shape when total pages is derived (e.g. `computed(() => Math.ceil(totalCount.val / pageSize))`) — causes `isSignal` to return `false`, `read()` returns the `Computed` object itself, and `Math.max(1, computedObject)` collapses to `1`. The component appears broken in a way that's easy to misdiagnose: the page list shows a single "1" forever, no error is thrown, and the parent has to materialize the computed value into a plain number snapshot inside a reactive template slot to work around it.

Caught in the demo's friction log (`~/Documents/code/zero_demo/FRAMEWORK_NOTES.md:37`, severity 🟡). The same pattern is **duplicated verbatim in `Combobox.ts:34`**, where `disabled?: Signal<boolean> | boolean` has the identical bug — `Computed<boolean>` disables nothing. Both files copy the duck-type definition; the fix has to land in both and the pattern has to be unduplicated so the next component that needs "signal or plain" doesn't reintroduce it.

## Background

### Why `Computed` exists separately

`Signal<T>` (read + write) and `Computed<T>` (read-only, derived) are distinct interfaces in `runtime/zero.d.ts:5-13`. The public reactivity API treats them as siblings:

- `signal<T>(initial: T): Signal<T>` — read-write cell.
- `computed<T>(fn: () => T): Computed<T>` — read-only derived value, re-computed when dependencies change.
- `each(source: Signal<T[]> | Computed<T[]>, …)` — the framework's own list primitive already accepts either, signalling that "reactive read-only-or-read-write" is the canonical shape.

A component prop that says "give me a number, or something I can read a number from reactively" must accept both. Today, every shipped component that follows the `Signal<T> | T` pattern silently rejects `Computed<T>`. This is a runtime bug, not a type-system gap — TypeScript would flag the bad assignment if the prop types listed `Computed<T>` explicitly, but the prop types don't.

### Where the duplicated duck-type lives

Two copies today:

- `crates/zero-scaffold/src/scaffold/.zero/components/Pagination.ts:27-46` — `isSignal` + `read`. Used on `totalPages` (line 121: `read(props.totalPages)`) and `disabled` (line 128: `read(props.disabled)`).
- `crates/zero-scaffold/src/scaffold/.zero/components/Combobox.ts:34-53` — identical `isSignal` + `read`. Used on `disabled`.

Every other component either reads stateful props as signals directly (`props.value.val`) or never branches on "is this a signal." So the duck-type only matters in these two files today, but the pattern will recur the next time someone writes a prop that's "signal-or-plain."

### Component conventions the fix must respect

From `issues/pagination/spec.md` and `issues/table/spec.md`:

- Stateful props are signals; structural props are plain values. The `Signal<T> | T` shape is an *escape hatch* used when "either is reasonable" (`totalPages` may be derived or a literal; `disabled` may be reactive or a constant).
- Components are plain functions returning `TemplateResult`. No new framework primitives needed for this fix — just a helper module.
- Each shipped component has a per-file SCSS partial and a `*.test.ts` neighbor; the scaffold manifest (`crates/zero-scaffold/src/lib.rs`) tracks every framework-owned file with a `TPL_*` constant.

### Adjacent surfaces

- **`crates/zero-scaffold/src/scaffold/.zero/components/Pagination.ts`** — remove duplicated helpers, import shared ones, update `totalPages` and `disabled` prop types.
- **`crates/zero-scaffold/src/scaffold/.zero/components/Combobox.ts`** — same.
- **New: `crates/zero-scaffold/src/scaffold/.zero/components/_internal.ts`** — the shared helpers' home. Filename prefix `_` marks it as internal-to-the-components-dir; not re-exported via `index.ts`.
- **`crates/zero-scaffold/src/lib.rs`** — add `TPL_INTERNAL_TS = include_str!("scaffold/.zero/components/_internal.ts");` and a `FrameworkFile` entry in `framework_manifest()` so the file ships with every scaffold.
- **`crates/zero-scaffold/src/scaffold/.zero/components/index.ts`** — verify the import-`_internal`-don't-re-export shape. If `index.ts` is "re-export everything in this directory," adjust so leading-underscore filenames are skipped.
- **`crates/zero-scaffold/src/scaffold/.zero/components.d.ts`** — if it has a hand-maintained re-export, no edit needed (internal helpers aren't a public type). Verify during planning.
- **Tests:** existing `Pagination.test.ts` and `Combobox.test.ts` keep passing; new test cases cover the `Computed` path.

## Requirements

### R1 — Shared `isReactive` + `read` in `_internal.ts`

Create `crates/zero-scaffold/src/scaffold/.zero/components/_internal.ts` exporting:

```ts
import type { Signal, Computed } from "zero";

/**
 * Either a writable signal or a read-only computed of the same value.
 * Component props that say "reactive or plain" should accept this plus `T`.
 */
export type Reactive<T> = Signal<T> | Computed<T>;

export function isReactive<T>(p: Reactive<T> | T): p is Reactive<T> { … }

export function read<T>(p: Reactive<T> | T): T { … }
```

The duck-type checks for `.val` only — **not** `.set`:

```ts
return typeof p === "object" && p !== null && "val" in p;
```

`read` returns `p.val` when `isReactive(p)` is true, else returns `p` unchanged. The functions are typed precisely enough that consumers don't need a cast at the call site.

Both helpers carry `@internal` JSDoc per CLAUDE.md (the components directory's `index.ts` is the public API surface; this file is implementation detail).

### R2 — `Pagination.ts` uses shared helpers, prop types accept `Computed`

`crates/zero-scaffold/src/scaffold/.zero/components/Pagination.ts`:

- Delete the local `isSignal` and `read` definitions (lines 19-46).
- Import `isReactive`, `read`, `Reactive` from `./_internal.ts`.
- Update prop types:
  - `totalPages: Reactive<number> | number` (was `Signal<number> | number`).
  - `disabled?: Reactive<boolean> | boolean` (was `Signal<boolean> | boolean`).
- Call sites (`read(props.totalPages)` at line 121, `read(props.disabled)` at line 128) are unchanged because the new `read` has the same signature.
- The JSDoc on the component itself updates to mention `Computed` is accepted everywhere a signal is accepted.

After R2, `Pagination({ totalPages: computed(() => Math.ceil(total.val / size)), page })` works as the demo's friction-log entry expected.

### R3 — `Combobox.ts` uses shared helpers, `disabled` accepts `Computed`

`crates/zero-scaffold/src/scaffold/.zero/components/Combobox.ts`:

- Delete the local `isSignal` and `read` definitions (lines 26-53).
- Import the same three names from `./_internal.ts`.
- Update `disabled?: Signal<boolean> | boolean` → `disabled?: Reactive<boolean> | boolean`.
- Existing call site (`read(props.disabled)`) is unchanged.

No behavior change for users who pass a signal or a plain boolean today; `Computed<boolean>` newly works.

### R4 — Scaffold manifest ships `_internal.ts`

`crates/zero-scaffold/src/lib.rs`:

- Add a `TPL_INTERNAL_TS = include_str!("scaffold/.zero/components/_internal.ts");` constant alongside the other component template constants (near `TPL_AVATAR_TS` etc., kept alphabetically sorted by name).
- Add a `FrameworkFile` entry in `framework_manifest()` so the file is materialized into every new project and updated on `zero update`.
- The `.zero/components/index.ts` template should not re-export `_internal.ts` — verify whether `index.ts` is a hand-maintained list (likely yes) or generated. If hand-maintained, leave it alone. If generated, exclude leading-underscore filenames.

### R5 — Tests

`crates/zero-scaffold/src/scaffold/.zero/components/Pagination.test.ts` gains:

- `accepts_computed_total_pages` — render with `totalPages: computed(() => 5)`, assert the rendered page list shows pages 1-5 (or however the existing tests assert page-count visibility).
- `recomputes_when_computed_total_changes` — total pages derived from a signal; flip the signal, assert the pagination's page list updates to reflect the new total. Covers the "reactive" guarantee, not just "not-1".

`crates/zero-scaffold/src/scaffold/.zero/components/Combobox.test.ts` gains:

- `accepts_computed_disabled` — pass `disabled: computed(() => guard.val)`, flip `guard`, assert the input's `disabled` attribute toggles.

New file `crates/zero-scaffold/src/scaffold/.zero/components/_internal.test.ts` covers `isReactive` + `read` directly:

- Plain numbers, strings, objects without `.val` → `isReactive` false, `read` returns the value.
- `signal(5)` → `isReactive` true, `read` returns `5`.
- `computed(() => 7)` → `isReactive` true, `read` returns `7`.
- `null` and `undefined` → `isReactive` false, `read` returns the value (no crash on null).

Manifest tests (if any in `crates/zero-scaffold/`) get extended to assert `_internal.ts` is present in the framework file list.

### R6 — Docs

`docs/components.md` (or wherever components are referenced — verify during planning):

- One-line note on Pagination: "`totalPages` and `disabled` accept a `Signal`, a `Computed`, or a plain value."
- Same for Combobox `disabled`.
- If the docs have a "writing your own components" section that demonstrates the `Signal<T> | T` pattern, point readers at `_internal.ts`'s `isReactive`/`read` or document the duck-type requirement (`.val` only) so user-authored components don't re-introduce the bug.

`runtime/zero.d.ts` — no change required. `Signal` and `Computed` are already exported.

## Constraints

- No new npm dependencies; the helper is plain TypeScript over the existing `zero` runtime types.
- `_internal.ts` is the only new file. The fix must not introduce a new public export surface — no top-level `zero/components` re-export of `isReactive`/`read`/`Reactive`. Users who want this can import from `"zero/components/_internal"` if absolutely needed, but it's deliberately not a documented entry point.
- `Reactive<T>` lives in `_internal.ts`, not `runtime/zero.d.ts`. The runtime types stay minimal; the helper type is a convenience for component-author ergonomics, not a runtime primitive.
- Backwards compatibility: every existing call site that passes `signal(x)` or a plain value to `totalPages` or `disabled` keeps working. The change is additive — the new accepted shape is `Computed<T>`.
- The 80-line per-function guideline applies; both new helpers are one-liners.
- `_internal.ts` must not import from `"./Pagination.ts"` etc. — the dependency direction is one-way: component files import from `_internal.ts`, never the reverse.

## Out of Scope

- **Auditing every shipped component for the same bug shape.** Only Pagination and Combobox use the `Signal<T> | T` escape hatch today (grepped: those are the only files with a local `isSignal` definition). If a future audit surfaces more, that's a follow-up. The shared helper is the future-proofing.
- **Exporting `isReactive`/`read`/`Reactive` from `"zero"` or `"zero/components"`.** Internal-only. User components that want the same ergonomics can copy-paste five lines or import from the underscore path.
- **A general `Reactive<T>` type in `runtime/zero.d.ts`.** Out of scope for this fix; if the framework wants a public `Reactive` union later, that's its own decision.
- **Changing `each(Signal<T[]> | Computed<T[]>, …)`'s signature** to use a `Reactive` alias. Cosmetic, no behavior change, separate issue.
- **Reactive `siblingCount` / `boundaryCount` / `prevLabel` / `nextLabel` on Pagination.** These remain plain values. The bug is about props that today are typed `Signal<T> | T`; expanding the reactive-prop surface is a different feature decision.
- **Anything Table-related.** Table sort is a separate spec (`issues/table-sort/`).

## Open Questions

- **Filename / path for the helper.** Spec recommends `crates/zero-scaffold/src/scaffold/.zero/components/_internal.ts`. Alternatives the planner can consider: `utils.ts` (no leading underscore, plainer), or `reactive.ts` (descriptive of contents). The leading-underscore convention is borrowed from SCSS partials in the same scaffold; if TypeScript / module resolution treats `_*.ts` files quirky in any of the toolchains (transpile, bundler, lint), planner picks a non-underscore name and the rest of the spec applies unchanged.
- **`index.ts` regeneration.** If `.zero/components/index.ts` is hand-edited, no change. If generated by `zero update` or a similar tool, the generator must skip `_internal.ts`. Planner reads `index.ts` first and decides.
- **Should the duck-type also accept a `peek` method as evidence of reactivity?** `Signal`/`Computed` don't currently expose `peek`, so the answer today is no. If a future API adds `peek`, the duck-type stays correct because `.val` is still present. Noting for the record; no action.
- **`Reactive<T>` placement.** Inside `_internal.ts` (recommended), or `runtime/zero.d.ts` so it's a public type? Spec keeps it internal; planner re-confirms based on whether any user-facing doc would benefit from the alias.
