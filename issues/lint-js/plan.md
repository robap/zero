# Plan: JS/TS framework-idiom lint

## Summary

Add a JS/TS pass to `zero lint` that flags ten framework-idiom violations
(R01–R03, T01–T04, C01–C02, I01–I02, S01) plus a P01 parse-error rule.
The pass parses each `.ts` / `.js` / `.tsx` / `.jsx` file under
`<project_root>/src/` via `zero-transpile`'s SWC parser, runs one
`Visit`-style rule per check, and emits diagnostics through the existing
`Diagnostic` struct so they interleave with SCSS output in the existing
sort order. No new top-level dependencies, no autofix, no per-line
suppression — the rule list is hard-coded and opinionated.

## Prerequisites

The spec's six open questions are resolved as follows. Each step below
encodes these decisions; no further user input is needed.

1. **R03 store exemption** — hard-coded `src/stores/**` directory match.
   No `zero.toml` knob. Documented in the rule message and in
   `AGENTS.md` (Step 16).
2. **C01 scope** — narrow per spec text: fires only in
   `src/components/**` and `src/routes/**`. Other directories may use
   classes. (Test files are NOT exempt from C01.)
3. **T02 allowed modifier set** — sourced from `runtime/template.js`
   reality, not from spec text. Allowed: `prevent`, `stop`, `once`,
   `throttle`, `debounce`, `enter`, `escape`, `space`, `tab`, `up`,
   `down`, `left`, `right`. (Spec mentioned `self`, `passive`, `capture`,
   `delete` — those are NOT in the runtime and would produce silent
   no-ops, so the lint rejects them. The rule's source file gets a doc
   comment pointing at `runtime/template.js` so future runtime additions
   are reflected here.)
4. **I01 allowlist** — exact set sourced from the bundler resolver
   (`crates/zero-bundler/src/resolver.rs`) plus the test runner
   specifier: `"zero"`, `"zero/components"`, `"zero/http"`, `"zero/test"`.
   Hard-coded const in the rule module; comment cites the resolver as
   source of truth. (Spec's broader `z` / `z/wc` / `zero/wc` set is not
   wired up in the resolver yet — those specifiers would fail at build
   time, so flagging them here too is the right call.)
5. **S01 counting** — body span is the line of the opening `{` through
   the line of the matching `}`, inclusive. The rule message names the
   exact rule (`"<n> lines (open brace to close brace inclusive); zero
   targets <= 80"`). Arrow functions with expression bodies (no braces)
   are excluded.
6. **Diagnostic struct** — repurpose the existing `property` / `value`
   fields rather than extending `Diagnostic`. `property` carries the
   offending construct text (e.g. `count.val`, `addEventListener`,
   `each(items, render)`); `value` carries a short category label
   (e.g. `template`, `assignment`, `import`, `module-scope`). The
   existing `write_diag` helper in `crates/zero/src/cmd/lint.rs` renders
   this without changes.

## Steps

- [x] **Step 1: Expose AST parsing from `zero-transpile`**
- [x] **Step 2: Add JS/TS pipeline scaffolding to `zero-lint` (walker, parse, P01, integration)**
- [x] **Step 3: Implement R01 — `${signal.val}` inside `html\`\``**
- [x] **Step 4: Implement R02 — assignment to `signal.val`**
- [x] **Step 5: Implement R03 — module-level `signal/computed/effect`**
- [x] **Step 6: Implement T01 — `addEventListener` / `removeEventListener` in components/routes**
- [x] **Step 7: Implement T03 — `each(items, render)` without a key fn**
- [x] **Step 8: Implement T04 — direct DOM access in components/routes**
- [x] **Step 9: Implement T02 — unknown `@event.modifier` in templates**
- [x] **Step 10: Implement C01 — class declaration in components/routes**
- [x] **Step 11: Implement C02 — `customElements.define(...)` call**
- [x] **Step 12: Implement I01 — unknown import specifier**
- [x] **Step 13: Implement I02 — relative import into `.zero/`**
- [x] **Step 14: Implement S01 — function body > 80 lines**
- [x] **Step 15: End-to-end fixture + agent-failure regression + clean-examples verification**
- [x] **Step 16: Documentation (`zero-framework-spec.md`, `AGENTS.md`)**

---

## Step Details

### Step 1: Expose AST parsing from `zero-transpile`

**Goal:** Give `zero-lint` a way to parse a TS/JS file into an SWC
`Module` AST plus the SWC `SourceMap` needed to translate `Span` ↔
`line/column`, without duplicating the parser config or pulling SWC
directly into `zero-lint` for the parse step. Existing
`transpile_typescript()` is refactored to call the new parse function so
there's still one parse path.

**Files:**
- `crates/zero-transpile/src/lib.rs` (modify)

**Changes:**
- Add a new public function:
  ```rust
  pub fn parse_module(
      source: &str,
      filename: &str,
  ) -> Result<ParsedModule, TranspileError>;
  ```
  where
  ```rust
  pub struct ParsedModule {
      pub module: swc_core::ecma::ast::Module,
      pub source_map: swc_core::common::sync::Lrc<swc_core::common::SourceMap>,
  }
  ```
- Internally, `parse_module` builds the `Lrc<SourceMap>`, calls the
  existing `parse_ts_source` helper, and returns both the parsed
  `Module` and the `SourceMap`. The existing `transpile_typescript`
  continues to work — refactor it to call `parse_module` first, then
  run `strip_types` / `emit_js` on `module` using `source_map`.
