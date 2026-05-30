# Spec: DOM-shim selectors — descendant, child, and selector-list combinators

## Problem Statement

The DOM shim's selector engine (`runtime/dom-shim.js`) only understands a
single *compound* selector — a tag optionally followed by `#id`, `.class`,
and `[attr]` parts. Any whitespace throws:

```
findAll(el, "tbody tr")
// Error: dom-shim: malformed selector "tbody tr" at position 5 (unexpected character ' ')
```

The descendant combinator is the single most common selector form in test
code — `findAll(container, "tbody tr")`, `find(card, ".header .title")`,
`findAll(list, "li button")`. Because it throws, every test author who reaches
for it has to rewrite the assertion: use a more specific single tag/class
(`tr.table-row`), drop to `text(el).toContain(...)`, or two-step
(`find(el, "tbody")` then traverse `childNodes`). This was logged as
friction-log entry L59 (`zero_demo/FRAMEWORK_NOTES.md`, 🟡, Area: zero/test).

Closing it makes `find` / `findAll` / `text(el, selector)` behave the way an
author coming from `document.querySelectorAll` expects, removing a recurring
papercut with no good workaround.

## Background

### Where the relevant code lives

- `runtime/dom-shim.js`
  - `_parseSelector(selector)` (line 11) — tokenizes one compound selector
    into `{ tag, id, classes, attrs }`. Throws `dom-shim: malformed selector
    …` on any character it doesn't expect, including space, `>`, and `,`.
  - `_matchSelector(node, selector)` (line 100) — parses the selector and
    tests a single node against the compound descriptor.
  - `_walkDescendants(root, fn)` (line 122) — depth-first walk of all
    descendant nodes in document order.
  - Element `querySelector` / `querySelectorAll` (lines 911, 918) and
    `closest` (line 925) — all call `_matchSelector` per node.
  - `document` `querySelector` / `querySelectorAll` (lines 1194, 1201) — a
    second copy of the same per-node walk.
- `runtime/test.js`
  - `find(el, selector)` (line 157) → `el.querySelector(selector)`.
  - `findAll(el, selector)` (line 167) → `el.querySelectorAll(selector)`.
  - `text(el, selector)` (line 178) → `el.querySelector(selector)` when a
    selector is passed.
  These are thin pass-throughs; they need no change once the element/document
  query methods understand combinators.
- `runtime/dom-shim.test.js` — `node:test` self-tests for the shim. There is
  currently **no** direct selector-parser coverage here; this slice adds it.
- `crates/zero-runtime/src/lib.rs` — embeds `runtime/dom-shim.js` verbatim as
  a string constant. The body grows; no other Rust change is needed.

### How matching works today

`_parseSelector` returns a flat descriptor. `_matchSelector` checks, in order:
tag (case-insensitive), id, every class is present in the node's `class`
attribute tokens, and every `[attr]` / `[attr=value]` constraint. `nodeType`
must be `ELEMENT_NODE`. Both `querySelector*` implementations walk descendants
in document order and keep nodes for which `_matchSelector` returns true.

The cleanest extension point is a new layer *above* the existing single-
compound parser: split the raw selector into a sequence of compound parts
joined by combinators (and into a list of such sequences on `,`), keep
`_parseSelector` exactly as the per-compound tokenizer, and add the chain/list
matching logic in `_matchSelector` (or a new `_matchComplexSelector`) so all
five call sites benefit without change.

### Scope decision (already made with the user)

Support **descendant** (`a b`), **child** (`a > b`), and **selector lists**
(`a, b`). Do **not** support adjacent-sibling (`a + b`) or general-sibling
(`a ~ b`) combinators — they are rare in test code and add matching
complexity; they continue to throw a `malformed selector` error.

## Requirements

All paths relative to repo root.

### 1. Selector grammar

A selector is a comma-separated **list** of complex selectors. Each complex
selector is a sequence of compound selectors separated by combinators:

- whitespace → **descendant** combinator
- `>` (optionally surrounded by whitespace) → **child** combinator

