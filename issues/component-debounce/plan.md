# Plan: `debounceMs` prop on signal-writing components

## Summary

Add an opt-in `debounceMs?: number` prop to the six signal-writing
components in the scaffold component library — `Input`, `TextArea`,
`Checkbox`, `Toggle`, `Radio`, `Select` — so consumers can delay the
signal write from JS without hand-rolling a `setTimeout` in an
`effect()`. The mechanism is a small components-only `debounce<T>(fn,
ms)` helper added to the already-private `_internal.ts`; each
component imports it and wraps its existing event handler with
`debounce(handler, props.debounceMs ?? 0)`. The helper returns `fn`
unchanged when `ms <= 0`, so every callsite passes `props.debounceMs
?? 0` without branching and existing callers (no prop) keep today's
synchronous behaviour. Visible-value bindings are untouched: the
native control reflects input/clicks immediately; only the signal
write is deferred. No public API, no template-modifier changes, no
Combobox refactor.

## Prerequisites

None blocking. Two notes carried from the spec's Open Questions,
both already resolved in favour of the recommendation:

- **Scope is all six components** (wide ship), not just
  `Input`/`TextArea`. The click-driven semantics caveat is handled in
  the docs step.
- **Default is `0`** (synchronous / opt-in), not a non-zero default.

One out-of-repo item: Requirement #7 (friction-log fix annotation in
`zero_demo/FRAMEWORK_NOTES.md`) targets a file that does **not** exist
in this repository — it lives in the separate `zero_demo` project. It
cannot be executed here. See Risks and Assumptions; it is excluded
from the steps below.

## Steps

- [x] **Step 1: Add the `debounce` helper to `_internal.ts` + unit tests**
- [x] **Step 2: `Input` — `debounceMs` prop, wrapped handler, tests**
- [x] **Step 3: `TextArea` — `debounceMs` prop, wrapped handler, tests**
- [x] **Step 4: Click-driven four (`Checkbox`, `Toggle`, `Radio`, `Select`) — `debounceMs` prop, wrapped handlers, tests**
- [x] **Step 5: Documentation in `docs/components.md`**

---

## Step Details

### Step 1: Add the `debounce` helper to `_internal.ts` + unit tests

**Goal:** Land the shared trailing-edge debounce utility that every
following step depends on, with its own unit coverage. Done first so
the component steps can import a tested helper.

**Files:**
- `crates/zero-scaffold/src/scaffold/.zero/components/_internal.ts` (modify)
- `crates/zero-scaffold/src/scaffold/.zero/components/_internal.test.ts` (modify — append)

**Changes:**
- In `_internal.ts`, append the exported function exactly as
  specified in the spec's Background section:

  ```ts
  /**
   * Wrap `fn` so that successive invocations within `ms` reset a
   * trailing-edge timer. Same shape as the framework's
   * `@event.debounce:<ms>` template modifier, lifted into a
   * components-only helper so component implementations can apply
   * it from JS when the modifier route isn't available.
   *
   * Returns the wrapped function unchanged when `ms <= 0` so a
   * caller can pass `props.debounceMs ?? 0` without branching.
   *
   * @template T
   * @param fn Handler to wrap.
   * @param ms Trailing-edge debounce window in milliseconds; `<= 0` is a no-op.
   * @returns The wrapped handler, or `fn` itself when `ms <= 0`.
   * @internal
   */
  export function debounce<T extends (...args: any[]) => void>(
    fn: T,
    ms: number,
  ): T {
    if (!(ms > 0)) return fn;
    let timer: ReturnType<typeof setTimeout> | null = null;
    return ((...args: Parameters<T>) => {
      if (timer != null) clearTimeout(timer);
      timer = setTimeout(() => fn(...args), ms);
    }) as T;
  }
  ```

  Note the full `@template` / `@param` / `@returns` annotation to
  satisfy the repo's TS JSDoc rule (the existing `read` / `isReactive`
  entries are the pattern to match). `T extends (...args: any[]) =>
  void` and the `any[]` in the constraint are intentional and
  consistent with the existing liberal-generic style in this file.
- Do **not** export `debounce` from `index.ts`. `_internal.ts` stays
  private (constraint: no new public API).

**Tests:** Append a `describe("debounce", ...)` block to
`_internal.test.ts`, importing `debounce` from `./_internal.ts`:
- `debounce(fn, 0)` returns the same function reference
  (`expect(debounce(fn, 0)).toBe(fn)`).
- `debounce(fn, -5)` returns the same function reference (defensive
  early return).
- `debounce(fn, 50)` delays the call: assert `fn` not called
  synchronously, then `await new Promise(r => setTimeout(r, 80))` and
  assert it was called once.
- Multiple calls within the window collapse to one trailing call with
  the **last** args: call the wrapped fn three times with distinct
  args, wait `> 50 ms`, assert called once with the last args.

  Use a simple call-recording closure (a `let calls: unknown[][] =
  []` plus a `fn` that pushes its args) — the existing test file uses
  no mocking library, so don't introduce one.

