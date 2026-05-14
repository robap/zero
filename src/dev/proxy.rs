//! Reverse-proxy handler for `zero dev` when `[dev].proxy` is configured.

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{HeaderMap, HeaderName, HeaderValue, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use reqwest::Client;

use crate::dev::inject::inject;

/// Hop-by-hop headers that must not be forwarded (RFC 7230 §6.1).
static HOP_BY_HOP: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailers",
    "transfer-encoding",
    "upgrade",
];

/// Cache and validator headers stripped from upstream responses.
static STRIP_FROM_UPSTREAM: &[&str] = &[
    "cache-control",
    "pragma",
    "expires",
    "etag",
    "last-modified",
    "content-encoding",
];

/// Build a shared `reqwest::Client` for proxy use.
///
/// Disables compression, follows no redirects, and sets a 30-second timeout.
///
/// # Returns
/// A configured `Client`.
pub fn build_client() -> anyhow::Result<Client> {
    Ok(Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .timeout(Duration::from_secs(30))
        .build()?)
}

/// Forward an incoming request to the backend proxy and return its response,
/// with script injection applied to HTML responses.
///
/// # Parameters
/// - `proxy_base`: base URL of the backend (e.g. `http://localhost:8080`).
/// - `client`: shared HTTP client.
/// - `req`: the incoming axum request.
/// - `app_entry_href`: bootstrap script `src` to substitute into injected HTML.
///
/// # Returns
/// A proxied response, or a 502/501 on error.
pub async fn proxy_request(
    proxy_base: &url::Url,
    client: &Client,
    req: Request<Body>,
    app_entry_href: &str,
) -> Response {
    // Reject WebSocket upgrade requests.
    if req
        .headers()
        .get("upgrade")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false)
    {
        return (
            StatusCode::NOT_IMPLEMENTED,
            "zero dev: WebSocket proxying is out of scope in this slice",
        )
            .into_response();
    }

    // Build upstream URL.
    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");

    let upstream_url = match build_upstream_url(proxy_base, path_and_query) {
        Ok(u) => u,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("zero dev: failed to build upstream URL: {e}"),
            )
                .into_response();
        }
    };

    // Forward headers (minus hop-by-hop), override Accept-Encoding to identity.
    let mut forward_headers = HeaderMap::new();
    for (name, value) in req.headers() {
        let name_lower = name.as_str().to_ascii_lowercase();
        if HOP_BY_HOP.contains(&name_lower.as_str()) {
            continue;
        }
        forward_headers.insert(name.clone(), value.clone());
    }
    forward_headers.insert(
        HeaderName::from_static("accept-encoding"),
        HeaderValue::from_static("identity"),
    );

    // Build and execute the upstream request.
    let method = req.method().clone();
    let body_bytes = axum::body::to_bytes(req.into_body(), usize::MAX)
        .await
        .unwrap_or_default();

    let upstream_req = client
        .request(method, upstream_url.as_str())
        .headers(forward_headers)
        .body(body_bytes);

    let upstream_resp = match upstream_req.send().await {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                [(
                    axum::http::header::CONTENT_TYPE,
                    HeaderValue::from_static("text/html; charset=utf-8"),
                )],
                format!(
                    "<h1>zero dev</h1><p>Cannot reach backend at {}: {e}</p>",
                    proxy_base
                ),
            )
                .into_response();
        }
    };

    // Build the outgoing response.
    let status = StatusCode::from_u16(upstream_resp.status().as_u16())
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    let mut resp_headers = HeaderMap::new();
    for (name, value) in upstream_resp.headers() {
        let name_lower = name.as_str().to_ascii_lowercase();
        if HOP_BY_HOP.contains(&name_lower.as_str()) {
            continue;
        }
        if STRIP_FROM_UPSTREAM.contains(&name_lower.as_str()) {
            continue;
        }
        resp_headers.insert(name.clone(), value.clone());
    }

    let content_type = upstream_resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();

    if content_type.starts_with("text/html") {
        let body_bytes = upstream_resp.bytes().await.unwrap_or_default();
        let injected = inject(&body_bytes, app_entry_href);
        let len = injected.len();
        resp_headers.insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        );
        resp_headers.insert(
            axum::http::header::CONTENT_LENGTH,
            HeaderValue::from(len as u64),
        );
        let mut builder = http_response_builder(status, resp_headers);
        *builder.body_mut() = Some(injected);
        return builder.into_response_body();
    }

    // Non-HTML: stream body through unchanged.
    let body_bytes = upstream_resp.bytes().await.unwrap_or_default();
    let mut response = Response::builder()
        .status(status)
        .body(Body::from(body_bytes))
        .unwrap();
    *response.headers_mut() = resp_headers;
    response
}

struct HtmlResponseBuilder {
    status: StatusCode,
    headers: HeaderMap,
    body: Option<Vec<u8>>,
}

impl HtmlResponseBuilder {
    fn body_mut(&mut self) -> &mut Option<Vec<u8>> {
        &mut self.body
    }

    fn into_response_body(self) -> Response {
        let body_bytes = self.body.unwrap_or_default();
        let mut resp = Response::new(Body::from(body_bytes));
        *resp.status_mut() = self.status;
        *resp.headers_mut() = self.headers;
        resp
    }
}

fn http_response_builder(status: StatusCode, headers: HeaderMap) -> HtmlResponseBuilder {
    HtmlResponseBuilder {
        status,
        headers,
        body: None,
    }
}

fn build_upstream_url(base: &url::Url, path_and_query: &str) -> anyhow::Result<url::Url> {
    let mut u = base.clone();
    let pq = path_and_query.trim_start_matches('/');
    u.set_path(path_and_query);
    let _ = pq;
    Ok(u)
}

/// Shared state addition for proxy mode.
pub struct ProxyState {
    /// Backend base URL.
    pub proxy_base: url::Url,
    /// Shared HTTP client.
    pub client: Arc<Client>,
}

impl ProxyState {
    /// Create from a config URL.
    ///
    /// # Parameters
    /// - `url`: the backend proxy URL.
    ///
    /// # Returns
    /// A new `ProxyState` or an error if the client can't be built.
    pub fn new(url: url::Url) -> anyhow::Result<Self> {
        Ok(Self {
            proxy_base: url,
            client: Arc::new(build_client()?),
        })
    }
}