Each compound selector is parsed by the existing `_parseSelector` (unchanged
tag/id/class/attr grammar). Leading/trailing/collapsed whitespace around
combinators and list commas is tolerated (`"th , td"`, `"ul  >  li"`,
`" tbody tr "` all parse).

### 2. Parsing layer

Add a function (e.g. `_parseComplexSelector(selector)`) that:

- Splits the raw string on top-level `,` into list branches; an empty branch
  (e.g. trailing comma, `"a,"`) throws a `malformed selector` error.
- Within a branch, splits into `{ combinator, compound }` steps where
  `combinator` is `"descendant"` or `"child"`, and `compound` is the
  descriptor returned by `_parseSelector`.
- Reuses `_parseSelector`'s existing error message format
  (`dom-shim: malformed selector "<full-original-selector>" at position N
  (<reason>)`) so error text stays consistent. The reported position refers to
  the original full selector string, not a split fragment.
- `>` with no compound before or after it (`"> a"`, `"a >"`, `"a > > b"`)
  throws `malformed selector`.
- Sibling combinators `+` / `~` throw `malformed selector` with an
  `unexpected character` reason (preserving today's behavior for those chars).

### 3. Matching layer

Add complex-selector matching that, given a candidate node and a parsed list:

- A node matches the **list** if it matches **any** branch.
- A node matches a **branch** if its **rightmost** compound matches the node,
  and the preceding compounds match an ancestor chain reading right-to-left:
  - **descendant**: some ancestor (any depth) matches the next-left compound;
  - **child**: the *immediate* `parentNode` matches the next-left compound.
- Ancestor walking for combinator matching is **bounded by the query root**
  (the element `querySelector*` was invoked on, or `document` for the document
  methods): a left-hand compound only matches ancestors that are descendants of
  — or equal to — the query root. (See Constraints for the rationale and the
  divergence from spec `:scope` semantics.)

### 4. Wire into all call sites

Route the element `querySelector` / `querySelectorAll` / `closest` (lines
911–932) and the document `querySelector` / `querySelectorAll` (lines
1194–1207) through the new matching path. `querySelectorAll` must:

- return results in **document order**, and
- return each matching node **at most once**, even if it matches multiple
  branches of a selector list.

The depth-first `_walkDescendants` already yields document order; testing each
node once against the whole list yields natural de-duplication.

`closest(selector)` accepts a **selector list** (matches self-or-ancestor
against any branch). Whether `closest` honors descendant/child combinators
inside a branch is an open question (see below) — at minimum it must not
regress and must handle comma lists.

### 5. `find` / `findAll` / `text`

No code change required in `runtime/test.js`; they delegate to the query
methods. Confirm via tests that `findAll(el, "tbody tr")`, `find(el, "ul >
li")`, and `text(el, "thead th")` now work.

### 6. Self-tests (`runtime/dom-shim.test.js`)

Add a `describe` block for the selector engine covering:

- **Descendant**: `findAll(root, "ul li")` returns nested `<li>` at any depth;
  a non-matching outer structure is excluded.
- **Child**: `"ul > li"` matches only direct children; a grandchild `<li>`
  inside a nested `<ul>` is excluded where descendant would include it.
- **Selector list**: `"th, td"` returns both, in document order, with no
  duplicates when a node could match more than one branch.
- **Mixed**: `"table tbody > tr td"` exercises descendant + child in one
  branch.
- **Scope bound**: a left-hand compound matching only an ancestor *outside*
  the query root does not produce a match (documents the bounded-walk choice).
- **Whitespace tolerance**: `" tbody tr "`, `"ul  >  li"`, `"th , td"` parse.
- **Errors preserved**: `"a > > b"`, `"a >"`, `"> a"`, `"a + b"`, `"a ~ b"`,
  `"a,"` each throw a `dom-shim: malformed selector` error; an existing
  single-compound error case (e.g. `".#bad"` style) still throws unchanged.
- **Single-compound regression**: existing `tag` / `#id` / `.class` /
  `[attr]` / `[attr=value]` selectors behave exactly as before.

### 7. Spec-text / docs touch-ups

- `docs/testing.md` — if it documents the selector limitation ("single-element
  selectors only") update it to state descendant / child / list are supported
  and sibling combinators are not. (Verify whether such text exists; add a
  brief note under the `find`/`findAll` reference if not.)
- `zero_demo/FRAMEWORK_NOTES.md` L59 — flip `- [ ]` to `- [x]` and append the
  `**FIXED YYYY-MM-DD** (commit/issue): …` annotation per that file's
  convention. (Done at land time, not in this repo's build.)

## Constraints

- **No new public API.** `find` / `findAll` / `text` / `closest` signatures are
  unchanged; this is purely a parser/matcher capability expansion. No new
  exports from `"zero"` or `"zero/test"`.
- **`_parseSelector` stays the per-compound tokenizer.** Don't rewrite the
  working tag/id/class/attr grammar; layer combinator handling on top so the
  blast radius is small and existing single-compound behavior is provably
  unchanged.
- **Error message format is preserved**: `dom-shim: malformed selector
  "<selector>" at position N (<reason>)`. Test code and any future lint that
  greps for this string must keep matching.
- **Ancestor walk is bounded by the query root.** Real `querySelectorAll`
  evaluates the full selector against the whole document and only *returns*
  descendants of the root (so `el.querySelectorAll("div span")` can match a
  `div` ancestor *outside* `el`). The shim instead bounds left-hand matching to
  within the query root. This is simpler, matches test-author intuition
  ("everything is relative to the element I queried"), and avoids needing a
  document-rooted context. Documented as an intentional divergence.
- **No `:scope`, no pseudo-classes, no pseudo-elements, no `*` universal
  selector** unless one already happens to work — this slice does not add them.
- **Sibling combinators (`+`, `~`) remain unsupported** and throw.
- Embedded-string parity: `crates/zero-runtime/src/lib.rs` carries
  `runtime/dom-shim.js` byte-for-byte; the build picks up the grown body with
  no Rust edit.

## Out of Scope

- Adjacent-sibling (`+`) and general-sibling (`~`) combinators.
- The universal selector `*`.
- Pseudo-classes (`:first-child`, `:not()`, `:nth-child()`, `:hover`, …) and
  pseudo-elements (`::before`).
- `:scope` and full document-rooted selector semantics (the shim uses the
  bounded-root model above).
- Attribute-selector operators beyond `=` (`^=`, `$=`, `*=`, `~=`, `|=`).
- Case-insensitive attribute value matching (`[attr=val i]`).
- Namespace selectors.
- Any change to `find` / `findAll` / `text` / `spy` / `cleanup` surfaces, or to
  the broader dom-shim expansion tracked in `issues/dom-shim/`.

## Open Questions

- **`closest` and combinators.** Real `Element.closest(selectors)` accepts
  complex selectors and matches self-or-ancestor against the *document*. With
  the bounded-root model that's awkward. Recommendation: `closest` supports
  selector **lists** (cheap, common) but treats each branch as a single
  compound for combinator purposes — i.e. ignore/forbid combinators inside a
  `closest` branch — unless the plan finds a clean way to support them. Plan to
  decide and document; ensure no regression for today's single-compound
  `closest` callers.
- **Should `_parseComplexSelector` results be cached?** Selectors are usually
  string literals re-passed across many `findAll` calls in a test loop. A small
  `Map<string, parsed>` cache is cheap and avoids re-tokenizing. Recommendation:
  add a tiny memo keyed on the raw selector string; defer final call to plan.
- **Position reporting after splitting.** When a malformed compound is found in
  a non-first branch/step, the reported `position` should index into the
  original full selector. Plan confirms the offset bookkeeping so the existing
  error format stays accurate rather than pointing into a fragment.
- **`text(el, selector)` "matched nothing" behavior.** `text` throws when its
  selector matches nothing; combinator selectors that legitimately match
  nothing must still throw the same clear error — confirm no behavioral change
  beyond the wider grammar.
