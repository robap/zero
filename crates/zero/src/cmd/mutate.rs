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

use super::mutate_cache::{self, CacheEntry, CacheMode, CachedSite, MutateCache};

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

    /// Inverse of [`MutantStatus::as_str`]; used to replay cached verdicts.
    fn parse(s: &str) -> Option<MutantStatus> {
        match s {
            "killed" => Some(MutantStatus::Killed),
            "survived" => Some(MutantStatus::Survived),
            "errored" => Some(MutantStatus::Errored),
            _ => None,
        }
    }
}

/// How per-mutant test execution is isolated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Isolation {
    /// Run each mutant in the calling process. Fast; a panic in the JS engine
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
    /// Per-source-file operator tallies (same arrays as `per_operator`,
    /// restricted to one file). Feeds cache entries.
    pub per_file_operator: BTreeMap<PathBuf, PerOperatorSummary>,
    /// Verdicts replayed from `mutation/cache.json` instead of executed.
    pub reused_mutants: usize,
    /// Source files whose entire verdict set was replayed from the cache.
    pub reused_files: usize,
    /// `true` when the all-unchanged fast path skipped the baseline.
    pub baseline_skipped: bool,
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
    if summary.reused_mutants > 0 {
        writeln!(
            w,
            "  Generated: {} mutants across {} files ({} reused from cache across {} files)",
            summary.generated, files_count, summary.reused_mutants, summary.reused_files
        )?;
    } else {
        writeln!(
            w,
            "  Generated: {} mutants across {} files",
            summary.generated, files_count
        )?;
    }
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
    cache_mode: CacheMode,
    progress: &mut dyn Write,
) -> anyhow::Result<MutationSummary> {
    let DiscoveryResult { files: test_files } = discover(DiscoveryOpts {
        root,
        out_dir,
        extra_skip_dirs: &[],
        target: None,
        cwd: root,
    })?;

    let mut summary = MutationSummary::default();
    if test_files.is_empty() {
        summary.baseline_passed = true;
        return Ok(summary);
    }

    let scope = CoverageScope::new(root.to_path_buf(), out_dir.to_path_buf());
    // Loader paths are canonical; rel_key must agree with them.
    let canon_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let all_src = walk_src(&scope);
    let mut memo: HashMap<PathBuf, Option<String>> = HashMap::new();
    let old_cache = match cache_mode {
        CacheMode::ReadWrite => mutate_cache::load(&canon_root, env!("CARGO_PKG_VERSION")),
        _ => None,
    };
    if target.is_none()
        && let Some(replay) = try_fast_path(
            &canon_root,
            old_cache.as_ref(),
            &test_files,
            &all_src,
            &mut memo,
            progress,
        )
    {
        return Ok(replay);
    }

    let baseline = run_baseline(root, &scope, &test_files);
    summary.baseline_passed = baseline.passed;
    if !baseline.passed {
        return Ok(summary);
    }

    let src_files = filter_src_files(all_src.clone(), cwd, target, progress)?;
    let to_run = apply_cache_reuse(
        &canon_root,
        old_cache.as_ref(),
        &src_files,
        &baseline,
        &test_files,
        &mut summary,
        &mut memo,
    );
    let queue = generate_queue(
        &to_run,
        &baseline.covered,
        operators,
        max_mutants,
        &mut summary,
    );
    let total = queue.len();

    let result_rx = dispatch_mutants(root, queue, &baseline, &test_files, isolation, threads);
    consume_mutant_results(result_rx, total, quiet, cwd, progress, &mut summary);

    // Persist verdicts for the next run. The red-baseline return above keeps
    // a failing suite from ever touching the cache (R9).
    if cache_mode != CacheMode::Bypass {
        persist_cache(
            &canon_root,
            (&all_src, &src_files, &test_files),
            &baseline,
            &summary,
            old_cache.as_ref(),
            &mut memo,
        );
    }
    Ok(summary)
}

/// Build and write `mutation/cache.json`. Failures degrade silently: a
/// failed write just means a full run next time.
fn persist_cache(
    canon_root: &Path,
    (all_src, src_files, test_files): (&[PathBuf], &[PathBuf], &[PathBuf]),
    baseline: &BaselineRun,
    summary: &MutationSummary,
    old_cache: Option<&MutateCache>,
    memo: &mut HashMap<PathBuf, Option<String>>,
) {
    let cache = build_cache(
        canon_root, all_src, src_files, test_files, baseline, summary, old_cache, memo,
    );
    let _ = mutate_cache::save(canon_root, env!("CARGO_PKG_VERSION"), &cache);
}

/// Generate sites for the files that need the pipeline, apply the global
/// `--max-mutants` cap, and pre-apply into the dispatch queue.
fn generate_queue(
    to_run: &[PathBuf],
    covered: &HashMap<PathBuf, HashSet<u32>>,
    operators: &[Operator],
    max_mutants: Option<usize>,
    summary: &mut MutationSummary,
) -> Vec<MutantWork> {
    let mut all_sites = generate_all_sites(to_run, covered, operators, summary);
    if let Some(max) = max_mutants
        && all_sites.len() > max
    {
        all_sites.truncate(max);
    }
    pre_apply_to_queue(&all_sites, summary)
}

/// Route the queue to sequential or parallel dispatch.
fn dispatch_mutants(
    root: &Path,
    queue: Vec<MutantWork>,
    baseline: &BaselineRun,
    test_files: &[PathBuf],
    isolation: Isolation,
    threads: usize,
) -> std::sync::mpsc::Receiver<MutantResult> {
    if threads <= 1 || isolation == Isolation::InProcess {
        dispatch_sequential(
            root.to_path_buf(),
            queue,
            baseline.src_to_tests.clone(),
            test_files.to_vec(),
            isolation,
        )
    } else {
        dispatch_parallel(
            root.to_path_buf(),
            queue,
            baseline.src_to_tests.clone(),
            test_files.to_vec(),
            threads,
        )
    }
}

/// All-unchanged fast path (R3): when the cached universe matches the
/// discovered one exactly, replay every entry without running the baseline.
/// Returns the replayed summary on a hit; `None` falls through to the
/// normal pipeline. Never fires for `[pattern]` runs (caller gates) or a
/// partial cache.
fn try_fast_path(
    canon_root: &Path,
    old_cache: Option<&MutateCache>,
    test_files: &[PathBuf],
    all_src: &[PathBuf],
    memo: &mut HashMap<PathBuf, Option<String>>,
    progress: &mut dyn Write,
) -> Option<MutationSummary> {
    let cache = old_cache?;
    if !fast_path_applies(canon_root, cache, test_files, all_src, memo) {
        return None;
    }
    let mut replay = MutationSummary::default();
    for (key, entry) in &cache.entries {
        // An unparseable site declines the whole fast path: soundness
        // over hit rate.
        if !fold_cached_entry(&mut replay, &canon_root.join(key), entry) {
            return None;
        }
    }
    // Printed unconditionally (quiet included): the user must never wonder
    // whether tests actually ran.
    let _ = writeln!(
        progress,
        "zero mutate: no changes since last run — replaying cached result (baseline skipped)"
    );
    replay.baseline_passed = true;
    replay.baseline_skipped = true;
    Some(replay)
}

