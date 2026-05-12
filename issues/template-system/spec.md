# Spec: Template System

## Problem Statement

Phase 1 gave us reactive primitives (`signal`, `computed`, `effect`). Phase 2 connects those primitives to the DOM. The `html` tagged template is the authoring surface for all UI in zero — every component returns one. Without the template system, nothing can render, and no subsequent phase can make meaningful progress.

## Background

Phase 1 is complete in `runtime/reactivity.js`. It exports `signal`, `computed`, `effect`, and the internal `_createScope`. The scope system (`createScope`, `scope.run()`, `scope.dispose()`) is the mechanism by which component cleanup works — Phase 2 must create effects that are owned by whatever scope is active at commit time.

Tagged template literals provide a key property: the `strings` array is the **same object reference** for every call to the same template site in source code. This enables template caching — parse the HTML structure once, clone it on every instantiation.

The template system targets the real browser DOM. For testing in Node without npm or jsdom, a minimal DOM shim will be built as part of this phase. It implements only the DOM APIs the template system uses (~150–200 lines). This shim is the seed of the Phase 5 test runner DOM.

Event bindings (`@click`, `@submit.prevent`, etc.) appear in the **static** string parts of the tagged template, not the values array. Parsing scans the strings array directly to determine the context of each `${}` placeholder and identify event attribute names and their modifiers.

## Requirements

### `html` tagged template

- `html` is a tagged template function: `html(strings, ...values) → TemplateResult`
- Returns a `TemplateResult`: `{ _template: Template, _values: any[] }`
- The `Template` (parsed structure + parts descriptor) is **cached** by the `strings` array reference — parsing runs once per unique template site, not once per call
- `_values` is a fresh array per call (the dynamic parts for that instance)

### Template parsing

Scan the strings array to classify the context of each `${}` placeholder (by index into the values array). Context is determined by inspecting the static string immediately before the placeholder:

- **Text node**: placeholder is in text content — the preceding string ends at a `>` boundary or in bare text
- **Attribute value**: preceding string ends with `attrname="` or `attrname='` (standard attribute)
- **Event handler**: attribute name starts with `@` (e.g., `@click="`, `@submit.prevent="`)
- **Ref**: attribute name is `ref`

From the strings, build:
1. A cloneable `DocumentFragment` (the template element) with **placeholder anchors** marking where dynamic values go (e.g., comment nodes for text/node positions, sentinel attribute values for attribute positions)
2. A `Part[]` array — one entry per `${}` — describing where and how to apply each dynamic value at commit time

### Part types

```js
// Text content slot — update a Text node
{ type: 'text', node: Text }

// Attribute value slot — set/remove an attribute
{ type: 'attr', el: Element, name: string }

// Event handler slot — addEventListener on the element
{ type: 'event', el: Element, event: string, modifiers: string[] }

// Ref slot — assign el to ref.el after commit
{ type: 'ref', el: Element }

// Node slot — insert TemplateResult, array, or reactive block
// anchor is a Comment node that acts as a stable insertion point
{ type: 'node', anchor: Comment }
```

### `commit(templateResult, container)`

Commits a `TemplateResult` to a DOM container:

1. Clone the template's `DocumentFragment`
2. For each `Part`, wire up the corresponding value from `_values`:
   - `string | number` → set text or attribute directly
   - `boolean` → on `attr` parts: `true` sets the attribute (to `""`), `false` removes it
   - `null | undefined` → clear text / remove attribute / remove child nodes
   - `Signal` → create an `effect()` that reads `.val` and updates the DOM; the effect auto-subscribes and re-runs on change
   - `() => TemplateValue` (reactive block) → create an `effect()` that calls the function, diffing the return type to decide whether to update text, swap a TemplateResult, or clear
   - `TemplateResult` → recursively `commit()` into the anchor
   - `Array` → commit each item in order into the anchor
3. All `effect()` calls during commit run within the **currently active scope** (from `_activeScope` in `reactivity.js`). This means the caller is responsible for wrapping `commit()` in a scope's `run()` if cleanup is needed.
4. Append committed DOM to the container
5. Set `el` on any `ref` parts

### Dynamic value type table

| Value type | `text` part | `attr` part | `node` part |
|---|---|---|---|
| `string` / `number` | set `nodeValue` | `setAttribute` | replace with Text node |
| `boolean` | — | `true` → set attr, `false` → remove | — |
| `null` / `undefined` | clear to `""` | `removeAttribute` | remove child nodes |
| `Signal` | wrap in effect | wrap in effect | wrap in effect |
| `() => TemplateValue` | wrap in effect | wrap in effect | wrap in effect |
| `TemplateResult` | — | — | recursive commit |
| `Array` | — | — | commit each item |

