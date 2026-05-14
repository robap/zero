//! Serve the local `<root>/index.html` with scripts injected (no-proxy mode).

use std::path::PathBuf;

use axum::http::StatusCode;
use axum::http::header;
use axum::response::{IntoResponse, Response};

use crate::dev::inject::inject;

/// Read `root/index.html`, inject dev scripts, and return it as a response.
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

    let body = inject(&raw);
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        body,
    )
        .into_response()
}
