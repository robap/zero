# Plan: Polished User Documentation

## Summary

Stand up a Jekyll-rendered docs site under `docs/` (served via GitHub
Pages from `main` / `/docs`), write fifteen polished user-facing
chapters whose pedagogical voice bridges from React mental models,
move `BEST_PRACTICES.md` into the new site, delete the now-redundant
`zero-framework-spec.md`, and rewrite `README.md` as a tight three-
tier pitch (elevator → quickstart → comparison). The work proceeds
chapter-by-chapter so every intermediate commit leaves the repo in a
consistent state; the spec deletion is the last load-bearing step
because it depends on every chapter having absorbed the content it
held.

## Prerequisites

The spec's nine open questions are resolved as follows. Each step
encodes these decisions; no further user input is needed.

1. **Jekyll theme** — `just-the-docs`. Supported by GitHub Pages,
   ships a sidebar and client-side search, supports `nav_order`
   front-matter for stable chapter ordering, and renders Markdown
   tables and fenced code blocks cleanly. Chosen over `minima`
   because the docs are reference-shaped (sidebar matters).
2. **Pages URL** — derived at PR-time. Plan steps reference the URL
   as `<PAGES_URL>`; Step 18 resolves it from
   `git remote get-url origin` (form
   `https://robap.github.io/zero/`) and the README links it
   absolutely so the file reads correctly on github.com.
3. **Tier 3 comparison table** — four columns: React, Vue, Solid,
   Svelte. Six rows: build tool, npm dependencies, state model,
   virtual DOM, component model, bundle-size posture. Step 18
   includes the literal table content.
