use std::env;
use std::sync::Mutex;
use tempfile::tempdir;

static CWD_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn load_from_cwd_missing_toml_includes_hint() {
    let _guard = CWD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempdir().unwrap();
    let prev = env::current_dir().unwrap();
    env::set_current_dir(dir.path()).unwrap();
    let err = zero::config::Config::load_from_cwd().expect_err("should fail");
    env::set_current_dir(prev).unwrap();
    let msg = format!("{err}");
    assert!(
        msg.contains("zero.toml not found"),
        "msg should mention zero.toml not found, got: {msg}"
    );
    assert!(
        msg.contains("zero init"),
        "msg should hint at `zero init`, got: {msg}"
    );
}

#[test]
fn load_from_cwd_returns_config_when_file_exists() {
    let _guard = CWD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("zero.toml"), "[project]\nroot = \"web\"\n").unwrap();
    let prev = env::current_dir().unwrap();
    env::set_current_dir(dir.path()).unwrap();
    let cfg = zero::config::Config::load_from_cwd().expect("should load");
    env::set_current_dir(prev).unwrap();
    assert_eq!(cfg.project.root, "web");
}
