// SPDX-License-Identifier: GPL-3.0-or-later
//! Settings of the server: the plain Rust structs. Reading them from a TOML file,
//! environment variables and the command line is in `file_config.rs`.
//!
//! Every number below is a placeholder. UNCERTAIN: none of them is measured yet.

use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use cc_proto::JoinMode;
use ipnet::IpNet;
use serde::Deserialize;

/// How the server is operated.
///
/// The differences are listed in docs/server.md ("Open and internal mode").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Public server that anyone may use. Mediation only: ALTCHA is always required
    /// to create a room, limits are strict, no TURN credentials are issued (a plain STUN list may be).
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
    /// Requests per minute to `/altcha/challenge`.
    pub http_per_min: u32,
    /// Requests per minute to `/ice-servers`. Its own bucket: a client that fetches the ICE
    /// list on every connect must not use up the budget of the ALTCHA puzzles, and a 429 on
    /// the ICE list silently costs the client its STUN server.
    pub ice_per_min: u32,
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
    /// Most WebSocket connections open at the same time from one client address.
    pub max_connections_per_ip: usize,
    /// Most rooms one client address may own at the same time.
    pub max_rooms_per_ip: usize,
    /// The server pings every connection this often.
    pub ping_interval: Duration,
    /// A connection that sent nothing (no pong either) for `ping_interval + pong_timeout`
    /// is closed.
    pub pong_timeout: Duration,
    /// A connection in a room is closed when nothing was relayed for this long AND it
    /// has not answered the server's WebSocket pings for `ping_interval + pong_timeout`.
    /// A connection that keeps answering pings is alive, however quiet the signalling
    /// channel is (media goes peer to peer, so quiet is normal during a presentation).
    pub paired_idle: Duration,
    /// Longest life of any room, however busy and however alive its connections are.
    /// This is a hard cap; clients see the room end (`peer_left` / closed connection)
    /// even if the video is still running.
    pub max_room_lifetime: Duration,
    /// Messages per second one connection may send (refill rate).
    pub msg_rate_per_sec: u32,
    /// Messages one connection may send in a burst.
    pub msg_burst: u32,
    /// Bytes per second one connection may send (refill rate).
    pub bytes_per_sec: u32,
    /// Bytes one connection may send in a burst (at least `max_message_bytes`).
    pub bytes_burst: u32,
    /// Most bytes relayed through one room over its whole life.
    pub max_room_relay_bytes: u64,
    /// A connection that sends this many bad messages (garbage, wrong role) is closed.
    pub max_bad_messages: u32,
    /// All failed joins of the whole server in one minute above this number
    /// slow down every join attempt (emergency brake against distributed guessing).
    pub global_failed_joins_per_min: u32,
    /// An IPv6 /48 network is banned after `ban_after_failed_joins * wide_failure_factor`
    /// failures (a /48 has many more users than a /64). UNCERTAIN: the default is a
    /// placeholder.
    pub wide_failure_factor: u32,
    /// Most senders that may wait for approval in one room at the same time.
    pub max_pending_per_room: usize,
    /// Most senders waiting for approval (in all rooms) that one client address may have.
    pub max_pending_per_ip: usize,
    /// A join request that the display device denied or let time out counts like this many
    /// failures toward a temporary ban of the sender's address (0 = never). UNCERTAIN: a
    /// everybody behind one shared address shares this budget.
    pub denied_join_weight: u32,
}

