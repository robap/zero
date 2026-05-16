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
    },
    /// Refresh framework files in .zero/
    Update {
        /// Skip the pre-flight confirmation prompt.
        #[arg(long, short = 'y', default_value_t = false)]
        yes: bool,
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
        Commands::Test { target } => cmd::test::run(target).await,
        Commands::Update { yes } => cmd::update::run(yes).await,
    };
    if let Err(err) = result {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