4. **`docs/index.md` vs README overlap** — both ship. README is the
   github.com landing surface; `docs/index.md` is the Pages site's
   home (rendered as the theme's home page) and serves as the
   navigation hub. They differ in voice: README sells; `index.md`
   orients. No content overlap beyond a one-paragraph framing.
5. **Mermaid / diagrams** — skipped for v1. If a chapter benefits
   (boot sequence, navigation lifecycle), use an ASCII diagram in a
   fenced block (renders fine in Jekyll without plugins).
6. **Sequencing** — single linear sequence baked into Steps 1–19.
   No multi-PR coordination assumed; the executor lands changes as
   work proceeds.
7. **`docs/api.md` maintenance** — option (a) from the spec: hand-
   maintained, drift accepted, source of truth remains
   `runtime/*.d.ts` and `crates/zero-scaffold/src/scaffold/.zero/
   components.d.ts`. Adding `api.md` to a per-release checklist is
   out of scope for this work.
8. **`.claude/commands/refine.md` replacement wording** — Step 17
   replaces the line with: *"`docs/index.md` — overview of every
   shipped capability and where the user-facing reference lives.
   Browse `issues/` for the spec/plan of any past or in-flight item
   adjacent to the new one."*
9. **README authoring primer treatment** — seed material. Copy
   relevant prose into the new docs chapters as a starting point,
   then polish chapter-by-chapter.

`issues/lint-js/` is already shipped (every step `[x]` in
`issues/lint-js/plan.md`); its R6 docs already live in
`zero-framework-spec.md §1` and `crates/zero-scaffold/src/scaffold/
AGENTS.md`. This work absorbs that content into `docs/linting.md`
along with everything else from the spec — no coordination needed.

## Steps

- [x] **Step 1: Scaffold `docs/` (Jekyll config + stub chapters)**
- [x] **Step 2: Write `docs/getting-started.md`**
- [x] **Step 3: Write `docs/reactivity.md`**
- [x] **Step 4: Write `docs/templates.md`**
- [x] **Step 5: Write `docs/components.md`**
- [x] **Step 6: Write `docs/routing.md`**
- [x] **Step 7: Write `docs/http.md`**
- [x] **Step 8: Write `docs/testing.md`**
- [x] **Step 9: Write `docs/theming.md`**
- [x] **Step 10: Write `docs/building-and-deploying.md`**
- [x] **Step 11: Write `docs/config-and-cli.md`**
- [x] **Step 12: Write `docs/linting.md`**
- [x] **Step 13: Write `docs/api.md`**
- [x] **Step 14: Write `docs/why-zero.md` and `docs/examples-tour.md`**
- [x] **Step 15: Write `docs/index.md` (landing) and enable GitHub Pages (manual)**
- [x] **Step 16: Move `BEST_PRACTICES.md` → `docs/best-practices.md` and update outbound references**
- [x] **Step 17: Delete `zero-framework-spec.md` and update `.claude/commands/refine.md`**
- [x] **Step 18: Rewrite `README.md` (three-tier pitch with live Pages URLs)**
- [x] **Step 19: Final link-check pass**

---

## Step Details

### Step 1: Scaffold `docs/` (Jekyll config + stub chapters)

**Goal:** Stand up the Jekyll site structure so every subsequent step
can write into a known directory layout. The site builds (when
Pages is enabled later in Step 15) even with empty stubs. No content
yet.

**Files (all new):**
- `docs/_config.yml`
- `docs/index.md`
- `docs/getting-started.md`
- `docs/reactivity.md`
- `docs/templates.md`
- `docs/components.md`
- `docs/routing.md`
- `docs/http.md`
- `docs/testing.md`
- `docs/theming.md`
- `docs/building-and-deploying.md`
- `docs/config-and-cli.md`
- `docs/linting.md`
- `docs/api.md`
- `docs/why-zero.md`
- `docs/examples-tour.md`

**Changes:**

1. **`docs/_config.yml`**:
   ```yaml
   title: zero
   description: A zero-dependency frontend framework with a Rust CLI.
   remote_theme: just-the-docs/just-the-docs
   color_scheme: dark
   search_enabled: true
   heading_anchors: true
   nav_external_links: []
   aux_links:
     "GitHub":
       - "https://github.com/robap/zero"
   # exclude README and the issues tree from the Jekyll build
   exclude:
     - README.md
     - BEST_PRACTICES.md
     - zero-framework-spec.md
     - issues/
     - examples/
     - runtime/
     - crates/
     - target/
     - showcase/
     - Cargo.toml
     - Cargo.lock
   ```
   `robap/zero` is left literal in this step and filled in by
   Step 15 once the Pages URL is resolved.

2. **Each chapter stub** is a front-matter-only file:
   ```markdown
   ---
   title: <Human-readable title>
   nav_order: <N>
   ---

   # <Title>

   *(placeholder — written in step <N+1>)*
   ```

   Nav order is the reader sequence:
   - `index.md` — `nav_order: 1`, `title: "zero"`
   - `getting-started.md` — `2`
   - `reactivity.md` — `3`
   - `templates.md` — `4`
   - `components.md` — `5`
   - `routing.md` — `6`
   - `http.md` — `7`
   - `testing.md` — `8`
   - `theming.md` — `9`
   - `building-and-deploying.md` — `10`
   - `config-and-cli.md` — `11`
   - `linting.md` — `12`
   - `api.md` — `13`
   - `best-practices.md` — `14` (created in Step 16; reserve the slot now by leaving 14 unused until then)
   - `examples-tour.md` — `15`
   - `why-zero.md` — `16`

**Tests:** none (markdown stubs; Jekyll renders on GitHub's side).
Verify `cargo test --workspace` remains green — Jekyll files don't
affect Rust builds. Verify the `exclude` list keeps `examples/`,
`runtime/`, `crates/`, etc. out of the build so no spurious doc
pages appear.

---

### Step 2: Write `docs/getting-started.md`

**Goal:** Land the new-adopter walkthrough — install through first
edit. End-to-end self-contained: a reader arriving here from search
can get a running zero app without reading anything else.

**Files:**
- `docs/getting-started.md` (replace stub)

**Content outline (≤ 400 lines):**

1. **Install**. `cargo install zero --locked` (or build from source
   per current README). One-liner verification: `zero --version`.
2. **Scaffold a project**. `mkdir my-app && cd my-app && zero init`
   walkthrough. Show the interactive prompts; show what the
   non-interactive path looks like with a pre-written `zero.toml`.
   Annotate every file the scaffold creates: `index.html`,
   `src/app.ts`, `src/routes/home.ts`, `styles/app.scss`,
   `tsconfig.json`. Note the role of `.zero/` (auto-managed,
   `.gitignore`'d).
3. **Start dev**. `zero dev`, open the browser, verify the home
   route renders.
4. **Anatomy of `src/app.ts`**. Annotated 10-line example showing
   `new App()`, `app.state(...)`, `app.route(...)`, `app.run("#app")`.
   Forward-link to Routing for the long story.
5. **Anatomy of a route component**. Annotated `src/routes/home.ts`
   walking through `signal`, `html`, `${expr}`, an event binding.
   Forward-link to Reactivity and Templates.
6. **Make the first edit**. Reader changes the home route to render
   a counter. Save → page reloads. Notes that the dev server does a
   full reload (HMR is roadmapped, not shipped).
7. **Build for production**. `zero build`, point at `dist/`,
   forward-link to Building & Deploying.
8. **What to read next**. Three links: Reactivity (most important
   new concept), Components, and the Examples Tour.

**Seed material:** README §"Quick start", §"Writing a zero app"
(the authoring primer subsections "App entry", "Components",
"Signals" — strip down to the smallest possible scope here; deeper
treatment lives in Reactivity / Components / Templates).

**Tests:** none. Manual: read the rendered markdown on
github.com/blob to verify formatting holds.

---

### Step 3: Write `docs/reactivity.md`

**Goal:** Teach `signal`, `computed`, `effect` from first principles
with React mental-model bridges. This is the load-bearing
pedagogical chapter — the spec calls it out by name. ≤ 400 lines.

**Files:**
- `docs/reactivity.md` (replace stub)

**Content outline (matching spec R4 exactly):**

1. **What a signal is.** Reactive value with `.val`, `.set()`,
   `.update()`. Counter example. Name-field example.
2. **If you're coming from React.** Translation table:
   | React | zero | Note |
   |---|---|---|
   | `useState` | `signal` | Read via `.val`; write via `.set()` / `.update()`. No destructured setter. |
   | `useMemo` | `computed` | No deps array. Tracking is automatic; deps re-collect on each run. |
   | `useEffect` | `effect` | No deps array. Cleanup returned from the body, not from a second hook. |
   Three small contrasting code snippets follow.
3. **What a computed is.** Read-only derived value. Auto-recomputes.
   Price × quantity → total example. Note: computed is lazy — only
   evaluates when read.
4. **What an effect is.** Side effect with cleanup. Two examples:
   `console.log` on signal change; focus-on-mount via `ref`.
5. **Auto-tracking explained.** What "dependency" means: any `.val`
   read inside the reactive function on its current run. Why there's
   no deps array. Conditional-branch example showing deps shift
   between runs.
6. **Ownership scopes & cleanup.** Components create a scope.
   Signals and effects registered to the enclosing scope dispose
   with it. Reader rarely calls the returned `stop()` from
   `effect()`. Forward-link to Components for the scope/mount story.
7. **Common pitfalls.**
   - Reading `.val` outside a reactive context — works, but no
     subscription, so it never updates.
   - In a template, `${signal}` is reactive but `${signal.val}` is
     not — `zero lint`'s R01 catches this. Cross-link to Linting.
   - Effects that capture stale closures (the auto-tracking story
     already handles this; called out explicitly).
   - Module-level `signal()` / `effect()` leaks scope — `R03` flags
     it; cross-link to Linting.

**Closing:** "→ See `examples/counter/`" with a relative link to
the example directory. Forward-link to Templates.

**Seed material:** README §Signals; `BEST_PRACTICES.md` §4 (status-
tagged signals — referenced briefly with forward-link to
best-practices); spec §4 (reactivity system).

**Tests:** none. Manual format check.

---

### Step 4: Write `docs/templates.md`

**Goal:** Cover the `html` tagged template, valid `${...}` values,
attribute/event binding, modifiers, `each`, `ref`. ≤ 400 lines.

**Files:**
- `docs/templates.md` (replace stub)

**Content outline:**

1. **What `html` does.** Tagged template returns a lightweight
   `TemplateResult`. Caches structure per call site; clones
   `DocumentFragment` per render. Components run once; granular
   updates happen at the `${}` substitution sites.
2. **Valid substitution values.** Reproduce spec §3 "Valid Template
   Values" table verbatim (strings, numbers, booleans, null/undef,
   Signal, TemplateResult, arrays, reactive blocks `() => …`). One
   short example per row.
3. **Attribute binding.** `class=${...}`, `value=${...}`,
   `disabled=${...}` (boolean handling).
4. **Event binding.** `@click=${handler}`, with-event-object,
   inline lambdas.
5. **Event modifiers.** Reproduce the runtime's allowed set
   (`prevent`, `stop`, `once`, `throttle`, `debounce`, `enter`,
   `escape`, `space`, `tab`, `up`, `down`, `left`, `right`).
   One example per family. Cross-link to Linting (T02 flags typos).
6. **Reactive blocks.** `() => …` for conditional rendering. The
   conditional example from spec §3 — auth status branch.
7. **`each()`** with key function. Why key matters. Cross-link to
   Reactivity (per-item scope disposal). Cross-link to Linting
   (T03 flags the no-key form).
8. **`ref()`**. Element handles. Auto-focus example. Cross-link to
   Reactivity (effect + ref pattern).

**Seed material:** spec §3 (component model / templates); README
authoring primer (lists, refs, attributes); scaffold AGENTS.md
§Imports / §Components.

**Tests:** none.

---

### Step 5: Write `docs/components.md`

**Goal:** Teach components-as-functions, props, children,
composition; then list the shipped `zero/components` library with
short usage snippets. ≤ 400 lines.

**Files:**
- `docs/components.md` (replace stub)

**Content outline:**

1. **Components are functions.** A component is `() =>
   TemplateResult`. Run once on commit. State and effects belong to
   the scope opened by the call.
2. **Props.** Plain object. Show passing static + signal-shaped
   props. The "signal passed in stays reactive" rule.
3. **Children / slots.** Children are a prop. Multiple slots are
   multiple props. Card-with-title-and-body example.
4. **Composition.** Components compose by call. Show a Page →
   Header + Sidebar + Main composition.
5. **Component library reference.** Subsection: `Button`, `Input`,
   `Checkbox`, `Toggle`, `Select`, `Radio`, `TextArea`, `Dialog`,
   `Tabs`, `Card`, `Avatar`, `Badge`, `Spinner`, `Toast`, `Table`.
   One row per component:
   | Component | Props (summary) | Example |
   Cross-link to Theming for token customization. Cross-link to
   `docs/best-practices.md §7` (when to reach for the shipped
   component vs raw HTML).

**Seed material:** spec §3 (component model); spec §11 (component
library list); `crates/zero-scaffold/src/scaffold/.zero/
components.d.ts` (signatures); scaffold AGENTS.md §Component
library; README authoring primer (props, children).

**Tests:** none.

---

### Step 6: Write `docs/routing.md`

**Goal:** Cover `app.route()`, params, lazy imports, guards,
`load()`, `meta`, nested routes, navigation, active-link styling,
and route-scoped `fetch`. ≤ 400 lines.

**Files:**
- `docs/routing.md` (replace stub)

**Content outline:**

1. **Defining routes.** `app.route("/", Home)`, params (`:id`),
   wildcard `*`. First match wins.
2. **Lazy routes.** `app.route("/blog/:slug", () =>
   import("./routes/post.ts"))` for code splitting.
3. **The route module's exports.** `default` (component), `load`
   (data fetch), `meta` (route policy). Show the co-location
   pattern from `BEST_PRACTICES.md §5`.
4. **`load()` contract.** Receives `{ params, query, state, fetch
   }`. Side-effect-shaped (the framework awaits but doesn't pipe
   the return into the component). Pattern: hydrate a store; the
   component reads via `inject`. Cross-link to best-practices.
5. **Route guards.** `{ guard: ({ state, redirect }) => … }`.
   Auth-check example.
6. **Nested routes.** `children` array on the parent route; parent
   component renders `${children}`.
7. **Navigation.** Plain `<a href>`; the framework intercepts
   same-origin clicks. Programmatic: `navigate`, `back`, `forward`.
8. **Active-link styling.** `data-active` and `data-active-exact`
   attributes. CSS selectors.
9. **Route-scoped `fetch`.** The contract from spec §6: each
   navigation owns an `AbortController`; the injected `fetch`
   threads its signal; navigating away aborts in-flight requests
   automatically; caller-supplied signals compose. Cross-link to
   HTTP and to best-practices.
10. **Navigation lifecycle.** Reproduce the boot/navigation
    sequence diagram from spec §6 as an ASCII fenced block.

**Seed material:** spec §6 (router); `BEST_PRACTICES.md §5`
(routes); README §Routing; scaffold AGENTS.md §Navigation.

**Tests:** none.

---

### Step 7: Write `docs/http.md`

**Goal:** Cover `createHttp()`, methods, middleware, `HttpError`,
fetch threading. ≤ 400 lines.

**Files:**
- `docs/http.md` (replace stub)

**Content outline:**

1. **Why a wrapper.** Every realistic app fetches; signal-driven
   apps want middleware, cancellation, and typed errors.
2. **Constructing a client.** `createHttp()` returns a client.
   `client.get<T>(url, init?)`, `post`, `put`, `patch`, `delete`,
   plus `request<T>(input, init?)`.
3. **JSON I/O.** Plain-object bodies → `JSON.stringify` +
   `Content-Type: application/json`. JSON responses parsed
   automatically; non-JSON returns the raw `Response`.
4. **Errors.** `HttpError` (status, statusText, body) on non-2xx.
   Network failures: `TypeError`. Aborts: `AbortError`.
5. **Middleware.** `client.use(mw)`. Signature:
   `(req, next) => Promise<Response>`. Onion model — outermost-
   first down, innermost-first up. Three canonical examples:
   auth-header injector, 401-redirect, short-circuit (mock /
   cache).
6. **Route-scoped fetch threading.** Inside `load()`:
   `api.get(url, { fetch: ctx.fetch })`. Cross-link to Routing.
7. **One client per backend.** Cross-link to best-practices §6 for
   the organization pattern.

**Seed material:** spec §6 (route-scoped fetch); spec §11
(`zero/http` surface); `BEST_PRACTICES.md §6`; `runtime/zero-
http.d.ts`.

**Tests:** none.

---

### Step 8: Write `docs/testing.md`

**Goal:** Cover the test API, DOM helpers, in-memory DOM scope, web
platform shim list, spies. ≤ 400 lines.

**Files:**
- `docs/testing.md` (replace stub)

**Content outline:**

1. **Running tests.** `zero test`, `zero test pattern`,
   `--watch`, `--coverage`, `--update-snapshots`. Brief mention of
   `zero mutate` as a sibling command with a forward-link to
   Config & CLI.
2. **Structure API.** `describe`, `it`, `beforeEach`, `afterEach`,
   `beforeAll`, `afterAll`.
3. **Assertions.** Reproduce the `expect` matcher list from spec
   §8. One example each for the non-obvious ones
   (`toBeTemplateResult`, `toMatchSnapshot`).
4. **DOM helpers.** `render`, `find`, `findAll`, `text`, `fire`,
   `cleanup`. The `afterEach(cleanup)` discipline.
5. **Testing signals.** Direct test of `signal` / `computed`
   without rendering.
6. **Testing components.** Counter test from spec §8.
7. **Testing routes.** Pre-seed store; render route component.
   Note: `load()` is NOT invoked by `render` — the test seeds the
   store the route reads from.
8. **In-memory DOM scope.** What the runtime ships (real `Event`
   constructors with bubble/capture, classList/dataset/style,
   storage, matchMedia, navigator, crypto, observers, timers).
9. **Web Platform surface.** Reproduce spec §8 "Web Platform
   surface" section verbatim — the audited list (Fetch, URL,
   encoding, binary, structuredClone, queueMicrotask) and the
   "clear error" discipline. This is reference content.
10. **Spies.** `spy()` for call recording. Asserting Web API
    calls (the `localStorage.setItem = spy(...)` pattern).
11. **E2E tests.** Out of scope for `zero test` — use Playwright
    or similar.

**Seed material:** spec §8 (testing) — the largest single source.
`BEST_PRACTICES.md §9` for the testing patterns. Scaffold AGENTS.md
§Testing.

**Tests:** none.

---

### Step 9: Write `docs/theming.md`

**Goal:** Teach the design system: tokens, palette, layout
primitives, utilities, light/dark, brand theme, typography.
≤ 400 lines.

**Files:**
- `docs/theming.md` (replace stub)

**Content outline:**

1. **The design surface.** Three layers: framework-internal palette
   (`--gray-*` etc.), public semantic tokens (`--color-*`), non-
   color invariants (spacing, radius, fonts, shadow, border).
   Reproduce spec §7 token table.
2. **Layout primitives.** The six classes (`cluster`, `stack`,
   `frame`, `split`, `flank`, `grid`) with the "when to reach for
   which primitive" table verbatim from spec §7.1.
3. **Utilities.** `gap-*`, `pad-*`, `border` / `border-{t,r,b,l}`,
   alignment family, justify family, text alignment, flex
   direction. Brief table.
4. **Theming.** Light is default on `:root`; dark via
   `@media (prefers-color-scheme: dark)`; explicit
   `<html data-theme="…">` overrides both. No JavaScript toggle
   helper.
5. **Authoring a brand theme.** The 13-token public surface; one
   complete worked example reproducing the brand-theme snippet
   from `BEST_PRACTICES.md §8`.
6. **Typography.** Twelve utility classes (`.text-display`,
   `.text-h1`–`.text-h4`, `.text-eyebrow`, `.text-body`,
   `.text-small`, `.text-muted`, `.text-code`, `.text-link`,
   `.divider`). Geist + Geist Mono shipping in `.zero/fonts/`.
7. **Override an individual token.** Re-declare in
   `styles/app.scss` after the `@use '../.zero/styles/zero';`
   line. Cross-link to best-practices.

**Seed material:** spec §7 + §7.1; `BEST_PRACTICES.md §8`;
scaffold AGENTS.md §Styles.

**Tests:** none.

---

### Step 10: Write `docs/building-and-deploying.md`

**Goal:** Cover `zero build`, manifest shape, backend integration,
static deploy, `zero preview`. ≤ 200 lines.

**Files:**
- `docs/building-and-deploying.md` (replace stub)

**Content outline:**

1. **`zero build`.** Output structure (`dist/assets/*`,
   `dist/manifest.json`, `dist/index.html`). Flags (`--out`,
   `--analyze`, `--sourcemap`, `--target`).
2. **`manifest.json`.** Shape: logical name → hashed filename.
   Example contents.
3. **Backend integration.** Three steps: serve `dist/assets/`,
   read `dist/manifest.json`, inject `<script>` / `<link>` tags
   into server-rendered HTML.
4. **Static deploys.** `dist/index.html` is ready to upload to
   any static host.
5. **`zero preview`.** Serve the production build locally.

**Seed material:** current README "zero build" section + spec §1
"zero build" subsection.

**Tests:** none.

---

### Step 11: Write `docs/config-and-cli.md`

**Goal:** Full `zero.toml` schema + per-subcommand reference.
Reference-shaped, not tutorial. ≤ 400 lines.

**Files:**
- `docs/config-and-cli.md` (replace stub)

**Content outline:**

1. **`zero.toml` schema.** Reproduce the full schema from the
   current README §Configuration plus any keys the README is
   missing (cross-check against `crates/zero-config/`). Every
   key's purpose, default, and validation rule.
2. **CLI commands.** One subsection per: `init`, `update`, `dev`,
   `build`, `test`, `mutate`, `check`, `fmt`, `lint`, `gen`,
   `preview`, `upgrade`. Each subsection: synopsis, flags, what
   it does, exit codes where they matter.
3. **Global flags.** `-q/--quiet`, `-v/--verbose`, `--no-color`,
   `--version`, `-h/--help`.

**Seed material:** spec §1 entirely (CLI Interface). Cross-check
each command against the binary by running `zero <cmd> --help` and
reconciling.

**Tests:** none.

---

### Step 12: Write `docs/linting.md`

**Goal:** Single reference for everything `zero lint` catches.
SCSS rules + JS/TS rules + posture. ≤ 400 lines.

**Files:**
- `docs/linting.md` (replace stub)

**Content outline:**

1. **Running `zero lint`.** Command, output format, `--quiet`,
   exit codes. No config, no suppression, opinionated.
2. **SCSS / design system rules.** Reproduce the L01–L13 table
   from `crates/zero-scaffold/src/scaffold/AGENTS.md:660` as a
   reference table. For each rule, link to Theming for the
   underlying primitive.
3. **JS/TS framework idiom rules.** Reproduce the R01-S01 table
   from `zero-framework-spec.md` line 182 verbatim (plus P01 for
   parse errors). For each rule, link to the teaching chapter
   that explains the underlying primitive: R-rules → Reactivity;
   T-rules → Templates; C-rules → Components; I-rules → Getting
   Started (project layout); S01 → no link, just convention.
4. **Test-file exemptions.** Reproduce the "Tests
   (`*.test.{ts,js,tsx,jsx}` / `*.spec.{ts,js,tsx,jsx}`) are
   exempt from the T-rules and R03; R02, C01, C02, I01, I02, S01
   still apply" note from the spec.
5. **Authoring posture.** Three bullets: no `--fix`, no
   per-line disables, no config knobs. Why: drift is the bug;
   the rule is the contract.

**Seed material:** `zero-framework-spec.md §1` JS/TS lint table
(lines 178–198); `crates/zero-scaffold/src/scaffold/AGENTS.md`
"Common mistakes" tables; `issues/design-system-lint/spec.md`
for L-rule rationale; `issues/lint-js/spec.md` for R/T/C/I/S
rationale.

**Tests:** none.

---

### Step 13: Write `docs/api.md`

**Goal:** Flat search-in-page reference of every export across
every public module. Mirrors the deleted spec §11. ≤ 400 lines.

**Files:**
- `docs/api.md` (replace stub)

**Content outline:**

Five module sections, each with one row per export. Per export:
identifier, signature (copied from the `.d.ts`), one-line
description, optional forward-link to the teaching chapter.

1. **`"zero"`** — source: `runtime/zero.d.ts` (106 lines).
   `App` class plus `signal`, `computed`, `effect`, `html`, `each`,
   `ref`, `inject`, `navigate`, `back`, `forward`, `route`, `group`.
   Also `Signal<T>`, `Computed<T>`, `Ref<T>`, `TemplateResult`,
   `RouteView`, `StateTypes` interfaces.
2. **`"zero/test"`** — source: `runtime/zero-test.d.ts` (67 lines).
   `describe`, `it`, `beforeEach`, `afterEach`, `beforeAll`,
   `afterAll`, `expect`, `render`, `find`, `findAll`, `text`,
   `fire`, `cleanup`, `spy`, `settled`.
3. **`"zero/http"`** — source: `runtime/zero-http.d.ts` (36
   lines). `createHttp`, `HttpClient`, `HttpInit`, `HttpError`.
4. **`"zero/components"`** — source:
   `crates/zero-scaffold/src/scaffold/.zero/components.d.ts` (163
   lines). All 15 shipped components.
5. **`"zero/wc"`** — note: deferred / not yet exposed at runtime.
   Reserved-slot mention only; do not write API rows for un-
   shipped exports.

**Top-of-page note:** "Source of truth lives in `.zero/*.d.ts` —
this page mirrors those files for human reading and drifts
between releases. When a signature on this page disagrees with
the type files, trust the type files."

**Seed material:** spec §11 + the four `.d.ts` files.

**Tests:** none.

---

### Step 14: Write `docs/why-zero.md` and `docs/examples-tour.md`

**Goal:** Wrap up the long-tail chapters. `why-zero` is the long-
form companion to the README Tier-3 comparison table;
`examples-tour` bridges into the example apps and
best-practices. Both ≤ 200 lines.

**Files:**
- `docs/why-zero.md` (replace stub)
- `docs/examples-tour.md` (replace stub)

**`why-zero.md` outline:**

For each design decision, a short subsection with the decision,
the win, and the tradeoff:

1. **No virtual DOM.** Win: smaller runtime, no diffing.
   Tradeoff: granular update model is unusual; some libraries
   that assume a vDOM (e.g. React-shaped UI libraries) don't
   port.
2. **No `node_modules`.** Win: single binary, no supply-chain
   surface, no install step beyond Cargo. Tradeoff: lose the
   npm ecosystem; rebuild what you need or interop via web
   components (`zero/wc`, future).
3. **Signals over hooks.** Win: granular updates without re-
   renders; auto-tracking removes deps arrays. Tradeoff:
   different mental model; reader has to learn signals.
4. **Plain functions, no classes.** Win: testable, no
   lifecycle coupling, no `this`. Tradeoff: less familiar to
   developers coming from `React.Component` legacy code.
5. **No file-system routing.** Win: explicit, debuggable, no
   build-time magic. Tradeoff: a `routes/` table is more
   typing than Next-style discovery.
6. **Single binary for everything.** Win: zero toolchain
   debugging. Tradeoff: extending the build (e.g. PostCSS
   plugins) means changing the CLI.

**Seed material:** spec §13 "Key Design Decisions Summary"
(verbatim table — extract and prose-ify each row).

**`examples-tour.md` outline:**

1. **`examples/counter/`** — what it demonstrates (~50 LOC):
   `signal`, `app.state`, `inject`, `app.route`, `html`.
   Pointer at `web/src/app.ts` and `web/src/routes/home.ts`.
2. **`examples/todos/`** — what it adds: `each` keyed
   rendering, the module-store pattern, structured single
   signal, typed key registry. Pointer at
   `web/src/state.ts`, `web/src/stores/todos.ts`,
   `web/src/routes/home.ts`.
3. **`examples/tracker/`** — what it adds: auth flow,
   query-param filters, route guards, `load()`,
   status-tagged signal, nested route layout, `zero/http`
   end-to-end. Pointer at `web/src/app.ts`,
   `web/src/stores/auth.ts`, `web/src/routes/issues/`.
4. **What to read after.** Forward-link to
   `docs/best-practices.md` (the long-form companion).

**Tests:** none for either file.

---

### Step 15: Write `docs/index.md` (landing) and enable GitHub Pages

**Goal:** Land the navigation hub for the Pages site and turn on
the hosting. This is the last content step before the spec
deletion in Step 17 becomes safe.

**Files:**
- `docs/index.md` (replace stub)
- `docs/_config.yml` (modify — replace `robap/zero` placeholder)

**Changes:**

1. **`docs/index.md`** — landing page, four sections:
   - **Intro** (~3 sentences). What zero is. Link to README for
     pitch / install.
   - **Start here.** Two links: Getting Started, Reactivity.
   - **Reference.** Bulleted list pointing at Templates,
     Components, Routing, HTTP, Testing, Theming,
     Building & Deploying, Config & CLI, Linting, API.
   - **After your first app.** Best Practices, Examples Tour,
     Why zero.

2. **Resolve the Pages URL.** From the working directory, run
   `git remote get-url origin` to discover the GitHub remote.
   Derive `robap` and `zero` and rewrite `_config.yml`'s
   `aux_links` accordingly. Pages URL for later steps:
   `https://robap.github.io/zero/`. Save it as a note in
   `issues/user-docs/plan.md` near this step (or pass to the
   executor); Step 18 needs it.

3. **Enable GitHub Pages (manual instruction).** Insert this
   block at the head of `docs/index.md`'s body (HTML comment,
   not rendered):
   ```html
   <!--
   GitHub Pages setup (one-time, manual via repo Settings):
     Settings → Pages → Source: Deploy from a branch
     Branch: main, Folder: /docs
     Save. First build takes ~1 minute.
   -->
   ```
   Print the same instructions to stdout so the user sees them
   and performs the action. **This step is not Claude-
   executable in the repo settings**; the executor either asks
   the user to do it, or the user does it on their end before
   merging.

