---
title: Config and CLI
nav_order: 12
---

# Config and CLI

The CLI is the framework. Every operation — scaffolding, dev
serving, building, testing, linting, formatting, deploying — is
a subcommand of `zero`. This page is the reference.

## `zero.toml` schema

`zero.toml` lives at the root of your project (the directory you
run `zero` commands from). All keys are validated; unknown keys
are rejected with a clear error.

```toml
[project]
root = "web"                         # required

[dev]
port = 3000                          # default 3000
proxy = "http://localhost:8080"      # optional
sourcemap = true                     # default true (inline)

[build]
out = "dist"                         # default "dist"
sourcemap = false                    # default false (external .map files)
```

### `[project]`

| Key    | Type   | Default | Description                                                 |
|--------|--------|---------|-------------------------------------------------------------|
| `root` | string | (required) | The directory containing your app (relative; no `..`, no absolute paths). |

### `[dev]`

| Key         | Type    | Default  | Description                                                                        |
|-------------|---------|----------|------------------------------------------------------------------------------------|
| `port`      | integer | `3000`   | TCP port for the dev server. 1–65535.                                              |
| `proxy`     | string  | (none)   | Backend URL to forward unknown requests to. Must use `http://` (no HTTPS proxying). |
| `sourcemap` | boolean | `true`   | Emit inline source maps in dev `.ts` responses.                                    |

### `[build]`

| Key         | Type    | Default  | Description                                                       |
|-------------|---------|----------|-------------------------------------------------------------------|
| `out`       | string  | `"dist"` | Output directory (relative; no `..`, no absolute paths).          |
| `sourcemap` | boolean | `false`  | Emit external `.map` files alongside the bundle.                  |

### Validation rules

- `root` and `out` must be **relative** paths with no `..`
  segments and no absolute components. Anything else is
  rejected.
- `port` must be in the range 1–65535.
- `proxy`, if set, must parse as an `http://` URL. `https://` is
  rejected (no TLS termination at the dev proxy).
- Unknown keys at any nesting level cause a clear error. This
  catches typos like `[devv]` early.

## CLI commands

```
zero — the zero-dependency web framework

Usage: zero <command> [options]

Commands:
  zero init                   Scaffold a new project (interactive)
  zero update                 Refresh framework files in .zero/
  zero dev                    Start dev server
  zero build                  Production build
  zero test [pattern]         Run tests
  zero mutate [pattern]       Run mutation testing
  zero fmt                    Format all source files
  zero lint                   Lint all source files
  zero gen component <name>   Generate a component
  zero gen route <path>       Generate a route
  zero preview                Serve the production build locally
  zero upgrade                Self-update the CLI
```

### `zero init`

Scaffolds a new project. Runs an interactive wizard the first
time; reads an existing `zero.toml` if present.

```sh
zero init                 # interactive wizard
zero init --yes           # skip wizard (requires zero.toml present)
```

Refuses to overwrite a non-empty project root.

### `zero update`

Refreshes the framework-managed `.zero/` directory from the
embedded CLI. Diffs the on-disk tree against what the current
CLI version would emit and prints an Add / Update / Remove
plan.

```sh
zero update               # print plan, prompt [Y/n/i], then apply
zero update --yes         # apply all without prompting
```

At the prompt: `Y` applies everything, `n` aborts, `i` enters
interactive mode (per-operation y/n followed by a final
confirm). If `.zero/` is already up to date, exits 0 with a
single-line message. Declined operations are not an error —
exit code is 0 regardless. Never writes outside `.zero/`.

### `zero dev`

Starts the dev server. Reads `zero.toml` from the current
directory; clear error if missing.

| Flag                  | Description                                |
|-----------------------|--------------------------------------------|
| `-p, --port <n>`      | Override `[dev] port`.                     |
| `--host <addr>`       | Bind address (default `localhost`).        |
| `-o, --open`          | Open the URL in a browser on start.        |
| `--https`             | Enable self-signed TLS (development).      |

File watching with full-page reload is always on. There is no
HMR yet — module-state preservation is roadmapped.

### `zero build`

Bundles the app for production. See
[Building and Deploying](./building-and-deploying.html) for the
output shape.

| Flag                  | Description                                          |
|-----------------------|------------------------------------------------------|
| `-o, --out <dir>`     | Override `[build] out`.                              |
| `--analyze`           | Print a bundle-size breakdown.                       |
| `--sourcemap`         | Emit external `.map` files (overrides config).       |
| `--target <env>`      | `static` (default), `server`, `worker`.              |

Production output is always minified (both JS and CSS). The dev
server is unaffected.