- Re-export the SWC subset that callers need:
  ```rust
  pub use swc_core::ecma::ast;
  pub use swc_core::common::{BytePos, Span, sync::Lrc, SourceMap};
  ```
  This keeps `zero-lint` from importing `swc_core` directly for the
  parse and span-conversion surface. (Rule visitors still need
  `swc_core::ecma::visit`, which is added as a direct dep on
  `zero-lint` in Step 2 — re-exporting the entire `visit` module from
  `zero-transpile` is awkward because of macro definitions.)

**Tests:**
- `parse_module_returns_ast_and_source_map` — parses `const x: number =
  1;`, asserts the returned module has one item and source-map lookup
  on its span yields line 1.
- `parse_module_surfaces_parse_error_with_position` — parses
  `const x: = ;`, asserts the returned `TranspileError` has line ≥ 1
  and a non-empty message.
- `transpile_typescript_still_works` — sanity check; the existing test
  `strips_simple_type_annotations` already covers this and must
  continue to pass.

---

### Step 2: Add JS/TS pipeline scaffolding to `zero-lint`

**Goal:** Stand up the JS/TS lint pipeline end-to-end with no rules
beyond P01 (parse error). After this step, `lint_project()` discovers
JS files, parses each one, surfaces parse errors as `P01` diagnostics,
and produces empty output for valid files. Each subsequent step adds
one rule module.

**Files:**
- `crates/zero-lint/Cargo.toml` (modify)
- `crates/zero-lint/src/lib.rs` (modify)
- `crates/zero-lint/src/js/mod.rs` (new)
- `crates/zero-lint/src/js/walk.rs` (new)
- `crates/zero-lint/src/js/context.rs` (new)
- `crates/zero-lint/src/js/diag.rs` (new)
- `crates/zero-lint/src/js/parse_error.rs` (new — P01)
- `crates/zero-lint/src/js/rules/mod.rs` (new, initially empty)

**Changes:**

1. **Cargo.toml** — add deps:
   - `zero-transpile = { path = "../zero-transpile" }`
   - `swc_core = { workspace = true }`

2. **`js/walk.rs`** — `pub fn user_js_files(root: &Path) -> Vec<PathBuf>`:
   - Build an `ignore::WalkBuilder` rooted at `root.join("src")` (gated:
     return empty if `root.join("src")` doesn't exist — this happens in
     framework-internal directories like `runtime/` that aren't user
     projects).
   - Honors `.gitignore`. Skips `.zero/`, `dist/`, `node_modules/`,
     `coverage/`, `mutation/`, `target/` via the same override patterns
     as `walk::user_scss_files`.
   - Yields files whose extension is one of `ts`, `tsx`, `js`, `jsx`
     (case-insensitive).
   - **Test files are returned by the walker** — per-rule scope decides
     whether to flag them. (Spec R3 says tests are exempt from T-rules
     and R03 but still covered by R02/C01/C02/I01/I02/S01.)

3. **`js/context.rs`** — `pub struct FileCtx`:
   ```rust
   pub struct FileCtx<'a> {
       pub file: &'a Path,
       pub source: &'a str,
       pub root: &'a Path,
       pub source_map: Lrc<SourceMap>,
       pub module: ast::Module,
       pub is_test_file: bool,
       pub is_under_components_or_routes: bool,
       pub is_under_stores: bool,
       /// Names imported from "zero" (e.g. `signal`, `effect`, aliased
       /// names included). Populated once per file.
       pub zero_imports: HashSet<String>,
   }
   ```
   - `is_test_file` — matches `*.test.{ts,js,tsx,jsx}` or
     `*.spec.{ts,js,tsx,jsx}` on the file's basename.
   - `is_under_components_or_routes` — true if the file's path under
     `<root>/src/` starts with `components/` or `routes/`.
   - `is_under_stores` — true if it starts with `stores/`.
   - `zero_imports` — populated by scanning `module.body` for
     `ImportDecl`s whose `src.value == "zero"`, recording each
     specifier's local-binding name.

4. **`js/diag.rs`** — helpers to emit `Diagnostic` from an SWC `Span`:
   ```rust
   pub fn diag_at(
       rule: &'static str,
       ctx: &FileCtx,
       lo: BytePos,
       property: impl Into<String>,
       value: impl Into<String>,
       message: impl Into<String>,
   ) -> Diagnostic;
   ```
   Uses `ctx.source_map.lookup_char_pos(lo)` to derive 1-based line +
   1-based column (matching the existing SCSS rules' position scheme,
   and the `write_diag` helper's expectations).

5. **`js/parse_error.rs`** — `pub fn check(file: &Path, source: &str,
   root: &Path) -> Result<FileCtx, Diagnostic>`:
   - Calls `zero_transpile::parse_module(source, file_str)`.
   - On success, builds and returns a `FileCtx`.
   - On error, returns `Err(Diagnostic { rule: "P01", file, line,
     column, property: "", value: "parse", message: "<swc error
     text>" })`.

6. **`js/mod.rs`** — `pub fn lint_js_file(file: &Path, source: &str,
   root: &Path) -> Vec<Diagnostic>`:
   - Call `parse_error::check`. If `Err(d)`, return `vec![d]`.
   - If `Ok(ctx)`, call each rule module's `check(&ctx)` (none in this
     step), aggregate, return.

