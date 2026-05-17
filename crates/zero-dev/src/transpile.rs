//! Dev-server endpoint that transpiles `.ts` files on the fly.

use std::path::PathBuf;

use axum::body::Body;
use axum::http::StatusCode;
use axum::http::header;
use axum::response::{IntoResponse, Response};

use zero_transpile::{TranspileOptions, transpile_typescript};

/// Read `abs_path`, run it through the swc TS transpiler, and return JS.
///
/// # Parameters
/// - `abs_path`: absolute on-disk location of the source file.
/// - `logical_path`: URL-like path used in error messages and source-map sources.
/// - `inline_source_map`: append a base64 inline source map to the JS body.
///
/// # Returns
/// 200 with `application/javascript` on success; 500 with a plain-text error body
/// on transpile failure; 404 if the file cannot be read.
pub async fn serve_typescript_file(
    abs_path: PathBuf,
    logical_path: String,
    inline_source_map: bool,
) -> Response {
    let source = match tokio::fs::read_to_string(&abs_path).await {
        Ok(s) => s,
        Err(_) => return (StatusCode::NOT_FOUND, "not found").into_response(),
    };

    match transpile_typescript(
        &source,
        &TranspileOptions {
            filename: &logical_path,
            inline_source_map,
            emit_source_map: false,
        },
    ) {
        Ok(out) => (
            StatusCode::OK,
            [(
                header::CONTENT_TYPE,
                "application/javascript; charset=utf-8",
            )],
            Body::from(out.code),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            format!(
                "zero dev: transpile error\n  {}:{}:{}\n  {}",
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
        let resp = serve_typescript_file(
            PathBuf::from("/nonexistent/__missing__.ts"),
            "/src/missing.ts".to_string(),
            true,
        )
        .await;
        let (status, _) = read_body(resp).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn valid_ts_with_inline_source_map_emits_sourcemap_comment() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("a.ts");
        std::fs::write(&p, "const x: number = 1; export { x };").unwrap();
        let resp = serve_typescript_file(p, "/src/a.ts".to_string(), true).await;
        let (status, body) = read_body(resp).await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            body.contains("sourceMappingURL"),
            "expected inline source map: {body}"
        );
    }

    #[tokio::test]
    async fn valid_ts_without_inline_source_map_omits_sourcemap_comment() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("a.ts");
        std::fs::write(&p, "const x: number = 1; export { x };").unwrap();
        let resp = serve_typescript_file(p, "/src/a.ts".to_string(), false).await;
        let (status, body) = read_body(resp).await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            !body.contains("sourceMappingURL"),
            "expected no source map: {body}"
        );
    }

    #[tokio::test]
    async fn ok_response_has_js_content_type() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("a.ts");
        std::fs::write(&p, "export const x = 1;").unwrap();
        let resp = serve_typescript_file(p, "/src/a.ts".to_string(), false).await;
        let ct = resp.headers().get(header::CONTENT_TYPE).unwrap();
        assert_eq!(
            ct.to_str().unwrap(),
            "application/javascript; charset=utf-8"
        );
    }

    #[tokio::test]
    async fn syntax_error_returns_500_with_error_body() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("bad.ts");
        std::fs::write(&p, "const x: = ;").unwrap();
        let resp = serve_typescript_file(p, "/src/bad.ts".to_string(), false).await;
        let (status, body) = read_body(resp).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(body.contains("transpile error"), "body: {body}");
    }
}
