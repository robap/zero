//! Integration test: `zero dev` in the showcase serves the home route and
//! injects an importmap that resolves `"zero/components"` to the
//! `.zero/components/index.ts` URL.

use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

mod common;

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
fn showcase_dev_serves_home_and_components_index() {
    let tmp = common::prepare_showcase();
    let port = pick_free_port();

    // Override the showcase's dev port with the free one.
    let toml = tmp.path().join("zero.toml");
    let body = std::fs::read_to_string(&toml).unwrap();
    let rewritten = body
        .lines()
        .map(|l| {
            if l.starts_with("port") {
                format!("port = {port}")
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&toml, rewritten).unwrap();

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

        let resp = client.get(format!("{base}/")).send().await.unwrap();
        assert_eq!(resp.status(), 200);
        let body = resp.text().await.unwrap();
        assert!(
            body.contains(r#""zero/components":"/.zero/components/index.ts""#),
            "importmap missing zero/components entry: {body}"
        );

        let resp = client
            .get(format!("{base}/.zero/components/index.ts"))
            .send()
            .await
            .unwrap();
        assert_eq!(
            resp.status(),
            200,
            "components/index.ts must be reachable through dev server"
        );
        let body = resp.text().await.unwrap();
        assert!(
            body.contains("Avatar"),
            "components index body missing Avatar export: {body}"
        );
    });
}
