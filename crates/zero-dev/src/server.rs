//! Dev server setup and lifecycle.

use std::path::PathBuf;
use std::sync::Arc;

use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::http::header;
use axum::response::IntoResponse;
use axum::routing::get;
use tokio::sync::watch as shutdown_watch;

use zero_config::Config;
use zero_runtime::{
    ZERO_HTTP_TYPES_BODY, ZERO_TEST_TYPES_BODY, ZERO_TYPES_BODY, http_module, runtime_module,
};

use crate::files::{
    serve_root_file, serve_under, serve_under_with_sass, serve_under_with_transpile,
};
use crate::headers::no_cache_layer;
use crate::local::serve_local_index;
use crate::proxy::{ProxyState, proxy_request};
use crate::sse::{ReloadBus, sse_handler};
use crate::watch;

/// Shared state passed to dev-server handlers.
#[derive(Clone)]
pub struct AppState {
    /// Precomputed runtime module text (built once at server start).
    pub runtime: String,
    /// Precomputed `zero/http` module text.
    pub http: String,
    /// Canonicalized path to `<project-root>/<config.project.root>`.
    pub root: PathBuf,
    /// Proxy state; `None` in no-proxy (static SPA) mode.
    pub proxy: Option<Arc<ProxyState>>,
    /// Broadcast bus for dev-mode reload events.
    pub bus: Arc<ReloadBus>,
    /// Set to `true` on shutdown so long-lived handlers (e.g. SSE) can end
    /// their streams and let graceful shutdown complete.
    pub shutdown: shutdown_watch::Receiver<bool>,
    /// Whether `.ts` responses should include an inline source map.
    pub dev_sourcemap: bool,
}

/// Start the dev server and block until shutdown.
///
/// # Parameters
/// - `config`: the validated `zero.toml` configuration.
///
/// # Returns
/// `Ok(())` on graceful shutdown, an error on bind or runtime failure.
pub async fn serve(config: Config) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let root = resolve_project_root(&cwd, &config.project.root)?;
    write_type_decls(&root.join(".zero"));

    let proxy = config
        .dev
        .proxy
        .clone()
        .map(|url| ProxyState::new(url).map(Arc::new))
        .transpose()?;
    let bus = Arc::new(ReloadBus::new());
    let (shutdown_tx, shutdown_rx) = shutdown_watch::channel(false);
    let state = Arc::new(AppState {
        runtime: runtime_module(),
        http: http_module(),
        root: root.clone(),
        proxy,
        bus: bus.clone(),
        shutdown: shutdown_rx,
        dev_sourcemap: config.dev.sourcemap,
    });

    let listener = bind_listener(config.dev.port).await?;
    println!("zero dev — listening on http://{}", listener.local_addr()?);

    let out_dir = cwd.join(&config.build.out);
    let out_dir = out_dir.canonicalize().unwrap_or(out_dir);
    let watch_handle = watch::start(root.clone(), out_dir, bus)?;
    if watch_handle.is_some() {
        println!("zero dev — watching {} for changes", root.display());
    }

    axum::serve(listener, build_app(state))
        .with_graceful_shutdown(async move {
            let _ = tokio::signal::ctrl_c().await;
            let _ = shutdown_tx.send(true);
        })
        .await?;
    drop(watch_handle);
    Ok(())
}

/// Resolve the project root from `cwd` + `config.project.root`; verify it
/// exists; canonicalize if possible.
fn resolve_project_root(cwd: &std::path::Path, project_root: &str) -> anyhow::Result<PathBuf> {
    let root = cwd.join(project_root);
    if !root.exists() {
        anyhow::bail!(
            "configured `[project] root = {}` not found at {}; run `zero init` first",
            project_root,
            root.display()
        );
    }
    Ok(root
        .canonicalize()
        .unwrap_or_else(|_| cwd.join(project_root)))
}

/// Write the four ambient `*.d.ts` files into `dot_zero`. Failures are warned
/// but non-fatal — the dev server still works without them.
fn write_type_decls(dot_zero: &std::path::Path) {
    if let Err(e) = std::fs::create_dir_all(dot_zero) {
        eprintln!("zero dev: failed to create .zero/: {e}");
    }
    for (name, body) in [
        ("zero.d.ts", ZERO_TYPES_BODY),
        ("zero-test.d.ts", ZERO_TEST_TYPES_BODY),
        ("zero-http.d.ts", ZERO_HTTP_TYPES_BODY),
    ] {
        if let Err(e) = std::fs::write(dot_zero.join(name), body) {
            eprintln!("zero dev: failed to write .zero/{name}: {e}");
        }
    }
}

