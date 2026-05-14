//! Integration test for `zero dev` proxy mode.
//!
//! Starts a stub axum backend, then starts `zero dev` with that backend as
//! `[dev].proxy`, and asserts the proxy forwarding + HTML injection behavior.

use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::Router;
use axum::response::IntoResponse;
use axum::routing::get;
use tokio::net::TcpListener as AsyncListener;

fn pick_free_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

fn wait_for_port(port: u16, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if std::net::TcpStream::connect_timeout(
            &format!("127.0.0.1:{port}").parse().unwrap(),
            Duration::from_millis(100),
        )
        .is_ok()
        {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Spawn a minimal stub backend returning known responses.
async fn start_stub_backend(port: u16) {
    let app = Router::new()
        .route(
            "/",
            get(|| async {
                (
                    axum::http::StatusCode::OK,
                    [
                        ("content-type", "text/html; charset=utf-8"),
                        ("cache-control", "max-age=3600"),
                        ("etag", "\"abc\""),
                    ],
                    "<html><head><title>X</title></head><body>hi</body></html>",
                )
                    .into_response()
            }),
        )
        .route(
            "/api/data",
            get(|| async {
                (
                    axum::http::StatusCode::OK,
                    [("content-type", "application/json")],
                    r#"{"x":1}"#,
                )
                    .into_response()
            }),
        );

    let listener = AsyncListener::bind(format!("127.0.0.1:{port}"))
        .await
        .expect("failed to bind stub backend");
    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });
}

#[test]
fn proxy_mode_injects_html_strips_cache_and_forwards_json() {
    let tmp = tempfile::tempdir().unwrap();
    let backend_port = pick_free_port();
    let dev_port = pick_free_port();

    // Write toml with proxy configured.
    std::fs::write(
        tmp.path().join("zero.toml"),
        format!(
            "[project]\nroot = \"web\"\n\n[dev]\nport = {dev_port}\nproxy = \"http://127.0.0.1:{backend_port}\"\n"
        ),
    )
    .unwrap();
    // The server requires the project root dir to exist.
    std::fs::create_dir_all(tmp.path().join("web")).unwrap();

    // Start the stub backend and the dev server.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(start_stub_backend(backend_port));
    assert!(
        wait_for_port(backend_port, Duration::from_secs(3)),
        "stub backend did not start"
    );

    let bin = assert_cmd::cargo::cargo_bin("zero");
    let child = Command::new(&bin)
        .arg("dev")
        .current_dir(tmp.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let _guard = Arc::new(std::sync::Mutex::new(ChildGuard(child)));

    assert!(
        wait_for_port(dev_port, Duration::from_secs(5)),
        "dev server did not start"
    );

    rt.block_on(async move {
        let client = reqwest::Client::new();
        let base = format!("http://127.0.0.1:{dev_port}");

        // HTML response: scripts injected, cache headers stripped + replaced.
        let resp = client.get(format!("{base}/")).send().await.unwrap();
        assert_eq!(resp.status(), 200);
        let cc = resp
            .headers()
            .get("cache-control")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert_eq!(cc, "no-store, no-cache, must-revalidate, max-age=0");
        assert!(
            resp.headers().get("etag").is_none(),
            "etag must be stripped"
        );
        let body = resp.text().await.unwrap();
        assert!(
            body.contains(r#"<script type="importmap">"#),
            "importmap script missing"
        );

        // JSON response: body unchanged, content-type preserved.
        let resp = client.get(format!("{base}/api/data")).send().await.unwrap();
        assert_eq!(resp.status(), 200);
        let ctype = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(ctype.contains("application/json"), "content-type: {ctype}");
        let body = resp.text().await.unwrap();
        assert_eq!(body, r#"{"x":1}"#);

        // WebSocket upgrade → 501.
        let resp = client
            .get(format!("{base}/ws"))
            .header("upgrade", "websocket")
            .header("connection", "upgrade")
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 501, "websocket upgrade must be 501");
    });
}

#[test]
fn proxy_returns_502_when_backend_unreachable() {
    let tmp = tempfile::tempdir().unwrap();
    let backend_port = pick_free_port();
    let dev_port = pick_free_port();

    std::fs::write(
        tmp.path().join("zero.toml"),
        format!(
            "[project]\nroot = \"web\"\n\n[dev]\nport = {dev_port}\nproxy = \"http://127.0.0.1:{backend_port}\"\n"
        ),
    )
    .unwrap();
    std::fs::create_dir_all(tmp.path().join("web")).unwrap();

    let bin = assert_cmd::cargo::cargo_bin("zero");
    let child = Command::new(&bin)
        .arg("dev")
        .current_dir(tmp.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let _guard = ChildGuard(child);

    assert!(
        wait_for_port(dev_port, Duration::from_secs(5)),
        "dev server did not start"
    );

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async move {
        let client = reqwest::Client::new();
        let resp = client
            .get(format!("http://127.0.0.1:{dev_port}/anything"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 502, "unreachable backend should be 502");
        let body = resp.text().await.unwrap();
        assert!(
            body.contains("Cannot reach backend"),
            "502 body should mention backend: {body}"
        );
    });
}
