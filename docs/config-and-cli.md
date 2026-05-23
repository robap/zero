---
title: Config and CLI
nav_order: 11
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

### `zero mutate [pattern]`

Mutation testing. Runs the baseline test suite, then iterates
over mutation sites in `src/`, re-running the affected tests
with each mutation applied.

| Flag                          | Description                                            |
|-------------------------------|--------------------------------------------------------|
| `--operators <csv>`           | Restrict to operator families (e.g. `arith,bool`).     |
| `--max-mutants <n>`           | Cap total mutants generated.                           |
| `--threads <n>`               | Run mutants in parallel.                               |
| `-q, --quiet`                 | Suppress per-mutant lines; print summary only.         |

Operator ids accepted by `--operators`: `arith`, `cmp`, `bool`,
`cond_neg`, `boundary`, `lit_bool`, `lit_num`, `lit_str`.

#### Reading `Generated: 0`

`Generated: 0 mutants` on a `--operators` run can mean three things:

- **No matches in `src/`.** The operator is implemented, but no AST
  node in the codebase matched its swap rules.
- **All matches on uncovered lines.** Sites were found but no
  baseline test exercises those lines; the coverage filter drops them.
- **All matches equivalent.** Sites were found and reached, but the
  mutated JS was byte-identical to the baseline (rare).

The per-operator breakdown printed under the headline distinguishes
the three. A row like `arith: matched 12, executed 0 (...), unreachable
12, equivalent 0` says "12 arith sites exist, every one is on a line
no test reaches" — write a test that calls into that code.

Writes `mutation/mutation.json` (schema version 1) with structured
results, including a per-operator breakdown under `operators`. Exit
code is non-zero if any mutant survived or errored.

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

Serves the production build locally. See
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
