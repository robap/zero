# Spec: TypeScript support

## Problem Statement

The framework's design document (`zero-framework-spec.md`) is written entirely in TypeScript: every example uses `.ts`, the scaffolded entry point is `src/app.ts`, and the spec calls for a `tsconfig.json` in the generated project. The implementation today does none of this — the scaffold emits `.js`, the dev server serves source files raw, the bundler operates only on JS, and the test runner feeds `.js` to boa. Adding TypeScript closes the largest visible gap between spec and implementation, unblocks future work that the spec assumes is in place (a generated component library, `zero gen`, eventually `zero check`), and gives users editor-grade type help when calling `import { signal, html, App } from "zero"`.

The user also intends to add decorators in a later issue. That commits us to a transpiler that does more than strip-only: decorators are a real swc transform with non-trivial semantics. Picking that transpiler now (rather than reaching for a minimal type-stripper) avoids a rip-and-replace when decorators land.

## Background

### What exists today (relevant pieces)

- **Embedded runtime pipeline**: `build.rs` concatenates `runtime/reactivity.js`, `template.js`, `router.js`, `app.js` (with `dom-shim.js` and `test.js` handled separately), strips `import` statements, flattens `export` declarations, and writes three files into `OUT_DIR`. `src/runtime.rs` reads them as `include_str!` constants and composes two ES-module strings: one for `"zero"` and one for `"zero/test"`.
- **Dev server** (`src/dev/`): axum-based; serves `/zero.js` from the precomputed runtime string, plus disk passthrough for `/src/**`, `/styles/**`, `/public/**`. `src/dev/inject.rs` injects a script tag importing `/src/app.js` and the SSE reload client. The dev server has no awareness of file extensions beyond MIME mapping in `src/dev/files.rs::content_type_for`.
- **Bundler** (`src/build/bundler.rs` + `resolver.rs`): regex-based ES → CJS rewriter that walks the import graph starting from `<root>/src/app.js`, resolves `"zero"` to the embedded runtime and `./`/`../` to user files, and emits a single CJS-style bundle. The entry path is hard-coded to `src/app.js`. The resolver only recognizes `"zero"` and relative specifiers.
- **Test runner** (`src/test_runner/`): boa-based. `harness.rs` builds a boa `Context`, evaluates `ZERO_DOM_SHIM_BODY` as a script, then parses the test file as an ES module and runs it. `loader.rs::ZeroModuleLoader` resolves `"zero"` and `"zero/test"` from the embedded strings, relative paths from disk, and rejects everything else.
- **Scaffold** (`src/scaffold/`): six files — `index.html`, `src/app.js`, `src/routes/home.js`, `src/routes/home.test.js`, `styles/app.css`, `AGENTS.md`. AGENTS.md documents the JSDoc convention currently in use.
- **`zero.toml` schema** (`src/config.rs`): three sections — `[project] root`, `[dev] port` and optional `proxy`, `[build] out`. Unknown keys are rejected.
- **Discovery** (`src/test_runner/discovery.rs`): matches `*.test.js` and `*.spec.js`.
- **Runtime tests** under `runtime/*.test.js`: framework-internal, run by `node --test`. These are not user code and are out of scope for this issue.

### Design decisions already made

- **Scope** is type-stripping, declaration files, and tsconfig. **`zero check` is explicitly out of scope** and will be its own issue. The CLI does no type-checking; types are an editor-only concern for users.
- **swc** is the chosen transpiler (over oxc). Reason: decorators are planned, swc's transform pipeline is the mature option for that, and locking it in now avoids a future migration.
- **boa stays** as the test-runner engine. swc transpiles `.ts` → `.js` before files reach boa.
- **JS and TS coexist as first-class.** `.js` files in `src/` continue to work everywhere they do today.
- **Explicit extensions on imports.** No extension-elision; `import Home from "./routes/home.ts"` is required and `import Home from "./routes/home"` is rejected by the resolver. This matches the current `.js` policy (`src/build/resolver.rs` requires the extension today).
- **Hand-written `.d.ts` files** for `"zero"` and `"zero/test"`, authored in `runtime/`, embedded into the CLI binary at compile time, written into the user's project root at `zero init`, and refreshed on `zero dev` startup so a CLI upgrade keeps user types current.
- **Source maps**:
  - **Dev server**: inline source maps on `.ts` responses by default. Configurable off via `zero.toml`.
  - **Bundler**: external `.map` files emitted when `--sourcemap` is passed (per spec). Configurable default via `zero.toml`.
  - **Test runner**: no source-map remapping; stack frames refer to stripped JS. Note as a known limitation.

