# Spec: AGENTS.md in the `zero init` scaffold

## Problem Statement

A freshly scaffolded zero project ships only code (`index.html`, `src/app.js`, `src/routes/home.js`, `src/routes/home.test.js`, `styles/app.css`). There is no in-project documentation. To write anything beyond the home route, a developer — or, more pointedly, an AI coding agent invoked inside the project — has to leave the scaffold and read the framework spec, the runtime source, or guess.

For a framework whose pitch is "zero dependencies, single binary, no magic," that documentation gap is the loudest piece of magic left: you can't write a program against zero without consulting external materials. We want a single reference file dropped into every scaffold that gives an agent or developer enough to write any program against the *currently implemented* zero API without leaving the project directory.

## Background

### The scaffold today

`zero init` materializes the templates under `src/scaffold/` via `src/scaffold.rs`. Templates are `include_str!`'d at build time and written to the user's project root with `{{title}}` substitution. No `AGENTS.md`, `README.md`, or `CLAUDE.md` is generated. Adding a new template file means:

1. Add the file under `src/scaffold/…`
2. Add an `include_str!` constant in `src/scaffold.rs`
3. Add a `fs::write` call in `write_to`
4. Add an assertion to the existing tests

### What the framework actually exposes today

The runtime in `runtime/` exports the surface used by the scaffold via the `"zero"` and `"zero/test"` import paths. The framework spec (`zero-framework-spec.md`) lists the *planned* API, but several items are not implemented yet (Phase 4 is deferred; parts of Phases 5/6 are unfinished). Examples of planned-but-not-yet-implemented surface that **must not** be documented: `app.use()` middleware, route guards, route groups (`group()`), route transitions, `--coverage`, `--watch`, snapshot assertions, `zero gen`, `zero check`, `zero fmt`, `zero lint`, state machines. The planner must derive the actual current surface by reading the `runtime/*.js` exports, not from the framework spec. When the framework spec and the runtime disagree, the runtime is the source of truth.

### Audience

"Agent/dev" — the doc serves both AI coding agents working inside a scaffolded project and human developers. The format leans toward agents (dense, scannable, exhaustive reference) but stays readable for humans. `AGENTS.md` is the chosen filename because it's an emerging convention picked up by Claude Code, Cursor, and similar tools as the authoritative in-repo agent context file, while remaining a plain markdown doc.

### Style constraints in this repo

- JavaScript files in this codebase must be fully JSDoc-annotated (per `CLAUDE.md`). Examples in `AGENTS.md` should reflect that style where it doesn't clutter the example. The scaffold's existing `home.js` is a good calibration: `@typedef` import plus `@returns` JSDoc tags on exported functions.
- The framework philosophy is "no magic, no noise." Examples should be minimal, idiomatic, and *the kind of code we want the agent to write*. Avoid teaching-comment noise.

## Requirements

1. **Deliverable.** A new file `AGENTS.md` is written to the project root by `zero init`, alongside the existing scaffold files.

2. **Comprehensive reference + examples.** The doc covers the full public API of *currently-implemented* features, with a runnable example for each. An agent reading only this file plus the generated scaffold should be able to write any program the current framework supports — routes, components, signals, effects, lists, refs, app-level state, testing — without reading the runtime source or the framework spec.

3. **API surface to cover.** At minimum, document the following (verify each by grepping the actual `runtime/` exports before including):
   - From `"zero"`: `App` class and its currently-implemented methods (`state`, `route`, `layout`, `run`, anything else exported), `signal`, `computed`, `effect`, `html`, `each`, `ref`, `inject`, `navigate`, `back`, `forward`, `route()`, `TemplateResult` typedef.
   - From `"zero/test"`: `describe`, `it`, `expect` and the assertions actually implemented, `beforeEach`/`afterEach`/`beforeAll`/`afterAll` if present, `render`, `find`, `findAll`, `text`, `fire`, `cleanup`, plus any helpers like `settled()` if implemented.
   - Anything else the runtime exports today that isn't on this list should be included; anything on this list that *isn't* exported should be omitted.

4. **Implemented-only.** Do not document deferred or unimplemented features. If the planner encounters a feature in the framework spec but cannot find it in the runtime, it is excluded. No "coming soon" sections.

5. **Examples are correct.** Every code example in `AGENTS.md` must be valid against the current runtime. The planner should structure the work so examples can be sanity-checked (e.g. by mirroring the patterns in `runtime/*.test.js` and the existing `home.js`/`home.test.js`).

6. **Self-contained.** The doc must not require the reader to fetch external resources. No links to a website that doesn't exist. References to other in-project files (`src/app.js`, `src/routes/home.js`, `src/routes/home.test.js`) as worked examples are encouraged.

