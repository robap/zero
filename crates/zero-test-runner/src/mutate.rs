//! SWC-driven mutation generator.
//!
//! Two entry points share one AST visitor:
//!
//! - [`generate`] walks a source file's AST collecting every candidate
//!   mutation site without modifying the tree.
//! - [`apply`] re-parses, walks to the Nth site of a matching operator, and
//!   emits the mutated JS.
//!
//! Mutation locations are recorded in the original `.ts`/`.js` source
//! coordinates (1-based line + column). The emitted JS carries no source
//! map; the harness reports mutant failures by the pre-recorded position.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use swc_core::common::sync::Lrc;
use swc_core::common::{
    BytePos, FileName, GLOBALS, Globals, Mark, SourceMap as SwcSourceMap, Spanned,
};
use swc_core::ecma::ast::{
    BinExpr, BinaryOp, CallExpr, Callee, CondExpr, Decl, DoWhileStmt, EsVersion, Expr, ForStmt,
    Ident, IfStmt, Lit, MemberProp, Module, ModuleDecl, ModuleItem, Pat, Prop, PropOrSpread, Stmt,
    TsType, UnaryExpr, UnaryOp, VarDecl, VarDeclarator, WhileStmt,
};
use swc_core::ecma::codegen::Emitter;
use swc_core::ecma::codegen::text_writer::JsWriter;
use swc_core::ecma::parser::{Parser, StringInput, Syntax, TsSyntax, lexer::Lexer};
use swc_core::ecma::transforms::base::{fixer::fixer, hygiene::hygiene, resolver};
use swc_core::ecma::transforms::typescript::strip;
use swc_core::ecma::visit::{Visit, VisitMut, VisitMutWith, VisitWith};

use zero_transpile::TranspileError;

/// A family of code mutations. Each variant matches a set of AST shapes and
/// produces one mutant per match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Operator {
    /// Swap binary arithmetic operators: `+`↔`-`, `*`↔`/`, `%`→`*`.
    Arith,
    /// Swap relational operators (8 variants).
    Cmp,
    /// Swap logical operators: `&&`↔`||`.
    Bool,
    /// Wrap the test of `if`/`?:`/`while`/`do-while`/`for` in `!`.
    CondNeg,
    /// Boundary swap: `<`↔`<=`, `>`↔`>=`.
    Boundary,
    /// Flip boolean literals.
    LitBool,
    /// Replace small ints: `0`↔`1`.
    LitNum,
    /// Flip empty strings: `""`↔`"zero"`; non-empty literal → `""`.
    LitStr,
}

impl Operator {
    /// All eight operator families in declaration order.
    pub const ALL: &'static [Operator] = &[
        Operator::Arith,
        Operator::Cmp,
        Operator::Bool,
        Operator::CondNeg,
        Operator::Boundary,
        Operator::LitBool,
        Operator::LitNum,
        Operator::LitStr,
    ];

    /// Short string identifier.
    pub fn id(self) -> &'static str {
        match self {
            Operator::Arith => "arith",
            Operator::Cmp => "cmp",
            Operator::Bool => "bool",
            Operator::CondNeg => "cond_neg",
            Operator::Boundary => "boundary",
            Operator::LitBool => "lit_bool",
            Operator::LitNum => "lit_num",
            Operator::LitStr => "lit_str",
        }
    }

    /// Parse an operator ID back into the enum.
    pub fn parse(id: &str) -> Option<Operator> {
        Operator::ALL.iter().copied().find(|op| op.id() == id)
    }

    /// Comma-separated list of every accepted operator id, in declaration
    /// order. The exact string returned is parseable token-by-token by
    /// [`Operator::parse`] — split on `, ` and feed each piece back in.
    pub fn list_ids() -> String {
        Operator::ALL
            .iter()
            .map(|op| op.id())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// 0-based position in [`Operator::ALL`]. Stable across releases.
    pub fn index(self) -> usize {
        Operator::ALL.iter().position(|o| *o == self).unwrap()
    }
}

/// One concrete mutation.
#[derive(Debug, Clone)]
pub struct MutationSite {
    /// Absolute path of the source file the site belongs to.
    pub file: PathBuf,
    /// Which operator family generated this site.
    pub operator: Operator,
    /// 1-based line number in the original source.
    pub line: u32,
    /// 1-based column number in the original source.
    pub column: u32,
    /// Short text of the original code at the site (≤ 40 chars).
    pub original: String,
    /// Short text of the mutated code at the site (≤ 40 chars).
    pub replacement: String,
}

/// Options for [`generate`].
pub struct GenerateOptions<'a> {
    /// Which operator families to enumerate.
    pub operators: &'a [Operator],
    /// Optional cap on the total number of mutants returned.
    pub max_mutants: Option<usize>,
    /// If `Some`, mutants on lines not in this set are tallied as
    /// "unreachable" and dropped from the returned `Vec`.
    pub covered_lines: Option<&'a HashSet<u32>>,
}

/// Result of a [`generate`] pass.
#[derive(Debug)]
pub struct GenerateResult {
    /// Concrete mutation sites the caller will apply and execute.
    pub sites: Vec<MutationSite>,
    /// Total sites filtered out because `covered_lines` did not include
    /// their line. Aggregated across all operators.
    pub skipped_unreachable: usize,
    /// Total sites the visitor proved no-op by AST shape (e.g. `as const`
    /// arrays used only for type derivation). Aggregated across operators.
    pub skipped_equivalent_static: usize,
    /// Per-operator tally captured during the collect walk. Indexed by
    /// `Operator::index()`.
    pub per_operator: PerOperatorTally,
}

/// Per-operator counts produced by the collect-mode visitor. Indexed
/// the same way as [`Operator::ALL`].
#[derive(Debug, Default, Clone, Copy)]
pub struct PerOperatorTally {
    /// AST nodes the operator's swap function accepted, before any
    /// filtering. For arith this includes the string-concat exclusion
    /// (matches `+` only when both sides are not string literals).
    pub matched: [usize; 8],
    /// Subset of `matched` that was filtered by `covered_lines` and not
    /// returned in `sites`.
    pub unreachable: [usize; 8],
    /// Subset of `matched` that the visitor proved no-op by AST shape
    /// (static-equivalent) and dropped before adding to `sites`.
    pub equivalent_static: [usize; 8],
}

impl PerOperatorTally {
    /// Lookup helper.
    pub fn get(&self, op: Operator) -> OperatorCounts {
        OperatorCounts {
            matched: self.matched[op.index()],
            unreachable: self.unreachable[op.index()],
            equivalent_static: self.equivalent_static[op.index()],
        }
    }
}

/// View of a single operator's collect-mode counts.
#[derive(Debug, Clone, Copy)]
pub struct OperatorCounts {
    pub matched: usize,
    pub unreachable: usize,
    pub equivalent_static: usize,
}

