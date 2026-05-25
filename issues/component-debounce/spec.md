# Spec: `debounceMs` prop on signal-writing components

## Problem Statement

From `zero_demo/FRAMEWORK_NOTES.md` (2026-05-24):

> The new `@event.debounce:250` suffix is a real win, but
> `zero/components`' `Input` wraps the `<input>` and binds the signal
> internally; consumers can't reach the `@input` slot to apply the
> modifier. To debounce input changes feeding external state, you
> still need a hand-rolled `setTimeout` in an `effect()`. A
> `debounceMs?: number` prop on `Input` would close the gap.

The `.debounce:<ms>` template modifier is parsed from the *static*
portion of an `html\`\`` template string at parse time
(`runtime/template.js:670-686`); a component consumer cannot inject a
modifier into the inner `<input>` because the template lives inside
the component implementation. The friction-log workaround
(`setTimeout` in an `effect()`) reintroduces the boilerplate the
template modifier was supposed to delete.

Prior art already exists in the components library: `Combobox` has
`debounceMs?: number` (defaults to 200 ms) that controls the wait
before the internal `loadOptions(query)` runs after the last
keystroke. Extending the same prop to other signal-writing components
makes the gap reachable through the public component API.

Scope (per user direction): add `debounceMs?: number` to six
signal-writing components — `Input`, `TextArea`, `Checkbox`,
`Toggle`, `Radio`, `Select`. The keystroke-driven members (`Input`,
`TextArea`) are the canonical motivation; the click-driven members
(`Checkbox`, `Toggle`, `Radio`, `Select`) get the prop for uniformity
across the signal-writing surface, even though the per-keystroke
storm doesn't apply. See Open Questions for the trade-off there.

## Background

### Where the relevant code lives

- `crates/zero-scaffold/src/scaffold/.zero/components/Input.ts` —
  one-liner `onInput` writes to `props.value` on every `input` event.
- `crates/zero-scaffold/src/scaffold/.zero/components/TextArea.ts` —
  same shape as `Input` but renders a `<textarea>`.
- `crates/zero-scaffold/src/scaffold/.zero/components/Checkbox.ts`,
  `Toggle.ts`, `Radio.ts`, `Select.ts` — each has a single
  `onChange` handler that writes to `props.<signal>` on the native
  `change` event.
- `crates/zero-scaffold/src/scaffold/.zero/components/Combobox.ts:533`
  — already takes `debounceMs?: number` and applies it to its
  internal `scheduleFetch` (`setTimeout(doFetch, ctx.debounceMs)`).
  Sets the precedent for the prop name, semantics, and default
  handling.
- `crates/zero-scaffold/src/scaffold/.zero/components/_internal.ts`
  — shared, components-only helpers (`Reactive<T>`, `isReactive`,
  `read`). Where the new `debounce` helper will live (per user
  direction).
- `runtime/template.js:718-724` — the framework-internal
  `_debounce(fn, ms)` powering the `@event.debounce:<ms>` template
  modifier. Same shape as the helper we'll add to `_internal.ts`,
  but reachable only inside `runtime/`. Not exported.
- `docs/components.md:155-170` — the component prop reference table.
  Will gain a `debounceMs?` mention per affected component.

### Why a prop, not a template modifier

The `.debounce:<ms>` modifier is parsed by the html-template tokeniser
from the literal text between substitutions in the tagged template:

```
html`<input @input.debounce:250=${onInput}>`
              ^^^^^^^^^^^^^^^ static text — tokenised at parse time
```

A consumer rendering `Input(...)` doesn't write that template — the
component does, inside its body. Substitutions like
`@input.debounce:${ms}=${onInput}` aren't supported because the
modifier name has to be a literal in the static text. So the only
way to expose debouncing through the component API is a JS-side
wrapper on the handler.

### The helper

`_internal.ts` gains:

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

Notes on the shape:
- `ms <= 0` (including `undefined` coerced to `NaN`) returns `fn`
  unchanged. Components pass `props.debounceMs ?? 0` and don't have
  to branch.
- The wrapper is trailing-edge only (no leading-edge fire) to match
  `runtime/template.js`'s `_debounce`.
