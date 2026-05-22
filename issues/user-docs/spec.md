# Spec: Polished User Documentation

## Problem Statement

Zero ships with three Markdown documents at the repo root —
`README.md` (354 lines), `BEST_PRACTICES.md` (602 lines), and
`zero-framework-spec.md` (1508 lines) — plus a folder of internal
issue specs. The first is a mix of pitch, install, command reference,
and authoring primer; the second is application-pattern guidance; the
third is the capability reference and was written for implementors,
not adopters.

None of these is the document a new visitor reads in the first minute
on the repo and walks away knowing whether zero fits what they're
trying to do. None is hosted as a navigable site. Concepts that are
new to most adopters — signals and effects — are explained inline
inside the spec and the README but never as a stand-alone teaching
piece that bridges from their existing React/hooks mental model.

Separately, `zero-framework-spec.md` is now redundant. It was an
implementor-facing handoff for the original build; the framework has
shipped, and the per-feature specs in `issues/*/spec.md` are the
authoritative point-in-time records of each capability. Keeping the
monolithic spec alongside the issue tree means two documents to keep
in sync — neither aimed at adopters.

This work delivers (a) a tight, decision-enabling `README.md`,
(b) a polished, navigable user guide hosted via GitHub Pages that
serves as both the teaching surface *and* the capability reference
for adopters, and (c) the removal of the now-redundant
`zero-framework-spec.md`. Aimed at the evaluator who wants to know
if zero suits their use case, and at the new adopter who wants to
learn the framework end-to-end.

## Background

### What exists today

- **`README.md`** (354 lines). Today does too much: pitch, install,
  per-command reference, a 130-line authoring primer, `zero.toml`
  schema, and a repo-layout dump. A first-time visitor cannot read it
  in a minute.
- **`BEST_PRACTICES.md`** (602 lines). Excellent content but
  positioned as a repo-root sibling to the spec. Aimed at "you've
  built your first app; here's how to organize a real one." Reachable
  only by browsing the repo.
- **`zero-framework-spec.md`** (1508 lines). The capability
  reference. Reads like a spec — exhaustive, dense, dry. Not a
  teaching document. Now redundant with `issues/*/spec.md` for
  capability history and slated for deletion as part of this work.
- Three buildable examples under `examples/` (`counter`, `todos`,
  `tracker`) — these are the canonical worked references and the
  current docs already point at them.
- No GitHub Pages site. No published, navigable user docs.

### Why this matters now

Phases 1–13 of `zero-framework-spec.md` are complete. The framework
is feature-stable: reactivity, templates, router, test runner,
design system, component library, HTTP, web-platform shims, and the
`.zero/` upgrade pipeline have all shipped. The remaining gap to
adoption is not capability — it's discoverability and onboarding.
A polished README + Pages site is the highest-leverage move left
before public visibility.

### Audience

Two readers drive the design:

1. **The evaluator** — has come from a link, has 1–2 minutes, wants
   to know what zero is, whether it's serious, and whether it
   matches their constraints. Their entire experience is the README.
2. **The new adopter** — has decided to try zero. Wants a guide that
   gets them from `zero init` to a real understanding of signals,
   effects, templates, routes, and testing. Their experience is the
   Pages site.

### Constraints inherited from earlier specs

`zero-framework-spec.md` and the existing `issues/best-practices/`
spec set the framework ethos: zero npm dependencies, single binary
does everything, no magic. The docs surface honors that — Jekyll
runs on the GitHub Pages side (not in our repo build), and we
publish plain Markdown.

## Requirements

### R1. README rewrite — layered pitch

Rewrite `README.md` as three stacked tiers, in this order:

1. **Tier 1 — Elevator pitch** (target: first screen, ~40 lines).
   One-paragraph statement of what zero is. One compact code sample
   (component + signal + reactivity, ~12 lines including imports).
   Four bullet points of headline differentiation (zero npm deps,
   single binary, no virtual DOM, signals). Install command. Single-
   command "get started" block. A link to the Pages site labeled
   "Full docs."

