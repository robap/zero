//! Cache-defeating response headers for `zero dev`.

use axum::http::HeaderValue;
use axum::http::header::{CACHE_CONTROL, EXPIRES, PRAGMA};
use tower::layer::util::Stack;
use tower_http::set_header::SetResponseHeaderLayer;

/// Compose the three `Set-Header` layers that override `Cache-Control`,
/// `Pragma`, and `Expires` on every outgoing response.
///
/// # Returns
/// A stacked tower layer that can be added to an axum `Router`.
pub fn no_cache_layer() -> Stack<
    SetResponseHeaderLayer<HeaderValue>,
    Stack<SetResponseHeaderLayer<HeaderValue>, SetResponseHeaderLayer<HeaderValue>>,
> {
    let cache_control = SetResponseHeaderLayer::overriding(
        CACHE_CONTROL,
        HeaderValue::from_static("no-store, no-cache, must-revalidate, max-age=0"),
    );
    let pragma = SetResponseHeaderLayer::overriding(PRAGMA, HeaderValue::from_static("no-cache"));
    let expires = SetResponseHeaderLayer::overriding(EXPIRES, HeaderValue::from_static("0"));
    Stack::new(expires, Stack::new(pragma, cache_control))
}
