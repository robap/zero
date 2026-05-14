//! Disk file serving for `zero dev` under `<root>/src`, `<root>/styles`,
//! `<root>/public`, and a handful of well-known root files.

use std::path::{Path, PathBuf};

use axum::body::Body;
use axum::http::StatusCode;
use axum::http::header;
use axum::response::{IntoResponse, Response};

/// Map a path's extension to a content-type string.
///
/// # Parameters
/// - `path`: the file path to inspect.
///
/// # Returns
/// A `'static` content-type string; `application/octet-stream` for unknown ext.
pub fn content_type_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("js") => "application/javascript; charset=utf-8",
        Some("mjs") => "application/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("ico") => "image/x-icon",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// Serve a file beneath `root`, given a URI path that begins with `prefix`.
///
/// # Parameters
/// - `root`: the subdirectory to serve from (e.g. `project_root/src`).
/// - `prefix`: leading URI segment (e.g. `"/src"`).
/// - `uri_path`: the full URI path of the incoming request.
///
/// # Returns
/// A `Response`: 200 with file body on success, 403 on traversal, 404 if missing.
pub async fn serve_under(root: PathBuf, prefix: &'static str, uri_path: &str) -> Response {
    // Reject any path component that is or contains `..` before normalization.
    if uri_path.split('/').any(|seg| seg == "..") {
        return (StatusCode::FORBIDDEN, "forbidden").into_response();
    }
    let Some(rest) = uri_path.strip_prefix(prefix) else {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    };
    let rel = rest.trim_start_matches('/');
    let candidate = root.join(rel);
    serve_file_within(&root, &candidate).await
}

/// Like `serve_under` but routes `.ts` requests through the TS transpiler.
///
/// # Parameters
/// - `root`: directory to serve from.
/// - `prefix`: leading URI segment used to strip the request path.
/// - `uri_path`: the full URI path of the incoming request.
/// - `inline_source_map`: whether to append an inline source map to TS responses.
///
/// # Returns
/// A `Response`. `.ts` files are transpiled before responding; all other
/// extensions follow the byte-pure `serve_under` path.
pub async fn serve_under_with_transpile(
    root: PathBuf,
    prefix: &'static str,
    uri_path: &str,
    inline_source_map: bool,
) -> Response {
    if uri_path.split('/').any(|seg| seg == "..") {
        return (StatusCode::FORBIDDEN, "forbidden").into_response();
    }
    let Some(rest) = uri_path.strip_prefix(prefix) else {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    };
    let rel = rest.trim_start_matches('/');
    let candidate = root.join(rel);

    let is_ts = candidate
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("ts"))
        .unwrap_or(false);

    if !is_ts {
        return serve_file_within(&root, &candidate).await;
    }

    // Path-traversal check before transpiling: ensure the resolved path is
    // beneath `root`. Reuse `serve_file_within`'s logic by first canonicalizing.
    let Ok(root_canon) = std::fs::canonicalize(&root) else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "root missing").into_response();
    };
    let canonical = match std::fs::canonicalize(&candidate) {
        Ok(c) => c,
        Err(_) => return (StatusCode::NOT_FOUND, "not found").into_response(),
    };
    if !canonical.starts_with(&root_canon) {
        return (StatusCode::FORBIDDEN, "forbidden").into_response();
    }

    crate::dev::transpile::serve_typescript_file(canonical, uri_path.to_string(), inline_source_map)
        .await
}

/// Serve a single well-known root-level file (e.g., `/favicon.ico`).
///
/// # Parameters
/// - `root`: canonicalized project root directory.
/// - `filename`: file name immediately under `root`.
///
/// # Returns
/// A `Response`: 200 with file body on success, 404 otherwise.
pub async fn serve_root_file(root: PathBuf, filename: &str) -> Response {
    let candidate = root.join(filename);
    serve_file_within(&root, &candidate).await
}

async fn serve_file_within(root: &Path, candidate: &Path) -> Response {
    let Ok(root_canon) = std::fs::canonicalize(root) else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "project root missing").into_response();
    };
    // Canonicalize the candidate. If that fails because the path doesn't
    // exist, walk up until we get a canonical parent and verify that. We
    // only forbid escape — non-existence is a normal 404.
    let canonical = std::fs::canonicalize(candidate).ok();
    if let Some(c) = &canonical {
        if !c.starts_with(&root_canon) {
            return (StatusCode::FORBIDDEN, "forbidden").into_response();
        }
    } else {
        // Walk parents to find one that exists, ensure it's still under root.
        let mut walk = candidate.parent();
        while let Some(p) = walk {
            if let Ok(c) = std::fs::canonicalize(p) {
                if !c.starts_with(&root_canon) {
                    return (StatusCode::FORBIDDEN, "forbidden").into_response();
                }
                break;
            }
            walk = p.parent();
        }
    }

    let path = match canonical {
        Some(p) => p,
        None => return (StatusCode::NOT_FOUND, "not found").into_response(),
    };

    if !path.is_file() {
        return (StatusCode::NOT_FOUND, "not found").into_response();
    }

    let bytes = match tokio::fs::read(&path).await {
        Ok(b) => b,
        Err(_) => return (StatusCode::NOT_FOUND, "not found").into_response(),
    };

    let ctype = content_type_for(&path);
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, ctype)],
        Body::from(bytes),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_type_js() {
        assert_eq!(
            content_type_for(Path::new("a.js")),
            "application/javascript; charset=utf-8"
        );
    }

    #[test]
    fn content_type_css() {
        assert_eq!(
            content_type_for(Path::new("styles/app.css")),
            "text/css; charset=utf-8"
        );
    }

    #[test]
    fn content_type_html_and_json() {
        assert_eq!(
            content_type_for(Path::new("index.html")),
            "text/html; charset=utf-8"
        );
        assert_eq!(content_type_for(Path::new("m.json")), "application/json");
    }

    #[test]
    fn content_type_default_is_octet_stream() {
        assert_eq!(
            content_type_for(Path::new("blob.unknownext")),
            "application/octet-stream"
        );
    }

    #[test]
    fn traversal_serve_under_returns_403() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("secret.txt"), "no").unwrap();
        // serve_under is called with the subdirectory as root (not the project root).
        let src_root = dir.path().join("src");

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let resp = rt
            .block_on(async { serve_under(src_root.clone(), "/src", "/src/../secret.txt").await });
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn missing_file_returns_404() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        let root = dir.path().to_path_buf();

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let resp =
            rt.block_on(async { serve_under(root.clone(), "/src", "/src/nonexistent.js").await });
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
