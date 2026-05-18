//! Reverse-proxy handler for `zero dev` when `[dev].proxy` is configured.

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{HeaderMap, HeaderName, HeaderValue, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use reqwest::Client;

use crate::inject::inject;

/// The observed failure the explainer is describing.
enum ExplainerFailure {
    /// Upstream returned a non-2xx status.
    UpstreamStatus(StatusCode),
    /// Upstream is unreachable (connection refused, DNS failure, timeout).
    Unreachable(String),
}

/// Render the diagnostic explainer page describing why proxy mode failed
/// at the root.
fn render_explainer_html(proxy_base: &url::Url, failure: &ExplainerFailure) -> String {
    let base_str = proxy_base.as_str();
    let base_trimmed = base_str.strip_suffix('/').unwrap_or(base_str);
    let base_escaped = html_escape(base_trimmed);
    let diagnostic = match failure {
        ExplainerFailure::UpstreamStatus(code) => format!(
            "Your backend at {base_escaped} returned {} for /",
            code.as_u16()
        ),
        ExplainerFailure::Unreachable(err) => format!(
            "Could not reach your backend at {base_escaped}: {}",
            html_escape(err)
        ),
    };
    format!(
        "<!doctype html>\n\
<html lang=\"en\">\n\
<head>\n\
  <meta charset=\"utf-8\">\n\
  <title>zero dev — backend not serving /</title>\n\
  <style>\n\
    body {{ font-family: system-ui, sans-serif; max-width: 40rem; margin: 4rem auto; padding: 0 1rem; line-height: 1.5; color: #222; }}\n\
    code {{ background: #f3f3f3; padding: 0 0.25rem; border-radius: 3px; }}\n\
    h1 {{ font-size: 1.25rem; margin-bottom: 0.5rem; }}\n\
    p.diagnostic {{ font-weight: 600; }}\n\
  </style>\n\
</head>\n\
<body>\n\
  <h1>zero dev</h1>\n\
  <p class=\"diagnostic\">{diagnostic}</p>\n\
  <p>\n\
    In proxy mode, <code>zero dev</code> forwards requests to your backend.\n\
    Your backend is expected to serve the HTML at <code>/</code>. <code>zero dev</code>\n\
    will inject the dev scripts (import map, app entry, reload client)\n\
    into that response automatically.\n\
  </p>\n\
</body>\n\
</html>\n"
    )
}

/// Build a `Response` carrying the rendered explainer page.
///
/// Sets `Content-Type: text/html; charset=utf-8` and `Content-Length`.
fn explainer_response(
    status: StatusCode,
    proxy_base: &url::Url,
    failure: ExplainerFailure,
) -> Response {
    let body = render_explainer_html(proxy_base, &failure);
    let len = body.len();
    let mut resp = Response::new(Body::from(body));
    *resp.status_mut() = status;
    let headers = resp.headers_mut();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    headers.insert(
        axum::http::header::CONTENT_LENGTH,
        HeaderValue::from(len as u64),
    );
    resp
}

/// Escape the five HTML-significant characters so a `reqwest::Error` string
/// (or any other unsafe text) embeds safely in the rendered page.
fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(c),
        }
    }
    out
}

