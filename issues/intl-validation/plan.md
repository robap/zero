# Plan: `Intl` shim option validation

## Summary

Add hand-written option validation to all three constructors in
`runtime/intl-shim.js` so that invalid option values throw browser-faithful
`RangeError`/`TypeError` at the points the spec throws, spec-valid but
shim-unimplemented options throw the established `zero test: … is not
implemented` shim error, and `timeZone: "UTC"` becomes genuinely supported via
a single date-parts accessor switch. The work is one shim file plus its test
file plus one docs section — no Rust changes, no loading-path changes (the
`build.rs` concatenation is untouched). Steps are ordered constructor-by-
constructor so each lands compilable and green on its own.

## Prerequisites

The spec's three open questions are resolved here by codebase evidence
(no usage of any of them in the demo, examples, or scaffold):

1. **`RelativeTimeFormat` `style: "short"`/`"narrow"` → reject** with the
   shim error, not implement. Nothing in the demo/examples/scaffold passes a
   non-long style; en-US abbreviation tables are fiddly to get
   browser-faithful, and rejecting is honest. The `docs/testing.md` line
   documenting "short/narrow render as long" is updated to match (Step 5).
2. **`resolvedOptions().timeZone` → present only when `timeZone: "UTC"` was
   passed**, omitted otherwise. Nothing reads it; documented in Step 5.
3. **`hour12` default pin** — a regression test asserting hour-only formats
   default to 12-hour lands in Step 2 *before* the accessor refactor.

No dependent issues. Note one deliberate strictness beyond browsers, carried
over from the spec: `hour12` / `useGrouping` must be actual booleans
(`RangeError` otherwise), where ECMA-402's GetOption would coerce via
`ToBoolean`. For a test-runtime shim, catching `hour12: "yes"` is the point;
Step 5 documents it.

## Steps

- [x] **Step 1: Shared validation helpers + DateTimeFormat validation**
- [x] **Step 2: `timeZone: "UTC"` support via a date-parts accessor**
- [x] **Step 3: NumberFormat validation**
- [x] **Step 4: RelativeTimeFormat validation**
- [x] **Step 5: Documentation (`docs/testing.md`)**
- [x] **Step 6: Full-suite verification and closeout notes**

---

## Step Details

### Step 1: Shared validation helpers + DateTimeFormat validation

**Goal:** Establish the three validation primitives every constructor will
use, apply them to `DateTimeFormat` (the constructor the friction log names),
and clean the stale Boa comments while in the file. After this step the
friction-log probe (`day: ""`) throws.

**Files:** `runtime/intl-shim.js`, `runtime/intl-shim.test.js`

**Changes:**

1. Rewrite the stale file-header paragraph and the three constructor JSDoc
   notes that cite the Boa 0.21.1 GC rationale (Boa was removed 2026-05-31;
   the runner is QuickJS). Keep the constructors as `function`s — just drop
   the obsolete justification. Also fix the test-file header comment
   ("host/Boa timezone").