Exit code: `0` on success, non-zero on any build failure.

### `zero test [pattern]`

Runs the test suite. See [Testing](./testing.html) for the API.

| Flag                    | Description                                           |
|-------------------------|-------------------------------------------------------|
| `--watch`               | Re-run on file change.                                |
| `--coverage`            | Write coverage data to `coverage/`.                   |
| `--update-snapshots`    | Accept current outputs as new snapshot baselines.     |

`pattern` matches test paths and `describe` / `it` names. Exit
code is non-zero on any failure.

`zero test` runs with or without a `zero.toml`: absent one, it
discovers tests from the current directory (skipping `dist/` and
`build/`). A file argument is resolved relative to your current
directory first, then the project root — so `zero test web/src/app.test.ts`
(the path as you see it) and `zero test src/app.test.ts` (project-root-relative)
both work. See [Testing](./testing.html) for details.

The `--coverage` line metric is **per executable statement**: each
statement carries its own counter, so the reported line count reflects
which statements actually ran rather than only function entries and
top-level declarations.

Setting the `ZERO_TEST_TIMING` environment variable (to any non-empty
value) prints a per-phase timing breakdown to **stderr** after the run —
discovery, context build, DOM-shim eval, runtime eval, transpile, and test
execution, with cumulative milliseconds and call counts. It is a diagnostic
for investigating slow suites; it is off by default and does not change test
output, results, or exit code.

### `zero mutate [pattern]`

Mutation testing. Runs the baseline test suite, then iterates
over mutation sites in `src/`, re-running the affected tests
with each mutation applied.

| Flag                          | Description                                            |
|-------------------------------|--------------------------------------------------------|
| `--operators <csv>`           | Restrict to operator families (e.g. `arith,bool`).     |
| `--max-mutants <n>`           | Cap total mutants generated.                           |
| `--threads <n>`               | Run mutants in parallel. Defaults to `min(cores, 8)` — parallel by default; the cap keeps headroom on bigger boxes for IDE / build processes. Pass `1` for sequential. |
| `-q, --quiet`                 | Suppress per-mutant lines; print summary only.         |
| `--no-cache`                  | Ignore the incremental cache: re-run every mutant and rewrite `mutation/cache.json`. |
| `--timeout <dur>`             | Per-mutant execution budget. Accepts `10s`, `500ms`, or a bare integer (seconds). Default: `max(2s, baseline × 5)`. |

Operator ids accepted by `--operators`: `arith`, `cmp`, `bool`,
`cond_neg`, `boundary`, `lit_bool`, `lit_num`, `lit_str`.

#### Timeouts

Mutation testing deliberately runs *hostile* variants of your code, and some
of them don't terminate — inverting a loop's progress step (`i += 1` →
`i -= 1`) or growing a value the loop waits to shrink turns a bounded loop
into an infinite one. Rather than hang the run, `zero mutate` gives every
mutant a per-run budget and aborts any mutant that exceeds it.

- **Budget.** Derived once per run from the measured baseline suite
  wall-time: `max(2s, baseline × 5)`. This adapts to slow suites and slow
  machines. Override it with `--timeout <dur>` (`10s`, `500ms`, or a bare
  integer meaning seconds). The flag composes with `--threads`,
  `--operators`, `--max-mutants`, `--no-cache`, and `--quiet`.
- **Classification.** A mutant that does not terminate within the budget is
  **killed** — an infinite loop is a real, test-detectable divergence, not a
  survivor — and is additionally reported as a `timed-out` sub-count so you
  can see how many kills were timeouts, e.g.
  `Killed: 9 (incl. 2 timed-out)`. The per-operator breakdown carries the
  same sub-count.
- **Two guards.** In-process runs (and the unit tests) rely on a QuickJS
  engine deadline that lets the JS loop self-abort precisely. The CLI's
  subprocess workers arm that same deadline *and* the parent hard-waits
  `budget + 2s` and kills a child that wedges for any reason the engine
  deadline can't observe, so a run can never stall past `budget + grace`.

`mutation.json` is unchanged by this — still **schema version 2**. The
`timed_out` counts (per-operator and in `totals`) are additive, optional
fields that older readers ignore; the frozen `killed / survived / errored`
buckets and their meanings are the same.

#### Incremental runs

Runs are incremental by default. A source file's mutant verdicts are
reused from the previous run when its full closure is byte-identical:
the file itself, every test that exercises it, and every module those
tests load. Any doubt — an edited file, a new test, an unreadable
member — resolves to re-execution. Reuse is reported on the summary's
`Generated:` line, e.g.
`Generated: 12 mutants across 3 files (41 reused from cache across 9 files)`.

