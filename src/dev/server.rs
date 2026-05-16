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

use crate::config::Config;
use crate::dev::files::{
    serve_root_file, serve_under, serve_under_with_sass, serve_under_with_transpile,
};
use crate::dev::headers::no_cache_layer;
use crate::dev::local::serve_local_index;
use crate::dev::proxy::{ProxyState, proxy_request};
use crate::dev::sse::{ReloadBus, sse_handler};
use crate::dev::watch;
use crate::runtime::{
    ZERO_HTTP_TYPES_BODY, ZERO_TEST_TYPES_BODY, ZERO_TYPES_BODY, http_module, runtime_module,
};

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
    let root = cwd.join(&config.project.root);
    if !root.exists() {
        anyhow::bail!(
            "configured `[project] root = {}` not found at {}; run `zero init` first",
            config.project.root,
            root.display()
        );
    }
    let root = root
        .canonicalize()
        .unwrap_or_else(|_| cwd.join(&config.project.root));

    let dot_zero = root.join(".zero");
    if let Err(e) = std::fs::create_dir_all(&dot_zero) {
        eprintln!("zero dev: failed to create .zero/: {e}");
    }
    if let Err(e) = std::fs::write(dot_zero.join("zero.d.ts"), ZERO_TYPES_BODY) {
        eprintln!("zero dev: failed to write .zero/zero.d.ts: {e}");
    }
    if let Err(e) = std::fs::write(dot_zero.join("zero-test.d.ts"), ZERO_TEST_TYPES_BODY) {
        eprintln!("zero dev: failed to write .zero/zero-test.d.ts: {e}");
    }
    if let Err(e) = std::fs::write(dot_zero.join("zero-http.d.ts"), ZERO_HTTP_TYPES_BODY) {
        eprintln!("zero dev: failed to write .zero/zero-http.d.ts: {e}");
    }

    let proxy = config
        .dev
        .proxy
        .map(|url| ProxyState::new(url).map(Arc::new))
        .transpose()?;

    let bus = Arc::new(ReloadBus::new());
    let root_for_watch = root.clone();
    let root_display = root.display().to_string();
    let bus_for_watch = bus.clone();
    let (shutdown_tx, shutdown_rx) = shutdown_watch::channel(false);
    let state = Arc::new(AppState {
        runtime: runtime_module(),
        http: http_module(),
        root,
        proxy,
        bus,
        shutdown: shutdown_rx,
        dev_sourcemap: config.dev.sourcemap,
    });
    let port = config.dev.port;

    let app = Router::new()
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
        .with_state(state);

    let addr = format!("127.0.0.1:{port}");
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            anyhow::bail!(
                "port {port} is already in use; pick a different [dev].port in zero.toml"
            );
        }
        Err(e) => anyhow::bail!("failed to bind {addr}: {e}"),
    };

    println!("zero dev — listening on http://{addr}");

    let out_dir = cwd.join(&config.build.out);
    let out_dir = out_dir.canonicalize().unwrap_or(out_dir);
    let watch_handle = watch::start(root_for_watch, out_dir, bus_for_watch)?;
    if watch_handle.is_some() {
        println!("zero dev — watching {root_display} for changes");
    }

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = tokio::signal::ctrl_c().await;
            // Tell long-lived handlers (SSE) to end their streams so the
            // in-flight connections finish and graceful shutdown completes.
            let _ = shutdown_tx.send(true);
        })
        .await?;

    drop(watch_handle);
    Ok(())
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
