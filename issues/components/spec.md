# Spec: Component Library

## Problem Statement

The framework roadmap (Phase 9) calls for a built-in component library — Button, Input, Dialog, and friends — that real applications can use without rebuilding the same dozen HTML-element wrappers per project. Phase 7 (the `.zero/` boundary) and Phase 8 (alignment utilities) gave zero a tokens-and-utilities foundation. This issue puts the next layer on top.

Without component primitives, every zero app reinvents the same checkbox-toggle wiring, the same dialog-with-backdrop, the same toast positioning, the same input-with-label markup. Each reinvention is a chance to drift off the design-system token palette. With a fixed component set that consumes only `var(--*)` tokens, applications stay on-spec by default, and the framework gets to dogfood its own primitives.

This issue ships fourteen components as TypeScript modules under `.zero/components/`, distributed and refreshed through the existing `zero update` plumbing. Each component is a plain function in zero's once-per-mount style; stateful props accept signals directly. A new `showcase/` zero project — itself a real zero app — demonstrates every component with a light/dark theme switcher and builds via `zero build`.

## Background

### What exists today (relevant pieces)

- **`.zero/` directory (Phase 7).** Hidden, gitignored, framework-owned. Refreshed by `zero update`. Currently contains:
  - `zero.d.ts`, `zero-test.d.ts` — type declarations for `"zero"` and `"zero/test"`.
  - `styles/_tokens.scss`, `_base.scss`, `_layout.scss`, `_utilities.scss`, `_alignment.scss`, `zero.scss` — the design system.
- **`framework_manifest()` in `src/scaffold.rs`.** Returns the list of `(path, content)` tuples that `zero init` and `zero update` write into `.zero/`. Currently eight entries; each is a `TPL_*` constant pointing at `include_str!("scaffold/.zero/...")`. The test `framework_manifest_lists_eight_files` (or its current equivalent) asserts the length.
- **Design-system tokens (Phases 7–8).** CSS custom properties on `:root`: `--space-{xs,sm,md,lg,xl}`, `--color-{bg,surface,text,text-muted,primary,primary-fg,border}`, `--radius-{sm,md,lg}`, `--font-{sm,md,lg,xl}`, `--weight-{normal,bold}`, `--leading-{tight,normal}`, `--shadow-{sm,md,lg}`, `--border-{thin,md,thick}`. Dark-mode overrides for the seven `--color-*` tokens via `prefers-color-scheme: dark` and `[data-theme="dark"]`.
- **Reactivity primitives.** `signal`, `computed`, `effect`. Components are plain functions that run once per mount; reactivity updates DOM granularly via bare-signal-in-template or reactive-block patterns.
- **Module resolution for `"zero"` and `"zero/test"`.** The dev server (`src/dev/transpile.rs`) and bundler (`src/build/bundler.rs` / `src/build/resolver.rs`) already map these bare specifiers to the framework's bundled runtime. This issue extends that map with `"zero/components"`.
- **Test runner.** `zero test` discovers `*.test.ts` / `*.spec.ts` recursively. Tests run against the lightweight DOM in `runtime/dom-shim.js`. `render`, `find`, `findAll`, `text`, `fire`, `cleanup`, `spy` are imported from `"zero/test"`.

### Decisions made during refine

The user confirmed each of the following:

