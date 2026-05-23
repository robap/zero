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

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use swc_core::common::sync::Lrc;
use swc_core::common::{FileName, GLOBALS, Globals, Mark, SourceMap as SwcSourceMap, Spanned};
use swc_core::ecma::ast::{
    BinExpr, BinaryOp, CondExpr, DoWhileStmt, EsVersion, Expr, ForStmt, IfStmt, Lit, Module,
    UnaryExpr, UnaryOp, WhileStmt,
};
use swc_core::ecma::codegen::Emitter;
use swc_core::ecma::codegen::text_writer::JsWriter;
use swc_core::ecma::parser::{Parser, StringInput, Syntax, TsSyntax, lexer::Lexer};
use swc_core::ecma::transforms::base::{fixer::fixer, hygiene::hygiene, resolver};
use swc_core::ecma::transforms::typescript::strip;
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

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
}

impl PerOperatorTally {
    /// Lookup helper.
    pub fn get(&self, op: Operator) -> OperatorCounts {
        OperatorCounts {
            matched: self.matched[op.index()],
            unreachable: self.unreachable[op.index()],
        }
    }
}

/// View of a single operator's collect-mode counts.
#[derive(Debug, Clone, Copy)]
pub struct OperatorCounts {
    pub matched: usize,
    pub unreachable: usize,
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

    let (sites, skipped, matched, unreachable_per_op) = GLOBALS.set(&Globals::new(), || {
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
        );
        v.visit_mut_module(&mut module);
        (
            v.sites,
            v.skipped_unreachable,
            v.matched,
            v.unreachable_per_op,
        )
    });

    let limited = match opts.max_mutants {
        Some(max) if sites.len() > max => sites.into_iter().take(max).collect(),
        _ => sites,
    };
    Ok(GenerateResult {
        sites: limited,
        skipped_unreachable: skipped,
        per_operator: PerOperatorTally {
            matched,
            unreachable: unreachable_per_op,
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
        let mut v =
            MutateVisitor::new_apply(cm.clone(), file.to_path_buf(), site.operator, target_index);
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
    sites: Vec<MutationSite>,
    counts: [usize; 8],
    skipped_unreachable: usize,
    matched: [usize; 8],
    unreachable_per_op: [usize; 8],
}

impl<'a> MutateVisitor<'a> {
    fn new_collect(
        cm: Lrc<SwcSourceMap>,
        file: PathBuf,
        operators_filter: &'a [Operator],
        covered_lines: Option<&'a HashSet<u32>>,
    ) -> Self {
        Self {
            cm,
            file,
            mode: Mode::Collect,
            operators_filter: Some(operators_filter),
            covered_lines,
            sites: Vec::new(),
            counts: [0; 8],
            skipped_unreachable: 0,
            matched: [0; 8],
            unreachable_per_op: [0; 8],
        }
    }

    fn new_apply(
        cm: Lrc<SwcSourceMap>,
        file: PathBuf,
        operator: Operator,
        target_index: usize,
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
            sites: Vec::new(),
            counts: [0; 8],
            skipped_unreachable: 0,
            matched: [0; 8],
            unreachable_per_op: [0; 8],
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
                if let Some(cov) = self.covered_lines
                    && !cov.contains(&line)
                {
                    self.skipped_unreachable += 1;
                    self.unreachable_per_op[idx] += 1;
                    return false;
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
                let current = self.counts[idx];
                self.counts[idx] += 1;
                current == target_index
            }
        }
    }
}

impl<'a> VisitMut for MutateVisitor<'a> {
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
}
