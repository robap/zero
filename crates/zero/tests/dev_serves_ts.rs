//! Integration test: `zero dev` transpiles `/src/*.ts` on the fly.

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

fn write_basic_project(tmp: &std::path::Path, dev_sourcemap: Option<bool>) -> std::path::PathBuf {
    let mut toml = String::from("[project]\nroot = \"web\"\n\n[dev]\nport = ");
    // port set by caller (we just want a placeholder; replaced below)
    toml.push_str("3000\n");
    if let Some(v) = dev_sourcemap {
        toml.push_str(&format!("sourcemap = {v}\n"));
    }
    std::fs::write(tmp.join("zero.toml"), toml).unwrap();

    let web = tmp.join("web");
    std::fs::create_dir_all(web.join("src")).unwrap();
    std::fs::write(
        web.join("index.html"),
        "<!doctype html><html><head><title>x</title></head><body><div id=app></div></body></html>",
    )
    .unwrap();
    std::fs::write(
        web.join("src/app.ts"),
        "const x: number = 1; export { x };\n",
    )
    .unwrap();
    std::fs::write(web.join("src/bad.ts"), "const x: = ;\n").unwrap();
    std::fs::write(web.join("src/plain.js"), "export const y = 2;\n").unwrap();
    web
}

fn spawn_dev(tmp: &std::path::Path, port: u16) -> ChildGuard {
    // Patch zero.toml with the chosen port (we wrote 3000 above).
    let p = tmp.join("zero.toml");
    let s = std::fs::read_to_string(&p).unwrap();
    std::fs::write(&p, s.replace("port = 3000", &format!("port = {port}"))).unwrap();

    let bin = assert_cmd::cargo::cargo_bin("zero");
    let child = Command::new(&bin)
        .arg("dev")
        .current_dir(tmp)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    ChildGuard(child)
}

#[test]
fn dev_transpiles_ts_with_inline_source_map_by_default() {
    let tmp = tempfile::tempdir().unwrap();
    write_basic_project(tmp.path(), None);
    let port = pick_free_port();
    let _guard = spawn_dev(tmp.path(), port);

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
        let resp = client
            .get(format!("{base}/src/app.ts"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        let ctype = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        assert!(
            ctype.starts_with("application/javascript"),
            "wrong content-type: {ctype}"
        );
        let body = resp.text().await.unwrap();
        assert!(body.contains("const x = 1"), "types not stripped: {body}");
        assert!(!body.contains(": number"), "type leaked: {body}");
        assert!(
            body.contains("//# sourceMappingURL=data:application/json;base64,"),
            "missing inline source map: {body}"
        );
    });
}

#[test]
fn dev_transpiles_ts_without_source_map_when_disabled() {
    let tmp = tempfile::tempdir().unwrap();
    write_basic_project(tmp.path(), Some(false));
    let port = pick_free_port();
    let _guard = spawn_dev(tmp.path(), port);
    assert!(wait_for_port(port, Duration::from_secs(5)));

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async move {
        let client = reqwest::Client::new();
        let body = client
            .get(format!("http://127.0.0.1:{port}/src/app.ts"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(
            !body.contains("//# sourceMappingURL="),
            "source map should be absent: {body}"
        );
    });
}

#[test]
fn dev_transpile_error_returns_500_with_location() {
    let tmp = tempfile::tempdir().unwrap();
    write_basic_project(tmp.path(), None);
    let port = pick_free_port();
    let _guard = spawn_dev(tmp.path(), port);
    assert!(wait_for_port(port, Duration::from_secs(5)));

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async move {
        let client = reqwest::Client::new();
        let resp = client
            .get(format!("http://127.0.0.1:{port}/src/bad.ts"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 500);
        let body = resp.text().await.unwrap();
        assert!(body.contains("transpile error"), "body: {body}");
    });
}

#[test]
fn dev_serves_js_files_unchanged() {
    let tmp = tempfile::tempdir().unwrap();
    write_basic_project(tmp.path(), None);
    let port = pick_free_port();
    let _guard = spawn_dev(tmp.path(), port);
    assert!(wait_for_port(port, Duration::from_secs(5)));

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async move {
        let client = reqwest::Client::new();
        let body = client
            .get(format!("http://127.0.0.1:{port}/src/plain.js"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert_eq!(body, "export const y = 2;\n");
    });
}