- Type signature uses a generic so wrapped handlers keep their
  parameter types. `any[]` inside the generic is acceptable here
  (`_internal.ts` is the existing home for such utility types and
  doesn't shy from `any` in the generic constraint — `read` uses a
  similar liberal pattern).

### Visible-value lag under debounce (Input/TextArea)

Today `Input` does:

```ts
@input=${onInput} value=${() => props.value.val}
```

`onInput` fires on every keystroke and synchronously calls
`props.value.set(target.value)`, which triggers the reactive
`value=` binding back to the same string — a no-op write.

With `debounceMs`, the `onInput` write is delayed. During the
debounce window:
- The DOM input element holds whatever the user typed (native
  behaviour, unaffected by the framework).
- `props.value.val` is the *previous* value.
- The reactive `value=` binding does NOT re-fire on its own — it
  only re-evaluates when `props.value` changes, which now happens
  on the debounce trailing edge.

So no flicker, no cursor-snap, no overwrite of typed text. The
caveat: if the parent's code externally writes to `props.value`
during the debounce window (e.g. a reset effect, a network
response), the reactive binding *will* re-fire and replace the
visible value. This matches the current behaviour without
`debounceMs` — an external write is always immediately reflected —
and is therefore not a new footgun. Worth a one-line note in the
docs.

### Click-driven components: what `debounceMs` actually means

`Checkbox`, `Toggle`, `Radio`, `Select` don't have keystroke
storms. A user click produces a single `change` event and a single
signal write. Debouncing it means:

- The visible control (checkbox, toggle thumb, select option)
  toggles in the DOM immediately (native behaviour).
- The signal write is delayed by `debounceMs`.

For these, the *only* observable effect of `debounceMs > 0` is to
delay downstream effects driven by the signal — same as wrapping
the click handler in a `setTimeout`. This is rarely what users
want (the visible state and the signal diverge during the window);
the more common pattern is to debounce the *downstream effect*, not
the signal write.

The user requested uniform coverage anyway. The spec includes the
prop on these components but flags this concern under Open
Questions for the plan phase to revisit.

## Requirements

### 1. Components-internal `debounce` helper

In
`crates/zero-scaffold/src/scaffold/.zero/components/_internal.ts`:

- Add the `debounce<T>(fn: T, ms: number): T` function as specified
  in Background. `@internal` JSDoc tag, full `@param` / `@returns`
  / `@template` annotation per the repo's TS style rule.
- No new public surface — `_internal.ts` is not re-exported from
  `index.ts`, so user code outside `.zero/components/` cannot
  import `debounce` from `"zero/components"`.

### 2. `Input.ts`

- Add `debounceMs?: number` to `InputProps`. JSDoc on the prop:
  "Optional debounce window in milliseconds for the `value`
  signal write. `0` or omitted means synchronous (current
  behaviour)."
- Import `debounce` from `./_internal.ts`.
- Wrap the `onInput` handler:
  `const handler = debounce(onInput, props.debounceMs ?? 0);`
- Pass `handler` to `@input=${handler}`.
- The visible `value=${() => props.value.val}` binding is
  unchanged.

### 3. `TextArea.ts`

- Same pattern: `debounceMs?: number` on `TextAreaProps`, wrap
  `onInput` via `debounce(onInput, props.debounceMs ?? 0)`.
- Visible binding `${() => props.value.val}` unchanged.

### 4. `Checkbox.ts`, `Toggle.ts`, `Radio.ts`, `Select.ts`

For each:
- Add `debounceMs?: number` to the props type.
- Import `debounce` from `./_internal.ts`.
- Wrap the existing `onChange` handler:
  `const handler = debounce(onChange, props.debounceMs ?? 0);`
- Pass `handler` to `@change=${handler}`.

The visible-state bindings (`checked=${() => checked.val}`,
option `selected=${...}`, etc.) are unchanged. The native input
reflects the click immediately; only the signal write is delayed.

### 5. Tests

Per component, in its existing `*.test.ts`:

- An "honours `debounceMs`" test that:
  - Renders the component with `debounceMs: 50` (or similar small
    value).
  - Fires the relevant event (`input` for Input/TextArea, `change`
    for the others).
  - Synchronously after the event: asserts the signal value has
    NOT yet been written (still the initial value).
  - Waits the debounce interval (use `await new Promise(r =>
    setTimeout(r, 80))` — the Boa harness already supports
    timers).
  - Asserts the signal value HAS been written.
