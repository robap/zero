# Spec: CLI Bootstrap — `zero init`, `zero dev`, `zero build`

## Problem Statement

Phases 1–3 built a working runtime (reactivity, template system, app +
router) — but it lives only in `runtime/*.js` and is exercised only by
`node --test`. There is no way for a developer to start a new project,
run it in a browser, or produce a deployable artifact. The framework
is unusable end-to-end.

This spec ships the smallest CLI slice that proves the full developer
flow:

1. **Scaffold** a zero app into an existing project (`zero init`).
2. **Develop** it in a browser, integrated with whatever backend the
   developer already has (`zero dev`).
3. **Build** a deployable artifact the developer's existing backend
   can drop into its own HTML (`zero build`).

The DX bar for this slice is specifically: **a developer with an
existing project (some backend, possibly other tooling) can `cd` to
the project root, run `zero init` to add a zero app into a subdirectory
without touching anything else, run `zero dev` against their existing
backend (Express, Rails, Go, PHP, whatever), iterate on the UI, then
run `zero build` and have the backend serve the bundled JS from its
own templated HTML — all commands run from the project root, no CORS,
no language-specific glue, no Node bundler in the backend's build
pipeline.**

The test runner (Phase 5 in the framework spec) is **deferred**.
`node:test` and `bun:test` are adequate for both the framework's own
runtime tests and downstream apps in the meantime; the test runner's
API surface should be shaped by real apps, which require this slice to
exist first.

## Background

### What exists

- `runtime/reactivity.js`, `runtime/template.js`, `runtime/router.js`,
  `runtime/app.js`, `runtime/dom-shim.js` — the full Phase 1–3 runtime.
  All ESM, all browser-targetable. Reads `globalThis.document` /
  `globalThis.window`, which the browser provides natively.
- `Cargo.toml` and `src/main.rs` — a hello-world Rust binary. No
  dependencies declared, no real CLI structure.
- `runtime/dom-shim.js` is the test-only shim. **It is not used in
  production** — the runtime in a browser uses real DOM. The shim
  must not be served by `zero dev`.

### The transforming-reverse-proxy dev model

`zero dev` is the **single origin** the browser talks to. There is
no CORS surface in dev because the browser never directly contacts
the developer's backend.

```
Browser ──► http://localhost:3000 (zero dev)
                │
                ├── /zero.js          → embedded runtime, served from memory
                ├── /src/**           → file from disk, served as ESM
                ├── /styles/**        → file from disk, served as CSS
                ├── /favicon.ico, etc → file from disk if present
                │
                └── anything else     → proxied to backend (e.g. localhost:8080)
                                        ── if backend response is text/html,
                                           inject <script type="importmap">
                                           and <script type="module">
                                           into the response before returning
```

When `[dev].proxy` is omitted from `zero.toml`, the no-proxy fallback
is to serve the project's local `index.html` from disk (still injecting
the script tags). This covers pure-SPA developers who have no backend.

The injection makes the developer's HTML — whether backend-rendered or
local — boot the framework without the developer having to know about
import maps or module URLs. The backend's HTML can be exactly the HTML
the backend will serve in production; only `zero dev` adds the dev-time
plumbing.

### The "drop into any backend" production model

`zero build` is shaped to make the backend's job in production trivial:

```
dist/
├── assets/
│   ├── app.<hash>.js         ← single ES module: runtime + user code, fully bundled
│   ├── app.<hash>.css        ← copied (and optionally hashed) from styles/
│   └── ... other assets if referenced
├── manifest.json             ← logical name → hashed filename mapping
└── index.html                ← static-deploy convenience: index.html with script tags pre-injected
```

The backend developer's job in production:

1. Read `dist/manifest.json` (a tiny JSON file: `{ "app.js": "assets/app.a3f2b1.js", "app.css": "assets/app.a3f2b1.css" }`).
2. Inject the right `<script type="module" src="...">` and
   `<link rel="stylesheet" href="...">` tags into their templated
   HTML using whatever templating they already use.
3. Serve `dist/assets/` as static files (or upload to a CDN).