---

### Step 2: `Input` — `debounceMs` prop, wrapped handler, tests

**Goal:** Apply the helper to the canonical keystroke-driven
component and prove debounce + collapse behaviour end to end.

**Files:**
- `crates/zero-scaffold/src/scaffold/.zero/components/Input.ts` (modify)
- `crates/zero-scaffold/src/scaffold/.zero/components/Input.test.ts` (modify)

**Changes:**
- Add `debounceMs?: number;` to `InputProps` with JSDoc on the prop:
  "Optional debounce window in milliseconds for the `value` signal
  write. `0` or omitted means synchronous (current behaviour)."
- Add `import { debounce } from "./_internal.ts";` (the file currently
  imports only from `"zero"`).
- After the existing `onInput` definition, add:
  `const handler = debounce(onInput, props.debounceMs ?? 0);`
- Change the template binding from `@input=${onInput}` to
  `@input=${handler}`.
- Leave `value=${() => props.value.val}` and everything else
  unchanged.

**Tests:** In `Input.test.ts`, add:
- **honours `debounceMs`:** render `Input({ value, debounceMs: 50 })`;
  fire `input` with `{ target: { value: "hello" } }`; synchronously
  assert `value.val === ""` (not yet written); `await new Promise(r
  => setTimeout(r, 80))`; assert `value.val === "hello"`.
- **successive events within window collapse to one write:** render
  with `debounceMs: 50`; fire `input` three times with values `"a"`,
  `"ab"`, `"abc"` back to back; `await` past the window; assert
  `value.val === "abc"` (only the last value landed).
- **without `debounceMs` writes synchronously (regression):** the
  existing "updates its signal on input events" test already covers
  the no-prop case; optionally pass `debounceMs: 0` explicitly in a
  small added assertion or a new tiny test confirming the synchronous
  write still happens with `debounceMs: 0`. Keep it lean — one extra
  `it` at most.

---

### Step 3: `TextArea` — `debounceMs` prop, wrapped handler, tests

**Goal:** Same treatment as `Input` for the other keystroke-driven
component.

**Files:**
- `crates/zero-scaffold/src/scaffold/.zero/components/TextArea.ts` (modify)
- `crates/zero-scaffold/src/scaffold/.zero/components/TextArea.test.ts` (modify)

**Changes:**
- Add `debounceMs?: number;` to `TextAreaProps` with the same prop
  JSDoc wording as `Input`.
- Add `import { debounce } from "./_internal.ts";`.
- After `onInput`, add `const handler = debounce(onInput,
  props.debounceMs ?? 0);`.
- Change `@input=${onInput}` to `@input=${handler}`. Leave the
  `<textarea>` content binding `${() => props.value.val}` unchanged.

**Tests:** In `TextArea.test.ts`, add the "honours `debounceMs`"
test mirroring Step 2 (fire `input` on the `textarea` element with a
test value, assert not-yet-written synchronously, `await ~80 ms`,
assert written). The collapse test is Input-only per the spec; not
required here.

---

### Step 4: Click-driven four (`Checkbox`, `Toggle`, `Radio`, `Select`) — `debounceMs` prop, wrapped handlers, tests

**Goal:** Uniform coverage of the signal-writing surface. These four
share an identical mechanical change (wrap the `@change` handler), so
they're grouped into one step; the codebase stays compilable and
green after it.

**Files:**
- `crates/zero-scaffold/src/scaffold/.zero/components/Checkbox.ts` (modify)
- `crates/zero-scaffold/src/scaffold/.zero/components/Toggle.ts` (modify)
- `crates/zero-scaffold/src/scaffold/.zero/components/Radio.ts` (modify)
- `crates/zero-scaffold/src/scaffold/.zero/components/Select.ts` (modify)
- `crates/zero-scaffold/src/scaffold/.zero/components/Checkbox.test.ts` (modify)
- `crates/zero-scaffold/src/scaffold/.zero/components/Toggle.test.ts` (modify)
- `crates/zero-scaffold/src/scaffold/.zero/components/Radio.test.ts` (modify)
- `crates/zero-scaffold/src/scaffold/.zero/components/Select.test.ts` (modify)

**Changes (per component):**
- Add `debounceMs?: number;` to the props type
  (`CheckboxProps`, `ToggleProps`, `RadioProps`, `SelectProps`) with
  prop JSDoc adapted to name the relevant signal: for `Checkbox` and
  `Toggle` the `checked` signal, for `Radio` the `selected` signal,
  for `Select` the `value` signal. Wording template: "Optional
  debounce window in milliseconds for the `<signal>` signal write.
  `0` or omitted means synchronous (current behaviour)."
- Add `import { debounce } from "./_internal.ts";`.
- Wrap the existing `onChange`:
  `const handler = debounce(onChange, props.debounceMs ?? 0);`.
