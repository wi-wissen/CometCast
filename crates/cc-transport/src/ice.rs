// SPDX-License-Identifier: GPL-3.0-or-later
//! STUN/TURN server list from `GET /ice-servers` (docs/server.md).
//!
//! The server answers with a JSON array shaped like the browser's `RTCIceServer` list; `404`
//! means the server has no TURN configured. Fetching is best effort: callers that fail to get
//! a list simply connect without STUN/TURN.

use serde::{Deserialize, Deserializer};

use crate::altcha::http_status_error;
use crate::error::TransportError;
use crate::net::{ServerAddr, TlsTrust, http_get};

/// One STUN or TURN server (the fields of a browser `RTCIceServer` that we use).
#[derive(Clone, PartialEq, Eq, Deserialize)]
pub struct IceServerConfig {
    /// `stun:`, `turn:` or `turns:` URLs. A single string is accepted as a list of one.
    #[serde(deserialize_with = "one_or_many")]
    pub urls: Vec<String>,
    /// TURN user name.
    #[serde(default)]
    pub username: Option<String>,
    /// TURN password.
    #[serde(default)]
    pub credential: Option<String>,
}

impl std::fmt::Debug for IceServerConfig {
    /// The credential is a secret and must not end up in logs.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IceServerConfig")
            .field("urls", &self.urls)
            .field("username", &self.username)
            .field(
                "credential",
                &self.credential.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

/// Most servers we keep from one answer. UNCERTAIN: placeholder; the real list has a handful
/// of entries (STUN plus up to three TURN transports).
pub const MAX_ICE_SERVERS: usize = 16;
/// Most URLs we keep per server entry. UNCERTAIN: placeholder.
pub const MAX_ICE_URLS: usize = 8;
/// Longest URL, user name or credential we accept, in bytes. UNCERTAIN: placeholder.
pub const MAX_ICE_FIELD: usize = 512;

fn url_is_sane(u: &str) -> bool {
    let l = u.to_ascii_lowercase();
    u.len() <= MAX_ICE_FIELD
        && ["stun:", "stuns:", "turn:", "turns:"]
            .iter()
            .any(|p| l.starts_with(p))
        && !u.chars().any(char::is_control)
}

/// Keep only entries that can be valid and bound the list: at most [`MAX_ICE_SERVERS`]
/// entries, [`MAX_ICE_URLS`] URLs each, with `stun(s):`/`turn(s):` schemes and short fields.
/// A TURN entry without user name and credential is dropped (it would make the library
/// refuse the whole list).
fn sanitize(list: Vec<IceServerConfig>) -> Vec<IceServerConfig> {
    let mut out = Vec::new();
    for mut e in list {
        e.urls.retain(|u| url_is_sane(u));
        e.urls.truncate(MAX_ICE_URLS);
        if e.urls.is_empty() {
            continue;
        }
        let long = |v: &Option<String>| v.as_ref().is_some_and(|s| s.len() > MAX_ICE_FIELD);
        if long(&e.username) || long(&e.credential) {
            continue;
        }
        let needs_auth = e.urls.iter().any(|u| {
            let l = u.to_ascii_lowercase();
            l.starts_with("turn:") || l.starts_with("turns:")
        });
        if needs_auth && (e.username.is_none() || e.credential.is_none()) {
            continue;
        }
        out.push(e);
        if out.len() == MAX_ICE_SERVERS {
            break;
        }
    }
    out
}

fn one_or_many<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(String),
        Many(Vec<String>),
    }
    Ok(match OneOrMany::deserialize(d)? {
        OneOrMany::One(s) => vec![s],
        OneOrMany::Many(v) => v,
    })
}

/// Parse the body of `/ice-servers`.
pub fn parse_ice_servers(body: &[u8]) -> Result<Vec<IceServerConfig>, TransportError> {
    serde_json::from_slice(body)
        .map(sanitize)
        .map_err(|e| TransportError::Protocol(format!("bad /ice-servers answer: {e}")))
}

/// Ask the server for its STUN/TURN list. `Ok(vec![])` when the server has none (404).
pub async fn fetch_ice_servers(
    addr: &ServerAddr,
    trust: &TlsTrust,
) -> Result<Vec<IceServerConfig>, TransportError> {
    let resp = http_get(addr, "/ice-servers", trust).await?;
    match resp.status {
        200 => parse_ice_servers(&resp.body),
        404 => Ok(Vec::new()),
        _ => Err(http_status_error(&resp)),
    }
}

/// Like [`fetch_ice_servers`] but any failure means "no list": a missing TURN list must not
/// stop a connection that could work directly.
pub async fn fetch_ice_servers_or_none(
    addr: &ServerAddr,
    trust: &TlsTrust,
) -> Vec<IceServerConfig> {
    fetch_ice_servers(addr, trust).await.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_server_shape() {
        let body = br#"[
          {"urls": ["stun:relay.example.org:3478"]},
          {"urls": ["turn:relay.example.org:3478?transport=udp"],
           "username": "1793985394:deec920ccf0cda4a",
           "credential": "SLw5VvvBZMywXQoq1rfKEBSlcwE=", "ttl": 3600}
        ]"#;
        let list = parse_ice_servers(body).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].urls, ["stun:relay.example.org:3478"]);
        assert_eq!(list[0].username, None);
        assert_eq!(
            list[1].username.as_deref(),
            Some("1793985394:deec920ccf0cda4a")
        );
        assert_eq!(
            list[1].credential.as_deref(),
            Some("SLw5VvvBZMywXQoq1rfKEBSlcwE=")
        );
    }

    #[test]
    fn accepts_single_url_string_and_rejects_junk() {
        let list = parse_ice_servers(br#"[{"urls": "stun:a:1"}]"#).unwrap();
        assert_eq!(list[0].urls, ["stun:a:1"]);
        assert!(parse_ice_servers(b"{}").is_err());
        assert!(parse_ice_servers(b"[{\"nope\":1}]").is_err());
    }

    #[test]
    fn debug_redacts_the_credential() {
        let list = parse_ice_servers(
            br#"[{"urls": "turn:t:1", "username": "u", "credential": "TOPSECRET"}]"#,
        )
        .unwrap();
        let text = format!("{list:?}");
        assert!(!text.contains("TOPSECRET"), "{text}");
        assert!(text.contains("redacted"));
    }

    #[test]
    fn bounds_the_list_and_drops_junk_entries() {
        let mut entries = Vec::new();
        for i in 0..100 {
            entries.push(format!(r#"{{"urls": ["stun:h{i}:3478"]}}"#));
        }
        let list = parse_ice_servers(format!("[{}]", entries.join(",")).as_bytes()).unwrap();
        assert_eq!(list.len(), MAX_ICE_SERVERS);

        let many_urls: Vec<String> = (0..50).map(|i| format!("\"stun:h{i}:1\"")).collect();
        let body = format!(r#"[{{"urls": [{}]}}]"#, many_urls.join(","));
        assert_eq!(
            parse_ice_servers(body.as_bytes()).unwrap()[0].urls.len(),
            MAX_ICE_URLS
        );

        let long = "a".repeat(MAX_ICE_FIELD + 1);
        let body = format!(
            r#"[{{"urls": "stun:{long}"}}, {{"urls": "http://evil"}}, {{"urls": "turn:t:1"}},
                {{"urls": "turn:t:1", "username": "{long}", "credential": "c"}},
                {{"urls": "turns:ok:1", "username": "u", "credential": "c"}}]"#
        );
        let list = parse_ice_servers(body.as_bytes()).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].urls, ["turns:ok:1"]);
    }
}
