// SPDX-License-Identifier: GPL-3.0-or-later
//! Reading the configuration: TOML file, then environment variables, then command line.
//! Later sources win. Every setting of the file can also be set from the outside with a
//! dotted key such as `limits.max_rooms=500` (see `apply_override`).

use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ipnet::IpNet;
use serde::Deserialize;

use crate::config::{AltchaMode, ConfigError, Mode, Secret, ServerConfig, TurnConfig};
use cc_proto::JoinMode;

/// The commented default configuration printed by `--print-default-config`.
/// A test checks that it describes exactly `ServerConfig::default()`.
pub const DEFAULT_CONFIG_TOML: &str = include_str!("default-config.toml");

/// A list given either as a TOML array or as one comma-separated text
/// (handy for environment variables).
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum StringList {
    One(String),
    Many(Vec<String>),
}

impl StringList {
    fn items(self) -> Vec<String> {
        match self {
            Self::One(s) => s
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect(),
            Self::Many(v) => v,
        }
    }
}

/// An `allowed_origins` text (one string, comma separated) that was written down but names no
/// origin: `""`, `","`, `" , "`. Treating it as "not set" would switch the Origin check off
/// without anyone noticing, so it is an error. Absent (and a TOML `[]`) still mean "not set".
fn origins_text_is_empty(text: &str) -> bool {
    StringList::One(text.to_string()).items().is_empty()
}

/// Error for an explicitly given `allowed_origins` text that names no origin. `source` says
/// where it came from (for example `--allowed-origins` or `COMETCAST_ALLOWED_ORIGINS`).
pub fn check_allowed_origins_text(source: &str, text: &str) -> Result<(), ConfigError> {
    if origins_text_is_empty(text) {
        return Err(ConfigError::new(format!(
            "{source}: {text:?} names no origin, which would silently switch the Origin check off. \
             List the exact web origins (https://host[:port], comma separated) or leave the setting out"
        )));
    }
    Ok(())
}

