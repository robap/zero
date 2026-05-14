//! `zero dev` subcommand entry point.

use crate::config::Config;
use crate::dev::server::serve;

/// Run the `zero dev` subcommand.
///
/// # Returns
/// `Ok(())` on graceful shutdown, an error otherwise.
pub async fn run() -> anyhow::Result<()> {
    let config = Config::load_from_cwd()?;
    serve(config).await
}
