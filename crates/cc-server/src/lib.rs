// SPDX-License-Identifier: GPL-3.0-or-later
//! Comet Cast mediation server (library part).
//!
//! It only mediates: it hands out room codes and relays WebRTC offer/answer/ICE
//! and control messages between one receiver and one sender. It never stores or
//! forwards media, images or files. Rooms live in memory only.
//!
//! Not done yet (later milestones): TLS, rate limiting and bot protection for
//! `Mode::Open` (milestone M8 / measurement point 8 in docs/measurement-points.md).

mod rooms;
mod ws;

use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::routing::get;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

/// How the server is operated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Public server that anyone may use.
    /// Rate limiting and bot protection are not implemented yet (milestone M8).
    Open,
    /// Server inside an organisation, for its own devices only.
    Internal,
}

/// Server settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerConfig {
    /// Address and port to listen on.
    pub listen_addr: SocketAddr,
    /// Operating mode.
    pub mode: Mode,
    /// Optional directory with the web app, served from the same address as `/ws`.
    pub web_dir: Option<PathBuf>,
    /// A room without a sender is removed after this time.
    pub idle_room_timeout: Duration,
    /// A connection must send a valid `hello` within this time.
    pub hello_timeout: Duration,
    /// A connection that said hello but is in no room is closed after this time
    /// (counted from the moment it connected).
    pub no_room_timeout: Duration,
    /// Largest accepted WebSocket message in bytes.
    pub max_message_bytes: usize,
    /// How many messages may wait to be sent to one client before it is closed as too slow.
    pub outgoing_queue_len: usize,
}

/// UNCERTAIN: 30 minutes is a placeholder. A good value needs real usage data
/// (how long people wait with a code on screen), measured later.
pub const DEFAULT_IDLE_ROOM_TIMEOUT: Duration = Duration::from_secs(30 * 60);
/// UNCERTAIN: 5 seconds is a placeholder; to be checked on slow mobile networks.
pub const DEFAULT_HELLO_TIMEOUT: Duration = Duration::from_secs(5);
/// UNCERTAIN: 64 KiB is a placeholder; it must be checked against real SDP sizes
/// (an SDP with many ICE candidates may be several KiB) and measured.
pub const DEFAULT_MAX_MESSAGE_BYTES: usize = 64 * 1024;

/// UNCERTAIN: 60 seconds is a placeholder for how long a connection may stay
/// without creating or joining a room; to be measured with real clients.
pub const DEFAULT_NO_ROOM_TIMEOUT: Duration = Duration::from_secs(60);
/// UNCERTAIN: 256 queued messages is a placeholder; the right size depends on
/// how bursty ICE candidates and control messages are in practice.
pub const DEFAULT_OUTGOING_QUEUE_LEN: usize = 256;

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            // 127.0.0.1 = only this computer. 8443 is a placeholder port.
            // UNCERTAIN: the real default port is not decided.
            listen_addr: SocketAddr::from(([127, 0, 0, 1], 8443)),
            mode: Mode::Internal,
            web_dir: None,
            idle_room_timeout: DEFAULT_IDLE_ROOM_TIMEOUT,
            hello_timeout: DEFAULT_HELLO_TIMEOUT,
            no_room_timeout: DEFAULT_NO_ROOM_TIMEOUT,
            max_message_bytes: DEFAULT_MAX_MESSAGE_BYTES,
            outgoing_queue_len: DEFAULT_OUTGOING_QUEUE_LEN,
        }
    }
}

/// Version of this server library (from Cargo.toml).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Data shared by all connections.
#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) config: Arc<ServerConfig>,
    pub(crate) rooms: Arc<Mutex<rooms::Rooms>>,
}

/// Build the HTTP router: `/ws` (WebSocket), `/healthz`, and optionally static files.
pub fn build_app(config: ServerConfig) -> Router {
    let web_dir = config.web_dir.clone();
    let state = AppState {
        config: Arc::new(config),
        rooms: Arc::new(Mutex::new(rooms::Rooms::default())),
    };
    let mut app = Router::new()
        .route("/ws", get(ws::ws_handler))
        .route("/healthz", get(|| async { "ok" }))
        .with_state(state);
    if let Some(dir) = web_dir {
        // Anything that is not /ws or /healthz is looked up as a file in `dir`.
        app = app.fallback_service(ServeDir::new(dir));
    }
    app
}

/// Serve on an already bound listener until `shutdown` completes.
/// Tests bind port 0 (the OS picks a free port) and pass the listener in.
pub async fn run(
    listener: TcpListener,
    config: ServerConfig,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> io::Result<()> {
    axum::serve(listener, build_app(config))
        .with_graceful_shutdown(shutdown)
        .await
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