For the pure-SPA case (no backend), `dist/index.html` is ready to deploy
to any static host (Netlify, Vercel, S3, GitHub Pages) as-is.

### Why a single bundle for build, ESM-per-file for dev

Different priorities:

- **Dev:** edit-reload latency matters. Each user file served as its
  own ES module means a future watcher can invalidate just the changed
  module rather than rebundling the whole graph. Native ESM is "free"
  — no bundler in the dev hot path.
- **Build:** bytes-over-the-wire and request count matter. A single
  bundle (one network request, one parse) is the right shape for
  production. Tree-shaking and minification land here too (deferred,
  but the bundler choice should support them).

This is the same split Vite makes; it's the right tradeoff.

### `zero.toml`

The framework spec §1 said "no config files (except `tsconfig.json`)."
This spec amends that: a single `zero.toml` at the **project root**
(the directory the developer runs `zero` commands from) is permitted
and is the source of truth for where the zero app lives.

The project root is typically a polyglot directory containing other
things — a backend, build tooling, README, etc. The zero app lives
**in a subdirectory** of the project root, named in `zero.toml`'s
`[project] root` key.

```toml
[project]
root = "web"                         # zero app lives in ./web/ (required)

[dev]
port = 3000                          # default 3000
proxy = "http://localhost:8080"      # optional; omit for static-SPA mode

[build]
out = "dist"                         # default "dist"; relative to project root
```

The `[project] root` key is the only required field. `[dev]` and
`[build]` sections are optional; all keys within them have defaults.
The framework spec text should be updated to match when next revised.

### Project layout

```
my-existing-project/      ← developer's project root; `zero` commands run from here
├── zero.toml             ← lives at the project root
├── backend/              ← whatever the developer had before — zero never touches this
│   └── ...
├── web/                  ← created by `zero init`; everything zero owns
│   ├── index.html
│   ├── src/
│   │   ├── app.js
│   │   └── routes/home.js
│   └── styles/
│       └── app.css
└── dist/                 ← `zero build` output (project root, not under web/)
```

The `web/` subdirectory name is whatever the developer chose during
`zero init`. `web/` is the only place zero scaffolds or modifies
files; `dist/` is the only place `zero build` writes. Everything else
at the project root is invisible to zero.

## Requirements

### Rust binary structure

- `src/main.rs` becomes a real CLI entry point with subcommand dispatch.
- Subcommands shipped in this slice: `init`, `dev`, `build`.
- Dispatch on the first positional argument; unknown subcommands print
  help and exit non-zero.
- Global flags: `-h`/`--help`, `--version`. (Verbose / quiet / no-color
  deferred.)
- An argument-parsing crate is allowed (clap is the obvious choice;
  the plan may pick another lightweight crate).
- All other crate choices (HTTP server, JS bundler, HTML transformer,
  interactive-prompt library) are plan-level decisions; this spec
  lists the constraints those crates must satisfy.

### Runtime embedding

- Each `runtime/*.js` file is embedded into the binary at compile time
  via `include_str!` (or a `build.rs` that emits constants).
- A const `ZERO_RUNTIME: &str` exists, holding the **concatenated**
  contents of the runtime files in dependency order
  (`reactivity.js` → `template.js` → `router.js` → `app.js`). Concatenation
  is done at build time, not at server startup.
- Concatenation strategy: each file's `import` / `export` statements
  are stripped and the file bodies are joined into a single ES module
  whose final `export` block re-exports the public surface
  (`signal`, `computed`, `effect`, `html`, `commit`, `each`, `ref`,
  `App`, `inject`, `navigate`, `back`, `forward`, `route`).
- The `dom-shim.js` file is **excluded** — it is test-only and the
  browser provides the real DOM.
- The plan picks the concrete strategy for stripping/rewriting imports
  (regex-substitute is acceptable given the runtime files use a small
  fixed set of import shapes; a real ESM rewriter is overkill for this
  slice).

### `zero init`

Takes no positional arguments. Operates on the CWD (the project root).

#### When `zero.toml` is absent at the CWD

