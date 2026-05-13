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

## Code style

All JavaScript files must be fully JSDoc-annotated. Every exported function, class, and class method needs `@param`, `@returns`, and where applicable `@template`. Module-level variables need `@type`. Use `@internal` for exports that are not part of the public API. Use `@private` for private class methods.

## Key constraint

**Static HTML attributes are invisible to the template system.** `html\`<a href="/about">\`` produces an anchor with no `href`. Use `html\`<a href=${'/about'}>\`` to set attributes.
