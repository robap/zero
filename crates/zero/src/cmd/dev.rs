//! `zero dev` subcommand entry point.

use zero_config::Config;
use zero_dev::server::serve;

/// Run the `zero dev` subcommand.
///
/// # Returns
/// `Ok(())` on graceful shutdown, an error otherwise.
pub async fn run() -> anyhow::Result<()> {
    let config = Config::load_from_cwd()?;
    serve(config).await
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

    #[tokio::test]
    async fn missing_zero_toml_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        let _g = CwdGuard::enter(tmp.path());
        let err = super::run().await.expect_err("should fail");
        let msg = format!("{err}");
        assert!(msg.contains("zero.toml"), "msg: {msg}");
    }

    #[tokio::test]
    async fn missing_project_root_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        let _g = CwdGuard::enter(tmp.path());
        // Write a config that points at a non-existent root directory.
        std::fs::write(
            tmp.path().join("zero.toml"),
            "[project]\nroot = \"absent\"\n",
        )
        .unwrap();
        let err = super::run().await.expect_err("should fail");
        let msg = format!("{err}");
        assert!(msg.contains("not found"), "msg: {msg}");
    }
}