- **Roster: fourteen components.** Button, Input, TextArea, Checkbox, Radio, Select, Toggle (switch), Card, Dialog, Spinner, Badge, Avatar, Toast, Tabs.
- **Distribution: framework-owned under `.zero/components/`.** Each component ships as a `.ts` file refreshed by `zero update`. Mirrors the Phase 7 pattern for `.zero/styles/`.
- **Import path: bare specifier `"zero/components"`.** The framework's dev-server and bundler resolve `"zero/components"` to `.zero/components/index.ts` (which re-exports every component). Consistent with how `"zero"` and `"zero/test"` already work.
- **CSS location: per-component SCSS partials in `.zero/styles/components/`.** One partial per component (`_button.scss`, `_input.scss`, …, `_tabs.scss`). A new aggregate `.zero/styles/_components.scss` `@use`s each partial; the existing `zero.scss` aggregate gains one `@use 'components';` line.
- **Class-name convention: flat names wrapped in `@layer components`.** Class names stay short (`.button`, `.button-primary`, `.dialog`, `.dialog-open`) for consistency with design-system primitives. All component CSS sits inside `@layer components { ... }`. Unlayered user CSS in `styles/app.scss` automatically wins on override without prefixes or `!important`.
- **Stateful props: accept signals directly.** Components that have observable state (Checkbox, Toggle, Radio, Input, TextArea, Select, Dialog, Toast, Tabs) take a `Signal` as the prop. The component reads `.val` and writes via `.set()` on the passed signal. Parents own the signal lifecycle.
- **Showcase: full zero project at `/showcase`.** Top-level directory in this repo. Has its own `zero.toml`, `index.html`, `src/`. Light/dark switcher toggles `<html data-theme="...">`. Built with `zero build`; served with `zero dev` from inside `showcase/`.
- **Tests: one `*.test.ts` per component.** Fourteen test files. Each renders the component and exercises one or two key interactions (Checkbox toggles its signal; Dialog opens/closes via its `open` signal; Button fires its `onClick` spy). No exhaustive prop matrix.

### Component contract

All components share the runtime signature:

```ts
type Component<P = {}> = (props?: P) => TemplateResult
```

Uniform conventions across the roster:

- **Variants are string-typed props.** `Button({ variant: "primary" | "secondary" | "ghost" | "danger" })`. Each variant maps to a `.{component}-{variant}` class. The component picks `variant ?? <default>` and emits `class="{component} {component}-${variant}"`.
- **Sizes are string-typed props.** `Button({ size: "sm" | "md" | "lg" })` — when applicable. Maps to `.{component}-{size}`.
- **Boolean states are signals when reactive, plain booleans otherwise.** A `disabled` prop is a plain boolean (rarely reactive). A `checked` / `open` / `active` prop is a `Signal<boolean>`.
- **Children are a `children` prop.** Standard zero pattern. Multiple slots are multiple props (`Card({ header, children })`).
- **Event callbacks are plain functions.** `Button({ onClick: () => ... })`. Tests pass `spy()` as the callback.

### Component roster — props sketch

(Final prop names and defaults are confirmed by the plan phase; this is the spec-level contract.)

| Component  | Stateful prop(s)               | Variants                                         | Sizes                  | Other props |
| ---------- | ------------------------------ | ------------------------------------------------ | ---------------------- | ----------- |
| `Button`   | —                              | `primary` (default), `secondary`, `ghost`, `danger` | `sm`, `md` (default), `lg` | `disabled`, `loading`, `onClick`, `children` |
| `Input`    | `value: Signal<string>`        | —                                                | `sm`, `md`, `lg`       | `type`, `placeholder`, `disabled`, `label` |
| `TextArea` | `value: Signal<string>`        | —                                                | —                      | `rows`, `placeholder`, `disabled`, `label` |
| `Checkbox` | `checked: Signal<boolean>`     | —                                                | —                      | `label`, `disabled` |
| `Radio`    | `selected: Signal<string>`     | —                                                | —                      | `name`, `value`, `label`, `disabled` |
| `Select`   | `value: Signal<string>`        | —                                                | `sm`, `md`, `lg`       | `options: { value, label }[]`, `disabled`, `label` |
| `Toggle`   | `checked: Signal<boolean>`     | —                                                | —                      | `label`, `disabled` |
| `Card`     | —                              | `surface` (default), `outlined`                  | —                      | `title?`, `children` |
| `Dialog`   | `open: Signal<boolean>`        | —                                                | `sm`, `md`, `lg`       | `title?`, `children`, `onClose?` |
| `Spinner`  | —                              | `primary`, `muted`                               | `sm`, `md`, `lg`       | `label?` (sr-only) |
| `Badge`    | —                              | `default` (default), `primary`, `success`, `warning`, `danger` | `sm`, `md` | `children` |
| `Avatar`   | —                              | —                                                | `sm`, `md`, `lg`, `xl` | `src?`, `alt`, `initials?` |
| `Toast`    | `open: Signal<boolean>`        | `info` (default), `success`, `warning`, `danger` | —                      | `message`, `duration?`, `onDismiss?` |
| `Tabs`     | `active: Signal<string>`       | —                                                | —                      | `tabs: { id, label }[]`, `panels: Record<string, TemplateResult>` |