- Change `@change=${onChange}` to `@change=${handler}` in the
  template. Leave all visible-state bindings unchanged
  (`checked=${() => checked.val}`, `aria-checked`, option
  `selected=${...}`, radio `checked=${...}`).

**Tests (per component):** Add an "honours `debounceMs`" test (~6
lines each, per the spec's full-coverage recommendation):
- Render with `debounceMs: 50` and the same props the existing
  "flips/updates" test uses.
- Fire `change` on the relevant element (`input` for Checkbox/Toggle/
  Radio; `select` with `{ target: { value } }` for Select).
- Synchronously assert the signal is **still** its initial value.
- `await new Promise(r => setTimeout(r, 80))`.
- Assert the signal now holds the changed value.
- For `Radio`, mirror the existing two-radio group setup but add
  `debounceMs: 50` to the radio whose change is fired.
- **`Checkbox` synchronous regression:** per the spec, add an
  explicit synchronous-write check for `Checkbox` (the existing
  "flips its signal on change" covers no-prop; an added
  `debounceMs: 0` assertion is fine if it doesn't bloat the file).
  The other three are already covered by their existing change tests.

---

### Step 5: Documentation in `docs/components.md`

**Goal:** Surface the prop in the reference table and explain its
semantics — including the two caveats (visible-value lag for
keystroke components; surprising semantics for click-driven ones).

**Files:**
- `docs/components.md` (modify)

**Changes:**
- In the component reference table (lines ~154–170), append
  `debounceMs` to the optional-props list of the six rows: `Input`,
  `TextArea`, `Checkbox`, `Toggle`, `Radio`, `Select` — matching how
  the `Combobox` row (line 159) already lists `debounceMs` among its
  optional props.
- Add a short prose paragraph in/after the convention section
  (around line 172, after the "State-shaped props use signals"
  bullet group). Content:
  - Signal-writing components accept `debounceMs?: number` to delay
    the signal write by N milliseconds (trailing edge), mirroring the
    `@event.debounce:<ms>` template modifier for cases where the
    template isn't reachable through the component API. Defaults to
    `0` (synchronous).
  - One sentence on the **visible-value lag** for `Input`/`TextArea`:
    during the debounce window the DOM shows the typed text while the
    signal still holds the previous value; an external write to the
    signal during that window still re-renders immediately (same as
    today — not a new footgun).
  - One sentence on **click-driven semantics**
    (`Checkbox`/`Toggle`/`Radio`/`Select`): the control toggles in the
    DOM immediately and only the signal write is delayed, so the
    visible state and signal diverge during the window — usually you
    want to debounce the downstream effect, not the signal write.
  - One sentence noting that `Combobox.debounceMs` means something
    different (gap before `loadOptions` runs), so the same prop name
    carries a different meaning on that component.
- No new docs page.

---

## Risks and Assumptions

- **Timer-based tests can be flaky.** The `await new Promise(r =>
  setTimeout(r, 80))` pattern assumes the Boa test harness honours
  real timers and that 80 ms comfortably exceeds the 50 ms window. The
  spec confirms the harness supports timers. If tests prove flaky,
  widen the margin (e.g. 50 ms window / 120 ms wait) rather than
  changing the helper. Note the [[boa_maplock_finalizer]] memory: keep
  any code-path-variant branches in their own functions in
  `runtime/*.js` — not expected to bite here since this work is in
  `.ts` scaffold files, not `runtime/`.
- **`fire(..., "change")` synchronous-write assumption.** The plan
  asserts the signal is unchanged *synchronously* after firing the
  event under `debounceMs > 0`. This holds only if `fire` dispatches
  synchronously and the debounced timer hasn't elapsed — true for the
  trailing-edge `setTimeout`. If `fire` were async this assertion
  would need rethinking; existing tests fire synchronously, so this is
  safe.
- **Requirement #7 (FRAMEWORK_NOTES annotation) is out of repo.**
  `zero_demo/FRAMEWORK_NOTES.md` is not present in this repository, so
  no execution step touches it. The annotation
  (`**FIXED YYYY-MM-DD** (commit SHA): …`) must be applied in the
  `zero_demo` project after this lands, by whoever owns that repo.
  Flag this to the user at hand-off; it is not a code deliverable
  here.
- **Type-checking the `as T` cast.** The helper casts the wrapped
  arrow back to `T`. This matches the spec verbatim and the existing
  liberal-generic style in `_internal.ts`; assumes the project's TS
  config doesn't reject it (the existing `read`/`isReactive` casts
  suggest it won't).
- **Scaffold vs runtime mirror.** The spec mentions both
  `crates/zero-scaffold/.../components/` and
  `runtime/.zero/components/Combobox.ts` paths. This plan edits only
  the canonical scaffold source under
  `crates/zero-scaffold/src/scaffold/.zero/components/`. Assumption:
  the scaffold tree is the single source of truth for these component
  files; no parallel runtime copy needs hand-editing. If a synced
  runtime copy exists and is not generated from the scaffold, it would
  need the same edits — verify during execution before declaring done.
