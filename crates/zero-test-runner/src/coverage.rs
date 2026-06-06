//! SWC-driven coverage instrumenter.
//!
//! Transforms TS/JS source to inject per-line and per-function counter
//! increments into a module-global `__zero_coverage__` map. Emitted via a
//! VisitMut pass inserted between SWC's TypeScript strip and hygiene passes.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use swc_core::common::sync::Lrc;
use swc_core::common::{FileName, GLOBALS, Globals, Mark, SourceMap as SwcSourceMap, Spanned};
use swc_core::ecma::ast::{
    AssignExpr, AssignOp, AssignTarget, BlockStmt, Bool, ClassMember, ComputedPropName, EsVersion,
    Expr, ExprStmt, Function, Ident, IdentName, Invalid, KeyValueProp, Lit, MemberExpr,
    MemberProp, Module, ModuleDecl, ModuleExportName, ModuleItem, Number, ObjectLit, Prop,
    PropName, PropOrSpread, SeqExpr, SimpleAssignTarget, Stmt, Str, UnaryExpr, UnaryOp,
    UpdateExpr, UpdateOp, VarDecl, VarDeclKind, VarDeclarator,
};
use swc_core::ecma::codegen::Emitter;
use swc_core::ecma::codegen::text_writer::JsWriter;
use swc_core::ecma::parser::{Parser, StringInput, Syntax, TsSyntax, lexer::Lexer};
use swc_core::ecma::transforms::base::{fixer::fixer, hygiene::hygiene, resolver};
use swc_core::ecma::transforms::typescript::strip;
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

use zero_transpile::{TranspileError, TranspileOptions};

/// The set of executable lines and function identifiers discovered in one file.
#[derive(Debug, Clone)]
pub struct CoverageMap {
    /// Absolute, normalized path of the source file.
    pub file: PathBuf,
    /// Source line numbers that hold a counter increment, sorted ascending.
    pub lines: Vec<u32>,
    /// Function identifiers in source order: declarations use their name,
    /// expressions/arrows/anonymous methods use `anon@<line>`, class
    /// constructors use `<ClassName>.constructor` when the name is known.
    pub fns: Vec<String>,
}

/// Output of one [`instrument`] call.
#[derive(Debug)]
pub struct InstrumentOutput {
    /// Emitted JavaScript with counter increments injected.
    pub code: String,
    /// Optional source-map JSON (mirrors `opts.emit_source_map`).
    pub source_map: Option<String>,
    /// Universe of lines and functions for this file.
    pub map: CoverageMap,
}

/// Instrument `source` for coverage. Returns the rewritten code, an optional
/// source map back to the original TS/JS, and a [`CoverageMap`] describing the
/// known line/function universe.
///
/// # Parameters
/// - `source`: TS or JS source text.
/// - `opts`: filename + source-map controls; `opts.filename` is used as the
///   canonical key in `globalThis.__zero_coverage__`.
///
/// # Returns
/// `Ok(InstrumentOutput)` on success, `Err(TranspileError)` if parsing or
/// codegen fails.
pub fn instrument(
    source: &str,
    opts: &TranspileOptions<'_>,
) -> Result<InstrumentOutput, TranspileError> {
    let cm: Lrc<SwcSourceMap> = Default::default();
    let module = parse_typescript_module(&cm, source, opts.filename)?;
    let (module, lines, fns) = transform_with_counters(&cm, module, opts.filename);
    let (code, srcmap_buf) = emit_module(&cm, &module, opts.filename)?;
    let source_map = if opts.emit_source_map {
        Some(build_source_map_json(&cm, &srcmap_buf, opts.filename)?)
    } else {
        None
    };
    let map = CoverageMap {
        file: PathBuf::from(opts.filename),
        lines: lines.into_iter().collect(),
        fns,
    };
    Ok(InstrumentOutput {
        code,
        source_map,
        map,
    })
}

/// Lex + parse `source` as a TypeScript module against `cm`.
fn parse_typescript_module(
    cm: &Lrc<SwcSourceMap>,
    source: &str,
    filename: &str,
) -> Result<Module, TranspileError> {
    let fm = cm.new_source_file(
        Lrc::new(FileName::Custom(filename.to_string())),
        source.to_string(),
    );
    let lexer = Lexer::new(
        Syntax::Typescript(TsSyntax {
            decorators: false,
            tsx: false,
            ..Default::default()
        }),
        EsVersion::EsNext,
        StringInput::from(&*fm),
        None,
    );
    let mut parser = Parser::new_from(lexer);
    parser.parse_module().map_err(|e| TranspileError {
        file: filename.to_string(),
        line: 0,
        column: 0,
        message: format!("parse error: {e:?}"),
    })
}

/// Run resolver → strip → InstrumenterVisitor → hygiene → fixer; return the
/// instrumented module plus the set of recorded lines and function names.
fn transform_with_counters(
    cm: &Lrc<SwcSourceMap>,
    module: Module,
    filename: &str,
) -> (Module, BTreeSet<u32>, Vec<String>) {
    GLOBALS.set(&Globals::new(), || {
        let unresolved_mark = Mark::new();
        let top_level_mark = Mark::new();
        let mut program = swc_core::ecma::ast::Program::Module(module);
        program.mutate(resolver(unresolved_mark, top_level_mark, true));
        program.mutate(strip(unresolved_mark, top_level_mark));

        let mut module = match program {
            swc_core::ecma::ast::Program::Module(m) => m,
            swc_core::ecma::ast::Program::Script(_) => unreachable!(),
        };

        let mut visitor = InstrumenterVisitor::new(cm.clone());
        visitor.visit_mut_module(&mut module);

        let prologue = build_prologue(filename, &visitor.lines, &visitor.fns);
        module.body.insert(0, ModuleItem::Stmt(prologue));

        let mut program = swc_core::ecma::ast::Program::Module(module);
        program.mutate(hygiene());
        program.mutate(fixer(None));

        let module = match program {
            swc_core::ecma::ast::Program::Module(m) => m,
            _ => unreachable!(),
        };
        (module, visitor.lines, visitor.fns)
    })
}

