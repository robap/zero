---
title: Linting
nav_order: 12
---

# Linting

`zero lint` is a single command that runs every check the
framework enforces. There are no plugins, no config files, no
suppression directives. The rules are the contract.

## Running `zero lint`

```sh
zero lint                  # check every source file under the project
zero lint --quiet          # summary only
```

Output is one diagnostic per violation, formatted as
`<file>:<line>:<col>  <rule-id>  <message>`. Exit code:

- `0` — no violations.
- non-zero — one or more violations.

`zero lint` is intentionally exit-code-driven so CI catches drift
without parsing output. There is no `--fix` and no `// zero-lint
disable` — if a rule fires, the lint says you wrote something
inconsistent with the framework's idioms, and the answer is to
change the code rather than silence the rule.

## SCSS / design system rules

These rules cover styles under `<project-root>/styles/` and any
`.scss` files outside the framework-managed `.zero/` tree. They
exist so the design system stays the source of truth — every
hard-coded value the lint flags is a drift away from a token.

| Rule | Don't write                                              | Use instead                                                                                                       |
|------|----------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------|
| L01  | `font-weight: 600;`                                      | `font-weight: var(--weight-semi);` — see [Theming](./theming.html).                                               |
| L02  | `font-size: 0.875rem;`                                   | `font-size: var(--font-size-sm);`, or a `text-*` utility (`.text-small`, `.text-body`, `.text-h2`, …).            |
| L03  | `line-height: 1.4;`                                      | `line-height: var(--leading-snug);`                                                                               |
| L04  | `letter-spacing: 0.04em;`                                | `letter-spacing: var(--tracking-wide);`                                                                           |
| L05  | `background: #228be6;` / `color: red;`                   | Semantic color token — `var(--color-primary)`, `var(--color-danger)`, etc.                                        |
| L06  | `border-radius: 999px;` / `border-radius: 50%;`          | `border-radius: var(--radius-3xl);` (the pill step).                                                              |
| L07  | `border: 1px solid var(--color-border);`                 | `class="border"` (utility) or `border-width: var(--border-thin);`                                                 |
| L08  | `padding: 16px;`                                         | `padding: var(--space-md);` or `class="pad-md"`.                                                                  |
| L09  | `margin-top: 24px;`                                      | `margin-top: var(--space-lg);` (margin has no utility — prefer `gap` on the parent layout primitive).             |
| L10  | `gap: 8px;`                                              | `gap: var(--space-sm);` or `class="gap-sm"`.                                                                      |
| L11  | `.toolbar { display: flex; flex-wrap: wrap; gap: … }`    | `class="cluster gap-sm"` — see [Theming § Layout primitives](./theming.html#layout-primitives).                   |
| L12  | `align-items: center; justify-content: center;`          | Utility classes from `_alignment.scss`: `class="… align-center justify-center"`.                                  |
| L13  | `var(--radius-pill)` (renamed) / `var(--pad-sm)` (utility name, not a token) | The lint names the missing custom property. Either fix the typo, run `zero update`, or declare the token in `styles/app.scss`. |

## JS/TS framework idiom rules

These rules cover `src/**.{ts,js,tsx,jsx}`. The teaching chapter
that explains each underlying primitive is linked in the right
column.

| Rule | Trigger                                                                                                          | What it tells you                                                              |
|------|------------------------------------------------------------------------------------------------------------------|--------------------------------------------------------------------------------|
| R01  | `${signal.val}` inside an ``html\`...\``` template                                                               | Reading `.val` breaks reactivity — pass the signal. See [Reactivity](./reactivity.html#common-pitfalls). |
| R02  | `signal.val = ...`                                                                                               | Signals are immutable from the outside — use `.set()` / `.update()`. See [Reactivity](./reactivity.html#what-a-signal-is). |
| R03  | Module-level `signal()` / `computed()` / `effect()` (outside `src/stores/**` and `src/app.{ts,js,tsx,jsx}`)      | Leak — module-level reactives never dispose. Move into a function, the app entry, or a store factory. See [Reactivity § Ownership scopes & cleanup](./reactivity.html#ownership-scopes--cleanup). |
| T01  | `addEventListener` / `removeEventListener` in `src/{components,routes}/**`                                       | Use `@event=` bindings. See [Templates § Event binding](./templates.html#event-binding). |
| T02  | Unknown `@event.modifier`                                                                                        | Typo — the allowed set is in [Templates § Event modifiers](./templates.html#event-modifiers). |
| T03  | `each(items, render)` with no key function                                                                       | Pass a key fn for stable identity. See [Templates § each()](./templates.html#each--keyed-lists). |
| T04  | `document.querySelector` / `el.appendChild` etc. in `src/{components,routes}/**`                                 | Use `ref()` for element handles. See [Templates § ref()](./templates.html#ref--element-handles). |
| C01  | `class X { ... }` in `src/{components,routes}/**`                                                                | Components are plain functions. See [Components § Components are functions](./components.html#components-are-functions). |
| C02  | `customElements.define(...)`                                                                                     | Use the documented `'zero/wc'` escape hatch (deferred).                        |
| I01  | Bare specifier outside the allowlist (`zero`, `zero/components`, `zero/http`, `zero/test`)                       | No `node_modules` in zero. See [Getting Started](./getting-started.html).      |
| I02  | Relative import into `.zero/`                                                                                    | Import the public surface, not the managed copy.                               |
| S01  | Function body > 80 lines                                                                                         | Split into named helpers. Convention; no underlying primitive.                 |
| P01  | Parse error                                                                                                      | One diagnostic per parse failure (reserved).                                   |

## Test-file exemptions

`*.test.{ts,js,tsx,jsx}` and `*.spec.{ts,js,tsx,jsx}` files are
exempt from the **T-rules** and **R03**. Tests legitimately
reach into the DOM (`querySelector`-style assertions, custom
event dispatch helpers) and legitimately declare module-level
signals as test fixtures.

`R02`, `C01`, `C02`, `I01`, `I02`, and `S01` still apply in
tests — they're about correctness or code health, not about
framework-idiomatic UI code.

## Authoring posture

Three principles drive every choice above:

1. **No `--fix`.** A lint that auto-fixes invites mechanical
   patches that paper over a structural issue. If a rule fires,
   the answer is to change the code by hand, with the
   understanding of *why* it fires.

2. **No per-line disables.** There is no `// zero-lint disable`
   comment. If a rule's body is wrong for your code, the rule
   needs revising upstream — not silencing locally. (In
   practice, the rules survive review precisely because they
   don't have escape hatches; every drift gets surfaced.)

3. **No config knobs.** The rule set is the framework's
   opinion. Tuning each project to its own preference defeats
   the point of having a shared idiom in the first place. The
   rule is the contract; drift is the bug.

If you find a rule that's genuinely wrong, open an issue under
`issues/`. The rule definitions live in `crates/zero-lint/`.