2. **Tier 2 — Quickstart walkthrough**. Steps from a clean checkout
   to a running edit loop: `zero init`, project layout overview,
   `zero dev`, what a component looks like, `zero build`. Tight prose,
   not a copy of the framework spec.

3. **Tier 3 — Comparison table**. A side-by-side row-per-dimension
   table covering React, Vue, Solid, Svelte on: build tool, npm
   dependencies, state model, virtual DOM, component model, bundle
   size posture. Followed by 2–3 sentences setting expectations
   about the table's limits (snapshots age, claims are coarse).

The README must:

- Not duplicate the framework spec. Where deeper content exists, link
  to the Pages docs.
- Not host the `zero.toml` reference, per-command reference, or
  repository-layout sections — those move to the docs site (R3, R5).
- Drop the "Running the runtime tests" and "Development workflow"
  sections at the bottom — those belong in a `CONTRIBUTING.md`-style
  doc, out of scope here.

Tier 1 must stand alone: a reader who reads only it should be able
to decide whether to try zero.

### R2. `docs/` folder — Jekyll on GitHub Pages

A new `docs/` directory at repo root, configured to publish via
GitHub Pages using Jekyll. The repo settings will need to be set to
serve Pages from `/docs` on `main` (a one-time configuration step
flagged in Open Questions).

Files:

```
docs/
├── _config.yml              # Jekyll: theme, title, nav order
├── index.md                 # Landing page
├── getting-started.md
├── reactivity.md            # signal + computed + effect (one page)
├── templates.md
├── components.md
├── routing.md
├── http.md
├── testing.md
├── theming.md
├── building-and-deploying.md
├── config-and-cli.md
├── linting.md               # SCSS L-rules + JS/TS rules (R5.5)
├── api.md                   # flat API surface reference (R6.5)
├── why-zero.md
├── examples-tour.md
└── best-practices.md        # moved from /BEST_PRACTICES.md (R6)
```

`_config.yml` declares one of GitHub Pages' supported Jekyll themes
(plan phase picks; recommend `minima` or `just-the-docs` — the latter
gives a sidebar and search). The theme must be on GitHub Pages'
supported-themes list to avoid a custom build pipeline.

No npm. No Node. No build step we run. GitHub Pages renders the
Markdown directly.

### R3. Chapter content — `docs/index.md` and `docs/getting-started.md`

- **`docs/index.md`** is the landing page. Restates the README pitch
  briefly, then becomes a curated table of contents pointing at the
  rest of the chapters. Not a duplicate of the README — its job is
  orientation, not selling.

- **`docs/getting-started.md`** walks a reader from install to a
  running app to their first hand-edited component. Includes:
  installation, `zero init` (interactive wizard), project layout
  walkthrough, `zero dev`, anatomy of `src/app.ts`, anatomy of a
  route component, where to go next. Ends with explicit "next read"
  links to Reactivity and Components.

### R4. Reactivity page — `docs/reactivity.md`

One page covering `signal`, `computed`, and `effect`. Pedagogy
sequence:

1. **What a signal is.** A reactive value with `.val`, `.set()`,
   `.update()`. Plain working example: a counter, then a name field.
2. **If you're coming from React** subsection. Short translation
   table: `signal` ≈ `useState` (but `.val` not destructuring,
   updates are not batched the same way); `computed` ≈ `useMemo`
   (but no deps array, auto-tracked); `effect` ≈ `useEffect` (but
   no deps array, auto-tracked, cleanup returns a function from the
   effect body).
3. **What a computed is.** Read-only derived value. Auto-recomputes
   on dependency change. Plain working example: price × quantity →
   total.
4. **What an effect is.** A side effect that re-runs when its
   dependencies change. Cleanup via a returned function. Plain
   working example: log-on-change, then DOM-focus-on-mount.
5. **Auto-tracking explained.** What "dependency" means in this
   model. Why there's no deps array. The conditional-branch example
   (deps re-tracked each run).
6. **Ownership scopes & cleanup.** Components own a scope; signals
   and effects created inside the scope dispose with it. The
   developer rarely calls `stop()` manually.
