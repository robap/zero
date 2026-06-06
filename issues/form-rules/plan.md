# Plan: Built-in validation rules for `createForm`

## Summary

Widen `FieldConfig.validate` to accept a single validator function (today's
style), or an array run in order with first-failure-wins, and add a new
vendored `rules.ts` module exporting six typed rule factories — `required`,
`minLength`, `maxLength`, `intRange`, `pattern`, `email` — each returning a
plain validator function, so `createForm`'s machinery (`runValidators`,
`makeField` live re-check, `isValid`, `submit`) needs only one normalization
helper and no rule-specific branches. Everything rides the existing scaffold
manifest mechanism (`include_str!` in `crates/zero-scaffold/src/lib.rs` →
`zero init`/`zero update`); examples/showcase `.zero/` trees are gitignored
and materialized by `zero update`, so there is no manual sync.

Resolved spec open questions (decisions baked into this plan):

- **Empty-enforcement option name:** `allowEmpty?: boolean`, default `true`
  (matches the spec's own example `intRange(1, 999, { allowEmpty: false })`).
- **Module placement:** a new sibling `rules.ts` + `rules.test.ts` (form.ts
  is already ~310 lines; six JSDoc'd factories would push it well past
  readable), re-exported from the components `index.ts`.
- **`email()` regex:** pragmatic HTML5-ish `/^[^\s@]+@[^\s@]+\.[^\s@]+$/`
  on the trimmed value; documented as pragmatic, not RFC-strict.
- **`intRange` parsing:** trimmed value must match `/^[+-]?\d+$/`, then
  `Number(...)` with inclusive `[min, max]` bounds. Rejects `1e3`, decimals,
  whitespace-infixed input; accepts leading `+`/`-` and leading zeros.
- **Type-variance trap (verified against `form.ts`):** rules must return a
  *single-parameter* validator type `Rule = (value: string) => string | null`.
  A `Rule` is assignable to the field validator
  `(value: string, values: Record<K, string>) => string | null` (fewer params
  is fine); the reverse construction — typing rules as
  `Validator<string>` — would fail under `strictFunctionTypes` because
  `Record<K, string>` has no index signature. Don't fight this; rules never
  need `values`.
- **`required()` signature:** `required(message?: string)` — `allowEmpty` is
  meaningless for the one rule whose job is rejecting empty, so it takes the
  message-string shorthand only (the spec's req-4 "each such rule" already
  scopes the option to non-required rules).

## Prerequisites

None.

## Steps

- [x] **Step 1: widen `validate` to `Validator | Validator[]` in `form.ts`**
- [x] **Step 2: `rules.ts` module — options plumbing + `required`/`minLength`/`maxLength`**
- [x] **Step 3: `intRange`, `pattern`, `email`**
- [x] **Step 4: integration guards + full slow suite**
- [x] **Step 5: docs — `components.md`, `api.md`**

---

## Step Details

> **Verification loop for scaffold changes** (same as the forms plan):
> scaffold sources are `include_str!`-embedded, so component/test changes are
> exercised via
> `cargo test -p zero --test component_library -- --include-ignored --nocapture`
> (copies `showcase/`, runs `zero update --yes` from the freshly built binary,
> then `zero test`). Scaffold-level assertions run with
> `cargo test -p zero-scaffold`. Use both per step.

### Step 1: widen `validate` to `Validator | Validator[]` in `form.ts`

**Goal:** The contract change lands first, with no behavioral change for
existing single-function callers, so the rule factories in Steps 2–3 have a
typed slot to plug into.

**Files:**
- `crates/zero-scaffold/src/scaffold/.zero/components/form.ts`
- `crates/zero-scaffold/src/scaffold/.zero/components/form.test.ts`
- `crates/zero-scaffold/src/scaffold/.zero/components/index.ts`
- `crates/zero-scaffold/src/scaffold/.zero/components.d.ts`

**Changes:**
- `form.ts`: export a named validator type and widen `FieldConfig`:

  ```ts
  /** Per-field validator: return an error message or `null` when valid. */
  export type Validator<K extends string = string> = (
    value: string,
    values: Record<K, string>,
  ) => string | null;

  export type FieldConfig<K extends string> = {
    initial: string;
    /** One validator or an array run in order; first non-null message wins. */
    validate?: Validator<K> | Validator<K>[];
  };
  ```

- `form.ts`: in `createForm`, normalize once up front —
  `const validators = {} as Record<K, Validator<K>[]>` filled per key with
  `[]`, `[fn]`, or the array as-given (small `@internal` helper
  `toList<K>(v: Validator<K> | Validator<K>[] | undefined): Validator<K>[]`).
  `runValidators` changes from the single `config.fields[k].validate?.(...)`
  call to a `for` over `validators[k]` that records the **first** non-null
  message and stops. No other function changes — `makeField`'s `afterWrite`,
  `isValid`, and `submit` all consume `runValidators` and are untouched.
- `index.ts`: add `Validator` to the `export type { ... } from "./form.ts"`
  list.
- `components.d.ts`: mirror — add `export type Validator<K extends string = string> = ...`
  and change `FieldConfig.validate` to `Validator<K> | Validator<K>[]`.

**Tests** (extend `form.test.ts`, inside the existing `describe("createForm")`):
- array runs in declaration order, first non-null wins (two validators that
  both fail; assert the first's message; assert via a recording array that
  the second never runs once the first fails — or that order is respected);
- mixed array of two plain functions works for both pass and fail values;
- single-function `validate` (non-array) still behaves identically — the
  existing matrix already covers this; add one explicit
  `validate: [fn]`-equals-`validate: fn` sanity case;
- empty array `validate: []` behaves like no validator.

### Step 2: `rules.ts` module — options plumbing + `required`/`minLength`/`maxLength`

**Goal:** The new module exists, is registered in the scaffold manifest, and
ships the three most-repeated rules, establishing the options-resolution
pattern Steps 3 reuses.

**Files:**
- `crates/zero-scaffold/src/scaffold/.zero/components/rules.ts` (new)
- `crates/zero-scaffold/src/scaffold/.zero/components/rules.test.ts` (new)
- `crates/zero-scaffold/src/scaffold/.zero/components/index.ts`
- `crates/zero-scaffold/src/scaffold/.zero/components.d.ts`
- `crates/zero-scaffold/src/lib.rs`

**Changes:**
- `rules.ts` (fully JSDoc'd, no `any`):

  ```ts
  /** A validator produced by a rule factory; ignores cross-field values. */
  export type Rule = (value: string) => string | null;

  /** Options accepted by every rule factory except `required`. */
  export type RuleOptions = {
    /** Replaces the rule's default message. */
    message?: string;
    /** When false, the rule also rejects empty (whitespace-only) values.
     *  Default true: empty passes, so optional fields compose. */
    allowEmpty?: boolean;
  };
  ```

  - `@internal` helper
    `resolveOptions(opts?: string | RuleOptions): { message?: string; allowEmpty: boolean }`
    — a plain string is shorthand for `{ message }`; `allowEmpty` defaults
    `true`.
  - `@internal` helper `isEmpty(value: string): boolean` → `value.trim() === ""`.
  - `required(message?: string): Rule` — fails iff trimmed value is empty.
    Default message: `"This field is required."`
  - `minLength(n: number, opts?: string | RuleOptions): Rule` — trimmed
    length ≥ `n`. Default message:
    `` `Must be at least ${n} character${n === 1 ? "" : "s"}.` ``
  - `maxLength(n: number, opts?: string | RuleOptions): Rule` — trimmed
    length ≤ `n`. Default message:
    `` `Must be ${n} character${n === 1 ? "" : "s"} or fewer.` ``
  - Empty-skip contract for `minLength`/`maxLength`: if `allowEmpty` (the
    default) and `isEmpty(value)`, return `null` before any other check;
    with `allowEmpty: false`, an empty value falls through to the length
    check (so `minLength(2, { allowEmpty: false })("")` fails with the
    length message).
- `rules.test.ts`: `describe("validation rules", () => { ... })` — the
  describe name is load-bearing for Step 4's `component_library.rs`
  assertion. Imports from `"./rules.ts"`; also one integration case wiring
  `createForm` + `[required(), maxLength(10)]` from `"./form.ts"` to prove
  assignability of `Rule` into `Validator<K>` under the real TS config.
- `index.ts`: append (only what exists in this step — the line is extended
  to all six factories in Step 3):

  ```ts
  export { maxLength, minLength, required } from "./rules.ts";
  export type { Rule, RuleOptions } from "./rules.ts";
  ```
- `components.d.ts`: add `Rule`, `RuleOptions`, and the three factory
  signatures next to the `createForm` block.
- `crates/zero-scaffold/src/lib.rs`: add
  `const TPL_RULES_TS: &str = include_str!("scaffold/.zero/components/rules.ts");`
  and `TPL_RULES_TEST_TS` likewise, plus the two manifest entries
  `(".zero/components/rules.ts", TPL_RULES_TS)` and
  `(".zero/components/rules.test.ts", TPL_RULES_TEST_TS)` adjacent to the
  existing `form.ts` entries (lib.rs:159–160).

**Tests** (`rules.test.ts`, per rule):
- valid value → `null`; invalid value → default message (exact string);
- custom message via string shorthand and via `{ message }`;
- empty + whitespace-only value passes by default (`minLength`/`maxLength`);
- `{ allowEmpty: false }` makes empty fail (`minLength`);
- `required()` fails on `""` and `"   "`, passes on `"a"`;
- boundary cases: `minLength(2)` on `"ab"` passes, `"a"` fails; `maxLength(2)`
  on `"ab"` passes, `"abc"` fails; trimming: `maxLength(2)` on `" ab "`
  passes;
- the `createForm` integration case: submit with empty value sets the
  `required` message; fix the value, message clears.

### Step 3: `intRange`, `pattern`, `email`

**Goal:** Complete the v1 rule set on the pattern established in Step 2.

**Files:**
- `crates/zero-scaffold/src/scaffold/.zero/components/rules.ts`
- `crates/zero-scaffold/src/scaffold/.zero/components/rules.test.ts`
- `crates/zero-scaffold/src/scaffold/.zero/components/index.ts`
- `crates/zero-scaffold/src/scaffold/.zero/components.d.ts`

**Changes:**
- `intRange(min: number, max: number, opts?: string | RuleOptions): Rule` —
  empty-skip first; then trimmed value must match `/^[+-]?\d+$/` and
  `Number(trimmed)` must satisfy `min <= n && n <= max`. Default message:
  `` `Must be a whole number between ${min} and ${max}.` ``
- `pattern(re: RegExp, opts?: string | RuleOptions): Rule` — empty-skip
  first; then test the **raw** (untrimmed) value. Defuse stateful regexes:
  construct once at factory time
  `const safe = new RegExp(re.source, re.flags.replace(/[gy]/g, ""))` so a
  passed `/x/g` can't alternate results via `lastIndex`. Default message:
  `"Invalid format."` — JSDoc steers users toward passing a message.
- `email(opts?: string | RuleOptions): Rule` — empty-skip first; then
  trimmed value must match `/^[^\s@]+@[^\s@]+\.[^\s@]+$/`. Default message:
  `"Enter a valid email address."` JSDoc notes the check is pragmatic, not
  RFC 5322.
- `index.ts`: extend the Step-2 export line to all six factories.
- `components.d.ts`: add the three signatures.

**Tests** (`rules.test.ts`):
- `intRange(1, 999)`: passes `"1"`, `"999"`, `"010"`, `"+5"`, `" 42 "`;
  fails `"0"`, `"1000"`, `"1e3"`, `"3.5"`, `"abc"`, `"-1"`;
  `intRange(-5, 5)` passes `"-3"`; empty passes by default;
  `{ allowEmpty: false }` makes `""` fail; custom message both forms;
- `pattern(/^[A-Z]+$/)`: pass/fail, default message, custom message; a `/g`
  regex returns the same result on two consecutive calls with the same
  failing-then-passing value (the `lastIndex` defusal);
- `email()`: passes `"a@b.co"`, `" a@b.co "`; fails `"a@b"`, `"a b@c.d"`,
  `"@b.co"`, `"a@"`; empty passes by default; custom message.

### Step 4: integration guards + full slow suite

**Goal:** The hard-coded report-name guard knows about the new test file, and
the materialized-project paths (`zero update` → bundle → test) prove out.

**Files:**
- `crates/zero/tests/component_library.rs`

**Changes:**
- Add `"validation rules"` to the hard-coded name list in
  `showcase_test_runs_all_component_tests` (`component_library.rs:37`),
  with the existing comment style noting it is `rules.test.ts`'s describe
  name, load-bearing like `createForm`.
- Check whether `cargo test -p zero-scaffold` carries a file-manifest
  assertion that needs the two new paths (lib.rs tests around line 337
  assert per-component files; `form.ts` is special-cased — mirror whatever
  it does for `rules.ts`, or nothing if only `COMPONENT_NAMES` are checked).

**Tests:**
- `cargo test -p zero-scaffold`
- `cargo test -p zero --test component_library -- --include-ignored --nocapture`
- `cargo test --workspace -- --include-ignored` (spec req 9 — examples_*,
  showcase_*, build_full all consume the updated manifest).

### Step 5: docs — `components.md`, `api.md`

**Goal:** Spec reqs 10–11; docs are part of done.

**Files:**
- `docs/components.md`
- `docs/api.md`

**Changes:**
- `components.md`, in the Forms section (~line 473): update the
  `FieldConfig.validate` line in the `createForm(config)` reference to
  `Validator | Validator[]` with the first-failure-wins rule; add a
  **Built-in rules** subsection containing:
  - a signature table for the six factories (rule, signature, default
    message, empty-value behavior);
  - the empty-skip contract and `allowEmpty: false` opt-out;
  - custom messages (`string` shorthand vs `RuleOptions`);
  - array composition mixing rules and hand-written functions;
  - a worked before/after: the existing hand-written
    `code`-required-and-≤10-chars ternary (already in the docs example at
    ~line 489) rewritten as `validate: [required(), maxLength(10)]`.
- `api.md`: add rows for `required`, `minLength`, `maxLength`, `intRange`,
  `pattern`, `email` to the `zero/components` function table (~line 131,
  next to `createForm`), each linking to
  `Components § Built-in rules`; extend the exported-types sentence
  (~line 151) with `Validator<K>`, `Rule`, `RuleOptions`.

**Tests:** none (prose). Sanity-check that every code snippet added compiles
against the real API (signatures copied from `rules.ts`, not retyped).

## Risks and Assumptions

- **TS variance:** the whole design leans on `Rule` (one param) being
  assignable to `Validator<K>` (two params). This is standard TS behavior,
  and Step 2's `createForm`-integration test case locks it in early; if the
  vendored tree's TS config somehow rejected it, the fallback is
  `Rule = Validator<never>`-style gymnastics — replan if the simple form
  fails.
- **Describe-name guard:** `component_library.rs` greps the test report for
  hard-coded names. The plan adds `"validation rules"`; if the executor
  names the describe block differently, the slow suite fails loudly (by
  design) — keep the two in sync.
- **`components.d.ts` is hand-maintained** (per the forms plan); drift
  between it and `rules.ts`/`form.ts` won't be caught by a test. The plan
  mirrors every type change in the same step that touches the source.
- **No manual example sync assumed:** examples/showcase `.zero/` trees are
  gitignored and materialized by `zero update` (verified in the forms plan,
  and `prepare_showcase` does this in-test). If any example's `.zero/` turns
  out to be committed after all, those copies would need regeneration in
  Step 4.
- **Backward compatibility** rests on `toList` normalization preserving
  single-function semantics exactly; the untouched existing `form.test.ts`
  matrix is the regression net.
