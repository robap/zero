---
title: Why zero
nav_order: 17
---

# Why zero

The README's comparison table summarises *what's different*; this
page is the longer story for each row. Every framework is a set
of choices, and every choice trades one set of problems for
another. Read this, and you should be able to tell whether the
trades zero makes are the trades you want.

## No virtual DOM

**The decision.** zero binds reactive primitives directly to the
DOM nodes they affect. There is no parallel tree, no diff, no
reconciliation pass. A signal change patches exactly the text
node, attribute, or block bound to it.

**Win.** A smaller runtime (~4 KB), and updates that don't pay
for unrelated parts of the tree. Component functions run once,
not on every render.

**Tradeoff.** The granular update model is unusual. Some
ecosystems assume a vDOM — React-shaped UI libraries don't drop
in. Mental models from prior frameworks need a moment to
recalibrate (see the React-translation table in
[Reactivity](./reactivity.html#if-youre-coming-from-react)).

## No `node_modules`

**The decision.** zero is one Rust binary. The dev server,
transpiler, bundler, test runner, formatter, linter, and
component library are all inside it. There is no `package.json`,
no `npm install`, no lockfile.

**Win.** A single binary install. No supply-chain surface to
audit, no `node_modules` GC, no dependency drift, no "you have
the wrong Node version" support tickets. CI is `cargo install
zero` and run.

**Tradeoff.** You give up the npm ecosystem. If you need
something the framework doesn't ship — a charting library, a
date picker, a Markdown renderer — you either rebuild it
yourself or interop via web components (`zero/wc` is the
reserved slot for that future). For most line-of-business apps,
the framework's built-in surface plus a small amount of bespoke
code is plenty. For shops that lean heavily on npm packages,
this is a real cost.

## Signals over hooks

**The decision.** Reactive state is `signal` (cell),
`computed` (derived), `effect` (side-effect). No render
function, no deps array, automatic dependency tracking.

**Win.** Granular updates without re-renders. No deps-array
bookkeeping. No stale-closure footguns. Reactivity composes the
same way at every layer — a route, a component, a module-level
store.

**Tradeoff.** Different mental model. Developers fluent in
hooks have to relearn one piece: `useState` updates re-run the
component body, `signal.set` updates the bound text node. The
chapter on [Reactivity](./reactivity.html) bridges the gap, but
the relearning is real.

## Plain functions, no classes

**The decision.** Components are `() => TemplateResult`
functions. There is no `class Component`, no `this`, no
lifecycle methods. State lives in signals; teardown is
automatic via ownership scopes.

**Win.** Testable in isolation. No `this` binding, no
`super.componentWillUnmount`. Composing components is calling a
function. The `C01` lint rule enforces it across an app.

**Tradeoff.** Developers from React class components or other
class-based frameworks lose the legacy mental model. (In
practice this hasn't been a blocker — most current React code
is functional anyway.)

## No file-system routing

**The decision.** Routes are registered explicitly:
`app.route("/users/:id", UserPage)`. There is no `pages/`
directory convention, no file-name-driven route table.

**Win.** Explicit. Debuggable. No build-time magic. A reader
who's never seen a zero app before can read `src/app.ts` and
know every route the app responds to.

**Tradeoff.** A `routes/` table is more typing than Next-style
auto-discovery. For very large apps with hundreds of routes,
you'd want to split the table into modules and `app.route(...)
.route(...)` per module — but that's also explicit, and you can
see where every route comes from.

## Single binary for everything

**The decision.** The CLI is the framework. Every operation —
dev server, build, test, format, lint, scaffold, mutate — is a
subcommand of one binary.

**Win.** Zero toolchain debugging. There is no
"package.json/dev/build/test scripts" hierarchy to learn. Behaviour
is consistent across every project: `zero <verb>` does the
expected thing. CI is one binary install.

**Tradeoff.** Extending the build is harder than the npm
equivalent. There is no PostCSS plugin slot, no webpack loader
slot, no custom test-reporter API. If a project needs a
non-trivial customisation, the customisation is a Rust pull
request to the framework rather than a config change.

---

→ See also: [Examples Tour](./examples-tour.html) — the three
shipped example apps and what each demonstrates.

→ See also: [Best Practices](./best-practices.html) — how to
organise a real app once you've internalised these choices.