7. **`lib.rs::lint_project`** — extend to also walk JS files:
   - After the existing SCSS loop, run:
     ```rust
     for file in js::walk::user_js_files(root) {
         if let Ok(source) = std::fs::read_to_string(&file) {
             out.extend(js::lint_js_file(&file, &source, root));
         }
     }
     ```
   - The existing final sort by `(file, line, column, rule)` already
     handles the mixed stream.

**Tests:**
- `js/walk.rs` unit tests:
  - `walker_yields_ts_js_tsx_jsx_under_src` — fixtures inside a temp
    dir; assert each extension found.
  - `walker_excludes_dot_zero_dist_node_modules_target` — variations
    on the existing SCSS walker test.
  - `walker_returns_empty_when_no_src_dir` — root with no `src/`.
  - `walker_yields_test_and_spec_files` (per-rule filtering happens
    later — walker returns everything).
- `js/parse_error.rs` unit tests:
  - `valid_file_returns_ctx` — `export const x = 1;` parses cleanly,
    no diagnostic.
  - `parse_error_emits_p01` — `const x: = ;` returns one `P01`
    diagnostic with `line >= 1`, `column >= 1`, non-empty message.
- `js/mod.rs` unit test:
  - `clean_file_emits_no_diagnostics` — a valid module with no rules
    registered yet produces zero diagnostics.
  - `parse_error_short_circuits` — a syntactically broken file yields
    exactly one `P01` diagnostic.

---

### Step 3: Implement R01 — `${signal.val}` inside `html\`\``

**Goal:** Flag `.val` reads inside `html\`\`` tagged-template
substitutions, the highest-frequency drift mode (silently breaks
reactivity, no runtime signal).

**Files:**
- `crates/zero-lint/src/js/rules/mod.rs` (modify — register module)
- `crates/zero-lint/src/js/rules/r01_template_val_read.rs` (new)
- `crates/zero-lint/src/js/mod.rs` (modify — wire into `lint_js_file`)

**Changes:**
- A `Visit` impl walks the module:
  - On `visit_tagged_tpl(t: &TaggedTpl)`:
    - If `t.tag` is `Expr::Ident(i)` and `i.sym == "html"`:
      - For each `Expr` in `t.tpl.exprs`, if the expression is
        `Expr::Member(m)` with `m.obj == Expr::Ident(_)` and
        `m.prop == MemberProp::Ident(p)` where `p.sym == "val"`, emit
        an R01 diagnostic at `m.span.lo`.
  - Continue recursion to catch nested `html\`\`` (composed templates).
- Diagnostic shape: `property = "<name>.val"`, `value = "template"`,
  `message` = spec wording verbatim.
- **Scope:** all files under `<root>/src/**`. Test files are not exempt
  (R-rules run everywhere).

**Tests:**
- `fires_on_val_read_in_html_template` — `html\`${count.val}\``.
- `does_not_fire_on_signal_pass` — `html\`${count}\``.
- `does_not_fire_outside_html_tag` — `css\`${x.val}\``.
- `does_not_fire_on_val_outside_template` — `const v = count.val;`.
- `fires_in_nested_html_template` — `html\`${html\`${x.val}\`}\``.

---

### Step 4: Implement R02 — assignment to `signal.val`

**Goal:** Flag any `x.val = …` / `x.val += …` etc. anywhere in a file
that imports from `"zero"`. The import guard reduces false positives on
unrelated `.val` shapes in non-zero code.

**Files:**
- `crates/zero-lint/src/js/rules/r02_val_assignment.rs` (new)
- `crates/zero-lint/src/js/mod.rs` (modify — register)

**Changes:**
- Skip the file if `ctx.zero_imports.is_empty()`.
- Visit each `AssignExpr`. If the LHS pattern is
  `AssignTarget::Simple(SimpleAssignTarget::Member(m))` where
  `m.prop == MemberProp::Ident(p)` and `p.sym == "val"`, emit R02 at
  `m.span.lo`.
- `property = "<obj>.val"` (best-effort textual rendering of the LHS;
  if the object is an Ident, use its sym, else use the literal source
  slice between `m.span.lo()` and `m.span.hi()`). `value =
  "assignment"`. Message per spec.
- **Scope:** all files under `<root>/src/**`, including tests (spec R3:
  "Tests are still covered by R02 / C01 / C02 / I01 / I02 / S01").

**Tests:**
- `fires_on_simple_val_assignment` — `count.val = 1` in a file that
  imports `signal` from `"zero"`.
- `fires_on_compound_val_assignment` — `count.val += 1`.
- `does_not_fire_without_zero_import` — same code but no
  `import … from "zero"` line.
- `does_not_fire_on_non_val_assignment` — `count.value = 1`.
- `fires_in_test_file` (regression for R3 exemption text).

---

### Step 5: Implement R03 — module-level `signal/computed/effect`

**Goal:** Flag calls to `signal(…)`, `computed(…)`, `effect(…)` at
module top level (not inside any function/method/arrow). Exempt
`src/stores/**` (the canonical factory location) and `*.test.{ts,js}`.

**Files:**
- `crates/zero-lint/src/js/rules/r03_module_reactive.rs` (new)
- `crates/zero-lint/src/js/mod.rs` (modify — register)

**Changes:**
- Skip if `ctx.is_under_stores || ctx.is_test_file`.
- Skip if `ctx.zero_imports.is_empty()` (defensive — if the file
  doesn't import from `"zero"`, the named call can't be the framework
  primitive).
