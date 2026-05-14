//! Render an `Answers` struct (collected from the `zero init` wizard) into
//! the text of a `zero.toml` file. The output parses back through
//! `Config::from_toml_str` to the same effective settings.

use crate::prompts::Answers;

/// Render the user's prompt answers into the text of a `zero.toml` file.
///
/// # Parameters
/// - `a`: the wizard's collected answers.
///
/// # Returns
/// A string holding the TOML text.
pub fn render_toml(a: &Answers) -> String {
    let mut out = String::new();
    out.push_str("[project]\n");
    out.push_str(&format!("root = \"{}\"\n", a.root));
    out.push('\n');

    out.push_str("[dev]\n");
    out.push_str(&format!("port = {}\n", a.port));
    match &a.proxy {
        Some(p) if !p.is_empty() => out.push_str(&format!("proxy = \"{p}\"\n")),
        _ => out.push_str("# proxy = \"http://localhost:8080\"\n"),
    }
    out.push('\n');

    out.push_str("[build]\n");
    out.push_str(&format!("out = \"{}\"\n", a.out));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn rendered_toml_round_trips_with_proxy() {
        let answers = Answers {
            root: "web".to_string(),
            port: 4000,
            proxy: Some("http://localhost:8080".to_string()),
            out: "build".to_string(),
        };
        let text = render_toml(&answers);
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
        let answers = Answers {
            root: "web".to_string(),
            port: 3000,
            proxy: None,
            out: "dist".to_string(),
        };
        let text = render_toml(&answers);
        let cfg = Config::from_toml_str(&text).expect("should parse");
        assert!(cfg.dev.proxy.is_none());
    }
}
