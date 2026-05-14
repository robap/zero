# Zero Framework

A zero-dependency frontend framework with a Rust CLI for scaffolding, development, and production builds.

## Prerequisites

- Rust toolchain (`cargo`, `rustc`) — [install via rustup](https://rustup.rs)
- Node.js — only needed to run the runtime's own test suite (`node --test`); not required for building or running apps

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

The framework's own JavaScript runtime tests use Node's built-in test runner:

```bash
node --test runtime/*.test.js
```

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
