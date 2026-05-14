//! Integration test: `zero dev` serves files from the project root directory.

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
fn serves_src_and_styles_with_correct_types_and_no_cache() {
    let tmp = tempfile::tempdir().unwrap();
    let port = pick_free_port();

    // Scaffold via `zero init` using an existing toml.
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

        // /src/app.ts is transpiled to JS on the fly.
        let resp = client
            .get(format!("{base}/src/app.ts"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200, "/src/app.ts should be 200");
        let ctype = header_str(&resp, "content-type");
        assert!(
            ctype.starts_with("application/javascript"),
            "wrong content-type for /src/app.ts: {ctype}"
        );
        assert_no_cache(&resp);
        let body = resp.text().await.unwrap();
        assert!(
            body.contains("import { App, signal } from"),
            "transpiled body missing imports: {body}"
        );

        // /styles/app.scss compiles to CSS on the fly.
        let resp = client
            .get(format!("{base}/styles/app.scss"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200, "/styles/app.scss should be 200");
        let ctype = header_str(&resp, "content-type");
        assert!(
            ctype.starts_with("text/css"),
            "wrong content-type for /styles/app.scss: {ctype}"
        );
        assert_no_cache(&resp);

        // Traversal attempt: reqwest normalizes `..` away, so the request
        // arrives as `/etc/passwd`, which doesn't match /src/*, and now hits
        // the SPA fallback (returns HTML index). Sensitive file is NOT served.
        // Raw `..` traversal protection (403) is covered by files.rs unit tests.
        let resp = client
            .get(format!("{base}/src/../../etc/passwd"))
            .send()
            .await
            .unwrap();
        let body = resp.text().await.unwrap();
        assert!(
            !body.contains("root:"),
            "response must not contain /etc/passwd contents"
        );

        // missing file → 404
        let resp = client
            .get(format!("{base}/src/nonexistent.js"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 404, "missing file should be 404");
    });
}

fn header_str(resp: &reqwest::Response, name: &str) -> String {
    resp.headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string()
}

fn assert_no_cache(resp: &reqwest::Response) {
    let cc = header_str(resp, "cache-control");
    assert_eq!(
        cc, "no-store, no-cache, must-revalidate, max-age=0",
        "wrong cache-control"
    );
}
