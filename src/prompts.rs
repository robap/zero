//! Interactive prompts for `zero init`.

use dialoguer::Input;
use dialoguer::theme::ColorfulTheme;

/// Settings gathered by the `zero init` wizard.
#[derive(Debug, Clone)]
pub struct Answers {
    /// Zero app subdirectory name (`[project] root`).
    pub root: String,
    /// Dev server port (`[dev] port`).
    pub port: u16,
    /// Optional backend proxy URL (`[dev] proxy`); `None` for static-SPA mode.
    pub proxy: Option<String>,
    /// Build output directory (`[build] out`).
    pub out: String,
}

/// Run the interactive wizard, returning the collected `Answers`.
///
/// # Returns
/// The user's answers, with each entry validated inline.
pub fn prompt_user() -> anyhow::Result<Answers> {
    let theme = ColorfulTheme::default();

    let root: String = Input::with_theme(&theme)
        .with_prompt("Zero app folder name")
        .default("web".to_string())
        .validate_with(|s: &String| validate_path_segment(s).map_err(|e| e.to_string()))
        .interact_text()?;

    let port: u16 = Input::with_theme(&theme)
        .with_prompt("Dev server port")
        .default(3000u16)
        .validate_with(|p: &u16| {
            if *p == 0 {
                Err("port must be in 1-65535".to_string())
            } else {
                Ok(())
            }
        })
        .interact_text()?;

    let proxy_raw: String = Input::with_theme(&theme)
        .with_prompt("Backend proxy URL (leave blank for static SPA mode)")
        .default(String::new())
        .allow_empty(true)
        .validate_with(|s: &String| {
            if s.is_empty() {
                return Ok(());
            }
            let url = url::Url::parse(s).map_err(|e| format!("not a valid URL: {e}"))?;
            if url.scheme() != "http" {
                return Err(
                    "proxy URL must use http:// (HTTPS dev proxy is out of scope)".to_string(),
                );
            }
            Ok(())
        })
        .interact_text()?;
    let proxy = if proxy_raw.is_empty() {
        None
    } else {
        Some(proxy_raw)
    };

    let out: String = Input::with_theme(&theme)
        .with_prompt("Build output folder")
        .default("dist".to_string())
        .validate_with(|s: &String| validate_path_segment(s).map_err(|e| e.to_string()))
        .interact_text()?;

    Ok(Answers {
        root,
        port,
        proxy,
        out,
    })
}

/// Reject path segments that contain separators or escape constructs.
///
/// # Parameters
/// - `s`: the candidate folder name.
///
/// # Returns
/// `Ok(())` if valid; an error describing the violation otherwise.
fn validate_path_segment(s: &str) -> anyhow::Result<()> {
    if s.is_empty() {
        anyhow::bail!("must not be empty");
    }
    if s.contains('/') || s.contains('\\') {
        anyhow::bail!("must not contain path separators");
    }
    if s == ".." || s.starts_with("../") {
        anyhow::bail!("must not contain '..'");
    }
    if s.starts_with('.') {
        anyhow::bail!("must not start with '.'");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_path_segment_accepts_web() {
        validate_path_segment("web").unwrap();
    }

    #[test]
    fn validate_path_segment_rejects_empty() {
        assert!(validate_path_segment("").is_err());
    }

    #[test]
    fn validate_path_segment_rejects_slashes() {
        assert!(validate_path_segment("a/b").is_err());
        assert!(validate_path_segment("a\\b").is_err());
    }

    #[test]
    fn validate_path_segment_rejects_dotdot() {
        assert!(validate_path_segment("..").is_err());
    }

    #[test]
    fn validate_path_segment_rejects_leading_dot() {
        assert!(validate_path_segment(".hidden").is_err());
    }
}
