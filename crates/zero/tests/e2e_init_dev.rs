//! End-to-end test: `zero init` + `zero dev` full developer flow.

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
#[ignore = "slow"]
fn init_then_dev_serves_all_expected_urls() {
    let tmp = tempfile::tempdir().unwrap();
    let port = pick_free_port();

    // Step 1–2: write a zero.toml and run `zero init`.
    std::fs::write(
        tmp.path().join("zero.toml"),
        format!("[project]\nroot = \"web\"\n\n[dev]\nport = {port}\n"),
    )
    .unwrap();
    assert_cmd::Command::cargo_bin("zero")
        .unwrap()
        .arg("init")
        .arg("--yes")
        .current_dir(tmp.path())
        .assert()
        .success();

    // Assert scaffold files are present.
    assert!(tmp.path().join("web/index.html").exists());
    assert!(tmp.path().join("web/src/app.ts").exists());
    assert!(tmp.path().join("web/src/routes/home.ts").exists());
    assert!(tmp.path().join("web/styles/app.scss").exists());

    // Step 3: start `zero dev`.
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
        "dev server did not start"
    );

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async move {
        let client = reqwest::Client::new();
        let base = format!("http://127.0.0.1:{port}");

        // GET / → index.html with injected scripts.
        let resp = client.get(format!("{base}/")).send().await.unwrap();
        assert_eq!(resp.status(), 200, "GET / should be 200");
        let body = resp.text().await.unwrap();
        assert!(
            body.contains(r#"<script type="importmap">"#),
            "importmap must be injected"
        );
        assert!(
            body.contains(r#"<script type="module" src="/src/app.ts">"#),
            "module script must be injected"
        );

        // GET /zero.js → runtime (length > 1000).
        let resp = client.get(format!("{base}/zero.js")).send().await.unwrap();
        assert_eq!(resp.status(), 200, "GET /zero.js should be 200");
        let body = resp.text().await.unwrap();
        assert!(body.len() > 1000, "runtime should be > 1000 bytes");
        assert!(body.contains("function signal("));

        // GET /src/app.ts → transpiled JS body.
        let resp = client
            .get(format!("{base}/src/app.ts"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200, "GET /src/app.ts should be 200");
        let body = resp.text().await.unwrap();
        assert!(
            body.contains("import { App, signal } from"),
            "transpiled body missing imports: {body}"
        );

        // GET /styles/app.scss → compiled text/css + no-cache.
        let resp = client
            .get(format!("{base}/styles/app.scss"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200, "GET /styles/app.scss should be 200");
        let ctype = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(ctype.starts_with("text/css"));
        let cc = resp
            .headers()
            .get("cache-control")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert_eq!(cc, "no-store, no-cache, must-revalidate, max-age=0");
    });
}
