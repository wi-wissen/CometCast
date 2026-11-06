// SPDX-License-Identifier: GPL-3.0-or-later
//! Short-lived TURN credentials (the "TURN REST" scheme that coturn understands with
//! `use-auth-secret`):
//!
//! - `username   = "<expiry unix time>:<id>"`
//! - `credential = base64(HMAC-SHA1(secret, username))`
//!
//! coturn recomputes the same value with the shared secret, so no user list is needed
//! and a leaked credential stops working after `ttl`. coturn itself runs as a separate
//! program (see deploy/coturn/).

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use axum::Extension;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Json, Response};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use hmac::{Hmac, KeyInit, Mac};
use rand::Rng;
use serde::Serialize;
use sha1::Sha1;

use crate::AppState;
use crate::config::TurnConfig;
use crate::net::ClientIp;

/// One entry of the JSON list; it has the shape of a browser `RTCIceServer`
/// (`ttl` is an extra field that browsers ignore).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct IceServer {
    /// `stun:` / `turn:` / `turns:` URLs.
    pub urls: Vec<String>,
    /// TURN user name (absent for plain STUN).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// TURN password (absent for plain STUN).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential: Option<String>,
    /// Seconds the credential stays valid.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl: Option<u64>,
}

/// `base64(HMAC-SHA1(secret, username))`.
pub fn credential_for(secret: &str, username: &str) -> String {
    // HMAC accepts a key of any length, so this cannot fail.
    let mut mac = <Hmac<Sha1> as KeyInit>::new_from_slice(secret.as_bytes())
        .expect("HMAC takes any key length");
    mac.update(username.as_bytes());
    BASE64.encode(mac.finalize().into_bytes())
}

/// Build the list for one client. `now_unix` and `id` are parameters so tests can fix them.
pub fn ice_servers(cfg: &TurnConfig, now_unix: u64, id: &str) -> Vec<IceServer> {
    let mut list = Vec::new();
    if !cfg.stun_urls.is_empty() {
        list.push(IceServer {
            urls: cfg.stun_urls.clone(),
            username: None,
            credential: None,
            ttl: None,
        });
    }
    if !cfg.urls.is_empty() {
        let ttl = cfg.ttl.as_secs();
        let username = format!("{}:{id}", now_unix.saturating_add(ttl));
        list.push(IceServer {
            urls: cfg.urls.clone(),
            credential: Some(credential_for(&cfg.secret.0, &username)),
            username: Some(username),
            ttl: Some(ttl),
        });
    }
    list
}

/// `GET /ice-servers`. 404 when TURN is not configured.
pub(crate) async fn ice_servers_handler(
    State(state): State<AppState>,
    Extension(ClientIp(ip)): Extension<ClientIp>,
) -> Response {
    let Some(cfg) = &state.config.turn else {
        return (StatusCode::NOT_FOUND, "TURN is not configured\n").into_response();
    };
    if let Err(deny) = state.guard.check_http(ip, Instant::now()) {
        return crate::deny_response(deny);
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    // A random id so credentials of different clients differ.
    let mut raw = [0u8; 8];
    rand::rng().fill(&mut raw);
    let id: String = raw.iter().map(|b| format!("{b:02x}")).collect();
    (
        [(header::CACHE_CONTROL, "no-store")],
        Json(ice_servers(cfg, now, &id)),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Secret;
    use std::time::Duration;

    #[test]
    fn hmac_matches_independent_python_vectors() {
        // Computed with: python3 -c "import hmac,hashlib,base64; print(base64.b64encode(
        //   hmac.new(b'test-secret', b'1700003600:abcdef0123456789', hashlib.sha1).digest()).decode())"
        assert_eq!(
            credential_for("test-secret", "1700003600:abcdef0123456789"),
            "4U2q5TZ1WsdkbQSGkl0x+bFkFeo="
        );
        // Same with key b'secret' and message b'1700000000'.
        assert_eq!(
            credential_for("secret", "1700000000"),
            "WGw37+g43pfwVUmrc9tgArn/juE="
        );
    }

    #[test]
    fn list_has_stun_and_turn_entries() {
        let cfg = TurnConfig {
            urls: vec!["turn:relay.example.org:3478?transport=udp".into()],
            stun_urls: vec!["stun:relay.example.org:3478".into()],
            secret: Secret("test-secret".into()),
            ttl: Duration::from_secs(3600),
        };
        let list = ice_servers(&cfg, 1_700_000_000, "abcdef0123456789");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].username, None);
        assert_eq!(
            list[1].username.as_deref(),
            Some("1700003600:abcdef0123456789")
        );
        assert_eq!(
            list[1].credential.as_deref(),
            Some("4U2q5TZ1WsdkbQSGkl0x+bFkFeo=")
        );
        assert_eq!(list[1].ttl, Some(3600));
        // STUN entry has no credential fields on the wire.
        let json = serde_json::to_value(&list).unwrap();
        assert_eq!(
            json[0],
            serde_json::json!({"urls":["stun:relay.example.org:3478"]})
        );
    }
}
