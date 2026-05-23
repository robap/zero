# Plan: Template Fixes — Attribute Interpolation & Configurable Timing Modifiers

## Summary

Fix two `runtime/template.js` bugs called out in the friction log: (1)
multi-placeholder attribute values silently drop their static fragments, and
(2) `.debounce` / `.throttle` cannot have their interval configured. The
attribute fix changes the `attr` part shape to carry a `statics: string[]`
plus an implicit value-count (`statics.length - 1`), generalizes parser
state in `ATTR_VALUE_*` to accumulate multiple placeholders per attribute,
and updates `commit()` to advance a value cursor and pass a slice into a
rewritten `_commitAttr`. The timing-modifier fix extends `_wrapEventHandler`
to parse a `:NNN` suffix on `throttle` / `debounce` segments only, with
loud failure on malformed suffixes. T02 lint and `docs/templates.md` are
updated to match.

## Prerequisites

Three open questions from the spec are resolved here so execution does not
stall:

1. **`parts.length === values.length` invariant — drop it.** Use option (a)
   from the spec: each `attr` part may consume multiple values. The
   commit-loop advances a value cursor by `part.statics.length - 1` for
   `attr` parts and by `1` for all other part types. No `groupId` /
   consecutive-part scheme — that adds bookkeeping for no readability gain.
2. **Effect cleanup on multi-placeholder attrs.** A single `effect()`
   wrapping the whole attribute join is registered with the active scope
   the same way single-placeholder attrs are today. The Step 1 tests
   include a scope-dispose verification to confirm this works without
   special casing.
3. **T02 grammar.** The existing regex `@(\w+)((?:\.\w+)+)\s*=` cannot
   match `@input.debounce:250=` because `\w` excludes `:`. Update the
   regex to allow an optional `:<digits>` tail per segment, then validate
   each segment string. Detailed in Step 3.

## Steps

- [x] **Step 1: Multi-placeholder attribute support (parser + commit)**
- [x] **Step 2: Configurable `:NNN` interval on `throttle` / `debounce`**
- [x] **Step 3: T02 lint rule accepts `:NNN` on timing modifiers only**
- [x] **Step 4: Docs update for both fixes**

---

## Step Details

### Step 1: Multi-placeholder attribute support (parser + commit)

**Goal:** Make `html\`<span class="chip chip--${status} active">\`` produce
the DOM the user wrote, and generalize to any number of placeholders mixed
with static text in one attribute value. Parser and commit ship together —
they share a data-shape change, and splitting them would leave the
codebase in a state where parser output and commit input don't match.

**Files:**
- `runtime/template.js` — parser state changes; `attr` part shape;
  `_commitAttr` rewrite; `commit()` dispatch loop value-cursor.
- `runtime/template.test.js` — parser-inspection tests and end-to-end
  commit tests.

**Changes:**

#### 1a. New `attr` part shape

```js
// Before
{ type: 'attr', path, name }
// After
{ type: 'attr', path, name, statics: string[] }
```

`statics.length` is always `valueCount + 1`. The legacy single-placeholder
case (`class=${x}`) emits `statics: ['', '']` (value-count 1). A
multi-placeholder case (`class="chip chip--${a} ${b} active"`) emits
`statics: ['chip chip--', ' ', ' active']` (value-count 2).

Document the shape change in the JSDoc typedef block near the top of
`runtime/template.js` (the `@typedef Template` block on line 5–7 is the
spot — extend it or add a `@typedef AttrPart`).

#### 1b. Parser state extension

Add three locals to `_parseTemplate` (alongside `currentAttrName` /
`currentAttrValue` at line 46):

```js
let attrStatics = null;      // string[] — built across multi-placeholder attr values
let attrHasPlaceholders = false; // boolean — was at least one ${} seen in this attr?
```

These reset every time a new attribute name starts (in the
`IN_TAG` → `ATTR_NAME` transition at line 156).

When entering an attribute value state (`ATTR_VALUE_DQ` / `SQ` /
`UNQUOTED`), nothing changes initially — we still collect into
`currentAttrValue`. The change happens at the placeholder boundary and
the value-close boundary:

**At placeholder boundary inside an attribute value** (lines 252–272):