/// Generate the mutation sites in `source`.
///
/// # Parameters
/// - `source`: TS/JS source text.
/// - `file`: absolute path; copied into every emitted [`MutationSite::file`].
/// - `opts`: operator filter, max-mutant cap, optional coverage filter.
///
/// # Returns
/// `Ok(GenerateResult { sites, skipped_unreachable, per_operator })`.
pub fn generate(
    source: &str,
    file: &Path,
    opts: &GenerateOptions<'_>,
) -> Result<GenerateResult, TranspileError> {
    let logical = file.to_string_lossy().into_owned();
    let cm: Lrc<SwcSourceMap> = Default::default();
    let fm = cm.new_source_file(
        Lrc::new(FileName::Custom(logical.clone())),
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
    let module = parser.parse_module().map_err(|e| TranspileError {
        file: logical.clone(),
        line: 0,
        column: 0,
        message: format!("parse error: {e:?}"),
    })?;

    // Run the static-equivalence pre-pass on the *pre-strip* module so
    // `as const` (TsConstAssertion) nodes are still present.
    let static_equivalence = analyze_static_equivalence(&module, &cm);

    let (sites, skipped, skipped_static, matched, unreachable_per_op, equivalent_static_per_op) =
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
            let mut v = MutateVisitor::new_collect(
                cm.clone(),
                file.to_path_buf(),
                opts.operators,
                opts.covered_lines,
                static_equivalence,
            );
            v.visit_mut_module(&mut module);
            (
                v.sites,
                v.skipped_unreachable,
                v.skipped_equivalent_static,
                v.matched,
                v.unreachable_per_op,
                v.equivalent_static_per_op,
            )
        });

    let limited = match opts.max_mutants {
        Some(max) if sites.len() > max => sites.into_iter().take(max).collect(),
        _ => sites,
    };
    Ok(GenerateResult {
        sites: limited,
        skipped_unreachable: skipped,
        skipped_equivalent_static: skipped_static,
        per_operator: PerOperatorTally {
            matched,
            unreachable: unreachable_per_op,
            equivalent_static: equivalent_static_per_op,
        },
    })
}

/// Apply `site` to `source` and return the mutated JS.
///
/// # Parameters
/// - `source`: TS/JS source text the site was generated from.
/// - `file`: logical filename (used for diagnostics).
/// - `site`: the site to apply.
///
/// # Returns
/// `Ok(String)` containing valid JS.
pub fn apply(source: &str, file: &Path, site: &MutationSite) -> Result<String, TranspileError> {
    let logical = file.to_string_lossy().into_owned();
    let cm: Lrc<SwcSourceMap> = Default::default();
    let fm = cm.new_source_file(
        Lrc::new(FileName::Custom(logical.clone())),
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
    let module = parser.parse_module().map_err(|e| TranspileError {
        file: logical.clone(),
        line: 0,
        column: 0,
        message: format!("parse error: {e:?}"),
    })?;

    let target_index = locate_index(source, file, site)?;

    // Mirror `generate()`'s pre-pass so Apply-mode visits skip the same
    // literals Collect-mode skipped — otherwise the Nth-site index drifts and
    // the wrong literal gets mutated.
    let static_equivalence = analyze_static_equivalence(&module, &cm);

    let module = GLOBALS.set(&Globals::new(), || {
        let unresolved_mark = Mark::new();
        let top_level_mark = Mark::new();
        let mut program = swc_core::ecma::ast::Program::Module(module);
        program.mutate(resolver(unresolved_mark, top_level_mark, true));
        program.mutate(strip(unresolved_mark, top_level_mark));
        let mut module = match program {
            swc_core::ecma::ast::Program::Module(m) => m,
            swc_core::ecma::ast::Program::Script(_) => unreachable!(),
        };
        let mut v = MutateVisitor::new_apply(
            cm.clone(),
            file.to_path_buf(),
            site.operator,
            target_index,
            static_equivalence,
        );
        v.visit_mut_module(&mut module);
        let mut program = swc_core::ecma::ast::Program::Module(module);
        program.mutate(hygiene());
        program.mutate(fixer(None));
        match program {
            swc_core::ecma::ast::Program::Module(m) => m,
            _ => unreachable!(),
        }
    });

    emit_js(&module, &cm, &logical)
}

/// Look up the 0-based index of `site` among same-operator sites in source
/// order. We re-run a collect pass and find the matching `(line, column)`.
fn locate_index(source: &str, file: &Path, site: &MutationSite) -> Result<usize, TranspileError> {
    let ops = [site.operator];
    let opts = GenerateOptions {
        operators: &ops,
        max_mutants: None,
        covered_lines: None,
    };
    let sites = generate(source, file, &opts)?.sites;
    sites
        .iter()
        .position(|s| s.line == site.line && s.column == site.column)
        .ok_or_else(|| TranspileError {
            file: file.to_string_lossy().into_owned(),
            line: site.line,
            column: site.column,
            message: format!(
                "mutation site not found in apply pass ({:?}:{}:{})",
                site.operator, site.line, site.column
            ),
        })
}

fn emit_js(
    module: &Module,
    cm: &Lrc<SwcSourceMap>,
    logical: &str,
) -> Result<String, TranspileError> {
    let mut code_buf: Vec<u8> = Vec::new();
    {
        let writer = JsWriter::new(cm.clone(), "\n", &mut code_buf, None);
        let mut emitter = Emitter {
            cfg: swc_core::ecma::codegen::Config::default(),
            cm: cm.clone(),
            comments: None,
            wr: writer,
        };
        if let Err(e) = emitter.emit_module(module) {
            return Err(TranspileError {
                file: logical.to_string(),
                line: 0,
                column: 0,
                message: format!("codegen error: {e}"),
            });
        }
    }
    String::from_utf8(code_buf).map_err(|e| TranspileError {
        file: logical.to_string(),
        line: 0,
        column: 0,
        message: format!("non-UTF-8 codegen output: {e}"),
    })
}

/// Render an expression to source text via SWC's printer, truncated to 40
/// chars. Used only for report messages.
fn print_expr(expr: &Expr, cm: &Lrc<SwcSourceMap>) -> String {
    use swc_core::ecma::ast::Stmt;
    let stmt = Stmt::Expr(swc_core::ecma::ast::ExprStmt {
        span: Default::default(),
        expr: Box::new(expr.clone()),
    });
    let module = Module {
        span: Default::default(),
        body: vec![swc_core::ecma::ast::ModuleItem::Stmt(stmt)],
        shebang: None,
    };
    let mut code_buf: Vec<u8> = Vec::new();
    {
        let writer = JsWriter::new(cm.clone(), "\n", &mut code_buf, None);
        let mut emitter = Emitter {
            cfg: swc_core::ecma::codegen::Config::default(),
            cm: cm.clone(),
            comments: None,
            wr: writer,
        };
        if emitter.emit_module(&module).is_err() {
            return "<print-error>".into();
        }
    }
    let s = String::from_utf8_lossy(&code_buf).to_string();
    let s = s.trim().trim_end_matches(';').trim().to_string();
    if s.chars().count() > 40 {
        let truncated: String = s.chars().take(39).collect();
        format!("{truncated}…")
    } else {
        s
    }
}

/// Result of the static-equivalence pre-pass. Holds the `(line, column)` of
/// every literal the visitor should tally as `equivalent_static` instead of
/// emitting as a [`MutationSite`]. The pre-pass runs *before* TS strip so
/// `as const` assertions are still visible in the AST.
#[derive(Debug, Default)]
struct StaticEquivalence {
    sites: HashSet<(u32, u32)>,
}

impl StaticEquivalence {
    fn contains(&self, line: u32, column: u32) -> bool {
        self.sites.contains(&(line, column))
    }
}

/// A module-level `const Name = [...] as const` binding captured by the
/// pre-pass first phase. The pre-pass second phase decides whether it
/// qualifies for static-equivalent tallying.
#[derive(Debug)]
struct AsConstCandidate {
    name: swc_core::atoms::Atom,
    /// `BytePos` of the binding identifier itself, so the reference walker
    /// can skip the declaration site.
    binding_lo: BytePos,
    /// `(line, column)` of every literal element in the array body.
    members: Vec<(u32, u32)>,
}

/// Walk `module` and produce a [`StaticEquivalence`] map. Must be called on
/// the parsed AST *before* TS type stripping — `as const` (`TsConstAssertion`)
/// nodes are gone after `strip()`.
fn analyze_static_equivalence(module: &Module, cm: &Lrc<SwcSourceMap>) -> StaticEquivalence {
    let mut out = StaticEquivalence::default();
    analyze_as_const(module, cm, &mut out);
    analyze_signal_init(module, cm, &mut out);
    out
}

fn analyze_as_const(module: &Module, cm: &Lrc<SwcSourceMap>, out: &mut StaticEquivalence) {
    let candidates = collect_as_const_candidates(module, cm);
    if candidates.is_empty() {
        return;
    }
    let mut bad: HashSet<swc_core::atoms::Atom> = HashSet::new();
    let mut analyzer = ReferenceAnalyzer {
        candidates: &candidates,
        ts_depth: 0,
        bad: &mut bad,
    };
    module.visit_with(&mut analyzer);
    for c in &candidates {
        if !bad.contains(&c.name) {
            for pos in &c.members {
                out.sites.insert(*pos);
            }
        }
    }
}

/// First pre-pass phase: find every module-level `const <Name> = [...] as const`
/// and capture each array literal's element positions.
fn collect_as_const_candidates(module: &Module, cm: &Lrc<SwcSourceMap>) -> Vec<AsConstCandidate> {
    let mut out = Vec::new();
    for_each_top_level_var(module, |v| extract_as_const_decls(v, cm, &mut out));
    out
}

/// Apply `f` to every module-level `VarDecl`, including those wrapped in an
/// `export` declaration.
fn for_each_top_level_var<F: FnMut(&VarDecl)>(module: &Module, mut f: F) {
    for item in &module.body {
        match item {
            ModuleItem::Stmt(Stmt::Decl(Decl::Var(v))) => f(v),
            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(e)) => {
                if let Decl::Var(v) = &e.decl {
                    f(v);
                }
            }
            _ => {}
        }
    }
}

