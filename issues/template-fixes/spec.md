# Spec: Template Fixes — Attribute Interpolation & Configurable Timing Modifiers

## Problem Statement

Two template-system bugs surfaced while building a real app on top of zero
(see `~/Documents/code/zero_demo/FRAMEWORK_NOTES.md`):

1. **🔴 Partial-string attribute interpolation silently produces wrong DOM.**
   `html\`<span class="chip chip--${status} active">\`` sets `className` to
   the value of `status` alone — the static `chip chip--` prefix and ` active`
   suffix are silently dropped. Only the full-attribute form
   `class=${expr}` works. There is no error, no lint warning, and the
   resulting CSS class string is just wrong. This is a footgun: the most
   natural way to compose a class string (text + dynamic flag) silently
   produces the wrong DOM, and the misbehavior is shaped exactly like a
   passing render.

2. **🟡 `.debounce` and `.throttle` event modifiers are hard-coded to 100ms.**
   Documented as 100ms but no syntax exists to change it. 100ms is wrong for
   the most common use case (search-input debounce typically wants 200–300ms).
   Users have to bypass the modifier entirely and roll a `setTimeout`-inside-
   `effect()` to get any other interval.

Both bugs live in `runtime/template.js`. Together they're a coherent
template-parser pass: one fixes attribute-value tokenization, the other
extends event-modifier tokenization to accept a numeric argument.

## Background

### Current parser shape (`runtime/template.js`)

`_parseTemplate` is a character-level state machine that walks every
character of every static string in the tagged-template `strings` array. At
each boundary between `strings[i]` and `strings[i+1]` — where a `${}`
placeholder sits — it emits one entry into the `parts: Part[]` array based
on the parser's current state.

Part shapes today (line 219–225 of the existing `template-system` plan):

```js
{ type: 'attr',  path, name }
{ type: 'event', path, event, modifiers }
{ type: 'ref',   path }
{ type: 'node',  path }
```

At commit time (line 588–610), each part is wired into the cloned fragment.
`_commitAttr` (line 332–340) just calls `_applyAttr(el, name, value)`, which
calls `setAttribute(name, String(v))` with no awareness of any
static-string prefix or suffix.

### Why the class= bug happens

When the parser is in `ATTR_VALUE_DQ` (line 196) and reaches the end of the
static string (a placeholder boundary), it falls into the
`ATTR_VALUE_DQ`/`SQ`/`UNQUOTED` branch at line 252–272. There it emits the
`attr` part and **clears `currentAttrValue`** (line 268), losing the
already-collected prefix. After the placeholder, parsing resumes inside
`ATTR_VALUE_DQ`, accumulates the suffix into `currentAttrValue`, hits the
closing `"`, and `flushStaticAttr()` runs — but by then the part has
already been emitted and the suffix gets written to the static attribute
**which was never set on the element to begin with**.

The end result: `setAttribute('class', '')` is called by `flushStaticAttr`
for the suffix (silently, because the static value is non-empty); then
`_commitAttr` runs at commit time and overwrites it with just the dynamic
value. The static prefix is gone entirely.

### Why `.debounce` is hard-coded

`_wrapEventHandler` (line 538–556) hard-codes both intervals to 100ms:

```js
const throttleMs = modifiers.includes('throttle') ? 100 : 0;
const debounceMs = modifiers.includes('debounce') ? 100 : 0;
```

`modifiers` is a `string[]` produced by splitting the attribute name on `.`
at line 258 (`currentAttrName.slice(1).split('.')`). Today every segment
after the event name is treated as a flag — there's no numeric-argument
channel.

### Adjacent surfaces touched

- **`docs/templates.md`** — the "Valid substitution values" table and the
  event-modifier table both have rows that misrepresent current behavior:
  there is no working example of mid-attribute interpolation, and the
  timing-modifier rows say "100 ms" with no syntax for overriding it.
- **`crates/zero-lint/src/js/rules/t02_event_modifier.rs`** — flags typos
  in modifier names. It will need to learn that `throttle:NNN` and
  `debounce:NNN` are valid (and that `:NNN` is only valid on those two).
- **`runtime/template.test.js`** — the existing test file is where new
  parser and commit tests land.

### Design context (decided in scoping)

- **Mixed concatenation in scope.** Any number of placeholders mixed with
  static text in one attribute value must work, e.g.
  `class="chip ${size} chip--${status} ${extra} active"`. Not just one
  placeholder with prefix/suffix. The parser stays general — no
  one-vs-many surprise edge case later.
- **Timing-modifier syntax: colon-suffix.** `@input.debounce:250` reads as
  one modifier, scans cleanly, and is one new character for users to
  learn. Vue uses `.250` (dot-numeric), but T02 then has to special-case
  numeric segments and visually `enter.250` blends with `enter.escape`.
  `(250)` would force the parser to handle parens inside attribute names.
