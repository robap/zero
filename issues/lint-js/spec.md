# Spec: JS/TS framework-idiom lint

## Problem Statement

Agents writing zero apps drift from the framework's idioms in ways that
none of the existing feedback loops catch. `zero test` checks runtime
correctness, `zero build` catches import / bundler errors, `zero lint`
covers SCSS design-system conformance — but nothing flags the
high-frequency JS/TS anti-patterns that quietly produce non-reactive UI,
leaked subscriptions, or class-based components. The agent's code looks
plausible, the dev server starts, and the only signal that something is
wrong is a stale render that no one notices until a human reads the
diff.

This item adds a JS/TS pass to `zero lint` whose sole purpose is to give
agents (and humans) immediate, source-mapped feedback that they have
written non-zero code. It is a conformance gate, not a style checker —
each rule maps one-to-one to a documented framework rule from the spec
or `BEST_PRACTICES.md`.

## Background

### Existing lint architecture

`crates/zero-lint/` is a library-only crate consumed by the
`zero lint` subcommand (`crates/zero/src/cmd/lint.rs`). The current
shape:

- `lint_project(root)` walks the project, runs rules, returns
  `Vec<Diagnostic>` sorted by `(file, line, column, rule)`.
- `Diagnostic` is `{ rule: &'static str, file, line, column, property,
  value, message }`.
- Rule modules under `crates/zero-lint/src/rules/` are pure functions
  over a `Decl` / `RuleBody` stream produced by `scan::scan(source)`
  (hand-rolled SCSS scanner; intentionally not a full parser).
- Rule IDs use the `L01`..`L11` namespace.
- Output format: `path:line:col  RuleID  property: value — message`,
  plus a caret-pointed source snippet by default (suppressed with
  `--quiet`).
- Any diagnostic fires exit code 1. No severity tiers, no
  per-line suppression directives.

### Already-available infrastructure

- SWC is wired through `crates/zero-transpile/` and used by the dev
  server, bundler, and test runner. The same crate is the natural home
  for an AST-walking pass over JS/TS source.
- File-walking for `src/**` (excluding `.zero/`) is straightforward —
  the bundler already does it for module graph discovery.

### Why purely syntactic

T-rules need to detect `addEventListener` calls and `document.*` access
inside components / routes. Determining whether the enclosing function
"is a component" via flow analysis (does it return an `html\`\``?) is
strictly more precise but introduces a class of subtle bugs and edge
cases. The chosen approach matches by file location
(`src/components/**`, `src/routes/**`) and the syntactic pattern.
False positives in unusual code are an accepted trade for predictable,
debuggable rules.

## Requirements

### R1 — Rule set (v1)

The pass MUST implement these ten rules. IDs are stable and namespaced
by category to avoid colliding with the existing `L01`..`L11` SCSS
namespace.

**Reactivity**

- **R01** — `${signal.val}` inside an `html\`\`` tagged template.
  Message: "reading `.val` inside a template breaks reactivity — pass
  the signal itself: `${name}` not `${name.val}`."
  Detection: walk every tagged template expression whose tag identifier
  is `html`; inspect each `${...}` substitution; flag member-expression
  reads whose property name is `val` where the object is an identifier.
- **R02** — direct assignment to `signal.val` (`count.val = 5`,
  `count.val += 1`, etc.).
  Message: "signals are immutable from the outside — use `.set(...)` or
  `.update(fn)`."
  Detection: assignment whose left-hand side is a member expression
  with property `val`. To avoid false positives, only fire when the
  enclosing file imports an identifier from `"zero"` / `"z"` (the
  framework binding for `signal`).
- **R03** — module-level `signal(...)` / `computed(...)` / `effect(...)`
  call.
  Message: "creating reactive primitives at module scope leaks — they
  have no owning component scope. Move into a function or store
  factory."
  Detection: call expression at program top level (not inside any
  function, arrow, or class method) whose callee identifier is one of
  `signal`, `computed`, `effect` and was imported from `"zero"` /
  `"z"`. Allowed: under `src/stores/**` (the canonical store-factory
  location pattern from `BEST_PRACTICES.md`).

**Template / event**

Applies to files under `src/components/**` and `src/routes/**`.

- **T01** — `addEventListener` / `removeEventListener` call.
  Message: "use the `@event=` syntax inside `html\`\`` — direct
  `addEventListener` bypasses scope cleanup."
- **T02** — unknown `@event.modifier` inside an `html\`\`` template.
  Allowed modifiers: `prevent`, `stop`, `once`, `self`, `passive`,
  `capture`, plus key filters `enter`, `escape`, `space`, `tab`,
  `up`, `down`, `left`, `right`, `delete`, plus `throttle`, `debounce`.
  Message: "unknown event modifier `.<name>` — see §3 'Event Handling'
  in the spec for the supported set."
  Detection: scan template static parts for `@<word>(\.\w+)+=`;
  validate every dotted segment against the allowed set.