- Visit with a `function_depth: u32` counter; increment on enter,
  decrement on exit, for: `FnDecl`, `FnExpr`, `ArrowExpr`,
  `Constructor`, `ClassMethod`, `MethodProp`, `GetterProp`,
  `SetterProp`, `PrivateMethod`.
- On `visit_call_expr(c)`:
  - If `function_depth == 0` and the callee is `Expr::Ident(i)` where
    `i.sym in {"signal", "computed", "effect"}` and `i.sym` is in
    `ctx.zero_imports`, emit R03 at `c.span.lo`.
- `property = i.sym.to_string()`, `value = "module-scope"`. Message
  per spec; reference `src/stores/**` as the canonical exemption.

**Tests:**
- `fires_at_top_level` — `const c = signal(0);` outside a function.
- `does_not_fire_inside_function` — `function f(){ signal(0); }`.
- `does_not_fire_inside_arrow` — `const f = () => signal(0);`.
- `does_not_fire_inside_class_method`.
- `does_not_fire_in_stores_directory` — same code under
  `src/stores/foo.ts`.
- `does_not_fire_in_test_file`.
- `does_not_fire_without_zero_import`.
- `respects_import_alias` — `import { signal as makeSig } from "zero"`
  followed by `const c = makeSig(0);` fires under the aliased name.

---

### Step 6: Implement T01 — `addEventListener` / `removeEventListener` in components/routes

**Goal:** Flag direct addEventListener / removeEventListener calls
inside `src/components/**` or `src/routes/**` (templates and `@event=`
are the canonical surface). Test files exempt from T-rules.

**Files:**
- `crates/zero-lint/src/js/rules/t01_event_listener.rs` (new)
- `crates/zero-lint/src/js/mod.rs` (modify — register)

**Changes:**
- Skip unless `ctx.is_under_components_or_routes && !ctx.is_test_file`.
- Visit `CallExpr`. If `callee` is `Callee::Expr(Expr::Member(m))` and
  `m.prop == MemberProp::Ident(p)` where `p.sym in {"addEventListener",
  "removeEventListener"}`, emit T01 at `c.span.lo`.
- `property = "<receiver>.addEventListener"` (or `removeEventListener`),
  `value = "event"`. Message per spec.

**Tests:**
- `fires_on_add_event_listener_in_components`.
- `fires_on_remove_event_listener_in_routes`.
- `does_not_fire_in_lib_directory`.
- `does_not_fire_in_test_file`.

---

### Step 7: Implement T03 — `each(items, render)` without a key fn

**Goal:** Flag `each(...)` calls with exactly two arguments where
`each` is imported from `"zero"`. The two-arg form falls back to
index-based reconciliation and breaks identity for object lists.

**Files:**
- `crates/zero-lint/src/js/rules/t03_each_no_key.rs` (new)
- `crates/zero-lint/src/js/mod.rs` (modify — register)

**Changes:**
- Skip unless `ctx.is_under_components_or_routes && !ctx.is_test_file`.
- Visit `CallExpr`. If the callee is `Expr::Ident(i)`,
  `i.sym in ctx.zero_imports`, the original spec name `each` is the
  identifier (after alias resolution — store both alias→original and
  original→alias maps in `FileCtx::zero_imports_original`, or just
  match the local sym against the set of locals that bind to `each`),
  AND `args.len() == 2`, emit T03 at `c.span.lo`.

  Implementation note: extend `FileCtx::zero_imports` to a richer
  structure if needed. Simplest: store
  `zero_import_local_to_original: HashMap<String, String>` (local
  name → original `zero` export name). R03/T03 look up by original
  name. R02 only checks "any zero import exists", so the set form
  suffices for it; bump to a map in this step or a prior step that
  needs it (do it here when T03 first needs it; backfill R03 to use
  it for the alias case).

- `property = "each(<arg-count>=2)"`, `value = "each"`. Message per
  spec — mention `keyFn`.

**Tests:**
- `fires_on_two_arg_each_in_components`.
- `does_not_fire_on_three_arg_each` — `each(items, render, x => x.id)`.
- `does_not_fire_in_test_file`.
- `does_not_fire_in_lib_directory`.
- `does_not_fire_on_non_zero_each` — `import { each } from "./util";`.

---

### Step 8: Implement T04 — direct DOM access in components/routes

**Goal:** Flag two heuristics: (a) any member access chain rooted at
the global identifier `document` with the DOM-query property names; (b)
mutating method calls (`appendChild`, `removeChild`, `insertBefore`,
`replaceChild`, `innerHTML`) whose immediate receiver is not a `ref`-
shaped `.el` access.

**Files:**
- `crates/zero-lint/src/js/rules/t04_direct_dom.rs` (new)
- `crates/zero-lint/src/js/mod.rs` (modify — register)

