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

/// The user name id of one client in one time bucket: a keyed hash of the client key (IPv4
/// address, IPv6 /64) and the bucket number, so the same client gets the SAME user name for
/// every call within a bucket and coturn's per-user quota really limits that client (a fresh
/// name per call would give a fresh quota per call). The hash is keyed with the TURN secret,
/// so the id does not reveal the address.
fn client_id(secret: &str, client_key: &str, bucket: u64) -> String {
    let mut mac = <Hmac<Sha1> as KeyInit>::new_from_slice(secret.as_bytes())
        .expect("HMAC takes any key length");
    mac.update(format!("id|{client_key}|{bucket}").as_bytes());
    mac.finalize().into_bytes()[..8]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Build the list for one client from one TURN section (its own `stun_urls` included).
/// `now_unix` and `client_key` are parameters so tests can fix them. The expiry is the end of
/// the NEXT `ttl` bucket, so a credential lives at least `ttl` and less than `2 * ttl`, and all
/// calls of one client within a bucket are identical.
pub fn ice_servers(cfg: &TurnConfig, now_unix: u64, client_key: &str) -> Vec<IceServer> {
    ice_servers_for(&cfg.stun_urls, Some(cfg), now_unix, client_key)
}

/// The general form: a STUN entry for `stun_urls` (no `username`/`credential`/`ttl` on the
/// wire) when there are any, then, with `turn`, a TURN entry with REST credentials.
pub fn ice_servers_for(
    stun_urls: &[String],
    turn: Option<&TurnConfig>,
    now_unix: u64,
    client_key: &str,
) -> Vec<IceServer> {
    let mut list = Vec::new();
    if !stun_urls.is_empty() {
        list.push(IceServer {
            urls: stun_urls.to_vec(),
            username: None,
            credential: None,
            ttl: None,
        });
    }
    if let Some(cfg) = turn
        && !cfg.urls.is_empty()
    {
        let ttl = cfg.ttl.as_secs().max(1);
        let bucket = now_unix / ttl;
        let expiry = bucket.saturating_add(2).saturating_mul(ttl);
        let username = format!("{expiry}:{}", client_id(&cfg.secret.0, client_key, bucket));
        list.push(IceServer {
            urls: cfg.urls.clone(),
            credential: Some(credential_for(&cfg.secret.0, &username)),
            username: Some(username),
            ttl: Some(expiry.saturating_sub(now_unix)),
        });
    }
    list
}

/// `GET /ice-servers`. 404 when neither `[stun]` nor `[turn]` is configured. A plain STUN
/// list is public information (no credentials), so it needs no ALTCHA, but it obeys the access
/// lists and the per-address `ice_per_min` limit (a bucket of its own, not shared with `/altcha/challenge`).
pub(crate) async fn ice_servers_handler(
    State(state): State<AppState>,
    Extension(ClientIp(ip)): Extension<ClientIp>,
) -> Response {
    let stun = state.config.all_stun_urls();
    let turn = state.config.turn.as_ref();
    if stun.is_empty() && turn.is_none() {
        return (StatusCode::NOT_FOUND, "no ICE servers configured\n").into_response();
    }
    if let Err(deny) = state.guard.check_ice(ip, Instant::now()) {
        return crate::deny_response(deny);
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    // Stable per client (IPv4 / IPv6 /64) and time bucket: repeated calls do not mint new
    // user names, so coturn's user-quota applies per client.
    let key = crate::limits::limit_key(ip).to_string();
    (
        [(header::CACHE_CONTROL, "no-store")],
        Json(ice_servers_for(&stun, turn, now, &key)),
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
    fn same_client_and_bucket_get_the_same_user() {
        let cfg = TurnConfig {
            urls: vec!["turn:relay.example.org:3478".into()],
            stun_urls: vec![],
            secret: Secret("test-secret".into()),
            ttl: Duration::from_secs(3600),
        };
        let user = |now, key| ice_servers(&cfg, now, key)[0].username.clone().unwrap();
        // Same bucket, same client: identical, however often it asks.
        assert_eq!(
            user(1_700_000_000, "192.0.2.1"),
            user(1_700_000_100, "192.0.2.1")
        );
        // Another client: another user (own quota).
        assert_ne!(
            user(1_700_000_000, "192.0.2.1"),
            user(1_700_000_000, "192.0.2.2")
        );
        // Next bucket: a new user; the credential never lives shorter than ttl.
        assert_ne!(
            user(1_700_000_000, "192.0.2.1"),
            user(1_700_003_600, "192.0.2.1")
        );
        let ttl = ice_servers(&cfg, 1_700_003_599, "x")[0].ttl.unwrap();
        assert!(ttl > 3600 && ttl <= 7200, "{ttl}");
    }

    #[test]
    fn stun_only_list_has_no_credential_fields() {
        let list = ice_servers_for(&["stun:relay.example.org:3478".into()], None, 1, "k");
        assert_eq!(
            serde_json::to_value(&list).unwrap(),
            serde_json::json!([{"urls": ["stun:relay.example.org:3478"]}])
        );
        assert!(ice_servers_for(&[], None, 1, "k").is_empty());
    }

    #[test]
    fn list_has_stun_and_turn_entries() {
        let cfg = TurnConfig {
            urls: vec!["turn:relay.example.org:3478?transport=udp".into()],
            stun_urls: vec!["stun:relay.example.org:3478".into()],
            secret: Secret("test-secret".into()),
            ttl: Duration::from_secs(3600),
        };
        let list = ice_servers(&cfg, 1_700_000_000, "192.0.2.1");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].username, None);
        // bucket = 1_700_000_000 / 3600 = 472222; expiry = (472222 + 2) * 3600.
        let user = list[1].username.as_deref().unwrap();
        let (expiry, id) = user.split_once(':').unwrap();
        assert_eq!(expiry, "1700006400");
        assert_eq!(id.len(), 16);
        assert_eq!(
            list[1].credential.as_deref(),
            Some(credential_for("test-secret", user).as_str())
        );
        assert_eq!(list[1].ttl, Some(6400));
        // STUN entry has no credential fields on the wire.
        let json = serde_json::to_value(&list).unwrap();
        assert_eq!(
            json[0],
            serde_json::json!({"urls":["stun:relay.example.org:3478"]})
        );
    }
}