2. Add three helpers near the top of the shim (each JSDoc'd, < 80 lines):
   - `_requireOneOf(ctor, option, value, allowed)` — no-op when `value ===
     undefined`; otherwise throws
     `RangeError(\`Value "${value}" out of range for Intl.${ctor} options property ${option}\`)`
     when `!allowed.includes(value)` (mirrors Chrome's message shape).
   - `_requireBoolean(ctor, option, value)` — no-op on `undefined`; throws
     the same-shaped `RangeError` when `typeof value !== "boolean"`.
   - `_rejectUnsupported(ctor, options, keys)` — for each key in `keys`
     present on `options` (`!== undefined`), throws
     `Error(\`zero test: Intl.${ctor} option "${key}" is not implemented. Remove it or guard the call for the test runtime.\`)`
     — the message shape `docs/testing.md` already promises for shim gaps.
3. In `_resolveDateTimeOptions` (or a new `_validateDateTimeOptions` called
   first from the constructor, keeping both under ~80 lines):
   - `_rejectUnsupported('DateTimeFormat', options, DTF_UNSUPPORTED)` with
     `DTF_UNSUPPORTED = ['era', 'timeZoneName', 'hourCycle', 'dayPeriod',
     'fractionalSecondDigits', 'calendar', 'numberingSystem',
     'formatMatcher', 'localeMatcher']`. (`timeZone` is **not** in this
     list — Step 2 handles it; until Step 2 lands it stays silently ignored
     for one commit, which is the status quo.)
   - `_requireOneOf` per component: `weekday` ∈
     `['long','short','narrow']`; `year` ∈ `['numeric','2-digit']`; `month`
     ∈ `['numeric','2-digit','long','short','narrow']`; `day`, `hour`,
     `minute`, `second` ∈ `['numeric','2-digit']`; `dateStyle`, `timeStyle`
     ∈ `['full','long','medium','short']`.
   - `_requireBoolean('DateTimeFormat', 'hour12', options.hour12)`.
   - Truly unknown keys (`foo`) remain ignored — validation only reads the
     enumerated names, so this falls out for free; pin it with a test.

**Tests** (extend the `Intl.DateTimeFormat` describe block):
- The exact friction probe:
  `new Intl.DateTimeFormat('en-US', { month: 'short', day: '', hour: 'numeric' })`
  throws `RangeError` (assert via `expect(() => …).toThrow()` or the
  harness's idiom for throw assertions).
- One invalid value per option family: `weekday: 'tiny'`, `year: 'long'`,
  `month: 'bogus'`, `day: ''`, `hour: 'full'`, `dateStyle: 'huge'`,
  `timeStyle: ''` — each `RangeError`.
- `hour12: 'yes'` throws `RangeError`.
- One unimplemented key per tier: `timeZoneName: 'short'` and
  `localeMatcher: 'lookup'` throw with a message containing
  `is not implemented`.
- `{ foo: 1 }` does **not** throw and formats as the default.
- All existing valid-option tests untouched and green (no output changes).

### Step 2: `timeZone: "UTC"` support via a date-parts accessor

**Goal:** Make the one spec-carved exception real: `timeZone: "UTC"` renders
through `getUTC*` accessors; anything else throws the shim error. Refactor
date-component access to a single site so the UTC switch can't half-apply.

**Files:** `runtime/intl-shim.js`, `runtime/intl-shim.test.js`

**Changes:**

1. **First**, add the `hour12` pin test (see Tests) so the refactor below is
   guarded.
2. In the DateTimeFormat options path: read `options.timeZone`; `undefined`
   → no-op; a string whose `toUpperCase() === 'UTC'` → record
   `components.timeZone = 'UTC'` and `resolved.timeZone = 'UTC'`; any other
   value → the `_rejectUnsupported`-shaped `Error` (hand-rolled message
   naming the value:
   `zero test: Intl.DateTimeFormat timeZone "${value}" is not implemented (only "UTC" is supported). …`).
   `resolvedOptions()` omits `timeZone` entirely when it wasn't passed.
3. Add `_dateParts(d, utc)` returning
   `{ year, monthIndex, day, weekdayIndex, hours, minutes, seconds }` from
   either the local (`getFullYear`…) or UTC (`getUTCFullYear`…) accessor
   family. Call it once in `DateTimeFormat.prototype.format` and thread the
   parts object through the rendering helpers — `_formatWeekday`,
   `_yearStr`, `_formatTextualDate`, `_formatNumericDate`,
   `_formatDatePortion`, `_formatTimePortion` switch their first parameter
   from `Date` to the parts object (rename `d` → `p`; internal reads become
   `p.year`, `p.monthIndex`, etc.). No other logic changes — byte-identical
   output for the non-UTC path.

**Tests:**
- Pin: `new Intl.DateTimeFormat('en-US', { hour: 'numeric' }).format(d)` for
  the existing 15:07 fixture is `'3 PM'` (12-hour default with no `hour12`)
  — added before the refactor commit-wise, or at minimum asserted green
  after.
- UTC rendering: build `new Date(Date.UTC(2024, 0, 5, 23, 7, 9))` and assert
  `{ timeZone: 'UTC', month: 'short', day: 'numeric', hour: 'numeric',
  minute: '2-digit' }` formats `'Jan 5, 11:07 PM'` regardless of host zone,
  and the date-rollover case (`Date.UTC(2024, 0, 5, 0, 30)`) renders day 5
  not the host-local day.
- Lowercase `'utc'` accepted and equivalent.
- `timeZone: 'America/New_York'` throws with `is not implemented` in the
  message.
- `resolvedOptions().timeZone === 'UTC'` when passed;
  `'timeZone' in resolvedOptions()` is `false` when not.

### Step 3: NumberFormat validation

**Goal:** Same contract for `NumberFormat`: invalid values throw, the
spec-mandated missing-currency `TypeError` lands, the silent `min > max`
clamp becomes the spec's `RangeError`, and unimplemented options throw the
shim error.

**Files:** `runtime/intl-shim.js`, `runtime/intl-shim.test.js`

**Changes:**

1. New `_validateNumberOptions(options)` called from the `NumberFormat`
   constructor before storing `_opts`:
   - `_rejectUnsupported('NumberFormat', options, NF_UNSUPPORTED)` with
     `NF_UNSUPPORTED = ['notation', 'unit', 'unitDisplay', 'signDisplay',
     'compactDisplay', 'currencySign', 'roundingMode', 'roundingPriority',
     'roundingIncrement', 'trailingZeroDisplay', 'minimumIntegerDigits',
     'minimumSignificantDigits', 'maximumSignificantDigits',
     'localeMatcher']`.
   - `_requireOneOf('NumberFormat', 'style', options.style,
     ['decimal', 'currency', 'percent'])` — note `'unit'` lands here as a
     `RangeError` naming the allowed set, which is the boundary message the
     spec asks for.
   - `currencyDisplay`: `undefined` or `'symbol'` pass; anything else throws
     the `_rejectUnsupported`-shaped shim `Error` (it's a spec-valid value
     the shim doesn't render).
   - If `style === 'currency'`: `currency === undefined` →
     `TypeError('Currency code is required with currency style')`;
     otherwise must match `/^[a-zA-Z]{3}$/` → else
     `RangeError(\`Invalid currency code: ${currency}\`)`; store uppercased
     (preserves the existing `CURRENCY_SYMBOLS` lookup and `"${code} "`
     fallback for well-formed unknown codes).
   - `minimumFractionDigits` / `maximumFractionDigits`: when present, must
     be integers (`Number.isInteger`) in 0–100, else
     `RangeError(\`${option} value is out of range\`)`. After both resolve:
     explicit `min > max` → `RangeError` — **delete the `if (max < min) max
     = min` clamp in `_resolveFractionDigits`** (the clamp only triggers
     when both are explicit and inverted, which now throws at construction,
     or when an explicit min exceeds a *default* max — keep that case by
     keeping `Math.max(defMax, min)`; only the explicit-vs-explicit
     inversion throws).
   - `_requireBoolean('NumberFormat', 'useGrouping', options.useGrouping)`.
2. `resolvedOptions()` continues to spread the (now-normalized) `_opts` —
   currency reports uppercased.

**Tests** (extend the `Intl.NumberFormat` describe block):
- `style: 'unit'` → `RangeError`; `style: ''` → `RangeError`.
- `{ style: 'currency' }` (no currency) → `TypeError`.
- `currency: 'DOLLARS'` and `currency: 'U$'` → `RangeError`;
  `currency: 'usd'` formats as `$` (uppercased).
- `minimumFractionDigits: -1`, `maximumFractionDigits: 101`,
  `maximumFractionDigits: 1.5` → `RangeError`.
- `{ minimumFractionDigits: 4, maximumFractionDigits: 2 }` → `RangeError`
  (was a silent clamp).
- `{ minimumFractionDigits: 4 }` alone still works (max floats up): format
  `1` → `'1.0000'`.
- `useGrouping: 'no'` → `RangeError`.
- `notation: 'compact'` and `currencyDisplay: 'code'` → shim `Error` with
  `is not implemented`.
- Existing valid cases green.

### Step 4: RelativeTimeFormat validation

**Goal:** Close the last constructor: option validation at construction,
unit/value validation in `format()`, and the silent `numeric` coercion and
silent long-rendering of `short`/`narrow` styles replaced with throws.

**Files:** `runtime/intl-shim.js`, `runtime/intl-shim.test.js`

**Changes:**

1. In the `RelativeTimeFormat` constructor:
   - `_rejectUnsupported('RelativeTimeFormat', options, ['localeMatcher'])`.
   - `_requireOneOf('RelativeTimeFormat', 'numeric', o.numeric,
     ['always', 'auto'])` — replaces the `o.numeric === 'auto' ? 'auto' :
     'always'` coercion (the default when `undefined` stays `'always'`).
   - `style`: `undefined`/`'long'` pass; `'short'`/`'narrow'` throw the
     shim `Error` (spec-valid, shim-unimplemented — per the resolved open
     question); any other value → `RangeError` via
     `_requireOneOf(…, ['long', 'short', 'narrow'])` run *first*, then the
     short/narrow shim rejection.
2. In `RelativeTimeFormat.prototype.format`:
   - `const v = Number(value); if (!Number.isFinite(v)) throw new
     RangeError('Value need to be finite number for Intl.RelativeTimeFormat.prototype.format()')`
     (Chrome's message shape).
   - After `_normalizeUnit`, the unit must be one of
     `['second','minute','hour','day','week','month','quarter','year']`,
     else `RangeError(\`Invalid unit argument for format() '${unit}'\`)`
     naming the *original* argument. Plural forms keep working via the
     existing normalize-then-check order.
   - Use `v` (the coerced number) in place of `value` downstream so numeric
     strings keep behaving as before.

**Tests** (extend the `Intl.RelativeTimeFormat` describe block):
- `numeric: 'sometimes'` → `RangeError`; `numeric: 'auto'` still words.
- `style: 'short'` and `style: 'narrow'` → shim `Error` with
  `is not implemented`; `style: 'compact'` → `RangeError`.
- `localeMatcher: 'lookup'` → shim `Error`.
- `format(-2, 'bananas')` → `RangeError` mentioning `bananas`.
- `format(Infinity, 'day')` and `format(NaN, 'day')` → `RangeError`.
- `format('-1', 'day')` → `'1 day ago'` (numeric string still coerces).
- Existing valid cases green.

### Step 5: Documentation (`docs/testing.md`)

**Goal:** The user-facing boundary in `docs/testing.md` §Internationalization
(~lines 336–375) states the new contract so nobody rediscovers it by failure.
The spec's docs requirement; the scaffold/showcase AGENTS.md files have no
`Intl` mention (verified by grep), so `docs/testing.md` is the only doc site.

**Files:** `docs/testing.md`

**Changes:**

1. After the constructor list, add a short **Option validation** paragraph:
   invalid option *values* throw `RangeError` exactly as a browser does
   (so formatter configuration is pinnable and `zero mutate` literal
   mutants on option strings die); spec-valid options the shim does not
   implement throw the standard `zero test: … is not implemented` error
   instead of silently changing nothing; truly unknown keys are ignored,
   as in browsers.
2. Note the one extra strictness: `hour12` and `useGrouping` must be real
   booleans (browsers coerce; the shim throws `RangeError`).
3. `DateTimeFormat` bullet: add `timeZone: "UTC"` (case-insensitive) is
   supported and renders via UTC accessors; any other zone throws;
   `resolvedOptions().timeZone` is reported only when it was passed.
4. Update the "Documented partial cases" paragraph: **remove** the
   "`RelativeTimeFormat` `short`/`narrow` styles render as `long`" clause
   (they now throw); keep `timeStyle` `full`/`long` → `medium` (still
   accepted values, still rendered as medium — unchanged).
5. `NumberFormat` bullet: mention the `TypeError` on missing currency and
   that fraction-digit bounds are range-checked (`min > max` throws).

**Tests:** None (docs). Sanity-read rendered markdown.

### Step 6: Full-suite verification and closeout notes

**Goal:** Prove no consumer of the shim anywhere in the workspace trips the
new throws, per the spec's "existing valid output is byte-identical"
constraint, and stage the friction-log follow-up.

**Files:** none (verification only).

**Changes / Actions:**

1. `cargo run -p zero -- test` — full JS runtime suite (includes
   `intl-shim.test.js`, `web-platform.test.js`).
2. `cargo test --workspace` — fast Rust loop.
3. `cargo test --workspace -- --include-ignored` — the slow showcase /
   examples / init integration tests, since showcase or example apps could
   construct formatters (grep found none, but this is the contract gate the
   spec names).
4. Closeout note for the user (actions in the *demo* repo, not this one —
   per the no-git rule, just report): after the next `zero update` in
   `zero_demo`, flip FRAMEWORK_NOTES.md entry #78 to `- [x]` with a
   `**FIXED**` annotation, and re-run `zero mutate` on
   `web/src/shared/format.ts` to confirm the 3 residual lit_str survivors
   (`"short"`/`"numeric"`/`"2-digit"` → `""`) are now killed — the demo's
   `formatWhen` constructs its formatter on first call, so any test calling
   it now dies on the mutated literal.

## Risks and Assumptions

- **The throw-on-unimplemented contract is a behavior change for code that
  passes `timeZone` (non-UTC), `timeZoneName`, etc., today.** Grep says
  nothing in this workspace or the demo does; the `--include-ignored` run in
  Step 6 is the backstop. If a showcase test trips, the fix is to amend that
  caller (the new behavior is the spec).
- **Browser error-message fidelity is approximate.** Messages mirror
  Chrome's *shape* (`Value "x" out of range for Intl.X options property y`)
  but tests must assert error *type* and a substring, not exact text, so
  the plan isn't brittle against wording tweaks.
- **The demo's `formatWhen` caches its formatter in a module-level `let`**
  (`whenFmt`), so a mutated option literal throws on first `formatWhen`
  call — killable, as the spec predicts. If the demo's tests never call
  `formatWhen` at all, the mutants die only when such a test exists; that's
  a demo-side follow-up, not a shim defect.
- **`_resolveFractionDigits` clamp removal** must distinguish
  explicit-min > explicit-max (throw at construction) from
  explicit-min > default-max (legal; max floats up via
  `Math.max(defMax, min)`). Getting this wrong breaks the existing
  `minimumFractionDigits: 2` decimal test — which is exactly why that test
  staying green is named in Step 3.
- **Assumes the QuickJS runner has no residue of the Boa function-vs-class
  constraint** — the constructors stay `function`s regardless, so this is
  comment-only risk (none).