**Changes:**
- Skip unless `ctx.is_under_components_or_routes && !ctx.is_test_file`.
- Visit `MemberExpr` and `CallExpr`:
  - **Heuristic (a):** if the member-expression `obj` chain's leftmost
    identifier is the unbound name `document` and the `prop` is one of
    `querySelector`, `getElementById`, `querySelectorAll`, emit T04.
    "Leftmost identifier" = repeatedly unwrap `Expr::Member.obj` until
    a non-member expression remains; if that is `Expr::Ident("document")`
    AND the file does not shadow `document` (i.e. no `let document
    = …` at module or function scope along the path — for v1, ignore
    shadowing; the spec accepts false-positive risk and the syntactic
    rule is the contract).
  - **Heuristic (b):** for `CallExpr` whose callee is a `MemberExpr m`
    with `m.prop` one of `appendChild`, `removeChild`, `insertBefore`,
    `replaceChild`, check the receiver `m.obj`:
    - If `m.obj` is `Expr::Member(inner)` and `inner.prop ==
      MemberProp::Ident(p)` with `p.sym == "el"`, **suppress** (a
      `ref().el.appendChild(...)` pattern).
    - Otherwise emit T04.
  - **`innerHTML` assignment:** `obj.innerHTML = ...` is an
    `AssignExpr` with LHS `MemberExpr.prop = Ident("innerHTML")`. Apply
    the same suppression as (b): suppress when the immediate receiver
    is `<x>.el`; otherwise emit T04.
- `property = "<text-of-construct>"` (e.g. `document.querySelector`,
  `el.appendChild`), `value = "dom"`. Message per spec.

**Tests:**
- `fires_on_document_query_selector`.
- `fires_on_document_get_element_by_id`.
- `fires_on_append_child_without_ref` —
  `containerEl.appendChild(child)`.
- `does_not_fire_on_ref_dot_el_append_child` —
  `myRef.el.appendChild(child)`.
- `fires_on_inner_html_assignment_without_ref`.
- `does_not_fire_in_lib_directory`.
- `does_not_fire_in_test_file`.

---

### Step 9: Implement T02 — unknown `@event.modifier` in templates

**Goal:** Inside `html\`\`` static template parts, every dotted modifier
on `@event` syntax must match the runtime's allowed set
(`prevent`, `stop`, `once`, `throttle`, `debounce`, `enter`, `escape`,
`space`, `tab`, `up`, `down`, `left`, `right`). Unknown modifiers
silently no-op at runtime; this rule surfaces the typo.

**Files:**
- `crates/zero-lint/src/js/rules/t02_event_modifier.rs` (new)
- `crates/zero-lint/src/js/mod.rs` (modify — register)

**Changes:**
- Skip unless `ctx.is_under_components_or_routes && !ctx.is_test_file`.
- Define
  ```rust
  /// SOURCE OF TRUTH: runtime/template.js
  /// (see `_wrapEventHandler` and `KEY_MODIFIERS`).
  const ALLOWED_MODIFIERS: &[&str] = &[
      "prevent", "stop", "once", "throttle", "debounce",
      "enter", "escape", "space", "tab",
      "up", "down", "left", "right",
  ];
  ```
- Visit `TaggedTpl`. When the tag is `Ident("html")`, walk each
  `TplElement` in `t.tpl.quasis`:
  - Scan the element's `raw` text with a regex
    `@\w+(?:\.\w+)+\s*=`. (Use `regex::Regex::new` lazily via
    `once_cell::sync::Lazy` if needed, or just compile per call — n is
    small.)
  - For each match, split the matched prefix on `.`. The first segment
    is the event name (always allowed). For every subsequent dotted
    segment, if it is not in `ALLOWED_MODIFIERS`, emit T02 at
    `quasi.span.lo + offset_in_raw_of_segment`. Use
    `ctx.source_map.lookup_char_pos` to translate.
  - `property = ".<modifier>"`, `value = "modifier"`. Message per spec.
- The quasi-span offset arithmetic is the only finicky bit. Verify by
  writing one test that asserts the column points exactly at the dot
  preceding the bad modifier.

**Tests:**
- `fires_on_unknown_modifier` — `html\`<button @click.foo=${h}/>\``.
- `does_not_fire_on_known_modifier` — `html\`<button
  @click.prevent=${h}/>\``.
- `does_not_fire_on_no_modifier` — `html\`<button @click=${h}/>\``.
- `fires_on_unknown_in_multi_modifier` — `html\`<input
  @keydown.enter.foo=${h}/>\`` flags `.foo` but not `.enter`.
- `column_points_at_the_dot_of_bad_modifier` — assert exact
  `(line, column)`.

---

### Step 10: Implement C01 — class declaration in components/routes

**Goal:** Flag any `class Foo { … }` or `class Foo extends Bar { … }`
declared inside `src/components/**` or `src/routes/**`. Test files NOT
exempt (per spec R3).

**Files:**
- `crates/zero-lint/src/js/rules/c01_no_class_component.rs` (new)
- `crates/zero-lint/src/js/mod.rs` (modify — register)

**Changes:**
- Skip unless `ctx.is_under_components_or_routes`.
- Visit `ClassDecl` and `ClassExpr`. For each, emit C01 at the
  declaration's class-keyword span.
- `property = "class <name>"` (or `class <anonymous>` for class
  expressions without a name), `value = "component-model"`. Message per
  spec.

**Tests:**
- `fires_on_class_declaration_in_components`.
- `fires_on_class_expression_in_routes`.
- `does_not_fire_in_lib_directory`.
- `does_not_fire_in_stores_directory`.
- `fires_in_components_test_file` (regression for C01 not being
  test-exempt).

---

### Step 11: Implement C02 — `customElements.define(...)` call

**Goal:** Flag every call to `customElements.define(...)`. The spec
mentions a `'z/wc'` escape hatch — in practice the runtime has no such
module yet, so v1 flags unconditionally. Message wording references
`'z/wc'` for forward-compatibility.

