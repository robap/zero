//! Render a `TomlInput` (a flat plain-data struct, decoupled from the
//! `zero init` wizard's `Answers`) into the text of a `zero.toml` file.
//! The output parses back through `Config::from_toml_str` to the same
//! effective settings.

/// Inputs for [`render_toml`]. Decoupled from `zero init`'s `Answers` so
/// `zero-config` does not depend on the `prompts` module in the binary
/// crate.
#[derive(Debug, Clone)]
pub struct TomlInput {
    /// Zero app subdirectory name (`[project] root`).
    pub root: String,
    /// Dev server port (`[dev] port`).
    pub port: u16,
    /// Optional backend proxy URL (`[dev] proxy`); `None` for static-SPA mode.
    pub proxy: Option<String>,
    /// Build output directory (`[build] out`).
    pub out: String,
}

/// Render `input` into the text of a `zero.toml` file.
///
/// # Parameters
/// - `input`: the plain-data settings to render.
///
/// # Returns
/// A string holding the TOML text.
pub fn render_toml(input: &TomlInput) -> String {
    let mut out = String::new();
    out.push_str("[project]\n");
    out.push_str(&format!("root = \"{}\"\n", input.root));
    out.push('\n');

    out.push_str("[dev]\n");
    out.push_str(&format!("port = {}\n", input.port));
    match &input.proxy {
        Some(p) if !p.is_empty() => out.push_str(&format!("proxy = \"{p}\"\n")),
        _ => out.push_str("# proxy = \"http://localhost:8080\"\n"),
    }
    out.push('\n');

    out.push_str("[build]\n");
    out.push_str(&format!("out = \"{}\"\n", input.out));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Config;

    #[test]
    fn rendered_toml_round_trips_with_proxy() {
        let input = TomlInput {
            root: "web".to_string(),
            port: 4000,
            proxy: Some("http://localhost:8080".to_string()),
            out: "build".to_string(),
        };
        let text = render_toml(&input);
        let cfg = Config::from_toml_str(&text).expect("should parse");
        assert_eq!(cfg.project.root, "web");
        assert_eq!(cfg.dev.port, 4000);
        assert_eq!(
            cfg.dev.proxy.as_ref().map(|u| u.as_str()),
            Some("http://localhost:8080/")
        );
        assert_eq!(cfg.build.out, "build");
    }

    #[test]
    fn rendered_toml_omits_proxy_when_none() {
        let input = TomlInput {
            root: "web".to_string(),
            port: 3000,
            proxy: None,
            out: "dist".to_string(),
        };
        let text = render_toml(&input);
        let cfg = Config::from_toml_str(&text).expect("should parse");
        assert!(cfg.dev.proxy.is_none());
    }
}
