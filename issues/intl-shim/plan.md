# Plan: `Intl` shim for the test runtime

## Summary

Add a hand-written, pure-JS `runtime/intl-shim.js` that installs a minimal
`Intl` namespace (`DateTimeFormat`, `NumberFormat`, `RelativeTimeFormat`) onto
`globalThis` in the Boa test runtime, following the exact conventions of the
existing Web Platform shims (`encoding-shim.js` is the template). The shim is
en-US-only by design: any `locales` argument is accepted and ignored, always
producing en-US output. It is concatenated into the shim blob via
`crates/zero-runtime/build.rs`'s `WEB_PLATFORM_FILES` list, so it loads as a
side effect of importing `"zero/test"` — no Rust dependencies, no Boa `intl`
feature, no ICU. The work is staged one constructor at a time (each leaving the
suite green), then documented in `docs/testing.md` (the canonical audited-
surface list).

## Prerequisites

None blocking. The three spec open questions are resolved for execution:

1. **`toLocale*` prototype delegation** — **out of scope** for this plan (spec
   default: "skip unless trivial"). Wiring `Date.prototype.toLocaleString` /
   `Number.prototype.toLocaleString` through the shim is deferred; reaching for
   them yields whatever Boa already returns. Revisit as a follow-up if it
   surfaces in the friction log.
2. **Canonical audited-surface list** — it is the **Web Platform surface**
   section of `docs/testing.md` (lines ~223–285). No `zero-framework-spec.md`
   exists in this repo. The docs step appends an `Internationalization`
   subsection there.
3. **Exact option matrix** — pinned per constructor in the Step Details below.

## Steps

- [x] **Step 1: Scaffold `intl-shim.js` with `Intl.DateTimeFormat`, wire into build**
- [x] **Step 2: Add `Intl.NumberFormat`**
- [x] **Step 3: Add `Intl.RelativeTimeFormat`**
- [x] **Step 4: Document the surface and close out the friction log**

---

## Step Details

### Step 1: Scaffold `intl-shim.js` with `Intl.DateTimeFormat`, wire into build

**Goal:** Stand up the shim file, register it in the build so `Intl` exists on
`globalThis` under test, and deliver the direct friction trigger
(`Intl.DateTimeFormat` producing `MMM D, h:mm A`). This step alone closes the
`ReferenceError: Intl is not defined` failure mode.

**Files:**
- `runtime/intl-shim.js` (new)
- `crates/zero-runtime/build.rs` (add to `WEB_PLATFORM_FILES`)
- `runtime/intl-shim.test.js` (new — DateTimeFormat cases)
- `runtime/web-platform.test.js` (add one smoke `it` for `Intl`)

**Changes:**

