//! Integration test: in no-proxy mode every unmatched URL returns `index.html`
//! with dev scripts injected.

use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

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

#[test]
fn no_proxy_fallback_injects_scripts_and_serves_no_cache() {
    let tmp = tempfile::tempdir().unwrap();
    let port = pick_free_port();

    std::fs::write(
        tmp.path().join("zero.toml"),
        format!("[project]\nroot = \"web\"\n\n[dev]\nport = {port}\n"),
    )
    .unwrap();
    assert_cmd::Command::cargo_bin("zero")
        .unwrap()
        .arg("init")
        .current_dir(tmp.path())
        .assert()
        .success();

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
        wait_for_port(port, Duration::from_secs(5)),
        "server did not start"
    );

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async move {
        let client = reqwest::Client::new();
        let base = format!("http://127.0.0.1:{port}");

        // GET / returns scaffolded index.html with scripts injected.
        let resp = client.get(format!("{base}/")).send().await.unwrap();
        assert_eq!(resp.status(), 200, "GET / should be 200");
        let ctype = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(
            ctype.starts_with("text/html"),
            "wrong content-type: {ctype}"
        );
        let cc = resp
            .headers()
            .get("cache-control")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert_eq!(cc, "no-store, no-cache, must-revalidate, max-age=0");
        let body = resp.text().await.unwrap();
        assert!(
            body.contains(r#"<script type="importmap">"#),
            "missing importmap script"
        );
        assert!(
            body.contains(r#"<script type="module" src="/src/app.js">"#),
            "missing module script"
        );

        // Any other path also returns the same SPA shell.
        let resp = client
            .get(format!("{base}/anything-else"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200, "GET /anything-else should be 200");
        let body2 = resp.text().await.unwrap();
        assert!(body2.contains(r#"<script type="importmap">"#));
    });
}
