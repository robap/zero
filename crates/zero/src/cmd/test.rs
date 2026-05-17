//! `zero test` subcommand: discover and run `*.test.js` / `*.spec.js` files.

use std::rc::Rc;

use zero_config::Config;
use zero_test_runner::coverage::{CoverageAggregator, CoverageScope};
use zero_test_runner::discovery::{DiscoveryOpts, DiscoveryResult, discover};
use zero_test_runner::harness::run_file_with_coverage;
use zero_test_runner::loader::CoverageContext;
use zero_test_runner::reporter::Reporter;

/// Run `zero test [target] [--coverage]`.
///
/// # Parameters
/// - `target`: optional file path or substring filter.
/// - `coverage`: when true, instrument `src/` files and emit a coverage
///   report (terminal table + `coverage/coverage.json`).
///
/// # Returns
/// `Ok(())` on success (all tests pass); propagates config / I/O errors.
pub async fn run(target: Option<String>, coverage: bool) -> anyhow::Result<()> {
    let config = Config::load_from_cwd()?;
    let cwd = std::env::current_dir()?;
    let root = cwd.join(&config.project.root);
    let out = cwd.join(&config.build.out);

    let DiscoveryResult { files } = discover(DiscoveryOpts {
        root: &root,
        out_dir: &out,
        target: target.as_deref(),
    })?;

    if files.is_empty() {
        println!("zero test — no test files found");
        return Ok(());
    }

    let mut stdout = std::io::stdout().lock();
    let mut reporter = Reporter::new_with_root(&mut stdout, root.clone());
    let mut aggregator = if coverage {
        Some(CoverageAggregator::new())
    } else {
        None
    };

    for f in &files {
        let cov_ctx = if coverage {
            let scope = CoverageScope::new(root.clone(), out.clone());
            Some(Rc::new(CoverageContext::new(scope)))
        } else {
            None
        };
        let outcome = run_file_with_coverage(&root, f, cov_ctx.clone());
        reporter.record_file(&outcome.result)?;
        if let (Some(ctx), Some(agg)) = (cov_ctx, aggregator.as_mut()) {
            for map in ctx.drain_maps() {
                agg.register(map);
            }
            if let Some(value) = &outcome.coverage {
                agg.ingest_run(value);
            }
        }
    }
    let totals = reporter.finish()?;

    if let Some(agg) = aggregator {
        let mut stdout = std::io::stdout().lock();
        agg.write_terminal(&mut stdout, &root)?;
        agg.write_json(&root)?;
    }

    if totals.failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::test_support::CWD_LOCK;
    use std::path::Path;

    struct CwdGuard {
        prev: std::path::PathBuf,
        _lock: std::sync::MutexGuard<'static, ()>,
    }
    impl CwdGuard {
        fn enter(target: &Path) -> Self {
            let lock = CWD_LOCK.lock().unwrap();
            let prev = std::env::current_dir().unwrap();
            std::env::set_current_dir(target).unwrap();
            CwdGuard { prev, _lock: lock }
        }
    }
    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.prev);
        }
    }

    fn write_minimal_project(root: &Path) {
        std::fs::write(
            root.join("zero.toml"),
            "[project]\nroot = \"web\"\n\n[build]\nout = \"dist\"\n",
        )
        .unwrap();
        let web = root.join("web");
        std::fs::create_dir_all(web.join("src")).unwrap();
        std::fs::write(
            web.join("index.html"),
            "<!doctype html><html><head></head><body></body></html>",
        )
        .unwrap();
        std::fs::write(web.join("src").join("app.ts"), "export const x = 1;\n").unwrap();
    }

    #[tokio::test]
    async fn missing_zero_toml_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        let _g = CwdGuard::enter(tmp.path());
        let err = super::run(None, false).await.expect_err("should fail");
        let msg = format!("{err}");
        assert!(msg.contains("zero.toml"), "msg: {msg}");
    }

    #[tokio::test]
    async fn no_tests_found_returns_ok_quietly() {
        let tmp = tempfile::tempdir().unwrap();
        let _g = CwdGuard::enter(tmp.path());
        write_minimal_project(tmp.path());
        // No `.test.js` files anywhere.
        let res = super::run(None, false).await;
        assert!(res.is_ok(), "should succeed when no tests discovered");
    }

    #[tokio::test]
    async fn target_filter_with_no_matches_returns_ok() {
        let tmp = tempfile::tempdir().unwrap();
        let _g = CwdGuard::enter(tmp.path());
        write_minimal_project(tmp.path());
        // Write a test file but filter it out by an unmatchable target.
        std::fs::write(
            tmp.path().join("web").join("src").join("a.test.js"),
            "test('x', () => {});\n",
        )
        .unwrap();
        let res = super::run(Some("definitely-not-matching".into()), false).await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn coverage_true_writes_coverage_json() {
        let tmp = tempfile::tempdir().unwrap();
        let _g = CwdGuard::enter(tmp.path());
        write_minimal_project(tmp.path());
        // Add a trivial passing test using the zero/test API.
        std::fs::write(
            tmp.path().join("web").join("src").join("simple.test.js"),
            "import { it } from 'zero/test';\nit('ok', () => {});\n",
        )
        .unwrap();
        let res = super::run(None, true).await;
        assert!(res.is_ok());
        let cov = tmp
            .path()
            .join("web")
            .join("coverage")
            .join("coverage.json");
        assert!(cov.is_file(), "expected coverage.json at {}", cov.display());
    }
}