**Tests:** none. After the user enables Pages, manually verify
the site builds (GitHub Pages action runs automatically on
push to `main`) and renders every chapter.

---

### Step 16: Move `BEST_PRACTICES.md` → `docs/best-practices.md` and update outbound references

**Goal:** Get the long-form companion onto the Pages site;
update every live reference to its new location. No content
rewrite; the file moves verbatim.

**Files:**
- `BEST_PRACTICES.md` → `docs/best-practices.md` (`git mv`)
- `docs/best-practices.md` (modify — only the three outbound
  references to `zero-framework-spec.md`)
- `crates/zero-scaffold/src/scaffold/AGENTS.md` (modify line
  1156)
- `docs/routing.md`, `docs/http.md`, `docs/components.md`,
  `docs/testing.md` (modify — add forward-links to relevant
  best-practices sections)

**Changes:**

1. **`git mv BEST_PRACTICES.md docs/best-practices.md`.**

2. **In `docs/best-practices.md`**, update the three spec
   references found at lines 3, 534, 599 of the original:
   - Line 3: "long-form companion to `zero-framework-spec.md`."
     → "long-form companion to the
     [user guide](./index.html)."
   - Line 534: "See `zero-framework-spec.md` §8 'Web Platform
     surface in `zero test`' for the closed list" →
     "See [docs/testing.md](./testing.html#web-platform-surface)
     for the closed list".
   - Line 599: `- `zero-framework-spec.md` — capability
     reference.` → remove this bullet from the "Related docs"
     section (the spec no longer exists post-Step-17; the
     adjacent `BEST_PRACTICES.md` bullet pointing at itself is
     also removed if present).

