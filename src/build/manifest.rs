//! Build manifest (`manifest.json`) serialization.

use std::collections::BTreeMap;
use std::path::Path;

/// Write a `manifest.json` from a list of `(logical-name, output-path)` pairs.
///
/// # Parameters
/// - `out`: build output directory.
/// - `entries`: list of `(source_relative, output_relative)` pairs in insertion order.
///
/// # Returns
/// `Ok(())` on success.
pub fn write(out: &Path, entries: &[(String, String)]) -> anyhow::Result<()> {
    let map: BTreeMap<&str, &str> = entries
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let json = serde_json::to_string_pretty(&map)?;
    std::fs::write(out.join("manifest.json"), json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn manifest_round_trips() {
        let out = tempdir().unwrap();
        let entries = vec![
            ("app.js".to_string(), "assets/app.abc12345.js".to_string()),
            (
                "styles/app.css".to_string(),
                "assets/app.5e8d9f01.css".to_string(),
            ),
        ];
        write(out.path(), &entries).unwrap();
        let text = std::fs::read_to_string(out.path().join("manifest.json")).unwrap();
        let parsed: BTreeMap<String, String> = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed["app.js"], "assets/app.abc12345.js");
        assert_eq!(parsed["styles/app.css"], "assets/app.5e8d9f01.css");
    }

    #[test]
    fn manifest_keys_are_sorted() {
        let out = tempdir().unwrap();
        let entries = vec![
            ("styles/app.css".to_string(), "assets/s.css".to_string()),
            ("app.js".to_string(), "assets/a.js".to_string()),
        ];
        write(out.path(), &entries).unwrap();
        let text = std::fs::read_to_string(out.path().join("manifest.json")).unwrap();
        let app_pos = text.find("app.js").unwrap();
        let css_pos = text.find("styles/app.css").unwrap();
        assert!(app_pos < css_pos, "keys should be alphabetically sorted");
    }
}
