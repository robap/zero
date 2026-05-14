//! Dev-server endpoint that transpiles `.ts` files on the fly.

use std::path::PathBuf;

use axum::body::Body;
use axum::http::StatusCode;
use axum::http::header;
use axum::response::{IntoResponse, Response};

use crate::transpile::{TranspileOptions, transpile_typescript};

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