fn extract_as_const_decls(var: &VarDecl, cm: &Lrc<SwcSourceMap>, out: &mut Vec<AsConstCandidate>) {
    for d in &var.decls {
        if let Some(c) = match_as_const_declarator(d, cm) {
            out.push(c);
        }
    }
}

fn match_as_const_declarator(
    d: &VarDeclarator,
    cm: &Lrc<SwcSourceMap>,
) -> Option<AsConstCandidate> {
    let binding = match &d.name {
        Pat::Ident(b) => &b.id,
        _ => return None,
    };
    let init = d.init.as_deref()?;
    let assertion = match init {
        Expr::TsConstAssertion(a) => a,
        _ => return None,
    };
    let arr = match &*assertion.expr {
        Expr::Array(a) => a,
        _ => return None,
    };
    let mut members = Vec::new();
    for elem in &arr.elems {
        if let Some(el) = elem
            && let Expr::Lit(lit) = &*el.expr
            && let Some(pos) = literal_line_col(lit, cm)
        {
            members.push(pos);
        }
    }
    Some(AsConstCandidate {
        name: binding.sym.clone(),
        binding_lo: binding.span.lo(),
        members,
    })
}

fn literal_line_col(lit: &Lit, cm: &Lrc<SwcSourceMap>) -> Option<(u32, u32)> {
    match lit {
        Lit::Str(_) | Lit::Num(_) | Lit::Bool(_) => {
            let pos = cm.lookup_char_pos(lit.span().lo());
            Some((pos.line as u32, (pos.col_display + 1) as u32))
        }
        _ => None,
    }
}

/// Second pre-pass phase: walk the module and disqualify any candidate whose
/// binding name appears in a runtime position (anywhere outside a TS type
/// node, excluding the binding declaration itself).
struct ReferenceAnalyzer<'a> {
    candidates: &'a [AsConstCandidate],
    ts_depth: u32,
    bad: &'a mut HashSet<swc_core::atoms::Atom>,
}

impl<'a> ReferenceAnalyzer<'a> {
    fn candidate_for(&self, ident: &Ident) -> Option<&AsConstCandidate> {
        self.candidates.iter().find(|c| c.name == ident.sym)
    }
}

impl<'a> Visit for ReferenceAnalyzer<'a> {
    fn visit_ts_type(&mut self, n: &TsType) {
        self.ts_depth += 1;
        n.visit_children_with(self);
        self.ts_depth -= 1;
    }

    fn visit_ident(&mut self, n: &Ident) {
        if self.ts_depth > 0 {
            return;
        }
        if let Some(c) = self.candidate_for(n)
            && n.span.lo() != c.binding_lo
        {
            self.bad.insert(c.name.clone());
        }
    }
}

/// A module-level `const Name = signal({...})` (or `computed({...})`)
/// candidate captured by the pre-pass first phase.
#[derive(Debug)]
struct SignalCandidate {
    name: swc_core::atoms::Atom,
    binding_lo: BytePos,
    /// `(line, column)` of each literal property *value* in the initial
    /// object passed to `signal(...)`.
    property_values: Vec<(u32, u32)>,
}

fn analyze_signal_init(module: &Module, cm: &Lrc<SwcSourceMap>, out: &mut StaticEquivalence) {
    let candidates = collect_signal_candidates(module, cm);
    if candidates.is_empty() {
        return;
    }
    let mut set_calls: HashMap<swc_core::atoms::Atom, Vec<BytePos>> = HashMap::new();
    let mut other_refs: HashMap<swc_core::atoms::Atom, Vec<BytePos>> = HashMap::new();
    let mut walker = SignalReferenceWalker {
        candidates: &candidates,
        ts_depth: 0,
        exempt: HashSet::new(),
        set_calls: &mut set_calls,
        other_refs: &mut other_refs,
    };
    module.visit_with(&mut walker);
    for c in &candidates {
        let sets = set_calls.get(&c.name);
        let Some(sets) = sets.filter(|v| !v.is_empty()) else {
            continue; // no .set() calls → can't prove overwrite-before-read
        };
        let min_set = *sets.iter().min().unwrap();
        let refs_empty = other_refs.get(&c.name).is_none_or(|v| v.is_empty());
        let refs_all_after_set = other_refs
            .get(&c.name)
            .is_none_or(|v| v.iter().all(|r| *r > min_set));
        if refs_empty || refs_all_after_set {
            for pos in &c.property_values {
                out.sites.insert(*pos);
            }
        }
    }
}

fn collect_signal_candidates(module: &Module, cm: &Lrc<SwcSourceMap>) -> Vec<SignalCandidate> {
    let mut out = Vec::new();
    for_each_top_level_var(module, |v| {
        for d in &v.decls {
            if let Some(c) = match_signal_declarator(d, cm) {
                out.push(c);
            }
        }
    });
    out
}

fn match_signal_declarator(d: &VarDeclarator, cm: &Lrc<SwcSourceMap>) -> Option<SignalCandidate> {
    let binding = match &d.name {
        Pat::Ident(b) => &b.id,
        _ => return None,
    };
    let call = match d.init.as_deref()? {
        Expr::Call(c) => c,
        _ => return None,
    };
    let callee_ident = match &call.callee {
        Callee::Expr(e) => match &**e {
            Expr::Ident(i) => i,
            _ => return None,
        },
        _ => return None,
    };
    if &*callee_ident.sym != "signal" && &*callee_ident.sym != "computed" {
        return None;
    }
    let first_arg = call.args.first()?;
    if first_arg.spread.is_some() {
        return None;
    }
    let obj = match &*first_arg.expr {
        Expr::Object(o) => o,
        _ => return None,
    };
    let mut property_values = Vec::new();
    for prop in &obj.props {
        if let PropOrSpread::Prop(p) = prop
            && let Prop::KeyValue(kv) = &**p
            && let Expr::Lit(lit) = &*kv.value
            && let Some(pos) = literal_line_col(lit, cm)
        {
            property_values.push(pos);
        }
    }
    Some(SignalCandidate {
        name: binding.sym.clone(),
        binding_lo: binding.span.lo(),
        property_values,
    })
}