### Where the work lands

Six surfaces:

1. New Rust dep + helper module: swc-based transpile function `(source, options) -> (js, optional_sourcemap)`. Used by the dev server, bundler, and test runner.
2. Dev server (`src/dev/files.rs`, `src/dev/server.rs`, possibly a new `src/dev/transpile.rs`): when serving `/src/*.ts`, transpile and respond with JS content-type, inlining the source map by default.
3. Bundler (`src/build/bundler.rs`, `src/build/resolver.rs`, `src/cmd/build.rs`): entry point becomes `src/app.ts` if present, else `src/app.js`. Resolver accepts `.ts` specifiers. `extract_imports` and the regex rewriter must handle the TS input *post-transpile* (simplest: transpile each file to JS first, then run the existing CJS rewrite path on the JS). `--sourcemap` flag honored; `zero.toml` provides the default.
4. Test runner (`src/test_runner/discovery.rs`, `src/test_runner/loader.rs`, `src/test_runner/harness.rs`): discovery adds `*.test.ts` / `*.spec.ts`. The loader transpiles `.ts` files before handing them to boa.
5. Type declarations: new `runtime/zero.d.ts` and `runtime/zero-test.d.ts`, embedded via `build.rs`/`runtime.rs`, written to `<root>/` by `zero init` and refreshed by `zero dev`.
6. Scaffold (`src/scaffold/`): canonical files flip to `.ts` (`app.ts`, `routes/home.ts`, `routes/home.test.ts`). New `tsconfig.json` template. `index.html` script reference flips from `/src/app.js` to `/src/app.ts` (and the dev-server injector at `src/dev/inject.rs` must match). `AGENTS.md` updates to show TS as the canonical authoring path.

## Requirements

### Transpiler integration

1. Add a swc-based transpiler module (working name `src/transpile.rs`, exact location TBD). It exposes one function: take a `.ts` source string + options (sourcemap on/off, file path for diagnostics) and return stripped JS plus an optional sourcemap.
2. The transpiler MUST be invoked once per file per request/build/test run. No global cache is required for v1; correctness over caching.
3. Transpile failure surfaces a structured error (file, line, column, message) — not a panic.
4. Decorators are NOT enabled in v1 (separate issue). The swc config explicitly disables them so accidental decorator syntax in user code produces a clear error.

### Dev server

