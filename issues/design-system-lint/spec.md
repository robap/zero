# Spec: Design-System Lint + AGENTS.md Negative Examples

## Problem Statement

An agent built a demo with zero and produced visually-plausible output that
bypassed most of the design system: raw `font-weight: 600`, `border-radius:
999px`, `font-size: 0.75rem`, raw `padding`/`border` values, and inline
flex/grid in place of layout primitives. The build was clean. Tests passed.
There was no feedback signal that any of this was wrong. The agent's own
post-mortem (`improved_agent_usage.md`) summarized it: "agents skim
documentation more aggressively and have stronger priors from generic web
work — if you want agents to use the framework's vocabulary, the framework
needs feedback loops that make non-framework code visibly wrong, not just
suboptimal."

The framework currently has the vocabulary (tokens, primitives, utilities,
typography utilities, components) and the documentation (`AGENTS.md`). It
does not have a feedback loop. Color tokens fared best — they're the one
category the agent did adopt — because `var(--color-*)` is the single
obvious shape. Everything else (radius, weight, size, spacing, layout
primitives) lost to muscle memory.

This spec adds the feedback loop:

1. A `zero lint` pass that flags raw values in user SCSS where the design
   system provides a token, utility, or primitive — and suggests the
   replacement by name.
2. A "Don't write X — use Y" section in the scaffolded `AGENTS.md` so that
   when the lint fires, the right answer is one paragraph away.
3. Token gaps the lint exposes (a wider radius scale) filled in the same
   ship.

## Background

### What the design system already ships

- **Color tokens.** Thirteen semantic `--color-*` tokens, theme-aware. Adopted
  cleanly by the agent. Not the problem.
- **Spacing scale.** `--space-{xs,sm,md,lg,xl}` plus `gap-{step}` and
  `pad-{step}` utility classes.
- **Radius.** `--radius-{sm,md,lg}`. Gap: no token for fully-rounded shapes
  (the agent reached for `999px` for a chip).
- **Type.** `--font-size-{sm,md,lg,xl}`, `--weight-{normal,medium,bold}`,
  `--leading-{tight,normal}`. No tracking tokens.
- **Borders.** `--border-{thin,md,thick}`, plus `border` and `border-{t,r,b,l}`
  utility classes.
- **Layout primitives.** `cluster`, `stack`, `frame`, `split`, `flank`, `grid`
  in `_layout.scss`. Cover ~all common flex/grid arrangements; the agent used
  raw `display: flex; gap: …` instead.
- **Typography utilities.** Twelve classes in `_typography.scss`
  (`text-display`, `text-h1`–`text-h4`, `text-eyebrow`, `text-body`,
  `text-small`, `text-muted`, `text-code`, `text-link`, `divider`). Cover the
  cases where the agent reached for raw `font-size`/`font-weight` combinations.

All of the above live in framework-owned partials under `.zero/styles/`.
User-authored styles in `styles/app.scss` (and any sibling SCSS files) are the
lint's target. Files under `.zero/` are out of scope — they declare the
tokens.

### What's already planned

`zero lint` is listed as unimplemented in `zero-framework-spec.md` §12
Phase 6. This spec defines its first rule set. Future Phase 6 work may add JS
lint rules under the same subcommand; nothing here forecloses that.

### Where AGENTS.md lives

`crates/zero-scaffold/src/scaffold/AGENTS.md` is the source of truth — it
ships into every new project via `zero init`. The "Don't write X" section
lives there.

## Requirements

### R1. `zero lint` subcommand

- `zero lint` runs the design-system lint over all user-authored SCSS and CSS
  in the project (everything under the project root except `.zero/`, `dist/`,
  `node_modules/`, and anything matched by `.gitignore` patterns).
- Exit code: `0` if no diagnostics, non-zero if any are emitted. Each
  diagnostic carries `file:line:col`, the offending property/value, and the
  suggested replacement.
- Output format: one diagnostic per line in the same shape as `zero test`
  failure locations (path + caret-pointed snippet). `--quiet` suppresses the
  snippets and prints one line per diagnostic; `--verbose` is the default.
- No autofix in v1. The diagnostic names the fix; the user applies it.

