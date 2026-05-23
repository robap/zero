# Spec: Production minification (JS + CSS)

## Problem Statement

`zero build` currently emits pretty-printed JavaScript and expanded CSS. The
bundle that ships to production carries every comment, every indent, every
long-form identifier, and every blank line. For the showcase app today that's
roughly two to three times the bytes a real production build would send.

The framework's pitch is "one binary, no toolchain, fast by default." Shipping
un-minified bundles undermines that — it forces users to either accept a heavy
bundle or reach for the very third-party tooling zero is meant to replace.
Both of the framework's prior specs that touched output (`scss`,
`cli-bootstrap`) explicitly deferred minification with the same note:
"future concern." This is that future.

Scope is JS *and* CSS minification, always on in `zero build`, no flag, no
config knob. Source maps continue to be opt-in via `--sourcemap`, and when
requested they map the *minified* output back to original sources — the
standard production pattern. Tree-shaking, which is a structural bundler
change rather than a post-process, stays out of scope (see Out of Scope for
why).

## Background

### What exists today

- **JS bundle.** `crates/zero-bundler/src/bundler.rs` walks the import graph,
  transpiles `.ts` via `swc_core`, rewrites each module's ES import / export
  forms into CJS-style `__zero_require` / `exports.x = x` via regex, and
  concatenates every module's source inside a `__zero_define(id, function(...)
  { ... })` wrapper. The output is the pretty-printed source itself — there is
  no post-process pass. The runtime modules (`zero` and `zero/http`) are
  embedded verbatim as `ZERO_RUNTIME_BODY` / `ZERO_HTTP_BODY` constants from
  `zero-runtime`, again with no minification.
- **CSS bundle.** `crates/zero-bundler/src/css.rs` walks `styles/`, copies
  plain `.css` through, and compiles `.scss` via `grass` in its default
  (expanded) output style. Hash-based filenames are computed from the
  *expanded* output bytes. `grass` already supports a compressed output style;
  it is simply not enabled.
- **Source maps.** Opt-in via `--sourcemap` / `[build] sourcemap = true`.
  Today's JS map is "coarse": `build_combined_sourcemap` in `bundler.rs`
  registers source *paths* but emits no row-level mappings — line-accurate
  stack traces inside the bundle are not in scope of the current
  implementation. SCSS minification via grass already supports an external
  `.map` file when compressed mode is set.
- **CLI surface.** `Build { sourcemap, no_sourcemap }` in
  `crates/zero/src/main.rs`. No `--minify` / `--no-minify` flag exists. The
  `--analyze` and `--target` flags shown in `docs/building-and-deploying.md`
  are documented but not yet implemented and are not part of this slice.
- **Transpiler.** `crates/zero-transpile` wraps `swc_core` for type-strip
  only. It exposes `parse_module`, returning the AST + a shared `SourceMap`
  the bundler can reuse for downstream passes.
- **SWC version.** Workspace pins `swc_core = "65"` with `ecma_parser`,
  `ecma_codegen`, `ecma_transforms*`, `ecma_visit`, `ecma_ast`, `common`,
  `common_sourcemap`. No `ecma_minifier` feature is currently enabled.
- **Manifest.** `manifest.json` keys are content-hashed filenames. A change
  in bundle bytes (which minification *will* cause) invalidates the hash and
  therefore the cached asset — by design.
- **Showcase integration.** `tests/showcase_build.rs` runs `zero build`
  against `showcase/`. The test currently asserts the build succeeds and the
  manifest is shaped correctly. It is the natural surface for an
  end-to-end "minification actually shrinks the bundle" assertion.
- **Test runner / engine.** `cargo run -p zero -- test` (and `zero test`)
  runs JS through Boa. Round-tripping a minified bundle through Boa is the
  cheapest way to assert that minification didn't break the emitted shape.

### Decisions made during refine

The user confirmed each of the following:

- **Scope: both JS and CSS.** A single coherent build-quality slice. CSS is
  essentially "free" given `grass` already supports a compressed mode; JS is
  the real work via `swc_ecma_minifier`. Hashed filenames recompute against
  the minified bytes (manifest stays correct by construction).
- **Always on, no flag.** Production builds always minify. Matches zero's
  opinionated, no-config posture (the same posture used for `fmt` and `lint`).
  No `--minify` / `--no-minify` CLI flag, no `[build] minify` key. If a user
  needs to inspect un-minified output, they can run the dev server (which
  serves un-minified ESM per file) or use a source map.
- **Source maps map *minified → original*.** When `--sourcemap` is set,
  minification runs first and the source map points from minified positions
  back to the original source files (the standard production pattern). The
  existing coarse map is replaced: this slice produces actual row/column
  mappings for the minified JS. Without `--sourcemap`, no map is emitted and
  the minifier runs without tracking positions.
- **Aggression: standard.** Strip whitespace + comments, shorten *local*
  identifiers, drop dead code. **Do not mangle property names** — user code
  may do `obj[dynamicKey]` or other reflective access, and property mangling
  silently breaks it. This matches the default posture of `terser` and
  `esbuild`'s `--minify` without `--mangle-props`.
- **Tree-shaking out of scope.** Tree-shaking interacts structurally with
  the bundler's CJS factory wrappers — modules currently live inside
  `__zero_define(id, function(exports, __zero_require) { ... })`, which is
  opaque to any tree-shaker. Real tree-shaking is a separate, larger item
  about preserving ESM through the bundle. SWC's minifier does still
  perform *intra-module* dead-code elimination, which captures a meaningful
  chunk of the same wins.
- **Verification: unit tests + showcase build size assertion.** Per-crate
  unit tests in `zero-bundler` confirm: (a) minified output is meaningfully
  smaller than un-minified for a representative input; (b) the minified
  bundle still evaluates correctly in Boa (round-trip). The existing
  `tests/showcase_build.rs` gains an assertion that the showcase's `app.js`
  is at least 30% smaller than its un-minified equivalent (the bundler
  exposes the un-minified intermediate to the test harness for comparison;
  see Requirement 18).
- **Library: `swc_ecma_minifier` as a separate dep.** Reuses the SWC AST /
  codegen layer the transpiler already uses, so the bundler doesn't gain a
  second parser. The crate version chosen must match the AST shape exposed
  by the pinned `swc_core = "65"` (plan resolves the exact version pin and
  whether `swc_core`'s own `ecma_minifier` feature is available at 65; if
  it is, prefer the feature flag over the standalone crate to keep the
  dep set tight).

### Current vs. target output shape (illustrative)

```js
// today — what `dist/assets/app.<hash>.js` looks like
const __zero_modules = {};
const __zero_cache = {};
function __zero_define(id, factory) { __zero_modules[id] = factory; }
function __zero_require(id) {
  if (__zero_cache[id]) return __zero_cache[id].exports;
  const mod = { exports: {} };
  __zero_cache[id] = mod;
  __zero_modules[id](mod.exports, __zero_require);
  return mod.exports;
}

