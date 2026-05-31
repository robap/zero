# Spec: `Intl` shim for the test runtime

## Problem Statement

`Intl` is undefined in the `zero test` runtime. Any module that does
`new Intl.DateTimeFormat(...)`, `new Intl.NumberFormat(...)`, or
`new Intl.RelativeTimeFormat(...)` at top level throws
`ReferenceError: Intl is not defined` the moment a test imports it — before
the test body runs. Locale-aware date and number formatting is a common reach
for UI code (the demo wanted `MMM D, h:mm A` for transaction timestamps, and
formats inventory dollar amounts), so this gap forces every adopter to either
guard every `Intl` use with `typeof Intl === "undefined"` and a hand-rolled
fallback, or defer all formatting to runtime-only paths that tests can't
exercise. It was logged as friction-log entry `2026-05-24` 🟡 in the demo's
`FRAMEWORK_NOTES.md`.

The framework's whole verification rhythm (test + coverage + mutate) depends on
UI code being importable under test. A formatting helper that can't be imported
is a hole in that rhythm.

## Background

### Why `Intl` is missing

The test harness runs on Boa 0.21.1, configured in the workspace `Cargo.toml`
with `features = ["annex-b"]` only. Boa gates its `Intl` implementation behind
a separate `intl` feature that pulls in the ICU/CLDR crates. That feature is
not enabled, so Boa provides no `Intl` global. Boa implements ECMAScript; it
provides nothing from the Web Platform or `Intl` on its own.

Enabling Boa's `intl` feature was considered and **rejected** for this item (see
Constraints): it adds ICU crate dependencies and binary weight, contradicting
the framework's hand-written, zero-extra-dependency shim philosophy. This item
takes the same path the Web Platform surface took (see
`issues/web-platform/spec.md`): a hand-written pure-JS shim, minimum viable
behavioral contract, explicit boundary.

### How shims are loaded

Web Platform shims are pure-JS script bodies (no `import` / `export`) that rely
on globals being installed onto `globalThis`. `crates/zero-runtime/build.rs`
concatenates `dom-shim.js` followed by an ordered `WEB_PLATFORM_FILES` list —
`fetch-shim.js`, `url-shim.js`, `encoding-shim.js`, `binary-shim.js`,
`clone-shim.js` — into a single `zero_dom_shim_body.js` blob. The test harness
(`crates/zero-test-runner/src/harness.rs::eval_dom_shim`) evaluates that blob as
a script before any user module runs. Each shim file is the model to follow:
`runtime/encoding-shim.js` (`TextEncoder` / `TextDecoder`) is the closest
analog — a self-contained constructor pair installed onto `globalThis`.

### Shim authoring conventions (from existing shims)

- Pure script body, no `import` / `export`; symbols are in scope post-concat.
- Install the global guarded: `if (typeof globalThis.Intl === "undefined") ...`.
- Full JSDoc annotations on every function, per `CLAUDE.md`.
- Functions under ~80 lines.
- **Boa MapLock finalizer constraint** (`boa_maplock_finalizer` memory): keep
  keyed / code-path-variant branches in their own named functions rather than
  large multi-branch closures, to avoid the Boa GC teardown panic.

### Scope decisions already made (resolved with the user)

- **Approach**: hand-written JS shim (`runtime/intl-shim.js`), not Boa's `intl`
  feature.
- **Constructors**: `Intl.DateTimeFormat`, `Intl.NumberFormat`, and
  `Intl.RelativeTimeFormat` — all three.
- **Locale handling**: accept any locale argument without error, but always
  produce `en-US` output. Non-`en-US` locales are silently formatted as
  `en-US`. This is a documented known limitation, *not* a thrown error — the
  shim never throws on an unsupported locale.

## Requirements

### 1. New shim file

- Add `runtime/intl-shim.js`, a pure script body following the
  `encoding-shim.js` conventions above.
- Register it in `crates/zero-runtime/build.rs` `WEB_PLATFORM_FILES` so it is
  concatenated into `zero_dom_shim_body.js` and evaluated before user modules.
  Order it after the shims it might depend on (it is self-contained, so order
  is not behaviorally critical, but keep the list deterministic).
- Install `globalThis.Intl` (and mirror onto `window.Intl` if the other shims
  follow that pattern) guarded by `typeof globalThis.Intl === "undefined"`.

### 2. `Intl.DateTimeFormat`

- Constructor `new Intl.DateTimeFormat(locales?, options?)`. `locales` accepted
  and ignored (always en-US).
- `format(date?)` returns a string. Accepts a `Date`, a timestamp number, or
  `undefined` (now).
- Support the option families the framework/examples reach for:
  - `dateStyle` / `timeStyle`: `"full" | "long" | "medium" | "short"`.
  - Explicit component options: `year`, `month` (incl. `"short"` →
    `Jan`…`Dec`, `"long"`, `"numeric"`, `"2-digit"`), `day`, `hour`, `minute`,
    `second`, `weekday`, `hour12`.
- Output must match real en-US `Intl.DateTimeFormat` output for the supported
  option combinations (verified against a reference, e.g. Node, in tests).
  The friction trigger `MMM D, h:mm A` (`{ month: "short", day: "numeric",
  hour: "numeric", minute: "2-digit", hour12: true }`) must produce the
  correct en-US string.
- `formatToParts(date?)` is **out of scope** unless an audited caller needs it
  (none known); omit it rather than stub it half-way.
- `resolvedOptions()` returns an object reporting the resolved options
  including `locale: "en-US"`, so code that introspects the formatter doesn't
  break.

