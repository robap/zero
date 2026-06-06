# Spec: Built-in validation rules for `createForm`

## Problem Statement

`createForm` (shipped 2026-06-06, [forms](../forms/spec.md)) accepts only a
hand-written validator function per field. The most common validations —
required, length caps, integer-in-range — are simple, highly repetitive, and
re-typed in every form with slightly different wording:

```ts
validate: (v) =>
  v.trim() === "" ? "Code is required."
  : v.trim().length > 10 ? "Code must be 10 characters or fewer."
  : null,
```

The forms spec deliberately deferred "a declarative rule DSL (`required`,
`maxLength`, …)" to keep v1 small. This item is that follow-up, in the form
chosen during refinement: **built-in rule factory functions** (typed,
autocompletable, no string parser), not a string DSL.

```ts
validate: [required(), maxLength(10)],
```

## Background

- **Where the code lives.** `createForm` is a vendored module:
  `crates/zero-scaffold/src/scaffold/.zero/components/form.ts`, exported from
  the components `index.ts`, typed in `.zero/components.d.ts`, with a sibling
  `form.test.ts`. The same tree is committed under `examples/*/web/.zero/` and
  exercised by `showcase/` and the slow `component_library` / `examples_*`
  integration tests.
- **Current validator contract** (`form.ts:24`):
  `validate?: (value: string, values: Record<K, string>) => string | null`.
  A rule factory that *returns* exactly this function type composes with the
  existing machinery for free — `runValidators`, live error re-check in
  `makeField`, `isValid`, and `submit` all consume plain validator functions
  and need no knowledge of rules.
- **Observed repeated patterns** (recorded in the forms spec from the
  zero_demo friction log): required + trimmed length caps, integer-in-range on
  string-valued number inputs. `pattern`/`email` were not yet observed but were
  selected for v1 during refinement.
- **Field values are strings** in `createForm` v1; numeric inputs stay strings
  until submit-time conversion.

## Requirements

### `validate` accepts a rule, an array, or a function (unchanged)

1. `FieldConfig.validate` widens from a single function to
   `Validator | Validator[]`, where
   `Validator = (value: string, values: Record<K, string>) => string | null`.
   - A single function (today's style) keeps working unchanged — full
     backward compatibility.
   - An array runs in declaration order; the **first non-null message wins**
     and is the field's error.
   - Because rule factories return plain `Validator` functions, arrays may
     freely mix built-in rules and hand-written functions:
     `validate: [required(), (v) => v === "x" ? "No x." : null]`.
2. The form-level cross-field `validate` keeps its current function-only
   signature; rules and arrays apply per-field only.

### Built-in rule factories

3. Export the following factories from the components index (so
   `import { required, maxLength } from "zero/components"` works); each
   returns a `Validator`:
   - `required()` — invalid when the trimmed value is empty.
   - `minLength(n)` / `maxLength(n)` — trimmed length bounds.
   - `intRange(min, max)` — value (trimmed) parses as an integer and falls in
     `[min, max]` inclusive. Covers the string-valued number-input pattern.
   - `pattern(regex)` — value matches the given `RegExp`.
   - `email()` — value looks like an email address (pragmatic regex; exact
     strictness decided in plan).
4. **Empty-value handling:** every rule *except* `required()` passes (returns
   `null`) when the trimmed value is empty, so optional fields with
   constraints work without hand-written functions. Each such rule accepts an
   option to opt out of this skip and enforce the rule on empty input too
   (per refinement: behavior is controllable per rule, default is skip).
5. **Custom messages:** each factory accepts an optional final argument
   `string | RuleOptions` — a plain string is shorthand for the custom
   message; the options object carries `message` and the empty-handling flag
   (req. 4). Example: `maxLength(200, "Keep notes under 200 chars.")`,
   `intRange(1, 999, { allowEmpty: false })`.
6. **Default messages:** each rule has a sensible generic English default that
   includes its parameters where helpful (e.g. "Must be 10 characters or
   fewer.", "Must be a whole number between 1 and 999."). Exact wording
   decided in plan. `pattern()` has no readable parameter, so its default is
   generic ("Invalid format.") — its docs should steer users toward passing a
   message.
7. Everything fully JSDoc-annotated and strongly typed per CLAUDE.md (no
   `any`; `RuleOptions` and `Validator` types exported if useful to users).

### Tests

8. Vendored test coverage (sibling `.test.ts` in the scaffold tree):
   - Each rule: valid value, invalid value, empty-value skip, empty-value
     enforcement via the option, custom message via string shorthand and via
     options object, default message content.
   - Array semantics: order, first-failure-wins, mixed rule + plain function.
   - Single-rule (non-array) `validate: required()` works.
   - Regression: existing single-function `validate` style still passes the
     current `form.test.ts` matrix untouched.

### Sync

9. The vendored copies under `examples/*/web/.zero/` and `showcase/` are
   regenerated/updated by the existing mechanism, `.zero/components.d.ts`
   updated, and the slow integration tests
   (`cargo test --workspace -- --include-ignored`) pass.

### Docs (user-facing — required)

10. `docs/components.md`: extend the `createForm` reference with the rule
    factories — full signature table, empty-value semantics, custom messages,
    array composition, and a worked example replacing a hand-written
    validator.
11. `docs/api.md`: add each exported factory (and any exported types) to the
    flat reference.

## Constraints

- Vendored components tree only — **no core runtime changes**
  (`runtime/*.js` untouched), no new import surface beyond `zero/components`.
- Rule factories must return plain `Validator` functions; `createForm`'s
  internal machinery (`runValidators`, `makeField` live re-validation,
  `isValid`, `submit`) must not grow rule-specific branches beyond
  normalizing `Validator | Validator[]` to a list.
- Backward compatible: every existing `validate: (v, values) => …` call site
  (scaffold tests, examples, showcase, docs examples) works unchanged.
- Field values remain `string`-typed (v1 contract).
- CLAUDE.md style rules: functions < ~80 lines, full JSDoc, strong types.

## Out of Scope

- A string DSL (`"required"`, `"max:10"`) — explicitly rejected during
  refinement in favor of typed factories.
- Async validators / server-roundtrip rules (uniqueness stays on the 409
  path).
- Array/rule support for the form-level cross-field `validate`.
- New rules beyond the six listed (e.g. `url`, `matches(otherField)`, numeric
  decimals) — easy follow-ups once a real form needs them.
- Boolean/array field values; touched/blur model changes; anything else from
  the forms spec's out-of-scope list.
- Migrating zero_demo's forms to the new rules (demo repo, after release).

## Open Questions

- **Option name for empty-value enforcement** (`allowEmpty: false`?
  `requireValue: true`?) — pick one consistent name in plan.
- **Module placement:** rules in `form.ts` alongside `createForm`, or a
  sibling `rules.ts` re-exported from `index.ts`? (`form.ts` is ~310 lines;
  six factories + options plumbing may justify a separate file.)
- **`email()` regex strictness** — pragmatic (HTML5-ish `x@y.z`) vs strict;
  decide in plan and document the choice.
- **`intRange` parsing edge cases** — leading `+`, leading zeros, `1e3`;
  define what "parses as an integer" means precisely in plan.