3. **Add Jekyll front-matter** to `docs/best-practices.md` so it
   slots into the nav:
   ```markdown
   ---
   title: Best Practices
   nav_order: 14
   ---
   ```
   (`nav_order: 14` was reserved in Step 1.) Replace the file's
   existing `# Zero Best Practices` H1 with the front-matter's
   title; Jekyll will render the front-matter title as the H1.

4. **Update
   `crates/zero-scaffold/src/scaffold/AGENTS.md:1156`** from:
   ```
   For longer rationale and worked examples, see `BEST_PRACTICES.md` at the framework repo root.
   ```
   to:
   ```
   For longer rationale and worked examples, see the [Best Practices](https://robap.github.io/zero/best-practices.html) chapter of the user guide.
   ```
   Replace `robap/zero` with the resolved values from
   Step 15.

5. **Forward-links from chapters into best-practices.** Append a
   `## See also` section near the end of:
   - `docs/routing.md` → "→ Best Practices §5 Routes"
   - `docs/http.md` → "→ Best Practices §6 HTTP"
   - `docs/components.md` → "→ Best Practices §7 Component
     usage"
   - `docs/testing.md` → "→ Best Practices §9 Testing"

**Tests:**
- Run `cargo test --workspace` — the scaffold AGENTS.md is
  embedded in the binary, so any path-format mistake there is
  worth catching.