```js
case AFTER_ATTR_NAME:
case ATTR_VALUE_DQ:
case ATTR_VALUE_SQ:
case ATTR_VALUE_UNQUOTED: {
  const path = [...parentPath];
  if (currentAttrName.startsWith('@')) {
    const [eventPart, ...modifiers] = currentAttrName.slice(1).split('.');
    parts.push({ type: 'event', path, event: eventPart, modifiers });
    currentAttrName = '';
    currentAttrValue = '';
    if (state === AFTER_ATTR_NAME) state = IN_TAG;
  } else if (currentAttrName === 'ref') {
    parts.push({ type: 'ref', path });
    currentAttrName = '';
    currentAttrValue = '';
    if (state === AFTER_ATTR_NAME) state = IN_TAG;
  } else {
    // attr — accumulate static fragment, defer emit until value closes.
    if (attrStatics === null) attrStatics = [];
    attrStatics.push(currentAttrValue);   // prefix (or interleaved static)
    currentAttrValue = '';
    attrHasPlaceholders = true;
    // Stay in the same attr-value state. AFTER_ATTR_NAME case below.
  }
  break;
}
```

For `AFTER_ATTR_NAME` (unquoted single-placeholder, e.g. `class=${x}`),
treat the placeholder as the entire value: push `''` to statics (prefix),
move to a synthetic `ATTR_VALUE_AFTER_PLACEHOLDER_UNQUOTED` mode — or
simpler, just set state to `ATTR_VALUE_UNQUOTED` and let the existing
whitespace/`>` terminator emit the final attr part. The simplest
implementation: when handling `AFTER_ATTR_NAME` placeholder for a
non-event/non-ref attr, push `''` to statics, set
`attrHasPlaceholders = true`, and transition state to
`ATTR_VALUE_UNQUOTED` with `currentAttrValue = ''`. The value-close
branch then runs as below.

**At attribute-value close** (the existing `flushStaticAttr` call sites,
lines 198–199, 207–208, 215–220, 167–170):

Replace `flushStaticAttr()` with `flushAttr()`:

```js
function flushAttr() {
  if (!currentAttrName) return;
  if (attrHasPlaceholders) {
    // Close out the trailing static fragment.
    attrStatics.push(currentAttrValue);
    parts.push({
      type: 'attr',
      path: [...parentPath],
      name: currentAttrName,
      statics: attrStatics,
    });
  } else {
    // Static-only attribute (no placeholders) — set at parse time as today.
    parent.setAttribute(currentAttrName, currentAttrValue);
  }
  currentAttrName = '';
  currentAttrValue = '';
  attrStatics = null;
  attrHasPlaceholders = false;
}
```

The old `flushStaticAttr` is removed. Every call site that referenced it
now calls `flushAttr`. Boolean-attribute terminators (`>` in `ATTR_NAME`
state at line 167, and `>` in `AFTER_ATTR_NAME` at line 181) still call
`flushAttr` — those flows never set `attrHasPlaceholders` so they fall
into the static-set branch.

#### 1c. Path computation parity