### File layout

```
.zero/
├── components/
│   ├── index.ts          # re-exports every component
│   ├── Button.ts
│   ├── Button.test.ts
│   ├── Input.ts
│   ├── Input.test.ts
│   ├── …                 # 14 components × (component + test) = 28 files
│   └── Tabs.test.ts
├── styles/
│   ├── _tokens.scss      # unchanged
│   ├── _base.scss        # unchanged
│   ├── _layout.scss      # unchanged
│   ├── _utilities.scss   # unchanged
│   ├── _alignment.scss   # unchanged
│   ├── components/
│   │   ├── _button.scss
│   │   ├── _input.scss
│   │   ├── …
│   │   └── _tabs.scss
│   ├── _components.scss  # aggregate: @use 'components/button'; ...
│   └── zero.scss         # adds @use 'components';
├── zero.d.ts             # unchanged
├── zero-test.d.ts        # unchanged
└── components.d.ts       # declares module "zero/components" for editors

showcase/
├── zero.toml
├── index.html
├── .zero/                # committed (showcase is internal tooling, not a user project)
├── src/
│   ├── app.ts            # routes per component + theme signal
│   └── routes/
│       ├── home.ts       # overview + theme toggle
│       ├── button.ts
│       └── …             # one route per component
├── styles/
│   └── app.scss          # @use '../.zero/styles/zero';
└── .gitignore            # dist/ only (do NOT gitignore .zero/)
```

## Requirements

### Component library

1. A new `.zero/components/` directory ships fourteen component files plus an `index.ts` re-export aggregate. Each component is a TypeScript module with a single default export — the component function.

2. Components are plain functions matching `type Component<P> = (props?: P) => TemplateResult`. They never use `class`, `extends`, or web-component machinery. They run once per mount and rely on signals + reactive blocks for reactivity.

3. Stateful props accept signals directly. A component reads `.val` (typically inside a reactive block or template position) and writes via `.set()`. The parent owns the signal — the component must not create or replace it.

4. Components consume **only** `var(--*)` tokens for spacing, colors, radii, fonts, weights, line heights, shadows, and border widths. Numeric literals appear only where there is no token (e.g. `opacity`, `transition-duration`, `z-index`).

5. `.zero/components/index.ts` re-exports every component as a named export. Suggested shape per line: `export { default as Button } from "./Button.ts"`.

6. Each component's prop types are declared in TypeScript at the top of the component's `.ts` source (as a local `type Props = { ... }` or exported `interface ButtonProps`). The plan phase decides whether to also export each prop type from `.zero/components/index.ts`.

### Component CSS