7. **Common pitfalls.** Reading `.val` outside a reactive context.
   Forgetting that a signal in a template auto-subscribes but a
   raw value does not. Effects that capture stale references.

Page ends with "→ See `examples/counter/`" and a forward-link to
Templates.

### R5. Reference chapters

- **`docs/templates.md`** — the `html` tagged template, valid value
  types in `${...}`, attribute and event binding, modifiers,
  `each()`, `ref()`. Worked examples per concept. Replaces the
  inline coverage currently in `README.md`'s authoring primer.
- **`docs/components.md`** — components-as-functions, props,
  children, composition. Then the shipped component library (the
  15 components from `zero/components`) as a reference subsection
  with usage snippets.
- **`docs/routing.md`** — `app.route()`, params/wildcards, lazy
  imports, route guards, `load()`, `meta`, nested routes,
  navigation, active-link styling, route-scoped `fetch`. Worked
  example from `examples/tracker/`.
- **`docs/http.md`** — `createHttp()`, methods, middleware (onion
  model), `HttpError`, route-scoped fetch threading. Forward-points
  to `docs/best-practices.md` for the one-client-per-backend
  pattern.
- **`docs/testing.md`** — `describe`/`it`/`expect`, `render`/`find`/
  `text`/`fire`, signals testing, component testing, the in-memory
  DOM and what it covers, the Web Platform shim list with the
  "clear error" discipline, `spy()` for assertions.
- **`docs/theming.md`** — the design system (tokens, palette, layout
  primitives, utilities), light/dark, authoring a brand theme,
  typography utilities. Pulls from `zero-framework-spec.md` §7
  but in tutorial voice.
- **`docs/building-and-deploying.md`** — `zero build`, the
  `manifest.json` shape, integrating with a backend server,
  deploying static, `zero preview`. Replaces the `### zero build`
  block in the current README.
- **`docs/config-and-cli.md`** — combined reference page. Full
  `zero.toml` schema (every key, every validation rule); per-
  subcommand reference (`init`, `dev`, `build`, `test`, `mutate`,
  `update`, `lint`, `check`, `fmt`, `gen`, `preview`). Stays a flat
  reference, not a tutorial.
- **`docs/linting.md`** — single reference page for everything
  `zero lint` catches. Two top-level sections: **SCSS / design
  system** (rules `L01`–`L11`, currently described in the
  existing per-rule sources under `crates/zero-lint/src/rules/`)
  and **JS/TS framework idioms** (rules `R01`–`R03`, `T01`–`T04`,
  `C01`–`C02`, `I01`–`I02`, `S01`, plus the `P01` parse-error
  reservation — specified by `issues/lint-js/spec.md`). Each rule
  gets ID, one-line description, a violating snippet, a passing
  snippet, and a one-line "why it matters" cross-link to the
  teaching chapter that explains the underlying primitive
  (Reactivity, Templates, Components, etc.). Closes with a short
  section on running `zero lint`, exit codes, and the no-
  suppression / no-config posture. Important: the lint-js spec's
  R6 originally targeted `zero-framework-spec.md §1` for the JS/TS
  rule table; that table lands on this page instead (see R7).
- **`docs/api.md`** — flat reference page enumerating the full
  public surface, mirroring what `zero-framework-spec.md` §11
  provided. Sections per module: `"zero"`, `"zero/test"`,
  `"zero/http"`, `"zero/components"`, `"zero/wc"`. Each export with
  its signature and a one-line description. Forward-points to the
  relevant teaching chapter for context. This page is the
  capability reference the deleted spec used to be — a single
  page a reader can search-in-page. Source of truth for types
  remains the `.zero/*.d.ts` files; this page mirrors them for
  human reading.
- **`docs/why-zero.md`** — long-form companion to the README
  comparison table. Walks through the design decisions: no virtual
  DOM, no node_modules, signals over hooks, single binary, plain
  functions over classes, no file-system routing. Each decision
  with a tradeoff statement so the reader can self-select out.
  Seed material: spec §13 "Key Design Decisions Summary".