- Run `cargo test -p zero-scaffold` specifically. Existing
  scaffold tests compare AGENTS.md content against fixtures or
  hash it — if any test asserts the exact line 1156 wording,
  update the assertion alongside the change.

---

### Step 17: Delete `zero-framework-spec.md` and update `.claude/commands/refine.md`

**Goal:** Remove the now-redundant monolithic spec and the only
remaining tooling reference to it. Every load-bearing chunk of
its content has been migrated by this point.

**Files:**
- `zero-framework-spec.md` (delete via `git rm`)
- `.claude/commands/refine.md` (modify)
- `README.md` (modify — interim edit only; full rewrite happens
  in Step 18, but the spec-link line must be killed now to keep
  the tree consistent)

**Changes:**

1. **Verify the migration is complete.** Before deletion, run a
   quick audit: for each row of the migration table in
   `issues/user-docs/spec.md §R7`, confirm the listed
   `docs/*.md` page contains the corresponding content. If any
   cell is unpopulated, return to its step and finish first.

2. **`git rm zero-framework-spec.md`.**

3. **`.claude/commands/refine.md:13`** — replace:
   ```
   - `zero-framework-spec.md` — understand all planned work and where this item fits in the larger sequence
   ```
   with:
   ```
   - `docs/index.md` — overview of every shipped capability and where the user-facing reference lives.
   - `issues/` — browse the spec/plan of any past or in-flight item adjacent to the new one.
   ```
   (Two bullets; the previous orientation step was a single
   bullet pointing at the spec.)