### R2. Raw-value rules

Each rule flags a property/value pattern in user SCSS and names the
replacement. All apply only to user files — framework partials under
`.zero/` are whitelisted unconditionally.

| # | Rule | Triggers on | Suggests |
| --- | --- | --- | --- |
| L01 | `font-weight` literal | numeric (`400`, `600`, …) or `bold`/`normal` keyword | `var(--weight-{normal,medium,bold})` |
| L02 | `font-size` literal | any `px`/`rem`/`em`/unitless number | nearest `var(--font-size-*)` plus "consider a `text-*` utility for body/heading text" |
| L03 | `line-height` literal | any numeric value | nearest `var(--leading-*)` |
| L04 | `letter-spacing` literal | any numeric value | nearest `var(--tracking-*)` (new token family — see R4) |
| L05 | `color` / `background` / `background-color` / `border-color` / `fill` / `stroke` / `outline-color` literal | hex, `rgb()`, `hsl()`, named color | nearest `var(--color-*)` semantic token. Framework partials whitelisted; CSS keywords `currentColor`, `inherit`, `transparent`, `initial`, `unset` allowed. |
| L06 | `border-radius` literal | any numeric value | nearest `var(--radius-*)` token; if value ≥ 999px or `50%`, suggest the widest scale step (see R4) |
| L07 | `border-width` literal, or `border`/`border-{side}` shorthand with numeric width | any numeric value | `var(--border-{thin,md,thick})` or the `border` / `border-{t,r,b,l}` utility class |
| L08 | `padding` / `padding-{side}` / `padding-{block,inline}` literal | any numeric value | `var(--space-*)` or the `pad-{step}` utility |
| L09 | `margin` / `margin-{side}` / `margin-{block,inline}` literal | any numeric value | `var(--space-*)`. (No utility for margin — the primitives use gap.) |
| L10 | `gap` / `row-gap` / `column-gap` literal | any numeric value | `var(--space-*)` or `gap-{step}` utility |

**Nearest-token logic.** For each numeric value, the rule resolves it against
the token scale by closest match and names the resulting token. If the value
is exactly between two steps, prefer the smaller. If the value is outside
the scale's range, suggest the nearest endpoint and note "outside the
scale" so the developer can decide whether to add a token or live with the
choice.

**Whitelist.** `0`, `0%`, `auto`, `none`, `inherit`, `initial`, `unset`,
`currentColor`, `transparent`, and any value that is already a
`var(--…)` reference. CSS calc expressions whose operands are all tokens or
zero (`calc(var(--space-md) + 4px)`) are flagged — using `calc` to mix raw
values back in is the same failure mode and deserves the diagnostic.

### R3. Layout-primitive suggestion rule (L11)

When user SCSS declares a rule whose body matches the shape of a layout
primitive, suggest the primitive class instead.

Detection patterns (must match all listed declarations in a single rule
body, regardless of order):

| Primitive | Body matches |
| --- | --- |
| `cluster` | `display: flex` + `flex-wrap: wrap` + (any `gap`) |
| `stack` | `display: flex` + `flex-direction: column` + (any `gap`) |
| `split` | `display: flex` + `justify-content: space-between` + (any `gap`, optional) |
| `flank` | `display: flex` + child selectors that set `flex: 0 0 auto` / `flex: 1` (heuristic — keep narrow to avoid noise) |
| `grid` | `display: grid` + `grid-template-columns: repeat(auto-fit, minmax(…, 1fr))` |
| `frame` | `aspect-ratio: <any>` + `overflow: hidden` |

Diagnostic names the primitive class and the AGENTS.md sub-section
`### When to reach for which primitive` (defined in R5a) so the developer
sees the canonical intent, not just the class name. Detection is conservative — when in doubt,
do not flag. False positives are worse than false negatives here because the
agent's failure mode was not knowing primitives existed, not knowing when to
use them.

### R4. Token additions

Add the following to `.zero/styles/_tokens.scss`. These fill gaps the lint
rules surface; without them the lint cannot name a replacement for the
agent's worst offenders.

