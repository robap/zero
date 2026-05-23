//! `zero mutate` subcommand: mutation testing for `src/`.
//!
//! Pipeline:
//!
//! 1. Discover test files and run them once with coverage on (baseline).
//! 2. Walk `src/` for files in the coverage scope; for each, generate
//!    mutation sites filtered by the baseline's covered lines.
//! 3. For each mutant, apply it, byte-compare against the unmutated
//!    baseline JS (equivalence skip), then re-run the test suite with a
//!    loader overlay so only that file's source is mutated.
//! 4. Tally killed / survived / errored counts and emit a summary plus
//!    `mutation/mutation.json`.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::rc::Rc;

use zero_config::Config;
use zero_test_runner::coverage::{CoverageMap, CoverageScope};
use zero_test_runner::discovery::{DiscoveryOpts, DiscoveryResult, discover};
use zero_test_runner::harness::{run_file_with_coverage, run_file_with_loader};
use zero_test_runner::loader::{CoverageContext, ZeroModuleLoader};
use zero_test_runner::mutate::{GenerateOptions, MutationSite, Operator, apply, generate};
use zero_test_runner::result::Status;
use zero_transpile::{TranspileOptions, transpile_typescript};

/// Final per-mutant verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutantStatus {
    Killed,
    Survived,
    Errored,
}

impl MutantStatus {
    fn as_str(self) -> &'static str {
        match self {
            MutantStatus::Killed => "killed",
            MutantStatus::Survived => "survived",
            MutantStatus::Errored => "errored",
        }
    }

    /// Encode a status as the exit code used by the `mutate-worker` IPC.
    pub fn to_exit_code(self) -> i32 {
        match self {
            MutantStatus::Survived => 0,
            MutantStatus::Killed => 1,
            MutantStatus::Errored => 2,
        }
    }
}

/// How per-mutant test execution is isolated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Isolation {
    /// Run each mutant in the calling process. Fast; a panic in Boa's GC
    /// kills the entire run. Used by tests and small projects.
    InProcess,
    /// Spawn a child `zero mutate-worker` per mutant. A child that aborts
    /// becomes an `Errored` mutant without taking down the parent. Used by
    /// the CLI.
    Subprocess,
}

/// Tally + per-file outcomes for one mutation run.
#[derive(Debug, Default)]
pub struct MutationSummary {
    pub generated: usize,
    pub killed: usize,
    pub survived: usize,
    pub errored: usize,
    pub skipped_unreachable: usize,
    /// Mutants whose mutated JS byte-matches the baseline JS (caught at
    /// pre-apply).
    pub skipped_equivalent_byte: usize,
    /// Mutants the visitor's static-equivalence pre-pass proved no-op by
    /// AST shape (never enter the worker queue).
    pub skipped_equivalent_static: usize,
    pub baseline_passed: bool,
    pub outcomes: BTreeMap<PathBuf, Vec<(MutationSite, MutantStatus)>>,
    /// Per-operator breakdown. Each field is indexed by
    /// `Operator::index()`. `matched + unreachable` is the visitor's
    /// view; `executed = killed + survived + errored`; `equivalent_byte`
    /// is the byte-compare skip count; `equivalent_static` is the
    /// AST-shape skip count.
    pub per_operator: PerOperatorSummary,
}

/// Per-operator aggregate of visitor matches and dispatch verdicts.
#[derive(Debug, Default, Clone, Copy)]
pub struct PerOperatorSummary {
    pub matched: [usize; 8],
    pub unreachable: [usize; 8],
    pub equivalent_byte: [usize; 8],
    pub equivalent_static: [usize; 8],
    pub killed: [usize; 8],
    pub survived: [usize; 8],
    pub errored: [usize; 8],
}

impl PerOperatorSummary {
    pub fn executed(&self, op: Operator) -> usize {
        let i = op.index();
        self.killed[i] + self.survived[i] + self.errored[i]
    }

    pub fn generated(&self, op: Operator) -> usize {
        self.executed(op)
    }
}

impl MutationSummary {
    /// Mutation score = killed / (killed + survived + errored), or 1.0 when
    /// no mutants were exercised.
    pub fn score(&self) -> f64 {
        let exec = self.killed + self.survived + self.errored;
        if exec == 0 {
            1.0
        } else {
            self.killed as f64 / exec as f64
        }
    }
}

/// Parse a comma-separated operator filter, e.g. `"arith,cmp"`.
fn parse_operators(spec: Option<&str>) -> anyhow::Result<Vec<Operator>> {
    match spec {
        None => Ok(Operator::ALL.to_vec()),
        Some(s) => {
            let mut out = Vec::new();
            for id in s.split(',').map(|p| p.trim()).filter(|p| !p.is_empty()) {
                let op = Operator::parse(id).ok_or_else(|| {
                    anyhow::anyhow!(
                        "unknown operator id: {id:?}; expected one of {}",
                        Operator::list_ids()
                    )
                })?;
                if !out.contains(&op) {
                    out.push(op);
                }
            }
            if out.is_empty() {
                Ok(Operator::ALL.to_vec())
            } else {
                Ok(out)
            }
        }
    }
}

/// Walk `root/src` collecting paths that pass `scope.covers`.
fn walk_src(scope: &CoverageScope) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk_dir(&scope.src_dir, scope, &mut out);
    out.sort();
    out
}

/// Narrow `src_files` to the user's `target`, resolved against the user's
/// `cwd`.
///
/// If `target` resolves to an existing file (relative to `cwd`), keep only
/// the entry whose canonical path matches. Otherwise treat `target` as a
/// substring filter applied to the `cwd`-relative path. If nothing
/// matches, warn on `progress` and return an empty list — the caller will
/// produce a 0-mutant run with a clear summary.
fn filter_src_files(
    src_files: Vec<PathBuf>,
    cwd: &Path,
    target: Option<&str>,
    progress: &mut dyn Write,
) -> anyhow::Result<Vec<PathBuf>> {
    let Some(t) = target else {
        return Ok(src_files);
    };
    let candidate = cwd.join(t);
    let filtered: Vec<PathBuf> = if candidate.is_file() {
        let abs = candidate.canonicalize().unwrap_or(candidate);
        src_files.into_iter().filter(|p| p == &abs).collect()
    } else {
        src_files
            .into_iter()
            .filter(|p| {
                let rel = p.strip_prefix(cwd).unwrap_or(p);
                rel.to_string_lossy().replace('\\', "/").contains(t)
            })
            .collect()
    };
    if filtered.is_empty() {
        let _ = writeln!(
            progress,
            "zero mutate: target {t:?} matched no in-scope source files under {}",
            cwd.display()
        );
    }
    Ok(filtered)
}