/// True iff the cached universe matches the discovered one exactly: same
/// rel test set, same rel src set, every recorded file rehashes to its
/// cached value, and every src file has an entry.
fn fast_path_applies(
    canon_root: &Path,
    cache: &MutateCache,
    test_files: &[PathBuf],
    all_src: &[PathBuf],
    memo: &mut HashMap<PathBuf, Option<String>>,
) -> bool {
    let rel_sorted = |paths: &[PathBuf]| -> Vec<String> {
        let mut keys: Vec<String> = paths
            .iter()
            .map(|p| mutate_cache::rel_key(canon_root, p))
            .collect();
        keys.sort();
        keys
    };
    let src_keys = rel_sorted(all_src);
    if rel_sorted(test_files) != cache.test_files || src_keys != cache.src_files {
        return false;
    }
    // Partial caches (e.g. after a `[pattern]` run dropped an entry) never
    // fast-path.
    if !src_keys.iter().all(|k| cache.entries.contains_key(k)) {
        return false;
    }
    // A file that fails to hash (e.g. deleted helper) declines too.
    cache.files.iter().all(|(k, want)| {
        mutate_cache::hash_file(&canon_root.join(k), memo).as_deref() == Some(want)
    })
}

/// Fold every reusable entry of the previous run's cache into `summary`.
/// Returns the files that still need the full pipeline.
fn apply_cache_reuse(
    canon_root: &Path,
    old_cache: Option<&MutateCache>,
    src_files: &[PathBuf],
    baseline: &BaselineRun,
    test_files: &[PathBuf],
    summary: &mut MutationSummary,
    memo: &mut HashMap<PathBuf, Option<String>>,
) -> Vec<PathBuf> {
    let Some(cache) = old_cache else {
        return src_files.to_vec();
    };
    let (reusable, mut to_run) = partition_reusable(
        src_files.to_vec(),
        cache,
        canon_root,
        baseline,
        test_files,
        memo,
    );
    for (src, entry) in reusable {
        if !fold_cached_entry(summary, &src, &entry) {
            // Unparseable site: soundness over hit rate — re-run the file.
            to_run.push(src);
        }
    }
    to_run.sort();
    to_run
}

/// Split the selected files into (reused, to_run). A file is reused iff the
/// cache has an entry whose fingerprint matches the one computed from THIS
/// run's baseline closures.
fn partition_reusable(
    src_files: Vec<PathBuf>,
    cache: &MutateCache,
    canon_root: &Path,
    baseline: &BaselineRun,
    all_tests: &[PathBuf],
    memo: &mut HashMap<PathBuf, Option<String>>,
) -> (Vec<(PathBuf, CacheEntry)>, Vec<PathBuf>) {
    let mut reused = Vec::new();
    let mut to_run = Vec::new();
    for src in src_files {
        let closure = mutate_cache::closure_for(
            &src,
            &baseline.src_to_tests,
            &baseline.test_loaded,
            all_tests,
        );
        let fresh = mutate_cache::fingerprint(canon_root, &closure, memo);
        let key = mutate_cache::rel_key(canon_root, &src);
        match (fresh, cache.entries.get(&key)) {
            (Some(fp), Some(entry)) if entry.fingerprint == fp => {
                reused.push((src, entry.clone()));
            }
            _ => to_run.push(src),
        }
    }
    (reused, to_run)
}

/// Replay a cached entry into `summary` so totals, per-operator rows, exit
/// semantics, and `mutation.json` equal what a full run would produce.
/// Returns `false` (and leaves `summary` untouched) when any site fails to
/// parse — the caller re-runs the file instead.
fn fold_cached_entry(summary: &mut MutationSummary, src: &Path, entry: &CacheEntry) -> bool {
    // Parse everything up front so a bad site can't leave a half-folded
    // summary behind.
    let mut parsed: Vec<(MutationSite, MutantStatus)> = Vec::with_capacity(entry.sites.len());
    for s in &entry.sites {
        let (Some(operator), Some(status)) =
            (Operator::parse(&s.operator), MutantStatus::parse(&s.status))
        else {
            return false;
        };
        parsed.push((
            MutationSite {
                file: src.to_path_buf(),
                operator,
                line: s.line,
                column: s.column,
                original: s.original.clone(),
                replacement: s.replacement.clone(),
            },
            status,
        ));
    }
    let per_op = &entry.per_operator;
    summary.skipped_unreachable += per_op.unreachable.iter().sum::<usize>();
    summary.skipped_equivalent_byte += per_op.equivalent_byte.iter().sum::<usize>();
    summary.skipped_equivalent_static += per_op.equivalent_static.iter().sum::<usize>();
    let global = &mut summary.per_operator;
    for i in 0..8 {
        global.matched[i] += per_op.matched[i];
        global.unreachable[i] += per_op.unreachable[i];
        global.equivalent_byte[i] += per_op.equivalent_byte[i];
        global.equivalent_static[i] += per_op.equivalent_static[i];
        global.killed[i] += per_op.killed[i];
        global.survived[i] += per_op.survived[i];
        global.errored[i] += per_op.errored[i];
    }
    summary.per_file_operator.insert(src.to_path_buf(), *per_op);
    summary.reused_mutants += parsed.len();
    summary.reused_files += 1;
    for (site, status) in parsed {
        summary.generated += 1;
        match status {
            MutantStatus::Killed => summary.killed += 1,
            MutantStatus::Survived => summary.survived += 1,
            MutantStatus::Errored => summary.errored += 1,
        }
        summary
            .outcomes
            .entry(src.to_path_buf())
            .or_default()
            .push((site, status));
    }
    true
}

