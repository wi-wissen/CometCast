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
mod file_config;
mod health;
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
    DEFAULT_WIDE_FAILURE_FACTOR, LimitsConfig, Mode, NetworkConfig, Secret, ServerConfig,
    TlsConfig, TurnConfig,
};
pub use file_config::{DEFAULT_CONFIG_TOML, load_config, overrides_from_env};
pub use health::healthcheck;
pub use turn::{IceServer, credential_for, ice_servers};

/// Version of this server library (from Cargo.toml).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Data shared by all connections.
#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) config: Arc<ServerConfig>,
    pub(crate) rooms: Arc<Mutex<rooms::Rooms>>,
    pub(crate) guard: Arc<limits::Guard>,
    pub(crate) puzzle: Arc<puzzle::Puzzle>,
}

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
/// `/ice-servers` (only with TURN configured), and optionally static files.
///
/// The router needs the peer address of each connection, so serve it with
/// `into_make_service_with_connect_info::<SocketAddr>()` (as [`run`] does).
pub fn build_app(mut config: ServerConfig) -> Router {
    // Networks written as IPv4-mapped IPv6 (`::ffff:a.b.c.d/N`) are turned into IPv4 ones,
    // because client addresses are compared in their IPv4 form.
    config.network = net::canonical_network(config.network);
    let web_dir = config.web_dir.clone();
    let state = AppState {
        guard: limits::Guard::new(config.limits.clone()),
        puzzle: Arc::new(puzzle::Puzzle::new(&config.altcha)),
        config: Arc::new(config),
        rooms: Arc::new(Mutex::new(rooms::Rooms::default())),
    };
    let mut app = Router::new()
        .route("/ws", get(ws::ws_handler))
        .route("/healthz", get(|| async { "ok" }))
        .route("/altcha/challenge", get(puzzle::challenge_handler))
        .route("/ice-servers", get(turn::ice_servers_handler));
    if let Some(dir) = web_dir {
        // Anything that is not one of the routes above is looked up as a file in `dir`.
        app = app.fallback_service(ServeDir::new(dir));
    }
    // The access check (allow/deny lists, client address) wraps everything, files included.
    app.layer(middleware::from_fn_with_state(
        state.clone(),
        net::access_middleware,
    ))
    .with_state(state)
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
    let app = build_app(config).into_make_service_with_connect_info::<SocketAddr>();
    match tls {
        None => {
            axum::serve(listener, app)
                .with_graceful_shutdown(shutdown)
                .await
        }
        Some(tls) => {
            let handle = axum_server::Handle::new();
            let for_shutdown = handle.clone();
            tokio::spawn(async move {
                shutdown.await;
                for_shutdown.graceful_shutdown(Some(Duration::from_secs(5)));
            });
            axum_server::from_tcp_rustls(listener.into_std()?, tls)?
                .handle(handle)
                .serve(app)
                .await
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