fn walk_dir(dir: &Path, scope: &CoverageScope, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with('.') || name_str == "node_modules" {
            continue;
        }
        if path.is_dir() {
            walk_dir(&path, scope, out);
        } else if scope.covers(&path) {
            out.push(path);
        }
    }
}

/// Build a per-file covered-line set from a `__zero_coverage__` JSON snapshot.
fn merge_covered_lines(out: &mut HashMap<PathBuf, HashSet<u32>>, snapshot: &serde_json::Value) {
    let obj = match snapshot.as_object() {
        Some(o) => o,
        None => return,
    };
    for (path_key, entry) in obj {
        let file = PathBuf::from(path_key);
        let set = out.entry(file).or_default();
        if let Some(lines) = entry.get("lines").and_then(|v| v.as_object()) {
            for (k, v) in lines {
                if let (Ok(line), Some(hits)) = (k.parse::<u32>(), v.as_u64())
                    && hits > 0
                {
                    set.insert(line);
                }
            }
        }
    }
}

/// Render the terminal summary block per spec §3.5. `report_base` is the
/// prefix stripped from each absolute path before display (usually the
/// user's `cwd`, so they see the paths they typed).
fn write_terminal_summary<W: Write>(
    w: &mut W,
    summary: &MutationSummary,
    report_base: &Path,
    quiet: bool,
    operator_filter: Option<&[Operator]>,
) -> std::io::Result<()> {
    let exec = summary.killed + summary.survived + summary.errored;
    let exec_f = exec.max(1) as f64;
    let pct = |n: usize| -> f64 { 100.0 * n as f64 / exec_f };

    let files_count = summary.outcomes.len();
    writeln!(w, "Mutation testing:")?;
    writeln!(
        w,
        "  Generated: {} mutants across {} files",
        summary.generated, files_count
    )?;
    writeln!(
        w,
        "  Killed:    {:>3}  ({:.1}%)",
        summary.killed,
        pct(summary.killed)
    )?;
    writeln!(
        w,
        "  Survived:  {:>3}  ({:.1}%)",
        summary.survived,
        pct(summary.survived)
    )?;
    writeln!(
        w,
        "  Errored:   {:>3}  ({:.1}%)",
        summary.errored,
        pct(summary.errored)
    )?;
    let total_skipped = summary.skipped_unreachable
        + summary.skipped_equivalent_byte
        + summary.skipped_equivalent_static;
    writeln!(
        w,
        "  Skipped:   {:>3}  [unreachable: {}, equivalent-byte: {}, equivalent-static: {}]",
        total_skipped,
        summary.skipped_unreachable,
        summary.skipped_equivalent_byte,
        summary.skipped_equivalent_static,
    )?;

    let ops_to_print = select_operators_for_block(summary, operator_filter);
    if !ops_to_print.is_empty() {
        writeln!(w)?;
        writeln!(w, "Per-operator breakdown:")?;
        for op in &ops_to_print {
            write_per_operator_row(w, &summary.per_operator, *op)?;
        }
    }

    if !quiet && summary.survived > 0 {
        writeln!(w)?;
        writeln!(w, "Survived mutants:")?;
        for (file, list) in &summary.outcomes {
            for (site, status) in list {
                if *status != MutantStatus::Survived {
                    continue;
                }
                let rel = file
                    .strip_prefix(report_base)
                    .unwrap_or(file)
                    .to_string_lossy()
                    .replace('\\', "/");
                writeln!(
                    w,
                    "  {}:{}:{:<3}  {:<9} `{}` → `{}`",
                    rel,
                    site.line,
                    site.column,
                    site.operator.id(),
                    site.original,
                    site.replacement
                )?;
            }
        }
    }

    writeln!(w)?;
    writeln!(w, "Mutation score: {:.1}%", 100.0 * summary.score())?;
    Ok(())
}

/// Pick the operators whose per-operator row should appear in the terminal
/// summary. With a filter set, every selected operator prints. Otherwise
/// only operators whose sites all dropped before execution
/// (`matched > 0 && executed == 0`) print — the ones most likely to confuse
/// a reader.
fn select_operators_for_block(
    summary: &MutationSummary,
    operator_filter: Option<&[Operator]>,
) -> Vec<Operator> {
    match operator_filter {
        Some(filter) => filter.to_vec(),
        None => Operator::ALL
            .iter()
            .copied()
            .filter(|op| {
                let i = op.index();
                summary.per_operator.matched[i] > 0 && summary.per_operator.executed(*op) == 0
            })
            .collect(),
    }
}

fn write_per_operator_row<W: Write>(
    w: &mut W,
    per_op: &PerOperatorSummary,
    op: Operator,
) -> std::io::Result<()> {
    let i = op.index();
    let matched = per_op.matched[i];
    if matched == 0 {
        writeln!(w, "  {}: 0 matches in src/", op.id())?;
        return Ok(());
    }
    let unreachable = per_op.unreachable[i];
    let equivalent_byte = per_op.equivalent_byte[i];
    let equivalent_static = per_op.equivalent_static[i];
    let killed = per_op.killed[i];
    let survived = per_op.survived[i];
    let errored = per_op.errored[i];
    let executed = killed + survived + errored;
    writeln!(
        w,
        "  {}: matched {}, executed {} (killed {}, survived {}, errored {}), unreachable {}, equivalent-byte {}, equivalent-static {}",
        op.id(),
        matched,
        executed,
        killed,
        survived,
        errored,
        unreachable,
        equivalent_byte,
        equivalent_static,
    )?;
    Ok(())
}