7. **`zero init` integration.** `src/scaffold.rs` is updated to emit `AGENTS.md` at the project root. Existing scaffold tests are extended (or new ones added) to assert the file is written and contains a sentinel string from each major section.

8. **Title/header.** The doc's H1 should identify the framework and project clearly. The `{{title}}` placeholder used in `index.html` can be reused if helpful, but isn't required.

9. **Structure (suggested, planner may refine).** Order from highest-frequency to lowest-frequency lookups, since agents often grep:
   - One-paragraph framework summary (what zero is, what's in the scaffold, how to run dev/test)
   - Project layout (what each generated file is for)
   - Writing components (`html`, props, children, events, conditional rendering, lists via `each`, refs)
   - Reactivity (`signal`, `computed`, `effect`)
   - App configuration (`new App()`, `state`, `route`, `layout`, `run`)
   - Routes (params, query, `load`, route component props, current route via `route()`)
   - Programmatic navigation (`navigate`, `back`, `forward`)
   - App-level state (`inject`)
   - Testing (`describe`/`it`/`expect`, DOM helpers, testing components, testing reactivity)
   - JSDoc style conventions used in scaffolded files

10. **Scaffold code itself stays minimal.** No new teaching comments added to `app.js`, `home.js`, or `home.test.js`. The reference lives in `AGENTS.md`; the scaffold files remain the canonical small example of *good code* an agent should imitate.

## Constraints

- **Source of truth = runtime.** The planner derives the documented API from `runtime/*.js` exports, not from `zero-framework-spec.md`. The spec doc describes the destination, the runtime describes the current state.
- **Implemented features only.** See Requirement 4.
- **Embedding mechanism matches existing pattern.** Use `include_str!` in `src/scaffold.rs`, write via `fs::write` in `write_to`. No new dependencies.
- **No external links** to docs sites, npm packages, blog posts, or anything that could 404 or drift.
- **Markdown only.** Plain CommonMark. No HTML, no admonition syntax that depends on a renderer.
- **Length budget.** Aim for a single file that an agent can load in one read. Roughly 400–800 lines is the target — dense enough to be exhaustive, short enough to skim. Hard cap left to the planner.
- **JSDoc-style examples.** When showing exported functions, examples carry the same `@param`/`@returns` annotations the project requires. Inline expression examples can omit JSDoc.

## Out of Scope

- **Top-level repo `README.md`.** Not touched by this work. The repo's own README is for people building the CLI from source; `AGENTS.md` is for people building apps with the CLI. Separate audiences.
- **External documentation website.** No site, no GitHub Pages, no `/docs` folder in the repo root.
- **Tutorial / "build your first app" walkthrough.** `AGENTS.md` is reference + per-feature example, not a narrative tutorial.
- **New CLI flags or subcommands.** `zero init` simply emits one additional file. No `zero docs`, no `--with-agents-md` flag.
- **Documenting deferred features.** State machines, middleware, route guards, route groups, transitions, `zero gen`, `zero check`, `zero fmt`, `zero lint`, `--watch`, `--coverage`, snapshot testing — explicitly omitted until each ships.
- **Updating `CLAUDE.md` (repo root).** That file is for working *on* the framework. `AGENTS.md` is for working *with* the framework.

## Open Questions

1. **Verifying the actual export list.** The planner needs to enumerate the real `"zero"` and `"zero/test"` exports. Best approach is probably to read every `runtime/*.js` file and list everything not marked `@internal` or starting with `_`. Worth confirming this is the boundary the framework intends (i.e., the `_`-prefix + `@internal` JSDoc tag is the de facto "private" marker).
2. **Where do reactive blocks (`${() => …}`) and signal auto-unwrapping fit in the doc structure?** They're core to the template system but cross-cut components, reactivity, and routes. Probably one short subsection under "Writing components" with cross-refs, but the planner should commit to a placement.
3. **Should the doc include a "patterns to avoid" or "gotchas" section?** E.g., "don't store DOM nodes in signals," "components are called once — don't put `signal()` outside the function body if you want fresh state." Useful for agents, but risks scope creep. Decide based on what surfaces while writing.
4. **Test assertion list.** The framework spec lists `toBe`, `toEqual`, `toBeTruthy`, `toBeFalsy`, `toBeNull`, `toContain`, `toThrow`, `toBeTemplateResult`, `toMatchSnapshot`. The planner must check which of these are actually implemented in `runtime/test.js` and document only those.
5. **Should examples use ES modules with explicit `.js` extensions (matching scaffold) or omit extensions?** The scaffold uses `./routes/home.js` with extension. Be consistent with that.
