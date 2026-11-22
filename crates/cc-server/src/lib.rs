// SPDX-License-Identifier: GPL-3.0-or-later
//! Comet Cast mediation server (library part).
//!
//! It only mediates: it hands out room codes and relays WebRTC offer/answer/ICE
//! and control messages between one receiver and one sender. It never stores or
//! forwards media, images or files. Rooms live in memory only.
//!
//! Milestone M8 (server side): TOML/env/CLI configuration, optional HTTPS/WSS, client
//! address handling with allow/deny lists, rate limits and temporary bans, ALTCHA
//! proof-of-work for creating rooms, and short-lived TURN credentials. See docs/server.md.

mod config;
mod conn_limit;
mod cors;
mod file_config;
mod health;
mod http_deadline;
mod limits;
mod net;
mod puzzle;
mod rooms;
mod tls;
mod turn;
mod ws;

use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::http::{HeaderValue, StatusCode, header};
use axum::middleware;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

pub use config::{
    AltchaConfig, AltchaMode, ConfigError, DEFAULT_HELLO_TIMEOUT, DEFAULT_IDLE_ROOM_TIMEOUT,
    DEFAULT_MAX_MESSAGE_BYTES, DEFAULT_NO_ROOM_TIMEOUT, DEFAULT_OUTGOING_QUEUE_LEN,
    DEFAULT_WIDE_FAILURE_FACTOR, DEFAULT_WIDE_OPEN_FACTOR, LimitsConfig, MAX_STUN_URLS, Mode,
    NetworkConfig, OUTGOING_QUEUE_BYTES, Secret, ServerConfig, TlsConfig, TurnConfig,
    check_stun_url, default_max_sockets, default_max_sockets_per_ip,
};
pub use file_config::{
    DEFAULT_CONFIG_TOML, check_allowed_origins_text, load_config, overrides_from_env,
};
pub use health::healthcheck;
pub use turn::{IceServer, credential_for, ice_servers, ice_servers_for};

/// Version of this server library (from Cargo.toml).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Data shared by all connections.
#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) config: Arc<ServerConfig>,
    pub(crate) rooms: Arc<Mutex<rooms::Rooms>>,
    pub(crate) guard: Arc<limits::Guard>,
    pub(crate) puzzle: Arc<puzzle::Puzzle>,
    /// Browser origins allowed to use the relay from another site (`allowed_origins`).
    pub(crate) origins: Arc<cors::OriginPolicy>,
    /// Becomes `true` when the server is asked to stop; open WebSockets then close.
    pub(crate) shutdown: tokio::sync::watch::Receiver<bool>,
}

/// Wait until shutdown was announced. Never returns when there is no sender (a router built
/// with [`build_app`] alone is never shut down this way).
pub(crate) async fn wait_shutdown(rx: &mut tokio::sync::watch::Receiver<bool>) {
    if rx.wait_for(|stop| *stop).await.is_err() {
        std::future::pending::<()>().await;
    }
}

/// Longest wait for open connections after shutdown was asked for (HTTP and HTTPS alike).
const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);

/// Turn a refusal into an HTTP answer.
pub(crate) fn deny_response(deny: limits::Deny) -> Response {
    let (status, text, retry) = match deny {
        limits::Deny::RateLimited(after) => {
            (StatusCode::TOO_MANY_REQUESTS, "rate_limited\n", Some(after))
        }
        limits::Deny::Banned(left) => (StatusCode::FORBIDDEN, "banned\n", Some(left)),
        limits::Deny::Busy => (StatusCode::SERVICE_UNAVAILABLE, "busy\n", None),
    };
    let mut resp = (status, text).into_response();
    if let Some(after) = retry
        && let Ok(v) = HeaderValue::from_str(&after.as_secs().max(1).to_string())
    {
        resp.headers_mut().insert(header::RETRY_AFTER, v);
    }
    resp
}

/// Build the HTTP router: `/ws` (WebSocket), `/healthz`, `/altcha/challenge`,
/// `/ice-servers` (only with `[stun]` or `[turn]` configured), and optionally static files.
///
/// The router needs the peer address of each connection, so serve it with
/// `into_make_service_with_connect_info::<SocketAddr>()` (as [`run`] does).
pub fn build_app(config: ServerConfig) -> Router {
    // The sender is dropped at once: this router never sees a shutdown announcement.
    build_app_with_shutdown(config, tokio::sync::watch::channel(false).1)
}