`zero init` runs an **interactive prompt** to gather the settings,
writes them to `./zero.toml`, then scaffolds the app subdirectory
based on those answers.

Prompts (in order), with defaults shown in parentheses; pressing
Enter accepts the default:

1. **Zero app folder name** (`web`) — written to `[project] root`.
2. **Dev server port** (`3000`) — written to `[dev] port`.
3. **Backend proxy URL** (none) — if non-empty, written to `[dev] proxy`;
   if empty, omitted from the toml entirely (no-proxy mode).
4. **Build output folder** (`dist`) — written to `[build] out`.

After the prompts:

- Write `./zero.toml` with the answers. Settings equal to their defaults
  may be included as commented-out hints rather than active keys (so
  the toml stays minimal but the developer sees what's tunable). The
  exact formatting is a plan decision; the spec only requires that
  the resulting file parses back to the same effective settings.
- Refuse and exit if `./zero.toml` already exists (we shouldn't reach
  this branch — the wizard only runs when it's absent).
- Then proceed to the scaffold step below.

Non-interactive use (CI, scripts) is **out of scope** for this slice
— developers who want non-interactive setup can hand-write
`zero.toml` first, then run `zero init` (which takes the second
branch).

#### When `zero.toml` is present at the CWD

`zero init` reads it, validates the `[project] root` key is set,
then scaffolds into `./<root>/` using the toml's settings. **No
prompts.** The toml is authoritative.

#### Scaffold step (both branches)

After settings are known (either from prompts or from existing toml):

- Refuse if `./<root>/` exists and is non-empty. Exit non-zero with a
  clear message ("zero init: `./<root>/` is not empty; refusing to
  overwrite").
- Create `./<root>/` if missing.
- Write the scaffold files into `./<root>/`:

```
<root>/
├── index.html               # entry HTML, no script tags (zero injects)
├── src/
│   ├── app.js               # configured App with one route
│   └── routes/
│       └── home.js          # home route component
└── styles/
    └── app.css              # one stylesheet, linked from index.html
```

- Scaffold templates are embedded in the binary as `&'static str`
  constants. Substitution placeholders (e.g., `{{title}}` for the
  HTML `<title>`) are plain string replacement — no template engine.
  The `<title>` defaults to the project root directory's basename.
- Scaffold does NOT include: `tsconfig.json` (no TS in this slice),
  `src/components/` (empty dirs are noise), `styles/vars.css` (the
  user adds when they need it), `.gitignore` (deferred — easy to add
  later). `zero.toml` itself is **not** scaffolded into `./<root>/`
  — it lives at the project root, written by the prompt branch above.
- `index.html` is **headless of script tags** — it has the
  `<div id="app"></div>` and the stylesheet `<link>` but no
  `<script>` and no import map. Both `zero dev` (in-memory) and
  `zero build` (on disk in `dist/index.html`) inject the appropriate
  script tags. This avoids the developer needing to maintain a
  dev-vs-prod script src by hand.

#### Safety properties

- Never write outside `./<root>/` and `./zero.toml`. Other files at
  the project root are untouched.
- Never overwrite an existing `zero.toml` or any file already in
  `./<root>/`.
- All file operations are local — no network, no global state.

### `zero.toml` parsing

- Look for `zero.toml` at the project root (the directory `zero` is
  invoked from). For `zero dev` and `zero build`, the file must exist
  — these commands have no useful behavior without a `[project] root`
  pointing at the app subdirectory. Exit non-zero with a clear error
  ("zero.toml not found at <cwd>; run `zero init` to create one").
- For `zero init`, behavior is the two-branch flow described above.
- Validate the parsed config:
  - `[project] root` must be a non-empty string and must not contain
    path separators that escape the project root (no `..`, no absolute
    paths). Reject with a clear error otherwise.
  - `[dev] port` must be in 1–65535. Reject otherwise.
  - `[dev] proxy`, if present, must be a valid `http://` URL. Reject
    `https://` with a clear "HTTPS dev proxy is out of scope" message.
  - `[build] out` must be a non-empty string, same path-escape rules
    as `[project] root`.
- Reject unknown top-level sections / unknown keys with a clear error
  (typo protection).
- TOML parser: a Rust crate (e.g., `toml`).

### `zero dev`

#### Server lifecycle

- Reads `zero.toml` once at startup.
- Binds `127.0.0.1:<port>` (default 3000). On bind failure (e.g. port
  in use), print a clear error and exit non-zero.
- Logs one line per request at default verbosity (method, path,
  status, ms).
- Ctrl-C exits cleanly.

#### No caching, ever

Every response from `zero dev` MUST include cache-defeating headers:

```
Cache-Control: no-store, no-cache, must-revalidate, max-age=0
Pragma: no-cache
Expires: 0
```

This applies to:

- `/zero.js` (embedded runtime)
- All disk-served files (`/src/**`, `/styles/**`, `/public/**`, well-known root files)
- The transformed local `index.html` (no-proxy mode)
- Proxied responses — **strip any `Cache-Control`, `Pragma`, `Expires`,
  `ETag`, and `Last-Modified` headers from the backend response, then
  add the no-cache headers above**. The dev workflow ("edit a file,
  hit refresh, see the change") is hostile to any caching anywhere in
  the chain.

The single-binary, no-build-step nature of dev means there's no
content-hashing to make caching safe. `Cache-Control: no-store`
guarantees the browser doesn't even keep a cached copy, eliminating
"why isn't my change showing up" debugging sessions caused by stale
disk caches in Chrome / Firefox.

#### Request routing

The server reads `zero.toml` once at startup; call the app
subdirectory `<root>` (e.g., `./web/`). For each incoming request,
the server decides one of three actions based on the URL path:

1. **Internal asset paths** (highest priority):
   - `GET /zero.js` → respond with the embedded `ZERO_RUNTIME`
     concatenated bundle. `Content-Type: application/javascript`.
     Cache headers per **No caching, ever** above.
2. **Project file paths** (read from disk under `./<root>/`):
   - `GET /src/**` → file at `./<root>/src/**`.
   - `GET /styles/**` → file at `./<root>/styles/**`.
   - `GET /public/**` → file at `./<root>/public/**` (if the
     developer creates one; the scaffold doesn't include this dir).
   - `GET /<wellknown>` for small set of root files (`/favicon.ico`,
     `/robots.txt`) → file at `./<root>/<wellknown>` if it exists.
   - `Content-Type` derived from extension (`.js`, `.css`, `.html`,
     `.svg`, `.png`, `.jpg`, `.json`, `.ico` cover the slice;
     unknown → `application/octet-stream`).
   - Path traversal protection: paths must canonicalize within
     `./<root>/`; otherwise respond `403`.
3. **Everything else** → either proxy or serve the local index:
   - If `[dev].proxy` is set in `zero.toml`: forward the request to
     the proxy target (preserving method, headers, body, query
     string).
   - If unset: read `./<root>/index.html` from disk and respond
     with it.
   - In **either** case, if the response body is `text/html`, run the
     **HTML injection** step (below) on the response body before
     returning it to the client.

#### HTML injection

The injection inserts two script tags into the `<head>` of any
HTML response:

```html
<script type="importmap">{"imports":{"zero":"/zero.js"}}</script>
<script type="module" src="/src/app.js"></script>
```

- Insertion strategy: case-insensitive find of `</head>` and insert
  before it. If `</head>` is absent (malformed HTML or fragment),
  fall back to inserting before `<body` or at the start of the body
  bytes; if neither marker is found, prepend the injected snippet
  to the response body and continue (best-effort — log a warning).
- Content-Length / encoding:
  - If the proxied response was `Transfer-Encoding: chunked`,
    re-emit chunked.
  - If `Content-Length` was set, recompute it after injection.
  - Decompress / re-compress on `Content-Encoding: gzip` or `br`
    is **out of scope** — instead, send `Accept-Encoding: identity`
    on outgoing proxy requests so the backend returns uncompressed
    HTML.
- The entry-script path (`/src/app.js`) is hard-coded in this slice;
  no override knob.

#### Proxy behavior

When `[dev].proxy` is set and the request falls through to the proxy:

- HTTP only (no HTTPS in this slice; reject https:// proxy targets at
  startup with a clear error).
- Forward request method, headers (excluding hop-by-hop headers per
  RFC 7230), body, and query string.
- Forward all status codes, including 3xx redirects (do not follow).
- WebSocket upgrades are **out of scope** — reject `Upgrade: websocket`
  requests with a clear error in the response body. (Live-reload over
  WS is a future spec.)
- On connection error to the backend (refused, timeout), respond `502`
  with a short HTML body that says "zero dev: cannot reach backend at
  &lt;url&gt;". This is the only "framework-renders-UI-you-didn't-write"
  case in this slice and is acceptable as a debugging aid.

### `zero build`

Reads `zero.toml`. Sources come from `./<root>/`; outputs go to
`./<out>/` (both relative to the project root, where `<root>` is
`[project] root` and `<out>` is `[build] out`).

1. **Bundle.** Produce a single ES module from `./<root>/src/app.js`
   plus all its transitive imports plus the `zero` runtime. Output
   filename: `app.<hash>.js` where `<hash>` is the first 8 hex chars
   of the sha256 of the bundle contents. Place in `./<out>/assets/`.
2. **Copy and hash CSS.** For each `.css` file under
   `./<root>/styles/`, compute a content hash and emit to
   `./<out>/assets/<name>.<hash>.css`.
3. **Emit manifest.** Write `./<out>/manifest.json`:
   ```json
   {
     "app.js": "assets/app.a3f2b1c4.js",
     "styles/app.css": "assets/app.5e8d9f01.css"
   }
   ```
   Keys are logical (source-relative, relative to `./<root>/`) names;
   values are output paths relative to `./<out>/`. The shape is
   intentionally simple — additional metadata (preloads, async
   chunks) is out of scope.
4. **Emit static `index.html`.** Read `./<root>/index.html`, inject
   `<script type="module" src="/<hashed-app.js>">` and
   `<link rel="stylesheet" href="/<hashed-app.css>">` into `<head>`,
   write to `./<out>/index.html`. **No import map** — the bundled file
   self-contains the runtime, so the bare `import "zero"` at module
   boundaries was already resolved by the bundler.

The bundler (Rust crate; `oxc`, `swc_bundler`, or similar — plan
chooses) must:

- Resolve ES module imports across the project and the embedded
  runtime.
- Treat the bundled-runtime symbol `"zero"` as resolvable to the
  embedded runtime source. (Either via a virtual filesystem entry
  pointing at the runtime concatenation, or by inlining the runtime
  before bundling.)
- Produce a single ES module output.
- Tree-shaking and minification are **nice-to-have** — if the
  chosen crate gives them for free, take them; otherwise defer.
- Sourcemaps are **out of scope** for this slice (deferred).

The build is **not** incremental — every `zero build` runs from
scratch. Incremental builds land in a later spec.

### File layout

```
src/
├── main.rs              # subcommand dispatch + arg parsing
├── cmd/
│   ├── init.rs          # zero init (prompts + scaffold)
│   ├── dev.rs           # zero dev (server, routing, injection, proxy)
│   └── build.rs         # zero build (bundle, hash, manifest, static index)
├── config.rs            # zero.toml parsing + validation
├── prompts.rs           # interactive prompt helpers (zero init wizard)
├── runtime.rs           # const ZERO_RUNTIME (or build.rs-generated)
└── scaffold/            # embedded scaffold templates as include_str!
    ├── index.html
    ├── src/app.js
    ├── src/routes/home.js
    └── styles/app.css

build.rs                 # concatenates runtime/*.js → ZERO_RUNTIME at compile time
Cargo.toml               # see "Crate choices" below
```

The exact split of code into modules is a plan-level decision; the
above is one reasonable shape. Note that `zero.toml` is NOT embedded
as a scaffold template — it's generated programmatically by the
prompt wizard with the user's answers.

#### Crate choices

The plan picks the final list, but the spec recommends:

- **CLI parsing:** `clap` (with `derive` feature). Mature, widely
  used, supports subcommands cleanly.
- **TOML parsing:** `toml`. The default; nothing else worth
  considering for this volume.
- **HTTP server + client + async runtime: `axum` + `tokio` +
  `hyper` + `reqwest` (or `hyper`'s client directly).** Rationale:
  - `axum` is built on `hyper`/`tokio`, ergonomic routing, easy to
    add WebSocket support later when HMR lands.
  - `reqwest` (which itself sits on `hyper`) handles the proxy
    HTTP-client side with streaming bodies and connection reuse.
    For tighter control over streaming pass-through (avoiding a
    full-body buffer for large proxied responses), the plan may
    drop to `hyper`'s client directly.
  - The `axum` + `tokio` + `reqwest` stack is the standard async
    HTTP shape in Rust circa 2026; abundant docs, well-tested.
  - Alternatives considered: `hyper` directly (more boilerplate,
    no real win); `actix-web` (heavier, separate runtime story);
    `tiny_http` + sync I/O (smaller but blocks the path to HMR
    and concurrent proxying). `axum` is the right default.
- **JS bundler:** see Open Questions — `oxc` if its bundler is
  ready by implementation time, else `swc_bundler`.
- **Interactive prompts:** `dialoguer` is the obvious default
  (lightweight, supports defaults and validation, mockable via a
  `Term` abstraction). Plan may pick `inquire` if richer prompt
  types are wanted later.
- **SHA-256 hashing (for content-hashed filenames):** `sha2`.
  Standard.

### Tests

- `cargo test` for Rust-side unit tests (config parsing + validation,
  scaffold rendering, HTML injection, path traversal protection,
  toml-writing from prompt answers).
- An end-to-end test that, in a temp dir, writes a minimal `zero.toml`
  (skipping the interactive prompts) and runs `zero init`; asserts
  the scaffold landed in `./<root>/`; runs `zero dev` against it (no
  proxy); curl `/`; asserts the HTML contains the injected script
  tags; asserts `/zero.js` returns the runtime; asserts
  `/src/app.js` returns the file at `./<root>/src/app.js`.
- An end-to-end test that runs `zero build`, asserts `./<out>/`
  contains the expected files, asserts the bundled JS evaluates
  in a Node child process without throwing.
- The interactive-prompt branch of `zero init` is tested by injecting
  a scripted stdin into the binary; the plan should pick a prompt
  crate that supports this (or factor the prompt-driver into a unit
  function that takes a `Read` and writes the toml string, which is
  testable without spawning the binary).
- Existing `node --test runtime/*.test.js` continues to pass
  (this slice does not change the runtime).

## Constraints

- **No npm dependencies** anywhere — neither in the framework's own
  source nor in scaffolded projects.
- **Rust crate dependencies are allowed** — they ship inside the
  binary, transparent to the user.
- **Single binary distribution** — runtime and scaffold templates
  embedded at compile time, not loaded from disk at runtime.
- **No CORS in dev** — single-origin model, all browser traffic to
  `zero dev`, which proxies to the backend.
- **No HMR / live-reload in this slice.** Manual browser refresh.
  The injection point reserves room for a future HMR client script.
- **No TypeScript** — `.js` files only; transpilation is a separate
  spec.
- **No HTTPS in dev** — the proxy target must be `http://`.
- **No WebSocket proxying** — reject `Upgrade: websocket` with a clear
  error.
- **Build is non-incremental** — full rebuild every time.
- **No sourcemaps** in build output.
- **Path traversal protection** — disk-served paths must canonicalize
  within `./<root>/` (the zero app subdir). The config-validation
  rules on `[project] root` and `[build] out` reject `..` and absolute
  paths so a misconfigured toml can't escape the project tree.
- **`zero init` never writes outside `./<root>/` and `./zero.toml`.**
  Other files at the project root stay untouched.
- **All commands run from the project root** (the directory containing
  `zero.toml`). The CLI does not search upward for `zero.toml` — if
  the developer `cd`s into a subdirectory, commands exit with
  "zero.toml not found at <cwd>". Upward search is a reasonable later
  enhancement; deferred.

## Out of Scope

- `zero test` — test runner deferred per the test-runner discussion.
- `zero check` — TypeScript type-checking; depends on TS support.
- `zero fmt` / `zero lint` — separate later specs.
- `zero gen` — code generation; later spec.
- `zero preview` — small wrapper around a static-file server for
  `dist/`; trivially added later, not in this slice.
- `zero upgrade` — self-update; later spec, depends on a release pipeline.
- TypeScript support (scaffolding `.ts`, transpilation in `dev`,
  `tsconfig.json` in scaffold).
- HMR / live-reload of any kind.
- HTTPS in dev (`--https`, self-signed certs).
- `--open`, `--host`, `--quiet`, `--verbose`, `--no-color` flags.
- Multi-target builds (`--target server`, `--target worker`).
- Code splitting (multiple bundles, dynamic-import chunks).
- Tree-shaking and minification as required features (allowed if
  free from the bundler).
- Sourcemaps in either dev or build.
- WebSocket proxying.
- Asset references inside CSS (`url(...)` rewriting).
- Asset references inside JS (importing `.png` / `.svg` as a URL).
- Build cache / incremental rebuilds.
- `--analyze` (bundle size breakdown).
- HMR / error overlay in the browser.
- A `manifest.json` shape richer than logical-name → hashed-path
  (no preload hints, no chunk metadata, no integrity hashes).
- Compression (gzip/br) of dev or build output.
- A `--force` flag for `zero init`.
- A `--yes` / non-interactive flag for `zero init`. Developers who
  want unattended setup can hand-write `zero.toml` first and then
  run `zero init`.
- Upward search for `zero.toml` (running zero from a subdirectory of
  the project root).
- Multiple zero apps in one project (e.g., `admin/` and `public/` both
  zero-scaffolded under the same `zero.toml`). One app per project for
  now.

## Open Questions

- **Bundler crate choice.** `oxc`, `swc_bundler`, `rollup`-via-deno,
  or something else? Each has tradeoffs: `oxc` is fastest and most
  active but newer; `swc_bundler` is mature but heavier. The plan
  must choose; the decision shapes the build pipeline's structure
  and what tree-shaking / minification come for free. Recommendation
  in the plan: pick `oxc` if its bundler is far enough along by
  implementation time, else `swc_bundler`.
- **Runtime concat strategy.** How aggressively to strip / rewrite
  the runtime's `import` / `export` statements during compile-time
  concatenation. The runtime files use a small, fixed set of import
  shapes (relative paths between siblings; named exports). A
  regex-based rewrite that joins files in dependency order and
  rewrites the final module's `export` list should suffice. The
  plan should validate this assumption against the actual runtime
  source and fall back to a real ESM rewriter if the shape is
  noisier than expected.
- **HTML parser vs string injection.** For the dev-mode HTML
  injection, simple string find-of-`</head>` is brittle (case,
  whitespace, comments containing the literal text). Recommendation
  in the plan: case-insensitive search, treat as best-effort, add
  a fallback. A real HTML parser (e.g., `html5ever`) is a much
  larger dependency for very little gain in this slice.
- **`manifest.json` key shape.** Source-relative paths
  (`"src/styles/app.css"`) vs. logical names (`"app.css"`) vs. a
  hybrid? Vite uses source-relative. Recommendation: match Vite's
  shape so backends / docs / examples can lean on prior art.
- **CSS imports inside JS.** A common pattern is `import "./foo.css"`
  in a JS file to declare a CSS dependency that the bundler
  collects. This slice's bundler may or may not handle this for
  free; if not, a developer who wants per-component CSS is forced
  to use `<link>` tags in `index.html`. Recommendation: defer
  explicit support; document the `<link>` approach for now.
- **Static-mode index.html when `[dev].proxy` is set but the
  request is for a path the project's index.html serves on disk**
  (e.g., the user keeps a `404.html` for static deploys). The spec
  only serves the project root's `index.html` in the no-proxy case.
  All proxy-mode html comes from the backend. This may surprise
  developers who expect `404.html` to be served. Recommendation:
  ignore for this slice; document the convention.
- **Port collision behavior.** Spec says exit non-zero on bind
  failure. Should `zero dev` instead retry on the next port up
  (3000 → 3001 → ...) like Vite does by default? The retry behavior
  is friendlier but changes the printed URL. Recommendation: exit
  non-zero with a clear error; auto-increment is a small follow-up.
- **The runtime's `globalThis.document` assumption.** The runtime
  reads `globalThis.document` directly; in tests this is set by
  `dom-shim.js`. In the browser, it's the real `document`, which
  is fine. In `zero build`, the bundler must not strip
  `globalThis.document` references as dead code (it has visible
  side effects). Plan should add a sanity test that the bundled
  output preserves these references. If a chosen bundler tree-shakes
  too aggressively, mark the relevant runtime functions with
  side-effect annotations (e.g., `/*#__PURE__*/` inversions, or
  `package.json` `"sideEffects"` flags — though the runtime has no
  package.json yet).
- **Where the framework spec text needs amendment.** §1 says "no
  config files (except a `tsconfig.json` ...)"; this spec adds
  `zero.toml`. §2 example HTML has a hand-written `<script
  type="module">` block; this spec scaffolds without script tags
  and injects them. The spec text should be updated when the
  broader framework spec is next revised.
- **What a `zero dev` log line should look like.** Bare-minimum
  format (`GET /src/app.js 200 4ms`)? Color? Aligned columns?
  Recommendation: pick the simplest format the plan can implement
  without a logging crate; refine later if it proves noisy or
  unhelpful.
- **Prompt crate for `zero init`.** Options include `dialoguer`,
  `inquire`, `requestty`. Each has tradeoffs in dep weight, terminal
  feature support (arrow keys, color), and testability (mocking
  stdin). Recommendation: pick the lightest crate that supports
  mocked-stdin testing, or hand-roll a minimal prompt loop (read line,
  trim, default-if-empty) if a crate adds too much weight for four
  simple prompts.
- **Validation of prompt answers.** Folder name must be a valid path
  segment (no `/`, no `..`, no empty). Port must parse as u16 in
  1–65535. Proxy URL must parse as `http://...` if non-empty. The
  plan should decide whether validation happens inline (re-prompt on
  bad input) or at the end (write the toml, then fail). Inline
  re-prompt is friendlier.
- **What if the project root has no `zero.toml` and the developer
  runs `zero dev` or `zero build` directly** (skipping `zero init`)?
  Spec says these commands error out with "zero.toml not found". An
  alternative is to suggest running `zero init` in the error message.
  Recommendation: include the hint in the error string.
- **Should `zero init`'s prompt branch refuse to run if `./<root>/`
  is already non-empty (before any prompts)?** Doing the check up
  front (before prompting) saves the developer's typing if they're
  going to be rejected. Doing it after means the prompts double as
  a confirmation that the developer really wants to scaffold here.
  Recommendation: check up front against a few common candidate
  names (or just `./web/`); if reachable through the prompt flow,
  re-check after the folder name is known and refuse cleanly.
- **Gitignored on-disk runtime mirror (deferred).** Considered and
  rejected for this slice; recording the design discussion so it
  isn't lost. The proposal: `zero dev` materializes the embedded
  runtime to `./<root>/.zero/runtime.js` on startup (overwriting
  every time — refresh is automatic, no drift), `zero init` adds
  `.zero/` (and `dist/`) to `./.gitignore`. Wins: editor
  jump-to-definition has a real file to land on; debug stack traces
  point at readable source; devs can poke the runtime to diagnose
  issues without it ever being committed. Costs: editor jump in
  most JS tooling needs `./<root>/node_modules/zero/{package.json,
  index.js}` shims to actually trigger language-server resolution
  (writing inside `node_modules/` in a project that proudly has zero
  npm deps is conceptually odd); `zero init` would write a third
  file outside `./<root>/` (`.gitignore` in addition to `zero.toml`);
  the use case isn't proven yet. Revisit when TypeScript support
  lands (since `.d.ts` files change the editor-support equation), or
  when a real developer reports needing to read the runtime source.