fn parse_nets(what: &str, list: StringList) -> Result<Vec<IpNet>, ConfigError> {
    list.items()
        .into_iter()
        .map(|s| {
            // A bare address such as "10.0.0.5" counts as a /32 (or /128).
            s.parse::<IpNet>()
                .or_else(|_| s.parse::<std::net::IpAddr>().map(IpNet::from))
                .map(crate::net::canonical_net)
                .map_err(|_| {
                    ConfigError::new(format!("{what}: {s:?} is not an address or CIDR network"))
                })
        })
        .collect()
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileTls {
    cert_file: Option<PathBuf>,
    key_file: Option<PathBuf>,
    self_signed: Option<bool>,
    self_signed_names: Option<StringList>,
    self_signed_cert_out: Option<PathBuf>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileNetwork {
    trusted_proxies: Option<StringList>,
    allow: Option<StringList>,
    deny: Option<StringList>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileLimits {
    connections_per_min: Option<u32>,
    create_room_per_min: Option<u32>,
    join_attempts_per_min: Option<u32>,
    failed_join_weight: Option<u32>,
    http_per_min: Option<u32>,
    ice_per_min: Option<u32>,
    max_rooms: Option<usize>,
    max_connections: Option<usize>,
    ban_after_failed_joins: Option<u32>,
    failed_join_window_secs: Option<u64>,
    ban_secs: Option<Vec<u64>>,
    ban_memory_secs: Option<u64>,
    max_connections_per_ip: Option<usize>,
    max_rooms_per_ip: Option<usize>,
    ping_interval_secs: Option<u64>,
    pong_timeout_secs: Option<u64>,
    paired_idle_secs: Option<u64>,
    max_room_lifetime_secs: Option<u64>,
    msg_rate_per_sec: Option<u32>,
    msg_burst: Option<u32>,
    bytes_per_sec: Option<u32>,
    bytes_burst: Option<u32>,
    max_room_relay_bytes: Option<u64>,
    max_bad_messages: Option<u32>,
    global_failed_joins_per_min: Option<u32>,
    wide_failure_factor: Option<u32>,
    max_pending_per_room: Option<usize>,
    max_pending_per_ip: Option<usize>,
    denied_join_weight: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileAltcha {
    mode: Option<AltchaMode>,
    secret: Option<String>,
    secret_file: Option<PathBuf>,
    cost: Option<u32>,
    base_counter: Option<u32>,
    max_difficulty_factor: Option<u32>,
    ttl_secs: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileTurn {
    urls: Option<StringList>,
    stun_urls: Option<StringList>,
    secret: Option<String>,
    secret_file: Option<PathBuf>,
    ttl_secs: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileStun {
    urls: Option<StringList>,
}

/// The file layout. Everything is optional; missing values keep the default of the mode.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    mode: Option<Mode>,
    listen: Option<SocketAddr>,
    web_dir: Option<PathBuf>,
    allowed_origins: Option<StringList>,
    default_join_mode: Option<JoinMode>,
    allow_open_rooms: Option<bool>,
    join_approval_secs: Option<u64>,
    idle_room_secs: Option<u64>,
    hello_secs: Option<u64>,
    no_room_secs: Option<u64>,
    max_message_bytes: Option<usize>,
    outgoing_queue_len: Option<usize>,
    tls: Option<FileTls>,
    network: Option<FileNetwork>,
    limits: Option<FileLimits>,
    altcha: Option<FileAltcha>,
    turn: Option<FileTurn>,
    stun: Option<FileStun>,
}

/// Read a secret from a file (trailing newline removed).
fn read_secret_file(path: &Path) -> Result<String, ConfigError> {
    fs::read_to_string(path)
        .map(|s| s.trim_end_matches(['\n', '\r']).to_string())
        .map_err(|e| ConfigError::new(format!("cannot read secret file {}: {e}", path.display())))
}

/// Pick `secret` or the content of `secret_file` (not both).
fn secret_from(
    what: &str,
    secret: Option<String>,
    file: Option<PathBuf>,
) -> Result<Option<String>, ConfigError> {
    match (secret, file) {
        (Some(_), Some(_)) => Err(ConfigError::new(format!(
            "{what}.secret and {what}.secret_file are both set; use only one"
        ))),
        (Some(s), None) => Ok(Some(s)),
        (None, Some(f)) => read_secret_file(&f).map(Some),
        (None, None) => Ok(None),
    }
}

fn secs(v: u64) -> Duration {
    Duration::from_secs(v)
}

impl FileConfig {
    fn into_server_config(self) -> Result<ServerConfig, ConfigError> {
        let mode = self.mode.unwrap_or(Mode::Internal);
        let mut c = ServerConfig::for_mode(mode);
        if let Some(v) = self.listen {
            c.listen_addr = v;
        }
        if let Some(v) = self.web_dir {
            // An empty path means "serve no files" (so `COMETCAST_WEB_DIR=` switches off the
            // directory that a Docker image presets); `/` and `/anything` are then 404.
            c.web_dir = if v.as_os_str().is_empty() {
                None
            } else {
                Some(v)
            };
        }
        if let Some(v) = self.allowed_origins {
            if let StringList::One(text) = &v {
                check_allowed_origins_text("allowed_origins", text)?;
            }
            c.allowed_origins = v.items();
        }
        if let Some(v) = self.default_join_mode {
            c.default_join_mode = v;
        }
        if let Some(v) = self.allow_open_rooms {
            c.allow_open_rooms = v;
        }
        if let Some(v) = self.join_approval_secs {
            c.join_approval_timeout = secs(v);
        }
        if let Some(v) = self.idle_room_secs {
            c.idle_room_timeout = secs(v);
        }
        if let Some(v) = self.hello_secs {
            c.hello_timeout = secs(v);
        }
        if let Some(v) = self.no_room_secs {
            c.no_room_timeout = secs(v);
        }
        if let Some(v) = self.max_message_bytes {
            c.max_message_bytes = v;
        }
        if let Some(v) = self.outgoing_queue_len {
            c.outgoing_queue_len = v;
        }
        if let Some(t) = self.tls {
            if let Some(v) = t.cert_file {
                c.tls.cert_file = Some(v);
            }
            if let Some(v) = t.key_file {
                c.tls.key_file = Some(v);
            }
            if let Some(v) = t.self_signed {
                c.tls.self_signed = v;
            }
            if let Some(v) = t.self_signed_names {
                c.tls.self_signed_names = v.items();
            }
            if let Some(v) = t.self_signed_cert_out {
                c.tls.self_signed_cert_out = Some(v);
            }
        }
        if let Some(n) = self.network {
            if let Some(v) = n.trusted_proxies {
                c.network.trusted_proxies = parse_nets("network.trusted_proxies", v)?;
            }
            if let Some(v) = n.allow {
                c.network.allow = parse_nets("network.allow", v)?;
            }
            if let Some(v) = n.deny {
                c.network.deny = parse_nets("network.deny", v)?;
            }
        }
        if let Some(l) = self.limits {
            let d = &mut c.limits;
            if let Some(v) = l.connections_per_min {
                d.connections_per_min = v;
            }
            if let Some(v) = l.create_room_per_min {
                d.create_room_per_min = v;
            }
            if let Some(v) = l.join_attempts_per_min {
                d.join_attempts_per_min = v;
            }
            if let Some(v) = l.failed_join_weight {
                d.failed_join_weight = v;
            }
            if let Some(v) = l.http_per_min {
                d.http_per_min = v;
            }
            if let Some(v) = l.ice_per_min {
                d.ice_per_min = v;
            }
            if let Some(v) = l.max_rooms {
                d.max_rooms = v;
            }
            if let Some(v) = l.max_connections {
                d.max_connections = v;
            }
            if let Some(v) = l.ban_after_failed_joins {
                d.ban_after_failed_joins = v;
            }
            if let Some(v) = l.failed_join_window_secs {
                d.failed_join_window = secs(v);
            }
            if let Some(v) = l.ban_secs {
                d.ban_durations = v.into_iter().map(secs).collect();
            }
            if let Some(v) = l.ban_memory_secs {
                d.ban_memory = secs(v);
            }
            if let Some(v) = l.max_connections_per_ip {
                d.max_connections_per_ip = v;
            }
            if let Some(v) = l.max_rooms_per_ip {
                d.max_rooms_per_ip = v;
            }
            if let Some(v) = l.ping_interval_secs {
                d.ping_interval = secs(v);
            }
            if let Some(v) = l.pong_timeout_secs {
                d.pong_timeout = secs(v);
            }
            if let Some(v) = l.paired_idle_secs {
                d.paired_idle = secs(v);
            }
            if let Some(v) = l.max_room_lifetime_secs {
                d.max_room_lifetime = secs(v);
            }
            if let Some(v) = l.msg_rate_per_sec {
                d.msg_rate_per_sec = v;
            }
            if let Some(v) = l.msg_burst {
                d.msg_burst = v;
            }
            if let Some(v) = l.bytes_per_sec {
                d.bytes_per_sec = v;
            }
            if let Some(v) = l.bytes_burst {
                d.bytes_burst = v;
            }
            if let Some(v) = l.max_room_relay_bytes {
                d.max_room_relay_bytes = v;
            }
            if let Some(v) = l.max_bad_messages {
                d.max_bad_messages = v;
            }
            if let Some(v) = l.global_failed_joins_per_min {
                d.global_failed_joins_per_min = v;
            }
            if let Some(v) = l.wide_failure_factor {
                d.wide_failure_factor = v;
            }
            if let Some(v) = l.max_pending_per_room {
                d.max_pending_per_room = v;
            }
            if let Some(v) = l.max_pending_per_ip {
                d.max_pending_per_ip = v;
            }
            if let Some(v) = l.denied_join_weight {
                d.denied_join_weight = v;
            }
        }
        if let Some(a) = self.altcha {
            let d = &mut c.altcha;
            if let Some(v) = a.mode {
                d.mode = v;
            }
            if let Some(s) = secret_from("altcha", a.secret, a.secret_file)? {
                d.secret = Some(Secret(s));
            }
            if let Some(v) = a.cost {
                d.cost = v;
            }
            if let Some(v) = a.base_counter {
                d.base_counter = v;
            }
            if let Some(v) = a.max_difficulty_factor {
                d.max_difficulty_factor = v;
            }
            if let Some(v) = a.ttl_secs {
                d.ttl = secs(v);
            }
        }
        if let Some(t) = self.turn {
            let secret = secret_from("turn", t.secret, t.secret_file)?
                .ok_or_else(|| ConfigError::new("[turn] needs secret or secret_file"))?;
            c.turn = Some(TurnConfig {
                urls: t.urls.map(StringList::items).unwrap_or_default(),
                stun_urls: t.stun_urls.map(StringList::items).unwrap_or_default(),
                secret: Secret(secret),
                // UNCERTAIN: 1 hour is the plan's "about 1 h".
                ttl: secs(t.ttl_secs.unwrap_or(3600)),
            });
        }
        if let Some(s) = self.stun {
            c.stun_urls = s.urls.map(StringList::items).unwrap_or_default();
        }
        c.validate()?;
        Ok(c)
    }
}

/// Turn the text of an override value into a TOML value: numbers, `true`/`false` and
/// `[...]` arrays are understood, everything else is plain text.
fn parse_override_value(text: &str) -> toml::Value {
    let attempt = format!("v = {text}");
    match toml::from_str::<toml::Table>(&attempt) {
        Ok(mut t) => t
            .remove("v")
            .unwrap_or_else(|| toml::Value::String(text.into())),
        Err(_) => toml::Value::String(text.into()),
    }
}

/// Set `a.b.c = value` inside a TOML table, creating the sub-tables on the way.
pub fn apply_override(table: &mut toml::Table, key: &str, value: &str) -> Result<(), ConfigError> {
    let parts: Vec<&str> = key.split('.').collect();
    let Some((last, parents)) = parts.split_last() else {
        return Err(ConfigError::new("empty setting name"));
    };
    if parts.iter().any(|p| p.is_empty()) {
        return Err(ConfigError::new(format!("bad setting name {key:?}")));
    }
    let mut cur = table;
    for p in parents {
        let entry = cur
            .entry((*p).to_string())
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
        cur = entry
            .as_table_mut()
            .ok_or_else(|| ConfigError::new(format!("{key:?}: {p:?} is not a section")))?;
    }
    // Secrets are always text: a secret like 1234567890123456 must not turn into a number,
    // and a type error must not print it.
    let val = if last.contains("secret") {
        toml::Value::String(value.into())
    } else {
        parse_override_value(value)
    };
    cur.insert((*last).to_string(), val);
    Ok(())
}

/// Turn environment variables into overrides.
///
/// - `COMETCAST__LIMITS__MAX_ROOMS=500` sets `limits.max_rooms` (double underscore = dot).
/// - `COMETCAST_LISTEN`, `COMETCAST_MODE`, `COMETCAST_WEB_DIR`, `COMETCAST_ALLOWED_ORIGINS` (comma-separated) are short forms.
///
/// `COMETCAST_CONFIG` (the config file path) is read by the caller, not here.
pub fn overrides_from_env(vars: impl Iterator<Item = (String, String)>) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for (name, value) in vars {
        if let Some(rest) = name.strip_prefix("COMETCAST__") {
            out.push((rest.to_ascii_lowercase().replace("__", "."), value));
        } else if let Some(short) = match name.as_str() {
            "COMETCAST_LISTEN" => Some("listen"),
            "COMETCAST_MODE" => Some("mode"),
            "COMETCAST_WEB_DIR" => Some("web_dir"),
            "COMETCAST_ALLOWED_ORIGINS" => Some("allowed_origins"),
            _ => None,
        } {
            out.push((short.to_string(), value));
        }
    }
    // Environment order is random; sort so results are repeatable.
    out.sort();
    out
}

/// Replace every secret value inside `text` with `<hidden>`.
fn redact(text: &str, secrets: &[String]) -> String {
    let mut out = text.to_string();
    for s in secrets.iter().filter(|s| !s.is_empty()) {
        out = out.replace(s.as_str(), "<hidden>");
    }
    out
}

/// Collect the values of all keys named `*secret*` of a TOML table (all levels of
/// sections), so they can be removed from error texts. Numbers are collected as text too:
/// a secret written without quotes (`secret = 1234567890123456`) is a number to TOML.
fn collect_file_secrets(table: &toml::Table, out: &mut Vec<String>) {
    for (k, v) in table {
        match v {
            toml::Value::String(s) if k.contains("secret") => out.push(s.clone()),
            toml::Value::Integer(n) if k.contains("secret") => out.push(n.to_string()),
            toml::Value::Float(n) if k.contains("secret") => out.push(n.to_string()),
            toml::Value::Table(t) => collect_file_secrets(t, out),
            _ => {}
        }
    }
}

/// Every key containing `secret` must hold text. Checked before the typed read, so a
/// wrongly typed secret (a bare number, a list, a section) gives a message that names the key
/// and never prints the value.
fn check_secret_types(table: &toml::Table, path: &str) -> Result<(), ConfigError> {
    for (k, v) in table {
        let here = if path.is_empty() {
            k.clone()
        } else {
            format!("{path}.{k}")
        };
        match v {
            toml::Value::String(_) => {}
            _ if k.contains("secret") => {
                return Err(ConfigError::new(format!(
                    "{here} must be text in quotes (the value is not shown)"
                )));
            }
            toml::Value::Table(t) => check_secret_types(t, &here)?,
            _ => {}
        }
    }
    Ok(())
}

/// The text of a TOML syntax error without the quoted source line (that line may be a
/// secret): only the position and the short reason.
fn describe_parse_error(text: &str, e: &toml::de::Error) -> String {
    match e.span() {
        Some(span) => {
            let before = &text[..span.start.min(text.len())];
            let line = before.matches('\n').count() + 1;
            let col = before.rsplit('\n').next().map_or(0, str::len) + 1;
            format!("line {line}, column {col}: {}", e.message())
        }
        None => e.message().to_string(),
    }
}

/// Build the final configuration: file (if any), then `overrides` in order.
pub fn load_config(
    file: Option<&Path>,
    overrides: &[(String, String)],
) -> Result<ServerConfig, ConfigError> {
    let mut table = match file {
        Some(path) => {
            let text = fs::read_to_string(path).map_err(|e| {
                ConfigError::new(format!("cannot read config file {}: {e}", path.display()))
            })?;
            text.parse::<toml::Table>().map_err(|e| {
                ConfigError::new(format!(
                    "{}: {}",
                    path.display(),
                    describe_parse_error(&text, &e)
                ))
            })?
        }
        None => toml::Table::new(),
    };
    if let Some(toml::Value::String(text)) = table.get("allowed_origins") {
        let source = match file {
            Some(p) => format!("{}: allowed_origins", p.display()),
            None => "allowed_origins".to_string(),
        };
        check_allowed_origins_text(&source, text)?;
    }
    // Secret values (from `--set` / environment, or the file) are removed from error texts.
    let mut secrets: Vec<String> = overrides
        .iter()
        .filter(|(k, _)| k.contains("secret"))
        .map(|(_, v)| v.clone())
        .collect();
    collect_file_secrets(&table, &mut secrets);
    let hide = |e: ConfigError| ConfigError::new(redact(&e.0, &secrets));
    for (k, v) in overrides {
        if k == "allowed_origins" && !v.trim_start().starts_with('[') {
            check_allowed_origins_text(
                "allowed_origins (environment, --allowed-origins or --set)",
                v,
            )?;
        }
        apply_override(&mut table, k, v).map_err(hide)?;
    }
    check_secret_types(&table, "").map_err(hide)?;
    let file_config: FileConfig = table
        .try_into()
        .map_err(|e| hide(ConfigError::new(format!("configuration: {e}"))))?;
    file_config.into_server_config().map_err(hide)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_text_matches_the_defaults() {
        let c = load_config_from_str(DEFAULT_CONFIG_TOML).unwrap();
        assert_eq!(c, ServerConfig::default());
    }

    fn load_config_from_str(text: &str) -> Result<ServerConfig, ConfigError> {
        let table = text.parse::<toml::Table>().unwrap();
        let f: FileConfig = table
            .try_into()
            .map_err(|e| ConfigError::new(format!("{e}")))?;
        f.into_server_config()
    }

    #[test]
    fn empty_web_dir_means_no_static_files() {
        // From the file, from `--set` and from the environment (the Docker image presets
        // COMETCAST_WEB_DIR; an empty value must switch the directory off, not serve "").
        let c = load_config_from_str("web_dir = \"\"\n").unwrap();
        assert!(c.web_dir.is_none());
        let c = load_config(None, &[("web_dir".into(), String::new())]).unwrap();
        assert!(c.web_dir.is_none());
        let o = overrides_from_env([("COMETCAST_WEB_DIR".to_string(), String::new())].into_iter());
        assert_eq!(o, vec![("web_dir".to_string(), String::new())]);
        assert!(load_config(None, &o).unwrap().web_dir.is_none());
        // A later empty override wins over an earlier real path.
        let c = load_config(
            None,
            &[
                ("web_dir".into(), "/srv/web".into()),
                ("web_dir".into(), String::new()),
            ],
        )
        .unwrap();
        assert!(c.web_dir.is_none());
        // A real path still works.
        let c = load_config(None, &[("web_dir".into(), "/srv/web".into())]).unwrap();
        assert_eq!(c.web_dir.as_deref(), Some(Path::new("/srv/web")));
    }

    #[test]
    fn ice_per_min_is_its_own_setting() {
        let open = load_config(None, &[("mode".into(), "open".into())]).unwrap();
        assert_eq!(open.limits.ice_per_min, 30);
        let c = load_config(None, &[("limits.ice_per_min".into(), "77".into())]).unwrap();
        assert_eq!(c.limits.ice_per_min, 77);
        assert_eq!(c.limits.http_per_min, 300);
        assert!(load_config(None, &[("limits.ice_per_min".into(), "0".into())]).is_err());
    }

    #[test]
    fn open_mode_has_stricter_defaults_than_internal() {
        let open = load_config(None, &[("mode".into(), "open".into())]).unwrap();
        let internal = load_config(None, &[]).unwrap();
        assert_eq!(open.altcha.mode, AltchaMode::Always);
        assert_eq!(internal.altcha.mode, AltchaMode::Optional);
        assert!(open.limits.create_room_per_min < internal.limits.create_room_per_min);
        assert!(open.limits.max_rooms < internal.limits.max_rooms);
    }

    #[test]
    fn overrides_win_over_the_file() {
        let dir = std::env::temp_dir().join(format!("cc-cfg-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.toml");
        fs::write(&file, "[limits]\nmax_rooms = 7\ncreate_room_per_min = 3\n").unwrap();
        let c = load_config(Some(&file), &[("limits.max_rooms".into(), "9".into())]).unwrap();
        assert_eq!(c.limits.max_rooms, 9);
        assert_eq!(c.limits.create_room_per_min, 3);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn env_names_map_to_dotted_keys() {
        let vars = vec![
            ("COMETCAST__LIMITS__MAX_ROOMS".to_string(), "5".to_string()),
            ("COMETCAST_LISTEN".to_string(), "0.0.0.0:9000".to_string()),
            ("OTHER".to_string(), "x".to_string()),
        ];
        let o = overrides_from_env(vars.into_iter());
        let c = load_config(None, &o).unwrap();
        assert_eq!(c.limits.max_rooms, 5);
        assert_eq!(c.listen_addr, "0.0.0.0:9000".parse().unwrap());
    }

    #[test]
    fn lists_accept_arrays_and_comma_text() {
        let c = load_config(
            None,
            &[
                (
                    "network.allow".into(),
                    "[\"10.0.0.0/8\", \"2001:db8::/32\"]".into(),
                ),
                ("network.deny".into(), "192.0.2.1, 198.51.100.0/24".into()),
            ],
        )
        .unwrap();
        assert_eq!(c.network.allow.len(), 2);
        assert_eq!(c.network.deny.len(), 2);
        assert_eq!(c.network.deny[0].to_string(), "192.0.2.1/32");
    }

    #[test]
    fn bad_input_is_refused() {
        let bad = |k: &str, v: &str| load_config(None, &[(k.into(), v.into())]).is_err();
        assert!(bad("nonsense", "1"), "unknown keys are errors");
        assert!(bad("network.allow", "not-a-net"));
        assert!(bad("limits.max_rooms", "0"));
        assert!(bad("mode", "sideways"));
        assert!(bad("tls.cert_file", "/x/cert.pem"), "cert without key");
    }

    #[test]
    fn open_mode_rules() {
        let open = |k: &str, v: &str| {
            load_config(
                None,
                &[("mode".into(), "open".into()), (k.into(), v.into())],
            )
        };
        assert!(open("altcha.mode", "optional").is_err());
        assert!(open("turn.secret", "s").is_err());
        assert!(open("altcha.mode", "always").is_ok());
    }

    #[test]
    fn turn_needs_a_secret_and_is_internal_only() {
        let c = load_config(
            None,
            &[
                ("turn.urls".into(), "turn:relay.example.org:3478".into()),
                ("turn.secret".into(), "abcdefghijklmnopqrstuvwxyz".into()),
            ],
        )
        .unwrap();
        assert_eq!(c.turn.unwrap().ttl, Duration::from_secs(3600));
        assert!(
            load_config(None, &[("turn.urls".into(), "turn:x".into())]).is_err(),
            "secret missing"
        );
    }

    #[test]
    fn stun_section_is_allowed_in_open_mode() {
        let c = load_config_from_str(
            "mode = \"open\"\n[stun]\nurls = [\"stun:relay.example.org:3478\"]\n",
        )
        .unwrap();
        assert_eq!(c.stun_urls, ["stun:relay.example.org:3478"]);
        assert!(c.turn.is_none());
        assert_eq!(c.all_stun_urls(), c.stun_urls);
        assert!(
            load_config_from_str("[stun]\nurls = [\"turn:x\"]\n").is_err(),
            "scheme"
        );
        assert!(
            load_config_from_str("[stun]\nbogus = 1\n").is_err(),
            "unknown key"
        );
    }

    #[test]
    fn secrets_are_always_text_and_never_echoed() {
        // A numeric-looking secret stays a string (and is long enough).
        let c = load_config(
            None,
            &[
                ("turn.urls".into(), "turn:x:3478".into()),
                ("turn.secret".into(), "1234567890123456".into()),
            ],
        )
        .unwrap();
        assert_eq!(c.turn.unwrap().secret.0, "1234567890123456");
        // Too short: refused, and the value is not in the message.
        let err = load_config(
            None,
            &[
                ("turn.urls".into(), "turn:x:3478".into()),
                ("turn.secret".into(), "short-secret".into()),
            ],
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("at least"), "{err}");
        assert!(!err.contains("short-secret"), "{err}");
        let err = load_config(None, &[("altcha.secret".into(), "tiny".into())])
            .unwrap_err()
            .to_string();
        assert!(!err.contains("tiny"), "{err}");
        // The empty secret is refused as well.
        assert!(load_config(None, &[("altcha.secret".into(), "".into())]).is_err());
        // A secret in a place where a list is expected is not echoed either.
        let err = load_config(
            None,
            &[(
                "network.allow.secret".into(),
                "hunter2hunter2hunter2".into(),
            )],
        )
        .unwrap_err()
        .to_string();
        assert!(!err.contains("hunter2"), "{err}");
    }

    fn write_temp(name: &str, text: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cc-cfg-{}-{name}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("c.toml");
        fs::write(&file, text).unwrap();
        file
    }

    #[test]
    fn a_numeric_or_mistyped_secret_in_the_file_is_never_printed() {
        for (name, text) in [
            (
                "int",
                "[turn]\nurls = [\"turn:x\"]\nsecret = 1234567890123456\n",
            ),
            ("float", "[altcha]\nsecret = 1234.5678\n"),
            ("list", "[altcha]\nsecret = [\"leaky-secret-value-1\"]\n"),
            (
                "table",
                "[altcha.secret]\nvalue = \"leaky-secret-value-2\"\n",
            ),
            ("bool", "[altcha]\nsecret_file = true\n"),
        ] {
            let file = write_temp(name, text);
            let err = load_config(Some(&file), &[]).unwrap_err().to_string();
            assert!(err.contains("secret"), "{name}: {err}");
            assert!(err.contains("must be text"), "{name}: {err}");
            for leak in ["1234567890123456", "1234.5678", "leaky-secret-value"] {
                assert!(!err.contains(leak), "{name}: {err}");
            }
            fs::remove_dir_all(file.parent().unwrap()).ok();
        }
    }

    #[test]
    fn a_syntax_error_does_not_quote_the_source_line() {
        let file = write_temp("syntax", "[altcha]\nsecret = leaky-secret-value-3 oops\n");
        let err = load_config(Some(&file), &[]).unwrap_err().to_string();
        assert!(err.contains("line 2"), "{err}");
        assert!(!err.contains("leaky-secret-value"), "{err}");
        fs::remove_dir_all(file.parent().unwrap()).ok();
    }

    #[test]
    fn mapped_ipv4_networks_become_ipv4() {
        let c = load_config(
            None,
            &[
                (
                    "network.deny".into(),
                    "::ffff:192.0.2.0/120, ::ffff:10.1.2.3".into(),
                ),
                (
                    "network.trusted_proxies".into(),
                    "::ffff:127.0.0.1/128".into(),
                ),
                ("network.allow".into(), "2001:db8::/32".into()),
            ],
        )
        .unwrap();
        assert_eq!(c.network.deny[0].to_string(), "192.0.2.0/24");
        assert_eq!(c.network.deny[1].to_string(), "10.1.2.3/32");
        assert_eq!(c.network.trusted_proxies[0].to_string(), "127.0.0.1/32");
        assert_eq!(c.network.allow[0].to_string(), "2001:db8::/32");
    }

    #[test]
    fn global_brake_above_the_event_cap_is_refused() {
        let ok = |v: &str| {
            load_config(
                None,
                &[("limits.global_failed_joins_per_min".into(), v.into())],
            )
        };
        assert!(ok("10000").is_ok());
        let err = ok("10001").unwrap_err().to_string();
        assert!(err.contains("global_failed_joins_per_min"), "{err}");
        assert!(load_config(None, &[("limits.wide_failure_factor".into(), "0".into())]).is_err());
    }

    #[test]
    fn secrets_are_hidden_in_debug_output() {
        let c = load_config(
            None,
            &[
                ("turn.urls".into(), "turn:x:3478".into()),
                ("turn.secret".into(), "very-secret-very-secret".into()),
            ],
        )
        .unwrap();
        assert!(!format!("{c:?}").contains("very-secret-very-secret"));
    }
}