7. Each component has a partial under `.zero/styles/components/_{name}.scss` (lowercase, matching the component's class root). Every rule lives inside `@layer components { ... }`.

8. A new aggregate `.zero/styles/_components.scss` `@use`s each per-component partial in alphabetical order. The aggregate is itself a partial; it is not addressable as a standalone stylesheet.

9. `.zero/styles/zero.scss` gains one new line — `@use 'components';` — placed last in the aggregate. Final order: tokens → base → layout → utilities → alignment → components.

10. No component partial uses `!important`. No component partial uses inline hex codes or magic numbers. All color, spacing, radius, font, shadow, and border values come from `var(--*)` tokens.

11. Component class names follow the design-system flat-name convention: lowercase, dash-separated, no framework prefix. Variants are dash-suffixed (`.button-primary`). Sizes are dash-suffixed (`.button-sm`). State classes are dash-suffixed (`.dialog-open`). No BEM `--` / `__` notation.

12. The `@layer components` wrapping ensures unlayered user CSS in `styles/app.scss` automatically overrides framework component rules without specificity tricks or `!important`. This is documented in `AGENTS.md` (see Requirement 30).

### Module resolution

13. The bare specifier `"zero/components"` resolves to `.zero/components/index.ts` in both:
    - The dev-server transpile pipeline (`src/dev/transpile.rs` or its resolver call site).
    - The production bundler (`src/build/resolver.rs` / `src/build/bundler.rs`).

14. The runtime exposes no new top-level export from `"zero"`. Components live behind `"zero/components"` only. Existing imports from `"zero"` and `"zero/test"` are unchanged.

15. A type-declaration file `.zero/components.d.ts` declares the module `"zero/components"` and its named exports for editor support. The file is part of `framework_manifest()` and is refreshed by `zero update`.

### Scaffold registration (`src/scaffold.rs`)

16. New `TPL_*` constants added for every new file:
    - 14 component `.ts` files.
    - 14 component `.test.ts` files.
    - `.zero/components/index.ts`.
    - 14 component SCSS partials.
    - `.zero/styles/_components.scss` aggregate.
    - `.zero/components.d.ts`.
    - Updated `.zero/styles/zero.scss` (single-line addition; existing constant gets a new value).

17. `framework_manifest()` gains an entry per new file. Manifest size grows from 8 to **approximately 53** (8 existing + 14 components + 14 component tests + 1 components index + 14 component SCSS partials + 1 components SCSS aggregate + 1 components.d.ts). The plan phase confirms the exact count after finalizing the per-file list.

18. The existing `framework_manifest_lists_eight_files` test is renamed and updated. Its expected-path set gains every new path; the length assertion is bumped to the new total.

19. New scaffold tests assert:
    - `components_index_re_exports_all` — `.zero/components/index.ts` exists and contains `export` for each of the 14 component names.
    - `component_files_emitted` — each of the 14 `.zero/components/{Name}.ts` files exists, is non-empty, and contains a `class="{name}` substring (the base class).
    - `component_test_files_emitted` — each of the 14 `.zero/components/{Name}.test.ts` files exists and imports from `"zero/test"`.
    - `component_partials_use_layer_components` — each `.zero/styles/components/_{name}.scss` contains `@layer components` and does not contain `!important`.
    - `components_aggregate_uses_each_partial` — `.zero/styles/_components.scss` contains `@use 'components/button'` (and a similar line for each of the 14 components).
    - `zero_scss_contains_aggregate_uses` — extended to also assert `@use 'components'` appears in `zero.scss`.

### Showcase project

20. A new top-level directory `showcase/` contains a full zero project. Required files:
    - `showcase/zero.toml` — minimal config (dev port, build output dir).
    - `showcase/index.html` — entry HTML following the scaffold pattern; `<html data-theme="">` (empty initial value); `<link rel="stylesheet" href="/styles/app.scss">`.
    - `showcase/src/app.ts` — builds the App, registers a `theme` signal under `app.state("theme", signal("auto"))`, registers one route per component plus a `/` overview route.
    - `showcase/src/routes/home.ts` — landing route. Theme toggle (cycles `auto` → `light` → `dark`), a navigation cluster linking to each per-component route, and a short intro.
    - `showcase/src/routes/{button,input,textarea,checkbox,radio,select,toggle,card,dialog,spinner,badge,avatar,toast,tabs}.ts` — one route per component. Each route renders the component in its primary variants and sizes with at least one interactive instance bound to a per-route signal.
    - `showcase/styles/app.scss` — `@use '../.zero/styles/zero';` followed by showcase-specific layout rules.
    - `showcase/.gitignore` — gitignores `dist/` only.

21. The showcase's `.zero/` directory is **committed to git**. Rationale: the showcase is a fixed in-repo dev artifact, not a user-generated project. `showcase/.gitignore` does **not** include `.zero/`. (User projects gitignore `.zero/` because the binary regenerates it; the showcase pins to whatever's currently in the binary.)