- An "without `debounceMs`, writes synchronously" regression test
  for at least `Input` and `Checkbox` — the others are already
  covered by the existing "updates its signal on change" tests
  but adding `debounceMs: 0` explicitly to those existing tests
  is fine if it doesn't bloat the file.
- A "successive events within window collapse to one write" test
  for `Input` (the canonical use case): fire `input` three times
  with different values inside the window, then wait, and assert
  the signal has *only* the last value (no intermediate writes).
  The other five components do not need this test — for
  click-driven ones the trailing-edge behaviour is the same but
  the test value is lower.
- A unit test for `debounce` itself in a new file
  `_internal.test.ts` (if it doesn't already exist) or appended to
  the existing one if there is one already. The existing file is
  `_internal.test.ts` per the scaffold listing — append to it.
  Cases:
  - `debounce(fn, 0)` returns the same function reference (early
    return).
  - `debounce(fn, -5)` returns the same function reference (early
    return; defensive).
  - `debounce(fn, 50)` delays the call; multiple calls within
    50 ms collapse to one trailing call with the last args.

### 6. Documentation

- `docs/components.md:155-170` — extend the prop reference rows
  for `Input`, `TextArea`, `Checkbox`, `Toggle`, `Radio`, `Select`
  with `debounceMs?: number` (matching the `Combobox` row's
  treatment of `debounceMs`).
- One short prose addition near the existing convention section
  (around line 172 in the file): a paragraph explaining that
  signal-writing components accept `debounceMs?: number` to delay
  the signal write, mirroring the `.debounce:<ms>` template
  modifier behaviour. Note the visible-value-vs-signal lag for
  Input/TextArea and the unusual semantics for click-driven
  components.
- No new docs page is needed.

### 7. Friction-log fix annotation

After the implementation lands, append a fix annotation to the
relevant FRAMEWORK_NOTES.md entry following the project's
established `**FIXED YYYY-MM-DD** (commit SHA): …` pattern. The
agent landing the change writes this annotation as part of the
slice's deliverable.

## Constraints

- **`_internal.ts` stays private.** It must not be re-exported
  from `crates/zero-scaffold/src/scaffold/.zero/components/index.ts`.
  Users importing from `"zero/components"` cannot reach the helper.
- **Trailing-edge only.** `debounce` matches the framework's
  `_debounce` in `runtime/template.js`: no leading fire, no
  `{leading: true}` option. If a leading-edge variant is needed
  later it's a separate slice.
- **`debounceMs <= 0` is a no-op.** Both `undefined` and `0`
  produce synchronous signal writes — same as today. This means
  `props.debounceMs ?? 0` works at every callsite.
- **No new public API.** No `debounce` export from `zero`. The
  template modifier already covers the user-facing template case;
  this slice fills only the component-wrapped case.
- **Prop name is `debounceMs`.** Matches the existing `Combobox`
  prop, the JS convention `*Ms` for millisecond durations, and the
  template modifier's `:<ms>` suffix.
- **No change to the template modifier.** `.debounce:<ms>` and
  `.throttle:<ms>` stay exactly as they are; this slice does not
  touch `runtime/template.js`.
- **No new dependencies.** Pure TS additions inside the existing
  scaffold tree.
- **No Combobox refactor.** `Combobox`'s internal
  `scheduleFetch` debounce is intertwined with serial-bumping for
  async race safety (`runtime/.zero/components/Combobox.ts:87-103`).
  A clean swap to the shared `debounce` helper is not in scope —
  it can be considered later if the helper's shape grows.
- **Cleanup timers on unmount is not in scope for this slice.**
  Today's `_debounce` in `runtime/template.js` doesn't clear its
  timer when the element is removed either; the symmetric trade-off
  is acceptable. If a leaked timer fires after `cleanup()` and
  writes to a disposed signal, the signal `set()` is a no-op (signals
  don't fail when their consumers are gone). The friction this
  causes (a stray write landing post-test) is bounded by the
  debounce window, so the impact is small. A timer-clearing
  refactor with a `ref` or a disposer can land separately.

## Out of Scope

- A public `debounce` / `throttle` export from `zero`. Future
  slice if user code starts asking for it; today's template
  modifier covers the templated case.
- A `throttleMs?: number` prop alongside `debounceMs?`. Search and
  form inputs almost always want debounce; throttle is more
  natural on scroll/resize, where the modifier is already
  reachable. Add later if asked.
- A leading-edge variant (`{leading: true}` or a
  `debounceLeading` helper).
- Cancelling the pending write on `cleanup()` / element removal.
  See Constraints above.
- Refactoring `Combobox`'s internal debounce to share the new
  helper. The shapes don't line up — `Combobox` debounces an async
  fetch with race-safety serial bumping.
- Surfacing `debounceMs` on non-signal-writing components
  (`Button`, `Card`, `Badge`, `Avatar`, `Spinner`, `Toast`,
  `Pagination`, `Tabs`, `Dialog`, `Table`). Most don't write
  signals on every event; the ones that do (`Pagination.onChange`,
  `Tabs.active`, `Dialog.onClose`) are already debounced upstream
  by user code if needed.
- A lint rule warning when `debounceMs` is passed alongside a
  click-driven component (`Checkbox`/`Toggle`/`Radio`/`Select`)
  where its semantics are surprising.
- Backporting the helper into a downstream `_internal.ts` in
  user projects that already cloned the components. Per the
  framework's update story, `zero update` brings these files into
  sync; users on outdated copies can re-run the scaffold step.

## Open Questions

- **Should click-driven components really accept `debounceMs`?**
  The user picked the wide scope. The recommendation in this spec
  is to honour that choice and include the prop on all six, but
  the plan phase can revisit. A narrower v1 would ship the prop on
  `Input` and `TextArea` only (the canonical motivation) and add
  the others later if requests come in. The wider shape is
  preferable for *consistency* of the component surface; the
  narrower shape is preferable for *clarity* of when the prop is
  useful. Recommendation: wide ship, mention the click-driven
  semantics caveat in the docs.
- **Default value vs explicit `0`.** Should `debounceMs` default
  to `0` (no debounce) or to a small value like `100` (matching
  the template modifier's bare-form default)? `Combobox` defaults
  to `200`. Recommendation: default to `0` for the keystroke
  cases (the current behaviour is "synchronous write" and changing
  that silently would be a regression for existing callers) and
  for the click cases. Users who want debouncing opt in. The
  template modifier's `100 ms` default is for the modifier-only
  form; a prop is more explicit and should default off.
- **Test for click-driven debounce on every component, or pick a
  representative?** Six tests is uniform but boilerplate-heavy.
  Recommendation: full coverage — each test is ~6 lines and the
  uniformity catches a future regression where a single
  component drops the wrapper.
- **Should `_internal.test.ts` get the unit test for `debounce`,
  or should it live next to the component tests?** The helper is
  a shared utility, so `_internal.test.ts` is the natural home.
  Confirmed it exists per the scaffold listing
  (`_internal.test.ts` in `crates/zero-scaffold/.../components/`).
  Recommendation: append.
- **Helper name: `debounce` vs `debounceFn` vs `_debounce`.**
  `debounce` matches the modifier name and reads naturally at
  callsites (`debounce(onInput, props.debounceMs ?? 0)`). No
  underscore prefix because `_internal.ts` is already private at
  the module-boundary level — the prefix would be redundant
  noise. Recommendation: `debounce`.
- **TypeScript generic on the helper: `T extends (...args: any[])
  => void` vs `T extends (...args: any[]) => unknown`.** The
  helper drops the return value (trailing-edge invocation can't
  return synchronously to the caller). `=> void` is the honest
  signature. Recommendation: `=> void`.
- **Combobox prop semantics review.** `Combobox.debounceMs`
  controls the gap between keystrokes and `loadOptions`, not the
  gap between keystrokes and `value` writes (those happen on pick,
  not on keystroke). The new prop on the other components controls
  the gap between keystrokes/changes and the signal write. Same
  *name*, different *meaning*. Recommendation: don't rename
  either; mention the distinction in the docs paragraph so users
  reading both pages aren't confused.
