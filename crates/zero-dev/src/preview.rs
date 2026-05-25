//! `zero preview` — static-file server for the production build.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, Request, StatusCode, header};
use axum::response::{IntoResponse, Response};

use zero_config::Config;

use crate::files::content_type_for;
use crate::headers::no_cache_layer;
use crate::proxy::ProxyState;
use crate::server::bind_listener;

#[derive(Clone)]
struct PreviewState {
    out_dir: PathBuf,
    proxy: Option<Arc<ProxyState>>,
}

/// Start the preview server and block until shutdown.
///
/// # Parameters
/// - `config`: validated `zero.toml` config; provides the output directory
///   and `[dev] port`.
///
/// # Returns
/// `Ok(())` on graceful shutdown, an error on bind or runtime failure.
pub async fn serve_preview(config: &Config) -> anyhow::Result<()> {
    let out_dir = config.out_dir_path();
    let proxy = config
        .dev
        .proxy
        .clone()
        .map(|url| ProxyState::new(url).map(Arc::new))
        .transpose()?;
    let listener = bind_listener(config.dev.port).await?;
    println!(
        "zero preview — listening on http://{}",
        listener.local_addr()?
    );
    if let Some(ps) = &proxy {
        println!(
            "zero preview — proxying unmatched paths to {}",
            ps.proxy_base
        );
    }
    let app = build_preview_app(out_dir, proxy);
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

/// Build the preview-server `Router`.
///
/// Extracted so the route table can be exercised in unit tests without
/// binding a listener.
///
/// # Parameters
/// - `out_dir`: directory `zero build` wrote into (`config.out_dir_path()`).
/// - `proxy`: optional upstream proxy; when set, unmatched requests are
///   forwarded and only fall through to the SPA index on a proxy 404.
///
/// # Returns
/// A `Router` with static-file routing, optional proxy, SPA fallback, and
/// the no-cache layer.
pub(crate) fn build_preview_app(out_dir: PathBuf, proxy: Option<Arc<ProxyState>>) -> Router {
    let state = Arc::new(PreviewState { out_dir, proxy });
    Router::new()
        .fallback(handle_request)
        .layer(no_cache_layer())
        .with_state(state)
}

async fn handle_request(State(state): State<Arc<PreviewState>>, req: Request<Body>) -> Response {
    let uri_path = req.uri().path().to_string();
    if uri_path.split('/').any(|seg| seg == "..") {
        return (StatusCode::FORBIDDEN, "forbidden").into_response();
    }
    let rel = uri_path.trim_start_matches('/');
    if rel.is_empty() {
        return serve_spa_index(&state.out_dir).await;
    }
    let candidate = state.out_dir.join(rel);
    if let Some(resp) = serve_static(&state.out_dir, &candidate).await {
        return resp;
    }
    if let Some(ps) = &state.proxy {
        match preview_proxy(ps, req).await {
            ProxyOutcome::Response(resp) => return resp,
            ProxyOutcome::NotFound => {}
        }
    }
    serve_spa_index(&state.out_dir).await
}

enum ProxyOutcome {
    /// Forward this response to the client verbatim.
    Response(Response),
    /// Upstream returned 404; let the caller fall back to the SPA index.
    NotFound,
}

/// Forward a request to the configured upstream and return the response
/// byte-pure. Reports 404s separately so the caller can fall back to the
/// SPA shell for client-routed paths (e.g. `/parts`).
async fn preview_proxy(ps: &ProxyState, req: Request<Body>) -> ProxyOutcome {
    let method = req.method().clone();
    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/")
        .to_string();
    let forward_headers = filter_request_headers(req.headers());
    let body_bytes = axum::body::to_bytes(req.into_body(), usize::MAX)
        .await
        .unwrap_or_default();

    let upstream_url = format!(
        "{}{}",
        ps.proxy_base.as_str().trim_end_matches('/'),
        path_and_query
    );

    let upstream = match ps
        .client
        .request(reqwest_method(&method), &upstream_url)
        .headers(forward_headers)
        .body(body_bytes)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return ProxyOutcome::Response(
                (
                    StatusCode::BAD_GATEWAY,
                    format!("zero preview: proxy to {} failed: {e}", ps.proxy_base),
                )
                    .into_response(),
            );
        }
    };

    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    if status == StatusCode::NOT_FOUND {
        return ProxyOutcome::NotFound;
    }
    let headers = filter_response_headers(upstream.headers());
    let bytes = upstream.bytes().await.unwrap_or_default();
    let mut response = Response::builder()
        .status(status)
        .body(Body::from(bytes))
        .unwrap_or_else(|_| Response::new(Body::empty()));
    *response.headers_mut() = headers;
    ProxyOutcome::Response(response)
}