- **Default delay stays 100ms.** Backward-compatible. New users who want
  250ms add `:250` explicitly. The friction-log complaint was about the
  *absence of an override*, not about the default itself.

## Requirements

### R1 — Multi-placeholder attribute concatenation (parser)

The `attr` part type gains a `statics: string[]` field describing the
inter-placeholder static fragments for that attribute. For a parsed
attribute value with N placeholders, `statics.length === N + 1`. Examples:

| Source                                              | Part `name` | Part `statics`            | Number of values consumed |
|-----------------------------------------------------|-------------|---------------------------|---------------------------|
| `class=${x}`                                        | `class`     | `['', '']`                | 1                         |
| `class="chip chip--${status} active"`               | `class`     | `['chip chip--', ' active']` | 1                      |
| `class="${a} ${b}"`                                 | `class`     | `['', ' ', '']`           | 2                         |
| `class="card ${a} pad ${b}"`                        | `class`     | `['card ', ' pad ', '']`  | 2                         |
| `style="color: ${color}; padding: ${pad}px;"`       | `style`     | `['color: ', '; padding: ', 'px;']` | 2               |

When an attribute has multiple placeholders, the parser emits **one**
`attr` part covering all of them, not one per placeholder. The part
descriptor records the count of values it consumes so commit can pull the
right slice from the values array. (Implementation note: this means the
existing 1:1 mapping between `parts.length` and `values.length` no longer
holds — that has commit-side consequences below.)

The same generalization applies inside `ATTR_VALUE_DQ`, `ATTR_VALUE_SQ`,
and `ATTR_VALUE_UNQUOTED`.

### R2 — Multi-placeholder attribute commit

`_commitAttr` is updated to take the part descriptor (with `statics` and
value-count) and a **slice** of the values array — not a single value.

Commit semantics:

- If all values are non-reactive primitives, interleave statics and
  values, join, and `setAttribute(name, joined)`.
- If **any** value in the slice is a `Signal` or a reactive function, wrap
  the entire attribute write in a single `effect(() => …)` that reads each
  reactive value (`.val` or `fn()`), joins, and writes. One effect per
  attribute, not one per placeholder.
- Boolean / null / undefined semantics from the existing `_applyAttr`
  table apply **only** when the entire attribute is a single placeholder
  with empty surrounding statics (`statics === ['', '']` and one value).
  For multi-placeholder attributes, every value is coerced to string via
  `String(v)` and `null`/`undefined` render as `""`. (Boolean-attribute
  semantics on a concatenated value would be ambiguous; concatenated
  attributes are inherently stringy.)

This preserves the existing single-placeholder behavior exactly — the
`['', '']` case is the same code path with a trivial join.

### R3 — Event-modifier numeric argument syntax

Modifier parsing accepts an optional `:NNN` suffix on `throttle` and
`debounce` only. The numeric portion must match `/^\d+$/` (integer
milliseconds). Examples:

| Source                          | Event   | Modifiers                          |
|---------------------------------|---------|------------------------------------|
| `@input.debounce=${h}`          | `input` | `['debounce']` (legacy 100ms)      |
| `@input.debounce:250=${h}`      | `input` | `['debounce:250']`                 |
| `@scroll.throttle:500=${h}`     | `scroll`| `['throttle:500']`                 |
| `@click.prevent.debounce:300=…` | `click` | `['prevent', 'debounce:300']`      |

Parser change: at line 258, the existing `split('.')` already produces
segments like `'debounce:250'`. No change to the splitter is needed —
the segment is preserved verbatim into the part descriptor.

### R4 — Event-modifier argument applied at commit

`_wrapEventHandler` interprets a modifier of the form `throttle:NNN` /
`debounce:NNN` by parsing the suffix and using it instead of 100. The
existing flag-style modifiers (`throttle`, `debounce` with no suffix) keep
the 100ms default unchanged.

Replace:

```js
const throttleMs = modifiers.includes('throttle') ? 100 : 0;
const debounceMs = modifiers.includes('debounce') ? 100 : 0;
```

with logic that scans modifiers for a `throttle` or `throttle:NNN` entry
and likewise for `debounce`. If a modifier is malformed (`debounce:` /
`debounce:abc` / `debounce:-5`), the framework throws at commit time with
a clear message — these will never have been valid, so failing loud is
correct.

The `modifiers.includes('once')` check on line 578 must continue to work
unchanged — `:NNN` is only legal on `throttle` and `debounce`.

### R5 — T02 lint rule update

`crates/zero-lint/src/js/rules/t02_event_modifier.rs` learns the new
syntax:

- For modifier strings, strip an optional `:<digits>` suffix before
  validating against the known-modifier set.
- The `:NNN` suffix is **only valid** on `throttle` and `debounce`. Any
  other modifier with a colon-suffix is a lint error
  (e.g. `prevent:200` is flagged, `enter:50` is flagged).
