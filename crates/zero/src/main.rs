use clap::{Parser, Subcommand};
use zero::cmd;

/// Default thread count for `zero mutate`: cgroup-aware core count, capped
/// at 8 to leave headroom on bigger machines for the user's IDE / build.
fn default_threads() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .clamp(1, 8)
}

#[derive(Parser)]
#[command(name = "zero", version, about = "The zero framework CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scaffold a new zero app in the current directory
    Init {
        /// Skip the pre-flight confirmation prompt.
        #[arg(long, short = 'y', default_value_t = false)]
        yes: bool,
    },
    /// Run the development server
    Dev,
    /// Build, then serve the production output locally
    Preview,
    /// Produce a production build
    Build {
        /// Emit an external source map alongside the bundle.
        #[arg(long, default_value_t = false)]
        sourcemap: bool,
        /// Suppress the source map even if `[build] sourcemap = true` is set.
        #[arg(long, default_value_t = false)]
        no_sourcemap: bool,
    },
    /// Run *.test.js / *.spec.js under the embedded engine
    Test {
        /// Optional file path or substring filter
        target: Option<String>,
        /// Emit line + function coverage from `src/` to terminal and `coverage/coverage.json`
        #[arg(long, default_value_t = false)]
        coverage: bool,
    },
    /// Run mutation testing across `src/`
    Mutate {
        /// Optional file path or substring filter
        target: Option<String>,
        /// Comma-separated operator IDs to restrict mutations to
        #[arg(long)]
        operators: Option<String>,
        /// Cap total mutants generated
        #[arg(long)]
        max_mutants: Option<usize>,
        /// Suppress per-mutant progress lines; print summary only
        #[arg(long, short = 'q', default_value_t = false)]
        quiet: bool,
        /// Number of mutants to exercise in parallel. Each worker runs in
        /// its own subprocess; defaults to `min(cores, 8)`. Pass `1` for
        /// sequential.
        #[arg(long, default_value_t = default_threads())]
        threads: usize,
        /// Ignore the incremental cache: re-run every mutant and rewrite
        /// `mutation/cache.json`.
        #[arg(long, default_value_t = false)]
        no_cache: bool,
        /// Per-mutant timeout (e.g. 10s, 500ms). Default: max(2s, baseline×5).
        #[arg(long)]
        timeout: Option<String>,
    },
    /// Refresh framework files in .zero/
    Update {
        /// Skip the pre-flight confirmation prompt.
        #[arg(long, short = 'y', default_value_t = false)]
        yes: bool,
    },
    /// Run the design-system lint over user SCSS / CSS.
    Lint {
        /// Drop the source-snippet line and caret per diagnostic.
        #[arg(long, short = 'q', default_value_t = false)]
        quiet: bool,
    },
    /// Internal: run one mutant's tests in an isolated child process. Used
    /// by `zero mutate` to keep engine-internal aborts from killing the parent.
    #[command(hide = true)]
    MutateWorker {
        #[arg(long)]
        root: std::path::PathBuf,
        #[arg(long)]
        mutated_src: std::path::PathBuf,
        #[arg(long)]
        mutated_js_file: std::path::PathBuf,
        #[arg(long)]
        tests_file: std::path::PathBuf,
        /// Per-mutant engine deadline in milliseconds (parent-supplied).
        #[arg(long)]
        timeout_ms: Option<u64>,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Commands::Init { yes } => cmd::init::run(yes).await,
        Commands::Dev => cmd::dev::run().await,
        Commands::Preview => cmd::preview::run().await,
        Commands::Build {
            sourcemap,
            no_sourcemap,
        } => {
            let override_flag = match (sourcemap, no_sourcemap) {
                (true, true) => {
                    eprintln!("error: --sourcemap and --no-sourcemap are mutually exclusive");
                    std::process::exit(2);
                }
                (true, false) => Some(true),
                (false, true) => Some(false),
                (false, false) => None,
            };
            cmd::build::run(override_flag).await
        }
        Commands::Test { target, coverage } => cmd::test::run(target, coverage).await,
        Commands::Mutate {
            target,
            operators,
            max_mutants,
            quiet,
            threads,
            no_cache,
            timeout,
        } => {
            cmd::mutate::run(
                target,
                operators,
                max_mutants,
                quiet,
                threads,
                no_cache,
                timeout,
            )
            .await
        }
        Commands::MutateWorker {
            root,
            mutated_src,
            mutated_js_file,
            tests_file,
            timeout_ms,
        } => {
            let status = cmd::mutate::worker_main(
                &root,
                &mutated_src,
                &mutated_js_file,
                &tests_file,
                timeout_ms,
            );
            std::process::exit(status.to_exit_code());
        }
        Commands::Update { yes } => cmd::update::run(yes).await,
        Commands::Lint { quiet } => cmd::lint::run(quiet).await,
    };
    if let Err(err) = result {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn parsed_threads(args: &[&str]) -> usize {
        let cli = Cli::try_parse_from(args).expect("parse");
        match cli.command {
            Commands::Mutate { threads, .. } => threads,
            _ => panic!("expected mutate"),
        }
    }

    fn parsed_timeout(args: &[&str]) -> Option<String> {
        let cli = Cli::try_parse_from(args).expect("parse");
        match cli.command {
            Commands::Mutate { timeout, .. } => timeout,
            _ => panic!("expected mutate"),
        }
    }

    #[test]
    fn timeout_absent_is_none() {
        assert_eq!(parsed_timeout(&["zero", "mutate"]), None);
    }

    #[test]
    fn timeout_flag_parses_value() {
        assert_eq!(
            parsed_timeout(&["zero", "mutate", "--timeout", "10s"]),
            Some("10s".to_string())
        );
    }

    #[test]
    fn timeout_composes_with_threads_and_no_cache() {
        let cli = Cli::try_parse_from([
            "zero",
            "mutate",
            "--timeout",
            "500ms",
            "--threads",
            "2",
            "--no-cache",
        ])
        .expect("parse");
        match cli.command {
            Commands::Mutate {
                timeout,
                threads,
                no_cache,
                ..
            } => {
                assert_eq!(timeout, Some("500ms".to_string()));
                assert_eq!(threads, 2);
                assert!(no_cache);
            }
            _ => panic!("expected mutate"),
        }
    }

    #[test]
    fn threads_default_uses_available_parallelism() {
        let got = parsed_threads(&["zero", "mutate"]);
        assert_eq!(got, default_threads());
    }

    #[test]
    fn threads_explicit_overrides_default() {
        assert_eq!(parsed_threads(&["zero", "mutate", "--threads", "2"]), 2);
    }

    #[test]
    fn threads_explicit_one_still_works() {
        assert_eq!(parsed_threads(&["zero", "mutate", "--threads", "1"]), 1);
    }

    #[test]
    fn default_threads_is_bounded_and_at_least_one() {
        let n = default_threads();
        assert!((1..=8).contains(&n), "got {n}");
    }

    fn parsed_no_cache(args: &[&str]) -> bool {
        let cli = Cli::try_parse_from(args).expect("parse");
        match cli.command {
            Commands::Mutate { no_cache, .. } => no_cache,
            _ => panic!("expected mutate"),
        }
    }

    #[test]
    fn no_cache_defaults_to_false() {
        assert!(!parsed_no_cache(&["zero", "mutate"]));
    }

    #[test]
    fn no_cache_flag_parses_to_true() {
        assert!(parsed_no_cache(&["zero", "mutate", "--no-cache"]));
    }

    #[test]
    fn mutate_worker_parses_timeout_ms() {
        let cli = Cli::try_parse_from([
            "zero",
            "mutate-worker",
            "--root",
            "/p",
            "--mutated-src",
            "/p/src/a.ts",
            "--mutated-js-file",
            "/tmp/x.js",
            "--tests-file",
            "/tmp/x.tests",
            "--timeout-ms",
            "1500",
        ])
        .expect("parse");
        match cli.command {
            Commands::MutateWorker { timeout_ms, .. } => assert_eq!(timeout_ms, Some(1500)),
            _ => panic!("expected mutate-worker"),
        }
    }

    #[test]
    fn mutate_worker_timeout_ms_optional() {
        let cli = Cli::try_parse_from([
            "zero",
            "mutate-worker",
            "--root",
            "/p",
            "--mutated-src",
            "/p/src/a.ts",
            "--mutated-js-file",
            "/tmp/x.js",
            "--tests-file",
            "/tmp/x.tests",
        ])
        .expect("parse");
        match cli.command {
            Commands::MutateWorker { timeout_ms, .. } => assert_eq!(timeout_ms, None),
            _ => panic!("expected mutate-worker"),
        }
    }

    #[test]
    fn no_cache_composes_with_threads() {
        let cli =
            Cli::try_parse_from(["zero", "mutate", "--no-cache", "--threads", "2"]).expect("parse");
        match cli.command {
            Commands::Mutate {
                no_cache, threads, ..
            } => {
                assert!(no_cache);
                assert_eq!(threads, 2);
            }
            _ => panic!("expected mutate"),
        }
    }
}