fn reqwest_method(m: &Method) -> reqwest::Method {
    reqwest::Method::from_bytes(m.as_str().as_bytes()).unwrap_or(reqwest::Method::GET)
}

/// Hop-by-hop headers (RFC 7230 §6.1) plus `host` must not be forwarded.
const HOP_BY_HOP: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailers",
    "transfer-encoding",
    "upgrade",
    "host",
];

fn filter_request_headers(src: &HeaderMap) -> reqwest::header::HeaderMap {
    let mut out = reqwest::header::HeaderMap::new();
    for (k, v) in src.iter() {
        if HOP_BY_HOP
            .iter()
            .any(|h| k.as_str().eq_ignore_ascii_case(h))
        {
            continue;
        }
        if let (Ok(name), Ok(value)) = (
            reqwest::header::HeaderName::from_bytes(k.as_str().as_bytes()),
            reqwest::header::HeaderValue::from_bytes(v.as_bytes()),
        ) {
            out.append(name, value);
        }
    }
    out
}

fn filter_response_headers(src: &reqwest::header::HeaderMap) -> HeaderMap {
    let mut out = HeaderMap::new();
    for (k, v) in src.iter() {
        if HOP_BY_HOP
            .iter()
            .any(|h| k.as_str().eq_ignore_ascii_case(h))
        {
            continue;
        }
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(k.as_str().as_bytes()),
            HeaderValue::from_bytes(v.as_bytes()),
        ) {
            out.append(name, value);
        }
    }
    out
}

/// Try to serve `candidate` as a static file beneath `out_root`. Returns
/// `None` if the file is missing, is a directory, or escapes `out_root` —
/// the caller should fall back to the SPA index in those cases (except
/// escape, which returns a 403 inline below).
async fn serve_static(out_root: &Path, candidate: &Path) -> Option<Response> {
    let root_canon = std::fs::canonicalize(out_root).ok()?;
    let canonical = std::fs::canonicalize(candidate).ok()?;
    if !canonical.starts_with(&root_canon) {
        return Some((StatusCode::FORBIDDEN, "forbidden").into_response());
    }
    if !canonical.is_file() {
        return None;
    }
    let bytes = tokio::fs::read(&canonical).await.ok()?;
    let ctype = content_type_for(&canonical);
    Some(
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, ctype)],
            Body::from(bytes),
        )
            .into_response(),
    )
}

