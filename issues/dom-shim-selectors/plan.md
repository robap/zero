# Plan: DOM-shim selectors — descendant, child, and selector-list combinators

## Summary

Extend the dom-shim selector engine (`runtime/dom-shim.js`) from single
compound selectors to complex selectors: descendant (`a b`), child (`a > b`),
and comma selector-lists (`a, b`). The approach layers a splitter + complex
parser *above* the existing per-compound tokenizer (`_parseSelector`, left
intact), and a chain matcher *above* the existing per-compound node test. All
five query call sites (element + document `querySelector`/`querySelectorAll`,
and `closest`) route through the new matcher; `find`/`findAll`/`text` in
`runtime/test.js` need no change since they delegate. Sibling combinators
(`+`/`~`) stay unsupported and continue to throw via the compound parser.

## Prerequisites

The spec's open questions are resolved in this plan:

- **`closest` + combinators** — resolved by passing an *unbounded* root to the
  matcher for `closest` (see Step 2), which gives correct self-or-ancestor
  combinator semantics for free. No need to forbid combinators in `closest`.
- **Caching** — adopt a module-level `Map<string, parsedList>` memo keyed on
  the raw selector string (Step 1).
- **Position reporting after splitting** — `_parseSelector` gains an `offset`
  and `fullSelector` parameter so malformed-compound errors index into the
  original string (Step 1).
- **`text(el, selector)` "matched nothing"** — unchanged; it delegates to
  `querySelector`, so wider grammar flows through with the same throw behavior.

None block execution.

## Steps

- [x] **Step 1: Parsing layer — splitter, complex parser, offset-aware compound parser, memo cache**
- [x] **Step 2: Matching layer + wire all query call sites + behavioral self-tests**
- [x] **Step 3: Docs update, friction-log annotation, and embedded-string parity verification**

---

## Step Details

### Step 1: Parsing layer

**Goal:** Produce a parsed representation of a complex selector (list of
branches, each a sequence of `{combinator, compound}` steps) without changing
any runtime behavior yet. This is additive scaffolding plus a behavior-
preserving refactor of `_parseSelector`'s signature. Existing tests stay green
because nothing is wired to the new functions and the single-compound path is
byte-for-byte equivalent.

**Files:**
- `runtime/dom-shim.js` (modify)

**Changes:**