22. The showcase's `.zero/` is regeneratable: running `zero update --yes` from inside `showcase/` produces identical files to the manifest. CI can use this to detect drift.

23. The showcase boots successfully with `zero dev` from inside `showcase/`. The dev-server bare-specifier resolver finds `"zero/components"` at `showcase/.zero/components/index.ts`. A new integration test (`tests/e2e_showcase_dev.rs`, or extension to an existing test) confirms `zero dev` starts and the home route renders.

24. The showcase builds successfully with `zero build` from inside `showcase/`. Output lands in `showcase/dist/`. A new integration test (or extension to `tests/build_full.rs`) confirms the built showcase contains the home route HTML and that the hashed compiled CSS contains `@layer components`.

25. The theme toggle on the home route writes to `document.documentElement.dataset.theme`. Mechanism: an `effect(() => { document.documentElement.dataset.theme = theme.val })`. Whether this lives in `home.ts` (component-scoped) or `app.ts` (mount-scoped global) is for the plan phase.

### Tests

26. One `*.test.ts` per component, all under `.zero/components/`. File names mirror the component (`Button.test.ts`, `Input.test.ts`, …, `Tabs.test.ts`).

27. Each test file imports the component (relatively, e.g. `./Button.ts`, since the file lives next to its target) and `"zero/test"` helpers, then:
    - Renders the component with default props; asserts the rendered DOM contains the base class (e.g. `find(el, ".button")` succeeds for `Button`).
    - Exercises one or two key interactions specific to the component:
      - `Button` — fires its `onClick` spy on `click`.
      - `Input` — input events update the `value` signal.
      - `Checkbox` — clicking flips `checked.val`.
      - `Radio` — clicking sets `selected.val` to the radio's `value`.
      - `Select` — change events update the `value` signal.
      - `Toggle` — clicking flips `checked.val`.
      - `Dialog` — flipping `open.val` toggles the dialog's `dialog-open` class.
      - `Toast` — flipping `open.val` shows/hides the toast.
      - `Tabs` — clicking a tab updates `active.val`; the matching panel renders.
      - (Display-only components — `Card`, `Spinner`, `Badge`, `Avatar`, `TextArea` — render with representative prop sets; one assertion per variant.)
    - Cleans up with `afterEach(cleanup)`.

28. The component tests are exercised in framework CI via a new integration test (`tests/component_library.rs`) that runs `zero test` inside `showcase/` and asserts the report shows all component tests passing. The exact mechanism (does the test harness already run `zero test` from inside a project? — see `tests/e2e_init_test.rs` for the pattern) is for the plan phase.

29. The component tests **do** ship to user projects (they live in `.zero/components/`, which is part of the manifest). This means every `zero test` run in a user project also runs the framework's component tests. The trade-off — visible, dogfooded, slightly noisier user output — was accepted in refine; the plan phase confirms.

### Documentation

30. `src/scaffold/AGENTS.md` gains a new `## Components` section, placed after `## Styles`. Content:
    - One-paragraph intro: "zero ships a fixed component library under `.zero/components/`. Import via `\"zero/components\"`. Components are plain function components in zero's once-per-mount style; stateful props accept signals directly."
    - Import example: `import { Button, Input, Dialog } from "zero/components"`.
    - A table listing every component, a one-line purpose, and the primary stateful prop type (or `—`).
    - One subsection per component category (Form Inputs, Display, Overlay, Feedback) showing a single usage example for the most representative component in that category.
    - A short note on the `@layer components` mechanic: "Component CSS lives in `@layer components`, so any rule in `styles/app.scss` automatically overrides framework component rules without `!important` or extra specificity."
    - A pointer to `showcase/` as the canonical live example.
    - The `.zero/` directory section is extended with the new paths (component files, partials, components.d.ts).

31. `zero-framework-spec.md` §11 (Complete API Surface) gains a new sub-section listing `"zero/components"` and its 14 exports.