4. **`.claude/commands/refine.md:14`** currently reads:
   ```
   - All files under `src/` — understand what exists already
   ```
   This is a stale instruction — the repo has no `src/`; it's a
   Rust workspace with `crates/`. Replace with:
   ```
   - Skim `crates/` (Rust workspace) and `runtime/` (JS runtime)
     to understand the shape of the code touched by the item.
   ```

5. **`README.md`** — interim edit only: kill the one live
   reference to the deleted file. The current README contains
   the line:
   ```
   The full surface is documented in `zero-framework-spec.md`;
   this section is a working primer.
   ```
   Replace with:
   ```
   See the [user guide](https://robap.github.io/zero/) for
   the full reference; this section is a working primer.
   ```
   The README will be rewritten end-to-end in Step 18; this
   edit is purely to avoid a dangling reference between the
   spec deletion and the rewrite.

6. **Verify no other live references remain.** Run:
   ```
   grep -rn "zero-framework-spec" \
     --include="*.md" --include="*.rs" --include="*.ts" \
     --include="*.js" --include="*.toml" \
     . \
     | grep -v target/ \
     | grep -v issues/ \
     | grep -v .git/
   ```
   Expected output: empty (or only the historical references
   inside `issues/*/{spec,plan,review}.md` which the
   `grep -v issues/` filter removes — the audit's whole point is
   to confirm those are the only remaining references).