1. **Make `_parseSelector` offset-aware (behavior-preserving).** Change the
   signature to `_parseSelector(selector, offset = 0, fullSelector = selector)`.
   Inside, `malformed(pos, reason)` builds its message from `fullSelector` and
   `pos + offset`:
   ```js
   function malformed(pos, reason) {
     throw new Error(`dom-shim: malformed selector "${fullSelector}" at position ${pos + offset} (${reason})`);
   }
   ```
   With default args (the only way it's called today) the message is identical
   to current output. The empty-string guard at the top stays as-is for the
   single-compound case (the list parser handles whole-string emptiness).

2. **Add `_splitSelector(selector)`** — a top-level scanner that returns an
   array of branches, each branch an array of step descriptors
   `{ combinator, start, end }` where `combinator` is `"none"` (first step of a
   branch), `"descendant"`, or `"child"`, and `[start, end)` bound the compound
   text within the original string. Scanning rules:
   - Track `bracketDepth` (`+`/`-` on `[` / `]`) and an in-quote flag (set on
     `"`/`'` while `bracketDepth > 0`). While inside a bracket or quote, all
     characters — including spaces, `>`, `,` — are literal compound content
     (so `[data-x="a b"]`, `[x=a>b]` don't split).
   - At top level (`bracketDepth === 0`, not quoted):
     - `,` closes the current compound and the current branch; begins a new
       branch (with a fresh `pendingCombinator = "none"`).
     - a run of whitespace closes the current compound (if one is open) and
       sets `pendingCombinator = "descendant"` (collapsing consecutive
       whitespace).
     - `>` closes the current compound (if open) and sets
       `pendingCombinator = "child"`, overriding any descendant pending from
       surrounding whitespace (so `a > b`, `a>b`, `a  >  b` are all child).
     - any other char: if no compound is currently open, mark its `start = i`.
   - When a compound closes, push `{ combinator: pendingCombinator, start, end }`
     to the current branch and reset `pendingCombinator` to `"none"` for the
     *next* compound boundary; a combinator is carried on the step it precedes.
   - **Validation (all throw via `malformed` against the original string):**
     - leading `>` in a branch (`"> a"`, `",> a"`): `expected selector before >`.
     - dangling `>` at branch/string end (`"a >"`, `"a > "`, `"a > > b"`):
       `expected selector after >`.
     - empty branch (`","`, `"a,"`, `",a"`, `"a,,b"`): `empty selector in list`.
     - a *trailing descendant* (plain trailing/leading whitespace, e.g.
       `" tbody tr "`) is benign and ignored — it is not a dangling combinator.
   - `+` / `~` need **no** handling here: they are not combinator chars, so they
     fall into compound text and `_parseSelector` rejects them with its existing
     `unexpected character '<c>'` error (preserving today's behavior).

3. **Add `_parseComplexSelector(selector)`** — the public-internal entry:
   - If `selector.trim() === ""` throw `dom-shim: empty selector` (matches the
     existing empty-selector contract).
   - Check the memo cache (`_selectorCache`, a module-level `Map`); return the
     cached parse on hit.
   - Call `_splitSelector`, then map each step's `[start, end)` slice through
     `_parseSelector(text, start, selector)` to attach a parsed `compound`
     descriptor, yielding `Array<Array<{ combinator, compound }>>`.
   - Store in the cache and return.

4. **Add `const _selectorCache = new Map();`** near the other module-level
   declarations.

**Tests:** No new tests in this step — the new functions are module-private and
have no observable effect until Step 2 wires them in. Verification is that the
existing suite stays green, confirming the `_parseSelector` signature refactor
is behavior-preserving:
- `cargo run -p zero -- test dom-shim.test.js`
- `cargo run -p zero -- test` (full runtime suite — `template`, `router`, etc.
  exercise `querySelector`/`find` indirectly).

### Step 2: Matching layer + wiring + behavioral self-tests

**Goal:** Make complex selectors actually work by adding chain matching and
routing all five query sites through it, then lock the behavior with the full
self-test suite from the spec. This is the step where behavior changes.

**Files:**
- `runtime/dom-shim.js` (modify)
- `runtime/dom-shim.test.js` (modify — add selector-engine `describe` block)

**Changes:**

1. **Split `_matchSelector` into a compound test.** Rename the node-vs-
   descriptor body (current lines 102–119) into
   `_matchCompound(node, parsed)` taking an already-parsed compound descriptor
   (drop the internal `_parseSelector(selector)` call; the `nodeType !==
   ELEMENT_NODE` guard and all tag/id/class/attr checks stay). Remove the old
   `_matchSelector` wrapper once all callers are migrated (Step 2.3).

2. **Add `_matchComplexSelector(node, parsedList, root)`** (`root` may be
   `null`/`undefined` ⇒ unbounded ancestor walk):
   - Returns true if `node` matches **any** branch.
   - Per branch (`steps`, left-to-right; `steps[0].combinator === "none"`):
     - The **rightmost** compound must `_matchCompound(node, …)`; if not, the
       branch fails fast.
     - Then recurse leftward via `_matchFrom(node, steps, steps.length - 1, root)`:
       - `i === 0` ⇒ return true (all compounds satisfied).
       - `combinator = steps[i].combinator` links `steps[i-1]` → `steps[i]`.
       - **child**: let `p = node.parentNode`; if `p` is within the root bound
         (inclusive) and `_matchCompound(p, steps[i-1].compound)`, return
         `_matchFrom(p, steps, i-1, root)`; else false.
       - **descendant**: walk `anc = node.parentNode` upward while within the
         root bound (inclusive); for each, if
         `_matchCompound(anc, steps[i-1].compound) && _matchFrom(anc, steps, i-1, root)`
         return true; else continue; return false when the bound is exhausted.
   - **Root bound (inclusive):** ancestors from the candidate's parent up to
     **and including** `root` are eligible; stop after testing `root`. When
     `root` is null/undefined, walk to the top of the tree (`parentNode ===
     null`). Candidate nodes from `_walkDescendants` are always strict
     descendants of the query root, so `root` lies on their ancestor chain.

3. **Rewire the five call sites** to parse once with `_parseComplexSelector`
   and match with `_matchComplexSelector`:
   - Element `querySelector` (line 911) and `querySelectorAll` (line 918): parse
     the selector once before the walk, pass `root = this`. `querySelectorAll`
     pushes each matching descendant once (DFS `_walkDescendants` already yields
     document order; testing each node once gives natural de-duplication across
     list branches).
   - Element `closest` (line 925): parse once; walk self-then-ancestors; for
     each candidate test `_matchComplexSelector(candidate, parsedList, null)`
     (unbounded root → correct combinator semantics for `closest`). Return the
     first match or null.
   - Document `querySelector` (line 1194) and `querySelectorAll` (line 1201):
     same as element versions with `root = this` (the `document` object).
   - Remove the now-unused `_matchSelector`.

4. **Add the selector-engine self-tests** to `runtime/dom-shim.test.js`,
   written against the public `zero/test` API (`render(html\`…\`)`, `find`,
   `findAll`, and `document.createElement`/`appendChild` where a precise tree is
   needed — matching the file's existing style). A new
   `describe('selector engine', …)` block with `afterEach(cleanup)` covering:
   - **Descendant**: `findAll(root, "ul li")` returns nested `<li>` at any
     depth; structure with no enclosing `<ul>` is excluded.
   - **Child**: `"ul > li"` matches only direct children; a grandchild `<li>`
     inside a nested `<ul>` is excluded (contrast with descendant including it).
   - **Selector list**: `"th, td"` returns both in document order; a node that
     could match more than one branch appears exactly once (no duplicates).
   - **Mixed**: `"table tbody > tr td"` (descendant + child in one branch).
   - **Scope bound**: a left-hand compound that matches only an ancestor
     *outside* the query root yields no match (locks the bounded-walk choice).
   - **closest**: `find(el,'span').closest('div')` and a combinator/list form
     resolve self-or-ancestor.
   - **Whitespace tolerance**: `" tbody tr "`, `"ul  >  li"`, `"th , td"` parse
     and match.
   - **Errors preserved**: `"a > > b"`, `"a >"`, `"> a"`, `"a + b"`, `"a ~ b"`,
     `"a,"`, `""` each throw a `dom-shim:` error (assert via
     `expect(() => findAll(el, sel)).toThrow(...)` or the matcher the suite
     uses); message still matches `dom-shim: malformed selector "<orig>" at
     position N (...)` for the malformed cases, with the position indexing the
     original string.
   - **Single-compound regression**: `tag`, `#id`, `.class`, `[attr]`,
     `[attr=value]`, and an existing malformed case (`".#bad"`) behave exactly
     as before.

**Tests:** The new `describe('selector engine')` block above. Run:
- `cargo run -p zero -- test dom-shim.test.js`
- `cargo run -p zero -- test` (full runtime suite — guards against regressions
  in template/router/test helpers that lean on `querySelector`/`find`).

### Step 3: Docs, friction-log annotation, and parity verification

**Goal:** Reflect the new capability in user docs, record the fix in the
friction log, and confirm the Rust side picks up the grown shim with no code
change.

**Files:**
- `docs/testing.md` (modify — if it states a selector limitation)
- `~/Documents/code/zero_demo/FRAMEWORK_NOTES.md` (modify — flip L59, append fix
  annotation)

**Changes:**

1. **`docs/testing.md`** — locate any text documenting the selector limitation
   ("single-element selectors only" / the `find`/`findAll` reference). Update it
   to state that descendant (`a b`), child (`a > b`), and comma lists (`a, b`)
   are supported, and that sibling combinators (`+`, `~`), pseudo-classes, and
   `*` are not. If no such text exists, add a brief note under the
   `find`/`findAll` reference. (Per the project memory on avoiding over-
   validation, no Jekyll build is needed — a sanity read of the edited section
   suffices.)

2. **`~/Documents/code/zero_demo/FRAMEWORK_NOTES.md`** — flip L59 from `- [ ]`
   to `- [x]` and append, per that file's convention:
   `**FIXED YYYY-MM-DD** (<commit>): _parseSelector kept as the per-compound
   tokenizer; new _splitSelector/_parseComplexSelector add descendant/child/
   list combinators routed through all querySelector*/closest sites. Sibling
   combinators still unsupported.` (Use today's date and the landing commit
   SHA.)

3. **Embedded-string parity** — `crates/zero-runtime/src/lib.rs` embeds
   `runtime/dom-shim.js` verbatim; no Rust edit is required, but confirm the
   build and Rust-side tests pass with the grown body:
   - `cargo test -p zero-runtime`
   - `cargo test --workspace` (fast loop; the slow `#[ignore]` integration tests
     are not needed for a runtime-JS change of this scope).

**Tests:** No new tests; this step is docs + verification. The `cargo test`
runs above confirm parity.

## Risks and Assumptions

- **Splitter correctness around brackets/quotes is the main risk.** A space,
  `>`, or `,` inside an attribute value (`[data-label="a b"]`, `[x=a>b]`) must
  not split. The bracket-depth + quote tracking in Step 1.2 handles this; the
  self-tests should include at least one attribute-value-with-space case to lock
  it (add to the Step 2 suite if not already implied).
- **Bounded-root semantics diverge from real `querySelectorAll`** (which
  matches against the whole document and only scopes the *results*). This is an
  intentional, documented choice in the spec. If a real test later needs true
  `:scope` semantics, the matcher's `root` parameter is the seam to revisit —
  passing `null` already yields unbounded behavior.
- **`closest` unbounded-root assumption.** Treating `closest` as unbounded gives
  textbook self-or-ancestor combinator behavior, but it's a slightly broader
  capability than the spec's minimum ("at least comma lists, no regression").
  It is strictly more capable and not a regression, so low risk.
- **Memo cache unboundedness.** `_selectorCache` grows with distinct selector
  strings. In a test process this is bounded by the number of literal selectors
  in the suite (small); not worth an eviction policy. If a test ever generates
  unbounded dynamic selector strings, revisit.
- **Assumption: no other code reaches `_matchSelector` or `_parseSelector`
  directly.** Grep confirmed only the five query sites use them and the
  functions are module-private (not exported). If a later grep finds another
  caller, migrate it to `_matchCompound` / `_parseComplexSelector` in Step 2.
- **Assumption: `_walkDescendants` order is stable document order** (DFS over
  `childNodes`), which de-duplication for selector lists relies on. Confirmed
  from the current implementation (line 122).