/// Build the cache to persist after a run: fresh entries for the files
/// selected this run, fingerprint-revalidated old entries for the rest.
/// Files whose closure cannot be fully hashed get no entry at all.
#[allow(clippy::too_many_arguments)]
fn build_cache(
    canon_root: &Path,
    all_src: &[PathBuf],
    selected: &[PathBuf],
    test_files: &[PathBuf],
    baseline: &BaselineRun,
    summary: &MutationSummary,
    old: Option<&MutateCache>,
    memo: &mut HashMap<PathBuf, Option<String>>,
) -> MutateCache {
    let selected_set: HashSet<&PathBuf> = selected.iter().collect();
    let mut cache = MutateCache::default();
    let mut universe: std::collections::BTreeSet<PathBuf> = std::collections::BTreeSet::new();
    for f in all_src {
        let closure =
            mutate_cache::closure_for(f, &baseline.src_to_tests, &baseline.test_loaded, test_files);
        let Some(fp) = mutate_cache::fingerprint(canon_root, &closure, memo) else {
            continue;
        };
        universe.extend(closure);
        let key = mutate_cache::rel_key(canon_root, f);
        if selected_set.contains(f) {
            cache.entries.insert(key, fresh_entry(fp, f, summary));
        } else if let Some(prev) = old.and_then(|c| c.entries.get(&key))
            && prev.fingerprint == fp
        {
            cache.entries.insert(key, prev.clone());
        }
    }
    universe.extend(test_files.iter().cloned());
    universe.extend(all_src.iter().cloned());
    for p in &universe {
        if let Some(h) = mutate_cache::hash_file(p, memo) {
            cache.files.insert(mutate_cache::rel_key(canon_root, p), h);
        }
    }
    cache.test_files = test_files
        .iter()
        .map(|p| mutate_cache::rel_key(canon_root, p))
        .collect();
    cache.test_files.sort();
    cache.src_files = all_src
        .iter()
        .map(|p| mutate_cache::rel_key(canon_root, p))
        .collect();
    cache.src_files.sort();
    cache
}

/// Cache entry for a file whose pipeline ran this invocation.
fn fresh_entry(fingerprint: String, src: &Path, summary: &MutationSummary) -> CacheEntry {
    let sites = summary
        .outcomes
        .get(src)
        .map(|list| {
            list.iter()
                .map(|(site, status)| CachedSite {
                    line: site.line,
                    column: site.column,
                    operator: site.operator.id().to_string(),
                    original: site.original.clone(),
                    replacement: site.replacement.clone(),
                    status: status.as_str().to_string(),
                })
                .collect()
        })
        .unwrap_or_default();
    CacheEntry {
        fingerprint,
        sites,
        per_operator: summary
            .per_file_operator
            .get(src)
            .copied()
            .unwrap_or_default(),
    }
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
    /// For each test file, every existing file it loaded (the test file
    /// itself included; directories and pseudo-modules excluded).
    test_loaded: HashMap<PathBuf, Vec<PathBuf>>,
}