fn build_app_with_shutdown(
    mut config: ServerConfig,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> Router {
    // Networks written as IPv4-mapped IPv6 (`::ffff:a.b.c.d/N`) are turned into IPv4 ones,
    // because client addresses are compared in their IPv4 form.
    config.network = net::canonical_network(config.network);
    let web_dir = config.web_dir.clone();
    let state = AppState {
        guard: limits::Guard::new(config.limits.clone()),
        puzzle: Arc::new(puzzle::Puzzle::new(&config.altcha)),
        origins: Arc::new(cors::OriginPolicy::new(&config.allowed_origins)),
        config: Arc::new(config),
        rooms: Arc::new(Mutex::new(rooms::Rooms::default())),
        shutdown,
    };
    let mut app = Router::new()
        .route("/ws", get(ws::ws_handler))
        .route("/healthz", get(|| async { "ok" }))
        .route("/altcha/challenge", get(puzzle::challenge_handler))
        .route("/ice-servers", get(turn::ice_servers_handler));
    if let Some(dir) = web_dir {
        // Anything that is not one of the routes above is looked up as a file in `dir`. A path that
        // is no file and has no file extension (`/display`, a join code such as `/K7QM2W`) is a page
        // of the web app: it gets `index.html`, and the app picks the page from the path.
        let index = dir.join("index.html");
        let spa = get(move |uri: axum::http::Uri| {
            let index = index.clone();
            async move {
                let last = uri.path().rsplit('/').next().unwrap_or("");
                if last.contains('.') {
                    return StatusCode::NOT_FOUND.into_response();
                }
                match tokio::fs::read(&index).await {
                    Ok(body) => {
                        ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], body).into_response()
                    }
                    Err(_) => StatusCode::NOT_FOUND.into_response(),
                }
            }
        });
        app = app.fallback_service(ServeDir::new(dir).fallback(axum::Router::new().fallback(spa)));
    }
    // The access check (allow/deny lists, client address) wraps everything, files included.
    // CORS is outside it so that refusals (429, 403) stay readable for an allowed page. The
    // security headers are outermost: every response gets them, refusals included.
    app.layer(middleware::from_fn_with_state(
        state.clone(),
        net::access_middleware,
    ))
    .layer(middleware::from_fn_with_state(
        state.clone(),
        cors::cors_middleware,
    ))
    .layer(middleware::from_fn_with_state(
        state.clone(),
        security_headers,
    ))
    .with_state(state)
}

/// How long browsers remember to use HTTPS only (`Strict-Transport-Security`): one year.
const HSTS: &str = "max-age=31536000";

/// Adds the security headers to every response (pages, files, API answers, refusals):
///
/// - `X-Content-Type-Options: nosniff`: the browser must not guess another content type.
/// - `X-Frame-Options: DENY` and `Content-Security-Policy: frame-ancestors 'none'`: no other
///   page may show this one in a frame (clickjacking on the join approval and share buttons).
/// - `Referrer-Policy: same-origin`: a room code in the address (`/K7QM2W`) is not sent to
///   other sites.
/// - `Strict-Transport-Security`: only when this server does TLS itself. Behind a reverse
///   proxy that does TLS, the proxy is the place for it.
///
/// A header that a handler already set is kept.
async fn security_headers(
    axum::extract::State(state): axum::extract::State<AppState>,
    req: axum::extract::Request,
    next: middleware::Next,
) -> Response {
    let mut resp = next.run(req).await;
    let mut set = |name: header::HeaderName, value: &'static str| {
        resp.headers_mut()
            .entry(name)
            .or_insert(HeaderValue::from_static(value));
    };
    set(header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    set(header::X_FRAME_OPTIONS, "DENY");
    set(header::CONTENT_SECURITY_POLICY, "frame-ancestors 'none'");
    set(header::REFERRER_POLICY, "same-origin");
    if state.config.tls.enabled() {
        set(header::STRICT_TRANSPORT_SECURITY, HSTS);
    }
    resp
}

/// Serve on an already bound listener until `shutdown` completes. Uses HTTPS when
/// `config.tls` asks for it. Tests bind port 0 (the OS picks a free port) and pass
/// the listener in.
pub async fn run(
    listener: TcpListener,
    config: ServerConfig,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> io::Result<()> {
    config.validate().map_err(io::Error::other)?;
    let tls = if config.tls.enabled() {
        Some(tls::build(&config.tls)?)
    } else {
        None
    };
    // Sockets are counted from `accept` on, before TLS and before any request (conn_limit.rs).
    let sockets = conn_limit::LimitAcceptor::new(conn_limit::SocketLimiter::new(
        &config.limits,
        &config.network,
    ));
    let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
    let app = build_app_with_shutdown(config, stop_rx.clone())
        .into_make_service_with_connect_info::<SocketAddr>();
    // One task turns the caller's signal into the announcement for WebSockets (they send a
    // Close frame and end) and for the HTTP server (stop accepting, drain).
    tokio::spawn(async move {
        shutdown.await;
        let _ = stop_tx.send(true);
    });
    let mut for_http = stop_rx;
    // Both paths give open connections SHUTDOWN_GRACE after the signal, then stop anyway.
    let handle = axum_server::Handle::new();
    let for_shutdown = handle.clone();
    tokio::spawn(async move {
        wait_shutdown(&mut for_http).await;
        for_shutdown.graceful_shutdown(Some(SHUTDOWN_GRACE));
    });
    // axum-server for plain HTTP too (not `axum::serve`): it lets us give hyper a timer, so
    // the header read timeout works, and wrap each stream (see http_deadline.rs).
    let listener = listener.into_std()?;
    match tls {
        None => {
            let mut server = axum_server::from_tcp(listener)?
                .acceptor(http_deadline::FirstRequestDeadline::new(sockets))
                .handle(handle);
            http_deadline::tune(&mut server);
            server.serve(app).await
        }
        Some(tls) => {
            let mut server = axum_server::from_tcp_rustls(listener, tls)?
                // Order: count the socket, then the TLS handshake, then the request deadline.
                .map(|tls| http_deadline::FirstRequestDeadline::new(tls.acceptor(sockets)))
                .handle(handle);
            http_deadline::tune(&mut server);
            server.serve(app).await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_local_and_internal() {
        let c = ServerConfig::default();
        assert!(c.listen_addr.ip().is_loopback());
        assert_eq!(c.mode, Mode::Internal);
        assert!(c.web_dir.is_none());
    }
}
