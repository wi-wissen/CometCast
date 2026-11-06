// SPDX-License-Identifier: GPL-3.0-or-later
//! Finding the client address and checking the allow/deny lists.
//!
//! Rule: the address of the direct peer is used. Only when that peer is a trusted
//! proxy, the `X-Forwarded-For` header is read, from the right: the first address
//! that is not itself a trusted proxy is the client. Everything left of it is
//! ignored (a client can write anything there).

use std::net::{IpAddr, SocketAddr};

use axum::extract::{ConnectInfo, Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use ipnet::IpNet;

use crate::AppState;
use crate::config::NetworkConfig;

/// The address a request is counted and checked under. Put into the request
/// extensions by `access_middleware`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ClientIp(pub(crate) IpAddr);

/// IPv4 addresses written as IPv6 (`::ffff:1.2.3.4`) become plain IPv4,
/// so one client cannot appear under two names.
pub(crate) fn canonical(ip: IpAddr) -> IpAddr {
    ip.to_canonical()
}

fn in_any(nets: &[IpNet], ip: IpAddr) -> bool {
    nets.iter().any(|n| n.contains(&ip))
}

/// Work out the client address. `xff` holds the values of all `X-Forwarded-For` headers.
pub(crate) fn resolve_client_ip(peer: IpAddr, xff: &[&str], trusted_proxies: &[IpNet]) -> IpAddr {
    let peer = canonical(peer);
    if !in_any(trusted_proxies, peer) {
        return peer;
    }
    // Several header lines count like one comma-separated list.
    let hops: Vec<&str> = xff
        .iter()
        .flat_map(|v| v.split(','))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    for hop in hops.iter().rev() {
        match hop.parse::<IpAddr>() {
            Ok(ip) => {
                let ip = canonical(ip);
                if !in_any(trusted_proxies, ip) {
                    return ip;
                }
            }
            // Garbage in the chain: do not guess, fall back to the proxy itself.
            Err(_) => return peer,
        }
    }
    peer
}

/// Is this client allowed by the lists? `deny` wins over `allow`.
pub(crate) fn is_permitted(net: &NetworkConfig, ip: IpAddr) -> bool {
    if in_any(&net.deny, ip) {
        return false;
    }
    net.allow.is_empty() || in_any(&net.allow, ip)
}

/// Runs for every request: finds the client address, refuses denied clients with 403,
/// and stores the address for the handlers.
pub(crate) async fn access_middleware(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    mut req: Request,
    next: Next,
) -> Response {
    let xff: Vec<&str> = req
        .headers()
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect();
    let ip = resolve_client_ip(peer.ip(), &xff, &state.config.network.trusted_proxies);
    // The health check from the same computer (e.g. a Docker health check) always works.
    let local_health = req.uri().path() == "/healthz" && canonical(peer.ip()).is_loopback();
    if !local_health && !is_permitted(&state.config.network, ip) {
        let mut resp = (StatusCode::FORBIDDEN, "forbidden\n").into_response();
        resp.headers_mut()
            .insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
        return resp;
    }
    req.extensions_mut().insert(ClientIp(ip));
    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nets(list: &[&str]) -> Vec<IpNet> {
        list.iter().map(|s| s.parse().unwrap()).collect()
    }
    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn direct_peer_is_used_without_trusted_proxies() {
        // Even a believable header is ignored when the peer is not trusted.
        assert_eq!(
            resolve_client_ip(ip("198.51.100.7"), &["203.0.113.9"], &[]),
            ip("198.51.100.7")
        );
        assert_eq!(
            resolve_client_ip(ip("198.51.100.7"), &["203.0.113.9"], &nets(&["10.0.0.0/8"])),
            ip("198.51.100.7")
        );
    }

    #[test]
    fn header_is_read_from_the_right_through_trusted_proxies() {
        let trusted = nets(&["10.0.0.0/8"]);
        // client, then one more proxy; the spoofed left part is ignored.
        assert_eq!(
            resolve_client_ip(
                ip("10.0.0.1"),
                &["6.6.6.6, 203.0.113.9, 10.0.0.2"],
                &trusted
            ),
            ip("203.0.113.9")
        );
        // Several header lines work like one list.
        assert_eq!(
            resolve_client_ip(ip("10.0.0.1"), &["6.6.6.6", "203.0.113.9"], &trusted),
            ip("203.0.113.9")
        );
        // Only proxies in the chain: use the peer.
        assert_eq!(
            resolve_client_ip(ip("10.0.0.1"), &["10.0.0.3"], &trusted),
            ip("10.0.0.1")
        );
        // No header at all.
        assert_eq!(
            resolve_client_ip(ip("10.0.0.1"), &[], &trusted),
            ip("10.0.0.1")
        );
    }

    #[test]
    fn garbage_in_the_chain_falls_back_to_the_peer() {
        let trusted = nets(&["10.0.0.0/8"]);
        assert_eq!(
            resolve_client_ip(ip("10.0.0.1"), &["1.2.3.4, not-an-ip"], &trusted),
            ip("10.0.0.1")
        );
    }

    #[test]
    fn mapped_ipv4_is_the_same_client() {
        assert_eq!(
            resolve_client_ip(ip("::ffff:198.51.100.7"), &[], &[]),
            ip("198.51.100.7")
        );
    }

    #[test]
    fn lists_deny_wins_and_empty_allow_means_everybody() {
        let mut n = NetworkConfig::default();
        assert!(is_permitted(&n, ip("1.2.3.4")));
        n.allow = nets(&["10.0.0.0/8", "2001:db8::/32"]);
        assert!(is_permitted(&n, ip("10.1.2.3")));
        assert!(is_permitted(&n, ip("2001:db8::1")));
        assert!(!is_permitted(&n, ip("11.0.0.1")));
        n.deny = nets(&["10.9.0.0/16"]);
        assert!(!is_permitted(&n, ip("10.9.0.5")));
        assert!(is_permitted(&n, ip("10.8.0.5")));
    }
}
