# zero

A zero-dependency frontend framework. Single Rust binary; no
`node_modules`; signals instead of hooks.

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

A Rust toolchain (`cargo`) is the only prerequisite — install
via [rustup.rs](https://rustup.rs).

## Get started

```sh
zero init
zero dev
```

Open `http://localhost:3000`.

→ Full documentation: <https://robap.github.io/zero/>

---

## Quickstart

```sh
mkdir my-app && cd my-app
zero init
```

`zero init` runs an interactive wizard (app folder, dev port,
optional backend proxy, build output folder), then scaffolds a
working app under `./web/`:

```
my-app/
├── zero.toml
├── tsconfig.json
└── web/
    ├── index.html
    ├── src/
    │   ├── app.ts
    │   └── routes/
    │       └── home.ts
    ├── styles/
    │   └── app.scss
    └── .zero/        # auto-managed by `zero update`
```

Start the dev server:

```sh
zero dev
# Listening on http://127.0.0.1:3000
```

Open the URL. The home route renders "Hello from zero" with a
working counter. Edits to `web/src/**` trigger a full reload —
the dev server transpiles `.ts` on the fly, no install step
beyond Cargo.

A minimal component:

```ts
import { html, signal } from "zero";
import type { TemplateResult } from "zero";

export default function Home(): TemplateResult {
  const name = signal("world");
  return html`
    <main class="stack pad-xl align-center">
      <h1>Hello ${name}</h1>
      <input value=${name} @input=${(e: Event) =>
        name.set((e.target as HTMLInputElement).value)} />
    </main>
  `;
}
```

When the input changes, the `<h1>` text patches in place —
no re-render, no diff. That's the whole reactive model.

Build for production:

```sh
zero build         # outputs dist/ (manifest.json, assets/, index.html)
zero preview       # serve dist/ locally
```

Going deeper:
[Best Practices](https://robap.github.io/zero/best-practices.html)
— application patterns for real apps.

---

## How zero compares

|                   | zero       | React      | Vue        | Solid      | Svelte     |
|-------------------|------------|------------|------------|------------|------------|
| Build tool        | built-in   | Vite (etc) | Vite (etc) | Vite (etc) | Vite       |
| npm dependencies  | 0          | many       | many       | many       | many       |
| State model       | signals    | hooks      | refs       | signals    | runes      |
| Virtual DOM       | no         | yes        | yes        | no         | no         |
| Component model   | functions  | functions  | functions  | functions  | files      |
| Bundle posture    | ~4 KB rt   | ~40 KB     | ~30 KB     | ~7 KB      | compiled   |

Numbers and claims are coarse and age fast; the table is for
orienting an evaluator, not for benchmarking. Pick the framework
that matches your problem, not the one that wins the table.

---

## License

[MIT](LICENSE)
