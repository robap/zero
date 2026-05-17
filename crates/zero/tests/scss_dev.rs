//! Integration tests for SCSS dev-server compilation.

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

fn write_scss_project(tmp: &std::path::Path, dev_sourcemap: Option<bool>) {
    let mut toml = String::from("[project]\nroot = \"web\"\n\n[dev]\nport = 3000\n");
    if let Some(v) = dev_sourcemap {
        toml.push_str(&format!("sourcemap = {v}\n"));
    }
    std::fs::write(tmp.join("zero.toml"), toml).unwrap();

    let web = tmp.join("web");
    std::fs::create_dir_all(web.join("styles")).unwrap();
    std::fs::create_dir_all(web.join("src")).unwrap();
    std::fs::write(
        web.join("index.html"),
        r#"<!doctype html><html><head><link rel="stylesheet" href="/styles/app.scss"></head><body><div id="app"></div></body></html>"#,
    )
    .unwrap();
    std::fs::write(web.join("src/app.ts"), "export const x = 1;\n").unwrap();
    std::fs::write(web.join("styles/_vars.scss"), "$c: red;").unwrap();
    std::fs::write(
        web.join("styles/app.scss"),
        "@use 'vars' as v; body { color: v.$c; }",
    )
    .unwrap();
    std::fs::write(web.join("styles/bad.scss"), "body { color: ; }").unwrap();
}

fn spawn_dev(tmp: &std::path::Path, port: u16) -> ChildGuard {
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
fn dev_scss_compiles_and_returns_css() {
    let tmp = tempfile::tempdir().unwrap();
    write_scss_project(tmp.path(), None);
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
            .get(format!("{base}/styles/app.scss"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200, "expected 200 for app.scss");
        let ctype = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        assert!(ctype.contains("text/css"), "wrong content-type: {ctype}");
        let body = resp.text().await.unwrap();
        assert!(body.contains("red"), "compiled CSS missing 'red': {body}");
        assert!(
            body.contains("/*# sourceMappingURL=data:application/json;base64,"),
            "inline sourcemap missing (default sourcemap=true): {body}"
        );
    });
}

#[test]
fn dev_scss_partial_returns_404() {
    let tmp = tempfile::tempdir().unwrap();
    write_scss_project(tmp.path(), None);
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
            .get(format!("http://127.0.0.1:{port}/styles/_vars.scss"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 404, "partial should be 404");
    });
}

#[test]
fn dev_scss_nonexistent_returns_404() {
    let tmp = tempfile::tempdir().unwrap();
    write_scss_project(tmp.path(), None);
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
            .get(format!("http://127.0.0.1:{port}/styles/nonexistent.scss"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 404, "missing file should be 404");
    });
}

#[test]
fn dev_scss_compile_error_returns_500() {
    let tmp = tempfile::tempdir().unwrap();
    write_scss_project(tmp.path(), None);
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
            .get(format!("http://127.0.0.1:{port}/styles/bad.scss"))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 500, "malformed scss should be 500");
        let ctype = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        assert!(
            ctype.contains("text/plain"),
            "error should be text/plain: {ctype}"
        );
        let body = resp.text().await.unwrap();
        assert!(
            body.contains("bad.scss"),
            "error body missing filename: {body}"
        );
    });
}

#[test]
fn dev_scss_omits_sourcemap_when_disabled() {
    let tmp = tempfile::tempdir().unwrap();
    write_scss_project(tmp.path(), Some(false));
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
            .get(format!("http://127.0.0.1:{port}/styles/app.scss"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(
            !body.contains("sourceMappingURL"),
            "sourcemap present when disabled: {body}"
        );
    });
}
