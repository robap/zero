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
    Init,
    /// Run the development server
    Dev,
    /// Produce a production build
    Build,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Commands::Init => cmd::init::run().await,
        Commands::Dev => cmd::dev::run().await,
        Commands::Build => cmd::build::run().await,
    };
    if let Err(err) = result {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}