/// True when the explainer should replace the upstream/error response.
///
/// The trigger is intentionally narrow: only `GET /` and `GET /index.html`
/// — the paths a browser asks for when loading the dev server. Match is
/// case-sensitive; the query string is ignored.
fn is_root_html_request(method: &axum::http::Method, path: &str) -> bool {
    method == axum::http::Method::GET && (path == "/" || path == "/index.html")
}

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
    if is_websocket_upgrade(&req) {
        return (
            StatusCode::NOT_IMPLEMENTED,
            "zero dev: WebSocket proxying is out of scope in this slice",
        )
            .into_response();
    }

    let req_method = req.method().clone();
    let req_path = req.uri().path().to_string();

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

    let forward_headers = build_forward_headers(req.headers());
    let method = req_method.clone();
    let body_bytes = axum::body::to_bytes(req.into_body(), usize::MAX)
        .await
        .unwrap_or_default();

    let upstream_resp = match client
        .request(method, upstream_url.as_str())
        .headers(forward_headers)
        .body(body_bytes)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            if is_root_html_request(&req_method, &req_path) {
                return explainer_response(
                    StatusCode::BAD_GATEWAY,
                    proxy_base,
                    ExplainerFailure::Unreachable(e.to_string()),
                );
            }
            return bad_gateway_response(proxy_base, e);
        }
    };

    let status = StatusCode::from_u16(upstream_resp.status().as_u16())
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    if is_root_html_request(&req_method, &req_path) && !status.is_success() {
        let _ = upstream_resp.bytes().await;
        return explainer_response(status, proxy_base, ExplainerFailure::UpstreamStatus(status));
    }

    let mut resp_headers = filter_upstream_headers(upstream_resp.headers());
    let content_type = upstream_resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();

    if content_type.starts_with("text/html") {
        let body_bytes = upstream_resp.bytes().await.unwrap_or_default();
        let injected = inject(&body_bytes, app_entry_href);
        return build_html_response(status, &mut resp_headers, injected);
    }

    let body_bytes = upstream_resp.bytes().await.unwrap_or_default();
    let mut response = Response::builder()
        .status(status)
        .body(Body::from(body_bytes))
        .unwrap();
    *response.headers_mut() = resp_headers;
    response
}

/// True if the request carries an `Upgrade: websocket` header.
fn is_websocket_upgrade(req: &Request<Body>) -> bool {
    req.headers()
        .get("upgrade")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false)
}

/// Copy `incoming` minus hop-by-hop headers; force `Accept-Encoding: identity`
/// so we never have to decompress upstream bodies before injection.
fn build_forward_headers(incoming: &HeaderMap) -> HeaderMap {
    let mut out = HeaderMap::new();
    for (name, value) in incoming {
        let name_lower = name.as_str().to_ascii_lowercase();
        if HOP_BY_HOP.contains(&name_lower.as_str()) {
            continue;
        }
        out.insert(name.clone(), value.clone());
    }
    out.insert(
        HeaderName::from_static("accept-encoding"),
        HeaderValue::from_static("identity"),
    );
    out
}

/// Drop hop-by-hop and cache/validator headers from the upstream response.
fn filter_upstream_headers(upstream: &HeaderMap) -> HeaderMap {
    let mut out = HeaderMap::new();
    for (name, value) in upstream {
        let name_lower = name.as_str().to_ascii_lowercase();
        if HOP_BY_HOP.contains(&name_lower.as_str())
            || STRIP_FROM_UPSTREAM.contains(&name_lower.as_str())
        {
            continue;
        }
        out.insert(name.clone(), value.clone());
    }
    out
}