/// Walks the module once collecting per-candidate `.set()` call positions and
/// "other reference" positions used to decide dominance.
struct SignalReferenceWalker<'a> {
    candidates: &'a [SignalCandidate],
    ts_depth: u32,
    /// `BytePos` of `Ident` nodes that are the receiver of a recognised
    /// `Name.set(...)` call; `visit_ident` skips these so they don't count
    /// as "other references."
    exempt: HashSet<BytePos>,
    set_calls: &'a mut HashMap<swc_core::atoms::Atom, Vec<BytePos>>,
    other_refs: &'a mut HashMap<swc_core::atoms::Atom, Vec<BytePos>>,
}

impl<'a> Visit for SignalReferenceWalker<'a> {
    fn visit_ts_type(&mut self, n: &TsType) {
        self.ts_depth += 1;
        n.visit_children_with(self);
        self.ts_depth -= 1;
    }

    fn visit_call_expr(&mut self, n: &CallExpr) {
        if let Callee::Expr(e) = &n.callee
            && let Expr::Member(m) = &**e
            && let Expr::Ident(obj_ident) = &*m.obj
            && let MemberProp::Ident(prop_ident) = &m.prop
            && &*prop_ident.sym == "set"
            && self.candidates.iter().any(|c| c.name == obj_ident.sym)
        {
            self.exempt.insert(obj_ident.span.lo());
            self.set_calls
                .entry(obj_ident.sym.clone())
                .or_default()
                .push(n.span.lo());
        }
        n.visit_children_with(self);
    }

    fn visit_ident(&mut self, n: &Ident) {
        if self.ts_depth > 0 {
            return;
        }
        let Some(c) = self.candidates.iter().find(|c| c.name == n.sym) else {
            return;
        };
        if n.span.lo() == c.binding_lo {
            return;
        }
        if self.exempt.contains(&n.span.lo()) {
            return;
        }
        self.other_refs
            .entry(c.name.clone())
            .or_default()
            .push(n.span.lo());
    }
}

enum Mode {
    Collect,
    Apply {
        operator: Operator,
        target_index: usize,
    },
}

struct MutateVisitor<'a> {
    cm: Lrc<SwcSourceMap>,
    file: PathBuf,
    mode: Mode,
    operators_filter: Option<&'a [Operator]>,
    covered_lines: Option<&'a HashSet<u32>>,
    /// Literal (line, column) positions the pre-pass marked as
    /// static-equivalent. In Apply mode this is always empty — apply-time
    /// visits never reach these positions because they were dropped from
    /// `sites` at collect time.
    static_equivalence: StaticEquivalence,
    /// Start line of the innermost enclosing statement / module item.
    /// Reachability is judged on this line (which carries a coverage counter)
    /// rather than the site's own line, so a mutable token on a continuation
    /// line of a multi-line statement is reached whenever the statement
    /// executed. 0 until the first statement/item is entered.
    reach_line: u32,
    sites: Vec<MutationSite>,
    counts: [usize; 8],
    skipped_unreachable: usize,
    skipped_equivalent_static: usize,
    matched: [usize; 8],
    unreachable_per_op: [usize; 8],
    equivalent_static_per_op: [usize; 8],
}

impl<'a> MutateVisitor<'a> {
    fn new_collect(
        cm: Lrc<SwcSourceMap>,
        file: PathBuf,
        operators_filter: &'a [Operator],
        covered_lines: Option<&'a HashSet<u32>>,
        static_equivalence: StaticEquivalence,
    ) -> Self {
        Self {
            cm,
            file,
            mode: Mode::Collect,
            operators_filter: Some(operators_filter),
            covered_lines,
            static_equivalence,
            reach_line: 0,
            sites: Vec::new(),
            counts: [0; 8],
            skipped_unreachable: 0,
            skipped_equivalent_static: 0,
            matched: [0; 8],
            unreachable_per_op: [0; 8],
            equivalent_static_per_op: [0; 8],
        }
    }

    fn new_apply(
        cm: Lrc<SwcSourceMap>,
        file: PathBuf,
        operator: Operator,
        target_index: usize,
        static_equivalence: StaticEquivalence,
    ) -> Self {
        Self {
            cm,
            file,
            mode: Mode::Apply {
                operator,
                target_index,
            },
            operators_filter: None,
            covered_lines: None,
            static_equivalence,
            reach_line: 0,
            sites: Vec::new(),
            counts: [0; 8],
            skipped_unreachable: 0,
            skipped_equivalent_static: 0,
            matched: [0; 8],
            unreachable_per_op: [0; 8],
            equivalent_static_per_op: [0; 8],
        }
    }

    fn line_col<S: Spanned>(&self, n: &S) -> (u32, u32) {
        let pos = self.cm.lookup_char_pos(n.span().lo());
        (pos.line as u32, (pos.col_display + 1) as u32)
    }

    fn filter_allows(&self, op: Operator) -> bool {
        match self.operators_filter {
            Some(list) => list.contains(&op),
            None => true,
        }
    }

    /// Decide whether the current node should be mutated for `op`.
    ///
    /// In Collect mode: records a site (after filter / coverage rules) and
    /// always returns false. In Apply mode: returns true exactly when the
    /// current node is the Nth same-operator node being visited.
    fn check(
        &mut self,
        op: Operator,
        line: u32,
        column: u32,
        original: &str,
        replacement: &str,
    ) -> bool {
        let idx = op.index();
        match self.mode {
            Mode::Collect => {
                if !self.filter_allows(op) {
                    return false;
                }
                self.matched[idx] += 1;
                // Static-equivalence runs *before* the coverage filter so a
                // statically-proven no-op never double-counts as unreachable.
                if self.static_equivalence.contains(line, column) {
                    self.skipped_equivalent_static += 1;
                    self.equivalent_static_per_op[idx] += 1;
                    return false;
                }
                // Judge reachability on the enclosing statement / module item
                // line (which carries a coverage counter), not the site's own
                // line — so a token on a continuation line of a multi-line
                // statement is reached whenever that statement executed. Fall
                // back to `line` only if no statement was entered (sentinel 0,
                // which cannot happen for a real site).
                if let Some(cov) = self.covered_lines {
                    let reach = if self.reach_line != 0 {
                        self.reach_line
                    } else {
                        line
                    };
                    if !cov.contains(&reach) {
                        self.skipped_unreachable += 1;
                        self.unreachable_per_op[idx] += 1;
                        return false;
                    }
                }
                self.counts[idx] += 1;
                self.sites.push(MutationSite {
                    file: self.file.clone(),
                    operator: op,
                    line,
                    column,
                    original: original.to_string(),
                    replacement: replacement.to_string(),
                });
                false
            }
            Mode::Apply {
                operator,
                target_index,
            } => {
                if operator != op {
                    return false;
                }
                // Apply mode must skip the same literals Collect skipped, or
                // the Nth-site index drifts and the wrong literal is mutated.
                if self.static_equivalence.contains(line, column) {
                    return false;
                }
                let current = self.counts[idx];
                self.counts[idx] += 1;
                current == target_index
            }
        }
    }
}

impl<'a> VisitMut for MutateVisitor<'a> {
    fn visit_mut_module_item(&mut self, item: &mut ModuleItem) {
        // Track the enclosing module item's start line so a site on a
        // continuation line of a multi-line top-level initializer is judged
        // reachable by the item's (counted) start line.
        let prev = self.reach_line;
        self.reach_line = self.line_col(item).0;
        item.visit_mut_children_with(self);
        self.reach_line = prev;
    }