`path: [...parentPath]` is the same for the new multi-placeholder attr
part as for the existing single one — the path points at the element
carrying the attribute. No path-walking changes are needed in `_walkPath`
or in `commit()`'s pre-walk on line 594. (The pre-walk maps `parts[i] →
target node`, which is still 1:1 since one attr part exists per
attribute regardless of placeholder count.)

#### 1d. `commit()` value-cursor

Today (line 596–607):

```js
for (let i = 0; i < _template.parts.length; i++) {
  const part = _template.parts[i];
  const target = targets[i];
  const value = _values[i];                  // 1:1 with parts
  switch (part.type) {
    case 'attr':  _commitAttr(target, part.name, value); break;
    // ...
  }
}
```

After:

```js
let valueCursor = 0;
for (let i = 0; i < _template.parts.length; i++) {
  const part = _template.parts[i];
  const target = targets[i];
  switch (part.type) {
    case 'attr': {
      const n = part.statics.length - 1;
      _commitAttr(target, part.name, part.statics, _values.slice(valueCursor, valueCursor + n));
      valueCursor += n;
      break;
    }
    case 'event': _commitEvent(target, part.event, part.modifiers, _values[valueCursor]); valueCursor++; break;
    case 'ref':   _commitRef(target, _values[valueCursor]); valueCursor++; break;
    case 'node':  _commitNode(target, _values[valueCursor]); valueCursor++; break;
  }
}
```

#### 1e. `_commitAttr` rewrite

New signature: `_commitAttr(el, name, statics, values)`. The function
decides between three paths based on `values` content.

```js
function _commitAttr(el, name, statics, values) {
  // Fast path: single placeholder with empty surrounding statics — keep
  // the exact behavior of the old single-placeholder attr (preserves
  // boolean / null / undefined semantics).
  if (statics.length === 2 && statics[0] === '' && statics[1] === '') {
    const value = values[0];
    if (_isReactive(value))             effect(() => _applyAttr(el, name, value.val));
    else if (typeof value === 'function') effect(() => _applyAttr(el, name, value()));
    else                                  _applyAttr(el, name, value);
    return;
  }

  // Concat path: any value reactive → one effect wraps the whole join.
  const anyReactive = values.some(v => _isReactive(v) || typeof v === 'function');
  if (anyReactive) {
    effect(() => _setJoinedAttr(el, name, statics, values));
  } else {
    _setJoinedAttr(el, name, statics, values);
  }
}

function _setJoinedAttr(el, name, statics, values) {
  let out = statics[0];
  for (let i = 0; i < values.length; i++) {
    out += _coerceConcatValue(values[i]) + statics[i + 1];
  }
  el.setAttribute(name, out);
}

function _coerceConcatValue(v) {
  if (v == null) return '';                          // null/undefined → empty
  if (_isReactive(v)) return _coerceConcatValue(v.val);
  if (typeof v === 'function') return _coerceConcatValue(v());
  return String(v);                                  // true/false stringified too
}
```

Note that the concat path **always** writes the attribute (never removes
it) — boolean/null semantics only apply to the fast-path single-value
case, per the spec.

#### 1f. JSDoc

Update the file-level `@typedef Template` block (line 5–7) to either
add an `@typedef AttrPart` or document the `statics` field inline. The
project's house rule (CLAUDE.md, "All JavaScript files must be fully
JSDoc-annotated") applies — new internal helpers (`_setJoinedAttr`,
`_coerceConcatValue`, `flushAttr`) get JSDoc with `@param` / `@returns`
and `@internal`.

**Tests:**

Add to `runtime/template.test.js`:

Parse-tree assertions (no commit):
- `html\`<div class=${x}></div>\`` → one part, `statics: ['', '']`.
- `html\`<div class="prefix ${x} suffix"></div>\`` → one part,
  `statics: ['prefix ', ' suffix']`.
- `html\`<div class="${a} ${b}"></div>\`` → one part,
  `statics: ['', ' ', '']`, value-count 2.
- `html\`<div class="a ${x} b ${y} c"></div>\`` → one part,
  `statics: ['a ', ' b ', ' c']`.
- `html\`<div style="color: ${c}; padding: ${p}px;"></div>\`` → one part,
  `statics: ['color: ', '; padding: ', 'px;']`.
- Boolean static attrs unaffected: `html\`<input disabled />\`` parses
  with `disabled=""` on the element and no parts.
- Static attr unaffected: `html\`<div class="x"></div>\`` sets `class="x"`
  at parse time, zero parts.

End-to-end commit assertions:
- Static-only mixed: `html\`<span class="a ${'x'} b"></span>\`` commits
  to `class="a x b"`.
- Signal in concat: `const s = signal('y'); commit(html\`<div class="p ${s} s"></div>\`, frag);`
  → `class="p y s"`; then `s.set('z')` → `class="p z s"`.
- Two signals in one attribute: shared `effect` re-runs on either change;
  count update-callbacks to verify one effect runs, not two.
- Reactive function in concat:
  `html\`<div class="${() => mode.val}"></div>\``.
- `null` in concat renders as empty string between statics
  (`class="a  b"`).
- Scope dispose: commit inside a scope, dispose, mutate the signal —
  no DOM update.
- Backward compat: every existing test that uses `class=${signal}` /
  `disabled=${bool}` continues to pass (true → `''`, false → remove
  attribute, null → remove).