/// Write `mutation/mutation.json` under `root`. Path keys are
/// `report_base`-relative so the JSON matches the terminal output.
fn write_mutation_json(
    root: &Path,
    report_base: &Path,
    summary: &MutationSummary,
) -> std::io::Result<()> {
    let dir = root.join("mutation");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("mutation.json");
    let mut files = serde_json::Map::new();
    for (file, list) in &summary.outcomes {
        let rel = file
            .strip_prefix(report_base)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");
        let mutants: Vec<_> = list
            .iter()
            .map(|(site, status)| {
                serde_json::json!({
                    "line": site.line,
                    "column": site.column,
                    "operator": site.operator.id(),
                    "original": site.original,
                    "replacement": site.replacement,
                    "status": status.as_str(),
                })
            })
            .collect();
        files.insert(rel, serde_json::json!({ "mutants": mutants }));
    }
    let mut operators_obj = serde_json::Map::new();
    for op in Operator::ALL {
        let i = op.index();
        let executed = summary.per_operator.killed[i]
            + summary.per_operator.survived[i]
            + summary.per_operator.errored[i];
        operators_obj.insert(
            op.id().to_string(),
            serde_json::json!({
                "matched":            summary.per_operator.matched[i],
                "unreachable":        summary.per_operator.unreachable[i],
                "equivalent_byte":    summary.per_operator.equivalent_byte[i],
                "equivalent_static":  summary.per_operator.equivalent_static[i],
                "killed":             summary.per_operator.killed[i],
                "survived":           summary.per_operator.survived[i],
                "errored":            summary.per_operator.errored[i],
                "executed":           executed,
            }),
        );
    }

    let total_skipped = summary.skipped_unreachable
        + summary.skipped_equivalent_byte
        + summary.skipped_equivalent_static;
    let value = serde_json::json!({
        "schema_version": 2,
        "totals": {
            "generated": summary.generated,
            "killed": summary.killed,
            "survived": summary.survived,
            "errored": summary.errored,
            "skipped": total_skipped,
            "skipped_unreachable": summary.skipped_unreachable,
            "skipped_equivalent_byte": summary.skipped_equivalent_byte,
            "skipped_equivalent_static": summary.skipped_equivalent_static,
            "score": (summary.score() * 1000.0).round() / 1000.0,
        },
        "operators": operators_obj,
        "files": files,
    });
    let s = serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".into());
    std::fs::write(&path, s)
}

/// Core orchestration that returns a [`MutationSummary`]. Pure with respect
/// to stdout / exit code so it can be tested.
///
/// # Parameters
/// - `root`: absolute project root (used for module loading, src walking).
/// - `out_dir`: absolute build output (skipped by coverage scope).
/// - `cwd`: directory the user invoked the command from. Targets are
///   resolved against this, and report paths are stripped against this so
///   the user sees the paths they typed.
/// - `threads`: number of mutants exercised in parallel (subprocess
///   isolation only). `1` = sequential; the CLI defaults to
///   `min(available_parallelism, 8)`.
/// - `target`, `operators`, `max_mutants`, `quiet`, `isolation`,
///   `progress`: see CLI surface in spec §3.1.
#[allow(clippy::too_many_arguments)]
pub fn run_inner(
    root: &Path,
    out_dir: &Path,
    cwd: &Path,
    target: Option<&str>,
    operators: &[Operator],
    max_mutants: Option<usize>,
    quiet: bool,
    isolation: Isolation,
    threads: usize,
    progress: &mut dyn Write,
) -> anyhow::Result<MutationSummary> {
    let DiscoveryResult { files: test_files } = discover(DiscoveryOpts {
        root,
        out_dir,
        target: None,
    })?;

    let mut summary = MutationSummary::default();
    if test_files.is_empty() {
        summary.baseline_passed = true;
        return Ok(summary);
    }

    let scope = CoverageScope::new(root.to_path_buf(), out_dir.to_path_buf());
    let baseline = run_baseline(root, &scope, &test_files);
    summary.baseline_passed = baseline.passed;
    if !baseline.passed {
        return Ok(summary);
    }

    let src_files = filter_src_files(walk_src(&scope), cwd, target, progress)?;
    let mut all_sites = generate_all_sites(&src_files, &baseline.covered, operators, &mut summary);
    if let Some(max) = max_mutants
        && all_sites.len() > max
    {
        all_sites.truncate(max);
    }

    let queue = pre_apply_to_queue(&all_sites, &mut summary);
    let total = queue.len();

    let result_rx = if threads <= 1 || isolation == Isolation::InProcess {
        dispatch_sequential(
            root.to_path_buf(),
            queue,
            baseline.src_to_tests,
            test_files,
            isolation,
        )
    } else {
        dispatch_parallel(
            root.to_path_buf(),
            queue,
            baseline.src_to_tests,
            test_files,
            threads,
        )
    };

    consume_mutant_results(result_rx, total, quiet, cwd, progress, &mut summary);
    Ok(summary)
}

/// Aggregate output of [`run_baseline`].
struct BaselineRun {
    /// `true` iff every baseline test passed and no file failed to load.
    passed: bool,
    /// For each in-scope source file, the set of source line numbers the
    /// baseline run touched.
    covered: HashMap<PathBuf, HashSet<u32>>,
    /// For each in-scope source file, the test files that loaded it.
    src_to_tests: HashMap<PathBuf, Vec<PathBuf>>,
}

/// Run every `test_files` entry with coverage on; collect per-source coverage
/// and the test-impact map.
fn run_baseline(root: &Path, scope: &CoverageScope, test_files: &[PathBuf]) -> BaselineRun {
    let mut covered: HashMap<PathBuf, HashSet<u32>> = HashMap::new();
    let mut src_to_tests: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
    let mut passed = true;
    for f in test_files {
        let ctx = Rc::new(CoverageContext::new(scope.clone()));
        let outcome = run_file_with_coverage(root, f, Some(ctx.clone()));
        if outcome.result.load_error.is_some() {
            passed = false;
        }
        if outcome
            .result
            .outcomes
            .iter()
            .any(|o| matches!(o.status, Status::Failed))
        {
            passed = false;
        }
        if let Some(snap) = &outcome.coverage {
            merge_covered_lines(&mut covered, snap);
        }
        for loaded in &outcome.loaded {
            if scope.covers(loaded) {
                src_to_tests
                    .entry(loaded.clone())
                    .or_default()
                    .push(f.clone());
            }
        }
        let _: Vec<CoverageMap> = ctx.drain_maps();
    }
    for v in src_to_tests.values_mut() {
        v.sort();
        v.dedup();
    }
    BaselineRun {
        passed,
        covered,
        src_to_tests,
    }
}

