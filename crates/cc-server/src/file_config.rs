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

fn parse_nets(what: &str, list: StringList) -> Result<Vec<IpNet>, ConfigError> {
    list.items()
        .into_iter()
        .map(|s| {
            // A bare address such as "10.0.0.5" counts as a /32 (or /128).
            s.parse::<IpNet>()
                .or_else(|_| s.parse::<std::net::IpAddr>().map(IpNet::from))
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
    max_rooms: Option<usize>,
    max_connections: Option<usize>,
    ban_after_failed_joins: Option<u32>,
    failed_join_window_secs: Option<u64>,
    ban_secs: Option<Vec<u64>>,
    ban_memory_secs: Option<u64>,
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

/// The file layout. Everything is optional; missing values keep the default of the mode.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    mode: Option<Mode>,
    listen: Option<SocketAddr>,
    web_dir: Option<PathBuf>,
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
            c.web_dir = Some(v);
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
    cur.insert((*last).to_string(), parse_override_value(value));
    Ok(())
}

/// Turn environment variables into overrides.
///
/// - `COMETCAST__LIMITS__MAX_ROOMS=500` sets `limits.max_rooms` (double underscore = dot).
/// - `COMETCAST_LISTEN`, `COMETCAST_MODE`, `COMETCAST_WEB_DIR` are short forms.
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
            _ => None,
        } {
            out.push((short.to_string(), value));
        }
    }
    // Environment order is random; sort so results are repeatable.
    out.sort();
    out
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
            text.parse::<toml::Table>()
                .map_err(|e| ConfigError::new(format!("{}: {e}", path.display())))?
        }
        None => toml::Table::new(),
    };
    for (k, v) in overrides {
        apply_override(&mut table, k, v)?;
    }
    let file_config: FileConfig = table
        .try_into()
        .map_err(|e| ConfigError::new(format!("configuration: {e}")))?;
    file_config.into_server_config()
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
                ("turn.secret".into(), "abc".into()),
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
    fn secrets_are_hidden_in_debug_output() {
        let c = load_config(
            None,
            &[
                ("turn.urls".into(), "turn:x:3478".into()),
                ("turn.secret".into(), "very-secret".into()),
            ],
        )
        .unwrap();
        assert!(!format!("{c:?}").contains("very-secret"));
    }
}