**Tests:**
- `cargo test --workspace` — the scaffold AGENTS.md and any
  embedded markdown should be untouched by this step; sanity-
  check that they still build.
- Manual: open `.claude/commands/refine.md` and verify the
  orientation step reads coherently end-to-end.

---

### Step 18: Rewrite `README.md` (three-tier pitch with live Pages URLs)

**Goal:** Replace the existing 354-line README with the layered
pitch the spec calls for. Tier 1 must be readable in one minute.
Live Pages URL is known by this step (Step 15 resolved it).

**Files:**
- `README.md` (replace contents)

**Structure (target ~200 lines):**

```markdown
# zero

A zero-dependency frontend framework. Single Rust binary; no
node_modules; signals instead of hooks.

```js
import { App, html, signal } from "zero";

function Counter() {
  const count = signal(0);
  return html`
    <button @click=${() => count.update(n => n + 1)}>
      Clicked ${count} times
    </button>
  `;
}

new App().route("/", Counter).run("#app");
```

- **Zero npm dependencies.** The CLI is the framework.
- **One binary.** Dev server, transpiler, test runner, builder,
  linter, formatter, generator.
- **No virtual DOM.** Granular reactive updates via signals.
- **Components are plain functions.** No classes, no JSX.

## Install

```sh
cargo install zero --locked
```

## Get started

```sh
zero init
zero dev
```

Open `http://localhost:3000`.

→ Full documentation: <PAGES_URL>

---

## Quickstart