    fn visit_mut_stmt(&mut self, stmt: &mut Stmt) {
        // Track the innermost enclosing statement's start line; nested
        // statements override and restore it on the way back up.
        let prev = self.reach_line;
        self.reach_line = self.line_col(stmt).0;
        stmt.visit_mut_children_with(self);
        self.reach_line = prev;
    }

    fn visit_mut_bin_expr(&mut self, b: &mut BinExpr) {
        // Walk inner expressions first so nested sites are enumerated before
        // the outer site.
        b.visit_mut_children_with(self);

        let (line, col) = self.line_col(b);
        if let Some(repl) = arith_swap(b.op) {
            let skip_string_concat = b.op == BinaryOp::Add
                && matches!(*b.left, Expr::Lit(Lit::Str(_)))
                && matches!(*b.right, Expr::Lit(Lit::Str(_)));
            if !skip_string_concat {
                let original = print_expr(&Expr::Bin(b.clone()), &self.cm);
                let mut mutated = b.clone();
                mutated.op = repl;
                let replacement = print_expr(&Expr::Bin(mutated), &self.cm);
                if self.check(Operator::Arith, line, col, &original, &replacement) {
                    b.op = repl;
                }
            }
        }
        if let Some(repl) = cmp_swap(b.op) {
            let original = print_expr(&Expr::Bin(b.clone()), &self.cm);
            let mut mutated = b.clone();
            mutated.op = repl;
            let replacement = print_expr(&Expr::Bin(mutated), &self.cm);
            if self.check(Operator::Cmp, line, col, &original, &replacement) {
                b.op = repl;
            }
        }
        if let Some(repl) = bool_swap(b.op) {
            let original = print_expr(&Expr::Bin(b.clone()), &self.cm);
            let mut mutated = b.clone();
            mutated.op = repl;
            let replacement = print_expr(&Expr::Bin(mutated), &self.cm);
            if self.check(Operator::Bool, line, col, &original, &replacement) {
                b.op = repl;
            }
        }
        if let Some(repl) = boundary_swap(b.op) {
            let original = print_expr(&Expr::Bin(b.clone()), &self.cm);
            let mut mutated = b.clone();
            mutated.op = repl;
            let replacement = print_expr(&Expr::Bin(mutated), &self.cm);
            if self.check(Operator::Boundary, line, col, &original, &replacement) {
                b.op = repl;
            }
        }
    }

    fn visit_mut_if_stmt(&mut self, s: &mut IfStmt) {
        self.handle_cond_neg(&mut s.test);
        s.visit_mut_children_with(self);
    }

    fn visit_mut_cond_expr(&mut self, c: &mut CondExpr) {
        self.handle_cond_neg(&mut c.test);
        c.visit_mut_children_with(self);
    }

    fn visit_mut_while_stmt(&mut self, s: &mut WhileStmt) {
        self.handle_cond_neg(&mut s.test);
        s.visit_mut_children_with(self);
    }

    fn visit_mut_do_while_stmt(&mut self, s: &mut DoWhileStmt) {
        self.handle_cond_neg(&mut s.test);
        s.visit_mut_children_with(self);
    }

    fn visit_mut_for_stmt(&mut self, s: &mut ForStmt) {
        if let Some(test) = &mut s.test {
            self.handle_cond_neg(test);
        }
        s.visit_mut_children_with(self);
    }

    fn visit_mut_lit(&mut self, lit: &mut Lit) {
        let (line, col) = self.line_col(lit);
        match lit {
            Lit::Bool(b) => {
                let original = if b.value { "true" } else { "false" };
                let replacement = if b.value { "false" } else { "true" };
                if self.check(Operator::LitBool, line, col, original, replacement) {
                    b.value = !b.value;
                }
            }
            Lit::Num(n) if n.value == 0.0 || n.value == 1.0 => {
                let original = if n.value == 0.0 { "0" } else { "1" };
                let replacement = if n.value == 0.0 { "1" } else { "0" };
                if self.check(Operator::LitNum, line, col, original, replacement) {
                    n.value = if n.value == 0.0 { 1.0 } else { 0.0 };
                    n.raw = None;
                }
            }
            Lit::Str(s) => {
                let is_empty = s.value.is_empty();
                let original = if is_empty {
                    "\"\"".to_string()
                } else {
                    format!("\"{}\"", s.value.as_str().unwrap_or(""))
                };
                let replacement = if is_empty {
                    "\"zero\"".to_string()
                } else {
                    "\"\"".to_string()
                };
                if self.check(Operator::LitStr, line, col, &original, &replacement) {
                    if is_empty {
                        s.value = "zero".into();
                    } else {
                        s.value = "".into();
                    }
                    s.raw = None;
                }
            }
            _ => {}
        }
    }
}

impl<'a> MutateVisitor<'a> {
    /// Handle cond_neg for a boxed `Expr` test (if / while / cond / for / do).
    fn handle_cond_neg(&mut self, test: &mut Box<Expr>) {
        let (line, col) = self.line_col(test.as_ref());
        let original = print_expr(test, &self.cm);
        let replacement = format!("!{}", original);
        if self.check(Operator::CondNeg, line, col, &original, &replacement) {
            wrap_with_bang(test);
        }
    }
}

fn wrap_with_bang(test: &mut Box<Expr>) {
    let inner = std::mem::replace(
        test.as_mut(),
        Expr::Lit(Lit::Bool(swc_core::ecma::ast::Bool {
            span: Default::default(),
            value: false,
        })),
    );
    let wrapped = Expr::Unary(UnaryExpr {
        span: Default::default(),
        op: UnaryOp::Bang,
        arg: Box::new(inner),
    });
    **test = wrapped;
}

fn arith_swap(op: BinaryOp) -> Option<BinaryOp> {
    match op {
        BinaryOp::Add => Some(BinaryOp::Sub),
        BinaryOp::Sub => Some(BinaryOp::Add),
        BinaryOp::Mul => Some(BinaryOp::Div),
        BinaryOp::Div => Some(BinaryOp::Mul),
        BinaryOp::Mod => Some(BinaryOp::Mul),
        _ => None,
    }
}

fn cmp_swap(op: BinaryOp) -> Option<BinaryOp> {
    match op {
        BinaryOp::Lt => Some(BinaryOp::GtEq),
        BinaryOp::LtEq => Some(BinaryOp::Gt),
        BinaryOp::Gt => Some(BinaryOp::LtEq),
        BinaryOp::GtEq => Some(BinaryOp::Lt),
        BinaryOp::EqEq => Some(BinaryOp::NotEq),
        BinaryOp::NotEq => Some(BinaryOp::EqEq),
        BinaryOp::EqEqEq => Some(BinaryOp::NotEqEq),
        BinaryOp::NotEqEq => Some(BinaryOp::EqEqEq),
        _ => None,
    }
}

fn bool_swap(op: BinaryOp) -> Option<BinaryOp> {
    match op {
        BinaryOp::LogicalAnd => Some(BinaryOp::LogicalOr),
        BinaryOp::LogicalOr => Some(BinaryOp::LogicalAnd),
        _ => None,
    }
}

