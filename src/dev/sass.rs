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