- **`docs/examples-tour.md`** — a walkthrough of the three shipped
  examples: what each demonstrates, the patterns it canonizes,
  pointers at the files worth reading first. Bridges the docs and
  `best-practices.md`.

Every chapter targets ≤ 400 lines. Where a topic would exceed that,
it should be cut down or split — these are user docs, not the spec.

### R6. Move `BEST_PRACTICES.md` to `docs/best-practices.md`

Move the file. Update:

- All internal `→ See …` pointers stay valid (they point at
  `examples/`, which is unchanged).
- References inside `BEST_PRACTICES.md` to `zero-framework-spec.md`
  rewrite to point at the appropriate new `docs/*.md` chapter (see
  R7 for the mapping).
- `crates/zero-scaffold/src/scaffold/AGENTS.md:1137` ("see
  `BEST_PRACTICES.md` at the framework repo root") rewrites to
  point at the Pages URL for `docs/best-practices.md`.
- The new `README.md` links to `docs/best-practices.md` from a
  "Going deeper" line near the bottom of Tier 2.
- The new docs chapters (routing, http, components, testing) each
  forward-point to the relevant `best-practices.md` section as
  "after you've built your first app, read this."

No content rewrite. The file moves verbatim; only outbound
references are updated.

### R7. Delete `zero-framework-spec.md`

The monolithic framework spec is removed. The capability content it
held lands across `docs/` per this mapping; the plan phase uses the
mapping as a content-migration checklist before the file is deleted.

| Spec section | New home |
|---|---|
| §1 CLI Interface | `docs/config-and-cli.md` |
| §1 "JS/TS lint" subsection (proposed by `issues/lint-js/spec.md` R6 — never written, since the spec is deleted before lint-js lands its docs) | `docs/linting.md` |
| §2 Entry Point & Boot Sequence | `docs/getting-started.md` (entry + boot); `docs/routing.md` (navigation lifecycle) |
| §3 Component Model | `docs/components.md` + `docs/templates.md` |
| §4 Reactivity System | `docs/reactivity.md` |
| §5 State Machines (deferred) | Dropped. The status-tagged-signal pattern survives in `docs/best-practices.md` §4 (already present). |
| §6 Router | `docs/routing.md`; route-scoped fetch contract repeats verbatim in `docs/http.md` |
| §7 CSS Strategy & Design System | `docs/theming.md` |
| §8 Testing | `docs/testing.md`; the Web Platform shim list lands as a reference subsection on the same page |
| §9 Transpiler / Compiler | Dropped from user-facing docs. Implementor detail; lives implicitly in the transpiler crate. |
| §10 tsconfig.json | `docs/getting-started.md` |
| §11 Complete API Surface | `docs/api.md` |
| §12 Implementation Priority (phase roadmap) | Dropped. Issue specs are the historical record. |
| §13 Key Design Decisions Summary | Seed for `docs/why-zero.md` |

External references the deletion touches:

- `README.md` — currently says "The full surface is documented in
  `zero-framework-spec.md`." Replace with a link to `docs/api.md`
  and the Pages site root.
- `BEST_PRACTICES.md` (now `docs/best-practices.md`) — all
  spec-section pointers (`zero-framework-spec.md §5`, `§6`, `§8`,
  `§11`, etc.) rewrite to the corresponding `docs/*.md` page.
- `.claude/commands/refine.md:13` — orientation step currently
  reads `zero-framework-spec.md`. Rewrite to read
  `issues/*/spec.md` index and the relevant past specs by topic;
  plan phase resolves the exact replacement wording.
- The ~35 references in `issues/*/spec.md` and `issues/*/plan.md`
  are historical point-in-time artifacts. **Do not rewrite them.**
  They describe what was decided when the issue shipped; the
  spec they reference existed then. Edit in place would falsify
  the historical record.

Order of operations:

1. Build out the `docs/` content (R3, R4, R5) including `api.md`,
   migrating spec content per the table above.
2. Verify every cell of the mapping table is populated in docs/.
3. Update outbound references in `README.md`,
   `docs/best-practices.md`, `.claude/commands/refine.md`, scaffold
   `AGENTS.md`.
4. `git rm zero-framework-spec.md` in the same commit that ships
   the last of the migration.

### R8. Cross-document link hygiene

After R6 and R7, every internal Markdown link is verified:

- `README.md` → absolute Pages site URLs (not relative paths into
  `docs/`), so the README reads correctly from any clone or fork
  view on GitHub.
- `docs/*.md` → relative links to sibling `docs/*.md` pages, and
  relative `../examples/…` links back into the repo for source
  pointers.
- No remaining live references to `zero-framework-spec.md` anywhere
  outside `issues/*/{spec,plan,review}.md` (historical artifacts —
  intentionally left).

The plan phase produces a link-check pass as the final gate before
the deletion lands.

### R9. Coordinate with `issues/lint-js/`

`issues/lint-js/spec.md` (the JS/TS lint pass — Phase 14) is
currently in-flight or queued. Its R6 ("Documentation") plans to:

- Add a `## JS/TS lint` section under §1 of `zero-framework-spec.md`.
- Append the same rule table to `src/scaffold/AGENTS.md`.
- Mark Phase 14 in `zero-framework-spec.md` §12.

This user-docs work deletes the spec. The coordination contract:

- **The lint-js work no longer touches `zero-framework-spec.md`.**
  Its rule-table content lands in `docs/linting.md` instead. The
  user-docs plan and the lint-js plan jointly agree on the
  ownership boundary: lint-js owns the rule definitions and the
  per-rule prose (it knows them best); user-docs owns the page
  shell (intro, "how to run", layout) and merges lint-js's rule
  prose in.
- **The scaffold `AGENTS.md` rule-table append stays in
  lint-js's scope** — that file lives in
  `crates/zero-scaffold/src/scaffold/AGENTS.md` and is
  unaffected by the docs deletion.
- **No Phase 14 checklist needs writing** anywhere; once §12 is
  deleted, the per-phase tracking disappears with it. The
  lint-js plan's own checkboxes still track its work.

Sequencing: if lint-js ships first, its docs intent for the
deleted spec needs interception — either lint-js writes its
docs straight into a stub `docs/linting.md` before user-docs
fills in the rest, or lint-js holds its docs work and user-docs
absorbs it. Plan phase resolves the order; in either case the
plan flags the touchpoint explicitly to whichever PR lands second.

### R10. GitHub Pages enablement

The repo's GitHub settings need Pages enabled with source
"Deploy from a branch" → `main` → `/docs`. This is a one-shot
manual setting outside our codebase; the spec calls it out so the
plan phase doesn't lose it. The chosen Jekyll theme must be on the
supported-themes list to avoid a workflow file.

## Constraints

- **No npm, no Node, no SSG build step we run.** Jekyll runs on
  GitHub Pages' side. We commit Markdown.
- **No translations.** Single-language (English) docs.
- **No per-version docs.** The site reflects whatever's on `main`.
  Pre-1.0 framework; semantic versioning of docs is premature.
- **No auto-generated API reference.** TypeScript surface is the
  framework spec's job (§11); docs/ teaches, doesn't enumerate.
- **`docs/` is both the teaching surface and the capability
  reference.** With `zero-framework-spec.md` deleted, there is no
  separate reference document for adopters. Teaching chapters
  cover concepts; `docs/api.md` covers the flat surface; the
  `.zero/*.d.ts` files remain the type source of truth.
- **Per-feature issue specs are the historical record, not adopter
  documentation.** `issues/*/spec.md` and `issues/*/plan.md` stay
  in the repo as point-in-time records of how each capability was
  designed; they are not linked from `docs/` or `README.md` and are
  not maintained as living documents.
- **Tier 1 of the README is the unmovable constraint.** Every other
  README decision compromises in favor of keeping Tier 1 readable
  in one minute.
- **Each docs chapter is self-contained enough to land on Google.**
  A reader arriving at `docs/routing.md` from a search result should
  not need to have read `getting-started.md` first to make sense
  of the page.
- **Voice.** Direct, plain. No marketing copy. No emoji. Match the
  voice of `BEST_PRACTICES.md` and the better sections of the
  framework spec.

## Out of Scope

- A static-site generator we own (VitePress, Astro, Docusaurus,
  custom Rust generator). Jekyll-on-GitHub-Pages only.
- Eating our own dogfood by building the docs with `zero` itself —
  attractive but a separate, larger initiative.
- Live, runnable code examples on the docs site. Static code blocks
  only; readers run the examples locally.
- Search beyond what the chosen Jekyll theme provides natively.
- A docs versioning scheme.
- Migration of `improved_agent_usage.md` or any other repo-root
  scratch files into `docs/`.
- A `CONTRIBUTING.md` covering the "Running the runtime tests" /
  "Development workflow" content removed from the README. That's a
  follow-up.
- API auto-generation from TypeScript declarations. `docs/api.md`
  is hand-maintained against the `.zero/*.d.ts` source of truth.
- A blog, changelog, or news section on the Pages site.
- Editing historical `issues/*/{spec,plan,review}.md` references to
  `zero-framework-spec.md`. Those are point-in-time records and
  stay untouched even after the spec is deleted.

## Open Questions

- **Jekyll theme.** Recommend `just-the-docs` for the sidebar +
  search; `minima` is the safe minimal choice. Plan phase picks
  one. Both are on GitHub Pages' supported list.
- **Pages URL.** The repo's GitHub URL determines the final
  `https://<owner>.github.io/<repo>/` path. README links to the
  Pages site need this resolved before the README rewrite ships.
- **Tier 3 comparison table — which frameworks?** Spec proposes
  React, Vue, Solid, Svelte. Plan should confirm; a five-column
  table risks wrapping on narrow viewports. Drop Svelte if it
  cramps the table.
- **`docs/index.md` vs README overlap.** Spec says they don't
  duplicate — index does orientation, README does pitch. Plan
  should confirm the index page's value-add justifies its
  existence; if not, fold it and let the Pages theme's default
  landing surface the chapter list.
- **Mermaid / diagram support.** GitHub renders Mermaid inline;
  Jekyll's stock setup may not. If any chapter benefits from a
  diagram (boot sequence, navigation lifecycle), plan needs to
  resolve whether to embed Mermaid (theme-dependent), commit SVGs,
  or skip diagrams.
- **Sequencing.** Plan should split the work into shippable
  increments: (a) `docs/` scaffolding + Pages enablement + getting-
  started + reactivity as the first content PR; (b) the rest of
  the chapters including `api.md` (the spec-deletion gate);
  (c) move `BEST_PRACTICES.md` + update outbound links + delete
  `zero-framework-spec.md` + update `refine.md` and scaffold
  `AGENTS.md`, all in one commit so no broken in-tree references
  exist at any intermediate state; (d) README rewrite last so it
  can link to live Pages URLs.
- **`docs/api.md` maintenance burden.** Hand-maintaining a flat
  reference page against the `.zero/*.d.ts` files is real work and
  drifts. Plan should resolve whether to (a) accept the drift risk
  and add `api.md` to the per-release checklist, (b) auto-generate
  from `.d.ts` with a tiny in-tree script (still requires re-
  running on each surface change), or (c) skip `api.md` and let
  per-chapter API subsections + the `.d.ts` files carry the
  surface. The spec recommends (a); plan picks.
- **`.claude/commands/refine.md` replacement wording.** Step 1
  currently instructs the agent to read `zero-framework-spec.md`
  for orientation. After deletion, the equivalent orientation is
  "read `docs/index.md` and any relevant `issues/*/spec.md` whose
  topic is adjacent to the new item." Plan finalizes the wording.
- **Treatment of the existing authoring primer in `README.md`.**
  ~130 lines covering components, signals, templates, lists, refs,
  inject, routing — strong material. Plan should resolve whether
  to use it as the seed for the new docs chapters (likely yes —
  copy in, then polish per-chapter) or rewrite each chapter from
  scratch.
- **AGENTS.md updates.** The scaffold AGENTS.md and the repo
  AGENTS.md (if it exists at root) currently reference
  `BEST_PRACTICES.md`. Plan needs to catalog every such reference
  before the R6 move.
