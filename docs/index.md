---
title: zero
nav_order: 1
---

<!--
GitHub Pages setup (one-time, manual via repo Settings):
  Settings → Pages → Source: Deploy from a branch
  Branch: main, Folder: /docs
  Save. First build takes ~1 minute.

The aux_links and any absolute Pages URLs in this site use the
literal robap/zero placeholder; replace with the actual
GitHub robap/zero before publishing.
-->

# zero

A zero-dependency frontend framework with a Rust CLI. One binary,
no `node_modules`, signals instead of hooks, no virtual DOM.

If you're new to zero, start with [the README on
GitHub](https://github.com/robap/zero) for the pitch and the
install command; this site is the user guide for the framework
once you've got it running.

## Start here

- **[Getting Started](./getting-started.html)** — install through
  first edit. The 5-minute on-ramp.
- **[Reactivity](./reactivity.html)** — `signal`, `computed`,
  `effect`. The one new concept the framework asks you to learn.

## Reference

- **[Templates](./templates.html)** — the `html` tagged template,
  substitution values, attribute and event binding, modifiers,
  `each`, `ref`.
- **[Components](./components.html)** — props, children,
  composition, and the sixteen shipped components.
- **[Routing](./routing.html)** — lazy routes, params, guards,
  `load()`, nested routes, navigation, route-scoped fetch.
- **[HTTP](./http.html)** — `createHttp()`, methods, middleware,
  `HttpError`, fetch threading.
- **[Testing](./testing.html)** — the `zero test` runner, DOM
  helpers, in-memory DOM, web platform surface, spies.
- **[Theming](./theming.html)** — design tokens, layout
  primitives, utilities, light/dark, brand themes, typography.
- **[Building and Deploying](./building-and-deploying.html)** —
  `zero build`, manifest shape, backend integration, static
  deploys, `zero preview`.
- **[Config and CLI](./config-and-cli.html)** — full `zero.toml`
  schema and the per-subcommand reference.
- **[Linting](./linting.html)** — every rule `zero lint`
  enforces, and why.
- **[API](./api.html)** — flat search-in-page reference of every
  export across `zero`, `zero/test`, `zero/http`, and
  `zero/components`.

## After your first app

- **[Best Practices](./best-practices.html)** — how to organise
  state, routes, HTTP, components, styles, and tests in a real
  app.
- **[Examples Tour](./examples-tour.html)** — guided walk
  through `examples/counter`, `examples/todos`, and
  `examples/tracker`.
- **[Why zero](./why-zero.html)** — the design choices behind
  the framework and the tradeoffs each one makes.