### Event binding

- `@eventname=${handler}` → `el.addEventListener(eventname, wrappedHandler)` at commit time
- The listener is registered with the element and removed when the owning scope is disposed (via the scope's cleanup mechanism)
- Modifier parsing happens at **template parse time** (from the static strings), not at commit time
- Supported modifiers (dot-separated, e.g. `@submit.prevent.stop`):
  - `.prevent` → `e.preventDefault()`
  - `.stop` → `e.stopPropagation()`
  - `.once` → listener removed after first fire
  - Key filters: `.enter`, `.escape`, `.space`, `.tab`, `.up`, `.down`, `.left`, `.right` → only invoke handler if `e.key` matches
  - `.throttle` → wrap handler to limit call rate
  - `.debounce` → wrap handler to delay until quiescent
- Multiple modifiers combine: `@keydown.enter.prevent` calls `preventDefault` and filters to Enter key

### `each(signalOfArray, renderFn)`

Unkeyed list rendering:

- `each(sig, fn)` returns a value embeddable in a template (treated as a `node` part)
- When the array signal changes: dispose all current item scopes, then re-render all items by calling `fn(item, index)` for each element
- Each item gets its own **child scope** created via `createScope().run(fn)`, so its effects are cleaned up independently
- `fn` receives `(item, index)` and returns a `TemplateResult`
- On initial commit, renders all current items; on signal change, tears down and re-renders the full list

### `ref()`

- `ref()` returns a mutable container: `{ el: null }`
- When `ref=${myRef}` is processed during commit, `myRef.el` is set to the committed element
- `myRef.el` is set to `null` when the owning scope is disposed

### Minimal DOM shim (`runtime/dom-shim.js`)

A new file implementing the DOM surface the template system uses — enough to run tests in Node with `node:test` and no npm:

- `document.createElement(tag)` — returns an Element
- `document.createTextNode(text)` — returns a Text node
- `document.createComment(data)` — returns a Comment node
- `document.createDocumentFragment()` — returns a DocumentFragment
- `Element`: `setAttribute`, `removeAttribute`, `getAttribute`, `addEventListener`, `removeEventListener`, `appendChild`, `insertBefore`, `removeChild`, `cloneNode(deep)`, `childNodes`, `parentNode`, `nodeType`, `nodeName`
- `Text`: `nodeValue`, `cloneNode`, `nodeType`, `parentNode`
- `Comment`: `data`, `cloneNode`, `nodeType`, `parentNode`
- `DocumentFragment`: `appendChild`, `childNodes`, `cloneNode(deep)`
- Tests `import { document } from './dom-shim.js'` and use it directly; the template system reads `document` from the global scope (tests set `globalThis.document` to the shim before importing)

### File layout

```
runtime/
  reactivity.js         # Phase 1 — complete
  template.js           # Phase 2 — html, commit, each, ref
  dom-shim.js           # Phase 2 — minimal DOM for Node testing
  reactivity.test.js    # Phase 1 tests — complete
  template.test.js      # Phase 2 tests
```

## Constraints

- No npm dependencies — plain JS with JSDoc, same as Phase 1
- Browser runtime: targets real `document` in production; shim replaces `globalThis.document` in tests
- Template cache key is the `strings` array object reference (same reference = same template site)
- Effects created during `commit()` run in whatever scope is active at call time — `commit()` does not create its own scope
- Event modifiers are parsed at template-parse time (from static strings), not at commit time
- `each()` is unkeyed — no key-based DOM reuse or reordering

## Out of Scope

- Keyed reconciliation in `each()` — full key-based diffing and DOM node reuse deferred to a later phase
- Production build-time template pre-compilation — `html` is runtime-only in Phase 2; compile-time optimization is a Phase 6 concern
- `inject()` — requires App context, Phase 3
- The `App` class, router, state machines — Phases 3 and 4
- The full Phase 5 test runner DOM (~500 lines) — Phase 2 builds only the subset needed for template testing
- `<style scoped>` — explicitly removed from the framework spec

## Open Questions

- When a `node` part transitions between types across reactive re-evaluations (e.g., was a `TemplateResult`, now `null`, now an `Array`), the commit logic must clear and replace the content between the anchor comment and its next sibling. The exact bookkeeping for this needs to be worked out in the plan — it's the trickiest part of the node-slot update path.
- `each()` returns something to embed in a `node` part. Should it return a `TemplateResult`-like object with a special marker, or a plain function (`() => ...`) that the commit logic handles as a reactive block? The plan phase should decide.