/// Emit `module` to JS bytes plus an SWC sourcemap-position buffer.
type SrcmapBuf = Vec<(swc_core::common::BytePos, swc_core::common::LineCol)>;

fn emit_module(
    cm: &Lrc<SwcSourceMap>,
    module: &Module,
    filename: &str,
) -> Result<(String, SrcmapBuf), TranspileError> {
    let mut code_buf: Vec<u8> = Vec::new();
    let mut srcmap_buf: SrcmapBuf = Vec::new();
    {
        let writer = JsWriter::new(cm.clone(), "\n", &mut code_buf, Some(&mut srcmap_buf));
        let mut emitter = Emitter {
            cfg: swc_core::ecma::codegen::Config::default(),
            cm: cm.clone(),
            comments: None,
            wr: writer,
        };
        emitter.emit_module(module).map_err(|e| TranspileError {
            file: filename.to_string(),
            line: 0,
            column: 0,
            message: format!("codegen error: {e}"),
        })?;
    }
    let code = String::from_utf8(code_buf).map_err(|e| TranspileError {
        file: filename.to_string(),
        line: 0,
        column: 0,
        message: format!("non-UTF-8 codegen output: {e}"),
    })?;
    Ok((code, srcmap_buf))
}

/// Build a JSON source-map string from the SWC position buffer.
fn build_source_map_json(
    cm: &Lrc<SwcSourceMap>,
    srcmap_buf: &SrcmapBuf,
    filename: &str,
) -> Result<String, TranspileError> {
    let map = cm.build_source_map(
        srcmap_buf,
        None,
        swc_core::common::source_map::DefaultSourceMapGenConfig,
    );
    let mut json: Vec<u8> = Vec::new();
    map.to_writer(&mut json).map_err(|e| TranspileError {
        file: filename.to_string(),
        line: 0,
        column: 0,
        message: format!("sourcemap serialization failed: {e}"),
    })?;
    String::from_utf8(json).map_err(|e| TranspileError {
        file: filename.to_string(),
        line: 0,
        column: 0,
        message: format!("non-UTF-8 sourcemap json: {e}"),
    })
}

/// The instrumenter walks the AST collecting lines/fns and injecting counter
/// statements.
struct InstrumenterVisitor {
    cm: Lrc<SwcSourceMap>,
    lines: BTreeSet<u32>,
    fns: Vec<String>,
    pending_fn_names: Vec<Option<String>>,
}

impl InstrumenterVisitor {
    fn new(cm: Lrc<SwcSourceMap>) -> Self {
        Self {
            cm,
            lines: BTreeSet::new(),
            fns: Vec::new(),
            pending_fn_names: Vec::new(),
        }
    }

    fn line_of<S: Spanned>(&self, n: &S) -> u32 {
        let pos = self.cm.lookup_char_pos(n.span().lo());
        pos.line as u32
    }

    /// Build a `__c.lines[N]++;` statement for line `n` and record it.
    fn line_counter_stmt(&mut self, n: u32) -> Stmt {
        self.lines.insert(n);
        Stmt::Expr(ExprStmt {
            span: Default::default(),
            expr: Box::new(Expr::Update(UpdateExpr {
                span: Default::default(),
                op: UpdateOp::PlusPlus,
                prefix: false,
                arg: Box::new(Expr::Member(MemberExpr {
                    span: Default::default(),
                    obj: Box::new(Expr::Member(MemberExpr {
                        span: Default::default(),
                        obj: Box::new(Expr::Ident(Ident::new(
                            "__c".into(),
                            Default::default(),
                            Default::default(),
                        ))),
                        prop: MemberProp::Ident(IdentName {
                            span: Default::default(),
                            sym: "lines".into(),
                        }),
                    })),
                    prop: MemberProp::Computed(ComputedPropName {
                        span: Default::default(),
                        expr: Box::new(Expr::Lit(Lit::Num(Number {
                            span: Default::default(),
                            value: n as f64,
                            raw: None,
                        }))),
                    }),
                })),
            })),
        })
    }

    /// Build a `__c.fns["name"]++;` statement and record it.
    fn fn_counter_stmt(&mut self, name: &str) -> Stmt {
        let expr = self.fn_counter_expr(name);
        Stmt::Expr(ExprStmt {
            span: Default::default(),
            expr: Box::new(expr),
        })
    }

    /// Build a bare `__c.fns["name"]++` update expression and record it.
    fn fn_counter_expr(&mut self, name: &str) -> Expr {
        self.fns.push(name.to_string());
        Expr::Update(UpdateExpr {
            span: Default::default(),
            op: UpdateOp::PlusPlus,
            prefix: false,
            arg: Box::new(Expr::Member(MemberExpr {
                span: Default::default(),
                obj: Box::new(Expr::Member(MemberExpr {
                    span: Default::default(),
                    obj: Box::new(Expr::Ident(Ident::new(
                        "__c".into(),
                        Default::default(),
                        Default::default(),
                    ))),
                    prop: MemberProp::Ident(IdentName {
                        span: Default::default(),
                        sym: "fns".into(),
                    }),
                })),
                prop: MemberProp::Computed(ComputedPropName {
                    span: Default::default(),
                    expr: Box::new(Expr::Lit(Lit::Str(Str {
                        span: Default::default(),
                        value: name.into(),
                        raw: None,
                    }))),
                }),
            })),
        })
    }

    /// Inject counters into the head of a block body.
    fn instrument_body(&mut self, body: &mut BlockStmt, fn_name: Option<&str>) {
        let mut prefix: Vec<Stmt> = Vec::new();
        if let Some(name) = fn_name {
            let s = self.fn_counter_stmt(name);
            prefix.push(s);
        }
        // Per-statement line counters (including the first body statement) are
        // injected by `visit_mut_stmts` during the recursive visit below.
        body.visit_mut_children_with(self);

        // Inject the counter prologue at the front.
        let mut new_stmts = prefix;
        new_stmts.extend(std::mem::take(&mut body.stmts));
        body.stmts = new_stmts;
    }
}