Run `cargo run -p zero -- test template.test.js` and the full workspace
test suite before moving on.

---

### Step 2: Configurable `:NNN` interval on `throttle` / `debounce`

**Goal:** Let users write `@input.debounce:250=${onSearch}` and get a
250ms debounce. Bare `.debounce` / `.throttle` keep the 100ms default.
Malformed suffixes throw at commit time.

**Files:**
- `runtime/template.js` — `_wrapEventHandler`.
- `runtime/template.test.js` — modifier-timing tests.

**Changes:**

#### 2a. Parse the suffix at commit time, not parse time

The parser's `currentAttrName.slice(1).split('.')` at line 258 already
preserves `debounce:250` as a single segment of the `modifiers` array —
no parser change is needed. The work lives in `_wrapEventHandler`
(line 538–556).

Replace the two hard-coded lines:

```js
const throttleMs = modifiers.includes('throttle') ? 100 : 0;
const debounceMs = modifiers.includes('debounce') ? 100 : 0;
```

with:

```js
const throttleMs = _readTimingModifier(modifiers, 'throttle');
const debounceMs = _readTimingModifier(modifiers, 'debounce');
```

And add:

```js
function _readTimingModifier(modifiers, name) {
  for (const m of modifiers) {
    if (m === name) return 100;                          // bare → default
    if (m.startsWith(name + ':')) {
      const tail = m.slice(name.length + 1);
      if (!/^\d+$/.test(tail)) {
        throw new Error(`html: invalid modifier '${m}' — expected '${name}:<ms>' with positive integer`);
      }
      const n = Number(tail);
      if (n <= 0) {
        throw new Error(`html: invalid modifier '${m}' — interval must be > 0`);
      }
      return n;
    }
  }
  return 0;                                              // not present
}
```

Notes:
- `/^\d+$/` rejects empty (`debounce:`), non-numeric (`debounce:abc`),
  signed (`debounce:-5`), and decimal (`debounce:1.5`) suffixes — all
  caught as the same error class with a single message.
- The `n > 0` check rejects `debounce:0` (which would otherwise be a
  no-op surprise).
- `_readTimingModifier` is called twice — once per timing kind. O(2N)
  on modifier-list length, fine for typical N < 5.

The `modifiers.includes('once')` check on line 578 is untouched —
`:NNN` is never legal on `once`, and that check still does an exact
match.

The key-filter logic (line 539, `modifiers.filter(m => m in KEY_MODIFIERS)`)
also continues to work: `'enter'`, `'escape'`, etc. don't match
`'debounce:250'` so the filter skips it. No change needed.

#### 2b. JSDoc

Add JSDoc on `_readTimingModifier` per the project rule (CLAUDE.md):

```js
/**
 * @internal
 * @param {string[]} modifiers
 * @param {'throttle' | 'debounce'} name
 * @returns {number} interval in ms, or 0 if not present
 */
function _readTimingModifier(modifiers, name) { ... }
```

**Tests:**

Add to `runtime/template.test.js`:

Modifier-parse assertions (inspect `_template.parts`):
- `@input.debounce=${h}` → `modifiers: ['debounce']`.
- `@input.debounce:250=${h}` → `modifiers: ['debounce:250']`.
- `@click.prevent.throttle:500=${h}` → `modifiers: ['prevent', 'throttle:500']`.

Behavior assertions (use `setTimeout` faking via the test runner's clock
helper if available, otherwise use real timers with a small tolerance —
the existing test suite already exercises `setTimeout` in some places;
match that pattern):
- Bare `.debounce` fires after ~100ms.
- `.debounce:250` fires after ~250ms; the difference is observable.
- `.throttle:500` allows at most one call per 500ms window.
- `commit()` of `@input.debounce:abc=${h}` throws an Error whose message
  contains `'debounce:abc'`.