/// UNCERTAIN: factor 10 is a placeholder (the first version used 4, which is probably too
/// low for a large company or campus network that owns a whole /48).
pub const DEFAULT_WIDE_FAILURE_FACTOR: u32 = 10;

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
                ice_per_min: 30,
                max_rooms: 10_000,
                max_connections: 5_000,
                ban_after_failed_joins: 10,
                failed_join_window: mins(10),
                ban_durations: bans,
                ban_memory: mins(60),
                max_connections_per_ip: 20,
                max_rooms_per_ip: 5,
                ping_interval: Duration::from_secs(20),
                pong_timeout: Duration::from_secs(20),
                paired_idle: mins(60),
                max_room_lifetime: mins(12 * 60),
                msg_rate_per_sec: 20,
                msg_burst: 100,
                bytes_per_sec: 64 * 1024,
                bytes_burst: 256 * 1024,
                max_room_relay_bytes: 8 * 1024 * 1024,
                max_bad_messages: 20,
                global_failed_joins_per_min: 300,
                wide_failure_factor: DEFAULT_WIDE_FAILURE_FACTOR,
                max_pending_per_room: 8,
                max_pending_per_ip: 3,
                denied_join_weight: 2,
            },
            Mode::Internal => Self {
                connections_per_min: 300,
                create_room_per_min: 60,
                join_attempts_per_min: 120,
                failed_join_weight: 2,
                http_per_min: 300,
                ice_per_min: 300,
                max_rooms: 100_000,
                max_connections: 20_000,
                ban_after_failed_joins: 30,
                failed_join_window: mins(10),
                ban_durations: bans,
                ban_memory: mins(60),
                max_connections_per_ip: 500,
                max_rooms_per_ip: 100,
                ping_interval: Duration::from_secs(20),
                pong_timeout: Duration::from_secs(20),
                paired_idle: mins(4 * 60),
                max_room_lifetime: mins(24 * 60),
                msg_rate_per_sec: 50,
                msg_burst: 200,
                bytes_per_sec: 256 * 1024,
                bytes_burst: 1024 * 1024,
                max_room_relay_bytes: 64 * 1024 * 1024,
                max_bad_messages: 50,
                global_failed_joins_per_min: 1000,
                wide_failure_factor: DEFAULT_WIDE_FAILURE_FACTOR,
                max_pending_per_room: 8,
                max_pending_per_ip: 8,
                denied_join_weight: 2,
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
    /// Web origins (`https://host[:port]`, exact match) whose pages may use this relay from
    /// another site: CORS for `/ice-servers`, `/altcha/challenge`, `/healthz`, and an `Origin`
    /// check on `/ws`. Empty = same-origin only for HTTP and any `Origin` on `/ws`.
    /// `*` = any origin (internal mode only). See docs/server.md.
    pub allowed_origins: Vec<String>,
    /// Join mode of rooms whose creator did not choose one. Never `locked`.
    pub default_join_mode: JoinMode,
    /// `false` = rooms with join mode `open` are refused (operators who want approval).
    pub allow_open_rooms: bool,
    /// How long a join request waits for the display device's decision.
    pub join_approval_timeout: Duration,
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
    /// Public STUN servers handed out at `GET /ice-servers` (`[stun] urls`). Allowed in BOTH
    /// modes: STUN needs no credentials and relays no media, so open servers may offer it
    /// (TURN stays internal-only). Empty = none.
    pub stun_urls: Vec<String>,
}

/// Most STUN URLs that `[stun] urls` (and `[turn] stun_urls`) may hold.
pub const MAX_STUN_URLS: usize = 4;

/// Check one STUN URL: `stun:` or `stuns:` (lowercase), a host name, IPv4 address or
/// `[IPv6]`, and an optional `:port` (1-65535). No credentials (`@`), path, query, spaces.
/// Returns the reason when it is not acceptable.
pub fn check_stun_url(url: &str) -> Result<(), String> {
    let rest = url
        .strip_prefix("stun:")
        .or_else(|| url.strip_prefix("stuns:"))
        .ok_or("only stun: and stuns: URLs are allowed")?;
    if rest.contains('@') {
        return Err("credentials in a STUN URL are not allowed".into());
    }
    if rest.contains(['/', '?', '#']) {
        return Err("a STUN URL has no path, query or fragment (use stun:host:port)".into());
    }
    let (host, port, is_ipv6) = if let Some(inner) = rest.strip_prefix('[') {
        let (h, after) = inner.split_once(']').ok_or("missing ] in IPv6 address")?;
        if h.parse::<std::net::Ipv6Addr>().is_err() {
            return Err("bad IPv6 address".into());
        }
        let port = match after {
            "" => None,
            _ => Some(after.strip_prefix(':').ok_or("junk after ]")?),
        };
        (h, port, true)
    } else {
        match rest.split_once(':') {
            Some((h, p)) => (h, Some(p), false),
            None => (rest, None, false),
        }
    };
    if !is_ipv6 {
        let ok = !host.is_empty()
            && host.len() <= 253
            && host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
            && !host.starts_with(['.', '-'])
            && !host.ends_with('-');
        if !ok {
            return Err("bad host name".into());
        }
    }
    if let Some(p) = port
        && !(p.bytes().all(|b| b.is_ascii_digit()) && matches!(p.parse::<u16>(), Ok(n) if n >= 1))
    {
        return Err("bad port (1-65535)".into());
    }
    Ok(())
}

/// Validate a list of STUN URLs (`what` names the setting in messages).
fn check_stun_list(what: &str, urls: &[String]) -> Result<(), ConfigError> {
    if urls.len() > MAX_STUN_URLS {
        return Err(ConfigError::new(format!(
            "{what}: at most {MAX_STUN_URLS} STUN URLs"
        )));
    }
    for u in urls {
        // A URL with credentials is not echoed back (it would end up in logs).
        check_stun_url(u).map_err(|e| {
            if u.contains('@') {
                ConfigError::new(format!("{what}: {e}"))
            } else {
                ConfigError::new(format!("{what}: {u:?}: {e}"))
            }
        })?;
    }
    Ok(())
}

/// Shortest accepted secret (ALTCHA, TURN). UNCERTAIN: 16 is a placeholder; `openssl rand -hex 32`
/// makes 64 characters.
pub const MIN_SECRET_LEN: usize = 16;