When *nothing* changed at all — same test files, same source files,
every recorded file hashing identically — the run skips the baseline
too and replays the previous result, printing:

```
zero mutate: no changes since last run — replaying cached result (baseline skipped)
```

The cache lives in `mutation/cache.json`. It is internal,
version-keyed (a different `zero` binary invalidates it), gitignored
in scaffolded projects, and always safe to delete — the next run is
simply a full run that rewrites it. `--operators` and `--max-mutants`
runs neither read nor write it; `[pattern]` runs reuse and refresh
only the files they cover. One caveat: a flaky suite freezes whichever
verdict was observed when the entry was written — `--no-cache` re-runs
everything from scratch.

`mutation/mutation.json` is unchanged by all of this (still schema
version 2): reused verdicts are folded in, so its totals always
describe the whole tree.

#### Reading `Generated: 0`

`Generated: 0 mutants` on a `--operators` run can mean four things:

- **No matches in `src/`.** The operator is implemented, but no AST
  node in the codebase matched its swap rules.
- **All matches unreachable.** Sites were found, but no baseline test
  executed the *enclosing statement*, so the coverage filter drops them.
  Reachability is judged on the site's enclosing statement (each
  statement carries a coverage counter), not on the site's own line — so
  fully-tested code is never misfiled here. A non-empty `unreachable`
  count means real code that no baseline test runs.
- **All matches byte-equivalent.** Sites were found and reached, but
  the mutated JS was byte-identical to the baseline (rare).
- **All matches statically-equivalent.** The visitor proved every
  matched literal no-op by AST shape (see below).

The per-operator breakdown printed under the headline distinguishes
the four. A row like
`arith: matched 12, executed 0 (...), unreachable 12, equivalent-byte 0,
equivalent-static 0`
says "12 arith sites exist, every one inside a statement no test
executes" — write a test that calls into that code.

#### Reading `equivalent-static`

`equivalent-static` counts mutants the visitor proved no-op by AST
shape before they reach the worker queue: members of a module-level
`const X = [...] as const` array that's only referenced in type
position (`typeof X`, `(typeof X)[number]`), and property literals
inside a module-level `signal({...})` / `computed({...})` initializer
that every later `.set(...)` overwrites in source order. These never
pad `Survived` and never burn worker cycles.

The headline `Skipped` row shows
`[unreachable: N, equivalent-byte: M, equivalent-static: K]` so the
three sub-buckets stay distinct.

Writes `mutation/mutation.json` (schema version 2) with structured
results, including a per-operator breakdown under `operators` (each
operator object has `equivalent_byte` and `equivalent_static` fields)
and split totals (`skipped_unreachable`, `skipped_equivalent_byte`,
`skipped_equivalent_static`). Exit code is non-zero if any mutant
survived or errored. The per-statement coverage change does not alter
this format — `mutation.json` is still `schema_version: 2`.

### Type-checking

The CLI does not ship a type-checker. The scaffold's `tsconfig.json`
drives editor type-checking; for an editor-independent CLI gate,
install TypeScript ≥ 5.0 either per-project
(`npm i -D typescript` → `npx tsc --noEmit`) or globally
(`npm i -g typescript` → `tsc --noEmit`). TypeScript < 5.0 will fail
against the shipped tsconfig because of `allowImportingTsExtensions` —
`tsc --version` confirms the installed release.

### `zero fmt`

Built-in formatter. Opinionated defaults; no config. Exits
non-zero if any file would change (CI-friendly check). Pass
`--write` to apply.

### `zero lint`

Built-in linter for SCSS and JS/TS. See [Linting](./linting.html)
for the rule list and posture. Exits non-zero on any violation.

### `zero gen`

Generators that emit canonical scaffolds for new files.

```sh
zero gen component Button        # → src/components/Button.ts
zero gen component ui/Card       # → src/components/ui/Card.ts
zero gen route /about            # → src/routes/about.ts
zero gen route /users/:id        # → src/routes/users/[id].ts
```

### `zero preview`

Builds, then serves the production output locally. See
[Building and Deploying § zero preview](./building-and-deploying.html#zero-preview).

### `zero upgrade`

Self-updates the CLI to the latest published version.

## Global flags

These work on every subcommand:

| Flag             | Description                          |
|------------------|--------------------------------------|
| `-q, --quiet`    | Suppress non-error output.           |
| `-v, --verbose`  | Verbose logging.                     |
| `--no-color`     | Disable colored output.              |
| `--version`      | Print version and exit.              |
| `-h, --help`     | Show help and exit.                  |