/// Build a 502 response with a human-readable HTML error body.
fn bad_gateway_response(proxy_base: &url::Url, e: reqwest::Error) -> Response {
    (
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
        .into_response()
}

/// Assemble the proxied HTML response with content-type/length set on
/// `resp_headers`.
fn build_html_response(
    status: StatusCode,
    resp_headers: &mut HeaderMap,
    injected: Vec<u8>,
) -> Response {
    let len = injected.len();
    resp_headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    resp_headers.insert(
        axum::http::header::CONTENT_LENGTH,
        HeaderValue::from(len as u64),
    );
    let mut builder = http_response_builder(status, resp_headers.clone());
    *builder.body_mut() = Some(injected);
    builder.into_response_body()
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::Router;
    use axum::routing::get;
    use http_body_util::BodyExt;
    use std::net::SocketAddr;

    async fn read_body(resp: Response) -> (StatusCode, HeaderMap, String) {
        let status = resp.status();
        let headers = resp.headers().clone();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        (status, headers, String::from_utf8_lossy(&bytes).to_string())
    }

    async fn start_backend<F>(make_app: F) -> (SocketAddr, tokio::task::JoinHandle<()>)
    where
        F: FnOnce() -> Router,
    {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = make_app();
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });
        (addr, handle)
    }

    #[test]
    fn build_client_succeeds() {
        let c = build_client();
        assert!(c.is_ok());
    }

    #[tokio::test]
    async fn explainer_response_sets_html_content_type_and_status() {
        let base = url::Url::parse("http://localhost:5080").unwrap();
        let resp = explainer_response(
            StatusCode::NOT_FOUND,
            &base,
            ExplainerFailure::UpstreamStatus(StatusCode::NOT_FOUND),
        );
        let (status, headers, _body) = read_body(resp).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let ct = headers.get("content-type").unwrap().to_str().unwrap();
        assert_eq!(ct, "text/html; charset=utf-8");
    }

    #[test]
    fn render_explainer_html_has_no_script_tags() {
        let base = url::Url::parse("http://localhost:5080").unwrap();
        let body = render_explainer_html(
            &base,
            &ExplainerFailure::UpstreamStatus(StatusCode::NOT_FOUND),
        );
        assert!(!body.contains("<script"), "body: {body}");
    }

    #[test]
    fn render_explainer_html_includes_contract_paragraph() {
        let base = url::Url::parse("http://localhost:5080").unwrap();
        let body = render_explainer_html(
            &base,
            &ExplainerFailure::UpstreamStatus(StatusCode::NOT_FOUND),
        );
        assert!(
            body.contains("forwards requests to your backend"),
            "body: {body}"
        );
        assert!(body.contains("serve the HTML at"), "body: {body}");
    }

    #[test]
    fn render_explainer_html_includes_url_and_error_for_unreachable_variant() {
        let base = url::Url::parse("http://localhost:5080").unwrap();
        let body = render_explainer_html(
            &base,
            &ExplainerFailure::Unreachable("connection refused".into()),
        );
        assert!(body.contains("http://localhost:5080"), "body: {body}");
        assert!(body.contains("connection refused"), "body: {body}");
    }

    #[test]
    fn render_explainer_html_includes_upstream_url_and_status_for_status_variant() {
        let base = url::Url::parse("http://localhost:5080").unwrap();
        let body = render_explainer_html(
            &base,
            &ExplainerFailure::UpstreamStatus(StatusCode::NOT_FOUND),
        );
        assert!(body.contains("http://localhost:5080"), "body: {body}");
        assert!(
            !body.contains("http://localhost:5080/"),
            "trailing slash should be stripped, got: {body}"
        );
        assert!(body.contains("404"), "body: {body}");
    }

    #[test]
    fn is_root_html_request_matches_get_root_and_index_html() {
        assert!(is_root_html_request(&axum::http::Method::GET, "/"));
        assert!(is_root_html_request(
            &axum::http::Method::GET,
            "/index.html"
        ));
        assert!(!is_root_html_request(
            &axum::http::Method::GET,
            "/api/health"
        ));
        assert!(!is_root_html_request(&axum::http::Method::POST, "/"));
        assert!(!is_root_html_request(
            &axum::http::Method::GET,
            "/Index.html"
        ));
    }

    #[test]
    fn proxy_state_new_constructs() {
        let s = ProxyState::new(url::Url::parse("http://127.0.0.1:9999").unwrap());
        assert!(s.is_ok());
    }

    #[tokio::test]
    async fn websocket_upgrade_returns_501() {
        let client = build_client().unwrap();
        let base = url::Url::parse("http://127.0.0.1:1").unwrap();
        let req = Request::builder()
            .uri("/ws")
            .header("upgrade", "websocket")
            .header("connection", "upgrade")
            .body(Body::empty())
            .unwrap();
        let resp = proxy_request(&base, &client, req, "/src/app.js").await;
        let (status, _, body) = read_body(resp).await;
        assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
        assert!(body.contains("WebSocket"), "body: {body}");
    }

    #[tokio::test]
    async fn unreachable_backend_returns_502() {
        let client = build_client().unwrap();
        // Port 1 should not be listening.
        let base = url::Url::parse("http://127.0.0.1:1").unwrap();
        let req = Request::builder()
            .uri("/anything")
            .body(Body::empty())
            .unwrap();
        let resp = proxy_request(&base, &client, req, "/src/app.js").await;
        let (status, _, body) = read_body(resp).await;
        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert!(body.contains("Cannot reach backend"), "body: {body}");
    }

    #[tokio::test]
    async fn html_response_is_injected_with_scripts() {
        let (addr, _h) = start_backend(|| {
            Router::new().route(
                "/",
                get(|| async {
                    (
                        StatusCode::OK,
                        [("content-type", "text/html; charset=utf-8")],
                        "<html><head><title>X</title></head><body>hi</body></html>",
                    )
                        .into_response()
                }),
            )
        })
        .await;
        let client = build_client().unwrap();
        let base = url::Url::parse(&format!("http://{addr}")).unwrap();
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        let resp = proxy_request(&base, &client, req, "/src/app.js").await;
        let (status, headers, body) = read_body(resp).await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            body.contains(r#"<script type="importmap">"#),
            "body: {body}"
        );
        // Content-Length should reflect the injected body length.
        let cl = headers.get("content-length").unwrap();
        assert_eq!(cl.to_str().unwrap().parse::<usize>().unwrap(), body.len());
    }

    #[tokio::test]
    async fn non_html_response_is_streamed_through_unchanged() {
        let (addr, _h) = start_backend(|| {
            Router::new().route(
                "/api",
                get(|| async {
                    (
                        StatusCode::OK,
                        [("content-type", "application/json")],
                        r#"{"x":1}"#,
                    )
                        .into_response()
                }),
            )
        })
        .await;
        let client = build_client().unwrap();
        let base = url::Url::parse(&format!("http://{addr}")).unwrap();
        let req = Request::builder().uri("/api").body(Body::empty()).unwrap();
        let resp = proxy_request(&base, &client, req, "/src/app.js").await;
        let (status, headers, body) = read_body(resp).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, r#"{"x":1}"#);
        let ct = headers.get("content-type").unwrap();
        assert!(
            ct.to_str().unwrap().contains("application/json"),
            "ct: {ct:?}"
        );
    }

    #[tokio::test]
    async fn html_response_strips_cache_and_validator_headers() {
        let (addr, _h) = start_backend(|| {
            Router::new().route(
                "/",
                get(|| async {
                    (
                        StatusCode::OK,
                        [
                            ("content-type", "text/html; charset=utf-8"),
                            ("cache-control", "max-age=3600"),
                            ("etag", "\"abc\""),
                            ("last-modified", "Wed, 21 Oct 2015 07:28:00 GMT"),
                        ],
                        "<html><head></head><body></body></html>",
                    )
                        .into_response()
                }),
            )
        })
        .await;
        let client = build_client().unwrap();
        let base = url::Url::parse(&format!("http://{addr}")).unwrap();
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        let resp = proxy_request(&base, &client, req, "/src/app.js").await;
        let (status, headers, _body) = read_body(resp).await;
        assert_eq!(status, StatusCode::OK);
        assert!(headers.get("etag").is_none(), "etag should be stripped");
        assert!(
            headers.get("last-modified").is_none(),
            "last-modified should be stripped"
        );
        // cache-control survives as-is because the no_cache_layer reapplies it at the router level.
        // The proxy itself strips the upstream value.
        assert!(
            !headers
                .get("cache-control")
                .map(|v| v.to_str().unwrap().contains("max-age=3600"))
                .unwrap_or(false),
            "upstream cache-control should not leak through"
        );
    }

    #[tokio::test]
    async fn upstream_status_is_propagated() {
        let (addr, _h) = start_backend(|| {
            Router::new().route(
                "/api/health",
                get(|| async {
                    (
                        StatusCode::BAD_GATEWAY,
                        [("content-type", "text/plain")],
                        "upstream is down",
                    )
                        .into_response()
                }),
            )
        })
        .await;
        let client = build_client().unwrap();
        let base = url::Url::parse(&format!("http://{addr}")).unwrap();
        let req = Request::builder()
            .uri("/api/health")
            .body(Body::empty())
            .unwrap();
        let resp = proxy_request(&base, &client, req, "/src/app.js").await;
        let (status, _, body) = read_body(resp).await;
        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert_eq!(body, "upstream is down");
    }

    #[tokio::test]
    async fn unreachable_backend_at_non_root_keeps_existing_502() {
        let client = build_client().unwrap();
        let base = url::Url::parse("http://127.0.0.1:1").unwrap();
        let req = Request::builder()
            .uri("/anything")
            .body(Body::empty())
            .unwrap();
        let resp = proxy_request(&base, &client, req, "/src/app.js").await;
        let (status, _, body) = read_body(resp).await;
        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert!(body.contains("Cannot reach backend"), "body: {body}");
    }

    #[tokio::test]
    async fn unreachable_backend_at_root_returns_explainer() {
        let client = build_client().unwrap();
        // Port 1 should not be listening.
        let base = url::Url::parse("http://127.0.0.1:1").unwrap();
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        let resp = proxy_request(&base, &client, req, "/src/app.js").await;
        let (status, _, body) = read_body(resp).await;
        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert!(
            body.contains("Could not reach your backend"),
            "body: {body}"
        );
        assert!(
            body.contains("forwards requests to your backend"),
            "body: {body}"
        );
    }

    #[tokio::test]
    async fn successful_html_root_response_is_injected_not_replaced() {
        let (addr, _h) = start_backend(|| {
            Router::new().route(
                "/",
                get(|| async {
                    (
                        StatusCode::OK,
                        [("content-type", "text/html; charset=utf-8")],
                        "<html><head><title>X</title></head><body>hi</body></html>",
                    )
                        .into_response()
                }),
            )
        })
        .await;
        let client = build_client().unwrap();
        let base = url::Url::parse(&format!("http://{addr}")).unwrap();
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        let resp = proxy_request(&base, &client, req, "/src/app.js").await;
        let (status, _, body) = read_body(resp).await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            body.contains(r#"<script type="importmap">"#),
            "body: {body}"
        );
        assert!(
            !body.contains("forwards requests to your backend"),
            "body: {body}"
        );
    }

    #[tokio::test]
    async fn non_root_404_passes_through_verbatim() {
        let (addr, _h) = start_backend(|| {
            Router::new().route(
                "/api/health",
                get(|| async {
                    (
                        StatusCode::NOT_FOUND,
                        [("content-type", "text/plain")],
                        "not found",
                    )
                        .into_response()
                }),
            )
        })
        .await;
        let client = build_client().unwrap();
        let base = url::Url::parse(&format!("http://{addr}")).unwrap();
        let req = Request::builder()
            .uri("/api/health")
            .body(Body::empty())
            .unwrap();
        let resp = proxy_request(&base, &client, req, "/src/app.js").await;
        let (status, _, body) = read_body(resp).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body, "not found");
        assert!(
            !body.contains("forwards requests to your backend"),
            "body: {body}"
        );
    }

    #[tokio::test]
    async fn explainer_replaces_upstream_404_at_index_html() {
        let (addr, _h) = start_backend(|| {
            Router::new().route(
                "/index.html",
                get(|| async {
                    (
                        StatusCode::NOT_FOUND,
                        [("content-type", "text/plain")],
                        "kestrel default 404",
                    )
                        .into_response()
                }),
            )
        })
        .await;
        let client = build_client().unwrap();
        let base = url::Url::parse(&format!("http://{addr}")).unwrap();
        let req = Request::builder()
            .uri("/index.html")
            .body(Body::empty())
            .unwrap();
        let resp = proxy_request(&base, &client, req, "/src/app.js").await;
        let (status, _, body) = read_body(resp).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body.contains("returned 404 for /"), "body: {body}");
        assert!(
            body.contains("forwards requests to your backend"),
            "body: {body}"
        );
    }

    #[tokio::test]
    async fn explainer_replaces_upstream_404_at_root() {
        let (addr, _h) = start_backend(|| {
            Router::new().route(
                "/",
                get(|| async {
                    (
                        StatusCode::NOT_FOUND,
                        [("content-type", "text/plain")],
                        "kestrel default 404",
                    )
                        .into_response()
                }),
            )
        })
        .await;
        let client = build_client().unwrap();
        let base = url::Url::parse(&format!("http://{addr}")).unwrap();
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        let resp = proxy_request(&base, &client, req, "/src/app.js").await;
        let (status, headers, body) = read_body(resp).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body.contains("returned 404 for /"), "body: {body}");
        assert!(
            body.contains("forwards requests to your backend"),
            "body: {body}"
        );
        let ct = headers.get("content-type").unwrap().to_str().unwrap();
        assert!(ct.starts_with("text/html"), "ct: {ct}");
    }
}