5. `GET /src/<path>.ts` returns transpiled JS with `Content-Type: application/javascript; charset=utf-8`.
6. By default, the dev-server response includes an inline source map (`//# sourceMappingURL=data:...`). When `zero.toml` disables source maps (key TBD; see Open Questions), the inline map is omitted.
7. `GET /src/<path>.js` continues to serve raw `.js` unchanged.
8. `src/dev/inject.rs::DEV_SCRIPTS` updates so the bootstrap `<script type="module" src="...">` points at `/src/app.ts` (since the scaffold's canonical entry is now `.ts`). This MUST remain compatible with `.js`-only projects — for them, the browser will 404 on `/src/app.ts`, which is acceptable IFF the scaffold's `index.html` continues to control the entry tag. **Resolution**: the dev-server injector continues to inject `/src/app.js` (current behavior), and the scaffold's `index.html` for new TS projects sets its own `<script type="module" src="/src/app.ts">` tag. See Open Questions for the alternative.
9. Transpile errors return HTTP 500 with the error body as text so the browser displays them clearly.

### Bundler / `zero build`

10. `zero build` detects the entry point: prefer `<root>/src/app.ts`, fall back to `<root>/src/app.js`. Exactly one MUST exist; an error is returned if both are present (parallels the "no extension collision" rule).
11. Imports may reference `.ts` files; the resolver accepts them. Extension is required; resolver still rejects extensionless and bare specifiers (except `"zero"`).
12. The bundler's existing CJS rewrite step operates on the transpiled JS of each module. Simplest implementation: walk-and-read produces the source for each module; if extension is `.ts`, transpile before feeding to `extract_imports` / `rewrite_module`. (`extract_imports` and the rewriter regexes were authored for JS syntax — running them against TS source is unsafe.)
13. `zero build --sourcemap` emits a `.map` file next to the bundle and adds `//# sourceMappingURL=` to the bundle output. Default is off, overridable in `zero.toml`. The build command grows the flag.
14. The hashed filename for the bundle continues to be `app.<hash>.js` regardless of whether the entry was `.ts` or `.js`. The `manifest.json` key `"app.js"` is preserved (manifest keys are logical, not source extensions).

### Test runner

15. Discovery picks up `*.test.ts` and `*.spec.ts` in addition to `*.test.js` and `*.spec.js`.
16. `ZeroModuleLoader::resolve_relative` accepts `.ts` specifiers, transpiling the source before passing to `Module::parse`.
17. Stack-trace line numbers refer to the transpiled JS, not the `.ts` source. This is a documented v1 limitation.
18. `.test.ts` and `.test.js` files MUST NOT both exist for the same logical test (e.g., `home.test.ts` and `home.test.js` for the same component). Discovery should error if it sees this collision. This matches the bundler's "no extension collision" rule.

### Type declarations

19. New files: `runtime/zero.d.ts` and `runtime/zero-test.d.ts`, hand-written. Surface MUST cover every name in `ZERO_RUNTIME_EXPORTS` (excluding underscore-prefixed internals) and `ZERO_TEST_EXPORTS`.
20. `build.rs` reads both files and embeds them as `ZERO_TYPES_BODY` and `ZERO_TEST_TYPES_BODY` (final names TBD). `src/runtime.rs` exposes them as `pub const`.
21. `zero init` writes `<root>/zero.d.ts` and `<root>/zero-test.d.ts` (final names/layout TBD — could be one combined file with `declare module` blocks). These files form the contract between the embedded types and the user's tsconfig.
22. `zero dev` re-writes those files on startup so a CLI upgrade keeps user types fresh without a re-init.
23. Generated types MUST cover at least: `signal<T>`, `computed<T>`, `effect`, `App`, `inject<T>`, `html` (TaggedTemplateFunction), `TemplateResult`, `each`, `ref<T>`, `navigate`, `back`, `forward`, `route`. From `zero/test`: `describe`, `it`, `expect` (chainable), `render`, `find`, `findAll`, `text`, `fire`, `cleanup`, `beforeEach`, `afterEach`, `beforeAll`, `afterAll`.

### Scaffold

24. The canonical scaffold flips to `.ts`:
    - `src/app.js` → `src/app.ts`
    - `src/routes/home.js` → `src/routes/home.ts`
    - `src/routes/home.test.js` → `src/routes/home.test.ts`
    - `index.html`'s `<script>` tag (if any) updates accordingly. (Today the scaffold's `index.html` has no `<script>` — dev/build inject — so this may be a no-op.)
25. A new `tsconfig.json` template ships in `src/scaffold/`. Exact contents TBD (see Open Questions), but it MUST:
    - Reference the locally-written `zero.d.ts` so `import { signal } from "zero"` typechecks in editors.
    - Allow `.ts` extensions on imports (e.g., `allowImportingTsExtensions: true`) since the project policy is explicit extensions.
    - Be marked "editor use only" in a comment (per framework spec §10).
26. `AGENTS.md` updates to show TS as the canonical authoring path. JSDoc examples remain referenced but no longer primary. Scope of the rewrite is an open question.
27. `zero init` still refuses to overwrite a non-empty `<root>/` directory.

### Configuration (`zero.toml`)

28. A new key (or keys) governs source-map emission. Exact placement TBD (see Open Questions). The validator rejects unknown keys today; the new key MUST be added to whichever struct(s) it belongs in (`DevConfig`, `BuildConfig`, or a new `[transpile]` section).

### Backwards compatibility

29. A pure-JS project from a previous version of `zero` continues to build, dev, and test without modification. Specifically:
    - `src/app.js` as entry still works in `zero build`.
    - `home.test.js` still works in `zero test`.
    - The bundler's existing regex rewriter is unchanged for `.js` files.

## Constraints