32. `zero-framework-spec.md` §12 — Phase 9 checklist items are marked `[x]` after implementation lands. (Implementation, not the plan.)

33. `zero-framework-spec.md` §13 (Key Design Decisions Summary) gains one row:
    `| Component library | 14 components shipped under .zero/components/; CSS wrapped in @layer components | Real apps shouldn't rebuild the same primitives; @layer keeps user overrides predictable without prefixing |`

34. `zero-framework-spec.md` §7.1 (Design system) is extended with a short pointer to the `@layer components` mechanic and the component-CSS partial location, mirroring how the existing token / utility documentation is structured.

### Out-of-band integration

35. `zero update` requires no new code: extending `framework_manifest()` is sufficient. The new files appear as **Add** ops on existing projects on next `zero update`. The plan reviews `src/cmd/update.rs` and `tests/update.rs` for length-coupled assertions that need updating.

36. The editor `tsconfig.json` emitted by `zero init` adds `"zero/components": [".zero/components/index.ts"]` to its `paths` block. The plan confirms the exact `paths` entry and which `TPL_*_TSCONFIG` constant requires editing.

37. `src/scaffold/.gitignore` — no change. The existing entry that gitignores `.zero/` continues to cover the new components and SCSS files inside `.zero/`.

## Constraints

- **No new Rust dependencies.** Rides on the existing `grass` SCSS pipeline, the existing transpiler, and the existing scaffold + manifest plumbing.
- **No new npm dependencies.** Framework-wide constraint, restated.
- **No CSS-in-JS, no CSS modules, no scoped styles.** Framework-wide constraint, restated.
- **No web components.** Components are plain functions; the existing `define()` escape hatch from `"zero/wc"` is not used internally.
- **No `!important` in component CSS.** `@layer components` is the override mechanism.
- **No magic numbers in component CSS.** Values come from `var(--*)` tokens. Exceptions limited to properties with no design-system token (`opacity`, `transition-duration`, `z-index`, animation timings).
- **Stateful props are signals.** Read `.val`, write `.set()`. No callback-based controlled-input pattern; no internal component state for `checked` / `value` / `open` / `active`.
- **No new top-level zero runtime exports.** Components live behind `"zero/components"` only.
- **Components are framework-owned.** Users override via the cascade (`@layer` makes this safe), via token re-declarations in `app.scss`, or by forking a component into `src/components/`. `zero update` rewrites `.zero/components/` and never touches user files.
- **Naming consistency.** Class names follow the design-system flat-dash convention. No BEM, no framework prefix.
- **Showcase is internal dev tooling.** Lives in this repo; not shipped to user projects; not bundled into release artifacts. Its `.zero/` is committed (in contrast to user projects).
- **Tests ship to user projects.** Framework component tests live in `.zero/components/` and run with the user's `zero test`. Accepted noise, see Requirement 29.

## Out of Scope

