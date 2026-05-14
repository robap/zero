//! Module specifier resolution for the bundler.

use std::path::{Path, PathBuf};

/// A resolved module identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ModuleId {
    /// The embedded zero runtime.
    Runtime,
    /// A user module at the given path, relative to the project root.
    User(PathBuf),
}

/// Resolve an import specifier relative to `importer_dir` within `root`.
///
/// - `"zero"` → `ModuleId::Runtime`
/// - `"./..."` or `"../..."` → `ModuleId::User` with a root-relative path
/// - anything else → error
///
/// # Parameters
/// - `specifier`: the import specifier string.
/// - `importer_dir`: directory containing the importing module.
/// - `root`: project root directory (used for path-escape checking).
///
/// # Returns
/// A `ModuleId` or an error if the specifier is unsupported or escapes the root.
pub fn resolve(specifier: &str, importer_dir: &Path, root: &Path) -> anyhow::Result<ModuleId> {
    if specifier == "zero" {
        return Ok(ModuleId::Runtime);
    }
    if specifier.starts_with("./") || specifier.starts_with("../") {
        let abs = importer_dir.join(specifier);
        let canonical = abs
            .canonicalize()
            .map_err(|e| anyhow::anyhow!("cannot resolve '{specifier}': {e}"))?;
        let root_canon = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        if !canonical.starts_with(&root_canon) {
            anyhow::bail!(
                "import specifier '{}' escapes project root; bundler refuses to follow",
                specifier
            );
        }
        let rel = canonical
            .strip_prefix(&root_canon)
            .map(|p| PathBuf::from("./").join(p))
            .unwrap_or(canonical);
        return Ok(ModuleId::User(rel));
    }
    anyhow::bail!(
        "unsupported import specifier '{}'; expected 'zero' or a relative path",
        specifier
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn zero_resolves_to_runtime() {
        let dir = tempdir().unwrap();
        let r = resolve("zero", dir.path(), dir.path()).unwrap();
        assert_eq!(r, ModuleId::Runtime);
    }

    #[test]
    fn relative_resolves_to_user() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/home.js"), "").unwrap();
        let importer_dir = dir.path().join("src");
        let r = resolve("./home.js", &importer_dir, dir.path()).unwrap();
        assert_eq!(r, ModuleId::User(PathBuf::from("./src/home.js")));
    }

    #[test]
    fn absolute_specifier_is_rejected() {
        let dir = tempdir().unwrap();
        let err = resolve("/absolute", dir.path(), dir.path()).unwrap_err();
        assert!(err.to_string().contains("unsupported"));
    }

    #[test]
    fn bare_specifier_is_rejected() {
        let dir = tempdir().unwrap();
        let err = resolve("lodash", dir.path(), dir.path()).unwrap_err();
        assert!(err.to_string().contains("unsupported"));
    }

    #[test]
    fn parent_escape_is_rejected() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        let root = dir.path().join("src");
        let importer_dir = root.clone();
        let secret = dir.path().join("secret.txt");
        std::fs::write(&secret, "").unwrap();
        let err = resolve("../secret.txt", &importer_dir, &root).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("escapes") || msg.contains("cannot resolve"),
            "unexpected error: {msg}"
        );
    }
}
