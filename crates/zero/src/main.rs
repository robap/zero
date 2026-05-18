use clap::{Parser, Subcommand};
use zero::cmd;

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
        /// Number of mutants to exercise in parallel. Each worker runs in its
        /// own subprocess; defaults to 1 (sequential).
        #[arg(long, default_value_t = 1)]
        threads: usize,
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
    /// by `zero mutate` to keep Boa-internal aborts from killing the parent.
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
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Commands::Init { yes } => cmd::init::run(yes).await,
        Commands::Dev => cmd::dev::run().await,
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
        } => cmd::mutate::run(target, operators, max_mutants, quiet, threads).await,
        Commands::MutateWorker {
            root,
            mutated_src,
            mutated_js_file,
            tests_file,
        } => {
            let status =
                cmd::mutate::worker_main(&root, &mutated_src, &mutated_js_file, &tests_file);
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
