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
    pub skipped_equivalent: usize,
    pub baseline_passed: bool,
    pub outcomes: BTreeMap<PathBuf, Vec<(MutationSite, MutantStatus)>>,
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
                        "unknown operator id: {id:?}; expected one of {:?}",
                        Operator::ALL
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
    let total_skipped = summary.skipped_unreachable + summary.skipped_equivalent;
    writeln!(
        w,
        "  Skipped:   {:>3}  [unreachable: {}, equivalent: {}]",
        total_skipped, summary.skipped_unreachable, summary.skipped_equivalent
    )?;

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
    let value = serde_json::json!({
        "totals": {
            "generated": summary.generated,
            "killed": summary.killed,
            "survived": summary.survived,
            "errored": summary.errored,
            "skipped": summary.skipped_unreachable + summary.skipped_equivalent,
            "score": (summary.score() * 1000.0).round() / 1000.0,
        },
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
///   isolation only). `1` = sequential, current default.
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
    // For `zero mutate`, `target` filters the *source* files to mutate
    // (spec §3.1), not the test files. We always run the full test suite
    // as the baseline so the test-impact map is complete; per-mutant we
    // still only run the tests that import the mutated source.
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

    // 1. Baseline run with coverage on. Also record, per test file, the
    // set of in-scope src paths it loaded — this becomes the test-impact
    // map used in step 4 (only run tests that actually import the mutated
    // file).
    let scope = CoverageScope::new(root.to_path_buf(), out_dir.to_path_buf());
    let mut covered: HashMap<PathBuf, HashSet<u32>> = HashMap::new();
    let mut src_to_tests: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
    let mut baseline_passed = true;
    for f in &test_files {
        let ctx = Rc::new(CoverageContext::new(scope.clone()));
        let outcome = run_file_with_coverage(root, f, Some(ctx.clone()));
        if outcome.result.load_error.is_some() {
            baseline_passed = false;
        }
        if outcome
            .result
            .outcomes
            .iter()
            .any(|o| matches!(o.status, Status::Failed))
        {
            baseline_passed = false;
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
        // Drain maps to register universe (even uncovered files appear so
        // their absence from `covered` is meaningful).
        let _: Vec<CoverageMap> = ctx.drain_maps();
    }
    // Sort + dedup each per-src test list so the per-mutant subprocess gets
    // a stable, minimal order (and the killed-test short-circuit is
    // reproducible across runs).
    for v in src_to_tests.values_mut() {
        v.sort();
        v.dedup();
    }
    summary.baseline_passed = baseline_passed;
    if !baseline_passed {
        return Ok(summary);
    }

    // 2. Walk `src/` and generate mutants per file.
    //
    // Coverage rule: if the baseline loaded any line of `src`, the lines it
    // touched are in `covered[src]`; we filter sites against that set.
    // If `src` has *no* coverage entry at all, no test exercises it — every
    // mutant would survive trivially. Treat that as an empty covered set so
    // the generator routes every site into `skipped_unreachable` instead of
    // queuing useless mutant runs.
    let empty_lines: HashSet<u32> = HashSet::new();
    let src_files = filter_src_files(walk_src(&scope), cwd, target, progress)?;
    let mut all_sites: Vec<(PathBuf, MutationSite, String, String)> = Vec::new();
    // (src_path, site, raw_source, baseline_js)
    for src in &src_files {
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
        let (sites, unreachable) = match generate(&raw, src, &gen_opts) {
            Ok(r) => r,
            Err(_) => continue,
        };
        summary.skipped_unreachable += unreachable;
        for s in sites {
            all_sites.push((src.clone(), s, raw.clone(), baseline_js.clone()));
        }
    }

    // 3. Apply global max-mutants cap.
    if let Some(max) = max_mutants
        && all_sites.len() > max
    {
        all_sites.truncate(max);
    }

    // 4a. Pre-apply pass on the main thread: turn every site into either
    // an enqueued unit of work or an immediate skip / errored tally. Apply
    // is CPU-bound but fast relative to a Boa test run, so doing it
    // sequentially is fine — it lets us count `total` accurately before
    // dispatching workers.
    let mut queue: Vec<MutantWork> = Vec::new();
    for (src_path, site, raw_src, baseline_js) in all_sites.iter() {
        match apply(raw_src, src_path, site) {
            Ok(mutated_js) => {
                if mutated_js == *baseline_js {
                    summary.skipped_equivalent += 1;
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
                summary
                    .outcomes
                    .entry(src_path.clone())
                    .or_default()
                    .push((site.clone(), MutantStatus::Errored));
            }
        }
    }
    let total = queue.len();

    // 4b. Dispatch — sequential if a single thread or running in-process,
    // otherwise spawn a worker pool of subprocess-driver threads pulling
    // from a shared work index. Either way, results stream back via the
    // same channel so the main loop below is identical for both.
    let result_rx = if threads <= 1 || isolation == Isolation::InProcess {
        dispatch_sequential(
            root.to_path_buf(),
            queue,
            src_to_tests,
            test_files,
            isolation,
        )
    } else {
        dispatch_parallel(root.to_path_buf(), queue, src_to_tests, test_files, threads)
    };

    // 4c. Consume results: print progress in completion order, fold into
    // the summary.
    let mut completed = 0usize;
    while let Ok((src_path, site, status)) = result_rx.recv() {
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
        match status {
            MutantStatus::Killed => summary.killed += 1,
            MutantStatus::Survived => summary.survived += 1,
            MutantStatus::Errored => summary.errored += 1,
        }
        summary
            .outcomes
            .entry(src_path)
            .or_default()
            .push((site, status));
    }

    Ok(summary)
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

    write_terminal_summary(&mut stdout, &summary, &cwd, quiet)?;
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
}