impl VisitMut for InstrumenterVisitor {
    fn visit_mut_module(&mut self, module: &mut Module) {
        // Walk children first (so nested fns / blocks get instrumented).
        // We'll insert top-level line counters AFTER recursive visit.
        module.visit_mut_children_with(self);

        let items = std::mem::take(&mut module.body);
        let mut new_items = Vec::with_capacity(items.len() * 2);
        for item in items {
            let line = match &item {
                ModuleItem::Stmt(s) => Some(self.line_of(s)),
                ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(d)) => Some(self.line_of(d)),
                ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultDecl(d)) => Some(self.line_of(d)),
                ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultExpr(d)) => Some(self.line_of(d)),
                ModuleItem::ModuleDecl(_) => None, // import/export-named/etc — not executable
            };
            if let Some(line) = line {
                let stmt = self.line_counter_stmt(line);
                new_items.push(ModuleItem::Stmt(stmt));
            }
            new_items.push(item);
        }
        module.body = new_items;
    }

    fn visit_mut_function(&mut self, f: &mut Function) {
        // Pull a name from the pending stack (set up by VarDeclarator/FnDecl/FnExpr).
        let name = self
            .pending_fn_names
            .pop()
            .flatten()
            .unwrap_or_else(|| format!("anon@{}", self.line_of(f)));
        if let Some(body) = &mut f.body {
            self.instrument_body(body, Some(&name));
        }
    }

    fn visit_mut_fn_decl(&mut self, decl: &mut swc_core::ecma::ast::FnDecl) {
        self.pending_fn_names.push(Some(decl.ident.sym.to_string()));
        decl.visit_mut_children_with(self);
    }

    fn visit_mut_fn_expr(&mut self, expr: &mut swc_core::ecma::ast::FnExpr) {
        let n = expr
            .ident
            .as_ref()
            .map(|i| i.sym.to_string())
            .unwrap_or_else(|| format!("anon@{}", self.line_of(expr)));
        self.pending_fn_names.push(Some(n));
        expr.visit_mut_children_with(self);
    }

    fn visit_mut_arrow_expr(&mut self, e: &mut swc_core::ecma::ast::ArrowExpr) {
        let name = format!("anon@{}", self.line_of(e));
        if let swc_core::ecma::ast::BlockStmtOrExpr::BlockStmt(block) = &mut *e.body {
            // Per-statement line counters come from `visit_mut_stmts` below; we
            // only prepend the function-entry counter here.
            let prefix: Vec<Stmt> = vec![self.fn_counter_stmt(&name)];
            block.visit_mut_children_with(self);
            let mut new_stmts = prefix;
            new_stmts.extend(std::mem::take(&mut block.stmts));
            block.stmts = new_stmts;
        } else {
            // Expression body: wrap it in a sequence expression so the entry
            // counter fires on every call — `(x) => expr` becomes
            // `(x) => (__c.fns["anon@N"]++, expr)`. Without this, arrows in
            // expression position (template interpolations, props, computed()
            // bodies, .map callbacks) register in the universe but never
            // increment, reading as permanently uncovered. The `fixer` pass
            // that runs after this visitor adds the required parentheses.
            e.visit_mut_children_with(self);
            if let swc_core::ecma::ast::BlockStmtOrExpr::Expr(expr) = &mut *e.body {
                let counter = self.fn_counter_expr(&name);
                let body = std::mem::replace(
                    expr,
                    Box::new(Expr::Invalid(Invalid {
                        span: Default::default(),
                    })),
                );
                *expr = Box::new(Expr::Seq(SeqExpr {
                    span: Default::default(),
                    exprs: vec![Box::new(counter), body],
                }));
            }
        }
    }

    fn visit_mut_method_prop(&mut self, m: &mut swc_core::ecma::ast::MethodProp) {
        let name = prop_name_string(&m.key)
            .unwrap_or_else(|| format!("anon@{}", self.line_of(&m.function)));
        self.pending_fn_names.push(Some(name));
        m.visit_mut_children_with(self);
    }

    fn visit_mut_class_method(&mut self, m: &mut swc_core::ecma::ast::ClassMethod) {
        let name = prop_name_string(&m.key)
            .unwrap_or_else(|| format!("anon@{}", self.line_of(&m.function)));
        self.pending_fn_names.push(Some(name));
        m.visit_mut_children_with(self);
    }

    fn visit_mut_constructor(&mut self, c: &mut swc_core::ecma::ast::Constructor) {
        let name = "constructor".to_string();
        if let Some(body) = &mut c.body {
            // Record the constructor's fn entry; per-statement line counters
            // come from `visit_mut_stmts` during the recursive visit.
            let prefix: Vec<Stmt> = vec![self.fn_counter_stmt(&name)];
            body.visit_mut_children_with(self);
            let mut new_stmts = prefix;
            new_stmts.extend(std::mem::take(&mut body.stmts));
            body.stmts = new_stmts;
        }
    }

    fn visit_mut_block_stmt(&mut self, block: &mut BlockStmt) {
        // Statement-level counters are injected by `visit_mut_stmts` when the
        // block's `stmts` vec is visited; here we only recurse.
        block.visit_mut_children_with(self);
    }

    fn visit_mut_stmts(&mut self, stmts: &mut Vec<Stmt>) {
        // Instrument nested statements first so inner blocks are counted.
        stmts.visit_mut_children_with(self);
        // Prepend a line counter before each statement. Dedupe by source line
        // within this Vec so multiple statements sharing a physical line add at
        // most one counter (keeps instrumentation density — and the GC-teardown
        // surface — minimal). Every `Vec<Stmt>` context flows through here:
        // function/block bodies, if/else/try/catch/for/while blocks, and switch
        // `case` statement lists. Top-level `Vec<ModuleItem>` is not a
        // `Vec<Stmt>`, so `visit_mut_module`'s per-item counters do not collide.
        let mut out: Vec<Stmt> = Vec::with_capacity(stmts.len() * 2);
        let mut seen: BTreeSet<u32> = BTreeSet::new();
        for stmt in std::mem::take(stmts) {
            let line = self.line_of(&stmt);
            if seen.insert(line) {
                out.push(self.line_counter_stmt(line));
            }
            out.push(stmt);
        }
        *stmts = out;
    }
}

