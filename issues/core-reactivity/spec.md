# Spec: Core Reactivity

## Problem Statement

Every other part of the framework — templates, components, state machines, the router — depends on reactive primitives. Phase 1 establishes the foundational layer that all subsequent phases build on. Without `signal`, `computed`, and `effect` working correctly, nothing else can be implemented.

## Background

The zero framework is distributed as a single Rust CLI binary. The JavaScript runtime (the code that ships to users' browsers) lives in this repo as plain `.js` files with JSDoc type annotations. When a developer runs `zero new my-app`, the Rust binary extracts these runtime files into the new project directory. There is no TypeScript compilation step for the runtime — plain JS with JSDoc avoids a bootstrap problem and keeps the files directly readable and debuggable.

The reactivity system follows the **global observer stack** pattern used by SolidJS, Preact Signals, and Vue. When a `computed` or `effect` runs its function, it pushes itself onto a global stack. Every `.val` read on a signal checks that stack and registers the current observer as a subscriber. When the function finishes, it pops off the stack. This is how "automatic dependency tracking" is achieved with no dependency arrays.

Components are plain functions — they are not a Phase 1 concern. The ownership scope system exists to support future component cleanup but is purely internal in Phase 1; there is no user-facing `createScope()` function.

## Requirements

### signal(initialValue)

- `signal(v)` creates a reactive container with an initial value
- `.val` reads the current value; if a computed or effect is currently executing, the signal registers it as a subscriber
- `.set(newVal)` writes a new value and synchronously notifies all subscribers
- `.update(fn)` calls `fn` with the current value and passes the return value to `.set()`
- Subscribers are not notified if the new value is strictly equal (`===`) to the old value

### computed(fn)

- `computed(fn)` creates a derived value that lazily re-evaluates when any dependency changes
- `.val` returns the current derived value; reading it from inside another `computed` or `effect` registers it as a dependency of that observer
- Has no `.set()` or `.update()` — it is read-only
- Dependencies are discovered automatically on each execution via the global observer stack; the dependency set is re-computed on each re-evaluation (supporting conditional branches)
- Does not re-run `fn` until `.val` is read after a dependency has changed (lazy evaluation)

### effect(fn)

- `effect(fn)` runs `fn` immediately and re-runs it whenever any signal or computed read inside `fn` changes
- `fn` may return a cleanup function; the cleanup runs before each re-execution and when the effect is disposed
- `effect()` returns a `stop` function; calling it disposes the effect and runs its cleanup
- Dependencies are tracked the same way as `computed` — via the global observer stack

### Ownership Scope (internal)

- An internal scope object tracks all signals, effects, and child scopes created within it
- `createScope()` creates a new scope and sets it as the active scope
- `scope.dispose()` runs cleanup on all registered effects and recursively disposes child scopes
- Scopes can be nested; child scopes are registered with their parent automatically
- The active scope is stored on the same global context as the observer stack

### File layout

- The runtime file lives at `runtime/reactivity.js` in this repo
- It exports `signal`, `computed`, and `effect` as named exports
- Internal scope functions are not exported

## Constraints

- Plain JavaScript only — no TypeScript syntax, no build step for the runtime
- JSDoc annotations on all public exports for editor autocomplete
- No external dependencies
- The global observer stack must be a module-level variable (not `globalThis`) to avoid conflicts if multiple copies of the runtime are loaded
- Subscribers must be notified synchronously on `.set()` — no batching or scheduling in Phase 1 (that can be added later if needed)

## Out of Scope

- The `html` tagged template and DOM rendering (Phase 2)
- The `each()`, `ref()`, and `inject()` functions (Phase 2+)
- The `App` class and router (Phase 3)
- The `machine()` state machine primitive (Phase 4)
- The test runner and `z/test` API (Phase 5)
- The Rust CLI commands (Phase 6)
- User-facing scope API — `createScope()` is internal only
- Batched/scheduled updates (notify synchronously for now)
- The `zero new` extraction mechanism — Phase 1 only produces the JS file; wiring it into the Rust binary is a Phase 6 concern

## Open Questions

- Should computed values use push or pull invalidation? (Recommendation: mark dirty on dependency change, re-evaluate lazily on next `.val` read — the classic pull model)
- Is there a maximum observer stack depth to guard against circular computed dependencies, or is that left to the developer?
- Should `.set()` on a signal that is read inside its own effect cause an infinite loop guard, or is that out of scope for Phase 1?
