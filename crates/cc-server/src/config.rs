// SPDX-License-Identifier: GPL-3.0-or-later
//! Settings of the server: the plain Rust structs. Reading them from a TOML file,
//! environment variables and the command line is in `file_config.rs`.
//!
//! Every number below is a placeholder. UNCERTAIN: none of them is measured yet.

use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use ipnet::IpNet;
use serde::Deserialize;

/// How the server is operated.
///
/// The differences are listed in docs/server.md ("Open and internal mode").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Public server that anyone may use. Mediation only: ALTCHA is always required
    /// to create a room, limits are strict, no TURN credentials are issued.
    Open,
    /// Server inside an organisation, for its own devices only. ALTCHA is optional,
    /// TURN credentials may be issued, limits are larger.
    Internal,
}

/// A secret text (HMAC key). Printing it with `{:?}` shows `<hidden>`, so it
/// cannot end up in a log by accident.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(pub String);

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<hidden>)")
    }
}

/// Problems found while checking a configuration.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ConfigError(pub String);

impl ConfigError {
    pub(crate) fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }
}

/// HTTPS / WSS settings. With nothing set, the server speaks plain HTTP.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TlsConfig {
    /// Certificate chain in PEM format (use together with `key_file`).
    pub cert_file: Option<PathBuf>,
    /// Private key in PEM format.
    pub key_file: Option<PathBuf>,
    /// Make a throw-away self-signed certificate at start. FOR DEVELOPMENT ONLY:
    /// browsers do not trust it and it changes at every start.
    pub self_signed: bool,
    /// Host names / IP addresses written into the self-signed certificate.
    pub self_signed_names: Vec<String>,
    /// Write the generated (public) certificate here, so a tool such as
    /// `curl --cacert` can trust it.
    pub self_signed_cert_out: Option<PathBuf>,
}

impl TlsConfig {
    /// Is HTTPS switched on?
    pub fn enabled(&self) -> bool {
        self.self_signed || self.cert_file.is_some() || self.key_file.is_some()
    }
}

/// Who may talk to the server, and how the client address is found.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NetworkConfig {
    /// Reverse proxies whose `X-Forwarded-For` header is believed. Empty = the
    /// header is ignored and the direct peer address is used.
    pub trusted_proxies: Vec<IpNet>,
    /// If not empty, only these networks may connect.
    pub allow: Vec<IpNet>,
    /// These networks may never connect (wins over `allow`).
    pub deny: Vec<IpNet>,
}

/// Rate limits and bans. All "per minute" numbers are per client address
/// (IPv4 address, or IPv6 /64 network).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LimitsConfig {
    /// New WebSocket connections per minute.
    pub connections_per_min: u32,
    /// `create_room` messages per minute.
    pub create_room_per_min: u32,
    /// `join_room` messages per minute.
    pub join_attempts_per_min: u32,
    /// A failed join uses this many join attempts instead of one.
    pub failed_join_weight: u32,
    /// Requests per minute to `/altcha/challenge` and `/ice-servers`.
    pub http_per_min: u32,
    /// Most rooms at the same time (whole server).
    pub max_rooms: usize,
    /// Most open WebSocket connections at the same time (whole server).
    pub max_connections: usize,
    /// This many failed joins within `failed_join_window` cause a temporary ban.
    pub ban_after_failed_joins: u32,
    /// Window in which failed joins are counted.
    pub failed_join_window: Duration,
    /// Length of the 1st, 2nd, 3rd ... ban of the same address (the last one repeats).
    pub ban_durations: Vec<Duration>,
    /// A ban history is forgotten after this long without a new ban.
    pub ban_memory: Duration,
}

impl LimitsConfig {
    /// Defaults of one mode. UNCERTAIN: all numbers are placeholders.
    pub fn for_mode(mode: Mode) -> Self {
        let mins = |m: u64| Duration::from_secs(m * 60);
        let bans = vec![mins(1), mins(5), mins(30)];
        match mode {
            Mode::Open => Self {
                connections_per_min: 30,
                create_room_per_min: 6,
                join_attempts_per_min: 20,
                failed_join_weight: 5,
                http_per_min: 30,
                max_rooms: 10_000,
                max_connections: 5_000,
                ban_after_failed_joins: 10,
                failed_join_window: mins(10),
                ban_durations: bans,
                ban_memory: mins(60),
            },
            Mode::Internal => Self {
                connections_per_min: 300,
                create_room_per_min: 60,
                join_attempts_per_min: 120,
                failed_join_weight: 2,
                http_per_min: 300,
                max_rooms: 100_000,
                max_connections: 20_000,
                ban_after_failed_joins: 30,
                failed_join_window: mins(10),
                ban_durations: bans,
                ban_memory: mins(60),
            },
        }
    }
}

/// Does creating a room need a solved ALTCHA puzzle?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AltchaMode {
    /// Required. The only choice in `Mode::Open`.
    Always,
    /// Not required, but a payload that is sent is checked (a wrong one is refused).
    Optional,
}

/// ALTCHA proof-of-work settings (see docs/server.md).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AltchaConfig {
    /// Required or optional.
    pub mode: AltchaMode,
    /// HMAC secret that signs the puzzles. `None` = a random one is made at start
    /// (then puzzles stop working after a restart and between several server instances).
    pub secret: Option<Secret>,
    /// PBKDF2 iterations per attempt.
    pub cost: u32,
    /// Base size of the search: the client needs about `base_counter * 3 / 4` attempts
    /// on average. Multiplied by 2 for every "pressure level".
    pub base_counter: u32,
    /// The multiplier from pressure never exceeds this.
    pub max_difficulty_factor: u32,
    /// A puzzle (and a solution) is only valid for this long.
    pub ttl: Duration,
}

