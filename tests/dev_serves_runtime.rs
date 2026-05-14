//! Integration test for `zero dev`: spawn the binary, hit `/zero.js`,
//! assert status / content-type / cache headers / body match the runtime.

use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Find an OS-assigned free TCP port by binding `127.0.0.1:0` and immediately
/// dropping the listener. There's a tiny race window before the dev server
/// rebinds, but it's adequate for tests.
fn pick_free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

/// Wait up to `timeout` for a TCP connect to `127.0.0.1:port` to succeed.
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

/// Guard that kills the child process on drop.
struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn write_toml(dir: &std::path::Path, port: u16) {
    std::fs::write(
        dir.join("zero.toml"),
        format!("[project]\nroot = \"web\"\n\n[dev]\nport = {port}\n"),
    )
    .unwrap();
}

#[test]
fn serves_runtime_with_no_cache_headers() {
    let tmp = tempfile::tempdir().unwrap();
    let port = pick_free_port();
    write_toml(tmp.path(), port);
    // The server now requires the project root dir to exist.
    std::fs::create_dir_all(tmp.path().join("web")).unwrap();

    let bin = assert_cmd::cargo::cargo_bin("zero");
    let child = Command::new(&bin)
        .arg("dev")
        .current_dir(tmp.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn zero dev");
    let _guard = ChildGuard(child);

    assert!(
        wait_for_port(port, Duration::from_secs(5)),
        "dev server did not start on port {port}"
    );

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async move {
        let client = reqwest::Client::new();
        let url = format!("http://127.0.0.1:{port}/zero.js");
        let resp = client.get(&url).send().await.expect("GET /zero.js failed");
        assert_eq!(resp.status(), 200);
        let ctype = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        assert!(
            ctype.starts_with("application/javascript"),
            "expected application/javascript content-type, got {ctype}"
        );
        let cc = resp
            .headers()
            .get("cache-control")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        assert_eq!(cc, "no-store, no-cache, must-revalidate, max-age=0");
        let pragma = resp
            .headers()
            .get("pragma")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert_eq!(pragma, "no-cache");
        let expires = resp
            .headers()
            .get("expires")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert_eq!(expires, "0");
        let body = resp.text().await.unwrap();
        assert_eq!(body, zero::runtime::runtime_module());
    });
}

#[test]
fn port_in_use_exits_non_zero() {
    let tmp = tempfile::tempdir().unwrap();
    // Bind a port and HOLD it for the duration of the test.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    write_toml(tmp.path(), port);
    // The server now requires the project root dir to exist.
    std::fs::create_dir_all(tmp.path().join("web")).unwrap();

    let bin = assert_cmd::cargo::cargo_bin("zero");
    let output = Command::new(&bin)
        .arg("dev")
        .current_dir(tmp.path())
        .output()
        .expect("failed to run zero dev");
    drop(listener);
    assert!(!output.status.success(), "expected non-zero exit");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("already in use"),
        "stderr should mention 'already in use', got: {stderr}"
    );
}
