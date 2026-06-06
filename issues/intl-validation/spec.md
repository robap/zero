# Spec: `Intl` shim option validation

## Problem Statement

The test-runtime `Intl` shim (`runtime/intl-shim.js`, shipped by
[`issues/intl-shim/`](../intl-shim/spec.md)) accepts any option value without
checking it. A browser throws `RangeError` on an invalid option value (e.g.
`new Intl.DateTimeFormat("en-US", { day: "" })`); the shim silently ignores it
and formats anyway. Friction-log entry `2026-06-06` 🟡 in the demo's
`FRAMEWORK_NOTES.md` (#78) records the two concrete costs:

1. **Formatting options are unpinnable.** Probing
   `{ month: "short", day: "", … }` and other invalid variants produced
   byte-identical output ("Jan 15, 8:05 AM") — a test cannot assert that a
   formatter is configured correctly, only that its happy-path output looks
   right.
2. **`zero mutate` lit_str mutants on option literals are unkillable.**
   Mutating `"numeric"` / `"2-digit"` → `""` changes nothing observable
   in-harness, so the mutants survive. 3 of the demo's 5 residual survivors
   are exactly this — the shim's permissiveness directly pads the demo's
   mutation score floor.

The same gap exists in all three shimmed constructors, and in a second form:
options that are *valid per ECMA-402* but that the shim never reads
(`timeZone`, `timeZoneName`, `era`, …) are also silently ignored, producing
output that silently differs from a browser's.

## Background

### Where the shim lives and how it loads

- `runtime/intl-shim.js` — pure script body (no `import`/`export`), installs
  `globalThis.Intl` with `function` constructors `DateTimeFormat`,
  `NumberFormat`, `RelativeTimeFormat`.
- `crates/zero-runtime/build.rs` concatenates it (via `WEB_PLATFORM_FILES`)
  into the `zero_dom_shim_body.js` blob the test harness evaluates before user
  modules. This path survived the Boa→QuickJS migration unchanged.
- Tests: `runtime/intl-shim.test.js` (27 `it` blocks today) runs under
  `zero test` / `cargo run -p zero -- test`.
- **Stale comments:** the file header and the three constructor JSDoc blocks
  still explain a Boa 0.21.1 GC constraint ("plain `function`s rather than
  `class`es"). Boa was removed 2026-05-31 (runner is rquickjs now); update or
  drop those comments while touching the file — but there is no need to
  convert the constructors to classes as part of this item.

### Current validation state (none)

- `_resolveDateTimeOptions` copies whatever values appear for
  `weekday/year/month/day/hour/minute/second`, `dateStyle`, `timeStyle`,
  `hour12`; rendering helpers then treat unknown values as the `else` branch
  (e.g. `_monthName` renders any non-`narrow`/non-`short` value as `long`,
  `_yearStr` renders anything non-`2-digit` as `numeric`) — that fallback is
  *why* invalid values produce plausible output.
- `NumberFormat` stores options verbatim; any unknown `style` falls through
  to decimal; `currency` is stringified with a `"${code} "` prefix fallback;
  fraction-digit options are used unchecked (no 0–100 range check, no
  `min > max` error — the shim silently clamps `max` up to `min`, where the
  spec throws).
- `RelativeTimeFormat` coerces `numeric` to `"always"` unless exactly
  `"auto"`, stores any `style` string, and `format()` accepts any unit
  (`_normalizeUnit` just strips a trailing `s` — so even `"bananas"`
  formats as `"banana"`).

### Browser-faithful behavior to match (ECMA-402 GetOption semantics)

- Option **values** outside the spec's allowed set → `RangeError`.
- Option **keys** the spec doesn't define at all (e.g. `{ foo: 1 }`) are
  ignored — browsers never read them. The shim must keep ignoring those.
- `NumberFormat` with `style: "currency"` and no `currency` → `TypeError`
  (the one spec-mandated TypeError in this surface).
- Fraction-digit options outside 0–100 → `RangeError`;
  `minimumFractionDigits > maximumFractionDigits` → `RangeError`.
- `RelativeTimeFormat.prototype.format` with an invalid unit → `RangeError`.

### Decisions already resolved with the user

1. **Scope: all three constructors**, not just the `DateTimeFormat` named in
   the friction entry — same bug class, same unkillable-mutant consequence,
   shared validation helper.
2. **Spec-valid but shim-unimplemented options throw a clear shim error**
   (the web-platform shims' "intentional stub throws a clear error"
   discipline), naming the option and the boundary. Loud beats
   silently-different-from-browser output.
3. **Exception: `timeZone: "UTC"` gets implemented**, not rejected — it is
   the standard test-determinism idiom. Exactly the string `"UTC"` is
   accepted and rendering switches to the `Date` `getUTC*` accessors; any
   other `timeZone` value throws the clear shim error.

## Requirements

### 1. Value validation on supported options (browser-faithful `RangeError`)

For every option the shim reads, an invalid value must throw `RangeError`
at **construction time** (matching where browsers throw), with a message
naming the option and the offending value:

- `DateTimeFormat`: `weekday` ∈ long/short/narrow; `year` ∈ numeric/2-digit;
  `month` ∈ numeric/2-digit/long/short/narrow; `day`/`hour`/`minute`/`second`
  ∈ numeric/2-digit; `dateStyle`/`timeStyle` ∈ full/long/medium/short;
  `hour12` must be a boolean.
- `NumberFormat`: `style` ∈ decimal/currency/percent (the shim's supported
  set — see Req. 2 for spec-valid-but-unsupported values like
  `"unit"`); `currency` must be a 3-letter alphabetic code (case-insensitive,
  uppercased; unknown-but-well-formed codes keep the current `"${code} "`
  prefix fallback); `style: "currency"` without `currency` → `TypeError`;
  `minimumFractionDigits`/`maximumFractionDigits` integers in 0–100, and
  `min > max` → `RangeError` (replacing the current silent clamp);
  `useGrouping` must be a boolean.
- `RelativeTimeFormat`: `numeric` ∈ always/auto (replacing the silent
  coercion of anything-but-`"auto"` to `"always"`); `style` ∈
  long/short/narrow. `format(value, unit)` must throw `RangeError` for any
  unit not in second/minute/hour/day/week/month/quarter/year (singular or
  plural form), and `TypeError`-free coercion of `value` via `Number(value)`
  with non-finite values → `RangeError` (matching the spec).

The acceptance probe from the friction log must hold:
`new Intl.DateTimeFormat("en-US", { month: "short", day: "", hour: "numeric" })`
throws `RangeError`, and a lit_str mutation of `"numeric"` → `""` in user
code is killable by any test that constructs the formatter.

### 2. Unimplemented options throw a clear shim error

Passing a spec-defined option key the shim does not implement throws an
`Error` (not `RangeError` — this is a shim boundary, not a spec violation)
whose message names the option and states the shim's boundary, e.g.
`intl-shim: option "timeZoneName" is not supported (en-US shim implements: …)`.
Concretely:

- `DateTimeFormat`: `era`, `timeZoneName`, `hourCycle`, `dayPeriod`,
  `fractionalSecondDigits`, `calendar`, `numberingSystem`, `formatMatcher`,
  `localeMatcher` (and `timeZone` values other than `"UTC"` — see Req. 3).
- `NumberFormat`: `notation`, `unit`, `unitDisplay`, `signDisplay`,
  `compactDisplay`, `currencySign`, `currencyDisplay` values other than
  `"symbol"`, `roundingMode`/`roundingPriority`/`roundingIncrement`,
  `trailingZeroDisplay`, `minimumIntegerDigits`,
  `minimumSignificantDigits`/`maximumSignificantDigits`, `localeMatcher`.
  (`style: "unit"` is rejected by Req. 1's style check; the message should
  point at this boundary.)
- `RelativeTimeFormat`: `localeMatcher`. `style` values `"short"`/`"narrow"`
  are accepted today but render identically to `"long"`; either implement
  the abbreviated forms (`1 day ago` → `1 day ago` is unchanged for long;
  short is e.g. `1 day ago` → `1 day ago` — en-US short/narrow differ only
  for some units, e.g. `sec.`/`mo.`) **or** reject them with the shim
  error. Plan phase decides; silently rendering long is no longer
  acceptable.

Truly unknown keys (not defined by ECMA-402, e.g. `{ foo: 1 }`) remain
ignored, matching browsers.

### 3. `timeZone: "UTC"` support

- `timeZone: "UTC"` (exact string, case per spec is case-insensitive for
  "UTC" — accept `"utc"` variants uppercased) is accepted; all date/time
  component rendering for that formatter uses the `getUTC*` accessors
  (`getUTCFullYear`, `getUTCMonth`, `getUTCDate`, `getUTCDay`,
  `getUTCHours`, `getUTCMinutes`, `getUTCSeconds`).
- `resolvedOptions().timeZone` reports `"UTC"` when set. When not set,
  report the spec default — the host timezone name is not available to the
  shim, so report `"UTC"` only when explicitly set and omit the key
  otherwise (document this).
- Any other `timeZone` value throws the Req. 2 shim error.

### 4. Tests

- Extend `runtime/intl-shim.test.js`: per-constructor invalid-value cases
  (each supported option × one invalid value, asserting `RangeError`),
  the `TypeError` currency case, the `min > max` fraction-digit case, the
  invalid-unit `format()` case, unimplemented-option shim-error cases
  (at least `timeZoneName`, `notation`, `localeMatcher`), the
  `{ foo: 1 }`-ignored case, and `timeZone: "UTC"` rendering (a timestamp
  whose local and UTC renderings differ, asserted against the known UTC
  string; plus the lowercase `"utc"` acceptance).
- The exact friction-log probe: `day: ""` throws; `"numeric"` vs `""`
  produce *different* observable behavior (string vs throw).
- Existing valid-option tests stay green — validation must not change any
  currently-correct output.
- `cargo test --workspace` and `cargo run -p zero -- test` stay green;
  run `cargo test --workspace -- --include-ignored` before declaring done
  (showcase/examples may construct formatters).

### 5. Documentation

- `docs/testing.md` §Internationalization (lines ~336–365): state the new
  contract — invalid option values throw `RangeError` (browser-faithful),
  unimplemented options throw a clear shim error instead of being silently
  ignored, `timeZone: "UTC"` is supported, other zones are not. Keep the
  supported-option lists there as the single user-facing boundary.
- If the scaffold/showcase `AGENTS.md` web-platform notes mention `Intl`,
  mirror the one-line contract there (check during planning; the
  `agents-update` flow keeps these in sync).

### 6. Friction-log closeout

After landing, flip the `2026-06-06` 🟡 entry (#78) in the demo's
`FRAMEWORK_NOTES.md` to `- [x]` with a `**FIXED**` annotation, and verify
the demo's 3 residual `DateTimeFormat`-option lit_str survivors become
killable (`zero mutate` on the affected demo file after `zero update`).
Done in the demo repo, not this one — note it for follow-up.

## Constraints

- **No new dependencies, pure-JS shim only** — same as the original
  intl-shim item. Validation is hand-written allowed-value checks, not a
  spec-conformance library.
- **Construction-time throwing** where the spec throws at construction;
  `format()`-argument errors (`RelativeTimeFormat` unit/value) throw at the
  call, matching the spec.
- **No output changes for currently-valid inputs.** Every option combination
  that is valid today must format byte-identically after this change
  (the only behavior changes are new throws, the `min > max` clamp→throw,
  the `numeric`-coercion→throw, and the new `timeZone: "UTC"` path).
- **Locale stays accepted-and-ignored.** The en-US-only locale contract from
  the original spec is untouched — locale strings are *not* validated
  (browsers do validate BCP-47 tags, but the shim's documented departure
  stands; widening it is not this item).
- **JSDoc + ~80-line functions** per `CLAUDE.md`; one validation helper per
  concern (e.g. a shared `_requireOneOf(option, value, allowed)`), not a
  monolithic validator.
- **Update the stale Boa comments** in `intl-shim.js` while in the file
  (header + three constructor JSDoc blocks) — the GC rationale they cite
  was removed with Boa on 2026-05-31.

## Out of Scope

- Implementing any of the rejected options (`timeZoneName`, `notation`,
  significant-digits, `hourCycle`, …) — they throw, they don't grow support.
- Time zones other than UTC; any tz database.
- Locale validation or non-en-US output (the original item's documented
  limitation stands).
- `formatToParts` / `formatRange` and the other `Intl` namespaces
  (`Collator`, `PluralRules`, …) — still absent, still out of scope.
- `Date.prototype.toLocale*` / `Number.prototype.toLocaleString` wiring
  (Open Question 1 of the original spec; unchanged here).
- The production (browser) runtime — native `Intl`, unaffected.
- The "lint for opposite-direction effect pairs" and other unrelated
  friction-log items.

## Open Questions

1. **`RelativeTimeFormat` `style: "short"`/`"narrow"`** — implement the
   abbreviated en-US forms or reject with the shim error (Req. 2 leaves
   this to the plan phase). Check whether the demo/examples pass a non-long
   style before deciding; rejecting is cheaper and honest, implementing is
   ~20 lines of unit-abbreviation table.
2. **`resolvedOptions().timeZone` default** — Req. 3 proposes omitting the
   key when `timeZone` wasn't set (the shim can't know the host zone).
   Verify nothing in the demo/examples reads `resolvedOptions().timeZone`
   and breaks on `undefined`; if something does, `"UTC"`-always may be the
   lesser evil (document whichever wins).
3. **`hour12` interaction with validation** — the shim's current default is
   `hour12 !== false` (12-hour unless explicitly false) even for
   hour-only formats; the real en-US default is also 12-hour, so this
   should be unaffected — but the plan should add a regression test pinning
   it before touching `_formatTimePortion` for the UTC accessors.