/// UNCERTAIN: 30 minutes is a placeholder. A good value needs real usage data
/// (how long people wait with a code on screen), measured later.
pub const DEFAULT_IDLE_ROOM_TIMEOUT: Duration = Duration::from_secs(30 * 60);
/// UNCERTAIN: 60 seconds is a placeholder for how long a join request waits for the display
/// device's decision.
pub const DEFAULT_JOIN_APPROVAL_TIMEOUT: Duration = Duration::from_secs(60);
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
            allowed_origins: Vec::new(),
            default_join_mode: JoinMode::Open,
            allow_open_rooms: true,
            join_approval_timeout: DEFAULT_JOIN_APPROVAL_TIMEOUT,
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
            stun_urls: Vec::new(),
        }
    }

    /// All STUN URLs offered at `/ice-servers`: `[stun] urls`, then `[turn] stun_urls`
    /// (without duplicates).
    pub fn all_stun_urls(&self) -> Vec<String> {
        let mut out = self.stun_urls.clone();
        if let Some(t) = &self.turn {
            for u in &t.stun_urls {
                if !out.contains(u) {
                    out.push(u.clone());
                }
            }
        }
        out
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
            ("limits.ice_per_min", l.ice_per_min),
            ("limits.ban_after_failed_joins", l.ban_after_failed_joins),
            ("limits.msg_rate_per_sec", l.msg_rate_per_sec),
            ("limits.msg_burst", l.msg_burst),
            ("limits.bytes_per_sec", l.bytes_per_sec),
            ("limits.bytes_burst", l.bytes_burst),
            ("limits.max_bad_messages", l.max_bad_messages),
            (
                "limits.global_failed_joins_per_min",
                l.global_failed_joins_per_min,
            ),
            ("limits.wide_failure_factor", l.wide_failure_factor),
            ("limits.max_pending_per_room", l.max_pending_per_room as u32),
            ("limits.max_pending_per_ip", l.max_pending_per_ip as u32),
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
        if l.global_failed_joins_per_min as usize > crate::limits::EVENT_CAP {
            // Only this many events are remembered, so a higher number could never be reached.
            return Err(ConfigError::new(format!(
                "limits.global_failed_joins_per_min must be at most {} (the number of remembered events)",
                crate::limits::EVENT_CAP
            )));
        }
        if self.default_join_mode == JoinMode::Locked {
            return Err(ConfigError::new(
                "default_join_mode must be \"open\" or \"confirm\" (a room that is locked from the start could never be joined)",
            ));
        }
        if !self.allow_open_rooms && self.default_join_mode == JoinMode::Open {
            return Err(ConfigError::new(
                "allow_open_rooms = false needs default_join_mode = \"confirm\"",
            ));
        }
        if self.join_approval_timeout.is_zero() {
            return Err(ConfigError::new("join_approval_secs must be at least 1"));
        }
        if l.max_rooms == 0 || l.max_connections == 0 {
            return Err(ConfigError::new(
                "limits.max_rooms and limits.max_connections must be at least 1",
            ));
        }
        if l.max_connections_per_ip == 0 || l.max_rooms_per_ip == 0 || l.max_room_relay_bytes == 0 {
            return Err(ConfigError::new(
                "limits.max_connections_per_ip, max_rooms_per_ip and max_room_relay_bytes must be at least 1",
            ));
        }
        for (name, d) in [
            ("limits.ping_interval_secs", l.ping_interval),
            ("limits.pong_timeout_secs", l.pong_timeout),
            ("limits.paired_idle_secs", l.paired_idle),
            ("limits.max_room_lifetime_secs", l.max_room_lifetime),
        ] {
            if d.is_zero() {
                return Err(ConfigError::new(format!("{name} must be at least 1")));
            }
        }
        if (l.bytes_burst as usize) < self.max_message_bytes {
            return Err(ConfigError::new(
                "limits.bytes_burst must be at least max_message_bytes (or the largest message could never be sent)",
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
        if let Some(secret) = &self.altcha.secret
            && secret.0.len() < MIN_SECRET_LEN
        {
            return Err(ConfigError::new(format!(
                "altcha.secret must be at least {MIN_SECRET_LEN} characters (make one with: openssl rand -hex 32)"
            )));
        }
        check_stun_list("stun.urls", &self.stun_urls)?;
        if let Some(turn) = &self.turn {
            check_stun_list("turn.stun_urls", &turn.stun_urls)?;
            if turn.urls.is_empty() && turn.stun_urls.is_empty() {
                return Err(ConfigError::new("turn needs at least one url or stun_url"));
            }
            if turn.secret.0.len() < MIN_SECRET_LEN {
                return Err(ConfigError::new(format!(
                    "turn.secret must be at least {MIN_SECRET_LEN} characters (make one with: openssl rand -hex 32)"
                )));
            }
            if turn.ttl.is_zero() {
                return Err(ConfigError::new("turn.ttl_secs must be at least 1"));
            }
        }
        for o in &self.allowed_origins {
            let o = o.trim();
            if o == "*" {
                if self.mode == Mode::Open {
                    return Err(ConfigError::new(
                        "allowed_origins: \"*\" is not allowed in open mode (list the exact origins of the web apps, e.g. https://cometcast.de)",
                    ));
                }
            } else if crate::cors::normalize_origin(o).is_none() {
                return Err(ConfigError::new(format!(
                    "allowed_origins: {o:?} is not an origin (use scheme://host[:port] with http or https, no path and no trailing slash)"
                )));
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