/// Bind a TCP listener on `127.0.0.1:<port>`, with a friendly message if the
/// port is already in use.
pub(crate) async fn bind_listener(port: u16) -> anyhow::Result<tokio::net::TcpListener> {
    let addr = format!("127.0.0.1:{port}");
    match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => Ok(l),
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            anyhow::bail!("port {port} is already in use; pick a different [dev].port in zero.toml")
        }
        Err(e) => anyhow::bail!("failed to bind {addr}: {e}"),
    }
}

/// `GET /zero.js` handler: respond with the precomputed runtime module.
///
/// # Parameters
/// - `state`: shared app state.
///
/// # Returns
/// A 200 response carrying the runtime as `application/javascript`.
async fn serve_runtime(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        state.runtime.clone(),
    )
}

/// `GET /zero-http.js` handler: respond with the precomputed `zero/http`
/// module body.
async fn serve_http_runtime(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8",
        )],
        state.http.clone(),
    )
}

/// Build the dev-server `Router` from shared state.
///
/// Extracted so the route table can be exercised in unit tests without
/// binding a listener or writing the `.zero/` cache directory.
///
/// # Parameters
/// - `state`: shared `AppState`.
///
/// # Returns
/// A `Router` configured with all dev-mode routes and the no-cache layer.
pub(crate) fn build_app(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/_zero/events", get(sse_handler))
        .route("/zero.js", get(serve_runtime))
        .route("/zero-http.js", get(serve_http_runtime))
        .route(
            "/src/*path",
            get(
                |State(s): State<Arc<AppState>>, Path(p): Path<String>| async move {
                    serve_under_with_transpile(
                        s.root.join("src"),
                        "/src",
                        &format!("/src/{p}"),
                        s.dev_sourcemap,
                    )
                    .await
                },
            ),
        )
        .route(
            "/styles/*path",
            get(
                |State(s): State<Arc<AppState>>, Path(p): Path<String>| async move {
                    serve_under_with_sass(
                        s.root.join("styles"),
                        "/styles",
                        &format!("/styles/{p}"),
                        s.dev_sourcemap,
                    )
                    .await
                },
            ),
        )
        .route(
            "/public/*path",
            get(
                |State(s): State<Arc<AppState>>, Path(p): Path<String>| async move {
                    serve_under(s.root.join("public"), "/public", &format!("/public/{p}")).await
                },
            ),
        )
        .route(
            "/.zero/components/*path",
            get(
                |State(s): State<Arc<AppState>>, Path(p): Path<String>| async move {
                    serve_under_with_transpile(
                        s.root.join(".zero").join("components"),
                        "/.zero/components",
                        &format!("/.zero/components/{p}"),
                        s.dev_sourcemap,
                    )
                    .await
                },
            ),
        )
        .route(
            "/.zero/fonts/*path",
            get(
                |State(s): State<Arc<AppState>>, Path(p): Path<String>| async move {
                    serve_under(
                        s.root.join(".zero").join("fonts"),
                        "/.zero/fonts",
                        &format!("/.zero/fonts/{p}"),
                    )
                    .await
                },
            ),
        )
        .route(
            "/favicon.ico",
            get(|State(s): State<Arc<AppState>>| async move {
                serve_root_file(s.root.clone(), "favicon.ico").await
            }),
        )
        .route(
            "/robots.txt",
            get(|State(s): State<Arc<AppState>>| async move {
                serve_root_file(s.root.clone(), "robots.txt").await
            }),
        )
        .fallback(
            |State(s): State<Arc<AppState>>,
             req: axum::http::Request<axum::body::Body>| async move {
                let app_entry_href = if s.root.join("src").join("app.ts").is_file() {
                    "/src/app.ts"
                } else {
                    "/src/app.js"
                };
                match s.proxy.as_deref() {
                    Some(ps) => {
                        proxy_request(&ps.proxy_base, &ps.client, req, app_entry_href).await
                    }
                    None => {
                        let _ = req;
                        serve_local_index(s.root.clone()).await
                    }
                }
            },
        )
        .layer(no_cache_layer())
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn make_state(root: PathBuf) -> Arc<AppState> {
        let (_tx, rx) = shutdown_watch::channel(false);
        Arc::new(AppState {
            runtime: "/* runtime */".to_string(),
            http: "/* http */".to_string(),
            root,
            proxy: None,
            bus: Arc::new(ReloadBus::new()),
            shutdown: rx,
            dev_sourcemap: false,
        })
    }

    #[tokio::test]
    async fn zero_js_route_serves_runtime_body() {
        let tmp = tempfile::tempdir().unwrap();
        let state = make_state(tmp.path().to_path_buf());
        let app = build_app(state);
        let req = Request::builder()
            .uri("/zero.js")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"/* runtime */");
    }

    #[tokio::test]
    async fn zero_http_js_route_serves_http_runtime_body() {
        let tmp = tempfile::tempdir().unwrap();
        let state = make_state(tmp.path().to_path_buf());
        let app = build_app(state);
        let req = Request::builder()
            .uri("/zero-http.js")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"/* http */");
    }

    #[tokio::test]
    async fn sse_endpoint_is_routed() {
        let tmp = tempfile::tempdir().unwrap();
        let state = make_state(tmp.path().to_path_buf());
        let app = build_app(state);
        let req = Request::builder()
            .uri("/_zero/events")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        // SSE handler returns 200 with an event-stream content-type.
        assert_eq!(resp.status(), StatusCode::OK);
        let ct = resp.headers().get("content-type").unwrap();
        assert!(
            ct.to_str().unwrap().contains("text/event-stream"),
            "ct: {ct:?}"
        );
    }

    #[tokio::test]
    async fn fonts_route_serves_woff2_with_correct_content_type() {
        let tmp = tempfile::tempdir().unwrap();
        let fonts_dir = tmp.path().join(".zero").join("fonts");
        std::fs::create_dir_all(&fonts_dir).unwrap();
        let body = b"\x77OF2\x00\x00\x00\x01stub-woff2-bytes";
        std::fs::write(fonts_dir.join("Geist-VariableFont_wght.woff2"), body).unwrap();
        let state = make_state(tmp.path().to_path_buf());
        let app = build_app(state);
        let req = Request::builder()
            .uri("/.zero/fonts/Geist-VariableFont_wght.woff2")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.headers().get("content-type").unwrap(), "font/woff2");
        let got = resp.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(got.as_ref(), body);
    }

    #[tokio::test]
    async fn fonts_route_returns_404_for_missing_font() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".zero").join("fonts")).unwrap();
        let state = make_state(tmp.path().to_path_buf());
        let app = build_app(state);
        let req = Request::builder()
            .uri("/.zero/fonts/nope.woff2")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn src_route_falls_through_to_404_for_missing_file() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("src")).unwrap();
        let state = make_state(tmp.path().to_path_buf());
        let app = build_app(state);
        let req = Request::builder()
            .uri("/src/missing.ts")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn fallback_serves_local_index_when_no_proxy() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("index.html"),
            "<!doctype html><html><head></head><body></body></html>",
        )
        .unwrap();
        let state = make_state(tmp.path().to_path_buf());
        let app = build_app(state);
        let req = Request::builder()
            .uri("/some/unknown/path")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let s = std::str::from_utf8(&body).unwrap();
        assert!(s.contains("/src/app.js"), "body: {s}");
    }

    #[tokio::test]
    async fn no_cache_layer_is_applied_to_all_responses() {
        let tmp = tempfile::tempdir().unwrap();
        let state = make_state(tmp.path().to_path_buf());
        let app = build_app(state);
        let req = Request::builder()
            .uri("/zero.js")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        let cc = resp.headers().get("cache-control").unwrap();
        assert!(cc.to_str().unwrap().contains("no-store"), "cc: {cc:?}");
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn serve_returns_error_when_root_missing() {
        static CWD_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = CWD_LOCK.lock().unwrap();
        // Build a Config pointing at a non-existent root subdirectory.
        let tmp = tempfile::tempdir().unwrap();
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        let cfg = zero_config::Config {
            project: zero_config::ProjectConfig {
                root: "missing_dir".to_string(),
            },
            dev: zero_config::DevConfig {
                port: 0,
                proxy: None,
                sourcemap: true,
            },
            build: zero_config::BuildConfig {
                out: "dist".to_string(),
                sourcemap: false,
            },
        };
        let res = serve(cfg).await;
        std::env::set_current_dir(prev).unwrap();
        assert!(res.is_err(), "expected error when root missing");
        let msg = res.unwrap_err().to_string();
        assert!(msg.contains("not found"), "msg: {msg}");
    }
}
