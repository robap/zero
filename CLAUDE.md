# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Run all Rust tests (workspace)
cargo test --workspace

# Run tests for a single crate
cargo test -p zero-bundler

# Run all JS runtime tests (framework-internal, unchanged)
node --test runtime/*.test.js

# Run a single JS test file
node --test runtime/app.test.js

# Run JS tests matching a name pattern
node --test --test-name-pattern="querySelector" runtime/dom-shim.test.js

# Build / install the CLI
cargo build --workspace --release
cargo install --path crates/zero --locked
```

### Quality

```bash
# Generate HTML coverage report (Rust, workspace-wide)
cargo llvm-cov --workspace --html

# Per-module summary table
cargo llvm-cov --workspace --summary-only
```

Functions over ~80 lines and modules under ~70% line coverage are signals to
refactor, not hard gates. When touching a module, glance at
`cargo llvm-cov --workspace --summary-only` and notice outliers. Don't chase a
number — fix the structure if a function feels too long or a path feels
under-tested.

## Code style

### Rust
- Keep functions less than ~80 lines.
- 
### Javascript/Typescript
- `.ts` is the canonical authoring extension for user projects (the scaffold emits `src/app.ts`, `src/routes/home.ts`, etc.). `.js` continues to work everywhere — the dev server transpiles `.ts` requests on the fly via swc, the bundler walks mixed `.ts` / `.js` graphs, and the test runner discovers both extensions. The `node --test runtime/*.test.js` command above (framework-internal tests) is unchanged; user-level tests run with `zero test`.
- All JavaScript files must be fully JSDoc-annotated. Every exported function, class, and class method needs `@param`, `@returns`, and where applicable `@template`. Module-level variables need `@type`. Use `@internal` for exports that are not part of the public API. Use `@private` for private class methods.
- Keep functions less than ~80 lines.
- Use strong types. Avoid `any`
