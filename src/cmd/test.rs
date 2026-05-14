//! `zero test` subcommand: discover and run `*.test.js` / `*.spec.js` files.

use crate::config::Config;
use crate::test_runner;
use crate::test_runner::discovery::{DiscoveryOpts, DiscoveryResult, discover};
use crate::test_runner::reporter::Reporter;

/// Run `zero test [target]`.
///
/// # Parameters
/// - `target`: optional file path or substring filter.
///
/// # Returns
/// `Ok(())` on success (all tests pass); propagates config / I/O errors.
pub async fn run(target: Option<String>) -> anyhow::Result<()> {
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
    let mut reporter = Reporter::new(&mut stdout);
    for f in &files {
        let result = test_runner::run_file(&root, f);
        reporter.record_file(&result)?;
    }
    let totals = reporter.finish()?;
    if totals.failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}