### 3. `Intl.NumberFormat`

- Constructor `new Intl.NumberFormat(locales?, options?)`. `locales` accepted
  and ignored (always en-US).
- `format(value)` returns a string.
- Support:
  - `style`: `"decimal"` (default), `"currency"`, `"percent"`.
  - `currency` (e.g. `"USD"` → `$`) and `currencyDisplay` at least for the
    common symbol case.
  - `minimumFractionDigits` / `maximumFractionDigits`.
  - `useGrouping` (thousands separators, default on for the supported styles).
- Output must match real en-US `Intl.NumberFormat` for supported combinations.
- `resolvedOptions()` returns the resolved options with `locale: "en-US"`.

### 4. `Intl.RelativeTimeFormat`

- Constructor `new Intl.RelativeTimeFormat(locales?, options?)`. `locales`
  accepted and ignored.
- `format(value, unit)` returns a string for the standard units (`"second"`,
  `"minute"`, `"hour"`, `"day"`, `"week"`, `"month"`, `"quarter"`, `"year"`).
- Support `options.numeric` (`"always"` | `"auto"`) and `options.style`
  (`"long"` | `"short"` | `"narrow"`) at least for `"long"`/`"always"`, with
  en-US output matching the real API (e.g. `format(-1, "day")` → `"1 day ago"`;
  with `numeric: "auto"`, → `"yesterday"`).
- `resolvedOptions()` returns the resolved options with `locale: "en-US"`.

### 5. Tests

- Add a `runtime/intl-shim.test.js` (or fold into the existing
  `web-platform.test.js`) covering each constructor's supported option matrix,
  asserting against known-correct en-US strings.
- Include the exact friction-trigger case (`MMM D, h:mm A`).
- Cover the locale-ignored behavior: a non-en-US locale produces en-US output
  and does not throw.
- The full suite (`cargo test --workspace -- --include-ignored`) and the JS
  runtime suite (`cargo run -p zero -- test`) stay green.

### 6. Documentation

- Note `Intl` (the three constructors, en-US-only, the unsupported-locale
  behavior, and which options are covered) in the testing / web-platform
  surface docs (`docs/testing.html` source, and wherever the Web Platform
  audited list lives — `zero-framework-spec.md` if that's the canonical list).
  The point is the boundary is explicit: users learn what's covered without
  rediscovering the edge by failure.

### 7. Friction-log closeout

- After landing, flip the `2026-05-24` 🟡 `Intl` entry in the demo's
  `FRAMEWORK_NOTES.md` to `- [x]` with a `**FIXED**` annotation (done in the
  demo repo, not this one — note it for follow-up).

## Constraints

- **No new Rust dependencies.** Do not enable Boa's `intl` feature or add ICU
  crates. Pure-JS shim only.
- **Hand-written, minimum viable.** Match the behavioral contract discipline of
  the Web Platform shims: implement what the framework, examples, and a
  reasonable user reach for — not the full `Intl` spec. Locale data is en-US
  only by design.
- **No silent locale error, but a documented silent-wrong.** Per the user
  decision, unsupported locales format as en-US rather than throwing. This is
  the one deliberate departure from the web-platform "intentional stub throws a
  clear error" contract; it must be called out in docs as a known limitation.
- **Boa GC safety.** Follow the `boa_maplock_finalizer` rule (branch-per-
  function) so the shim doesn't reintroduce the MapLock teardown panic.
- **JSDoc + function-length** per `CLAUDE.md`.

## Out of Scope

- Boa's `intl` feature / real ICU locale data.
- Any locale other than en-US producing locale-correct output.
- `Intl.DateTimeFormat.prototype.formatToParts` /
  `NumberFormat.prototype.formatToParts` / `formatRange` /
  `formatRangeToParts` unless a concrete audited caller needs them.
- Other `Intl` namespaces: `Intl.Collator`, `Intl.PluralRules`,
  `Intl.ListFormat`, `Intl.Segmenter`, `Intl.Locale`,
  `Intl.DisplayNames`, `Intl.supportedValuesOf`. Reaching for any of these
  yields a standard "not a function / undefined" error; document that the
  surface is the three constructors only.
- `Date.prototype.toLocaleString` / `toLocaleDateString` /
  `toLocaleTimeString` and `Number.prototype.toLocaleString` — these route
  through `Intl` in real engines but are a separate wiring task. Flagged in
  Open Questions; not committed here unless trivial to delegate.
- The production (browser) runtime, which has the native `Intl` and is
  unaffected.

## Open Questions

1. **`toLocale*` prototype methods.** Should the shim also wire
   `Date.prototype.toLocaleString` / `toLocaleDateString` /
   `toLocaleTimeString` and `Number.prototype.toLocaleString` to delegate
   through the new shim (they're a common alternate reach and Boa may provide
   only crude fallbacks)? Cheap if it's a thin delegation; the plan phase
   should check what Boa already returns for these and decide. Default: skip
   unless trivial.
2. **`Intl` membership in the Web Platform audited list.** The web-platform
   spec drew a *closed* enumerated surface and put future additions behind a
   spec (this one). Confirm where the canonical list lives
   (`zero-framework-spec.md`) and append the three `Intl` constructors there so
   the boundary stays single-sourced.
3. **Exact option matrix to guarantee.** The Requirements name the common
   options; the plan phase should pin the precise supported set per constructor
   (and what an *unsupported* option does — ignore vs. best-effort) against the
   examples/demo so the test matrix is concrete rather than open-ended.