- **Radius scale extended:** `--radius-xs`, `--radius-sm` (existing),
  `--radius-md` (existing), `--radius-lg` (existing), `--radius-xl`,
  `--radius-2xl`, `--radius-3xl`. The largest step is sized to render any
  pill/fully-rounded shape on common element heights (e.g. `9999px`); naming
  follows the existing scale, not semantic aliases. The lint's "border-radius
  ≥ 999px" branch suggests `--radius-3xl`.
- **Tracking scale (new family):** `--tracking-tight`, `--tracking-normal`,
  `--tracking-wide`. The shipped `.text-eyebrow` utility is migrated to read
  `var(--tracking-wide)` instead of its current inline value.

Token tables in `crates/zero-scaffold/src/scaffold/AGENTS.md` are updated to
list the new tokens. The framework spec (§7.1) is updated to match.

### R5. AGENTS.md negative-examples section

A new section, `## Common mistakes (the lint will catch these)`, is added to
`crates/zero-scaffold/src/scaffold/AGENTS.md`. Structure:

- One short paragraph framing: "These are the patterns the design system
  replaces. `zero lint` flags them; this section is the answer."
- A short table or bullet list with one row per lint rule (L01–L11), each
  shaped as `Don't write: <example>; Use: <replacement>`.
- A pointer to the spec file and to the layout primitives / typography
  utilities sections elsewhere in AGENTS.md.

The section is placed after `## Styles → Design system` so the negative
examples sit next to the positive ones.

### R5a. Layout-primitive canonical use cases

A new sub-section, `### When to reach for which primitive`, is added under
`## Styles → Design system → Layout primitives` in
`crates/zero-scaffold/src/scaffold/AGENTS.md`. The goal is to shift the
agent's default away from raw `display: flex` by giving each primitive a
named canonical case — not a definition, an *intent*. The text below is the
baseline (plan phase may tighten the language but should keep the shape: one
line per primitive, naming a concrete UI pattern):

| Primitive | Reach for it when… |
| --- | --- |
| `cluster` | Default choice for any horizontal layout. Wraps for free at narrow widths — use it whenever you'd otherwise reach for `display: flex` on a row of items (toolbars, button groups, chip lists, tag rows, inline metadata). |
| `stack` | Vertical layout where items should *not* spread horizontally — a form body, a card's contents, a sidebar's list of links. (For headers and footers where the row should span full width, prefer `split` or `flank`.) |
| `split` | Two end-anchored groups separated by stretched whitespace. Canonical case: page header with brand on the left and nav/actions on the right. Anywhere `justify-content: space-between` would have been the answer. |
| `flank` | A fixed-size element next to a flexible one. Canonical case: a form row with a label on one side and an input that fills the rest; also media objects (avatar + comment body), inline icons next to flowing text. |
| `grid` | A repeating column layout that auto-fits — card grids, tile lists, dashboard widgets. Override `--grid-min` to tune the breakpoint. Not for two-column page layouts (use `flank`). |
| `frame` | Fixed-aspect-ratio media boxes — video embeds, image thumbnails, hero art. Override `--frame-ratio` to change the ratio. |

The same table is mirrored into the framework spec (`zero-framework-spec.md`
§7.1, under "Layout primitives") so both audiences see the same canonical
intent.

The L11 diagnostic from R3 references this sub-section by name so the lint
output, the AGENTS.md table, and the framework spec all point at one
authoritative phrasing.

### R6. Integration with existing commands

- `zero lint` is its own subcommand; the existing `zero build` and `zero
  dev` pipelines do not run it. (Failing user CSS would block builds — too
  aggressive for v1.) The exit code of `zero lint` is the integration point
  for CI.
- The negative-examples AGENTS.md section ships through `zero init` like
  every other scaffold file; no separate publishing step.
- Token additions ship through `zero update` — existing projects refresh
  `.zero/styles/_tokens.scss` when they run it.

### R7. Tests

- Per-rule SCSS fixtures (one positive case + one negative case per rule)
  under the lint crate's test directory.
- An integration test that runs `zero lint` against `examples/tracker/web`
  and `showcase/` and asserts the diagnostic count for each (both should
  currently be zero, modulo whatever the audit surfaces — see Open Questions).
- An integration test that runs `zero lint` against a fixture project
  containing the exact patterns from `improved_agent_usage.md` and asserts
  the expected diagnostics fire.

