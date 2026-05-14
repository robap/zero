//! Integration test: `GET /_zero/events` emits an initial `hello` event.

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

/// Read SSE bytes until we collect a complete frame (`\n\n` delimited) that
/// contains the given event name. Times out after `timeout`.
async fn read_until_event(
    stream: &mut (impl futures_util::Stream<Item = reqwest::Result<bytes::Bytes>> + Unpin),
    name: &str,
    timeout: Duration,
) -> Option<String> {
    use futures_util::StreamExt;
    let deadline = Instant::now() + timeout;
    let needle = format!("event: {name}");
    let mut buf = String::new();
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let chunk = tokio::time::timeout(remaining, stream.next())
            .await
            .ok()??;
        let chunk = chunk.ok()?;
        buf.push_str(std::str::from_utf8(&chunk).unwrap_or(""));
        if let Some(frame) = buf.split("\n\n").find(|f| f.contains(&needle)) {
            return Some(frame.to_string());
        }
    }
    None
}

#[test]
fn sse_endpoint_emits_hello_on_connect() {
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
        "dev server did not start"
    );

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async move {
        let client = reqwest::Client::new();
        let base = format!("http://127.0.0.1:{port}");

        let resp = client
            .get(format!("{base}/_zero/events"))
            .send()
            .await
            .unwrap();

        assert_eq!(resp.status(), 200);

        let ctype = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(
            ctype.contains("text/event-stream"),
            "expected text/event-stream, got: {ctype}"
        );

        let cc = resp
            .headers()
            .get("cache-control")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        assert!(
            cc.contains("no-store"),
            "expected no-store in cache-control, got: {cc}"
        );

        let mut stream = resp.bytes_stream();
        let frame = read_until_event(&mut stream, "hello", Duration::from_secs(3))
            .await
            .expect("hello event must arrive within 3 seconds");

        assert!(
            frame.contains("data: ok"),
            "hello frame must have data: ok; got: {frame:?}"
        );
    });
}