**Files:**
- `crates/zero-lint/src/js/rules/c02_custom_elements.rs` (new)
- `crates/zero-lint/src/js/mod.rs` (modify — register)

**Changes:**
- **Scope:** all `<root>/src/**`, including tests.
- Visit `CallExpr`. If `callee` is `Expr::Member(m)` with `m.obj ==
  Expr::Ident("customElements")` and `m.prop == Ident("define")`,
  emit C02 at `c.span.lo`.
- `property = "customElements.define"`, `value = "component-model"`.

**Tests:**
- `fires_on_custom_elements_define`.
- `does_not_fire_on_unrelated_member_call`.
- `fires_in_test_file`.

---

### Step 12: Implement I01 — unknown import specifier

**Goal:** Flag every static `import … from "<x>"` and dynamic
`import("<x>")` whose specifier is not in the allowlist and is not a
relative or absolute path.

**Files:**
- `crates/zero-lint/src/js/rules/i01_unknown_specifier.rs` (new)
- `crates/zero-lint/src/js/mod.rs` (modify — register)

**Changes:**
- **Scope:** all `<root>/src/**`.
- Define:
  ```rust
  /// SOURCE OF TRUTH: crates/zero-bundler/src/resolver.rs + runtime/test.js.
  const ALLOWED_BARE_SPECIFIERS: &[&str] = &[
      "zero",
      "zero/components",
      "zero/http",
      "zero/test",
  ];
  ```
- Visit `ImportDecl`: take `decl.src.value` as the specifier string.
- Visit `CallExpr` whose callee is `Callee::Import(_)` and whose first
  argument is a string literal: take that literal's value.
- A specifier is OK if it starts with `"./"`, `"../"`, or `"/"`, or
  matches an entry in `ALLOWED_BARE_SPECIFIERS` exactly. Otherwise
  emit I01.
- `property = "<specifier>"`, `value = "import"`. Message per spec
  (mention "no node_modules").

**Tests:**
- `fires_on_npm_bare_specifier` — `import x from "lodash";`.
- `fires_on_node_protocol` — `import fs from "node:fs";`.
- `fires_on_npm_protocol` — `import x from "npm:left-pad";`.
- `does_not_fire_on_zero` / `does_not_fire_on_zero_components` /
  `does_not_fire_on_zero_http` / `does_not_fire_on_zero_test`.
- `does_not_fire_on_relative_path` — `import x from "./util.ts";`.
- `fires_on_dynamic_import_npm` — `await import("lodash")`.

---

### Step 13: Implement I02 — relative import into `.zero/`

**Goal:** Flag relative import specifiers whose resolved path lands
under `<root>/.zero/`. The user must reach `.zero/` through the public
surface (`"zero"`, `"zero/components"`, etc.) only.

**Files:**
- `crates/zero-lint/src/js/rules/i02_dot_zero_import.rs` (new)
- `crates/zero-lint/src/js/mod.rs` (modify — register)

