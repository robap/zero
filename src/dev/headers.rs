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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::body::Body;
    use axum::http::Request;
    use axum::routing::get;
    use tower::ServiceExt;

    #[tokio::test]
    async fn no_cache_layer_sets_cache_control_to_no_store() {
        let app = Router::new()
            .route("/", get(|| async { "hi" }))
            .layer(no_cache_layer());
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let cc = resp.headers().get("cache-control").unwrap();
        assert_eq!(
            cc.to_str().unwrap(),
            "no-store, no-cache, must-revalidate, max-age=0"
        );
    }

    #[tokio::test]
    async fn no_cache_layer_sets_pragma_no_cache() {
        let app = Router::new()
            .route("/", get(|| async { "hi" }))
            .layer(no_cache_layer());
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let pragma = resp.headers().get("pragma").unwrap();
        assert_eq!(pragma.to_str().unwrap(), "no-cache");
    }

    #[tokio::test]
    async fn no_cache_layer_sets_expires_zero() {
        let app = Router::new()
            .route("/", get(|| async { "hi" }))
            .layer(no_cache_layer());
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let expires = resp.headers().get("expires").unwrap();
        assert_eq!(expires.to_str().unwrap(), "0");
    }
}