(Tier 2 — ~80 lines. Distilled walkthrough taking a reader from
nothing to a running edit loop. Closes with "Going deeper:
[Best Practices](<PAGES_URL>/best-practices.html) — application
patterns for real apps.")

---

## How zero compares

(Tier 3 — comparison table + ~3 sentences of disclaimer.)

|                  | zero       | React      | Vue        | Solid      | Svelte     |
|------------------|------------|------------|------------|------------|------------|
| Build tool       | built-in   | Vite (etc) | Vite (etc) | Vite (etc) | Vite       |
| npm dependencies | 0          | many       | many       | many       | many       |
| State model      | signals    | hooks      | refs       | signals    | runes      |
| Virtual DOM      | no         | yes        | yes        | no         | no         |
| Component model  | functions  | functions  | functions  | functions  | files      |
| Bundle posture   | ~4 KB rt   | ~40 KB     | ~30 KB     | ~7 KB      | compiled   |

Numbers and claims are coarse and age fast; the table is for
orienting an evaluator, not for benchmarking. Pick the framework
that matches your problem, not the one that wins the table.
```

**Specific edits:**

1. Replace `<PAGES_URL>` with the value resolved in Step 15
   (e.g. `https://robapodaca.github.io/zero`).
2. Drop the current README sections: "Commands" (the per-
   subcommand reference — now in `docs/config-and-cli.md`),
   "Writing a zero app" (the authoring primer — distributed
   across `docs/{getting-started,reactivity,templates,
   components,routing}`), "Configuration (`zero.toml`)" (now in
   `docs/config-and-cli.md`), "Running the runtime tests" (out
   of scope; CONTRIBUTING.md follow-up), "Repository layout"
   (internal detail), "Development workflow" (CONTRIBUTING.md
   follow-up).
3. Keep "Prerequisites" only if Tier 2 references Cargo — and
   make it one sentence.

**Tests:** none. Manual: confirm Tier 1 is ≤ 40 lines and ≤ 1
screen on a typical viewport.

---

### Step 19: Final link-check pass

**Goal:** Catch every broken or stale link before declaring the
work done. Belt-and-suspenders gate on the deletion of the
spec.

**Files:** read-only audit; touches files only where a broken
link surfaces.

**Changes:**

1. **Internal links in `docs/*.md`.** Every `[text](./other.md)`
   or `[text](./other.html)` resolves to an existing chapter.
   The simple shell pass:
   ```
   for f in docs/*.md; do
     grep -oE '\]\(\.\/[a-z-]+\.(md|html)' "$f" | sort -u
   done
   ```
   Cross-check each unique target against the directory.

2. **Cross-chapter forward-links.** Each "Cross-link" / "→ See"
   reference in the plan steps above corresponds to an actual
   link in the rendered page. Spot-check.

3. **README links.** Open `README.md` on github.com (after
   merge) or `cat README.md | <markdown-to-html>` locally;
   verify every link resolves.

4. **`docs/best-practices.md` `→ See examples/...` pointers.**
   They were relative to repo root in the original
   `BEST_PRACTICES.md`; in the moved file at
   `docs/best-practices.md`, the relative path now needs to
   climb one level: `../examples/...`. Verify and fix.

5. **`grep -rn "zero-framework-spec"` outside `issues/` /
   `target/`** — expected empty (Step 17 already ran this; this
   is the regression check).

6. **`grep -rn "BEST_PRACTICES.md"` outside `issues/` /
   `target/`** — expected empty. (The scaffold AGENTS.md
   reference was updated in Step 16.)

**Tests:** `cargo test --workspace` one final time. All green.

---

## Risks and Assumptions

1. **GitHub Pages enablement is manual.** Step 15's repo-
   settings change is outside the Claude executor's surface
   (no API call wired). If the user does not enable Pages, the
   `<PAGES_URL>` referenced by Step 18 and the scaffold
   AGENTS.md links go to a 404. Mitigation: Step 15 prints the
   instructions and pauses until the user confirms.

2. **`just-the-docs` theme drift.** `remote_theme:
   just-the-docs/just-the-docs` pins HEAD of that repo, not a
   tag. If upstream ships a breaking change, the site renders
   degraded but still functional (markdown still parses).
   Mitigation in follow-up: pin to a tag.

3. **Embedded AGENTS.md path mismatch.** The scaffold AGENTS.md
   is embedded into the binary by `crates/zero-scaffold`. If
   any test in `crates/zero-scaffold/` asserts the exact
   content of the file, the line-1156 edit in Step 16 breaks
   that test. Risk-mitigation: run `cargo test -p zero-
   scaffold` as the first thing after the edit; update any
   golden-file assertion alongside the source edit.

4. **`<PAGES_URL>` resolution.** Step 15 uses
   `git remote get-url origin` — this works only if `origin`
   points at GitHub. If the executor runs in a clone with no
   `origin`, or `origin` is a non-GitHub mirror, the executor
   pauses and asks the user.

5. **README authoring-primer seed material drifts.** The
   primer in today's README covers the surface as of its
   write date. Some details (e.g. the exact set of event
   modifiers) have moved on. Steps 3–7 (the chapters that
   absorb that material) re-source from the runtime / spec /
   scaffold AGENTS.md as the canonical sources, not from the
   primer.

6. **`docs/api.md` drift.** The plan deliberately accepts that
   `api.md` will drift between releases of the framework. Per
   spec Constraints, `.zero/*.d.ts` files remain the source of
   truth. The top of `docs/api.md` says this in the page
   intro; no automation enforces it.

7. **Step ordering.** Steps 2–14 (chapter writing) can be
   reordered freely. The plan presents them in reader
   sequence because cross-links are easier to write when the
   target chapter already exists. The chapters that are
   forward-linked from earlier chapters (Reactivity, Routing,
   Components, etc.) are written before the chapters that
   link into them (best-practices, why-zero, examples-tour).
   Step 15 (index + Pages enablement) must run AFTER all
   chapter content exists so the landing page can cross-link
   correctly. Step 17 (spec deletion) must run AFTER every
   row of the migration table in spec R7 is populated.

8. **Test exemption mismatch with shipped lint rules.** Step 12
   reproduces the "Tests are exempt from T-rules and R03"
   line from the spec. The shipped lint-js code might exempt
   a different set; the source of truth is the rule code
   under `crates/zero-lint/src/js/rules/`. If divergence is
   discovered while writing `docs/linting.md`, update the
   docs to match the code.
