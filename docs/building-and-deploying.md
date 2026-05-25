---
title: Building and Deploying
nav_order: 11
---

# Building and Deploying

`zero build` produces a deployable bundle. No `node_modules`, no
external bundler, no toolchain configuration — the same binary
that runs the dev server compiles your app for production.

## `zero build`

```sh
zero build
```

The default output is `./dist/`:

```
dist/
├── assets/
│   ├── app.<hash>.js          # bundled runtime + your source
│   └── app.<hash>.css         # compiled, hashed CSS
├── manifest.json              # logical name → hashed filename
└── index.html                 # static index with <script>/<link> tags pre-injected
```

Both `app.<hash>.js` and `app.<hash>.css` are minified — production builds always minify; there is no flag.

Flags:

| Flag                    | Behaviour                                                                       |
|-------------------------|---------------------------------------------------------------------------------|
| `-o, --out <dir>`       | Output directory (default `dist/`, configurable via `[build] out` in `zero.toml`). |
| `--analyze`             | Print a bundle-size breakdown.                                                  |
| `--sourcemap`           | Emit external source maps (default off). When enabled, the JS map composes positions in the minified bundle back to the original source files. |
| `--target <env>`        | `static` (default), `server`, or `worker`.                                      |

## `manifest.json`

The manifest is a flat map from logical asset names to their
hashed filenames. Your server reads it to inject the right
`<script>` and `<link>` tags into server-rendered HTML.

```json
{
  "app.js":         "assets/app.a3f2b1c4.js",
  "styles/app.css": "assets/app.5e8d9f01.css"
}
```

Keys are stable across builds (they match the logical path of
the source); values change every time the file's content
changes. A change to any source you depend on invalidates the
hash and therefore the cached asset on the client.

## Integrating with a backend

For a server-rendered site (Rust, Go, Ruby, anything) that
embeds the zero bundle:

1. **Serve `dist/assets/` as static files.** Long cache TTL is
   safe — every asset is fingerprinted.
2. **Read `dist/manifest.json` at server start.** Cache it in
   memory; re-read on deploy.
3. **Inject the tags.** When rendering HTML, look up the asset
   names you need in the manifest and emit:

   ```html
   <link rel="stylesheet" href="/assets/app.5e8d9f01.css">
   <script type="module" src="/assets/app.a3f2b1c4.js"></script>
   ```

That's the whole integration. There is no SDK, no server
runtime, no plugin — `dist/manifest.json` is the contract.

## Static deploys

If you don't have a backend, `dist/index.html` is a complete
static page. Upload `dist/` to any static host (GitHub Pages,
Netlify, Cloudflare Pages, S3 + CloudFront, plain nginx):

```sh
zero build
rsync -av dist/ user@host:/var/www/my-app/
```

`dist/index.html` already has the hashed `<script>` and `<link>`
tags inlined, so no server-side templating is needed.

## `zero preview`

Before deploying, you can run the production build locally:

```sh
zero preview
```

This serves `dist/` on `http://127.0.0.1:3000` with the same
SPA fallback semantics as `zero dev` (unknown paths return
`index.html`), but using the compiled bundle. It's how you sanity-
check that the build outputs work end-to-end before pushing to
prod.

`zero preview` first runs `zero build`, then serves the result.
Because the build clears the output directory before writing,
`dist/` only ever contains the most recent build's artifacts — no
stragglers from prior runs.