- **T03** — `each(signal, fn)` call with no third argument when the
  rendered items appear to be objects.
  Message: "`each()` without a key function falls back to index-based
  reconciliation — pass a `keyFn` (`each(items, render, item => item.id)`)
  for stable identity."
  Detection: call expression to identifier `each` (imported from
  `"zero"` / `"z"`) with exactly two arguments.
- **T04** — `document.querySelector` / `document.getElementById` /
  `document.querySelectorAll` / `el.appendChild` etc. inside a
  components / routes file.
  Message: "direct DOM access inside a component bypasses the
  reactivity system — use `ref()` for element handles."
  Detection: member access whose object chain begins with the global
  `document` identifier, or a method-call on a value whose callee name
  is in `{appendChild, removeChild, insertBefore, replaceChild,
  innerHTML}`; the latter is a heuristic and only fires when the
  receiver is not a `ref` member access.

**Component model**

Applies to files under `src/**`.

- **C01** — `class Foo extends ...` declared in a components / routes
  file.
  Message: "components are plain functions — no class-based components
  in zero. See §3 'Component Model'."
  Detection: any `ClassDeclaration` or `ClassExpression` inside
  `src/components/**` or `src/routes/**`.
- **C02** — `customElements.define(...)` call outside the documented
  escape hatch (`"z/wc"`).
  Message: "register web components only via `import { define } from
  'z/wc'` — see §11 'Web Component Interop'."

**Imports**

Applies to files under `src/**`.

- **I01** — import from `"node:*"`, bare npm package specifiers
  (anything not starting with `.`, `/`, `z`, or `zero`), or
  `"npm:*"`.
  Message: "zero has no node_modules — `<specifier>` is not part of the
  framework runtime."
  Detection: every `ImportDeclaration` / dynamic `import("...")` whose
  specifier doesn't match the allowed prefixes.
- **I02** — relative import that climbs into `.zero/` (e.g.
  `import "../.zero/..."`, `import "../../.zero/..."`).
  Message: "`.zero/` is framework-owned — import from the public
  surface (`'z'`, `'zero/test'`, `'zero/http'`, `'zero/components'`,
  `'zero/wc'`)."
  Detection: import specifier whose resolved path lands under
  `.zero/`.

**Size**

- **S01** — function (declaration / expression / arrow) whose body
  exceeds 80 lines, counted from opening to closing brace inclusive.
  Message: "function `<name>` is `<n>` lines — zero targets <= 80 (see
  CLAUDE.md). Split into named helpers."
  Detection: any `FunctionDeclaration` / `FunctionExpression` /
  `ArrowFunctionExpression` / `MethodDefinition` whose body span,
  measured in source lines, exceeds 80. Applies to all files under
  `src/**`.

### R2 — Integration

- Same subcommand: `zero lint`. No new top-level command.
- Same `Diagnostic` shape. `property` and `value` fields are
  repurposed: `property` carries the offending construct's text (e.g.
  `addEventListener`, `count.val`), `value` carries a short
  category label (e.g. `template`, `assignment`, `import`) or is empty.
  The existing `write_diag` helper renders this correctly without
  changes.
- Same output format and caret snippet.
- Same exit behavior (any diagnostic → exit 1). No per-rule severity,
  no suppression comments.
- Same `--quiet` semantics (suppress snippet line + caret).
- JS/TS diagnostics interleave with SCSS diagnostics in the existing
  `(file, line, column, rule)` sort order.

### R3 — File discovery

- Walk every `.ts` / `.js` / `.tsx` / `.jsx` file under the project's
  `src/` directory.
- Skip `.zero/`, `node_modules` (defensive — should not exist),
  `dist/`, and the configured build output directory.
- Skip files matching `*.test.{ts,js}` and `*.spec.{ts,js}` — tests
  legitimately reach for direct DOM access, addEventListener, and
  module-level signals as fixtures. Tests are still covered by R02 /
  C01 / C02 / I01 / I02 / S01.

### R4 — Parsing

- Parse via SWC through `zero-transpile`. Add a thin entry point that
  returns an AST module rather than transpiled code; reuse the existing
  parser configuration (TS, no JSX, no decorators).
- Parser failures surface as a single diagnostic per file with a
  reserved rule ID `P01` (parse error) and the SWC error's line /
  column / message. The lint pass continues to the next file.
- Template-string content (T02) is scanned with a hand-rolled regex
  walk over the static parts of the tagged template, mirroring the
  pragmatism of the SCSS scanner. No full HTML parser.

### R5 — Tests

- One unit test per rule under `crates/zero-lint/src/rules/` covering
  at minimum: one positive case (rule fires with the expected
  `Diagnostic`), one negative case (rule does not fire on an adjacent
  valid pattern).
- Integration test under `crates/zero/tests/` that drives `zero lint`
  end-to-end against a fixture project with intentional violations and
  asserts the rendered output verbatim (rule ID, position, snippet).
- One real-world fixture: lint the three shipped examples (`counter`,
  `todos`, `tracker`) and the `showcase/` project. They MUST come up
  clean. Any diagnostic that fires is either a real bug in the example
  (fix it) or a false positive in the rule (fix the rule). This
  becomes a regression test.

### R6 — Documentation

- Add a `## JS/TS lint` section under §1 of `zero-framework-spec.md`
  enumerating the rule IDs and one-line descriptions.
- Append to `src/scaffold/AGENTS.md` the same rule table — agents
  reading the scaffold's AGENTS.md learn the rules before they write a
  line.
- Mark the v1 rules as a new Phase entry (Phase 14 — JS/TS framework
  lint) in §12 with checkboxes per rule.

## Constraints

- **No new top-level dependencies.** SWC is already vendored through
  `zero-transpile`; no new crates.
- **Performance.** Lint must complete in under one second on
  `examples/tracker` (the largest shipped example). This is well within
  reach for a single-pass AST walk over a small file set.
- **Determinism.** Output ordering is deterministic on identical input,
  matching the existing SCSS lint sort.
- **No editor integration in v1.** LSP / VSCode plugin are out of
  scope. The CLI is the surface.
- **No autofix in v1.** Diagnostics describe the problem and point at
  the rule; the fix is the developer's. Autofix is plausible later for
  R01 (`${x.val}` → `${x}`) and T03 but adds complexity disproportionate
  to v1 value.
- **No per-line suppression in v1.** If a rule produces false positives
  in practice, the rule is wrong — fix the rule. This matches the
  existing SCSS lint contract.
- **Test files are exempt from T-rules and R03**, not from all rules.
  See R3.
- **Single severity.** Every diagnostic exits 1.

## Out of Scope

- **JSDoc completeness rules** (D01, D02). Considered for v1 but
  deferred — likely noisy on a first pass; would benefit from a
  separate spec that addresses scaffold defaults and a one-shot
  bootstrap fix.
- **Full TypeScript type checking** (`zero check`). This spec adds
  *idiom* rules, not type-shape checking. They are complementary; a
  follow-on item.
- **Cross-file analysis.** All rules are single-file. No "this signal
  escapes the module" or "this component is never imported" rules.
- **Flow analysis.** No "does this function return `html\`\``"
  inference. T-rules fire purely on file-location + syntactic pattern.
- **Editor integration / LSP / language server.** CLI only.
- **Autofix.**
- **Per-line / per-file disable directives.**
- **Configuration.** No `zero.toml` knobs for enabling / disabling
  rules, no severity overrides. Opinionated, zero-config.
- **SCSS rule expansion.** This spec only adds JS/TS rules; the
  existing `L01`..`L11` SCSS rules are unchanged.

## Open Questions

1. **R03 store-factory escape.** The spec exempts `src/stores/**` from
   R03 (module-level reactive primitives). Should this be configurable,
   or is the directory name load-bearing? Plan should decide: hard-code
   the path, or add a single `[lint]` toggle in `zero.toml`. Current
   bias: hard-code `src/stores/**` and let the scaffold document it.
2. **C01 in `src/stores/**` / `src/lib/**`.** Class declarations
   *outside* component/route directories are allowed by the spec as
   written. Confirm: is the rule "no classes in zero apps, full stop"
   (broader) or "no class components" (narrower, the current text)?
   Plan should pick one and reflect it in the rule description.
3. **T02 modifier set.** The allowed-modifier list in the rule is
   pulled from §3 of `zero-framework-spec.md`. The plan should verify
   the canonical set against `runtime/template.js` and treat that file
   as the source of truth, regenerating the list at build time if
   feasible (a const Vec in the rule module) rather than hand-keeping
   the list.
4. **I01 specifier matching.** The "starts with `z` or `zero`"
   allowlist is loose — it would accept `"zebra"` or `"zer"`. Tighten
   to the exact public surface set (`"z"`, `"z/test"`, `"z/wc"`,
   `"zero"`, `"zero/test"`, `"zero/http"`, `"zero/components"`,
   `"zero/wc"`) — sourced from the bundler's resolver if that list
   exists in code already, otherwise hard-coded with a single
   source-of-truth constant.
5. **S01 counting.** "80 lines" — does that include the signature
   line? Closing brace? The plan should pin this (suggestion: span
   from opening `{` to closing `}` inclusive, matching how a reader
   sizes a function) and document it in the rule message.
6. **Diagnostic field repurposing.** R2 reuses `property` / `value` on
   the existing `Diagnostic` struct. Is that better than adding two
   optional fields (`construct`, `category`) that the SCSS rules don't
   populate? The plan should pick one — current bias toward
   repurposing to avoid touching the SCSS rule modules.