/// Walk `src_files` and produce a `(src_path, site, raw_source, baseline_js)`
/// tuple per generated site. Each `src_files` entry contributes to
/// `summary.skipped_unreachable` whenever the generator routes a site away
/// from execution.
fn generate_all_sites(
    src_files: &[PathBuf],
    covered: &HashMap<PathBuf, HashSet<u32>>,
    operators: &[Operator],
    summary: &mut MutationSummary,
) -> Vec<(PathBuf, MutationSite, String, String)> {
    let empty_lines: HashSet<u32> = HashSet::new();
    let mut all_sites: Vec<(PathBuf, MutationSite, String, String)> = Vec::new();
    for src in src_files {
        let raw = match std::fs::read_to_string(src) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let logical = src.to_string_lossy().into_owned();
        let baseline_js = match transpile_typescript(
            &raw,
            &TranspileOptions {
                filename: &logical,
                inline_source_map: false,
                emit_source_map: false,
            },
        ) {
            Ok(o) => o.code,
            Err(_) => continue,
        };
        let cov_set: &HashSet<u32> = covered.get(src).unwrap_or(&empty_lines);
        let gen_opts = GenerateOptions {
            operators,
            max_mutants: None,
            covered_lines: Some(cov_set),
        };
        let result = match generate(&raw, src, &gen_opts) {
            Ok(r) => r,
            Err(_) => continue,
        };
        summary.skipped_unreachable += result.skipped_unreachable;
        summary.skipped_equivalent_static += result.skipped_equivalent_static;
        for i in 0..8 {
            summary.per_operator.matched[i] += result.per_operator.matched[i];
            summary.per_operator.unreachable[i] += result.per_operator.unreachable[i];
            summary.per_operator.equivalent_static[i] += result.per_operator.equivalent_static[i];
        }
        for s in result.sites {
            all_sites.push((src.clone(), s, raw.clone(), baseline_js.clone()));
        }
    }
    all_sites
}

/// Apply each site once; sites whose mutated JS equals the baseline are
/// counted as equivalent skips, apply errors tally into `summary.errored`,
/// and the rest are enqueued for dispatch.
fn pre_apply_to_queue(
    all_sites: &[(PathBuf, MutationSite, String, String)],
    summary: &mut MutationSummary,
) -> Vec<MutantWork> {
    let mut queue: Vec<MutantWork> = Vec::new();
    for (src_path, site, raw_src, baseline_js) in all_sites {
        match apply(raw_src, src_path, site) {
            Ok(mutated_js) => {
                if mutated_js == *baseline_js {
                    summary.skipped_equivalent_byte += 1;
                    summary.per_operator.equivalent_byte[site.operator.index()] += 1;
                } else {
                    queue.push(MutantWork {
                        src_path: src_path.clone(),
                        site: site.clone(),
                        mutated_js,
                    });
                }
            }
            Err(_) => {
                summary.generated += 1;
                summary.errored += 1;
                summary.per_operator.errored[site.operator.index()] += 1;
                summary
                    .outcomes
                    .entry(src_path.clone())
                    .or_default()
                    .push((site.clone(), MutantStatus::Errored));
            }
        }
    }
    queue
}

/// Drain mutant results off `rx`, print progress lines, and fold into
/// `summary`.
fn consume_mutant_results(
    rx: std::sync::mpsc::Receiver<MutantResult>,
    total: usize,
    quiet: bool,
    cwd: &Path,
    progress: &mut dyn Write,
    summary: &mut MutationSummary,
) {
    let mut completed = 0usize;
    while let Ok((src_path, site, status)) = rx.recv() {
        completed += 1;
        if !quiet {
            let rel = src_path
                .strip_prefix(cwd)
                .unwrap_or(&src_path)
                .to_string_lossy()
                .replace('\\', "/");
            let _ = writeln!(
                progress,
                "[{}/{}] {}: {}:{}:{} {}",
                completed,
                total,
                status.as_str(),
                rel,
                site.line,
                site.column,
                site.operator.id()
            );
        }
        summary.generated += 1;
        let op_idx = site.operator.index();
        match status {
            MutantStatus::Killed => {
                summary.killed += 1;
                summary.per_operator.killed[op_idx] += 1;
            }
            MutantStatus::Survived => {
                summary.survived += 1;
                summary.per_operator.survived[op_idx] += 1;
            }
            MutantStatus::Errored => {
                summary.errored += 1;
                summary.per_operator.errored[op_idx] += 1;
            }
        }
        summary
            .outcomes
            .entry(src_path)
            .or_default()
            .push((site, status));
    }
}

/// One unit of mutant test work — produced by the pre-apply pass, consumed
/// by [`dispatch_sequential`] / [`dispatch_parallel`].
struct MutantWork {
    src_path: PathBuf,
    site: MutationSite,
    mutated_js: String,
}

/// Result type streamed from a worker (or the sequential path) back to
/// the main thread.
type MutantResult = (PathBuf, MutationSite, MutantStatus);

/// Sequential dispatch: process the queue in order on the calling thread,
/// pushing each result through an `mpsc` channel for a uniform consumer.
fn dispatch_sequential(
    root: PathBuf,
    queue: Vec<MutantWork>,
    src_to_tests: HashMap<PathBuf, Vec<PathBuf>>,
    test_files: Vec<PathBuf>,
    isolation: Isolation,
) -> std::sync::mpsc::Receiver<MutantResult> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for item in queue {
            let relevant = src_to_tests
                .get(&item.src_path)
                .map(|v| v.as_slice())
                .unwrap_or(test_files.as_slice());
            let status = match isolation {
                Isolation::InProcess => {
                    run_one_mutant_inproc(&root, &item.src_path, &item.mutated_js, relevant)
                }
                Isolation::Subprocess => {
                    run_one_mutant_subprocess(&root, &item.src_path, &item.mutated_js, relevant)
                }
            };
            if tx.send((item.src_path, item.site, status)).is_err() {
                break;
            }
        }
    });
    rx
}