## Constraints

- **SCSS-only in v1.** The lint reads `.scss` and `.css` source files. It does
  not parse `html\`\`` templates to inspect inline `style="…"` attributes,
  even though those are also possible failure sites. Inline styles are a
  follow-up; the agent's failures were all in `.scss`.
- **No autofix.** The diagnostic names the fix; the user applies it. Autofix
  is a future addition once the rules have settled.
- **Zero npm dependencies.** Implementation lives in a Rust crate (likely a
  new `zero-lint` crate or an extension of `zero-sass`) reusing the SCSS
  parser already in the workspace. No external linters (Stylelint, etc.).
- **Conservative on layout-primitive detection (L11).** False positives push
  users to suppress the lint or distrust it. Each pattern in the table must
  match all listed declarations exactly; partial matches do not fire.
- **Whitelist `.zero/`, `dist/`, vendored assets.** Framework partials
  declare the tokens (`--space-md: 1rem`) and would otherwise trigger every
  rule. Vendored assets are out of scope by definition.
- **Token additions stay on the existing naming scale.** New radius steps
  follow `--radius-{xs,sm,md,lg,xl,2xl,3xl}`. No semantic aliases like
  `--radius-pill` or `--radius-full`. Same for tracking
  (`--tracking-{tight,normal,wide}`), not `--tracking-eyebrow`.
- **AGENTS.md updates land at the scaffold source** —
  `crates/zero-scaffold/src/scaffold/AGENTS.md`. Existing projects refresh
  via `zero update`. The repo-root `BEST_PRACTICES.md` is unchanged in this
  spec; if it ends up duplicating content, a follow-up can dedupe.

## Out of Scope

- **JS lint rules under `zero lint`.** This spec ships SCSS rules only.
- **Autofix / `zero lint --fix`.** Diagnostic-only in v1.
- **Inline-`style` attribute checking.** Tagged-template parsing is its own
  effort; deferred.
- **Reframing AGENTS.md as "utilities by default."** The negative-examples
  section gives concrete don't/use pairs; the broader docs rewrite the agent
  suggested (make typography utilities the default route in prose) is not in
  this spec. Negative examples carry the same signal at lower cost.
- **Inlining full token tables with values into AGENTS.md.** The current
  AGENTS.md already lists token *names* and utility-class names; the lint
  diagnostic names the replacement at the call site. Inlining values
  duplicates `_tokens.scss` and rots whenever the values move.
- **Tooling for tracking which rules an agent triggers.** Telemetry is a
  separate concern.

## Open Questions

1. **Where does the lint crate live?** Two reasonable choices: extend
   `zero-sass` (already has a SCSS parser) with a `lint` module, or stand up
   a new `zero-lint` crate that consumes the parser. The plan phase should
   pick based on how cleanly the parser exposes an AST suitable for rule
   matching.
2. **Diagnostic output format machine-readable variant?** `zero test` writes
   `coverage/coverage.json` and `mutation/mutation.json`; should `zero lint`
   emit `lint/lint.json` (for CI integration and an eventual editor LSP)?
   Probably yes but spec doesn't require it. Plan phase decides.
3. **Nearest-token logic — exact thresholds.** "Closest match" is well-defined
   for numbers but the rounding behavior at midpoints, and the handling of
   values far outside the scale, deserve worked examples. Plan phase nails
   this down with a table of (input → suggested token) cases.
4. **What does an audit of `examples/tracker` and `showcase/` surface
   today?** Running the lint against the in-repo example apps may reveal
   that the framework's own examples violate rules. Those should be fixed
   in-scope so the integration test in R7 starts at zero diagnostics.
5. **L11 (primitive detection) — is the heuristic for `flank` workable?**
   The pattern is intrinsically harder to detect than the others (depends on
   child selectors, not just a property on the parent). If it proves noisy
   in the audit, drop it from v1 and keep the other five.
6. **`--tracking-*` values.** Sass mixin `.text-eyebrow` currently inlines a
   tracking value; the migration pinpoints the canonical "wide" value but
   the "tight"/"normal" values need numbers picked. Plan phase chooses.