fn prop_name_string(name: &PropName) -> Option<String> {
    match name {
        PropName::Ident(i) => Some(i.sym.to_string()),
        PropName::Str(s) => s.value.as_str().map(|s| s.to_string()),
        PropName::Num(n) => Some(n.value.to_string()),
        PropName::BigInt(b) => Some(b.value.to_string()),
        PropName::Computed(_) => None,
    }
}

/// Build the coverage-prologue Stmt:
/// `const __c = (globalThis.__zero_coverage__ ||= {})[<file>] ||= { lines: {...}, fns: {...} };`
fn build_prologue(filename: &str, lines: &BTreeSet<u32>, fns: &[String]) -> Stmt {
    let map_lit = ObjectLit {
        span: Default::default(),
        props: vec![
            ident_prop("lines", Expr::Object(lines_zero_map(lines))),
            ident_prop("fns", Expr::Object(fns_zero_map(fns))),
        ],
    };
    let assigned = file_slot_or_init(filename, Expr::Object(map_lit));
    const_decl("__c", assigned)
}

/// `{ <n>: 0, … }` for every line number, in BTreeSet order.
fn lines_zero_map(lines: &BTreeSet<u32>) -> ObjectLit {
    ObjectLit {
        span: Default::default(),
        props: lines.iter().map(|n| num_zero_prop(*n as f64)).collect(),
    }
}

/// `{ "name": 0, … }` for every recorded function name, in source order.
fn fns_zero_map(fns: &[String]) -> ObjectLit {
    ObjectLit {
        span: Default::default(),
        props: fns.iter().map(|name| str_zero_prop(name)).collect(),
    }
}

/// Numeric key → 0 literal property.
fn num_zero_prop(n: f64) -> PropOrSpread {
    PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
        key: PropName::Num(Number {
            span: Default::default(),
            value: n,
            raw: None,
        }),
        value: Box::new(zero_lit()),
    })))
}

/// String key → 0 literal property.
fn str_zero_prop(name: &str) -> PropOrSpread {
    PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
        key: PropName::Str(Str {
            span: Default::default(),
            value: name.into(),
            raw: None,
        }),
        value: Box::new(zero_lit()),
    })))
}

/// Identifier key → arbitrary value property.
fn ident_prop(name: &str, value: Expr) -> PropOrSpread {
    PropOrSpread::Prop(Box::new(Prop::KeyValue(KeyValueProp {
        key: PropName::Ident(IdentName {
            span: Default::default(),
            sym: name.into(),
        }),
        value: Box::new(value),
    })))
}

fn zero_lit() -> Expr {
    Expr::Lit(Lit::Num(Number {
        span: Default::default(),
        value: 0.0,
        raw: None,
    }))
}

/// `(globalThis.__zero_coverage__ ||= {})[<filename>] ||= <init>`
fn file_slot_or_init(filename: &str, init: Expr) -> Expr {
    let global_cov = Expr::Assign(AssignExpr {
        span: Default::default(),
        op: AssignOp::OrAssign,
        left: AssignTarget::Simple(SimpleAssignTarget::Member(MemberExpr {
            span: Default::default(),
            obj: Box::new(Expr::Ident(Ident::new(
                "globalThis".into(),
                Default::default(),
                Default::default(),
            ))),
            prop: MemberProp::Ident(IdentName {
                span: Default::default(),
                sym: "__zero_coverage__".into(),
            }),
        })),
        right: Box::new(Expr::Object(ObjectLit {
            span: Default::default(),
            props: vec![],
        })),
    });
    let file_lookup = MemberExpr {
        span: Default::default(),
        obj: Box::new(global_cov),
        prop: MemberProp::Computed(ComputedPropName {
            span: Default::default(),
            expr: Box::new(Expr::Lit(Lit::Str(Str {
                span: Default::default(),
                value: filename.into(),
                raw: None,
            }))),
        }),
    };
    Expr::Assign(AssignExpr {
        span: Default::default(),
        op: AssignOp::OrAssign,
        left: AssignTarget::Simple(SimpleAssignTarget::Member(file_lookup)),
        right: Box::new(init),
    })
}

/// `const <name> = <init>;`
fn const_decl(name: &str, init: Expr) -> Stmt {
    Stmt::Decl(swc_core::ecma::ast::Decl::Var(Box::new(VarDecl {
        span: Default::default(),
        kind: VarDeclKind::Const,
        declare: false,
        ctxt: Default::default(),
        decls: vec![VarDeclarator {
            span: Default::default(),
            name: swc_core::ecma::ast::Pat::Ident(swc_core::ecma::ast::BindingIdent {
                id: Ident::new(name.into(), Default::default(), Default::default()),
                type_ann: None,
            }),
            init: Some(Box::new(init)),
            definite: false,
        }],
    })))
}

// ----------------------------------------------------------------------------
// Coverage scope + aggregator
// ----------------------------------------------------------------------------

/// Inclusion rules: which source files should be instrumented for coverage.
#[derive(Debug, Clone)]
pub struct CoverageScope {
    pub project_root: PathBuf,
    pub src_dir: PathBuf,
    pub out_dir: PathBuf,
}