fn boundary_swap(op: BinaryOp) -> Option<BinaryOp> {
    match op {
        BinaryOp::Lt => Some(BinaryOp::LtEq),
        BinaryOp::LtEq => Some(BinaryOp::Lt),
        BinaryOp::Gt => Some(BinaryOp::GtEq),
        BinaryOp::GtEq => Some(BinaryOp::Gt),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::PathBuf;

    fn opts<'a>(operators: &'a [Operator]) -> GenerateOptions<'a> {
        GenerateOptions {
            operators,
            max_mutants: None,
            covered_lines: None,
        }
    }

    #[test]
    fn arith_operator_generates_swap() {
        let src = "const x = 1 + 2;\n";
        let ops = vec![Operator::Arith];
        let r = generate(src, &PathBuf::from("/abs/a.ts"), &opts(&ops)).expect("generate");
        let sites = r.sites;
        assert_eq!(r.skipped_unreachable, 0);
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].operator, Operator::Arith);
        assert!(
            sites[0].original.contains('+'),
            "original: {}",
            sites[0].original
        );
        assert!(
            sites[0].replacement.contains('-'),
            "replacement: {}",
            sites[0].replacement
        );
    }

    #[test]
    fn cmp_operator_swaps_relational() {
        let src = "const r = a < b;\n";
        let ops = vec![Operator::Cmp];
        let sites = generate(src, &PathBuf::from("/abs/a.ts"), &opts(&ops))
            .expect("g")
            .sites;
        assert_eq!(sites.len(), 1);
        assert!(
            sites[0].replacement.contains(">="),
            "replacement: {}",
            sites[0].replacement
        );
    }

    #[test]
    fn bool_operator_swaps_logical() {
        let src = "const r = a && b;\n";
        let ops = vec![Operator::Bool];
        let sites = generate(src, &PathBuf::from("/abs/a.ts"), &opts(&ops))
            .expect("g")
            .sites;
        assert_eq!(sites.len(), 1);
        assert!(
            sites[0].replacement.contains("||"),
            "replacement: {}",
            sites[0].replacement
        );
    }

    #[test]
    fn cond_neg_wraps_if_test() {
        let src = "if (a) { f(); }\n";
        let ops = vec![Operator::CondNeg];
        let sites = generate(src, &PathBuf::from("/abs/a.ts"), &opts(&ops))
            .expect("g")
            .sites;
        assert_eq!(sites.len(), 1);
        assert!(
            sites[0].replacement.starts_with('!'),
            "replacement: {}",
            sites[0].replacement
        );
    }

    #[test]
    fn boundary_swaps_lt_to_lte() {
        let src = "const r = a < b;\n";
        let ops = vec![Operator::Boundary];
        let sites = generate(src, &PathBuf::from("/abs/a.ts"), &opts(&ops))
            .expect("g")
            .sites;
        assert_eq!(sites.len(), 1);
        assert!(
            sites[0].replacement.contains("<="),
            "replacement: {}",
            sites[0].replacement
        );
    }

    #[test]
    fn lit_bool_flips() {
        let src = "const t = true;\n";
        let ops = vec![Operator::LitBool];
        let sites = generate(src, &PathBuf::from("/abs/a.ts"), &opts(&ops))
            .expect("g")
            .sites;
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].original, "true");
        assert_eq!(sites[0].replacement, "false");
    }

    #[test]
    fn lit_num_swaps_zero_and_one() {
        let src = "const a = 0; const b = 1;\n";
        let ops = vec![Operator::LitNum];
        let sites = generate(src, &PathBuf::from("/abs/a.ts"), &opts(&ops))
            .expect("g")
            .sites;
        assert_eq!(sites.len(), 2);
        let pairs: Vec<(String, String)> = sites
            .iter()
            .map(|s| (s.original.clone(), s.replacement.clone()))
            .collect();
        assert!(pairs.contains(&("0".to_string(), "1".to_string())));
        assert!(pairs.contains(&("1".to_string(), "0".to_string())));
    }

    #[test]
    fn lit_str_swaps_empty_and_nonempty() {
        let src = r#"const a = ""; const b = "abc";"#;
        let ops = vec![Operator::LitStr];
        let sites = generate(src, &PathBuf::from("/abs/a.ts"), &opts(&ops))
            .expect("g")
            .sites;
        assert_eq!(sites.len(), 2);
        let pairs: Vec<(String, String)> = sites
            .iter()
            .map(|s| (s.original.clone(), s.replacement.clone()))
            .collect();
        assert!(pairs.iter().any(|(o, r)| o == "\"\"" && r == "\"zero\""));
        assert!(pairs.iter().any(|(o, r)| o == "\"abc\"" && r == "\"\""));
    }

    #[test]
    fn respects_operator_filter() {
        // Source mixing arith (+) and cmp (<). With operators=[Arith] only
        // arith mutants should be returned.
        let src = "const r = (a + b) < c;\n";
        let ops = vec![Operator::Arith];
        let sites = generate(src, &PathBuf::from("/abs/a.ts"), &opts(&ops))
            .expect("g")
            .sites;
        assert!(sites.iter().all(|s| s.operator == Operator::Arith));
        assert!(!sites.is_empty());
    }

    #[test]
    fn respects_max_mutants() {
        // 10 arith sites in one expression.
        let src = "const r = 1+1+1+1+1+1+1+1+1+1+1;\n";
        let opts = GenerateOptions {
            operators: &[Operator::Arith],
            max_mutants: Some(3),
            covered_lines: None,
        };
        let sites = generate(src, &PathBuf::from("/abs/a.ts"), &opts)
            .expect("g")
            .sites;
        assert_eq!(sites.len(), 3);
    }

    #[test]
    fn skips_sites_on_uncovered_lines() {
        let src = "const a = 1 + 2;\nconst b = 3 + 4;\n";
        let mut covered: HashSet<u32> = HashSet::new();
        covered.insert(1);
        let opts = GenerateOptions {
            operators: &[Operator::Arith],
            max_mutants: None,
            covered_lines: Some(&covered),
        };
        let r = generate(src, &PathBuf::from("/abs/a.ts"), &opts).expect("g");
        assert_eq!(r.sites.len(), 1);
        assert_eq!(r.sites[0].line, 1);
        assert_eq!(r.skipped_unreachable, 1);
    }

    #[test]
    fn string_plus_is_not_mutated_as_arith() {
        let src = r#"const r = "a" + "b";"#;
        let sites = generate(src, &PathBuf::from("/abs/a.ts"), &opts(&[Operator::Arith]))
            .expect("g")
            .sites;
        assert_eq!(sites.len(), 0);
    }

    #[test]
    fn apply_emits_valid_js_for_arith() {
        let src = "const x = 1 + 2;\n";
        let sites = generate(src, &PathBuf::from("/abs/a.ts"), &opts(&[Operator::Arith]))
            .expect("g")
            .sites;
        let mutated = apply(src, &PathBuf::from("/abs/a.ts"), &sites[0]).expect("apply");
        assert!(
            mutated.contains("1 - 2"),
            "expected '1 - 2' in output:\n{mutated}"
        );
    }

    #[test]
    fn apply_emits_valid_js_for_cond_neg() {
        let src = "if (a) { f(); }\n";
        let sites = generate(
            src,
            &PathBuf::from("/abs/a.ts"),
            &opts(&[Operator::CondNeg]),
        )
        .expect("g")
        .sites;
        let mutated = apply(src, &PathBuf::from("/abs/a.ts"), &sites[0]).expect("apply");
        assert!(
            mutated.contains("!a"),
            "expected '!a' in output:\n{mutated}"
        );
    }

    #[test]
    fn operator_id_round_trip() {
        for op in Operator::ALL {
            assert_eq!(Operator::parse(op.id()), Some(*op));
        }
    }

    #[test]
    fn arith_matches_demo_division_in_math_ceil() {
        let src = "const pages = Math.ceil(tc / PAGE_SIZE);\n";
        let sites = generate(
            src,
            &PathBuf::from("/abs/demo.ts"),
            &opts(&[Operator::Arith]),
        )
        .expect("g")
        .sites;
        assert!(
            sites.iter().any(|s| s.operator == Operator::Arith),
            "expected arith site for `tc / PAGE_SIZE`, got {sites:?}"
        );
    }

    #[test]
    fn arith_matches_demo_mul_then_div() {
        let src = "const pct = SlotsUsed * 100 / SlotsTotal;\n";
        let sites = generate(
            src,
            &PathBuf::from("/abs/demo.ts"),
            &opts(&[Operator::Arith]),
        )
        .expect("g")
        .sites;
        let arith_count = sites
            .iter()
            .filter(|s| s.operator == Operator::Arith)
            .count();
        assert!(
            arith_count >= 2,
            "expected >= 2 arith sites, got {arith_count}"
        );
    }

    #[test]
    fn arith_matches_demo_simple_division() {
        let src = "function ratio(onHand: number, denom: number) { return onHand / denom; }\n";
        let sites = generate(
            src,
            &PathBuf::from("/abs/demo.ts"),
            &opts(&[Operator::Arith]),
        )
        .expect("g")
        .sites;
        assert!(sites.iter().any(|s| s.operator == Operator::Arith));
    }

    #[test]
    fn boundary_matches_demo_lte() {
        let src = "const low = onHand <= ReorderPoint;\n";
        let sites = generate(
            src,
            &PathBuf::from("/abs/demo.ts"),
            &opts(&[Operator::Boundary]),
        )
        .expect("g")
        .sites;
        assert!(sites.iter().any(|s| s.operator == Operator::Boundary));
    }

    #[test]
    fn boundary_matches_demo_lte_inside_pagination() {
        let src = "if (tc <= PAGE_SIZE) return 1;\n";
        let sites = generate(
            src,
            &PathBuf::from("/abs/demo.ts"),
            &opts(&[Operator::Boundary]),
        )
        .expect("g")
        .sites;
        assert!(sites.iter().any(|s| s.operator == Operator::Boundary));
    }

    #[test]
    fn visitor_reports_per_operator_match_counts() {
        let src = "const r = (a + b) < c;\nconst s = a <= b;\nconst t = a / b;\n";
        let r = generate(
            src,
            &PathBuf::from("/abs/a.ts"),
            &GenerateOptions {
                operators: Operator::ALL,
                max_mutants: None,
                covered_lines: None,
            },
        )
        .expect("g");
        let arith = r.per_operator.get(Operator::Arith);
        let cmp = r.per_operator.get(Operator::Cmp);
        let boundary = r.per_operator.get(Operator::Boundary);
        assert_eq!(arith.matched, 2, "arith: {:?}", arith);
        assert_eq!(cmp.matched, 2, "cmp: {:?}", cmp);
        assert_eq!(boundary.matched, 2, "boundary: {:?}", boundary);
        assert_eq!(arith.unreachable, 0);
        assert_eq!(cmp.unreachable, 0);
        assert_eq!(boundary.unreachable, 0);
    }

    #[test]
    fn visitor_counts_unreachable_per_operator() {
        let src = "const r = a + b;\nconst s = c + d;\n";
        let mut covered: HashSet<u32> = HashSet::new();
        covered.insert(1);
        let r = generate(
            src,
            &PathBuf::from("/abs/a.ts"),
            &GenerateOptions {
                operators: &[Operator::Arith],
                max_mutants: None,
                covered_lines: Some(&covered),
            },
        )
        .expect("g");
        let arith = r.per_operator.get(Operator::Arith);
        assert_eq!(arith.matched, 2);
        assert_eq!(arith.unreachable, 1);
        assert_eq!(r.sites.len(), 1);
    }

    #[test]
    fn reach_attributes_continuation_line_site_to_statement_start() {
        // The string literals sit on continuation lines (3, 4) of a `return`
        // statement that starts on line 2. With only line 2 covered, R3's
        // enclosing-statement attribution must still reach both sites.
        let src = "function f(cond) {\n  return cond\n    ? \"a\"\n    : \"b\";\n}\n";
        let mut covered: HashSet<u32> = HashSet::new();
        covered.insert(2);
        let r = generate(
            src,
            &PathBuf::from("/abs/a.ts"),
            &GenerateOptions {
                operators: &[Operator::LitStr],
                max_mutants: None,
                covered_lines: Some(&covered),
            },
        )
        .expect("g");
        assert_eq!(
            r.sites.len(),
            2,
            "both ternary arms should be reachable via the return line, got {:?}",
            r.sites
        );
        assert_eq!(r.skipped_unreachable, 0);
    }

    #[test]
    fn reach_top_level_multiline_initializer() {
        // `page: 1` sits on line 3, a continuation line of the `export const Q`
        // module item that starts on line 1. With only line 1 covered, the
        // enclosing-module-item attribution must reach the `1` site.
        let src = "export const Q = {\n  type: null,\n  page: 1,\n};\n";
        let mut covered: HashSet<u32> = HashSet::new();
        covered.insert(1);
        let r = generate(
            src,
            &PathBuf::from("/abs/a.ts"),
            &GenerateOptions {
                operators: &[Operator::LitNum],
                max_mutants: None,
                covered_lines: Some(&covered),
            },
        )
        .expect("g");
        assert_eq!(
            r.sites.len(),
            1,
            "the `1` site should be reachable via the export const line, got {:?}",
            r.sites
        );
        assert_eq!(r.skipped_unreachable, 0);
    }

    #[test]
    fn reach_unreached_statement_still_skipped() {
        // Two statements; only the first line is covered. The site in the
        // second statement is genuinely unreached and must stay skipped — the
        // unreachable bucket must not collapse.
        let src = "const a = 1 + 2;\nconst b = 3 + 4;\n";
        let mut covered: HashSet<u32> = HashSet::new();
        covered.insert(1);
        let r = generate(
            src,
            &PathBuf::from("/abs/a.ts"),
            &GenerateOptions {
                operators: &[Operator::Arith],
                max_mutants: None,
                covered_lines: Some(&covered),
            },
        )
        .expect("g");
        assert_eq!(r.sites.len(), 1, "only the covered statement is reachable");
        assert_eq!(r.sites[0].line, 1);
        assert_eq!(r.skipped_unreachable, 1);
    }

    #[test]
    fn reach_covered_site_on_own_line_unaffected() {
        // The common case: a single-line statement whose site is on its own
        // (and the statement's) line. Monotonic non-regression — still
        // produced when that line is covered.
        let src = "const a = 1 + 2;\n";
        let mut covered: HashSet<u32> = HashSet::new();
        covered.insert(1);
        let r = generate(
            src,
            &PathBuf::from("/abs/a.ts"),
            &GenerateOptions {
                operators: &[Operator::Arith],
                max_mutants: None,
                covered_lines: Some(&covered),
            },
        )
        .expect("g");
        assert_eq!(r.sites.len(), 1);
        assert_eq!(r.sites[0].line, 1);
        assert_eq!(r.skipped_unreachable, 0);
    }

    #[test]
    fn visitor_per_operator_respects_filter() {
        let src = "const r = (a + b) < c;\n";
        let r = generate(
            src,
            &PathBuf::from("/abs/a.ts"),
            &GenerateOptions {
                operators: &[Operator::Arith],
                max_mutants: None,
                covered_lines: None,
            },
        )
        .expect("g");
        assert_eq!(r.per_operator.get(Operator::Arith).matched, 1);
        assert_eq!(r.per_operator.get(Operator::Cmp).matched, 0);
    }

    #[test]
    fn list_ids_round_trips_through_parse() {
        let s = Operator::list_ids();
        let parsed: Vec<Operator> = s
            .split(", ")
            .map(|t| Operator::parse(t).expect("listed id should parse"))
            .collect();
        assert_eq!(parsed, Operator::ALL.to_vec());
    }

    #[test]
    fn static_equivalence_as_const_type_only() {
        let src = "const PART_STATUSES = [\"out\", \"critical\", \"needs-reorder\", \"in-stock\"] as const;\n\
            type PartStatus = (typeof PART_STATUSES)[number];\n\
            export function get(s: PartStatus): PartStatus { return s; }\n";
        let r = generate(src, &PathBuf::from("/abs/a.ts"), &opts(&[Operator::LitStr])).expect("g");
        assert!(r.sites.is_empty(), "expected no sites, got {:?}", r.sites);
        assert_eq!(r.skipped_equivalent_static, 4);
        let lit_str = Operator::LitStr.index();
        assert_eq!(r.per_operator.equivalent_static[lit_str], 4);
    }

    #[test]
    fn static_equivalence_as_const_runtime_read_disqualifies() {
        let src = "const TAGS = [\"a\", \"b\"] as const;\n\
            for (const t of TAGS) console.log(t);\n";
        let r = generate(src, &PathBuf::from("/abs/a.ts"), &opts(&[Operator::LitStr])).expect("g");
        let lit_str = Operator::LitStr.index();
        assert_eq!(r.sites.len(), 2, "expected 2 sites, got {:?}", r.sites);
        assert_eq!(r.per_operator.equivalent_static[lit_str], 0);
        assert_eq!(r.skipped_equivalent_static, 0);
    }

    #[test]
    fn static_equivalence_as_const_inside_function_not_eligible() {
        let src = "function f() {\n\
              const TAGS = [\"a\", \"b\"] as const;\n\
              type T = (typeof TAGS)[number];\n\
              return TAGS;\n\
            }\n";
        let r = generate(src, &PathBuf::from("/abs/a.ts"), &opts(&[Operator::LitStr])).expect("g");
        let lit_str = Operator::LitStr.index();
        assert_eq!(r.sites.len(), 2, "expected 2 sites, got {:?}", r.sites);
        assert_eq!(r.per_operator.equivalent_static[lit_str], 0);
    }

    #[test]
    fn static_equivalence_as_const_indexed_access_only_is_type_only() {
        let src = "const X = [\"a\"] as const;\ntype Y = typeof X;\ntype Z = Y[number];\n";
        let r = generate(src, &PathBuf::from("/abs/a.ts"), &opts(&[Operator::LitStr])).expect("g");
        let lit_str = Operator::LitStr.index();
        assert!(r.sites.is_empty(), "expected no sites, got {:?}", r.sites);
        assert_eq!(r.per_operator.equivalent_static[lit_str], 1);
    }

    #[test]
    fn static_equivalence_signal_init_overwritten_before_read() {
        let src = "import { signal } from \"zero\";\n\
            type S = { kind: \"loading\" | \"ok\" };\n\
            const s = signal<S>({ kind: \"loading\" });\n\
            export function load() { s.set({ kind: \"ok\" }); }\n\
            export function read() { return s.kind; }\n";
        let r = generate(src, &PathBuf::from("/abs/a.ts"), &opts(&[Operator::LitStr])).expect("g");
        let lit_str = Operator::LitStr.index();
        // "loading" is the static-equivalent initial value; "ok" is the value
        // being written inside `s.set(...)` and remains a normal mutation site.
        assert_eq!(r.per_operator.equivalent_static[lit_str], 1);
        let lit_sites: Vec<_> = r
            .sites
            .iter()
            .filter(|s| s.operator == Operator::LitStr)
            .collect();
        assert_eq!(
            lit_sites.len(),
            1,
            "expected only `\"ok\"`, got {:?}",
            lit_sites
        );
        assert_eq!(lit_sites[0].original, "\"ok\"");
    }

    #[test]
    fn static_equivalence_signal_read_precedes_set() {
        // The read (`console.log(s.kind)`) occurs in source order BEFORE any
        // .set() call → the initial `"x"` must remain a normal mutation site.
        let src = "const s = signal({ kind: \"x\" });\n\
            console.log(s.kind);\n\
            s.set({ kind: \"y\" });\n";
        let r = generate(src, &PathBuf::from("/abs/a.ts"), &opts(&[Operator::LitStr])).expect("g");
        let lit_str = Operator::LitStr.index();
        assert_eq!(r.per_operator.equivalent_static[lit_str], 0);
        let lit_sites: Vec<_> = r
            .sites
            .iter()
            .filter(|s| s.operator == Operator::LitStr)
            .collect();
        // Two sites: "x" (init) and "y" (set arg).
        assert_eq!(lit_sites.len(), 2);
        assert!(lit_sites.iter().any(|s| s.original == "\"x\""));
    }

    #[test]
    fn static_equivalence_signal_no_set_call() {
        let src = "const s = signal({ kind: \"loading\" });\n\
            export function read() { return s.kind; }\n";
        let r = generate(src, &PathBuf::from("/abs/a.ts"), &opts(&[Operator::LitStr])).expect("g");
        let lit_str = Operator::LitStr.index();
        assert_eq!(r.per_operator.equivalent_static[lit_str], 0);
        let lit_sites: Vec<_> = r
            .sites
            .iter()
            .filter(|s| s.operator == Operator::LitStr)
            .collect();
        assert_eq!(lit_sites.len(), 1, "got {:?}", lit_sites);
        assert_eq!(lit_sites[0].original, "\"loading\"");
    }

    #[test]
    fn static_equivalence_signal_inside_function_not_eligible() {
        let src = "function make() {\n\
              const s = signal({ kind: \"x\" });\n\
              s.set({ kind: \"y\" });\n\
              return s;\n\
            }\n";
        let r = generate(src, &PathBuf::from("/abs/a.ts"), &opts(&[Operator::LitStr])).expect("g");
        let lit_str = Operator::LitStr.index();
        // Not module-level → rule does not apply; "x" remains a site.
        assert_eq!(r.per_operator.equivalent_static[lit_str], 0);
        let lit_sites: Vec<_> = r
            .sites
            .iter()
            .filter(|s| s.operator == Operator::LitStr)
            .collect();
        assert!(lit_sites.iter().any(|s| s.original == "\"x\""));
    }

    #[test]
    fn static_equivalence_signal_multiple_properties() {
        let src = "const s = signal({ kind: \"loading\", count: 0 });\n\
            export function load() { s.set({ kind: \"ok\", count: 1 }); }\n\
            export function read() { return s.kind; }\n";
        let r = generate(
            src,
            &PathBuf::from("/abs/a.ts"),
            &opts(&[Operator::LitStr, Operator::LitNum]),
        )
        .expect("g");
        let lit_str = Operator::LitStr.index();
        let lit_num = Operator::LitNum.index();
        // Initial property values are static-equivalent:
        //   "loading" (str) and 0 (num).
        assert_eq!(r.per_operator.equivalent_static[lit_str], 1);
        assert_eq!(r.per_operator.equivalent_static[lit_num], 1);
        // .set(...) values stay live and are mutated sites: "ok" and 1.
        let str_sites: Vec<_> = r
            .sites
            .iter()
            .filter(|s| s.operator == Operator::LitStr)
            .collect();
        assert_eq!(str_sites.len(), 1, "got {:?}", str_sites);
        assert_eq!(str_sites[0].original, "\"ok\"");
        let num_sites: Vec<_> = r
            .sites
            .iter()
            .filter(|s| s.operator == Operator::LitNum)
            .collect();
        assert_eq!(num_sites.len(), 1, "got {:?}", num_sites);
        assert_eq!(num_sites[0].original, "1");
    }
}
