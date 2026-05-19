# Zero Framework

A zero-dependency frontend framework with a Rust CLI for scaffolding, development, and production builds.

## Prerequisites

- Rust toolchain (`cargo`, `rustc`) — [install via rustup](https://rustup.rs)

## Build the CLI

```bash
cargo build --release
# Binary is at ./target/release/zero
```

To use it globally, copy or symlink it somewhere on your PATH:

```bash
cp target/release/zero ~/.local/bin/zero
```

## Quick start

```bash
# 1. Create a project directory and enter it
mkdir my-app && cd my-app

# 2. Scaffold a zero app (interactive wizard, or pre-write zero.toml to skip prompts)
zero init

# 3. Start the dev server
zero dev

# 4. Open http://localhost:3000 in a browser
```

## Commands

### `zero init`

Scaffolds a new zero app into the current directory.

- **If `zero.toml` is absent**: runs an interactive wizard to collect four settings (app folder name, dev port, optional backend proxy URL, build output folder), writes `zero.toml`, then creates the scaffold.
- **If `zero.toml` is present**: reads it and scaffolds into `./<root>/` without prompting. Useful for scripted or CI environments — write the toml first, then run `zero init`.

The scaffold creates:

```
<root>/
├── index.html               # entry HTML; script tags are injected by zero dev/build
├── src/
│   ├── app.js               # app entry point
│   └── routes/
│       └── home.js          # home route component
└── styles/
    └── app.css
```

`zero init` refuses to overwrite a non-empty `<root>/` directory.

### `zero dev`

Starts the development server. Reads `zero.toml` from the current directory (exits with a clear error if absent).

```
zero dev
# Listening on http://127.0.0.1:3000
```

The server handles:

| Path | Behaviour |
|------|-----------|
| `GET /zero.js` | Returns the embedded runtime as an ES module |
| `GET /src/**` | Serves files from `./<root>/src/` |
| `GET /styles/**` | Serves files from `./<root>/styles/` |
| `GET /public/**` | Serves files from `./<root>/public/` |
| `/favicon.ico`, `/robots.txt` | Served from `./<root>/` if present |
| Everything else (no proxy) | Returns `./<root>/index.html` with scripts injected — SPA fallback |
| Everything else (proxy set) | Forwards to the configured backend; injects scripts into HTML responses |

Every response includes `Cache-Control: no-store` and related no-cache headers so browser refreshes always fetch the latest.

**With a backend proxy** — set `[dev] proxy` in `zero.toml`:

```toml
[dev]
proxy = "http://localhost:8080"
```

`zero dev` becomes a single origin: the browser talks only to `http://localhost:3000`, which proxies API and page requests to your backend. No CORS configuration needed.

**Graceful shutdown**: Ctrl-C.

### `zero build`

Produces a deployable bundle in `./<out>/` (default `dist/`). Reads `zero.toml` from the current directory.

```
zero build
```

Output:

```
dist/
├── assets/
│   ├── app.<hash>.js        # bundled runtime + user code
│   └── app.<hash>.css       # hashed copy of styles/app.css
├── manifest.json            # logical name → hashed filename
└── index.html               # static index with script/link tags pre-injected
```

**Using the build output in your backend**:

1. Serve `dist/assets/` as static files.
2. Read `dist/manifest.json` to get the hashed filenames:
   ```json
   {
     "app.js": "assets/app.a3f2b1c4.js",
     "styles/app.css": "assets/app.5e8d9f01.css"
   }
   ```
3. Inject `<script type="module" src="...">` and `<link rel="stylesheet" href="...">` into your server-rendered HTML.

For static deploys (no backend), `dist/index.html` is ready to upload directly to any static host.

## Writing a zero app

The framework is exposed as a single ES module imported from the bare specifier `"zero"` (resolved by `zero dev` and `zero build`). The full surface is documented in `zero-framework-spec.md`; this section is a working primer.

### App entry (`src/app.js`)

The app object owns the route table, app-level state, and middleware. Build it, then call `.run()` to mount.

```js
import { App, signal } from "zero";
import Home from "./routes/home.js";

const app = new App();
app.state("count", signal(0));   // app-level state — read anywhere via inject()
app.route("/", Home);            // first match wins; register more routes here
app.run("#app");                 // mount into <div id="app"></div>
```

`new App()` is chainable: `.state()`, `.route()`, `.layout()`, `.use()`, `.loading()`, `.error()` all return the app. Call them all before `.run()` — they throw afterward.

### Components

Components are plain functions that return a `TemplateResult` from the `html` tag. They run **once** when committed; the framework wires up granular reactive updates for any signals or `() => ...` blocks inside.

```js
import { html, signal } from "zero";

export default function Counter() {
  const count = signal(0);
  return html`
    <p>Count: ${count}</p>
    <button @click=${() => count.update(n => n + 1)}>+</button>
  `;
}
```

Props are a plain object passed when invoking the component as a function:

```js
function Greeting(props) {
  return html`<h1>Hello ${props.name}</h1>`;
}

// usage inside another template:
html`<div>${Greeting({ name: "Ada" })}</div>`
```

If a prop is a signal, reactivity flows through it — the parent doesn't re-render, only the bound text/attribute node updates.

### Signals

`signal(initial)` returns `{ val, set(v), update(fn) }`.

- `count.val` — read (inside an `html` template or `effect`, this auto-subscribes)
- `count.set(5)` — replace
- `count.update(n => n + 1)` — functional update

`computed(fn)` derives a read-only signal that re-evaluates lazily when its dependencies change. `effect(fn)` runs `fn` immediately and re-runs it whenever any signal it reads changes; the return value is a `stop()` function.

### Templates (`html`)

The `html` tag parses once per call-site (cached) and clones a `DocumentFragment` each render. Inside `${...}` you can place:

| Value | Behaviour |
|-------|-----------|
| `string` / `number` | rendered as text |
| `boolean` | for attributes — `false`/`null`/`undefined` removes the attribute, `true` sets it to `""` |
| `Signal` | auto-subscribes; updates the target text node or attribute in place |
| `() => value` | reactive block — re-evaluates when its dependencies change |
| `TemplateResult` | nested template |
| array of the above | inserts each item in order |

Attribute and event bindings:

```js
html`<input value=${name} @input=${e => name.set(e.target.value)} />`
html`<button @click.prevent.stop=${submit}>Go</button>`     // event modifiers
html`<input @keydown.enter=${submit} />`                    // key filters
html`<input ref=${inputRef} />`                             // DOM ref (see below)
```

Event modifiers: `.prevent`, `.stop`, `.once`, `.throttle` (100ms), `.debounce` (100ms), and key filters (`.enter`, `.escape`, `.space`, `.tab`, `.up`, `.down`, `.left`, `.right`).

### Lists with `each`

For keyed list rendering with per-item scopes:

```js
import { html, signal, each } from "zero";

const todos = signal([{ id: 1, text: "Learn zero" }]);

html`
  <ul>
    ${each(todos, todo => html`<li>${todo.text}</li>`)}
  </ul>
`;
```

Removing an item disposes only that item's effects; reordering moves DOM nodes rather than re-creating them.

### Refs

`ref()` returns `{ el: null }`. Pass it to a `ref=${...}` binding; after commit, `.el` points at the DOM node.

```js
import { html, ref, effect } from "zero";

function AutoFocus() {
  const input = ref();
  effect(() => input.el?.focus());
  return html`<input ref=${input} />`;
}
```

### App-level state via `inject`

Register state on the app, read it from any component — no prop drilling.

```js
// app.js
app.state("count", signal(0));
```

```js
// any component
import { inject } from "zero";

function Counter() {
  const count = inject("count");
  return html`<p>${count}</p>`;
}
```

`inject(key)` throws if no app is running or the key isn't registered. Use this for genuinely app-wide state (auth, theme, current user); prefer props for state that only belongs to one branch of the tree.

### Routing

`app.route(pattern, componentOrLoader, opts?)` registers a route. Patterns support `:param` segments and a bare `*` wildcard. First match wins.

```js
app.route("/", Home);
app.route("/users/:id", UserPage);
app.route("/admin", AdminPage, { guard: ({ redirect }) => { if (!loggedIn()) redirect("/login"); } });
app.route("/blog/:slug", () => import("./routes/post.js"));   // lazy — code-split
app.route("*", NotFound);
```

The route component receives `{ params, query, state, route }`. Use plain `<a href="/path">` for navigation — the framework intercepts same-origin clicks.

## Configuration (`zero.toml`)

Place this file at the project root (the directory you run `zero` commands from):

```toml
[project]
root = "web"                         # required; zero app lives in ./web/

[dev]
port = 3000                          # default 3000
proxy = "http://localhost:8080"      # optional; omit for static SPA mode

[build]
out = "dist"                         # default "dist"
```

Validation rules:
- `root` and `out` must be relative paths with no `..` or absolute components.
- `port` must be 1–65535.
- `proxy`, if set, must use `http://` (not `https://`).
- Unknown keys are rejected (typo protection).

## Running the runtime tests

The framework's own JavaScript runtime tests run under `zero test`, the same runner user apps use. From the repo root:

```bash
cargo run -p zero -- test
```

Once the CLI is installed (`cargo install --path crates/zero --locked`), the same suite runs as `zero test`.

## Repository layout

```
build.rs                 # compile-time runtime concatenation
runtime/                 # JavaScript runtime (reactivity, template, router, app)
src/
├── main.rs              # CLI entry point
├── lib.rs               # public modules (for integration tests)
├── config.rs            # zero.toml parsing and validation
├── runtime.rs           # embedded ZERO_RUNTIME_BODY constant
├── scaffold.rs          # embedded scaffold templates
├── prompts.rs           # zero init wizard
├── toml_writer.rs       # render zero.toml from wizard answers
├── cmd/                 # subcommand implementations
│   ├── init.rs
│   ├── dev.rs
│   └── build.rs
├── dev/                 # dev server modules
│   ├── server.rs
│   ├── files.rs
│   ├── inject.rs
│   ├── local.rs
│   ├── proxy.rs
│   └── headers.rs
└── build/               # bundler modules
    ├── bundler.rs
    ├── resolver.rs
    ├── css.rs
    ├── manifest.rs
    └── index_html.rs
tests/                   # integration tests
```

## Development workflow

```bash
# Run all Rust tests
cargo test

# Check for lint issues
cargo clippy --all-targets

# Format code
cargo fmt
```
