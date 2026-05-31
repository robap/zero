//! `zero.toml` parsing and validation.

use std::path::PathBuf;

use serde::Deserialize;

/// Top-level parsed and validated `zero.toml`.
#[derive(Debug)]
pub struct Config {
    /// `[project]` table.
    pub project: ProjectConfig,
    /// `[dev]` table (with defaults applied).
    pub dev: DevConfig,
    /// `[build]` table (with defaults applied).
    pub build: BuildConfig,
}

/// `[project]` section.
#[derive(Debug)]
pub struct ProjectConfig {
    /// The project root directory.
    pub root: String,
}

/// `[dev]` section.
#[derive(Debug)]
pub struct DevConfig {
    /// Port the dev server binds to.
    pub port: u16,
    /// Optional backend proxy URL.
    pub proxy: Option<url::Url>,
    /// Inline source maps in dev `.ts` responses. Default `true`.
    pub sourcemap: bool,
}

/// `[build]` section.
#[derive(Debug)]
pub struct BuildConfig {
    /// Output directory.
    pub out: String,
    /// Emit external `.map` files alongside the bundle. Default `false`.
    pub sourcemap: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    project: RawProject,
    #[serde(default)]
    dev: RawDev,
    #[serde(default)]
    build: RawBuild,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProject {
    root: String,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDev {
    port: Option<u16>,
    proxy: Option<String>,
    sourcemap: Option<bool>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBuild {
    out: Option<String>,
    sourcemap: Option<bool>,
}

impl Config {
    /// Parse and validate a TOML string into a `Config`.
    ///
    /// # Parameters
    /// - `s`: the TOML source text.
    ///
    /// # Returns
    /// A validated `Config`, or an error describing what was wrong.
    pub fn from_toml_str(s: &str) -> anyhow::Result<Config> {
        let raw: RawConfig = toml::from_str(s)?;
        validate_relative_path(&raw.project.root, "project.root")?;

        let port = raw.dev.port.unwrap_or(3000);
        if port == 0 {
            anyhow::bail!("dev.port must be in 1-65535");
        }

        let proxy = match raw.dev.proxy {
            Some(s) if s.is_empty() => None,
            Some(s) => {
                let url = url::Url::parse(&s)
                    .map_err(|e| anyhow::anyhow!("dev.proxy is not a valid URL: {e}"))?;
                if url.scheme() != "http" {
                    anyhow::bail!(
                        "dev.proxy must use http:// (HTTPS dev proxy is out of scope); got '{}'",
                        url.scheme()
                    );
                }
                Some(url)
            }
            None => None,
        };

        let out = raw.build.out.unwrap_or_else(|| "dist".to_string());
        validate_relative_path(&out, "build.out")?;

        let dev_sourcemap = raw.dev.sourcemap.unwrap_or(true);
        let build_sourcemap = raw.build.sourcemap.unwrap_or(false);

        Ok(Config {
            project: ProjectConfig {
                root: raw.project.root,
            },
            dev: DevConfig {
                port,
                proxy,
                sourcemap: dev_sourcemap,
            },
            build: BuildConfig {
                out,
                sourcemap: build_sourcemap,
            },
        })
    }

    /// Load `./zero.toml` from the current working directory and validate it.
    ///
    /// # Returns
    /// A validated `Config`, or an error if the file is missing, unreadable,
    /// or fails validation. The missing-file error string includes a hint
    /// telling the developer to run `zero init`.
    pub fn load_from_cwd() -> anyhow::Result<Config> {
        let cwd = std::env::current_dir()?;
        let path = cwd.join("zero.toml");
        let text = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                anyhow::bail!(
                    "zero.toml not found at {}; run `zero init` to create one",
                    cwd.display()
                );
            }
            Err(e) => {
                return Err(anyhow::anyhow!("failed to read {}: {e}", path.display()));
            }
        };
        Config::from_toml_str(&text)
    }

    /// Like [`Config::load_from_cwd`] but returns `Ok(None)` when `zero.toml`
    /// is absent, instead of erroring. A present-but-invalid file still
    /// returns `Err`. Used by `zero test`, which falls back to built-in
    /// defaults when no project config exists.
    ///
    /// # Returns
    /// `Ok(Some(config))` when a valid `zero.toml` exists, `Ok(None)` when it
    /// is absent, or `Err` when it is present but unreadable or invalid.
    pub fn load_from_cwd_optional() -> anyhow::Result<Option<Config>> {
        let cwd = std::env::current_dir()?;
        let path = cwd.join("zero.toml");
        match std::fs::read_to_string(&path) {
            Ok(text) => Ok(Some(Config::from_toml_str(&text)?)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(anyhow::anyhow!("failed to read {}: {e}", path.display())),
        }
    }

    /// Path to the project root subdirectory (`[project] root` joined to CWD).
    ///
    /// # Returns
    /// `PathBuf` to the directory the zero app lives in.
    pub fn project_root_path(&self) -> PathBuf {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(&self.project.root)
    }

    /// Path to the build output directory (`[build] out` joined to CWD).
    ///
    /// # Returns
    /// `PathBuf` to the directory `zero build` writes into.
    pub fn out_dir_path(&self) -> PathBuf {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(&self.build.out)
    }
}

/// Reject empty, absolute, parent-escaping, or backslash-containing relative paths.
///
/// # Parameters
/// - `value`: the path text to validate.
/// - `field`: dotted-name of the TOML field (for error messages).
///
/// # Returns
/// `Ok(())` if valid, an error otherwise.
fn validate_relative_path(value: &str, field: &str) -> anyhow::Result<()> {
    use std::path::{Component, Path};

    if value.is_empty() {
        anyhow::bail!("{field} must be a non-empty string");
    }
    if value.contains('\\') {
        anyhow::bail!("{field} must not contain '\\\\'; use forward slashes");
    }
    let path = Path::new(value);
    if path.has_root() {
        anyhow::bail!("{field} must be a relative path, not absolute");
    }
    for comp in path.components() {
        if matches!(comp, Component::ParentDir) {
            anyhow::bail!("{field} must not contain '..' segments");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_valid_toml() {
        let toml = r#"
[project]
root = "web"
"#;
        let cfg = Config::from_toml_str(toml).expect("should parse");
        assert_eq!(cfg.project.root, "web");
    }

    #[test]
    fn missing_project_root_is_an_error() {
        let toml = r#"
[project]
"#;
        let err = Config::from_toml_str(toml).expect_err("should fail");
        let msg = format!("{err}");
        assert!(
            msg.contains("root"),
            "error should mention `root`, got: {msg}"
        );
    }

    #[test]
    fn absolute_root_is_rejected() {
        let toml = r#"
[project]
root = "/etc"
"#;
        let err = Config::from_toml_str(toml).expect_err("should fail");
        let msg = format!("{err}");
        assert!(
            msg.to_lowercase().contains("absolute") || msg.to_lowercase().contains("relative"),
            "error should mention absolute/relative, got: {msg}"
        );
    }

    #[test]
    fn parent_dir_in_root_is_rejected() {
        let toml = r#"
[project]
root = "../escape"
"#;
        let err = Config::from_toml_str(toml).expect_err("should fail");
        let msg = format!("{err}");
        assert!(
            msg.to_lowercase().contains("root"),
            "error should mention root, got: {msg}"
        );
    }

    #[test]
    fn defaults_when_dev_and_build_sections_missing() {
        let toml = r#"
[project]
root = "web"
"#;
        let cfg = Config::from_toml_str(toml).expect("should parse");
        assert_eq!(cfg.dev.port, 3000);
        assert!(cfg.dev.proxy.is_none());
        assert_eq!(cfg.build.out, "dist");
    }

    #[test]
    fn port_zero_is_rejected() {
        let toml = r#"
[project]
root = "web"

[dev]
port = 0
"#;
        let err = Config::from_toml_str(toml).expect_err("should fail");
        let msg = format!("{err}");
        assert!(
            msg.contains("port"),
            "error should mention port, got: {msg}"
        );
    }

    #[test]
    fn https_proxy_is_rejected() {
        let toml = r#"
[project]
root = "web"

[dev]
proxy = "https://example.com"
"#;
        let err = Config::from_toml_str(toml).expect_err("should fail");
        let msg = format!("{err}");
        assert!(
            msg.contains("http"),
            "error should mention http scheme, got: {msg}"
        );
    }

    #[test]
    fn unknown_top_level_section_is_rejected() {
        let toml = r#"
[project]
root = "web"

[server]
host = "0.0.0.0"
"#;
        let err = Config::from_toml_str(toml).expect_err("should fail");
        let msg = format!("{err}");
        assert!(
            msg.to_lowercase().contains("unknown") || msg.contains("server"),
            "error should mention unknown field, got: {msg}"
        );
    }

    #[test]
    fn unknown_dev_key_is_rejected() {
        let toml = r#"
[project]
root = "web"

[dev]
host = "0.0.0.0"
"#;
        let err = Config::from_toml_str(toml).expect_err("should fail");
        let msg = format!("{err}");
        assert!(
            msg.to_lowercase().contains("unknown") || msg.contains("host"),
            "error should mention unknown field, got: {msg}"
        );
    }

    #[test]
    fn defaults_sourcemap_dev_true_build_false() {
        let toml = r#"
[project]
root = "web"
"#;
        let cfg = Config::from_toml_str(toml).expect("should parse");
        assert!(cfg.dev.sourcemap);
        assert!(!cfg.build.sourcemap);
    }

    #[test]
    fn explicit_dev_sourcemap_false_is_honored() {
        let toml = r#"
[project]
root = "web"

[dev]
sourcemap = false
"#;
        let cfg = Config::from_toml_str(toml).expect("should parse");
        assert!(!cfg.dev.sourcemap);
    }

    #[test]
    fn explicit_build_sourcemap_true_is_honored() {
        let toml = r#"
[project]
root = "web"

[build]
sourcemap = true
"#;
        let cfg = Config::from_toml_str(toml).expect("should parse");
        assert!(cfg.build.sourcemap);
    }

    #[test]
    fn non_boolean_sourcemap_is_rejected() {
        let toml = r#"
[project]
root = "web"

[dev]
sourcemap = "yes"
"#;
        let err = Config::from_toml_str(toml).expect_err("should fail");
        let msg = format!("{err}");
        assert!(
            msg.to_lowercase().contains("bool") || msg.to_lowercase().contains("sourcemap"),
            "error should mention bool/sourcemap, got: {msg}"
        );
    }

    /// Restores the previous CWD on drop so the change can't leak into other
    /// tests in this crate.
    struct CwdGuard {
        prev: std::path::PathBuf,
    }
    impl CwdGuard {
        fn enter(target: &std::path::Path) -> Self {
            let prev = std::env::current_dir().unwrap();
            std::env::set_current_dir(target).unwrap();
            CwdGuard { prev }
        }
    }
    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.prev);
        }
    }

    #[test]
    fn load_from_cwd_optional_covers_absent_valid_and_invalid() {
        let tmp = tempfile::tempdir().unwrap();
        let _g = CwdGuard::enter(tmp.path());

        // Absent zero.toml → Ok(None).
        let absent = Config::load_from_cwd_optional().expect("absent should not error");
        assert!(absent.is_none(), "absent zero.toml should yield None");

        // Valid zero.toml → Ok(Some(cfg)) with expected project.root.
        std::fs::write(tmp.path().join("zero.toml"), "[project]\nroot = \"web\"\n").unwrap();
        let present = Config::load_from_cwd_optional().expect("valid should parse");
        let cfg = present.expect("valid zero.toml should yield Some");
        assert_eq!(cfg.project.root, "web");

        // Present-but-invalid (unknown section) → Err.
        std::fs::write(
            tmp.path().join("zero.toml"),
            "[project]\nroot = \"web\"\n\n[server]\nhost = \"0.0.0.0\"\n",
        )
        .unwrap();
        assert!(
            Config::load_from_cwd_optional().is_err(),
            "present-but-invalid zero.toml should error"
        );
    }

    #[test]
    fn http_proxy_is_accepted() {
        let toml = r#"
[project]
root = "web"

[dev]
proxy = "http://localhost:8080"
"#;
        let cfg = Config::from_toml_str(toml).expect("should parse");
        let proxy = cfg.dev.proxy.expect("proxy should be set");
        assert_eq!(proxy.scheme(), "http");
        assert_eq!(proxy.host_str(), Some("localhost"));
    }
}