- `debounce:` (empty number) and `debounce:abc` (non-numeric) are lint
  errors.

The rule's existing typo-detection behavior on the base modifier name is
unchanged.

### R6 — Docs

`docs/templates.md`:

- **Attribute binding section** (line 73–96): add an example of
  partial-string interpolation, e.g.
  `html\`<span class="chip chip--${status} active">…</span>\``, and call
  out that any number of placeholders mixed with static text in one
  attribute value works.
- **Event modifiers section** (line 113–140): update the "Timing" row
  to show the `:NNN` syntax and document the 100ms default when no
  suffix is given. Add an example:
  `html\`<input @input.debounce:300=${onSearch} />\``.

### R7 — Tests

`runtime/template.test.js`:

- Parser tests for the new `statics` field on `attr` parts: each row of
  the table in R1 has a corresponding parse test inspecting `_template.parts`.
- Commit tests: static-only mixed (`class="a ${'x'} b"` → `class="a x b"`),
  signal + static (`class="prefix ${s} suffix"` — `s.set('y')` updates the
  whole class string), two signals in one attribute (one effect, both
  contribute), and reactive-function value inside concat.
- Event-modifier parse tests: `@input.debounce=${h}` keeps `['debounce']`,
  `@input.debounce:250=${h}` produces `['debounce:250']`, mixed-modifier
  ordering works.
- Event-modifier commit tests: a `debounce:250` handler fires after ~250ms
  (use a fake clock or whatever the existing test runner provides), a
  bare `debounce` handler fires after ~100ms, and `debounce:abc` throws
  at commit.

`crates/zero-lint/tests/`:

- T02 rule tests: `debounce:250` passes, `debounce:` fails, `debounce:abc`
  fails, `prevent:200` fails, `enter:50` fails, unknown bare modifier
  still fails.

## Constraints

- No npm dependencies; same testing harness (`zero test`) as the rest of
  the runtime.
- Parser stays single-pass / character-level. No move to a tokenizer +
  AST split.
- Backward compatibility:
  - Every currently-working `class=${x}` site keeps working bit-for-bit
    (same `setAttribute` call, same boolean/null/undefined semantics).
  - Every currently-working `.debounce` / `.throttle` site keeps the
    100ms timing.
  - The `attr` part shape gains a field — it does not change names of
    existing fields. Any external code reading parts is internal-only
    (`@internal`), but the patch is still purely additive on attr.
- Performance: an attribute with N reactive placeholders uses **one**
  `effect()`, not N. The whole attribute re-evaluates on any change.
  This matches what a user would write by hand
  (`<div class=${() => \`a ${s.val} b\`}>`).
- Error messages: malformed `:NNN` is a hard throw at commit, not a
  silent fallback. The point of the fix is that template bugs stop being
  silent.

## Out of Scope

- Adding `:NNN` to any modifier other than `throttle` and `debounce`. The
  spec mentions `enter:50` only as a lint failure case.
- Boolean-attribute semantics on concatenated attribute values. Mixed
  values are always coerced to string.
- HTML entity decoding inside static attribute fragments. The existing
  parser doesn't decode entities in text nodes; we don't add it for
  attributes.
- Partial-string interpolation in **event-handler attribute values**
  (`@click="foo ${bar}"`). Event values are always whole expressions;
  the parser already throws on partial event-value placeholders and
  that stays as-is.
- Partial-string `ref=` (`ref="x ${y}"`). Same — `ref` always takes a
  single object value.
- Compile-time pre-evaluation of attribute concatenation (would belong
  in a future bundler/transpile phase).
- Configurable default delay via `zero.toml` or similar global config.

## Open Questions

- **Path-walk on multi-placeholder attrs.** Today every part has a `path`
  to its target node, and `commit()` walks once per part. If one `attr`
  part now covers multiple placeholders, the existing `parts.length ===
  values.length` invariant breaks. The plan phase decides whether to:
  (a) keep `parts.length !== values.length` and record a `valueCount` on
  each part, or (b) keep `parts.length === values.length` and have N
  consecutive `attr` parts with the same `path` and a `groupId`. Option
  (a) reads cleaner; option (b) preserves the current loop shape. Both
  work — pick during planning.
- **Effect cleanup on attribute update.** Each commit-time `effect()`
  registers with the active scope. The N-placeholder attribute uses one
  effect, so cleanup is unchanged. Worth verifying in tests that a scope
  dispose still tears down the wrapped effect for multi-placeholder
  attrs.
- **Lint T02 grammar.** The T02 rule currently splits on `.` and
  validates each segment. With `:NNN` suffixes, the simplest change is
  to strip `:<digits>` before lookup; the planner should confirm this
  doesn't trip the existing tokenizer used by the rule.