impl AltchaConfig {
    /// Defaults of one mode. UNCERTAIN: cost and counter are placeholders. The goal is
    /// about one second or less on the oldest target device, which is not measured yet.
    pub fn for_mode(mode: Mode) -> Self {
        Self {
            mode: match mode {
                Mode::Open => AltchaMode::Always,
                Mode::Internal => AltchaMode::Optional,
            },
            secret: None,
            cost: 1_000,
            base_counter: 1_000,
            max_difficulty_factor: 8,
            ttl: Duration::from_secs(5 * 60),
        }
    }
}

/// TURN (relay) settings. The TURN server itself (coturn) is a separate program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnConfig {
    /// `turn:` / `turns:` URLs handed to clients, e.g. `turn:relay.example.org:3478?transport=udp`.
    pub urls: Vec<String>,
    /// `stun:` URLs handed to clients (no credentials needed).
    pub stun_urls: Vec<String>,
    /// Shared secret; must equal `static-auth-secret` in coturn.
    pub secret: Secret,
    /// How long issued credentials are valid.
    pub ttl: Duration,
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
    /// HTTPS settings.
    pub tls: TlsConfig,
    /// Client address handling and allow/deny lists.
    pub network: NetworkConfig,
    /// Rate limits and bans.
    pub limits: LimitsConfig,
    /// Proof-of-work for creating rooms.
    pub altcha: AltchaConfig,
    /// Short-lived TURN credentials. `None` = not offered.
    pub turn: Option<TurnConfig>,
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

impl ServerConfig {
    /// The defaults of one mode (limits and ALTCHA differ between modes).
    pub fn for_mode(mode: Mode) -> Self {
        Self {
            // 127.0.0.1 = only this computer. 8443 is a placeholder port.
            // UNCERTAIN: the real default port is not decided.
            listen_addr: SocketAddr::from(([127, 0, 0, 1], 8443)),
            mode,
            web_dir: None,
            idle_room_timeout: DEFAULT_IDLE_ROOM_TIMEOUT,
            hello_timeout: DEFAULT_HELLO_TIMEOUT,
            no_room_timeout: DEFAULT_NO_ROOM_TIMEOUT,
            max_message_bytes: DEFAULT_MAX_MESSAGE_BYTES,
            outgoing_queue_len: DEFAULT_OUTGOING_QUEUE_LEN,
            tls: TlsConfig {
                self_signed_names: vec!["localhost".into(), "127.0.0.1".into(), "::1".into()],
                ..TlsConfig::default()
            },
            network: NetworkConfig::default(),
            limits: LimitsConfig::for_mode(mode),
            altcha: AltchaConfig::for_mode(mode),
            turn: None,
        }
    }

    /// Check that the settings make sense together.
    pub fn validate(&self) -> Result<(), ConfigError> {
        let l = &self.limits;
        let counts = [
            ("limits.connections_per_min", l.connections_per_min),
            ("limits.create_room_per_min", l.create_room_per_min),
            ("limits.join_attempts_per_min", l.join_attempts_per_min),
            ("limits.failed_join_weight", l.failed_join_weight),
            ("limits.http_per_min", l.http_per_min),
            ("limits.ban_after_failed_joins", l.ban_after_failed_joins),
            ("altcha.cost", self.altcha.cost),
            ("altcha.base_counter", self.altcha.base_counter),
            (
                "altcha.max_difficulty_factor",
                self.altcha.max_difficulty_factor,
            ),
        ];
        for (name, value) in counts {
            if value == 0 {
                return Err(ConfigError::new(format!("{name} must be at least 1")));
            }
        }
        if l.max_rooms == 0 || l.max_connections == 0 {
            return Err(ConfigError::new(
                "limits.max_rooms and limits.max_connections must be at least 1",
            ));
        }
        if l.ban_durations.is_empty() {
            return Err(ConfigError::new("limits.ban_secs must not be empty"));
        }
        if self.mode == Mode::Open && self.altcha.mode != AltchaMode::Always {
            return Err(ConfigError::new(
                "altcha.mode must be \"always\" in open mode (open servers always require ALTCHA)",
            ));
        }
        if self.mode == Mode::Open && self.turn.is_some() {
            // The plan says streams through a server (TURN/SFU) exist on internal servers only.
            return Err(ConfigError::new(
                "[turn] is only available in internal mode (open servers only mediate)",
            ));
        }
        if let Some(turn) = &self.turn {
            if turn.urls.is_empty() && turn.stun_urls.is_empty() {
                return Err(ConfigError::new("turn needs at least one url or stun_url"));
            }
            if turn.secret.0.is_empty() {
                return Err(ConfigError::new("turn.secret must not be empty"));
            }
            if turn.ttl.is_zero() {
                return Err(ConfigError::new("turn.ttl_secs must be at least 1"));
            }
        }
        let t = &self.tls;
        if t.cert_file.is_some() != t.key_file.is_some() {
            return Err(ConfigError::new(
                "tls.cert_file and tls.key_file must be set together",
            ));
        }
        if t.self_signed && t.cert_file.is_some() {
            return Err(ConfigError::new(
                "tls.self_signed cannot be combined with tls.cert_file/key_file",
            ));
        }
        if t.self_signed && t.self_signed_names.is_empty() {
            return Err(ConfigError::new("tls.self_signed_names must not be empty"));
        }
        Ok(())
    }
}

impl Default for ServerConfig {
    /// Internal mode, plain HTTP on 127.0.0.1:8443 (only this computer).
    fn default() -> Self {
        Self::for_mode(Mode::Internal)
    }
}