__zero_define('zero', function(exports, __zero_require) {
  function signal(initialValue) {
    // ... full source, comments, indents ...
  }
  exports.signal = signal;
});

__zero_define('./src/app.ts', function(exports, __zero_require) {
  const { signal } = __zero_require('zero');
  const count = signal(0);
  count.set(count.val + 1);
});

__zero_require('./src/app.ts');
```

```js
// after — minified shape (illustrative, not byte-exact)
const e={},t={};function n(t,n){e[t]=n}function r(n){if(t[n])return t[n].exports;const o={exports:{}};t[n]=o;return e[n](o.exports,r),o.exports}n("zero",function(e,t){function n(t){/* … */}e.signal=n});n("./src/app.ts",function(e,t){const{signal:n}=t("zero"),r=n(0);r.set(r.val+1)});r("./src/app.ts");
```

Property names (`signal`, `set`, `val`, `exports`, `__zero_define`,
`__zero_require`) are preserved — the runtime's CJS-shim contract depends
on them being addressable from user code that's already been concatenated
into the same bundle.

## Requirements

### JS minification

1. The bundler gains a minification pass that runs after `bundle()`'s
   current emit but before the bundle bytes are returned. The
   `BundleOutput { code, source_map }` struct keeps the same shape; `code`
   contains minified output for production builds.

2. Minification is **always on** in `zero build`. There is no `--minify` /
   `--no-minify` CLI flag and no `[build] minify` config key. The dev
   server's per-file transpile (`zero dev`) is unaffected.

3. Minification uses SWC. The plan picks between (a) enabling
   `swc_core`'s `ecma_minifier` feature at version 65, or (b) adding the
   standalone `swc_ecma_minifier` crate at the version whose AST matches
   `swc_core 65`. If both are viable, prefer (a) — fewer crates in the
   build graph.

4. Minifier configuration:
   - `compress`: enabled with default options. Dead code elimination,
     constant folding, and the standard set of safe rewrites.
   - `mangle`: enabled for **local identifiers only**. Property names
     and top-level identifiers reachable from the bundle's
     `__zero_require` / `__zero_define` contract must **not** be
     mangled. Concretely: `mangle.props` is disabled / unset.
   - `format`: minimal whitespace, no comments retained except those
     marked legal (e.g. `/*!` license banners, if any user code uses
     them — opt-in via the standard `preserve` predicate).
   - Target: ES2022 or whatever EsVersion the existing transpiler emits
     against. The plan confirms by reading
     `crates/zero-transpile/src/lib.rs` (`EsVersion::EsNext` today).

5. Reserved identifier list: the names referenced by the CJS shim and
   re-export plumbing must be preserved through mangle. At minimum:
   `__zero_modules`, `__zero_cache`, `__zero_define`, `__zero_require`,
   `exports`, `module`, `default`. The plan enumerates the full list
   based on what the bundler's `PREAMBLE` and `rewrite_*` passes emit.

6. The minifier runs on the **whole bundled output** (one pass), not
   per-module. Running per-module would defeat cross-module dead-code
   elimination opportunities within the CJS shim itself. Acceptance: the
   minified bundle is a single string, not a list.

7. Comments are stripped unless explicitly preserved. The `/*!` legal-
   comment convention is respected (preserved in the output).

8. Minification failures are surfaced as `anyhow::Error` from `bundle()`
   with the originating file (when SWC reports a position) included in
   the message. Build fails with a non-zero exit code; no fallback to
   un-minified output. (Silent fallback would hide regressions.)

### CSS minification

9. `css.rs` switches `grass` from its default expanded output style to
   the compressed output style. This applies to both `.scss` compilation
   and the copy-through path for `.css` files (plain CSS is run through
   a single compressed-mode round-trip via grass so output is uniform —
   *or*, if grass's compressed mode requires the input to be SCSS, the
   plan picks the simplest path: either re-emit plain CSS through grass,
   or strip whitespace from plain CSS via a small in-house pass). The
   plan resolves which.

10. Hashed CSS filenames are computed from the **minified** output bytes
    so the content hash reflects the bytes shipped, not the intermediate.
    Existing logic in `css.rs` that hashes after compile already does
    this correctly; verify no path hashes pre-compile bytes.

11. CSS source maps continue to be opt-in via `--sourcemap` /
    `[build] sourcemap = true`. When enabled with minified output, the
    `.map` file maps positions in the minified CSS back to the original
    `.scss` / `.css` source. The `sourceMappingURL` comment in the CSS
    file points at the external `.map` (same as today).

### Source maps over minified output

12. JS source maps, when enabled, map the **minified bundle positions**
    back to original source files. The plan picks the simplest viable
    pipeline. Two acceptable shapes:
    - **(a) Single-pass minify-with-mappings.** Configure the minifier
      to emit a position buffer, then build a v3 source map directly
      from those positions against the pre-minify source.
    - **(b) Two-stage merge.** Keep the existing
      `build_combined_sourcemap` to produce a coarse map from
      original → un-minified bundle, then merge it with the minifier's
      un-minified → minified map via the `sourcemap` crate's chaining
      support, producing a single original → minified map.

    Either is acceptable; the plan picks based on what `swc_ecma_minifier`
    actually exposes at the chosen version. The contract the plan must
    meet: when `--sourcemap` is on, opening the produced map against the
    minified bundle in any standard tool resolves a position back to a
    real source file path that exists under the project root.

13. When `--sourcemap` is off (the default), the minifier runs **without**
    tracking positions. No `sourceMappingURL` comment is appended to the
    JS.

14. The existing coarse `build_combined_sourcemap` is removed and
    replaced by the new minified-map pipeline. The two unit tests in
    `bundler.rs` that touch source maps
    (`bundle_emits_no_source_map_by_default`,
    `bundle_emits_source_map_when_requested`) are kept but their
    assertions are updated to match the new contract — specifically, the
    "requested" test additionally asserts that the map contains real
    mappings (non-empty `"mappings"` field), not only that `"sources"` is
    populated.

### Bundler API & internals

15. `bundle(config, emit_sourcemap)` keeps its signature. Its
    `BundleOutput` keeps its shape. Internally, the function gains a
    final minify step between `emit_factories` / source-map build and
    return.

16. The implementation lives in a new module `crates/zero-bundler/src/
    minify.rs` exposing a single function:
    `pub fn minify_js(code: &str, source_map_in: Option<&str>, emit_source_map: bool) -> anyhow::Result<MinifyOutput>`
    where `MinifyOutput { code, source_map: Option<String> }`. Keeping
    minification in its own module isolates the SWC minifier
    configuration and the source-map merge logic from the bundler's
    module-graph code.

17. `bundler::bundle` calls `minify::minify_js` once at the end of its
    pipeline, passing the in-progress source map (if `emit_sourcemap`)
    so the merge happens inside `minify_js`. The returned
    `BundleOutput.source_map` reflects the merged map when
    `emit_sourcemap`.

### Bundler API surface for tests

18. To support the showcase size-budget assertion (Requirement 22), the
    bundler exposes a test-only function:
    `pub fn bundle_unminified(config: &Config, emit_sourcemap: bool) -> anyhow::Result<BundleOutput>`
    gated behind `#[cfg(any(test, feature = "test-internals"))]` or
    `pub(crate)` plus a re-export through the crate's test surface. The
    plan picks the precise mechanism. This is the only addition to the
    public surface; it does not appear in `cargo doc` for downstream
    consumers.

### Tests

19. New unit tests in `crates/zero-bundler/src/bundler.rs` (or a new
    `minify.rs` test module):
    - **JS shrinks.** Bundle a fixture project (the same shape used by
      `bundle_with_ts_entry_strips_types_and_imports_zero` is a fine
      base) and assert that the minified bundle is at least 40% smaller
      than the un-minified equivalent.
    - **JS still evaluates.** Take a fixture that does a known
      computation (e.g. defines a function, calls it, assigns the
      result to `globalThis.result`) and run the minified bundle
      through `boa_engine`. Assert the global comes out with the
      expected value. This catches mangle-induced breakage and the
      `__zero_require` / `exports` contract.
    - **Reserved names preserved.** Bundle a fixture and assert that
      the minified output still contains the literal strings
      `__zero_define`, `__zero_require`, `__zero_modules`,
      `__zero_cache`, and the user module IDs referenced by
      `__zero_define('./src/app.ts'`. (Strings, not regex — these are
      contractual.)
    - **Source map round-trip when requested.** With `emit_sourcemap =
      true`, the returned map has `version: 3`, a non-empty
      `"mappings"` field, and `"sources"` contains the original `.ts`
      file path.
    - **No source map when not requested.** With `emit_sourcemap =
      false`, `BundleOutput.source_map` is `None` and the code contains
      no `sourceMappingURL` comment.
    - **Comments stripped, legal comments retained.** Bundle a fixture
      whose `src/app.ts` contains both a regular `/* ... */` block and
      a `/*! license ... */` block. The regular comment is gone from
      the output; the legal comment is preserved.

20. New unit tests in `crates/zero-bundler/src/css.rs`:
    - **CSS shrinks.** A fixture with a `styles/app.scss` containing
      whitespace, comments, and indented rules produces compressed
      output. Assert the output contains no `\n  ` (two-space indent
      with a leading newline) and no `/* ... */` comments (other than
      legal comments — plan judges whether grass's compressed mode
      preserves these and adds an assertion if so).
    - **CSS still loads.** A fixture with `styles/app.scss` defining a
      simple rule (`.x { color: red }`) is parseable when re-fed to
      grass (round-trip), guarding against bad compressed output.
    - **CSS hash is over minified bytes.** Bundle the same `app.scss`
      twice, once with a trailing newline in the source and once
      without. Both should produce **the same** output hash (because
      compressed mode normalizes whitespace) — if they differ, the
      hash is over pre-compile bytes and should be fixed.

21. `tests/showcase_build.rs` gains assertions:
    - The on-disk `app.<hash>.js` produced by `zero build` against
      `showcase/` is **at least 30% smaller** than the equivalent
      un-minified bundle the test computes via
      `zero_bundler::bundle_unminified` against the same showcase
      config.
    - The on-disk `app.<hash>.css` does not contain four consecutive
      ASCII spaces (`"    "`) or any `\n\n` — proxy for "is minified".
      The plan can pick a tighter assertion if grass's compressed
      output guarantees something more specific.

22. The existing `bundle_with_ts_entry_strips_types_and_imports_zero`,
    `bundle_inlines_zero_http_when_imported`, and
    `bundle_mixed_ts_and_js_dependencies` tests are updated as needed.
    Their current assertions search for specific substrings
    (`__zero_define('./src/app.ts'`, `__zero_require('zero/http')`,
    etc.) — those substrings survive standard minification because they
    are string literals used as object keys. The plan verifies which (if
    any) of the existing assertions need adjustment.

23. `bundle_errors_when_both_entries_present` is unaffected (no minify
    pass on the error path).

### Documentation

24. `docs/building-and-deploying.md`:
    - The default-output description gains one sentence noting that JS
      and CSS are minified.
    - The `--sourcemap` flag's description gains one sentence noting
      that the map points from minified output back to original
      sources.
    - The `--analyze` and `--target` rows in the flag table are
      orthogonal and stay as-is.

25. `docs/config-and-cli.md`:
    - The `zero build` flag table gains no new rows (no `--minify`
      flag).
    - A short note under "`zero build`" mentions that production
      output is always minified; the dev server is unaffected.

26. `docs/why-zero.md` is *not* required to change. (Minification is a
    quality-of-output detail, not a positioning bet. The plan may
    decline to touch it.)

27. The deferred-future notes in `issues/scss/spec.md` and
    `issues/cli-bootstrap/spec.md` that reference minification as a
    future concern are **not** edited — those specs are historical
    records of what was true when they shipped.

### Build / cargo

28. `crates/zero-bundler/Cargo.toml` gains the chosen minifier dep
    (either an `ecma_minifier` feature added to the existing `swc_core`
    workspace dep, or a new `swc_ecma_minifier = "..."` line — the plan
    picks the one that compiles against pinned `swc_core = "65"`).

29. If the standalone crate is chosen, it goes in
    `[workspace.dependencies]` in the root `Cargo.toml` and is
    referenced from `zero-bundler` via `workspace = true`. Matches the
    pattern used by every other shared dep.

30. No new top-level dependency on a non-SWC minifier (`minify-js`,
    `oxc_minifier`, etc.) is added.

## Constraints

- **Always on.** Production builds always minify. No CLI flag, no
  config key. The dev server is unaffected (it ships per-file ESM via
  swc's transpile, not via the bundler).
- **No property mangling.** `mangle.props` is disabled. User code that
  does reflective property access (`obj[dynamicKey]`,
  `Object.keys(obj)`, etc.) must continue to work without an opt-out
  list.
- **Reserved names preserved.** The CJS-shim contract names
  (`__zero_modules`, `__zero_cache`, `__zero_define`, `__zero_require`,
  `exports`, `module`, `default`, etc.) are never mangled.
- **Source maps map minified → original.** When opt-in via
  `--sourcemap`, the produced map is usable in standard tools and
  resolves to original source paths under the project root.
- **No silent fallback.** Minification failures are hard errors with a
  non-zero exit code. We do not silently emit un-minified output on
  failure — that would mask regressions.
- **One library family.** Use SWC for JS (matches the existing
  transpiler) and grass for CSS (matches the existing SCSS path). No
  new parser / AST gets pulled into the bundler.
- **Hashed filenames reflect minified bytes.** The manifest contract
  ("hash changes iff content changes") is preserved by hashing
  *after* minification for both JS and CSS.
- **Bundle shape unchanged.** The CJS-shim contract
  (`__zero_define` / `__zero_require` / module IDs as string keys) is
  preserved. The plan does not rework the bundler to emit ESM, drop
  the runtime shim, or otherwise restructure the output — that's
  tree-shaking's job.
- **No new top-level zero APIs.** Minification is a build-time concern;
  no new runtime exports.
- **No new opt-out in the CLI surface.** Resist adding a debug flag.
  If a user needs the un-minified bundle for debugging, the dev
  server and the source-map workflow already cover it.

## Out of Scope

- **Tree-shaking.** A separate, structural change to the bundler
  (preserve ESM through the graph, eliminate unreachable exports
  across module boundaries). SWC minifier's *intra-module* dead-code
  elimination still applies and captures many of the same wins.
- **`--minify` / `--no-minify` CLI flag.** Minification is always on
  for production builds. No flag.
- **`[build] minify` config key.** Same reason.
- **Property mangling.** `mangle.props` stays off. Adding it would
  require an opt-in list and would silently break reflective access.
- **Bundle splitting / code splitting.** Single bundle remains the
  production shape; splitting is a separate future concern.
- **`--analyze` bundle-size breakdown.** Documented as a planned flag
  but not implemented today and not added by this slice.
- **Custom legal-comment patterns.** Standard `/*!` retention only.
  Users who need a custom predicate (`@license`, `@preserve`, etc.)
  open a separate issue.
- **HTML minification.** `dist/index.html` continues to be emitted as
  the existing `index_html.rs` writes it. Minifying HTML adds little
  for a single-file SPA shell and pulls in a fourth parser surface.
- **Source-map content embedding.** `sourcesContent` (the inline
  copy of original sources in the map) is not added by this slice.
  Plan may add it if cheap; otherwise external sources via path is
  fine.
- **Bundle-size budget file** (`bundle-budget.json`). Deferred until
  there are multiple entry points to budget against. The single
  showcase assertion is enough governance for v1.
- **Dev-server minification.** `zero dev` serves un-minified ESM per
  file. Not touched.
- **Per-target minification differences.** `--target server` /
  `--target worker` flags don't exist yet (only documented). When
  they do, minification settings may need to vary; not in this
  slice.

## Open Questions

- **swc_core 65's `ecma_minifier` feature.** Is the feature available
  at that version, and does it expose a stable enough API? The plan
  verifies by reading swc_core 65's `Cargo.toml` and either enabling
  the feature or pinning the standalone `swc_ecma_minifier` to the
  matching version.
- **Plain `.css` files: round-trip through grass, or in-house
  whitespace strip?** Grass is the existing tool; if it accepts plain
  CSS in compressed mode without surprise, prefer that. Plan
  confirms.
- **Single-pass vs. two-stage source-map pipeline (Requirement 12).**
  Plan picks based on what swc_ecma_minifier actually exposes.
- **Reserved-name list completeness.** Requirement 5 enumerates the
  obvious names; the plan greps `bundler.rs` and the runtime
  embedding paths to make sure none are missed.
- **Showcase shrinkage threshold (Requirement 21).** Spec proposes
  30%. Plan can tune up or down based on what an initial measurement
  reports.
- **`bundle_unminified` exposure mechanism (Requirement 18).** Plan
  picks between a `#[cfg(any(test, feature = "test-internals"))]`
  function, a `pub(crate)` in a test-shared module, or simply
  copying the un-minified output before minify and returning both
  from a richer test-only API.
- **Legal-comment preservation default in grass compressed mode.**
  Plan checks grass's behavior and adds the CSS-side comment
  assertion accordingly.
- **`sourcesContent` in JS map.** Cheap to include and makes the
  map standalone; only add if the chosen minifier API surfaces it
  without extra plumbing.