- **No new npm dependencies** ever. This is the framework's defining philosophy. The transpiler is a Rust crate (`swc_core` or the finer-grained `swc_ecma_*` crates) pulled in via `Cargo.toml`.
- **Single binary distribution preserved.** Even with swc, the CLI is one binary.
- **Build time impact acknowledged.** swc's transitive deps will lengthen clean builds significantly. This is accepted given the decorator roadmap.
- **No JSX.** Spec §13 forbids it. swc's TS transform is configured for `ts`, not `tsx`. `.tsx` files are not recognized.
- **No type-checking inside the CLI.** swc only strips. Users get type-checking via their editor's TS server reading `tsconfig.json`.
- **Decorators are NOT enabled in v1.** Even though we're picking swc partly *because* of decorators, this issue does not turn them on. The transpiler config explicitly disables them so accidental decorator use produces a clear error.
- **Embedded type declarations MUST stay in sync with `ZERO_RUNTIME_EXPORTS` / `ZERO_TEST_EXPORTS`.** Add a compile-time test (parallel to the existing `runtime.rs` tests) that fails if a name is exported by the runtime but missing from the embedded `.d.ts`.
- **No global module cache** in the dev-server transpiler for v1. Per-request transpile is fine. (File-watching invalidation is not a concern when we re-transpile every request.)

## Out of Scope

- **`zero check`** — full type-checking. Separate issue. Users get type-checking from their editor's TS server, not the CLI.
- **Decorators** — separate issue. swc is chosen *because* of this future need, but no decorator transform is enabled here.
- **Source-map remapping for boa stack traces.** Test failures show line numbers in the stripped JS. Documented limitation.
- **JSX / `.tsx`.** Not supported, per the framework spec.
- **`zero fmt` / `zero lint`** — separate Phase 6 items.
- **`zero gen`** — separate Phase 6 item. Once it lands, it benefits from this work (it'll generate `.ts` files).
- **HMR.** Independent of TS support.
- **A migration tool** to convert existing `.js` projects to `.ts`. Not needed since both coexist.
- **Caching transpiled output to disk** (a `.zero-cache/` or similar). v1 transpiles on every request and every test run. If perf becomes a problem, a cache is a follow-up.
- **Generating `.d.ts` from the runtime's JSDoc.** Hand-written, per the decision above.

## Open Questions

- **`zero.toml` schema for the sourcemap toggle.** Three plausible shapes:
  - `[dev] sourcemap = true/false` and `[build] sourcemap = true/false` — two independent toggles.
  - A single `[transpile] sourcemap = true/false` — applies to both.
  - One key shared across sections.

  Recommendation for the plan phase: two independent toggles under `[dev]` and `[build]`, since dev and build have different defaults (on vs. off) and likely different per-project preferences.

- **Dev-script injection (requirement 8).** Today `src/dev/inject.rs` hardcodes `<script type="module" src="/src/app.js">`. With TS-first scaffolds, that script tag is wrong for new projects. Options:
  - Keep injecting `app.js` and require the scaffold's `index.html` to add its own `<script>` for `app.ts`.
  - Make the injector probe the project to choose `.ts` vs `.js`.
  - Drop the injected app-entry script altogether and require every project's `index.html` to declare its own `<script>` (cleaner spec, breaks existing projects).

  The plan phase should pick one. Probe-the-project is the least disruptive.

- **Type-declaration file layout.** One file with two `declare module` blocks, or two files? And what tsconfig mechanism links them — `paths`, `types`, or `include`? This is a small but real decision; the plan phase should resolve it.

- **Exact tsconfig.json contents.** A starting point per framework spec §10 exists but doesn't account for the hand-written local `.d.ts` files or the explicit-extension policy. The plan phase should write the final template.

- **AGENTS.md rewrite scope.** Full rewrite to TS examples, or minimal additions noting the TS-first scaffold? The plan phase should pick.

- **`extract_imports` regex behavior on TS source.** The current regex was authored for JS. Plan should confirm whether transpiling before extraction (recommended in requirement 12) covers all cases, or whether the regex needs widening for TS-only forms (e.g., `import type { Foo } from "..."`).

- **Bundler error if entry exists in both `.ts` and `.js`** (requirement 10): is it a hard error, or does one win? Recommendation is hard error (matches the no-collision rule); the plan phase should confirm.

- **`zero dev` startup-time `.d.ts` refresh** (requirement 22): should it overwrite unconditionally, or only if the file is missing / older than the CLI binary? Unconditional overwrite is simplest but stomps user edits to a file users may not realize is auto-managed.