impl CoverageScope {
    /// Construct a scope for the conventional `<root>/src` layout.
    ///
    /// # Parameters
    /// - `project_root`: absolute path to the project root.
    /// - `out_dir`: absolute path to the build output directory.
    ///
    /// # Returns
    /// A scope rooted at `<project_root>/src`.
    pub fn new(project_root: PathBuf, out_dir: PathBuf) -> Self {
        let src_dir = project_root.join("src");
        Self {
            project_root,
            src_dir,
            out_dir,
        }
    }

    /// Returns `true` if `file` is a candidate for coverage instrumentation.
    ///
    /// Rules (per spec §2.2): under `src_dir`, ext is `.ts` or `.js`, not a
    /// test file, not under `out_dir`, not under any `.zero/` or
    /// `node_modules/` segment.
    pub fn covers(&self, file: &Path) -> bool {
        if !file.starts_with(&self.src_dir) {
            return false;
        }
        if file.starts_with(&self.out_dir) {
            return false;
        }
        for comp in file.components() {
            if let std::path::Component::Normal(s) = comp {
                let s = s.to_string_lossy();
                if s == ".zero" || s == "node_modules" {
                    return false;
                }
            }
        }
        let name = match file.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => return false,
        };
        if name.ends_with(".test.ts")
            || name.ends_with(".test.js")
            || name.ends_with(".spec.ts")
            || name.ends_with(".spec.js")
        {
            return false;
        }
        name.ends_with(".ts") || name.ends_with(".js")
    }
}

/// Per-file aggregated coverage hits.
#[derive(Debug, Default)]
struct FileAggregate {
    map: Option<CoverageMap>,
    line_hits: BTreeMap<u32, u64>,
    fn_hits: BTreeMap<String, u64>,
}

/// Accumulates per-file coverage results across a test run and renders reports.
#[derive(Debug, Default)]
pub struct CoverageAggregator {
    files: BTreeMap<PathBuf, FileAggregate>,
}

impl CoverageAggregator {
    /// Create an empty aggregator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a map so the aggregator knows the universe of lines/fns for a
    /// file before any execution.
    pub fn register(&mut self, map: CoverageMap) {
        let file = map.file.clone();
        let entry = self.files.entry(file).or_default();
        if entry.map.is_none() {
            // initialize hits at zero
            for ln in &map.lines {
                entry.line_hits.entry(*ln).or_insert(0);
            }
            for name in &map.fns {
                entry.fn_hits.entry(name.clone()).or_insert(0);
            }
        }
        entry.map = Some(map);
    }

    /// Ingest a single test file's `globalThis.__zero_coverage__` snapshot
    /// (serde JSON shape: `{ "<abs-path>": { lines: {N: count}, fns: {name: count} } }`).
    pub fn ingest_run(&mut self, run: &serde_json::Value) {
        let obj = match run.as_object() {
            Some(o) => o,
            None => return,
        };
        for (path, entry) in obj {
            let path = PathBuf::from(path);
            let agg = self.files.entry(path).or_default();
            if let Some(lines) = entry.get("lines").and_then(|v| v.as_object()) {
                for (k, v) in lines {
                    if let (Ok(ln), Some(hits)) = (k.parse::<u32>(), v.as_u64()) {
                        *agg.line_hits.entry(ln).or_insert(0) += hits;
                    }
                }
            }
            if let Some(fns) = entry.get("fns").and_then(|v| v.as_object()) {
                for (k, v) in fns {
                    if let Some(hits) = v.as_u64() {
                        *agg.fn_hits.entry(k.clone()).or_insert(0) += hits;
                    }
                }
            }
        }
    }

