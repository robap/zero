//! Integration test: `zero dev` writes `.zero/zero.d.ts` and
//! `.zero/zero-test.d.ts` into the project root every time it starts.

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
fn dev_writes_dts_files_on_startup() {
    let tmp = tempfile::tempdir().unwrap();
    let port = pick_free_port();
    std::fs::write(
        tmp.path().join("zero.toml"),
        format!("[project]\nroot = \"web\"\n\n[dev]\nport = {port}\n"),
    )
    .unwrap();
    let web = tmp.path().join("web");
    std::fs::create_dir_all(web.join("src")).unwrap();
    std::fs::write(
        web.join("index.html"),
        "<!doctype html><html><head><title>x</title></head><body><div id=app></div></body></html>",
    )
    .unwrap();
    std::fs::write(web.join("src/app.js"), "console.log('hi');\n").unwrap();

    let bin = assert_cmd::cargo::cargo_bin("zero");
    let child = Command::new(&bin)
        .arg("dev")
        .current_dir(tmp.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let _guard = ChildGuard(child);

    assert!(wait_for_port(port, Duration::from_secs(5)));

    let zero_dts = web.join(".zero/zero.d.ts");
    let zero_test_dts = web.join(".zero/zero-test.d.ts");
    assert!(zero_dts.is_file(), ".zero/zero.d.ts should exist");
    assert!(zero_test_dts.is_file(), ".zero/zero-test.d.ts should exist");
    let dts = std::fs::read_to_string(&zero_dts).unwrap();
    assert!(dts.contains(r#"declare module "zero""#));
    let test_dts = std::fs::read_to_string(&zero_test_dts).unwrap();
    assert!(test_dts.contains(r#"declare module "zero/test""#));
}
