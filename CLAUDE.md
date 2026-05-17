# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Run all tests
node --test runtime/*.test.js

# Run a single test file
node --test runtime/app.test.js

# Run tests matching a name pattern
node --test --test-name-pattern="querySelector" runtime/dom-shim.test.js
```

### Coverage

```bash
# Generate HTML coverage report (Rust)
cargo llvm-cov --html

# Per-module summary table (Rust)
cargo llvm-cov --summary-only
```

## Code style

### Rust
- Keep functions less than ~80 lines.
- 
### Javascript/Typescript
- `.ts` is the canonical authoring extension for user projects (the scaffold emits `src/app.ts`, `src/routes/home.ts`, etc.). `.js` continues to work everywhere — the dev server transpiles `.ts` requests on the fly via swc, the bundler walks mixed `.ts` / `.js` graphs, and the test runner discovers both extensions. The `node --test runtime/*.test.js` command above (framework-internal tests) is unchanged; user-level tests run with `zero test`.
- All JavaScript files must be fully JSDoc-annotated. Every exported function, class, and class method needs `@param`, `@returns`, and where applicable `@template`. Module-level variables need `@type`. Use `@internal` for exports that are not part of the public API. Use `@private` for private class methods.
- Keep functions less than ~80 lines.
- Use strong types. Avoid `any`