/// Parallel dispatch: spawn `threads` worker threads sharing an atomic
/// index into the work queue. Each worker pulls the next site, runs it in
/// a subprocess, and sends the result back. Always uses subprocess
/// isolation — the parallel path is the only one that benefits from
/// concurrency and parallel in-process Boa runs would multiply the GC
/// hazard we're isolating against.
fn dispatch_parallel(
    root: PathBuf,
    queue: Vec<MutantWork>,
    src_to_tests: HashMap<PathBuf, Vec<PathBuf>>,
    test_files: Vec<PathBuf>,
    threads: usize,
) -> std::sync::mpsc::Receiver<MutantResult> {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let (tx, rx) = std::sync::mpsc::channel();
    let work = Arc::new(queue);
    let next = Arc::new(AtomicUsize::new(0));
    let src_to_tests = Arc::new(src_to_tests);
    let test_files = Arc::new(test_files);
    let root = Arc::new(root);
    for _ in 0..threads {
        let work = work.clone();
        let next = next.clone();
        let src_to_tests = src_to_tests.clone();
        let test_files = test_files.clone();
        let root = root.clone();
        let tx = tx.clone();
        std::thread::spawn(move || {
            loop {
                let idx = next.fetch_add(1, Ordering::SeqCst);
                if idx >= work.len() {
                    break;
                }
                let item = &work[idx];
                let relevant = src_to_tests
                    .get(&item.src_path)
                    .map(|v| v.as_slice())
                    .unwrap_or(test_files.as_slice());
                let status =
                    run_one_mutant_subprocess(&root, &item.src_path, &item.mutated_js, relevant);
                if tx
                    .send((item.src_path.clone(), item.site.clone(), status))
                    .is_err()
                {
                    break;
                }
            }
        });
    }
    drop(tx);
    rx
}

/// Run a single mutant's full test loop in the calling process.
fn run_one_mutant_inproc(
    root: &Path,
    src_path: &Path,
    mutated_js: &str,
    test_files: &[PathBuf],
) -> MutantStatus {
    let mut overlay: HashMap<PathBuf, String> = HashMap::new();
    overlay.insert(src_path.to_path_buf(), mutated_js.to_string());
    for tf in test_files {
        let loader = Rc::new(ZeroModuleLoader::new(root).with_overlay(overlay.clone()));
        let result = run_file_with_loader(root, tf, loader);
        if result.load_error.is_some() {
            return MutantStatus::Errored;
        }
        if result
            .outcomes
            .iter()
            .any(|o| matches!(o.status, Status::Failed))
        {
            return MutantStatus::Killed;
        }
    }
    MutantStatus::Survived
}

/// Run a single mutant's full test loop in a child `zero mutate-worker`
/// process. Child exit codes encode the verdict; anything outside `0..=2`
/// (e.g. a Boa-internal abort during Context teardown) is reported as
/// [`MutantStatus::Errored`].
fn run_one_mutant_subprocess(
    root: &Path,
    src_path: &Path,
    mutated_js: &str,
    test_files: &[PathBuf],
) -> MutantStatus {
    let tmp_dir = std::env::temp_dir();
    let uniq = format!("zero-mutate-{}-{}", std::process::id(), next_tmp_counter());
    let mutated_path = tmp_dir.join(format!("{uniq}.js"));
    let tests_path = tmp_dir.join(format!("{uniq}.tests"));

    if std::fs::write(&mutated_path, mutated_js).is_err() {
        return MutantStatus::Errored;
    }
    let mut tests_body = String::new();
    for tf in test_files {
        tests_body.push_str(&tf.to_string_lossy());
        tests_body.push('\n');
    }
    if std::fs::write(&tests_path, tests_body).is_err() {
        let _ = std::fs::remove_file(&mutated_path);
        return MutantStatus::Errored;
    }

    let result = (|| -> MutantStatus {
        let exe = match std::env::current_exe() {
            Ok(p) => p,
            Err(_) => return MutantStatus::Errored,
        };
        let output = Command::new(exe)
            .arg("mutate-worker")
            .arg("--root")
            .arg(root)
            .arg("--mutated-src")
            .arg(src_path)
            .arg("--mutated-js-file")
            .arg(&mutated_path)
            .arg("--tests-file")
            .arg(&tests_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output();
        let status = match output {
            Ok(o) => o.status,
            Err(_) => return MutantStatus::Errored,
        };
        match status.code() {
            Some(0) => MutantStatus::Survived,
            Some(1) => MutantStatus::Killed,
            _ => MutantStatus::Errored,
        }
    })();

    let _ = std::fs::remove_file(&mutated_path);
    let _ = std::fs::remove_file(&tests_path);
    result
}

fn next_tmp_counter() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    N.fetch_add(1, Ordering::Relaxed)
}

/// Entry point invoked by `zero mutate-worker` (a hidden subcommand) per
/// mutant. Reads the overlay + test list from files, runs the suite, and
/// returns a [`MutantStatus`] that the parent maps to an exit code.
///
/// # Parameters
/// - `root`: project root (absolute).
/// - `mutated_src`: canonical source-file path being mutated (overlay key).
/// - `mutated_js_file`: file containing the mutated JS body.
/// - `tests_file`: file with newline-separated absolute test-file paths.
pub fn worker_main(
    root: &Path,
    mutated_src: &Path,
    mutated_js_file: &Path,
    tests_file: &Path,
) -> MutantStatus {
    let mutated_js = match std::fs::read_to_string(mutated_js_file) {
        Ok(s) => s,
        Err(_) => return MutantStatus::Errored,
    };
    let tests_blob = match std::fs::read_to_string(tests_file) {
        Ok(s) => s,
        Err(_) => return MutantStatus::Errored,
    };
    let tests: Vec<PathBuf> = tests_blob
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(PathBuf::from)
        .collect();
    run_one_mutant_inproc(root, mutated_src, &mutated_js, &tests)
}

