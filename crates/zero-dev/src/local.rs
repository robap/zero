//! Serve the local `<root>/index.html` with scripts injected (no-proxy mode).

use std::path::PathBuf;

use axum::http::StatusCode;
use axum::http::header;
use axum::response::{IntoResponse, Response};

use crate::inject::inject;

/// Read `root/index.html`, inject dev scripts pointing at whichever bootstrap
/// entry actually exists in the project, and return as an HTML response.
///
/// Probes for `src/app.ts` first; falls back to `src/app.js`.
///
/// # Parameters
/// - `root`: canonicalized path to the project root directory.
///
/// # Returns
/// A 200 HTML response with scripts injected, or 500 if `index.html` is missing.
pub async fn serve_local_index(root: PathBuf) -> Response {
    let index_path = root.join("index.html");
    let raw = match tokio::fs::read(&index_path).await {
        Ok(b) => b,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!(
                    "zero dev: {}/index.html not found; run `zero init` first",
                    root.display()
                ),
            )
                .into_response();
        }
    };

    let app_entry_href = if root.join("src").join("app.ts").is_file() {
        "/src/app.ts"
    } else {
        "/src/app.js"
    };

    let body = inject(&raw, app_entry_href);
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        body,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;

    async fn read_body(resp: Response) -> (StatusCode, String) {
        let status = resp.status();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8_lossy(&bytes).to_string())
    }

    #[tokio::test]
    async fn missing_index_html_returns_500() {
        let tmp = tempfile::tempdir().unwrap();
        let resp = serve_local_index(tmp.path().to_path_buf()).await;
        let (status, body) = read_body(resp).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(
            body.contains("index.html not found"),
            "body should mention missing index.html: {body}"
        );
    }

    #[tokio::test]
    async fn index_html_with_app_ts_injects_ts_entry() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("index.html"),
            "<!doctype html><html><head><title>X</title></head><body></body></html>",
        )
        .unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        std::fs::write(tmp.path().join("src").join("app.ts"), "// hi").unwrap();
        let resp = serve_local_index(tmp.path().to_path_buf()).await;
        let (status, body) = read_body(resp).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("/src/app.ts"), "body: {body}");
    }

    #[tokio::test]
    async fn index_html_without_app_ts_falls_back_to_app_js() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("index.html"),
            "<!doctype html><html><head><title>X</title></head><body></body></html>",
        )
        .unwrap();
        let resp = serve_local_index(tmp.path().to_path_buf()).await;
        let (status, body) = read_body(resp).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("/src/app.js"), "body: {body}");
        assert!(!body.contains("/src/app.ts"), "body: {body}");
    }

    #[tokio::test]
    async fn ok_response_has_html_content_type() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("index.html"),
            "<!doctype html><html><head></head><body></body></html>",
        )
        .unwrap();
        let resp = serve_local_index(tmp.path().to_path_buf()).await;
        let ct = resp.headers().get(header::CONTENT_TYPE).unwrap();
        assert_eq!(ct.to_str().unwrap(), "text/html; charset=utf-8");
    }
}