/// Read `out_root/index.html` and return it as the SPA fallback response.
async fn serve_spa_index(out_root: &Path) -> Response {
    let index_path = out_root.join("index.html");
    match tokio::fs::read(&index_path).await {
        Ok(bytes) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
            Body::from(bytes),
        )
            .into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!(
                "zero preview: {} not found; run `zero build` first",
                index_path.display()
            ),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn serves_index_for_unknown_path() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("index.html"), "<!doctype html>SPA").unwrap();
        let app = build_preview_app(tmp.path().to_path_buf(), None);
        let req = Request::builder()
            .uri("/some/client/route")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let ct = resp.headers().get("content-type").unwrap();
        assert!(ct.to_str().unwrap().contains("text/html"), "ct: {ct:?}");
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let s = std::str::from_utf8(&body).unwrap();
        assert!(s.contains("SPA"), "body: {s}");
    }

    #[tokio::test]
    async fn serves_static_file_with_no_cache_headers() {
        let tmp = tempfile::tempdir().unwrap();
        let assets = tmp.path().join("assets");
        std::fs::create_dir_all(&assets).unwrap();
        std::fs::write(assets.join("app.abc123.js"), "console.log(1)").unwrap();
        std::fs::write(tmp.path().join("index.html"), "<!doctype html>x").unwrap();
        let app = build_preview_app(tmp.path().to_path_buf(), None);
        let req = Request::builder()
            .uri("/assets/app.abc123.js")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let ct = resp.headers().get("content-type").unwrap();
        assert_eq!(
            ct.to_str().unwrap(),
            "application/javascript; charset=utf-8"
        );
        let cc = resp.headers().get("cache-control").unwrap();
        assert!(cc.to_str().unwrap().contains("no-store"), "cc: {cc:?}");
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"console.log(1)");
    }

    #[tokio::test]
    async fn serves_root_returns_index() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("index.html"), "<!doctype html>ROOT").unwrap();
        let app = build_preview_app(tmp.path().to_path_buf(), None);
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let s = std::str::from_utf8(&body).unwrap();
        assert!(s.contains("ROOT"), "body: {s}");
    }

    #[tokio::test]
    async fn traversal_returns_403() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("index.html"), "<!doctype html>x").unwrap();
        let app = build_preview_app(tmp.path().to_path_buf(), None);
        let req = Request::builder()
            .uri("/../etc/passwd")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn missing_index_returns_500_for_spa_fallback() {
        let tmp = tempfile::tempdir().unwrap();
        let app = build_preview_app(tmp.path().to_path_buf(), None);
        let req = Request::builder()
            .uri("/anything")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[tokio::test]
    async fn serves_public_subtree_files() {
        let tmp = tempfile::tempdir().unwrap();
        let public = tmp.path().join("public");
        std::fs::create_dir_all(&public).unwrap();
        std::fs::write(public.join("robots.txt"), "User-agent: *\n").unwrap();
        std::fs::write(tmp.path().join("index.html"), "<!doctype html>x").unwrap();
        let app = build_preview_app(tmp.path().to_path_buf(), None);
        let req = Request::builder()
            .uri("/public/robots.txt")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"User-agent: *\n");
    }

    async fn start_backend(
        app: axum::Router,
    ) -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });
        (addr, handle)
    }

    fn proxy_state_for(addr: std::net::SocketAddr) -> Arc<ProxyState> {
        let url = url::Url::parse(&format!("http://{addr}")).unwrap();
        Arc::new(ProxyState::new(url).unwrap())
    }

    #[tokio::test]
    async fn proxy_forwards_api_request_and_returns_json() {
        let backend = axum::Router::new().route(
            "/api/parts/categories",
            axum::routing::get(|| async {
                (
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    "[\"a\",\"b\"]",
                )
            }),
        );
        let (addr, handle) = start_backend(backend).await;
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("index.html"), "<!doctype html>SPA").unwrap();
        let app = build_preview_app(tmp.path().to_path_buf(), Some(proxy_state_for(addr)));
        let req = Request::builder()
            .uri("/api/parts/categories")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let ct = resp.headers().get("content-type").unwrap();
        assert_eq!(ct.to_str().unwrap(), "application/json");
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"[\"a\",\"b\"]");
        handle.abort();
        let _ = handle.await;
    }

    #[tokio::test]
    async fn proxy_404_falls_back_to_spa_index() {
        let backend = axum::Router::new(); // every path 404s
        let (addr, handle) = start_backend(backend).await;
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("index.html"), "<!doctype html>SPA-SHELL").unwrap();
        let app = build_preview_app(tmp.path().to_path_buf(), Some(proxy_state_for(addr)));
        let req = Request::builder()
            .uri("/some/client/route")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let ct = resp.headers().get("content-type").unwrap();
        assert!(ct.to_str().unwrap().contains("text/html"));
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        assert!(std::str::from_utf8(&body).unwrap().contains("SPA-SHELL"));
        handle.abort();
        let _ = handle.await;
    }

    #[tokio::test]
    async fn proxy_unreachable_returns_502() {
        // Reserve and immediately release a port so we have a closed address.
        let dead = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = dead.local_addr().unwrap();
        drop(dead);
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("index.html"), "<!doctype html>SPA").unwrap();
        let app = build_preview_app(tmp.path().to_path_buf(), Some(proxy_state_for(addr)));
        let req = Request::builder()
            .uri("/api/health")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);
    }

    #[tokio::test]
    async fn static_file_takes_precedence_over_proxy() {
        // Backend would gladly return JSON for /assets/x.js, but the local
        // file must win — preview never lets a backend override built assets.
        let backend = axum::Router::new().route(
            "/assets/x.js",
            axum::routing::get(|| async { "from-backend" }),
        );
        let (addr, handle) = start_backend(backend).await;
        let tmp = tempfile::tempdir().unwrap();
        let assets = tmp.path().join("assets");
        std::fs::create_dir_all(&assets).unwrap();
        std::fs::write(assets.join("x.js"), "from-dist").unwrap();
        std::fs::write(tmp.path().join("index.html"), "<!doctype html>x").unwrap();
        let app = build_preview_app(tmp.path().to_path_buf(), Some(proxy_state_for(addr)));
        let req = Request::builder()
            .uri("/assets/x.js")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"from-dist");
        handle.abort();
        let _ = handle.await;
    }

    #[tokio::test]
    async fn port_in_use_returns_friendly_error() {
        let occupied = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = occupied.local_addr().unwrap().port();
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("index.html"), "<!doctype html>x").unwrap();
        let cfg = zero_config::Config {
            project: zero_config::ProjectConfig {
                root: "web".to_string(),
            },
            dev: zero_config::DevConfig {
                port,
                proxy: None,
                sourcemap: true,
            },
            build: zero_config::BuildConfig {
                out: tmp.path().to_string_lossy().into_owned(),
                sourcemap: false,
            },
        };
        let res = serve_preview(&cfg).await;
        drop(occupied);
        assert!(res.is_err(), "expected error when port in use");
        let msg = res.unwrap_err().to_string().to_lowercase();
        assert!(
            msg.contains("port") && msg.contains("already in use"),
            "msg: {msg}"
        );
    }
}
