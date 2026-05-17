//! Dev-server endpoint that compiles `.scss` files on the fly.

use std::path::PathBuf;

use axum::body::Body;
use axum::http::StatusCode;
use axum::http::header;
use axum::response::{IntoResponse, Response};

use crate::sass::{SassOptions, compile_scss};

/// Read `abs_path`, run it through grass, and return CSS.
///
/// # Parameters
/// - `abs_path`: absolute on-disk location of the source file.
/// - `logical_path`: URL-like path used in error messages and source-map sources.
/// - `inline_source_map`: append a base64 inline source map to the CSS body.
///
/// # Returns
/// 200 with `text/css` on success; 500 with plain-text error body on compile failure;
/// 404 if the file cannot be read.
pub async fn serve_scss_file(
    abs_path: PathBuf,
    logical_path: String,
    inline_source_map: bool,
) -> Response {
    let source = match tokio::fs::read_to_string(&abs_path).await {
        Ok(s) => s,
        Err(_) => return (StatusCode::NOT_FOUND, "not found").into_response(),
    };
    match compile_scss(
        &source,
        &abs_path,
        &SassOptions {
            filename: &logical_path,
            inline_source_map,
            emit_source_map: false,
            load_paths: &[],
        },
    ) {
        Ok(out) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
            Body::from(out.code),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            format!(
                "zero dev: scss error\n  {}:{}:{}\n  {}",
                e.file, e.line, e.column, e.message
            ),
        )
            .into_response(),
    }
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
    async fn missing_file_returns_404() {
        let resp = serve_scss_file(
            PathBuf::from("/nonexistent/__missing__.scss"),
            "/styles/missing.scss".to_string(),
            false,
        )
        .await;
        let (status, _) = read_body(resp).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn valid_scss_compiles_to_css() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("a.scss");
        std::fs::write(&p, "$c: red; .a { color: $c; }").unwrap();
        let resp = serve_scss_file(p, "/styles/a.scss".to_string(), false).await;
        let (status, body) = read_body(resp).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains(".a"), "body: {body}");
        assert!(body.contains("red"), "body: {body}");
    }

    #[tokio::test]
    async fn ok_response_has_text_css_content_type() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("a.scss");
        std::fs::write(&p, ".a { color: red; }").unwrap();
        let resp = serve_scss_file(p, "/styles/a.scss".to_string(), false).await;
        let ct = resp.headers().get(header::CONTENT_TYPE).unwrap();
        assert_eq!(ct.to_str().unwrap(), "text/css; charset=utf-8");
    }

    #[tokio::test]
    async fn scss_syntax_error_returns_500() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("bad.scss");
        std::fs::write(&p, ".a { color: ; }").unwrap();
        let resp = serve_scss_file(p, "/styles/bad.scss".to_string(), false).await;
        let (status, body) = read_body(resp).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(body.contains("scss error"), "body: {body}");
    }
}