    /// Render the terminal table.
    pub fn write_terminal<W: Write>(&self, w: &mut W, project_root: &Path) -> io::Result<()> {
        let mut rows: Vec<(String, u32, u32, u32, u32, f64)> = Vec::new();
        let (mut tot_lines_c, mut tot_lines_t, mut tot_fns_c, mut tot_fns_t) =
            (0u32, 0u32, 0u32, 0u32);
        for (path, agg) in &self.files {
            let total_lines = agg.line_hits.len() as u32;
            let covered_lines = agg.line_hits.values().filter(|c| **c > 0).count() as u32;
            let total_fns = agg.fn_hits.len() as u32;
            let covered_fns = agg.fn_hits.values().filter(|c| **c > 0).count() as u32;
            let pct = if total_lines + total_fns == 0 {
                100.0
            } else {
                100.0 * (covered_lines + covered_fns) as f64 / (total_lines + total_fns) as f64
            };
            let rel = path
                .strip_prefix(project_root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            rows.push((rel, covered_lines, total_lines, covered_fns, total_fns, pct));
            tot_lines_c += covered_lines;
            tot_lines_t += total_lines;
            tot_fns_c += covered_fns;
            tot_fns_t += total_fns;
        }
        rows.sort_by(|a, b| a.5.partial_cmp(&b.5).unwrap_or(std::cmp::Ordering::Equal));
        let path_w = rows.iter().map(|r| r.0.len()).max().unwrap_or(4).max(4);
        writeln!(w, "Coverage:")?;
        writeln!(w, "  {:<pw$}  Lines       Fns", "File", pw = path_w)?;
        for (path, cl, tl, cf, tf, pct) in &rows {
            writeln!(
                w,
                "  {:<pw$}  {:>3} /{:>3}    {:>2} /{:>3}    {:>5.1}%",
                path,
                cl,
                tl,
                cf,
                tf,
                pct,
                pw = path_w
            )?;
        }
        let total_pct = if tot_lines_t + tot_fns_t == 0 {
            100.0
        } else {
            100.0 * (tot_lines_c + tot_fns_c) as f64 / (tot_lines_t + tot_fns_t) as f64
        };
        writeln!(w, "  {:-<pw$}--", "", pw = path_w + 28)?;
        writeln!(
            w,
            "  {:<pw$}  {:>3} /{:>3}    {:>2} /{:>3}    {:>5.1}%",
            "Total",
            tot_lines_c,
            tot_lines_t,
            tot_fns_c,
            tot_fns_t,
            total_pct,
            pw = path_w
        )?;
        Ok(())
    }

    /// Write `coverage/coverage.json` under `project_root`.
    pub fn write_json(&self, project_root: &Path) -> io::Result<()> {
        let dir = project_root.join("coverage");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("coverage.json");
        let value = self.to_json_value(project_root);
        let s = serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".into());
        std::fs::write(&path, s)?;
        Ok(())
    }

    /// Render the aggregator as a JSON value (used internally and by tests).
    pub fn to_json_value(&self, project_root: &Path) -> serde_json::Value {
        let mut files = serde_json::Map::new();
        let (mut tot_lc, mut tot_lt, mut tot_fc, mut tot_ft) = (0u64, 0u64, 0u64, 0u64);
        let mut entries: Vec<_> = self.files.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        for (path, agg) in entries {
            let total_lines = agg.line_hits.len() as u64;
            let covered_lines = agg.line_hits.values().filter(|c| **c > 0).count() as u64;
            let total_fns = agg.fn_hits.len() as u64;
            let covered_fns = agg.fn_hits.values().filter(|c| **c > 0).count() as u64;
            let uncovered_lines: Vec<u32> = agg
                .line_hits
                .iter()
                .filter_map(|(ln, c)| if *c == 0 { Some(*ln) } else { None })
                .collect();
            let uncovered_fns: Vec<String> = agg
                .fn_hits
                .iter()
                .filter_map(|(n, c)| if *c == 0 { Some(n.clone()) } else { None })
                .collect();
            let rel = path
                .strip_prefix(project_root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            files.insert(
                rel,
                serde_json::json!({
                    "lines": {
                        "covered": covered_lines,
                        "total": total_lines,
                        "uncovered": uncovered_lines,
                    },
                    "fns": {
                        "covered": covered_fns,
                        "total": total_fns,
                        "uncovered": uncovered_fns,
                    },
                }),
            );
            tot_lc += covered_lines;
            tot_lt += total_lines;
            tot_fc += covered_fns;
            tot_ft += total_fns;
        }
        serde_json::json!({
            "totals": {
                "lines": { "covered": tot_lc, "total": tot_lt },
                "fns":   { "covered": tot_fc, "total": tot_ft },
            },
            "files": files,
        })
    }
}

// ----------------------------------------------------------------------------
// Avoid unused-import warnings when feature combos vary.
#[allow(dead_code)]
fn _silence_unused_imports() {
    let _ = std::mem::size_of::<Arc<()>>();
    let _: Option<&ClassMember> = None;
    let _: Option<Bool> = None;
    let _: Option<UnaryExpr> = None;
    let _ = UnaryOp::Bang;
    let _: Option<ModuleExportName> = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rquickjs::{CatchResultExt, Context, Runtime, Value};

    fn opts(file: &str) -> TranspileOptions<'_> {
        TranspileOptions {
            filename: file,
            inline_source_map: false,
            emit_source_map: false,
        }
    }

    /// Eval `code` as a script under QuickJS and return
    /// `globalThis.__zero_coverage__` as a serde_json value.
    fn run_coverage(code: &str) -> serde_json::Value {
        let rt = Runtime::new().expect("runtime");
        let ctx = Context::full(&rt).expect("context");
        ctx.with(|ctx| {
            ctx.eval::<(), _>(code)
                .catch(&ctx)
                .unwrap_or_else(|e| panic!("eval failed: {e}\ncode:\n{code}"));
            let cov: Value = ctx
                .globals()
                .get("__zero_coverage__")
                .expect("__zero_coverage__ present");
            let json = ctx
                .json_stringify(cov)
                .expect("stringify")
                .expect("coverage not undefined")
                .to_string()
                .expect("to_string");
            serde_json::from_str(&json).expect("parse coverage json")
        })
    }

    fn lookup_lines_n(cov: &serde_json::Value, file: &str, n: u32) -> i64 {
        cov.get(file)
            .and_then(|f| f.get("lines"))
            .and_then(|l| l.get(n.to_string()))
            .and_then(|v| v.as_i64())
            .unwrap_or(-1)
    }

    fn lookup_fns(cov: &serde_json::Value, file: &str, name: &str) -> i64 {
        cov.get(file)
            .and_then(|f| f.get("fns"))
            .and_then(|m| m.get(name))
            .and_then(|v| v.as_i64())
            .unwrap_or(-1)
    }

    #[test]
    fn instruments_top_level_statement_increments_line_counter() {
        let src = "export const x = 1;\n";
        let out = instrument(src, &opts("/abs/foo.ts")).expect("instrument");
        // The export decl appears via the loader-style top-level script, so
        // wrap it in a function we then immediately call to evaluate. Avoid
        // module semantics so we can eval as a script.
        let exec = out.code.replace("export ", "");
        let cov = run_coverage(&exec);
        assert_eq!(
            lookup_lines_n(&cov, "/abs/foo.ts", 1),
            1,
            "top-level line counter should be 1\ncode:\n{exec}"
        );
    }

    #[test]
    fn instruments_function_prologue() {
        let src = "export function f(){ return 1 }\nf();\n";
        let out = instrument(src, &opts("/abs/foo.ts")).expect("instrument");
        let exec = out.code.replace("export ", "");
        let cov = run_coverage(&exec);
        assert_eq!(
            lookup_fns(&cov, "/abs/foo.ts", "f"),
            1,
            "fns.f should be 1 after one call\ncode:\n{exec}"
        );
        // The function body's first line is line 1 of the source (same line).
        let lines = lookup_lines_n(&cov, "/abs/foo.ts", 1);
        assert!(lines >= 1, "line 1 should fire at least once, got {lines}");
    }

    #[test]
    fn instruments_arrow_function() {
        let src = "const g = (x) => { return x + 1 };\ng(2);\n";
        let out = instrument(src, &opts("/abs/foo.ts")).expect("instrument");
        let cov = run_coverage(&out.code);
        // anon@<line of arrow start> — line 1.
        let cnt = lookup_fns(&cov, "/abs/foo.ts", "anon@1");
        assert_eq!(cnt, 1, "anon@1 should fire once\ncode:\n{}", out.code);
    }

    #[test]
    fn instruments_expression_bodied_arrow_entry() {
        // #76 regression: expression-bodied arrows used to register in the fns
        // universe but never increment — phantom "uncovered" functions.
        let src = "const g = (x) => x + 1;\ng(2);\ng(3);\n";
        let out = instrument(src, &opts("/abs/foo.ts")).expect("instrument");
        let cov = run_coverage(&out.code);
        assert_eq!(
            lookup_fns(&cov, "/abs/foo.ts", "anon@1"),
            2,
            "expression-bodied arrow should count each call\ncode:\n{}",
            out.code
        );
    }

    #[test]
    fn uncalled_expression_bodied_arrow_stays_zero() {
        let src = "const g = (x) => x + 1;\n";
        let out = instrument(src, &opts("/abs/foo.ts")).expect("instrument");
        let cov = run_coverage(&out.code);
        assert_eq!(
            lookup_fns(&cov, "/abs/foo.ts", "anon@1"),
            0,
            "uncalled arrow must stay at 0\ncode:\n{}",
            out.code
        );
    }

    #[test]
    fn instruments_arrow_in_expression_position() {
        // Arrows passed as call arguments (.map callbacks, computed() bodies,
        // template-interpolation props) must count entries per invocation.
        let src = "const out = [1, 2, 3].map((x) => x * 2);\n";
        let out = instrument(src, &opts("/abs/foo.ts")).expect("instrument");
        let cov = run_coverage(&out.code);
        assert_eq!(
            lookup_fns(&cov, "/abs/foo.ts", "anon@1"),
            3,
            ".map callback should fire once per element\ncode:\n{}",
            out.code
        );
    }

    #[test]
    fn instruments_arrow_with_object_literal_body() {
        // `() => ({...})` — the seq-expr wrap must stay parenthesized so the
        // body still parses as an object literal, not a block.
        let src = "const mk = () => ({ a: 1 });\nconst v = mk();\nif (v.a !== 1) throw new Error(\"bad\");\n";
        let out = instrument(src, &opts("/abs/foo.ts")).expect("instrument");
        let cov = run_coverage(&out.code);
        assert_eq!(
            lookup_fns(&cov, "/abs/foo.ts", "anon@1"),
            1,
            "object-literal-bodied arrow should count\ncode:\n{}",
            out.code
        );
    }

    #[test]
    fn block_bodied_arrow_registers_name_once() {
        // The old code pushed the name twice for block-bodied arrows
        // (once up front, once via the counter builder).
        let src = "const g = (x) => { return x + 1 };\n";
        let out = instrument(src, &opts("/abs/foo.ts")).expect("instrument");
        let count = out.map.fns.iter().filter(|n| *n == "anon@1").count();
        assert_eq!(count, 1, "anon@1 duplicated in fns universe: {:?}", out.map.fns);
    }

    #[test]
    fn instruments_every_statement_in_a_body() {
        let src =
            "export function f(go) {\n  const a = 1;\n  if (go) { return a; }\n  return 0;\n}\n";
        let out = instrument(src, &opts("/abs/foo.ts")).expect("instrument");
        // The trailing `return 0;` is on line 4 — below the function's first
        // body statement (line 2). Under coarse instrumentation only line 2
        // was counted; per-statement instrumentation must record line 4.
        assert!(
            out.map.lines.contains(&4),
            "line 4 (trailing return) should carry a counter, got {:?}",
            out.map.lines
        );
        // Every statement line is present: `const a` (2), the `if` and its
        // inner `return a` (3), and the trailing `return 0` (4).
        for ln in [2u32, 3, 4] {
            assert!(
                out.map.lines.contains(&ln),
                "line {ln} should carry a counter, got {:?}",
                out.map.lines
            );
        }
        // Counters fire at runtime: f(true) takes the inner return (line 3),
        // f(false) takes the trailing return (line 4).
        let exec = format!("{}\nf(true);\nf(false);\n", out.code.replace("export ", ""));
        let cov = run_coverage(&exec);
        assert!(
            lookup_lines_n(&cov, "/abs/foo.ts", 3) >= 1,
            "line 3 should fire when go is true\ncode:\n{exec}"
        );
        assert!(
            lookup_lines_n(&cov, "/abs/foo.ts", 4) >= 1,
            "line 4 should fire when go is false\ncode:\n{exec}"
        );
    }

    #[test]
    fn multiple_statements_one_line_count_once() {
        // Two statements share line 2 inside the body. `visit_mut_stmts`
        // dedupes by line, so line 2 gets exactly one counter — after one
        // call it reads 1, not 2.
        let src = "function f() {\n  const a = 1; const b = 2;\n  return a + b;\n}\nf();\n";
        let out = instrument(src, &opts("/abs/foo.ts")).expect("instrument");
        assert!(
            out.map.lines.contains(&2),
            "line 2 should carry a counter, got {:?}",
            out.map.lines
        );
        let cov = run_coverage(&out.code);
        assert_eq!(
            lookup_lines_n(&cov, "/abs/foo.ts", 2),
            1,
            "two statements on line 2 should dedupe to a single counter\ncode:\n{}",
            out.code
        );
    }

    #[test]
    fn coverage_map_contains_all_known_lines_and_fns_zero_initialized() {
        let src = "const a = 1;\nfunction h(){ return 2 }\n";
        let out = instrument(src, &opts("/abs/foo.ts")).expect("instrument");
        // Without executing, the prologue zero-initializes everything. Eval
        // only the prologue (skip the rest) by parsing the JS up to its first
        // newline-terminated stmt... simpler: eval the whole instrumented code
        // and assert that every recorded key is in __zero_coverage__.
        let cov = run_coverage(&out.code);
        // Lines and fns recorded in the map should exist as keys in the JS.
        for ln in &out.map.lines {
            let cnt = lookup_lines_n(&cov, "/abs/foo.ts", *ln);
            assert!(cnt >= 0, "line {ln} missing from __zero_coverage__");
        }
        for name in &out.map.fns {
            let cnt = lookup_fns(&cov, "/abs/foo.ts", name);
            assert!(cnt >= 0, "fn {name} missing from __zero_coverage__");
        }
        // h was never called.
        assert_eq!(lookup_fns(&cov, "/abs/foo.ts", "h"), 0);
    }

    #[test]
    fn preserves_source_map_back_to_original_ts() {
        let opts = TranspileOptions {
            filename: "/abs/foo.ts",
            inline_source_map: false,
            emit_source_map: true,
        };
        let out = instrument("const x: number = 1;\n", &opts).expect("instrument");
        let json = out.source_map.expect("source_map should be Some");
        assert!(
            json.contains(r#""version":3"#) || json.contains(r#""version": 3"#),
            "missing version in source map: {json}"
        );
    }

    #[test]
    fn scope_covers_src_ts_and_js() {
        let root = std::path::PathBuf::from("/proj");
        let s = CoverageScope::new(root, PathBuf::from("/proj/dist"));
        assert!(s.covers(Path::new("/proj/src/a.ts")));
        assert!(s.covers(Path::new("/proj/src/sub/b.js")));
    }

    #[test]
    fn scope_excludes_test_files() {
        let s = CoverageScope::new(PathBuf::from("/proj"), PathBuf::from("/proj/dist"));
        assert!(!s.covers(Path::new("/proj/src/a.test.ts")));
        assert!(!s.covers(Path::new("/proj/src/a.spec.js")));
    }

    #[test]
    fn scope_excludes_dot_zero() {
        let s = CoverageScope::new(PathBuf::from("/proj"), PathBuf::from("/proj/dist"));
        assert!(!s.covers(Path::new("/proj/src/.zero/components/Foo.ts")));
    }

    #[test]
    fn scope_excludes_out_dir() {
        let s = CoverageScope::new(PathBuf::from("/proj"), PathBuf::from("/proj/dist"));
        assert!(!s.covers(Path::new("/proj/dist/bundle.js")));
    }

    #[test]
    fn scope_excludes_node_modules() {
        let s = CoverageScope::new(PathBuf::from("/proj"), PathBuf::from("/proj/dist"));
        assert!(!s.covers(Path::new("/proj/src/node_modules/foo/index.js")));
    }

    #[test]
    fn aggregator_terminal_table_sorted_by_pct_ascending() {
        let mut agg = CoverageAggregator::new();
        agg.register(CoverageMap {
            file: PathBuf::from("/proj/src/low.ts"),
            lines: vec![1, 2, 3, 4],
            fns: vec!["f".into()],
        });
        agg.register(CoverageMap {
            file: PathBuf::from("/proj/src/high.ts"),
            lines: vec![1, 2],
            fns: vec![],
        });
        // 100% for high.ts, 25% for low.ts
        let run = serde_json::json!({
            "/proj/src/low.ts": { "lines": { "1": 1, "2": 0, "3": 0, "4": 0 }, "fns": { "f": 0 } },
            "/proj/src/high.ts": { "lines": { "1": 1, "2": 1 }, "fns": {} }
        });
        agg.ingest_run(&run);
        let mut buf: Vec<u8> = Vec::new();
        agg.write_terminal(&mut buf, Path::new("/proj")).unwrap();
        let s = String::from_utf8(buf).unwrap();
        let low_idx = s.find("src/low.ts").expect("low.ts in output");
        let high_idx = s.find("src/high.ts").expect("high.ts in output");
        assert!(low_idx < high_idx, "low coverage should appear first:\n{s}");
    }

    #[test]
    fn aggregator_json_paths_are_project_relative() {
        let mut agg = CoverageAggregator::new();
        agg.register(CoverageMap {
            file: PathBuf::from("/proj/src/a.ts"),
            lines: vec![1],
            fns: vec![],
        });
        let v = agg.to_json_value(Path::new("/proj"));
        let files = v["files"].as_object().expect("files object");
        assert!(
            files.contains_key("src/a.ts"),
            "expected project-relative key: {v}"
        );
    }

    #[test]
    fn aggregator_totals_sum_correctly() {
        let mut agg = CoverageAggregator::new();
        agg.register(CoverageMap {
            file: PathBuf::from("/proj/src/a.ts"),
            lines: vec![1, 2, 3],
            fns: vec!["f".into()],
        });
        agg.register(CoverageMap {
            file: PathBuf::from("/proj/src/b.ts"),
            lines: vec![1, 2],
            fns: vec![],
        });
        let run = serde_json::json!({
            "/proj/src/a.ts": { "lines": { "1": 1, "2": 1, "3": 0 }, "fns": { "f": 1 } },
            "/proj/src/b.ts": { "lines": { "1": 0, "2": 1 }, "fns": {} }
        });
        agg.ingest_run(&run);
        let v = agg.to_json_value(Path::new("/proj"));
        let totals = &v["totals"];
        assert_eq!(totals["lines"]["covered"], 3);
        assert_eq!(totals["lines"]["total"], 5);
        assert_eq!(totals["fns"]["covered"], 1);
        assert_eq!(totals["fns"]["total"], 1);
    }

    #[test]
    fn is_idempotent_within_one_module() {
        let src = "const a = 1;\nconst b = 2;\n";
        let first = instrument(src, &opts("/abs/foo.ts")).expect("first");
        // Second `instrument` over the same input must produce the same line
        // universe (no double-counting in the static map).
        let second = instrument(src, &opts("/abs/foo.ts")).expect("second");
        assert_eq!(first.map.lines, second.map.lines);
        assert_eq!(first.map.fns, second.map.fns);
    }
}
