# Plan: AGENTS.md in the `zero init` scaffold

## Summary

Add a single new template file `src/scaffold/AGENTS.md` containing a comprehensive, agent-and-developer-facing reference for the zero framework's currently-implemented public API, then wire it into `src/scaffold.rs` so `zero init` writes it to the project root alongside the existing scaffold files. The doc is authored against the verified runtime exports — not the framework spec — and includes a worked example for every documented feature. Splitting the work into "author the doc" and "wire it in" lets each step land independently green.

## Prerequisites

The spec's "Out of Scope" / "Background" sections list several features as deferred that are actually implemented in `runtime/`. Per the spec's binding rule — *"When the framework spec and the runtime disagree, the runtime is the source of truth"* — this plan follows the runtime. Concretely:

- **Will be documented (implemented in runtime, despite being on the spec's exclusion list):** `app.use()` middleware, `app.loading()`, `app.error()`, route opts `guard` / `load` / `children` (nested routes) / `meta` / route-level `loading` / route-level `error`.
- **Will NOT be documented (genuinely not implemented):** `group()` (not exported), route transitions (no `transition` opt accepted), `zero gen` / `zero check` / `zero fmt` / `zero lint` (no CLI subcommands), state machines (no exports), `expect().toMatchSnapshot()` (exported but its body throws *"snapshot testing is not in this slice yet"*), test `--watch` / `--coverage`.

No other prerequisites. No prior issues block this work.

## Steps

- [x] **Step 1: Author `src/scaffold/AGENTS.md` with the full reference content**
- [x] **Step 2: Wire `AGENTS.md` into the scaffold writer and tests**

---

## Step Details

### Step 1: Author `src/scaffold/AGENTS.md` with the full reference content

**Goal:** Produce the entire AGENTS.md body as a static template file. Doing this before any Rust plumbing keeps the (large) content change isolated from the (small) wiring change. The file is unreferenced after this step, so the Rust build and all existing tests pass unchanged.

**Files:**
- Create `src/scaffold/AGENTS.md` (new file).

**Changes:**

Author a single markdown file. Target length 400–800 lines. Sections in the order below — order is chosen by frequency-of-lookup, since agents grep.

1. **`# Zero — Agent & Developer Reference`** (H1). One short paragraph: zero is a zero-dependency frontend framework distributed as the `zero` CLI. This file is the authoritative API reference for what the framework currently supports. Examples are runnable against the version that scaffolded the project.

2. **`## Quick start`** (one screenful). Show the three commands a fresh user runs:
   - `zero dev` — start the dev server (file watching + full-page reload).
   - `zero test` — run all `*.test.js` files under `src/`.
   - `zero build` — produce a production build (output folder configured in `zero.toml`).
   List the generated project layout (`index.html`, `src/app.js`, `src/routes/home.js`, `src/routes/home.test.js`, `styles/app.css`, `AGENTS.md`) with one line each describing the file's role.

3. **`## Imports`** (one block per import path). Establish notation up front:
   ```js
   import { App, signal, computed, effect, html, each, ref, inject,
            navigate, back, forward, route } from "zero";
   import { describe, it, expect, beforeEach, afterEach, beforeAll, afterAll,
            render, find, findAll, text, fire, cleanup } from "zero/test";
   ```
   State the rule: any name starting with `_` or `__` is internal — do not import it.

4. **`## Components`**. Define a component: a plain function that returns a `TemplateResult`. Worked example mirroring `src/routes/home.js`. Subsections:
   - **Props** — props are plain objects; if a parent passes a signal, the signal reads stay reactive when used in the template.
   - **Children / slots** — pass any `TemplateResult` as a regular prop.
   - **Events** — `@event` attribute syntax; show `@click`, `@input`, `@submit`. List supported modifiers exactly as implemented in `runtime/template.js`:
     - `.prevent`, `.stop`, `.once`
     - Key filters (keyboard events): `.enter`, `.escape`, `.space`, `.tab`, `.up`, `.down`, `.left`, `.right`
     - `.throttle` (fixed 100ms), `.debounce` (fixed 100ms) — call out that the interval is currently not configurable.
     Combine examples: `@submit.prevent`, `@keydown.enter`, `@click.once`.
   - **Reactive blocks** — `${() => …}` inside templates re-evaluates when its dependencies change. This is the conditional-render and computed-text idiom. Cross-reference `## Reactivity`.
   - **Conditional rendering** — show a reactive-block example with an `if/else` that returns different templates.
   - **List rendering with `each`** — signature `each(signalOfArray, (item, index) => TemplateResult)`. Show a list example; note that `each` watches the signal and re-renders the whole list when it changes (no keyed reconciliation yet — describe the current behavior honestly).
   - **Refs** — `ref()` returns `{ el }`. Pass via `ref=${myRef}`; access `myRef.el` inside an `effect`. Worked example for autofocus.

5. **`## Reactivity`**. Three primitives, each with signature, example, and one-line semantics:
   - `signal(initialValue)` → `{ val, set, update }`. `val` is a getter; `.set(newVal)` is a no-op if `===` to current; `.update(fn)` is `set(fn(current))`.
   - `computed(fn)` → `{ val }` (read-only). Lazy: re-evaluates on the next `.val` read after a dependency changes.
   - `effect(fn)` → `stop()`. Runs immediately; re-runs when any dependency changes; if `fn` returns a function it is called as cleanup before each re-run and on `stop()`.
   - Note: dependencies are auto-tracked — no dependency arrays. Effects created inside a component are torn down when the component's scope is disposed.

6. **`## App configuration`**. Centered on `new App()` with all currently-implemented methods. Each method gets signature + one-line semantics + brief example. Cover, in builder order:
   - `state(key, value)` — register a value (typically a signal) for later `inject(key)`. Throws on duplicate key or post-`run()`.
   - `use(mw)` — register middleware. Signature: `mw({ route, state, redirect }) => void | Promise<void>`. Multiple `use()` calls form an ordered chain run once per navigation, before guards/loads. Show a redirect example.
   - `route(pattern, loaderOrComponent, opts?)` — register a route. Pattern supports exact paths, `:name` segments, and the bare `*` wildcard. `loaderOrComponent` may be an eager component (returns TemplateResult sync) or a lazy loader (returns a Promise of a module whose `.default` is the component). Document all currently-accepted `opts`:
     - `guard({ params, query, state, route, redirect })` — return `false` to abort. Show a logged-in-only example.
     - `load({ params, query, state, fetch, route })` — async data hydration; result is available to the route component via `inject` or by reading state mutated inside `load`. (Note: current runtime does not pass `load`'s return value as a prop — components read from `state` / `inject`. Document what's actually wired.)
     - `meta` — object merged root-to-leaf across the chain.
     - `loading` — per-route loading component override.
     - `error` — per-route error component override.
     - `children` — array of nested route descriptors, each `{ path, load, ...sameOpts }`. Required `load` (the child's `load` field doubles as the component loader, matching runtime behavior). Show one nested example; note that nested children render into a parent `outlet` signal passed into the parent component as a prop.
   - `layout(component)` — set a layout component that wraps every route. Receives `{ outlet }`; render `${props.outlet}` to mount the matched route.
   - `loading(component)` — global loading UI shown when a navigation takes longer than 150ms.
   - `error(component)` — global error UI; receives `{ error, retry }`.
   - `run(selector)` — mount and start; document side effects (mounts to `selector`, attaches `popstate`, attaches document `click` listener, marks itself as the running app for `inject` / `navigate` / `route()`).
   - `match(input)` — test helper; matches a path-and-query against the route table without rendering.

7. **`## Routes`**. Cross-references `## App configuration` for `route()` but focuses on what a route component receives and how to use it. Route component signature is the component function — the runtime invokes it with `{ params, query, state, outlet? }` where `outlet` is present only for parent (non-leaf) routes. Document:
   - Reading params (`props.params.id`) and query (`props.query.tab`).
   - Reading app state via `props.state.key` (which is the value registered with `app.state(key, …)`).
   - Rendering child route output via `${props.outlet}` (parent routes only).
   - Active-link styling: anchors inside the mounted tree get `data-active` (prefix match) and `data-active-exact` (exact match) attributes. Show one CSS snippet styling on those attributes.

8. **`## Navigation`**. Functions from `zero` (router):
   - `navigate(path, { replace?, state? })` — push or replace history; triggers navigation pipeline. Throws if no app is running.
   - `back()`, `forward()` — delegate to `window.history`.
   - `route()` — returns a reactive view `{ path, params, query }`; reads inside an effect/reactive block subscribe.
   - Plain `<a href="/path">` is intercepted automatically for same-origin links unless the anchor has `target`, `download`, or `data-external`.

9. **`## App-level state`**. `inject(key)` — retrieves the value registered with `app.state(key, value)` on the currently running app. Throws if no app is running or the key isn't registered. Pair with a worked example reading `inject("count").val` inside a reactive block (this exactly matches `src/routes/home.js`). Cross-reference `## Testing` for `render(tr, { state })`.

10. **`## Testing`**. From `zero/test`. Subsections:
    - **Structure**: `describe(name, fn)`, `it(name, fn)`, `beforeEach`/`afterEach`/`beforeAll`/`afterAll`. All accept sync or async `fn`.
    - **DOM helpers**: `render(templateResult, { state? })` returns a container element wrapping all rendered children. `find(el, selector)` / `findAll(el, selector)` (querySelector/All). `text(el, selector?)` returns concatenated text-node content; throws if `selector` matches nothing. `fire(el, type, data?)` dispatches a synthetic event (the event object has `preventDefault`, `stopPropagation`, and `defaultPrevented`). `cleanup()` disposes everything created by `render` since the last `cleanup` — wire this into `afterEach`.
    - **Assertions** on `expect(actual)` — list ONLY the implemented matchers:
      - `toBe(expected)`
      - `toEqual(expected)` — deep equality, signal-aware
      - `toBeTruthy()`, `toBeFalsy()`, `toBeNull()`
      - `toContain(item)` — string substring or array containment
      - `toThrow(message?)` — `actual` must be a function
      - `toBeTemplateResult()`
      Explicit note: `toMatchSnapshot` is not yet implemented and currently throws.
    - **Testing components**: worked example identical in shape to `src/routes/home.test.js` (render + fire + assertion).
    - **Testing signals/computeds**: example exercising `signal`+`computed` directly without rendering.
    - **Testing routes**: example using `app.match("/users/42")` to verify pattern + params.

11. **`## JSDoc conventions`**. State the project rule: every exported function, class, and class method gets `@param`, `@returns`, and `@template` where applicable; module-level variables get `@type`; `@internal` marks exports outside the public API; `@private` marks private class methods. Reproduce the canonical example from `src/routes/home.js`:
    ```js
    /**
     * @typedef {import("zero").TemplateResult} TemplateResult
     */

    /**
     * @returns {TemplateResult}
     */
    export default function Home() {
      return html`<h1>Hello from zero</h1>`;
    }
    ```

12. **`## Common pitfalls`** (short — 6–10 bullets). Examples:
    - Components run once per mount — putting `signal()` at module scope shares it across all instances; put it inside the component function for per-instance state.
    - Reading `signal.val` outside a reactive context (no `effect`, no template) returns a snapshot and does not subscribe.
    - `each(sig, fn)` re-renders the whole list when `sig.val` changes; if that matters for performance, restructure rather than rely on diffing.
    - `inject(key)` throws if `app.state(key, …)` wasn't called — register every key during app setup.
    - `app.run()` must be called exactly once; builder methods throw after `run`.
    - Anchors with `target`, `download`, or `data-external` are not intercepted — use those to opt out of SPA navigation.

**Style rules for the file:**
- Plain CommonMark only — no HTML, no admonition syntax.
- All code fences tagged with a language (`js`, `css`, `html`, `bash`).
- Examples use ES module imports with `.js` extensions (matches scaffold).
- Where examples mirror scaffold files, cross-reference by path (e.g. *"see `src/routes/home.js`"*).
- No external links.
- No "coming soon" / roadmap content. If a feature isn't implemented today, don't mention it.

**Tests:** No tests change in this step — the new file is not yet referenced from Rust, so cargo build/test status is unchanged. The Step 2 wire-up step adds the tests.

---

### Step 2: Wire `AGENTS.md` into the scaffold writer and tests

**Goal:** `zero init` writes `<root>/AGENTS.md` alongside the other scaffold files. The existing `write_to_emits_all_four_files` test (and a new sentinel test) confirm it.

**Files:**
- Modify `src/scaffold.rs`.
- (No other Rust file changes — `cmd/init.rs` already calls `write_to`.)

**Changes:**

1. In `src/scaffold.rs`, add a new constant alongside the existing `TPL_*` constants:
   ```rust
   const TPL_AGENTS_MD: &str = include_str!("scaffold/AGENTS.md");
   ```
2. In `write_to`, after the existing `fs::write` calls and before the final `Ok(())`, add:
   ```rust
   fs::write(root_dir.join("AGENTS.md"), TPL_AGENTS_MD)?;
   ```
   (`AGENTS.md` sits at the project root, not inside `src/`. No new `create_dir_all` needed — `root_dir` is already created at the top of `write_to`.)
3. Update the existing test `write_to_emits_all_four_files`:
   - Rename to `write_to_emits_all_scaffold_files` (or similar — "four" is now wrong).
   - Add an assertion that `<root>/AGENTS.md` exists and is non-empty.
4. Add a new test `write_to_agents_md_has_section_sentinels` that reads the written `AGENTS.md` and asserts presence of one sentinel substring per major section, e.g.:
   - `"# Zero — Agent & Developer Reference"`
   - `"## Quick start"`
   - `"## Imports"`
   - `"## Components"`
   - `"## Reactivity"`
   - `"## App configuration"`
   - `"## Routes"`
   - `"## Navigation"`
   - `"## App-level state"`
   - `"## Testing"`
   - `"## JSDoc conventions"`
   - `"## Common pitfalls"`
   This catches accidental section deletion during future edits.

**Tests:** The two scaffold tests above. Run via `cargo test -p zero scaffold` (or `cargo test` for the whole crate). They are fast (file IO into a tempdir) and don't require Node.

---

## Risks and Assumptions

- **Spec/runtime drift.** The spec's exclusion list was wrong about `use()`, `loading()`, `error()`, and route opts. This plan trusts the runtime (matching the spec's binding rule), but the user may have wanted those omitted regardless of implementation status. If so, revise Step 1 to scope down to just `state` + `route` + `layout` + `run` + global loading/error. Easy to retract.
- **Runtime behavior surprises in nested routes.** The route opts schema accepts `children` where each child's `load` doubles as the loader, which is non-obvious. The plan documents what `runtime/app.js` actually accepts (`_flattenRoutes` requires `child.load` and treats it as the component loader). If this API is in flux, the nested-routes subsection will need revision when it stabilizes.
- **Throttle/debounce intervals are hard-coded to 100ms.** Documenting "fixed 100ms, not configurable" surfaces a rough edge. The plan accepts this honestly; if the user prefers to omit those modifiers from the doc until they're configurable, drop those two bullets.
- **`commit()` and `ref()` exposure.** `commit` is exported but is essentially a runtime primitive used by `render`. Plan: do not document it (agents don't need it). Same for any future incidental exports.
- **Length budget.** 400–800 lines is the target; if the comprehensive treatment overflows, prioritize accuracy and prune *examples* before pruning *coverage*. Cutting an example is recoverable; omitting a documented method silently is a trap for agents.
- **Cargo build embeds the file at compile time.** `include_str!` means the binary must be rebuilt after editing `AGENTS.md` for `zero init` output to change. Worth a one-line note in a future doc-on-the-CLI but not required here.
- **No JS-side test verifies the markdown.** We verify Rust-side that the file is written with section headers present. Substantive accuracy (do the examples actually run?) is human-reviewed, since we have no markdown-codeblock-executor in this repo today.