/// Run every `test_files` entry with coverage on; collect per-source coverage
/// and the test-impact map.
fn run_baseline(root: &Path, scope: &CoverageScope, test_files: &[PathBuf]) -> BaselineRun {
    let mut covered: HashMap<PathBuf, HashSet<u32>> = HashMap::new();
    let mut src_to_tests: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
    let mut test_loaded: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
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
        // Behavioral closure for this test: every existing file the loader
        // resolved, plus the test file itself. The harness also registers
        // the test file's parent *directory* as a resolution base — the
        // `is_file()` filter drops it. Deliberately NOT filtered by
        // `scope.covers`: out-of-scope helpers are part of the closure.
        let mut loaded_files: Vec<PathBuf> = outcome
            .loaded
            .iter()
            .filter(|p| p.is_file())
            .cloned()
            .collect();
        loaded_files.push(f.clone());
        loaded_files.sort();
        loaded_files.dedup();
        test_loaded.insert(f.clone(), loaded_files);
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
        test_loaded,
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
        // Every walked file gets a per-file entry — even with zero matches —
        // so it can replay as a zero-contribution cache member.
        summary.per_file_operator.entry(src.clone()).or_default();
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
        let per_file = summary.per_file_operator.entry(src.clone()).or_default();
        for i in 0..8 {
            per_file.matched[i] += result.per_operator.matched[i];
            per_file.unreachable[i] += result.per_operator.unreachable[i];
            per_file.equivalent_static[i] += result.per_operator.equivalent_static[i];
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
                    summary
                        .per_file_operator
                        .entry(src_path.clone())
                        .or_default()
                        .equivalent_byte[site.operator.index()] += 1;
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
                    .per_file_operator
                    .entry(src_path.clone())
                    .or_default()
                    .errored[site.operator.index()] += 1;
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
        let per_file = summary
            .per_file_operator
            .entry(src_path.clone())
            .or_default();
        match status {
            MutantStatus::Killed => per_file.killed[op_idx] += 1,
            MutantStatus::Survived => per_file.survived[op_idx] += 1,
            MutantStatus::Errored => per_file.errored[op_idx] += 1,
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
/// concurrency and parallel in-process engine runs would multiply the
/// crash hazard we're isolating against.
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
/// (e.g. an engine-internal abort during context teardown) is reported as
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

/// Map the CLI flags onto a [`CacheMode`]. Narrowed runs (`--operators`,
/// `--max-mutants`) always bypass; `--no-cache` forces a fresh run that
/// rewrites the cache; the default reads and writes.
fn cache_mode_for(filter_set: bool, max_set: bool, no_cache: bool) -> CacheMode {
    if filter_set || max_set {
        CacheMode::Bypass
    } else if no_cache {
        CacheMode::Fresh
    } else {
        CacheMode::ReadWrite
    }
}

/// CLI entry point for `zero mutate`.
pub async fn run(
    target: Option<String>,
    operators: Option<String>,
    max_mutants: Option<usize>,
    quiet: bool,
    threads: usize,
    no_cache: bool,
) -> anyhow::Result<()> {
    let config = Config::load_from_cwd()?;
    let cwd = std::env::current_dir()?;
    let root = cwd.join(&config.project.root);
    let out = cwd.join(&config.build.out);
    let filter_was_set = operators.is_some();
    let ops = parse_operators(operators.as_deref())?;
    let cache_mode = cache_mode_for(filter_was_set, max_mutants.is_some(), no_cache);

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
        cache_mode,
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
    fn baseline_retains_per_test_loaded_sets() {
        // a.test.ts -> src/a.ts -> src/helper.ts: the test's loaded set must
        // contain all three files (the test file itself included) and no
        // directories.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_zero_toml(root);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("src/helper.ts"),
            "export function inc(n: number) { return n + 1 }\n",
        )
        .unwrap();
        fs::write(
            root.join("src/a.ts"),
            r#"import { inc } from "./helper.ts";
export function two() { return inc(1); }
"#,
        )
        .unwrap();
        fs::write(
            root.join("a.test.ts"),
            r#"import { describe, it, expect } from "zero/test";
import { two } from "./src/a.ts";
describe("a", () => { it("two", () => expect(two()).toBe(2)); });
"#,
        )
        .unwrap();

        let out = root.join("dist");
        let scope = CoverageScope::new(root.to_path_buf(), out.clone());
        let test_files = discover(DiscoveryOpts {
            root,
            out_dir: &out,
            extra_skip_dirs: &[],
            target: None,
            cwd: root,
        })
        .expect("discover")
        .files;
        let baseline = run_baseline(root, &scope, &test_files);
        assert!(baseline.passed, "baseline should pass");

        let (test_path, loaded) = baseline
            .test_loaded
            .iter()
            .find(|(p, _)| p.to_string_lossy().ends_with("a.test.ts"))
            .expect("a.test.ts should have a loaded set");
        let has = |suffix: &str| {
            loaded
                .iter()
                .any(|p| p.to_string_lossy().replace('\\', "/").ends_with(suffix))
        };
        assert!(has("a.test.ts"), "missing test file itself: {loaded:?}");
        assert!(has("src/a.ts"), "missing src/a.ts: {loaded:?}");
        assert!(has("src/helper.ts"), "missing src/helper.ts: {loaded:?}");
        assert!(
            loaded.iter().all(|p| p.is_file()),
            "loaded set must contain only files: {loaded:?}"
        );
        assert!(test_path.is_file());
    }

    #[test]
    fn per_file_operator_sums_to_global_and_covers_unreached_files() {
        // Two files: `foo.ts` is tested (kills + a survivor candidate),
        // `lonely.ts` is never imported so all its sites are unreachable.
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
        fs::write(
            root.join("src/lonely.ts"),
            "export function mul(a: number, b: number) { return a * b }\n",
        )
        .unwrap();
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
            CacheMode::Bypass,
            &mut sink,
        )
        .expect("ok");
        assert!(summary.baseline_passed);

        // (a) Per-file tallies sum to the global per-operator arrays.
        let mut folded = PerOperatorSummary::default();
        for per_file in summary.per_file_operator.values() {
            for i in 0..8 {
                folded.matched[i] += per_file.matched[i];
                folded.unreachable[i] += per_file.unreachable[i];
                folded.equivalent_byte[i] += per_file.equivalent_byte[i];
                folded.equivalent_static[i] += per_file.equivalent_static[i];
                folded.killed[i] += per_file.killed[i];
                folded.survived[i] += per_file.survived[i];
                folded.errored[i] += per_file.errored[i];
            }
        }
        for i in 0..8 {
            assert_eq!(
                folded.matched[i], summary.per_operator.matched[i],
                "matched[{i}]"
            );
            assert_eq!(
                folded.unreachable[i], summary.per_operator.unreachable[i],
                "unreachable[{i}]"
            );
            assert_eq!(
                folded.equivalent_byte[i], summary.per_operator.equivalent_byte[i],
                "equivalent_byte[{i}]"
            );
            assert_eq!(
                folded.equivalent_static[i], summary.per_operator.equivalent_static[i],
                "equivalent_static[{i}]"
            );
            assert_eq!(
                folded.killed[i], summary.per_operator.killed[i],
                "killed[{i}]"
            );
            assert_eq!(
                folded.survived[i], summary.per_operator.survived[i],
                "survived[{i}]"
            );
            assert_eq!(
                folded.errored[i], summary.per_operator.errored[i],
                "errored[{i}]"
            );
        }

        // (b) The unreached file still has an entry, with matched > 0.
        let lonely = summary
            .per_file_operator
            .iter()
            .find(|(p, _)| p.to_string_lossy().ends_with("lonely.ts"))
            .map(|(_, v)| v)
            .expect("lonely.ts must have a per-file entry");
        let total_matched: usize = lonely.matched.iter().sum();
        assert!(
            total_matched > 0,
            "lonely.ts should match sites: {lonely:?}"
        );
    }

    /// Scaffold a green two-file project: `src/foo.ts` (strongly tested,
    /// imports `src/helper.ts`) plus `foo.test.ts`. Used by the cache tests.
    fn make_cache_project() -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_zero_toml(root);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("src/helper.ts"),
            "export function inc(n: number) { return n + 1 }\n",
        )
        .unwrap();
        fs::write(
            root.join("src/foo.ts"),
            r#"import { inc } from "./helper.ts";
export function add(a: number, b: number) { return a + b }
export function addOne(n: number) { return inc(n); }
"#,
        )
        .unwrap();
        fs::write(
            root.join("foo.test.ts"),
            r#"import { describe, it, expect } from "zero/test";
import { add, addOne } from "./src/foo.ts";
describe("g", () => {
  it("adds 1+2", () => expect(add(1, 2)).toBe(3));
  it("adds 5+7", () => expect(add(5, 7)).toBe(12));
  it("addOne", () => expect(addOne(1)).toBe(2));
});
"#,
        )
        .unwrap();
        dir
    }

    /// `run_inner` with the standard test-fixture arguments.
    fn run_for_cache(root: &Path, cache_mode: CacheMode) -> MutationSummary {
        let out = root.join("dist");
        let mut sink: Vec<u8> = Vec::new();
        run_inner(
            root,
            &out,
            root,
            None,
            Operator::ALL,
            None,
            true,
            Isolation::InProcess,
            1,
            cache_mode,
            &mut sink,
        )
        .expect("run_inner ok")
    }

    #[test]
    fn readwrite_run_writes_cache_json() {
        let dir = make_cache_project();
        let root = dir.path();
        let summary = run_for_cache(root, CacheMode::ReadWrite);
        assert!(summary.baseline_passed);

        let s = fs::read_to_string(root.join("mutation/cache.json")).expect("cache written");
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["schema_version"], 1);

        let entries = v["entries"].as_object().expect("entries object");
        assert!(
            entries.contains_key("src/foo.ts"),
            "keys: {:?}",
            entries.keys()
        );
        assert!(
            entries.contains_key("src/helper.ts"),
            "keys: {:?}",
            entries.keys()
        );
        let foo_sites = entries["src/foo.ts"]["sites"].as_array().unwrap();
        assert!(!foo_sites.is_empty(), "tested file should record sites");

        let files = v["files"].as_object().expect("files object");
        assert!(
            files.contains_key("foo.test.ts"),
            "keys: {:?}",
            files.keys()
        );
        assert!(
            files.contains_key("src/helper.ts"),
            "keys: {:?}",
            files.keys()
        );
        for (k, h) in files {
            let h = h.as_str().unwrap();
            assert_eq!(h.len(), 64, "hash length for {k}");
            assert!(
                h.chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
                "hash charset for {k}: {h}"
            );
        }
    }

    #[test]
    fn bypass_run_never_touches_cache_file() {
        let dir = make_cache_project();
        let root = dir.path();

        // No cache exists and a Bypass run must not create one.
        run_for_cache(root, CacheMode::Bypass);
        assert!(
            !root.join("mutation/cache.json").exists(),
            "Bypass must not create a cache"
        );

        // Pre-seed a cache, then run Bypass again: bytes unchanged.
        run_for_cache(root, CacheMode::ReadWrite);
        let before = fs::read(root.join("mutation/cache.json")).unwrap();
        run_for_cache(root, CacheMode::Bypass);
        let after = fs::read(root.join("mutation/cache.json")).unwrap();
        assert_eq!(before, after, "Bypass must leave the cache bytes alone");
    }

    #[test]
    fn red_baseline_never_touches_cache_file() {
        let dir = make_cache_project();
        let root = dir.path();

        // Seed a valid cache from a green run.
        run_for_cache(root, CacheMode::ReadWrite);
        let before = fs::read(root.join("mutation/cache.json")).unwrap();

        // Break the suite, then run ReadWrite: refuses to mutate, cache
        // bytes unchanged (R9).
        fs::write(
            root.join("foo.test.ts"),
            r#"import { describe, it, expect } from "zero/test";
import { add } from "./src/foo.ts";
describe("g", () => { it("oops", () => expect(add(1, 1)).toBe(99)); });
"#,
        )
        .unwrap();
        let summary = run_for_cache(root, CacheMode::ReadWrite);
        assert!(!summary.baseline_passed);
        let after = fs::read(root.join("mutation/cache.json")).unwrap();
        assert_eq!(before, after, "red baseline must not rewrite the cache");

        // Same with no cache at all: still none created.
        fs::remove_file(root.join("mutation/cache.json")).unwrap();
        let summary = run_for_cache(root, CacheMode::ReadWrite);
        assert!(!summary.baseline_passed);
        assert!(!root.join("mutation/cache.json").exists());
    }

    /// Scaffold two *independent* tested files: `src/a.ts` + `a.test.ts`,
    /// `src/b.ts` + `b.test.ts`.
    fn make_two_file_project() -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_zero_toml(root);
        fs::create_dir_all(root.join("src")).unwrap();
        for (src, test, func) in [
            ("src/a.ts", "a.test.ts", "alpha"),
            ("src/b.ts", "b.test.ts", "beta"),
        ] {
            fs::write(
                root.join(src),
                format!("export function {func}(a: number, b: number) {{ return a + b }}\n"),
            )
            .unwrap();
            fs::write(
                root.join(test),
                format!(
                    r#"import {{ describe, it, expect }} from "zero/test";
import {{ {func} }} from "./{src}";
describe("{func}", () => {{
  it("adds 1+2", () => expect({func}(1, 2)).toBe(3));
  it("adds 5+7", () => expect({func}(5, 7)).toBe(12));
}});
"#
                ),
            )
            .unwrap();
        }
        dir
    }

    /// `run_inner` with a `[pattern]` target and the standard fixture args.
    fn run_for_cache_target(root: &Path, target: &str, cache_mode: CacheMode) -> MutationSummary {
        let out = root.join("dist");
        let mut sink: Vec<u8> = Vec::new();
        run_inner(
            root,
            &out,
            root,
            Some(target),
            Operator::ALL,
            None,
            true,
            Isolation::InProcess,
            1,
            cache_mode,
            &mut sink,
        )
        .expect("run_inner ok")
    }

    #[test]
    fn pattern_run_drops_stale_nonselected_entry_and_refreshes_selected() {
        let dir = make_two_file_project();
        let root = dir.path();

        // Full run seeds entries for both files.
        run_for_cache(root, CacheMode::ReadWrite);
        let v: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("mutation/cache.json")).unwrap())
                .unwrap();
        assert!(v["entries"].as_object().unwrap().contains_key("src/b.ts"));

        // Edit the *non-selected* file, then run targeting only a.ts.
        fs::write(
            root.join("src/b.ts"),
            "export function beta(a: number, b: number) { return a + b + 0 }\n",
        )
        .unwrap();
        run_for_cache_target(root, "src/a.ts", CacheMode::ReadWrite);

        let v: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("mutation/cache.json")).unwrap())
                .unwrap();
        let entries = v["entries"].as_object().unwrap();
        assert!(
            entries.contains_key("src/a.ts"),
            "selected file keeps a fresh entry: {:?}",
            entries.keys()
        );
        assert!(
            !entries.contains_key("src/b.ts"),
            "stale non-selected entry must be dropped: {:?}",
            entries.keys()
        );
        assert!(
            !entries["src/a.ts"]["sites"].as_array().unwrap().is_empty(),
            "a.ts entry should be fresh with sites"
        );
    }

    #[test]
    fn pattern_run_retains_unchanged_nonselected_entry() {
        let dir = make_two_file_project();
        let root = dir.path();
        run_for_cache(root, CacheMode::ReadWrite);
        // Nothing edited: a pattern run on a.ts must keep b.ts's entry.
        run_for_cache_target(root, "src/a.ts", CacheMode::ReadWrite);
        let v: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("mutation/cache.json")).unwrap())
                .unwrap();
        let entries = v["entries"].as_object().unwrap();
        assert!(
            entries.contains_key("src/b.ts"),
            "unchanged non-selected entry must be retained: {:?}",
            entries.keys()
        );
    }

    #[test]
    fn source_edit_reuses_untouched_file_and_matches_fresh_run() {
        let dir = make_two_file_project();
        let root = dir.path();
        let run1 = run_for_cache(root, CacheMode::ReadWrite);
        assert!(run1.generated > 0);
        assert_eq!(run1.reused_mutants, 0, "cold run reuses nothing");

        // Edit A (stays green); B's closure is untouched.
        fs::write(
            root.join("src/a.ts"),
            "export function alpha(a: number, b: number) { return b + a }\n",
        )
        .unwrap();

        let run2 = run_for_cache(root, CacheMode::ReadWrite);
        assert_eq!(run2.reused_files, 1, "only b.ts is reused: {run2:?}");
        assert!(run2.reused_mutants > 0, "{run2:?}");

        // R2 invariant: totals equal a from-scratch run of the same tree.
        let fresh = run_for_cache(root, CacheMode::Fresh);
        assert_eq!(fresh.reused_mutants, 0);
        assert_eq!(run2.generated, fresh.generated);
        assert_eq!(run2.killed, fresh.killed);
        assert_eq!(run2.survived, fresh.survived);
        assert_eq!(run2.errored, fresh.errored);
        assert!((run2.score() - fresh.score()).abs() < f64::EPSILON);
    }

    #[test]
    fn test_edit_invalidates_and_kills_previous_survivor() {
        let dir = make_two_file_project();
        let root = dir.path();
        // Weaken a's test: calls alpha but asserts nothing about it.
        fs::write(
            root.join("a.test.ts"),
            r#"import { describe, it, expect } from "zero/test";
import { alpha } from "./src/a.ts";
describe("alpha", () => { it("calls", () => { alpha(1, 2); expect(1).toBe(1); }); });
"#,
        )
        .unwrap();
        let run1 = run_for_cache(root, CacheMode::ReadWrite);
        assert!(run1.survived >= 1, "weak test leaves a survivor: {run1:?}");

        // Strengthen the assertion; only a's closure changes.
        fs::write(
            root.join("a.test.ts"),
            r#"import { describe, it, expect } from "zero/test";
import { alpha } from "./src/a.ts";
describe("alpha", () => {
  it("adds 1+2", () => expect(alpha(1, 2)).toBe(3));
  it("adds 5+7", () => expect(alpha(5, 7)).toBe(12));
});
"#,
        )
        .unwrap();
        let run2 = run_for_cache(root, CacheMode::ReadWrite);
        assert_eq!(run2.reused_files, 1, "b.ts stays reused: {run2:?}");
        assert_eq!(
            run2.survived, 0,
            "stronger test kills the survivor: {run2:?}"
        );
        assert!(run2.killed > run1.killed - run1.survived, "{run2:?}");
    }

    #[test]
    fn dependency_edit_invalidates_importer_but_not_unrelated() {
        let dir = make_two_file_project();
        let root = dir.path();
        // a.ts gains a dependency on src/c.ts, exercised through a's test.
        fs::write(
            root.join("src/c.ts"),
            "export function base(): number { return 1 }\n",
        )
        .unwrap();
        fs::write(
            root.join("src/a.ts"),
            r#"import { base } from "./c.ts";
export function alpha(a: number, b: number) { return a + b + base() - base() }
"#,
        )
        .unwrap();
        let run1 = run_for_cache(root, CacheMode::ReadWrite);
        assert!(run1.baseline_passed, "{run1:?}");

        // Edit only the dependency (still returns 1 net effect on alpha).
        fs::write(
            root.join("src/c.ts"),
            "export function base(): number { return 2 - 1 }\n",
        )
        .unwrap();
        let run2 = run_for_cache(root, CacheMode::ReadWrite);
        // a (imports c) and c itself both re-execute; only b is reused.
        assert_eq!(run2.reused_files, 1, "only b.ts reused: {run2:?}");
        let reused_keys: Vec<_> = run2
            .outcomes
            .keys()
            .filter(|p| p.to_string_lossy().ends_with("b.ts"))
            .collect();
        assert_eq!(reused_keys.len(), 1, "b.ts present in outcomes");
    }

    #[test]
    fn fresh_mode_ignores_valid_cache_and_rewrites_it() {
        let dir = make_two_file_project();
        let root = dir.path();
        let run1 = run_for_cache(root, CacheMode::ReadWrite);
        let mtime_before = fs::metadata(root.join("mutation/cache.json"))
            .unwrap()
            .modified()
            .unwrap();

        let run2 = run_for_cache(root, CacheMode::Fresh);
        assert_eq!(run2.reused_mutants, 0, "Fresh must not reuse: {run2:?}");
        assert_eq!(run2.reused_files, 0);
        assert_eq!(run2.generated, run1.generated, "everything re-executes");
        assert_eq!(run2.killed, run1.killed);

        let mtime_after = fs::metadata(root.join("mutation/cache.json"))
            .unwrap()
            .modified()
            .unwrap();
        assert!(mtime_after > mtime_before, "cache must be rewritten");
    }

    #[test]
    fn corrupt_or_version_skewed_cache_degrades_to_full_run() {
        let dir = make_two_file_project();
        let root = dir.path();
        let run1 = run_for_cache(root, CacheMode::ReadWrite);

        // Garbage JSON: full run, then a valid cache is rewritten.
        fs::write(root.join("mutation/cache.json"), "{garbage!").unwrap();
        let run2 = run_for_cache(root, CacheMode::ReadWrite);
        assert_eq!(run2.reused_mutants, 0, "corrupt cache reuses nothing");
        assert_eq!(run2.generated, run1.generated);
        let v: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("mutation/cache.json")).unwrap())
                .expect("rewritten cache parses");
        assert_eq!(v["schema_version"], 1);

        // Version skew: a structurally valid cache written by another CLI
        // version is treated as absent.
        let valid = mutate_cache::load(root, env!("CARGO_PKG_VERSION")).expect("valid cache");
        mutate_cache::save(root, "0.0.0-other", &valid).expect("doctored save");
        let run3 = run_for_cache(root, CacheMode::ReadWrite);
        assert_eq!(run3.reused_mutants, 0, "version skew reuses nothing");
        assert_eq!(run3.generated, run1.generated);
        assert!(
            mutate_cache::load(root, env!("CARGO_PKG_VERSION")).is_some(),
            "cache rewritten under the current version"
        );
    }

    #[test]
    fn summary_line_reports_reuse_in_quiet_mode() {
        let dir = make_two_file_project();
        let root = dir.path();
        run_for_cache(root, CacheMode::ReadWrite);
        // Edit a.ts so run 2 reuses exactly b.ts.
        fs::write(
            root.join("src/a.ts"),
            "export function alpha(a: number, b: number) { return b + a }\n",
        )
        .unwrap();
        let run2 = run_for_cache(root, CacheMode::ReadWrite);
        assert_eq!(run2.reused_files, 1);

        let mut buf: Vec<u8> = Vec::new();
        write_terminal_summary(&mut buf, &run2, root, true, None).expect("write");
        let s = String::from_utf8(buf).unwrap();
        let expect = format!(
            "Generated: {} mutants across {} files ({} reused from cache across {} files)",
            run2.generated,
            run2.outcomes.len(),
            run2.reused_mutants,
            run2.reused_files
        );
        assert!(s.contains(&expect), "missing reuse parenthetical in:\n{s}");
    }

    /// Like [`run_for_cache`] but returns the progress sink's contents too.
    fn run_for_cache_with_sink(root: &Path, cache_mode: CacheMode) -> (MutationSummary, String) {
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
            cache_mode,
            &mut sink,
        )
        .expect("run_inner ok");
        (summary, String::from_utf8(sink).unwrap())
    }

    #[test]
    fn unchanged_rerun_hits_fast_path_with_equal_totals() {
        let dir = make_two_file_project();
        let root = dir.path();
        let run1 = run_for_cache(root, CacheMode::ReadWrite);
        write_mutation_json(root, root, &run1).expect("json 1");
        let json1: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("mutation/mutation.json")).unwrap())
                .unwrap();

        let (run2, sink) = run_for_cache_with_sink(root, CacheMode::ReadWrite);
        assert!(
            run2.baseline_skipped,
            "fast path must skip the baseline: {run2:?}"
        );
        assert!(run2.baseline_passed);
        assert_eq!(run2.reused_mutants, run1.generated);
        assert_eq!(run2.generated, run1.generated);
        assert_eq!(run2.killed, run1.killed);
        assert_eq!(run2.survived, run1.survived);
        assert_eq!(run2.errored, run1.errored);
        assert_eq!(run2.skipped_unreachable, run1.skipped_unreachable);
        assert_eq!(run2.skipped_equivalent_byte, run1.skipped_equivalent_byte);
        assert_eq!(
            run2.skipped_equivalent_static,
            run1.skipped_equivalent_static
        );
        assert!((run2.score() - run1.score()).abs() < f64::EPSILON);
        assert!(
            sink.contains("no changes since last run — replaying cached result (baseline skipped)"),
            "missing marker line in:\n{sink}"
        );

        // The refreshed report equals the previous run's.
        write_mutation_json(root, root, &run2).expect("json 2");
        let json2: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("mutation/mutation.json")).unwrap())
                .unwrap();
        assert_eq!(json1["totals"], json2["totals"]);
    }

    #[test]
    fn any_edit_declines_fast_path() {
        let dir = make_two_file_project();
        let root = dir.path();
        run_for_cache(root, CacheMode::ReadWrite);
        fs::write(
            root.join("src/a.ts"),
            "export function alpha(a: number, b: number) { return b + a }\n",
        )
        .unwrap();
        let run2 = run_for_cache(root, CacheMode::ReadWrite);
        assert!(!run2.baseline_skipped, "edit must decline the fast path");
        assert_eq!(
            run2.reused_files, 1,
            "partial reuse still applies: {run2:?}"
        );
    }

    #[test]
    fn new_test_file_declines_fast_path_and_reexecutes_target() {
        let dir = make_two_file_project();
        let root = dir.path();
        run_for_cache(root, CacheMode::ReadWrite);

        // Closure-membership change: a brand-new test exercising A. Every
        // previously-known file is byte-identical, but the test set differs.
        fs::write(
            root.join("a2.test.ts"),
            r#"import { describe, it, expect } from "zero/test";
import { alpha } from "./src/a.ts";
describe("alpha again", () => { it("adds", () => expect(alpha(2, 2)).toBe(4)); });
"#,
        )
        .unwrap();
        let run2 = run_for_cache(root, CacheMode::ReadWrite);
        assert!(!run2.baseline_skipped, "test-set change must decline");
        assert_eq!(run2.reused_files, 1, "B stays reused: {run2:?}");
        assert!(
            run2.outcomes
                .keys()
                .any(|p| p.to_string_lossy().ends_with("a.ts")),
            "A re-executes with the new closure: {run2:?}"
        );
    }

    #[test]
    fn partial_cache_declines_fast_path_until_full_run_converges() {
        let dir = make_two_file_project();
        let root = dir.path();
        run_for_cache(root, CacheMode::ReadWrite);

        // Edit B, then refresh only A via a pattern run: B's stale entry is
        // dropped, leaving a partial cache.
        fs::write(
            root.join("src/b.ts"),
            "export function beta(a: number, b: number) { return b + a }\n",
        )
        .unwrap();
        run_for_cache_target(root, "src/a.ts", CacheMode::ReadWrite);

        // Full run: fast path declines (B has no entry); B re-executes
        // fresh while A is reused.
        let run3 = run_for_cache(root, CacheMode::ReadWrite);
        assert!(
            !run3.baseline_skipped,
            "partial cache must decline: {run3:?}"
        );
        assert_eq!(run3.reused_files, 1, "A reused: {run3:?}");

        // The cache is complete again: the next run hits the fast path.
        let run4 = run_for_cache(root, CacheMode::ReadWrite);
        assert!(
            run4.baseline_skipped,
            "pattern-refreshed cache must converge back to complete: {run4:?}"
        );
    }

    #[test]
    fn targeted_run_never_fast_paths() {
        let dir = make_two_file_project();
        let root = dir.path();
        run_for_cache(root, CacheMode::ReadWrite);
        // Unchanged universe, but a target is set: the baseline must run
        // even though every selected file is reused.
        let run2 = run_for_cache_target(root, "src/a.ts", CacheMode::ReadWrite);
        assert!(!run2.baseline_skipped, "{run2:?}");
        assert_eq!(run2.reused_files, 1, "a.ts itself is reused: {run2:?}");
    }

    #[test]
    fn cache_mode_for_maps_flags_to_modes() {
        assert_eq!(cache_mode_for(false, false, false), CacheMode::ReadWrite);
        assert_eq!(cache_mode_for(true, false, false), CacheMode::Bypass);
        assert_eq!(cache_mode_for(false, true, false), CacheMode::Bypass);
        assert_eq!(cache_mode_for(false, false, true), CacheMode::Fresh);
        // Narrowed runs bypass even when --no-cache is also given.
        assert_eq!(cache_mode_for(true, false, true), CacheMode::Bypass);
        assert_eq!(cache_mode_for(false, true, true), CacheMode::Bypass);
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
            CacheMode::Bypass,
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
            CacheMode::Bypass,
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
            CacheMode::Bypass,
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
            CacheMode::Bypass,
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
            CacheMode::Bypass,
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
            CacheMode::Bypass,
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
            CacheMode::Bypass,
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
            CacheMode::Bypass,
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
            CacheMode::Bypass,
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
            CacheMode::Bypass,
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

    /// The #61 `transactionsQueryString` shape: a multi-statement exported
    /// function whose mutable tokens live on lines *below* its first body
    /// statement, exercised by a strong sibling test across several inputs.
    const QUERY_SRC: &str = r#"export function q(p: { type: string | null; page: number }): string {
  const parts: string[] = [];
  if (p.type) parts.push("type=" + p.type);
  if (p.page !== 1) parts.push("page=" + String(p.page));
  return parts.length ? "?" + parts.join("&") : "";
}
"#;

    #[test]
    fn mutate_reaches_sites_below_function_first_line() {
        let dir = make_project(
            QUERY_SRC,
            r#"import { describe, it, expect } from "zero/test";
import { q } from "./src/foo.ts";
describe("q", () => {
  it("default", () => expect(q({ type: null, page: 1 })).toBe(""));
  it("type only", () => expect(q({ type: "a", page: 1 })).toBe("?type=a"));
  it("page only", () => expect(q({ type: null, page: 2 })).toBe("?page=2"));
  it("both", () => expect(q({ type: "a", page: 2 })).toBe("?type=a&page=2"));
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
            CacheMode::Bypass,
            &mut sink,
        )
        .expect("ok");

        assert!(summary.baseline_passed, "baseline failed: {summary:?}");
        // Mutants actually executed — not all filed unreachable (the bug).
        let executed = summary.killed + summary.survived + summary.errored;
        assert!(
            executed > 0,
            "expected executed mutants, got none (vacuous run): {summary:?}"
        );
        // The lit_str / lit_num / cmp sites live on lines 3–5, below the
        // function's first body line — exactly the sites the bug dropped.
        for op in [Operator::LitStr, Operator::LitNum, Operator::Cmp] {
            assert!(
                summary.per_operator.executed(op) > 0,
                "expected executed {} mutants below line 1: {summary:?}",
                op.id()
            );
        }
        // Non-vacuous: a strong test kills, and the score is killed-driven
        // rather than the vacuous 100% from zero execution.
        assert!(
            summary.killed > 0,
            "score should be killed-driven: {summary:?}"
        );
        assert_eq!(
            summary.survived, 0,
            "strong test should kill all: {summary:?}"
        );
    }

    #[test]
    fn mutate_weak_test_surfaces_survivor() {
        // Same source, but the sibling test calls `q(...)` without asserting
        // anything meaningful. Before the fix every site read `unreachable`
        // and the tool hid the vacuous test behind a perfect score; now the
        // mutants execute and survive, surfacing the gap.
        let dir = make_project(
            QUERY_SRC,
            r#"import { describe, it, expect } from "zero/test";
import { q } from "./src/foo.ts";
describe("q", () => {
  it("calls", () => {
    q({ type: "a", page: 2 });
    q({ type: null, page: 1 });
    expect(1).toBe(1);
  });
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
            CacheMode::Bypass,
            &mut sink,
        )
        .expect("ok");

        assert!(summary.baseline_passed, "baseline failed: {summary:?}");
        assert!(
            summary.survived > 0,
            "weak test should surface survivors, got none: {summary:?}"
        );
    }

    /// Scaffold the #64 `StatCard` shape: `src/widget.ts` (multi-line return
    /// with a continuation-line ternary, *no* sibling test), `src/page.ts`
    /// that calls it in both branches, and `page.test.ts` exercising `page`.
    fn make_transitive_project() -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_zero_toml(root);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("src/widget.ts"),
            r#"export function widget(props: { label: string; sub: string | null }): string {
  return `<div>` +
    `<b>${props.label}</b>` +
    (props.sub ? `<i>${props.sub}</i>` : "") +
    `</div>`;
}
"#,
        )
        .unwrap();
        fs::write(
            root.join("src/page.ts"),
            r#"import { widget } from "./widget.ts";
export function render(): string {
  return widget({ label: "X", sub: "y" }) + widget({ label: "Z", sub: null });
}
"#,
        )
        .unwrap();
        fs::write(
            root.join("page.test.ts"),
            r#"import { describe, it, expect } from "zero/test";
import { render } from "./src/page.ts";
describe("page", () => {
  it("renders both branches", () => {
    const out = render();
    expect(out.includes("<i>y</i>")).toBe(true);
    expect(out.includes("Z")).toBe(true);
  });
});
"#,
        )
        .unwrap();
        dir
    }

    #[test]
    fn mutate_reaches_transitively_tested_no_sibling() {
        let dir = make_transitive_project();
        let root = dir.path();
        let out = root.join("dist");

        // Transitive coverage is honored: `widget.ts` is credited via
        // `page.test.ts` even though it has no sibling test. Guards against a
        // future "sibling-only linkage" regression.
        let scope = CoverageScope::new(root.to_path_buf(), out.clone());
        let test_files = discover(DiscoveryOpts {
            root,
            out_dir: &out,
            extra_skip_dirs: &[],
            target: None,
            cwd: root,
        })
        .expect("discover")
        .files;
        let baseline = run_baseline(root, &scope, &test_files);
        let widget_covered = baseline
            .covered
            .iter()
            .find(|(p, _)| p.to_string_lossy().ends_with("widget.ts"));
        assert!(
            widget_covered.is_some_and(|(_, lines)| !lines.is_empty()),
            "widget.ts should be covered transitively: {:?}",
            baseline.covered.keys().collect::<Vec<_>>()
        );
        let widget_tests = baseline
            .src_to_tests
            .iter()
            .find(|(p, _)| p.to_string_lossy().ends_with("widget.ts"));
        assert!(
            widget_tests.is_some_and(|(_, tests)| tests
                .iter()
                .any(|t| t.to_string_lossy().ends_with("page.test.ts"))),
            "widget.ts should be linked to page.test.ts: {:?}",
            baseline.src_to_tests
        );

        // Targeting widget.ts, the continuation-line cond_neg site executes.
        let mut sink: Vec<u8> = Vec::new();
        let summary = run_inner(
            root,
            &out,
            root,
            Some("src/widget.ts"),
            Operator::ALL,
            None,
            true,
            Isolation::InProcess,
            1,
            CacheMode::Bypass,
            &mut sink,
        )
        .expect("ok");
        assert!(summary.baseline_passed, "baseline failed: {summary:?}");
        assert!(
            summary.per_operator.executed(Operator::CondNeg) > 0,
            "the continuation-line cond_neg site should execute, not be filed unreachable: {summary:?}"
        );
        let executed = summary.killed + summary.survived + summary.errored;
        assert!(executed > 0, "score should be non-vacuous: {summary:?}");
    }

    #[test]
    fn mutate_genuinely_unreached_still_unreachable() {
        // `unused` is imported (so the module loads) but never called, so its
        // body line never executes. After the fix the unreachable bucket must
        // not collapse: the arith site stays skipped, never executed.
        let dir = make_project(
            "export function unused(a: number, b: number) {\n  return a + b;\n}\n",
            r#"import { describe, it, expect } from "zero/test";
import { unused } from "./src/foo.ts";
describe("g", () => { it("noop", () => expect(typeof unused).toBe("function")); });
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
            CacheMode::Bypass,
            &mut sink,
        )
        .expect("ok");
        assert!(summary.baseline_passed, "baseline failed: {summary:?}");
        let arith = Operator::Arith.index();
        assert!(
            summary.per_operator.matched[arith] >= 1,
            "the arith site should be matched: {summary:?}"
        );
        assert_eq!(
            summary.per_operator.executed(Operator::Arith),
            0,
            "an uncalled function's sites must not execute: {summary:?}"
        );
        assert!(
            summary.skipped_unreachable > 0,
            "the unreachable bucket must not collapse to zero: {summary:?}"
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
            CacheMode::Bypass,
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
