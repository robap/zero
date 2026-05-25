//! `zero preview` subcommand entry point.

use zero_config::Config;
use zero_dev::preview::serve_preview;

use crate::cmd::build::build_inner;

/// Run the `zero preview` subcommand: build the app, then serve `dist/`.
///
/// # Returns
/// `Ok(())` on graceful shutdown, an error if config load, build, or bind fails.
pub async fn run() -> anyhow::Result<()> {
    let config = Config::load_from_cwd()?;
    run_with_config(&config).await
}

/// Build, then serve, using a pre-loaded `Config`.
///
/// Tests inject a `Config` constructed in-memory (so they can use port 0 /
/// ephemeral ports that the TOML loader rejects).
///
/// # Parameters
/// - `config`: validated `zero.toml` config.
///
/// # Returns
/// `Ok(())` on graceful shutdown, an error if build or bind fails.
pub(crate) async fn run_with_config(config: &Config) -> anyhow::Result<()> {
    println!("zero preview — running zero build…");
    build_inner(config, None).await?;
    serve_preview(config).await
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
            "<!doctype html><html><head><title>x</title></head><body></body></html>",
        )
        .unwrap();
        std::fs::write(
            web.join("src").join("app.ts"),
            "export const x = 1;\nconsole.log(x);\n",
        )
        .unwrap();
    }

    fn config_with_port(port: u16) -> zero_config::Config {
        zero_config::Config {
            project: zero_config::ProjectConfig {
                root: "web".to_string(),
            },
            dev: zero_config::DevConfig {
                port,
                proxy: None,
                sourcemap: true,
            },
            build: zero_config::BuildConfig {
                out: "dist".to_string(),
                sourcemap: false,
            },
        }
    }

    fn free_port() -> u16 {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
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
        std::fs::write(
            tmp.path().join("zero.toml"),
            "[project]\nroot = \"absent\"\n",
        )
        .unwrap();
        let err = super::run().await.expect_err("should fail");
        let msg = format!("{err}");
        // The build step bails on the missing project root before bind.
        assert!(
            msg.contains("absent") || msg.contains("not found") || msg.contains("No such"),
            "msg: {msg}"
        );
    }

    #[tokio::test]
    async fn auto_builds_before_serving_then_serves_index() {
        let tmp = tempfile::tempdir().unwrap();
        let _g = CwdGuard::enter(tmp.path());
        write_minimal_project(tmp.path());
        let port = free_port();
        let config = config_with_port(port);
        let task = tokio::spawn(async move { super::run_with_config(&config).await });

        let mut connected = false;
        for _ in 0..50 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            if tokio::net::TcpStream::connect(("127.0.0.1", port))
                .await
                .is_ok()
            {
                connected = true;
                break;
            }
        }
        assert!(connected, "server never accepted on port {port}");

        let resp = reqwest::get(format!("http://127.0.0.1:{port}/"))
            .await
            .expect("request failed");
        assert_eq!(resp.status(), reqwest::StatusCode::OK);
        let body = resp.text().await.unwrap();
        assert!(
            body.contains("assets/app."),
            "expected built index.html with script tag, got: {body}"
        );
        assert!(
            tmp.path().join("dist").join("index.html").is_file(),
            "auto-build did not produce dist/index.html"
        );

        task.abort();
        let _ = task.await;
    }

    #[tokio::test]
    async fn build_failure_does_not_bind() {
        let tmp = tempfile::tempdir().unwrap();
        let _g = CwdGuard::enter(tmp.path());
        // Project with broken TS so the build fails before bind.
        std::fs::write(tmp.path().join("zero.toml"), "[project]\nroot = \"web\"\n").unwrap();
        let web = tmp.path().join("web");
        std::fs::create_dir_all(web.join("src")).unwrap();
        std::fs::write(
            web.join("index.html"),
            "<!doctype html><html><head></head><body></body></html>",
        )
        .unwrap();
        std::fs::write(
            web.join("src").join("app.ts"),
            "this is !!! not @ valid ts $$",
        )
        .unwrap();

        let port = free_port();
        let config = config_with_port(port);
        let res = super::run_with_config(&config).await;
        assert!(res.is_err(), "expected build failure to surface as Err");
        // The listener was never bound, so the port must still be free.
        let bind = std::net::TcpListener::bind(("127.0.0.1", port));
        assert!(
            bind.is_ok(),
            "port {port} should be free after build failure"
        );
    }
}
