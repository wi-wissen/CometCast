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
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
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
}