**Changes:**
- **Scope:** all `<root>/src/**`.
- For each `ImportDecl` and dynamic-`import("...")` whose specifier
  starts with `"./"` or `"../"`:
  - Compute `target = canonicalize(file.parent().join(specifier))`. If
    canonicalization fails (the spec is willing to live with that —
    bundler will surface unresolvable imports anyway), skip.
  - Compute `root_dot_zero = canonicalize(root.join(".zero"))`. If
    `root_dot_zero` doesn't exist, skip the rule entirely for this
    file (clean projects with no `.zero/` directory shouldn't error).
  - If `target.starts_with(root_dot_zero)`, emit I02 at the specifier's
    span.
- `property = "<specifier>"`, `value = "import"`. Message per spec.

**Tests:**
- `fires_on_relative_climb_into_dot_zero` — set up a temp project
  with `<root>/.zero/components/Button.ts` and
  `<root>/src/foo.ts` containing `import x from
  "../.zero/components/Button.ts";`.
- `does_not_fire_on_in_src_relative_import` — `import x from
  "./util.ts";`.
- `does_not_fire_when_dot_zero_dir_does_not_exist`.

---

### Step 14: Implement S01 — function body > 80 lines

**Goal:** Flag any function whose body span (open brace line through
close brace line, inclusive) exceeds 80 lines.

**Files:**
- `crates/zero-lint/src/js/rules/s01_function_size.rs` (new)
- `crates/zero-lint/src/js/mod.rs` (modify — register)

**Changes:**
- **Scope:** all `<root>/src/**`.
- Visit `FnDecl`, `FnExpr`, `ArrowExpr`, `ClassMethod`, `MethodProp`,
  `PrivateMethod`, `Constructor`, `GetterProp`, `SetterProp`. For each:
  - Resolve a body `Span`:
    - `FnDecl` / `FnExpr` / `Constructor` / `ClassMethod` /
      `PrivateMethod` / `MethodProp` / `GetterProp` / `SetterProp`:
      use `function.body.span` (or `method.function.body.span`); skip
      if `body` is `None`.
    - `ArrowExpr`: if `body` is `BlockStmtOrExpr::BlockStmt(b)`, use
      `b.span`. Otherwise (expression body, no braces) skip.
  - `start_line = cm.lookup_char_pos(body_span.lo).line`,
    `end_line = cm.lookup_char_pos(body_span.hi).line`,
    `lines = end_line - start_line + 1`.
  - If `lines > 80`, emit S01 at the function/method declaration span
    (use the FnDecl's `function.span` if available, else `body.span`).
- Function name resolution: `FnDecl.ident.sym`, `FnExpr.ident.as_ref()
  .map(|i| i.sym.as_ref())`, `MethodProp.key` if `Ident`,
  `ClassMethod.key` if `Ident`, etc. Fallback `"<anonymous>"`.
- `property = "<function-name>"`, `value = format!("size:{lines}")`.
  Message: `"function \`<name>\` is <n> lines (open brace to close
  brace inclusive); zero targets <= 80 — split into named helpers."`

**Tests:**
- `fires_on_oversized_function_decl`.
- `does_not_fire_on_eighty_line_function`.
- `fires_on_oversized_arrow_with_block_body`.
- `does_not_fire_on_short_arrow_expression_body` —
  `const f = x => x + 1;`.
- `fires_on_oversized_class_method`.
- `reports_function_name_when_named`.
- `reports_anonymous_when_unnamed`.

---

### Step 15: End-to-end fixture + agent-failure regression + clean-examples verification

**Goal:** Wire the new rules through the binary surface, prove every
rule ID fires on a hand-crafted bad fixture, and confirm the shipped
examples (`counter`, `todos`, `tracker`) and `showcase/` lint clean.

**Files:**
- `crates/zero/tests/fixtures/js_agent_failures/` (new directory tree)
  - `zero.toml`
  - `src/components/Bad.ts` (R01, R02, T01, T03, T04, T02, C01, S01)
  - `src/routes/Bad.ts` (T-rules cross-cover)
  - `src/stores/ok.ts` (R03 exemption — must NOT fire)
  - `src/lib/bad.ts` (I01, I02, C02, S01)
  - `src/bad.ts` (additional R02/R03 cross-coverage)
  - `.zero/components/Button.ts` (target of an I02 violation)
- `crates/zero/tests/lint_js_agent_failures.rs` (new)
- `crates/zero/tests/lint_js_smoke.rs` (new)
- `crates/zero/tests/lint_examples.rs` (no change expected; verify
  passes — if any example trips a rule, fix the rule, not the example,
  per spec R5)
- `examples/{counter,todos,tracker}/` and `showcase/` (no change
  expected; if a JS lint rule misfires on real code, fix the rule)

**Changes:**

1. **Fixture project** under
   `crates/zero/tests/fixtures/js_agent_failures/`:
   - Each `.ts` file is hand-authored to trip its target rules with
     minimum noise (one issue per file when feasible).
   - `zero.toml`:
     ```toml
     [project]
     root = "."

     [build]
     out = "dist"
     ```
   - Concrete content sketches (the test asserts rule-IDs, not exact
     text):
     - `src/components/Bad.ts` imports `signal, html, each` from
       `"zero"`, defines a function component, uses `${name.val}`
       (R01), assigns `count.val = 1` (R02), calls
       `el.addEventListener("click", h)` (T01), `each(items, render)`
       (T03), `document.querySelector(".x")` (T04), a template with
       `@click.foo` (T02), a `class Widget {}` (C01), and a
       100-line function (S01).
     - `src/lib/bad.ts` imports from `"node:fs"` (I01), imports from
       `"../.zero/components/Button.ts"` (I02), calls
       `customElements.define("my-el", X)` (C02), and contains an
       oversized function (S01).
     - `src/stores/ok.ts` declares `const items = signal([])` at
       module scope — MUST NOT fire R03.

2. **`lint_js_agent_failures.rs`** — mirrors `lint_agent_failures.rs`:
   copy fixture into a temp dir, run `cargo run -- lint --quiet`, assert
   exit code is failure and stderr contains every rule ID:
   `R01`, `R02`, `R03` (negative — assert it does NOT appear for the
   stores file path; the simplest way is to assert that the only `R03`
   occurrences, if any, refer to a non-stores file — but per the
   fixture there are no R03 fires, so assert `R03` does NOT appear),
   `T01`, `T02`, `T03`, `T04`, `C01`, `C02`, `I01`, `I02`, `S01`.

3. **`lint_js_smoke.rs`** — three small tests at the binary level:
   - Minimal project with one R01 violation: assert failure + `R01` in
     stderr.
   - Minimal project with `--quiet`: assert no caret line.
   - Clean project: assert exit 0 and `"zero lint — clean"` printed.

4. **Examples / showcase** — run `cargo test -p zero --test
   lint_examples` after the rule changes land. If any rule misfires,
   fix the rule (e.g. tighten T04's heuristic, broaden T02's allowed
   modifier list to include something the runtime actually accepts but
   the rule missed). Common predictable issues:
   - `examples/tracker/web/src/stores/auth.ts` declares a module-level
     `signal()` — covered by the `is_under_stores` exemption; verify.
   - `showcase/src/routes/*.ts` uses `each(items, render)` in some
     places — confirm each call already passes a `keyFn`. If not, this
     is a real bug in the example to fix per R5.

**Tests:**
- The two new integration tests above.
- The existing `lint_examples.rs` tests (`tracker_lints_clean`,
  `showcase_lints_clean`) must continue to pass.

---

### Step 16: Documentation

**Goal:** Spec, AGENTS.md, and the Phase 14 checklist reflect the new
rules.

**Files:**
- `zero-framework-spec.md` (modify)
- `crates/zero-scaffold/src/scaffold/AGENTS.md` (modify)

**Changes:**

1. **`zero-framework-spec.md` §1 (`zero fmt` / `zero lint`)** — append
   a subsection:
   ```
   ## JS/TS lint

   `zero lint` also runs an idiom pass over `<root>/src/**.{ts,js,
   tsx,jsx}`. Rules are hard-coded; no config.

   | Rule | Trigger | Message gist |
   | --- | --- | --- |
   | R01 | `${signal.val}` inside `html\`\`` | reading `.val` breaks reactivity |
   | R02 | `signal.val = …` | use `.set()` / `.update()` |
   | R03 | module-level `signal/computed/effect` (outside `src/stores/**`) | leaks; move into a function or store |
   | T01 | `addEventListener` in `src/{components,routes}/**` | use `@event=` |
   | T02 | unknown `@event.modifier` in `html\`\`` | typo — see allowed set |
   | T03 | `each(items, render)` (no `keyFn`) | pass a key fn for stable identity |
   | T04 | `document.querySelector` / `el.appendChild` in `src/{components,routes}/**` | use `ref()` for element handles |
   | C01 | `class X` in `src/{components,routes}/**` | components are plain functions |
   | C02 | `customElements.define(...)` | use the documented `'z/wc'` escape hatch |
   | I01 | bare specifier outside the allowlist | no node_modules in zero |
   | I02 | relative import into `.zero/` | import the public surface |
   | S01 | function body > 80 lines | split into named helpers |
   | P01 | parse error | (reserved — emits one diagnostic per parse failure) |

   Tests (`*.test.{ts,js}` / `*.spec.{ts,js}`) are exempt from the
   T-rules and R03; R02, C01, C02, I01, I02, S01 still apply.
   ```

2. **`crates/zero-scaffold/src/scaffold/AGENTS.md`** — under the
   existing `## Common mistakes (the lint will catch these)` heading,
   add a second table for the JS/TS rules (same rows as above, "Don't
   write" / "Use" framing where it fits — for rules without a clear
   alternative line, leave the "Use" cell as a one-line guidance).

3. **`zero-framework-spec.md` §12** — append:
   ```
   ### Phase 14 — JS/TS framework lint (issues/lint-js/spec.md)
   - [x] R01 — `${signal.val}` inside `html\`\``
   - [x] R02 — assignment to `signal.val`
   - [x] R03 — module-level `signal/computed/effect`
   - [x] T01 — `addEventListener` / `removeEventListener` in components/routes
   - [x] T02 — unknown `@event.modifier` in templates
   - [x] T03 — `each()` without `keyFn`
   - [x] T04 — direct DOM access in components/routes
   - [x] C01 — class declaration in components/routes
   - [x] C02 — `customElements.define(...)`
   - [x] I01 — unknown import specifier
   - [x] I02 — relative import into `.zero/`
   - [x] S01 — function body > 80 lines
   - [x] P01 — parse error surfaces as one diagnostic per file
   ```

**Tests:** none; documentation only. Verify `cargo test --workspace`
remains green.

---

## Risks and Assumptions

1. **SWC visitor traversal of nested templates.** R01 must descend
   into `html\`${html\`${x.val}\`}\`` to flag the inner read. SWC's
   default `Visit` traversal does descend; the per-rule test
   `fires_in_nested_html_template` is the safety net.

2. **T02 quasi span arithmetic.** SWC's `TplElement.span` semantics
   (exactly what byte the `lo` points at — opening backtick vs.
   character after `${...}`) varies by parser version. The plan ships
   a column-position assertion test to lock the behavior. If `lo`
   doesn't include the leading delimiter, the offset arithmetic stays
   simple; if it does, the rule subtracts a constant. Discoverable on
   first run.

3. **R03 false positive in non-`src/stores/**` factory layouts.** A
   project that organizes stores under `src/state/` or `src/data/`
   instead of `src/stores/` will get false positives at module scope.
   The plan hard-codes `src/stores/**` per Q1 bias; if real projects
   surface, a follow-up adds a single `[lint]` knob.

4. **T04 `document` shadow.** A file that declares
   `const document = mockDocument;` followed by
   `document.querySelector(...)` will be flagged even though it's
   safe. Spec accepts this trade.

5. **I02 canonicalization.** If `.zero/` exists but a relative target
   file does not (typo), `canonicalize` fails and the rule skips it
   silently — the bundler will catch the typo. Acceptable.

6. **No new top-level workspace deps.** `swc_core` is already in
   `workspace.dependencies` with the `ecma_visit` feature enabled;
   `zero-lint` adds it to its own `[dependencies]` block referencing
   the workspace entry. No `Cargo.toml` change in the workspace root.

7. **Performance.** With ~50–100 source files per example and per-file
   parse cost of a few milliseconds via SWC, total wall time is well
   under the 1s budget for `examples/tracker`. The plan does not cache
   parses; lint runs are infrequent.

8. **Aliased imports.** `T03`/`R03` go through the local-name → original
   name map so `import { signal as makeSig } from "zero"` is detected.
   `R02` doesn't need the original-name resolution (any `"zero"` import
   is enough to gate the rule).

9. **Test file detection.** Basename suffix match only (`*.test.{ts,
   js,tsx,jsx}` / `*.spec.{ts,js,tsx,jsx}`). Files named `*Test.ts`
   are NOT treated as tests — this matches `zero test`'s discovery
   semantics.