- `commit()` of `@input.debounce:=${h}` throws.
- `commit()` of `@input.debounce:-5=${h}` throws.
- `commit()` of `@input.debounce:0=${h}` throws.
- `commit()` of `@input.debounce:1.5=${h}` throws.
- Other modifiers with a colon — verify the runtime ignores them in the
  timing path (key filters, etc., aren't affected). E.g., `@click.prevent`
  combined with `.debounce:300` works correctly; `prevent` is unaffected
  by anything `:NNN`-shaped.

Run `cargo run -p zero -- test template.test.js`.

---

### Step 3: T02 lint rule accepts `:NNN` on timing modifiers only

**Goal:** Allow `@input.debounce:250` through T02 without false-positive
errors, but flag `:NNN` on any other modifier (`prevent:200`, `enter:50`)
and flag malformed suffixes (`debounce:`, `debounce:abc`).

**Files:**
- `crates/zero-lint/src/js/rules/t02_event_modifier.rs` — regex update
  and per-segment validation.

**Changes:**

#### 3a. Regex update

Current:

```rust
Regex::new(r"@(\w+)((?:\.\w+)+)\s*=").unwrap()
```

`\w` excludes `:`, so `@input.debounce:250=` doesn't match. New regex
accepts an optional `:<digits>` after each segment:

```rust
Regex::new(r"@(\w+)((?:\.\w+(?::\d+)?)+)\s*=").unwrap()
```

This still requires at least one modifier (`(?:…)+`), still anchors on
`@` and `=`, still captures the event name and the dot-prefixed modifier
group, and now permits one numeric tail per segment.

#### 3b. Per-segment validation

In the existing `for seg in modifiers_text.split('.')` loop (line 58),
the `seg` string for `debounce:250` is now `"debounce:250"`. Split each
segment on `:` and validate:

```rust
// Skip empty segs (leading dot artifacts), as today.
if seg.is_empty() { cursor += 1; continue; }

let (base, suffix_opt) = match seg.split_once(':') {
    Some((b, s)) => (b, Some(s)),
    None => (seg, None),
};

// 1. Base modifier name must be in the allow-list.
let base_ok = ALLOWED_MODIFIERS.contains(&base);

// 2. Suffix, if present, must be valid digits and base must be a timing modifier.
let timing_modifiers: &[&str] = &["throttle", "debounce"];
let suffix_ok = match suffix_opt {
    None => true,
    Some(s) => timing_modifiers.contains(&base)
              && !s.is_empty()
              && s.chars().all(|c| c.is_ascii_digit()),
};

if !base_ok || !suffix_ok {
    // emit diagnostic at the dot of the segment, message text below
}
```

Diagnostic messaging:
- `!base_ok` (e.g. `.foo`) — keep the existing "unknown event modifier
  `.foo`" message.
- `!suffix_ok` (e.g. `.prevent:200`, `.debounce:`, `.debounce:abc`) —
  new message: "invalid modifier `.{seg}` — `:<ms>` suffix is only
  valid on `.throttle` / `.debounce` with positive integer milliseconds
  (e.g. `.debounce:250`)".

The `property` field on the diagnostic should be `.{seg}` (the full
segment including any suffix) so the column-pointer behavior and the
test-friendly property name stay consistent with the existing diag
shape.

`cursor += seg.len() + 1` still works — `seg` is the full segment
including any `:NNN`, so the byte arithmetic is correct.

#### 3c. ALLOWED_MODIFIERS unchanged

The list at line 18–21 stays as `prevent stop once throttle debounce
enter escape space tab up down left right` — the base names. The
`:NNN` syntax is layered on, not added to the allow-list.

**Tests:**

Add to the existing `#[cfg(test)] mod tests` block (lines 90–162):

- `@input.debounce:250` — no diagnostic.
- `@input.throttle:500` — no diagnostic.
- `@input.debounce` (bare) — no diagnostic (already covered by sibling
  tests; add only if not redundant).
- `@click.prevent.debounce:300` — no diagnostic on either modifier.
- `@input.debounce:` — diagnostic with property `.debounce:`.
- `@input.debounce:abc` — diagnostic with property `.debounce:abc`.
- `@click.prevent:200` — diagnostic with property `.prevent:200`.
- `@keydown.enter:50` — diagnostic with property `.enter:50`.
- `@click.foo:1` — diagnostic with property `.foo:1` (bad base; suffix
  isn't reached but the segment is whole).

Run `cargo test -p zero-lint`.

---

### Step 4: Docs update for both fixes

**Goal:** Document the new capabilities so users discover them. Without
this step, the friction-log entries persist for the next adopter.

**Files:**
- `docs/templates.md` — attribute binding and event modifiers sections.

**Changes:**

#### 4a. Attribute binding section (current lines 73–96)

After the existing examples, add a paragraph and example:

> Static text and placeholders mix freely inside a single attribute
> value:
>
> ```ts
> html`<span class="chip chip--${status} active">${label}</span>`
> html`<div style="color: ${color}; padding: ${pad}px;">…</div>`
> ```
>
> Any number of `${…}` substitutions can appear alongside static
> characters in one attribute. The framework joins the pieces — every
> reactive value in the attribute is tracked by a single effect, so the
> attribute re-renders once per change, not once per substitution.

Add a note that boolean / null / undefined semantics only apply when the
attribute is **just** a placeholder (`disabled=${flag}`) — in a concat
context, `null` and `undefined` render as empty strings.

#### 4b. Event modifiers section (current lines 113–140)

Update the "Timing" row of the modifier table:

| Family   | Modifiers                                                                                       |
|----------|-------------------------------------------------------------------------------------------------|
| Timing   | `.throttle` / `.throttle:<ms>` (default 100 ms), `.debounce` / `.debounce:<ms>` (default 100 ms) |

Add an example after the existing modifier examples block:

```ts
html`<input @input.debounce:250=${onSearch} />`
html`<div @scroll.throttle:500=${onScroll}>…</div>`
```

And one sentence: "The `:<ms>` suffix is only valid on `.throttle` and
`.debounce`; T02 flags it elsewhere. Malformed intervals (`:abc`, `:0`,
`:-5`) are runtime errors."

**Tests:** N/A — docs change. Run `cargo run -p zero -- test` once more
to confirm nothing broke.

---

## Risks and Assumptions

- **Test-runner clock for timing tests.** Step 2's tests for
  `.debounce:250` vs `.debounce` need millisecond resolution. If the
  `zero test` harness lacks a fake-clock helper, the tests fall back to
  real `setTimeout` with tolerance bounds (e.g. assert ≥230ms and ≤320ms
  for a nominal 250ms debounce). This is flaky on slow CI hardware but
  acceptable here. If flakiness appears, switch to inspecting the
  `_readTimingModifier` return value directly via a small exported test
  hook — defer that until needed.
- **Parser edge case: empty static fragment between adjacent
  placeholders.** `class="${a}${b}"` should produce
  `statics: ['', '', '']` — the empty static between `${a}` and `${b}`
  is preserved. Verify this case explicitly in Step 1 tests since the
  parser logic appends `currentAttrValue` (empty string) at each
  placeholder boundary.
- **Pre-walk targets array.** Today `commit()` builds `targets` via
  `parts.map(part => _walkPath(...))` (line 594). This still works
  unchanged — one target per part, regardless of placeholder count.
- **JSDoc compliance.** CLAUDE.md requires all JS to be fully
  JSDoc-annotated. The new helpers (`flushAttr`, `_setJoinedAttr`,
  `_coerceConcatValue`, `_readTimingModifier`) all need `@param` /
  `@returns` / `@internal`. The lint rule `R03` may flag missing JSDoc;
  run `zero lint` (per CLAUDE.md the project uses it) as part of
  finishing Step 1 and Step 2 to confirm.
- **No bundler / transpile changes required.** Both fixes are pure
  runtime + lint. The `zero-bundler` and `zero-transpile` crates don't
  see template internals.
- **Backward compatibility on `flushStaticAttr` removal.** That function
  is internal to `_parseTemplate` (a nested `function` declaration on
  line 70 of `runtime/template.js`); there's no external caller. Safe
  to rename to `flushAttr` with extended behavior.
- **Lint regex `\w+` greedy boundaries.** The updated regex
  `@(\w+)((?:\.\w+(?::\d+)?)+)\s*=` matches the longest segment then
  optional `:<digits>`. Confirmed mentally against `@a.b:1.c:2=` →
  segments are `b:1` and `c:2`. If the regex engine backtracks
  unexpectedly on pathological inputs, add a focused regex test
  separate from the diagnostic tests.