- **Theming customization API.** Users override `var(--*)` tokens in their own SCSS or via inline styles. No `setTheme()` helper, no theme-picker component, no localStorage persistence in the framework itself (the showcase's toggle is internal).
- **Form validation library.** Inputs accept signals; validation is the parent's job. No `Input({ validate })`, no error-state plumbing in v1.
- **General animation / transition utilities.** Spinner is the only animated component; its animation is a `@keyframes` rule inside `_spinner.scss`. No reusable animation primitives ship in this phase.
- **Date picker, time picker, color picker, autocomplete, combobox, menu, tooltip, popover, drawer, accordion, breadcrumbs, pagination, table.** All deferred.
- **Headless / unstyled variants of components.** Each component ships one styled form. Users needing different looks fork into `src/components/`.
- **Server-side rendering.** Out of scope framework-wide.
- **Full WCAG audit.** Components include reasonable ARIA defaults (`Dialog` uses `role="dialog" aria-modal="true"`; `Toggle` uses `role="switch"`; etc.) and basic focus handling for Dialog. A comprehensive WCAG audit is future work.
- **RTL polish beyond logical properties.** Components use `padding-inline`, `text-align: start`, etc. where appropriate. No explicit RTL test pass.
- **Lazy-loading individual components.** All 14 are imported eagerly when a user imports anything from `"zero/components"`. The bundler may tree-shake unused exports; lazy import is not a v1 contract.
- **Snapshot tests.** `expect().toMatchSnapshot()` is not yet implemented in `zero/test`; component tests assert on DOM selectors and signal values instead.
- **A standalone component-library package.** No npm publication, no `zero/components` distribution outside the binary.
- **Removing or renaming any existing design-system identifier.** Phases 7 and 8 are stable.

## Open Questions

- **Exact prop signatures.** The plan finalizes per-component TypeScript types: optional vs required props, defaults for variants/sizes, naming of event-callback props, the exact shape of `tabs` / `panels` for `Tabs`, the exact shape of `options` for `Select`.
- **Per-component SCSS rule budget.** Should the spec mandate "≤30 lines per component partial" as a complexity ceiling? The plan judges from the actual rule counts; a hard cap may invite token gymnastics.
- **`Dialog` modal: backdrop and focus trap.** Backdrop is straightforward. Focus trap (capture focus on open, restore on close, trap tab cycling) is involved. The plan decides whether v1 implements the trap or defers it to a future a11y-polish issue.
- **`Toast` stacking.** Single-toast UI is easy; queueing multiple toasts is more involved. v1 spec assumes single-toast; the plan confirms or escalates.
- **`Tabs` keyboard nav.** Arrow-key cycling is expected. The plan picks the exact key set and adds it to the `Tabs` test.
- **`Spinner` animation.** SVG-based or CSS-only? Both work. The plan picks.
- **Showcase routing — one route per component, or sections on a single long page?** Spec proposes one route per component plus a `/` overview. The plan confirms or simplifies.
- **Showcase theme toggle implementation.** Spec proposes a top-level `effect` writing to `document.documentElement.dataset.theme`. The plan confirms whether this lives in `app.ts` (mount-scoped global) or in `routes/home.ts` (component-scoped). Either works; the global option has the toggle persist across navigation.
- **`"zero/components"` resolver shape — single `index.ts` or per-component subpaths?** Spec proposes a single index re-export (one resolver entry). Alternative: per-component subpaths like `"zero/components/Button"` (14 resolver entries). Single index is simpler; the plan confirms.
- **Where component prop types are declared.** Spec proposes `.zero/components.d.ts` declaring the `"zero/components"` module. Alternative: rely on the `.ts` source's exported types being picked up by editors via tsconfig `paths`. The plan picks based on whether editors actually resolve `paths` aliases to TS sources inside `.zero/`.
- **Manifest count assertion.** Spec proposes 53. The plan confirms after enumerating the exact file list.
- **Component tests in user projects: ship as-is, or gate behind a flag?** Spec says ship as-is (accepted in refine). The plan can revisit if user-side test output noise turns out to be load-bearing.
- **Showcase `dist/`: gitignored.** Spec says yes. Plan confirms.
- **`Avatar` content when neither `src` nor `initials` is provided.** Fallback to a generic SVG silhouette, an empty colored circle, or a thrown error? The plan picks.
- **`Input` `type` attribute coverage.** Spec proposes `"text" | "email" | "password" | "number"`. Should `"search"`, `"url"`, `"tel"` be included? The plan picks; the styling is identical, so it's purely a TypeScript union question.
- **`Select` styling and the native `<select>` arrow.** Browsers vary in styling latitude for `<select>`. Plan decides whether to leave the native arrow or to mask it and draw a CSS arrow.
- **`Tabs` panel layout.** Should panels be in the DOM unconditionally (hidden via CSS) or conditionally rendered via a reactive block? Plan picks; conditional rendering matches zero's reactivity grain better.
- **Whether `framework_manifest()`'s assertion test should switch from a hard-coded length to a `let expected = …; assert!(manifest.len() == expected)` pattern.** With 50+ entries the maintenance burden grows. The plan judges.