/// CLI entry point for `zero mutate`.
pub async fn run(
    target: Option<String>,
    operators: Option<String>,
    max_mutants: Option<usize>,
    quiet: bool,
    threads: usize,
) -> anyhow::Result<()> {
    let config = Config::load_from_cwd()?;
    let cwd = std::env::current_dir()?;
    let root = cwd.join(&config.project.root);
    let out = cwd.join(&config.build.out);
    let filter_was_set = operators.is_some();
    let ops = parse_operators(operators.as_deref())?;

    let mut stdout = std::io::stdout().lock();
    let summary = run_inner(
        &root,
        &out,
        &cwd,
        target.as_deref(),
        &ops,
        max_mutants,
        quiet,
        Isolation::Subprocess,
        threads,
        &mut stdout,
    )?;

    if !summary.baseline_passed {
        let _ = writeln!(
            stdout,
            "zero mutate: baseline test run failed; refusing to mutate"
        );
        std::process::exit(1);
    }

    write_terminal_summary(
        &mut stdout,
        &summary,
        &cwd,
        quiet,
        if filter_was_set { Some(&ops) } else { None },
    )?;
    write_mutation_json(&root, &cwd, &summary)?;

    if summary.survived > 0 || summary.errored > 0 {
        std::process::exit(1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_zero_toml(root: &Path) {
        fs::write(
            root.join("zero.toml"),
            r#"[project]
root = "."
[build]
out = "dist"
"#,
        )
        .unwrap();
    }

    /// Scaffold a tempdir with a `src/foo.ts`, a `foo.test.ts`, and the
    /// matching `zero.toml`.
    fn make_project(src: &str, test_src: &str) -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_zero_toml(root);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/foo.ts"), src).unwrap();
        fs::write(root.join("foo.test.ts"), test_src).unwrap();
        dir
    }

    #[test]
    fn parses_operator_filter_csv() {
        let parsed = parse_operators(Some("arith,cmp")).unwrap();
        assert_eq!(parsed, vec![Operator::Arith, Operator::Cmp]);
    }

    #[test]
    fn parse_operators_rejects_unknown() {
        let err = parse_operators(Some("bogus")).unwrap_err();
        assert!(format!("{err}").contains("bogus"));
    }

    #[test]
    fn mutation_json_includes_per_operator_and_schema_version() {
        let dir = make_project(
            "export function add(a: number, b: number) { return a + b }\n",
            r#"import { describe, it, expect } from "zero/test";
import { add } from "./src/foo.ts";
describe("g", () => {
  it("adds", () => expect(add(1, 2)).toBe(3));
});
"#,
        );
        let root = dir.path();
        let out = root.join("dist");
        let mut sink: Vec<u8> = Vec::new();
        let summary = run_inner(
            root,
            &out,
            root,
            None,
            &[Operator::Arith],
            None,
            true,
            Isolation::InProcess,
            1,
            &mut sink,
        )
        .expect("ok");
        write_mutation_json(root, root, &summary).expect("write json");

        let s = fs::read_to_string(root.join("mutation/mutation.json")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();

        assert_eq!(v["schema_version"], 2);
        assert!(v["operators"].is_object());
        let ops = v["operators"].as_object().unwrap();
        assert_eq!(ops.len(), 8, "expected all 8 operators in json");
        for id in [
            "arith", "cmp", "bool", "cond_neg", "boundary", "lit_bool", "lit_num", "lit_str",
        ] {
            assert!(ops.contains_key(id), "missing operator {id}");
        }
        let arith = &ops["arith"];
        assert!(arith["matched"].as_u64().unwrap() >= 1);
        assert_eq!(
            arith["killed"].as_u64().unwrap(),
            arith["executed"].as_u64().unwrap()
        );
    }

    #[test]
    fn schema_version_is_2() {
        let dir = tempfile::tempdir().unwrap();
        let summary = MutationSummary::default();
        write_mutation_json(dir.path(), dir.path(), &summary).expect("write json");
        let s = fs::read_to_string(dir.path().join("mutation/mutation.json")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["schema_version"], 2);
        let totals = v["totals"].as_object().expect("totals object");
        assert_eq!(totals["skipped_equivalent_byte"].as_u64().unwrap(), 0);
        assert_eq!(totals["skipped_equivalent_static"].as_u64().unwrap(), 0);
        assert_eq!(totals["skipped_unreachable"].as_u64().unwrap(), 0);
        let ops = v["operators"].as_object().expect("operators object");
        let arith = &ops["arith"];
        assert_eq!(arith["equivalent_byte"].as_u64().unwrap(), 0);
        assert_eq!(arith["equivalent_static"].as_u64().unwrap(), 0);
        assert!(arith.get("equivalent").is_none());
    }

    #[test]
    fn terminal_summary_filtered_run_prints_per_operator_block() {
        let dir = make_project(
            "export function unused(a: number, b: number) {\n    return a + b;\n}\nexport const ok = 1;\n",
            r#"import { describe, it, expect } from "zero/test";
import { ok } from "./src/foo.ts";
describe("g", () => { it("ok", () => expect(ok).toBe(1)); });
"#,
        );
        let root = dir.path();
        let out = root.join("dist");
        let mut sink: Vec<u8> = Vec::new();
        let summary = run_inner(
            root,
            &out,
            root,
            None,
            &[Operator::Arith],
            None,
            true,
            Isolation::InProcess,
            1,
            &mut sink,
        )
        .expect("ok");

        let mut buf: Vec<u8> = Vec::new();
        write_terminal_summary(&mut buf, &summary, root, true, Some(&[Operator::Arith]))
            .expect("write");
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("Per-operator breakdown:"), "got:\n{s}");
        assert!(s.contains("arith:"), "got:\n{s}");
        assert!(s.contains("unreachable"), "got:\n{s}");
    }

    #[test]
    fn terminal_summary_filtered_run_zero_matches_says_so() {
        let dir = make_project(
            "export const ok = true;\n",
            r#"import { describe, it, expect } from "zero/test";
import { ok } from "./src/foo.ts";
describe("g", () => { it("ok", () => expect(ok).toBe(true)); });
"#,
        );
        let root = dir.path();
        let out = root.join("dist");
        let mut sink: Vec<u8> = Vec::new();
        let summary = run_inner(
            root,
            &out,
            root,
            None,
            &[Operator::Arith],
            None,
            true,
            Isolation::InProcess,
            1,
            &mut sink,
        )
        .expect("ok");
        let mut buf: Vec<u8> = Vec::new();
        write_terminal_summary(&mut buf, &summary, root, true, Some(&[Operator::Arith]))
            .expect("write");
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("arith: 0 matches in src/"), "got:\n{s}");
    }

    #[test]
    fn terminal_summary_default_run_quiet_on_clean_operators() {
        let dir = make_project(
            "export function add(a: number, b: number) { return a + b }\n",
            r#"import { describe, it, expect } from "zero/test";
import { add } from "./src/foo.ts";
describe("g", () => {
  it("adds 1+2", () => expect(add(1, 2)).toBe(3));
  it("adds 5+7", () => expect(add(5, 7)).toBe(12));
});
"#,
        );
        let root = dir.path();
        let out = root.join("dist");
        let mut sink: Vec<u8> = Vec::new();
        let summary = run_inner(
            root,
            &out,
            root,
            None,
            Operator::ALL,
            None,
            true,
            Isolation::InProcess,
            1,
            &mut sink,
        )
        .expect("ok");
        let mut buf: Vec<u8> = Vec::new();
        write_terminal_summary(&mut buf, &summary, root, true, None).expect("write");
        let s = String::from_utf8(buf).unwrap();
        assert!(
            !s.contains("Per-operator breakdown:"),
            "default run should be quiet on clean operators; got:\n{s}"
        );
    }

    #[test]
    fn per_operator_summary_filtered_run_counts_matches() {
        let dir = make_project(
            "export function add(a: number, b: number) { return a + b }\n",
            r#"import { describe, it, expect } from "zero/test";
import { add } from "./src/foo.ts";
describe("g", () => {
  it("adds 1+2", () => expect(add(1, 2)).toBe(3));
  it("adds 5+7", () => expect(add(5, 7)).toBe(12));
});
"#,
        );
        let root = dir.path();
        let out = root.join("dist");
        let mut sink: Vec<u8> = Vec::new();
        let summary = run_inner(
            root,
            &out,
            root,
            None,
            &[Operator::Arith],
            None,
            true,
            Isolation::InProcess,
            1,
            &mut sink,
        )
        .expect("ok");
        let arith = Operator::Arith.index();
        assert!(summary.per_operator.matched[arith] >= 1);
        assert_eq!(summary.per_operator.unreachable[arith], 0);
        assert_eq!(summary.per_operator.equivalent_byte[arith], 0);
        assert_eq!(summary.per_operator.equivalent_static[arith], 0);
        assert!(summary.per_operator.killed[arith] >= 1);
        assert_eq!(summary.per_operator.survived[arith], 0);
        let cmp = Operator::Cmp.index();
        assert_eq!(summary.per_operator.matched[cmp], 0);
    }

    #[test]
    fn per_operator_summary_unreachable_when_uncovered() {
        let dir = make_project(
            "export function unused(a: number, b: number) {\n    return a + b;\n}\nexport const ok = 1;\n",
            r#"import { describe, it, expect } from "zero/test";
import { ok } from "./src/foo.ts";
describe("g", () => { it("ok", () => expect(ok).toBe(1)); });
"#,
        );
        let root = dir.path();
        let out = root.join("dist");
        let mut sink: Vec<u8> = Vec::new();
        let summary = run_inner(
            root,
            &out,
            root,
            None,
            &[Operator::Arith],
            None,
            true,
            Isolation::InProcess,
            1,
            &mut sink,
        )
        .expect("ok");
        let arith = Operator::Arith.index();
        assert!(
            summary.per_operator.matched[arith] >= 1,
            "expected an arith match"
        );
        assert_eq!(
            summary.per_operator.matched[arith], summary.per_operator.unreachable[arith],
            "all arith matches should be unreachable"
        );
        assert_eq!(summary.per_operator.executed(Operator::Arith), 0);
    }

    #[test]
    fn parse_operators_error_lists_accepted_ids() {
        let err = parse_operators(Some("help")).unwrap_err();
        let msg = format!("{err}");
        for id in [
            "arith", "cmp", "bool", "cond_neg", "boundary", "lit_bool", "lit_num", "lit_str",
        ] {
            assert!(msg.contains(id), "missing id {id} in: {msg}");
        }
        for variant in [
            "Arith", "Cmp", "Bool", "CondNeg", "Boundary", "LitBool", "LitNum", "LitStr",
        ] {
            assert!(
                !msg.contains(variant),
                "leaked Debug name {variant} in: {msg}"
            );
        }
    }

    #[test]
    fn baseline_failure_aborts_run() {
        // A test that fails => baseline_passed should be false and no
        // mutants are generated.
        let dir = make_project(
            "export function add(a: number, b: number) { return a + b }\n",
            r#"import { describe, it, expect } from "zero/test";
import { add } from "./src/foo.ts";
describe("g", () => { it("oops", () => expect(add(1,1)).toBe(99)); });
"#,
        );
        let root = dir.path();
        let out = root.join("dist");
        let mut sink: Vec<u8> = Vec::new();
        let summary = run_inner(
            root,
            &out,
            root,
            None,
            Operator::ALL,
            None,
            true,
            Isolation::InProcess,
            1,
            &mut sink,
        )
        .expect("ok");
        assert!(!summary.baseline_passed, "expected baseline_passed=false");
        assert_eq!(summary.generated, 0);
    }

    #[test]
    fn killed_mutant_summary_correct() {
        // Strong test kills the arith mutant on `a + b`.
        let dir = make_project(
            "export function add(a: number, b: number) { return a + b }\n",
            r#"import { describe, it, expect } from "zero/test";
import { add } from "./src/foo.ts";
describe("g", () => {
  it("adds 1+2", () => expect(add(1, 2)).toBe(3));
  it("adds 5+7", () => expect(add(5, 7)).toBe(12));
});
"#,
        );
        let root = dir.path();
        let out = root.join("dist");
        let mut sink: Vec<u8> = Vec::new();
        let summary = run_inner(
            root,
            &out,
            root,
            None,
            &[Operator::Arith],
            None,
            true,
            Isolation::InProcess,
            1,
            &mut sink,
        )
        .expect("ok");
        assert!(summary.baseline_passed);
        assert!(
            summary.generated >= 1,
            "should generate at least one arith mutant"
        );
        assert_eq!(summary.survived, 0, "all arith mutants should be killed");
        assert!(summary.killed >= 1);
    }

    #[test]
    fn survived_mutant_reported() {
        // Weak test never checks `add` result. The arith mutant survives.
        let dir = make_project(
            "export function add(a: number, b: number) { return a + b }\n",
            r#"import { describe, it, expect } from "zero/test";
import { add } from "./src/foo.ts";
describe("g", () => { it("calls", () => { add(1,2); expect(1).toBe(1); }); });
"#,
        );
        let root = dir.path();
        let out = root.join("dist");
        let mut sink: Vec<u8> = Vec::new();
        let summary = run_inner(
            root,
            &out,
            root,
            None,
            &[Operator::Arith],
            None,
            true,
            Isolation::InProcess,
            1,
            &mut sink,
        )
        .expect("ok");
        assert!(summary.baseline_passed);
        assert!(
            summary.survived >= 1,
            "expected a surviving mutant: {summary:?}"
        );
    }

    #[test]
    fn respects_operator_filter() {
        // Source has both an arith and a cmp site. With operators=[Bool]
        // (which doesn't match anything in the source), no mutants are
        // generated.
        let dir = make_project(
            "export function f(a: number, b: number) { return a + b }\nexport function g(a: number, b: number) { return a < b }\n",
            r#"import { describe, it, expect } from "zero/test";
import { f, g } from "./src/foo.ts";
describe("g", () => {
  it("f", () => expect(f(1,2)).toBe(3));
  it("g", () => expect(g(1,2)).toBe(true));
});
"#,
        );
        let root = dir.path();
        let out = root.join("dist");
        let mut sink: Vec<u8> = Vec::new();
        let summary = run_inner(
            root,
            &out,
            root,
            None,
            &[Operator::Bool],
            None,
            true,
            Isolation::InProcess,
            1,
            &mut sink,
        )
        .expect("ok");
        assert!(summary.baseline_passed);
        assert_eq!(summary.generated, 0);
    }

    #[test]
    fn apply_skips_static_equivalent_literals_when_indexing() {
        // Regression guard: collect-mode skips the `"x"` signal-init literal
        // as static-equivalent, so apply-mode must skip it too — otherwise
        // the Nth-site index drifts and the "ok" mutant lands on "x".
        let src = r#"import { signal } from "zero";
const s = signal({ k: "x" });
export function setOk() { s.set({ k: "ok" }); }
export function getK() { return s.val.k; }
"#;
        let path = std::path::Path::new("/abs/foo.ts");
        let r = zero_test_runner::mutate::generate(
            src,
            path,
            &zero_test_runner::mutate::GenerateOptions {
                operators: &[Operator::LitStr],
                max_mutants: None,
                covered_lines: None,
            },
        )
        .expect("gen");
        assert_eq!(r.sites.len(), 1);
        let out = zero_test_runner::mutate::apply(src, path, &r.sites[0]).expect("apply");
        assert!(out.contains("k: \"x\""), "x should be untouched: {out}");
        assert!(
            out.contains("k: \"\""),
            "ok should be mutated to empty: {out}"
        );
    }

    #[test]
    fn mutate_reclassifies_static_equivalents_end_to_end() {
        // Mirrors the demo's `src/stores/parts.ts`: an `as const` array used
        // only for type derivation, plus a module-level `signal({...})` whose
        // initial value is overwritten before any read.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_zero_toml(root);
        fs::create_dir_all(root.join("src/stores")).unwrap();
        fs::write(
            root.join("src/stores/parts.ts"),
            r#"import { signal } from "zero";
export const PART_STATUSES = ["out", "critical", "needs-reorder", "in-stock"] as const;
export type PartStatus = (typeof PART_STATUSES)[number];
type PartsState =
  | { kind: "loading" }
  | { kind: "ok"; items: number[] }
  | { kind: "error" };
export const partsSignal = signal<PartsState>({ kind: "loading" });
export function load() {
  partsSignal.set({ kind: "ok", items: [] });
}
export function error() {
  partsSignal.set({ kind: "error" });
}
"#,
        )
        .unwrap();
        fs::write(
            root.join("parts.test.ts"),
            r#"import { describe, it, expect } from "zero/test";
import { load, partsSignal } from "./src/stores/parts.ts";
describe("parts", () => {
  it("loads ok", () => {
    load();
    expect(partsSignal.val.kind).toBe("ok");
  });
});
"#,
        )
        .unwrap();

        let out = root.join("dist");
        let mut sink: Vec<u8> = Vec::new();
        let summary = run_inner(
            root,
            &out,
            root,
            None,
            &[Operator::LitStr],
            None,
            true,
            Isolation::InProcess,
            1,
            &mut sink,
        )
        .expect("ok");

        assert!(summary.baseline_passed, "baseline failed: {summary:?}");
        assert_eq!(summary.survived, 0, "survived should be 0: {summary:?}");
        assert_eq!(
            summary.skipped_equivalent_static, 5,
            "expected 5 static-equivalents (4 array members + 1 signal init): {summary:?}"
        );
        let lit_str = Operator::LitStr.index();
        assert_eq!(summary.per_operator.equivalent_static[lit_str], 5);
        assert!(
            (summary.score() - 1.0).abs() < f64::EPSILON,
            "expected score 1.0, got {}",
            summary.score()
        );

        write_mutation_json(root, root, &summary).expect("write json");
        let s = fs::read_to_string(root.join("mutation/mutation.json")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["schema_version"], 2);
        assert_eq!(
            v["totals"]["skipped_equivalent_static"].as_u64().unwrap(),
            5
        );
        assert_eq!(
            v["operators"]["lit_str"]["equivalent_static"]
                .as_u64()
                .unwrap(),
            5
        );
    }
}
