// SPDX-License-Identifier: GPL-3.0-or-later
//! Comet Cast mediation server (library part): signalling, rooms, image store, security.
//!
//! No networking yet; this only holds the configuration types.
//! UNCERTAIN: the plan names axum on tokio for the HTTP/WebSocket part; it is added later.

use std::net::SocketAddr;

/// How the server is operated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Public server that anyone may use.
    Open,
    /// Server inside an organisation, for its own devices only.
    Internal,
}

/// Minimal server settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerConfig {
    /// Address and port to listen on.
    pub listen_addr: SocketAddr,
    /// Operating mode.
    pub mode: Mode,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            // 127.0.0.1 = only this computer. 8443 is a placeholder port.
            // UNCERTAIN: the real default port is not decided.
            listen_addr: SocketAddr::from(([127, 0, 0, 1], 8443)),
            mode: Mode::Internal,
        }
    }
}

/// Version of this server library (from Cargo.toml).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_local_and_internal() {
        let c = ServerConfig::default();
        assert!(c.listen_addr.ip().is_loopback());
        assert_eq!(c.mode, Mode::Internal);
    }
}