1. **`runtime/intl-shim.js`** — pure script body (no `import`/`export`), full
   JSDoc, functions < ~80 lines, branch-per-function per the
   `boa_maplock_finalizer` rule. Header comment mirrors `encoding-shim.js`.

   Shared module-level data (`@type`-annotated `const`s):
   - `MONTHS_LONG` / `MONTHS_SHORT` (`January…`, `Jan…`).
   - `WEEKDAYS_LONG` / `WEEKDAYS_SHORT` (`Sunday…`, `Sun…`).

   `class DateTimeFormat`:
   - `constructor(locales, options)` — `locales` accepted and ignored; store a
     normalized, resolved options object on `this`. Compute defaults the way
     en-US `Intl` does: if neither date nor time component (nor
     `dateStyle`/`timeStyle`) is given, default to `{ year:'numeric',
     month:'numeric', day:'numeric' }`.
   - `format(date)` — accept `Date`, timestamp `number`, or `undefined` (→
     `new Date()`); coerce to a `Date`; build the string from **local** date
     accessors (`getFullYear`, `getMonth`, `getDate`, `getHours`, `getMinutes`,
     `getSeconds`, `getDay`) so semantics match `Date.prototype` local
     rendering (deterministic w.r.t. the Date's own value).
   - `resolvedOptions()` — return the resolved options plus
     `{ locale: 'en-US', calendar: 'gregory', numberingSystem: 'latn' }`.

   Supported option matrix (each routed through its own small helper fn):
   - Component options:
     - `year`: `'numeric'` (`1970`) | `'2-digit'` (`70`).
     - `month`: `'numeric'` (`1`) | `'2-digit'` (`01`) | `'short'` (`Jan`) |
       `'long'` (`January`) | `'narrow'` (`J`).
     - `day`: `'numeric'` | `'2-digit'`.
     - `hour`: `'numeric'` | `'2-digit'`; honors `hour12` (default `true` for
       en-US → 12-hour clock with `AM`/`PM`; `hour12:false` → 24-hour).
     - `minute` / `second`: `'numeric'` | `'2-digit'`.
     - `weekday`: `'short'` (`Thu`) | `'long'` (`Thursday`) | `'narrow'` (`T`).
   - Preset styles (mapped to component presets, then assembled):
     - `dateStyle`: `full` → `Weekday, Month D, YYYY`; `long` → `Month D, YYYY`;
       `medium` → `Mon D, YYYY`; `short` → `M/D/YY`.
     - `timeStyle`: `short` → `h:mm AM/PM`; `medium` → `h:mm:ss AM/PM`. **`full`
       / `long` timeStyle include a timezone name** the shim can't reliably
       produce; treat them as `medium` and document the limitation (do **not**
       throw).
     - When both `dateStyle` and `timeStyle` are present, join with `, `
       (en-US: `Jan 1, 1970, 12:00 AM`).
   - Assembly order for component options (en-US): weekday, then month/day/year
     in `M/D/YYYY`-ish order per which components are present, then time. Match
     real en-US output for the combinations in the test matrix; an unsupported
     component value falls back to `'numeric'` rather than throwing.

   Install at end of file, guarded, matching the `Object.defineProperty`
   pattern used by `clone-shim.js`/`encoding-shim.js`:
   ```js
   if (typeof globalThis.Intl === 'undefined') {
     Object.defineProperty(globalThis, 'Intl', {
       value: { DateTimeFormat }, writable: true, configurable: true,
     });
   }
   ```
   (Steps 2–3 extend the installed namespace object literal.)

2. **`crates/zero-runtime/build.rs`** — add `"intl-shim.js"` to
   `WEB_PLATFORM_FILES`. Order after `clone-shim.js` (self-contained, so order
   is not behaviorally critical; keep the list deterministic).

**Tests:**
- `runtime/intl-shim.test.js` — `describe('Intl.DateTimeFormat')` with `it`
  cases asserting exact en-US strings against a fixed `Date`
  (`new Date(Date.UTC(...))` constructed so local accessors are deterministic —
  see Risks for the tz note; prefer constructing via local `new Date(y,m,d,…)`
  to sidestep tz ambiguity):
  - The friction trigger: `{ month:'short', day:'numeric', hour:'numeric',
    minute:'2-digit', hour12:true }` → e.g. `"Jan 5, 3:07 PM"`.
  - Each `dateStyle` (`full`/`long`/`medium`/`short`).
  - `timeStyle` `short` and `medium`.
  - `hour12:false` 24-hour rendering.
  - `weekday` long/short.
  - Locale ignored: `new Intl.DateTimeFormat('fr-FR', opts).format(d)` equals
    the `'en-US'` result and does not throw.
  - `resolvedOptions().locale === 'en-US'`.
- `runtime/web-platform.test.js` — one `it('Intl.DateTimeFormat formats en-US
  dates', …)` smoke case so `Intl` joins the audited-surface smoke file.
- Gate: `cargo run -p zero -- test` green; `cargo test --workspace` green.

---

### Step 2: Add `Intl.NumberFormat`

**Goal:** Cover the common companion to date formatting — currency, decimals,
percent — the demo's inventory `$` amounts being the motivating case.

**Files:**
- `runtime/intl-shim.js` (extend)
- `runtime/intl-shim.test.js` (extend)

**Changes:**

1. `class NumberFormat` in `runtime/intl-shim.js`:
   - `constructor(locales, options)` — `locales` ignored; resolve and store
     options with en-US defaults.
   - `format(value)` — coerce to `Number`; produce the string. Helpers split by
     style (branch-per-function): `_formatDecimal`, `_formatCurrency`,
     `_formatPercent`.
   - `resolvedOptions()` — resolved options + `{ locale:'en-US',
     numberingSystem:'latn' }`.
   - Supported options:
     - `style`: `'decimal'` (default) | `'currency'` | `'percent'`.
     - `currency` (required when `style==='currency'`) + small symbol map:
       `USD`→`$`, `EUR`→`€`, `GBP`→`£`, `JPY`→`¥` (JPY default 0 fraction
       digits); unknown code → use the code itself as a prefix. (Only the
       default `currencyDisplay:'symbol'` case is implemented.)
     - `minimumFractionDigits` / `maximumFractionDigits` — defaults: decimal
       `min 0 / max 3`; currency `min 2 / max 2` (0/0 for JPY); percent
       `min 0 / max 0`.
     - `useGrouping` — default `true`; insert `,` thousands separators.
   - Rounding: use `toFixed(maxFractionDigits)` then strip trailing zeros down
     to `minimumFractionDigits` (round-half-up; acceptable approximation of
     Intl's `halfExpand` for the supported precision — note in Risks).
   - Grouping helper applies `,` every three integer digits.

2. Extend the install block: add `NumberFormat` to the `Intl` namespace object.
   (If `globalThis.Intl` already exists from Step 1's literal, attach
   `NumberFormat` to it; keep the single guarded define-or-extend coherent.)

**Tests:** `describe('Intl.NumberFormat')`:
- `1234.5` decimal → `"1,234.5"`; `useGrouping:false` → `"1234.5"`.
- `1234.5` currency USD → `"$1,234.50"`; `0` USD → `"$0.00"`.
- `1234` currency JPY → `"¥1,234"` (no fraction digits).
- `0.1255` percent with `maximumFractionDigits:1` → `"12.6%"`.
- `minimumFractionDigits:2` on `1` decimal → `"1.00"`.
- Locale ignored: `'de-DE'` gives the same en-US string (`,` grouping / `.`
  decimal), no throw.
- `resolvedOptions().locale === 'en-US'`.
- Gate: `cargo run -p zero -- test` + `cargo test --workspace` green.

---

### Step 3: Add `Intl.RelativeTimeFormat`

**Goal:** Round out the three committed constructors with `"3 days ago"`-style
formatting.

**Files:**
- `runtime/intl-shim.js` (extend)
- `runtime/intl-shim.test.js` (extend)

**Changes:**

1. `class RelativeTimeFormat` in `runtime/intl-shim.js`:
   - `constructor(locales, options)` — `locales` ignored; store
     `numeric` (`'always'` default | `'auto'`) and `style`
     (`'long'` default | `'short'` | `'narrow'`).
   - `format(value, unit)` — `unit` one of `second|minute|hour|day|week|month|
     quarter|year` (accept plural forms too, normalizing trailing `s`).
   - `'always'` numeric (long style):
     - `value < 0` → `"N unit(s) ago"`; `value >= 0` → `"in N unit(s)"`.
       Pluralize unit when `|value| !== 1`.
   - `'auto'` numeric — en-US word substitutions where they exist, else fall
     back to the `'always'` form:
     - day: `-1`→`"yesterday"`, `0`→`"today"`, `1`→`"tomorrow"`.
     - week/month/quarter/year: `-1`→`"last X"`, `0`→`"this X"`, `1`→`"next X"`.
     - hour/minute: `0`→`"this hour"`/`"this minute"`; second `0`→`"now"`.
   - `'short'`/`'narrow'` style: long-form behavior is acceptable for the
     committed scope; implement long fully and map short/narrow to long unless a
     concrete caller needs the abbreviated forms (note as a documented
     limitation rather than a throw).
   - `resolvedOptions()` — resolved options + `{ locale:'en-US',
     numberingSystem:'latn' }`.

2. Extend the install block: add `RelativeTimeFormat` to the `Intl` namespace.

**Tests:** `describe('Intl.RelativeTimeFormat')`:
- Default (`always`,`long`): `format(-1,'day')` → `"1 day ago"`;
  `format(3,'day')` → `"in 3 days"`; `format(-2,'hour')` → `"2 hours ago"`.
- `numeric:'auto'`: `format(-1,'day')` → `"yesterday"`; `format(1,'day')` →
  `"tomorrow"`; `format(0,'day')` → `"today"`; `format(-1,'week')` →
  `"last week"`.
- Plural/singular boundary at `|value| === 1`.
- Locale ignored, no throw; `resolvedOptions().locale === 'en-US'`.
- Gate: `cargo run -p zero -- test` + `cargo test --workspace` green.

---

### Step 4: Document the surface and close out the friction log

**Goal:** Make the boundary explicit so users learn what's covered without
rediscovering the edge by failure, and record the fix.

**Files:**
- `docs/testing.md` (add to the Web Platform surface section)
- (follow-up, different repo) `zero_demo/FRAMEWORK_NOTES.md` entry flip — noted,
  not edited here.

**Changes:**

1. `docs/testing.md` — under "## Web Platform surface", add an
   **Internationalization** subsection after "Cloning & scheduling" and before
   the "clear error" discipline paragraph:
   - `Intl.DateTimeFormat`, `Intl.NumberFormat`, `Intl.RelativeTimeFormat` —
     en-US only. List the supported option families per constructor (mirroring
     the matrices above).
   - State the deliberate departure from the "clear error" discipline: a
     **non-en-US `locales` argument is accepted and silently formatted as
     en-US** (it does not throw) — this is a known limitation, called out
     explicitly.
   - Note the documented partial cases: `DateTimeFormat` `timeStyle`
     `full`/`long` render as `medium` (no timezone name); `RelativeTimeFormat`
     `short`/`narrow` styles render as `long`; `formatToParts` /
     `formatRange*` are not implemented.
   - Add the other `Intl` namespaces (`Collator`, `PluralRules`, `ListFormat`,
     `Segmenter`, `Locale`, `DisplayNames`, `supportedValuesOf`) to the
     out-of-scope list — reaching for them is `undefined`.

2. No `runtime/zero-test.d.ts` change: `Intl` is a TS standard-lib global, and
   the shim conforms to the standard interface shape, so existing types apply.

3. Add a note in this plan's closeout (and tell the user) that the demo repo's
   `FRAMEWORK_NOTES.md` `2026-05-24` 🟡 `Intl` entry should be flipped to
   `- [x]` with a `**FIXED**` annotation referencing the landing commit — done
   in `zero_demo`, not this repo.

**Tests:** Docs-only; no new tests. Final gate: full suite incl. slow
integration — `cargo test --workspace -- --include-ignored` — and
`cargo run -p zero -- test` both green.

---

## Risks and Assumptions

- **Timezone / local accessors.** `format` uses the `Date`'s *local* accessors
  (`getHours`, etc.), matching `Date.prototype` local semantics. Tests must
  construct fixtures with the local `new Date(y, m, d, h, min)` form (not a
  fixed UTC epoch read through local getters) so assertions are deterministic
  regardless of the host/Boa timezone. If Boa's local timezone proves
  non-deterministic in CI, fall back to UTC accessors and document that the
  shim formats in UTC — decide during Step 1 if a fixture flakes.
- **en-US output fidelity.** The assertions hard-code en-US strings; these were
  written from the spec, not machine-generated against a reference engine
  (the live probe was declined). If any expected string is off by punctuation
  (e.g. the comma between `dateStyle` and `timeStyle`, or `narrow` forms),
  adjust the expected value to the shim's spec-correct output during execution
  — the *behavior* is the contract, the exact literal may need a one-character
  tweak.
- **Rounding edge cases.** `toFixed`-based rounding approximates Intl's
  `halfExpand`; exotic half-way decimals could differ in the last digit. The
  test matrix stays on values that round unambiguously.
- **Single-namespace install across steps.** Steps 2–3 extend the `Intl`
  object literal created in Step 1. The executor must keep the guarded
  `Object.defineProperty(globalThis,'Intl',…)` block as the single install
  site (build it with all three by the end), not three competing guarded
  installs — assumed straightforward but called out to avoid a half-populated
  namespace.
- **Boa GC.** Following the branch-per-function rule should avoid the MapLock
  finalizer panic; the shim uses no top-level `Map`/`Set`, so risk is low.
