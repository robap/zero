//! `zero test` subcommand: discover and run `*.test.js` / `*.spec.js` files.

use std::rc::Rc;

use crate::config::Config;
use crate::test_runner::coverage::{CoverageAggregator, CoverageScope};
use crate::test_runner::discovery::{DiscoveryOpts, DiscoveryResult, discover};
use crate::test_runner::harness::run_file_with_coverage;
use crate::test_runner::loader::CoverageContext;
use crate::test_runner::reporter::Reporter;

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
