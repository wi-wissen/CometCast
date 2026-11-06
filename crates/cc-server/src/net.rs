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

/// A network written with an IPv4-mapped IPv6 address (`::ffff:192.0.2.0/120`) becomes the
/// plain IPv4 network (`192.0.2.0/24`), because client addresses are made canonical (IPv4)
/// before they are compared. Networks shorter than `/96` are kept as they are.
pub(crate) fn canonical_net(net: IpNet) -> IpNet {
    if let IpNet::V6(n) = net
        && let Some(v4) = n.addr().to_ipv4_mapped()
        && n.prefix_len() >= 96
        && let Ok(v4net) = ipnet::Ipv4Net::new(v4, n.prefix_len() - 96)
    {
        return IpNet::V4(v4net);
    }
    net
}

/// The lists of a configuration with every network made canonical.
pub(crate) fn canonical_network(mut net: NetworkConfig) -> NetworkConfig {
    for list in [&mut net.trusted_proxies, &mut net.allow, &mut net.deny] {
        for n in list.iter_mut() {
            *n = canonical_net(*n);
        }
    }
    net
}

fn in_any(nets: &[IpNet], ip: IpAddr) -> bool {
    nets.iter().any(|n| n.contains(&ip))
}

/// The `X-Forwarded-For` header of a request from a trusted proxy is unusable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BadForwardedFor;

/// Read one hop: `1.2.3.4`, `1.2.3.4:5555`, `2001:db8::1`, `[2001:db8::1]` or
/// `[2001:db8::1]:5555`. Anything else is refused.
fn parse_hop(hop: &str) -> Option<IpAddr> {
    if let Ok(ip) = hop.parse::<IpAddr>() {
        return Some(ip);
    }
    if let Ok(sa) = hop.parse::<SocketAddr>() {
        return Some(sa.ip());
    }
    let inner = hop.strip_prefix('[')?.strip_suffix(']')?;
    inner.parse::<std::net::Ipv6Addr>().ok().map(IpAddr::V6)
}

/// Work out the client address. `xff` holds the raw values of all `X-Forwarded-For`
/// header lines (`None` = the line is not valid text).
///
/// - Peer not trusted: the header is ignored, the peer is the client.
/// - Peer trusted, no header at all: the peer itself is the client.
/// - Peer trusted, header present: it must be fully valid, otherwise `Err` (the request is
///   refused; we never guess and never fall back to the proxy's address). The hops are read
///   from the right; the first one that is not a trusted proxy is the client.
pub(crate) fn resolve_client_ip(
    peer: IpAddr,
    xff: &[Option<&str>],
    trusted_proxies: &[IpNet],
) -> Result<IpAddr, BadForwardedFor> {
    let peer = canonical(peer);
    if !in_any(trusted_proxies, peer) || xff.is_empty() {
        return Ok(peer);
    }
    let mut hops = Vec::new();
    for line in xff {
        let line = line.ok_or(BadForwardedFor)?;
        if line.trim().is_empty() {
            return Err(BadForwardedFor);
        }
        for hop in line.split(',') {
            hops.push(canonical(parse_hop(hop.trim()).ok_or(BadForwardedFor)?));
        }
    }
    // A chain made only of trusted proxies: the first (left-most) hop is the origin.
    Ok(hops
        .iter()
        .rev()
        .find(|ip| !in_any(trusted_proxies, **ip))
        .copied()
        .unwrap_or(hops[0]))
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
    let xff: Vec<Option<&str>> = req
        .headers()
        .get_all("x-forwarded-for")
        .iter()
        .map(|v| v.to_str().ok())
        .collect();
    let Ok(ip) = resolve_client_ip(peer.ip(), &xff, &state.config.network.trusted_proxies) else {
        // A broken header from a trusted proxy: refuse, do not guess.
        return (StatusCode::BAD_REQUEST, "bad X-Forwarded-For\n").into_response();
    };
    // The health check from the same computer (e.g. a Docker health check) always works.
    // Not for a request that a trusted proxy forwards for somebody else: its client address
    // differs from the peer, and that client is checked like any other.
    let peer_ip = canonical(peer.ip());
    let local_health = req.uri().path() == "/healthz" && peer_ip.is_loopback() && ip == peer_ip;
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

    fn x(list: &[&'static str]) -> Vec<Option<&'static str>> {
        list.iter().map(|s| Some(*s)).collect()
    }

    #[test]
    fn direct_peer_is_used_without_trusted_proxies() {
        // Even a believable header is ignored when the peer is not trusted.
        assert_eq!(
            resolve_client_ip(ip("198.51.100.7"), &x(&["203.0.113.9"]), &[]),
            Ok(ip("198.51.100.7"))
        );
        assert_eq!(
            resolve_client_ip(
                ip("198.51.100.7"),
                &x(&["203.0.113.9"]),
                &nets(&["10.0.0.0/8"])
            ),
            Ok(ip("198.51.100.7"))
        );
        // Even garbage is harmless then.
        assert_eq!(
            resolve_client_ip(ip("198.51.100.7"), &[None], &[]),
            Ok(ip("198.51.100.7"))
        );
    }

    #[test]
    fn header_is_read_from_the_right_through_trusted_proxies() {
        let trusted = nets(&["10.0.0.0/8"]);
        assert_eq!(
            resolve_client_ip(
                ip("10.0.0.1"),
                &x(&["6.6.6.6, 203.0.113.9, 10.0.0.2"]),
                &trusted
            ),
            Ok(ip("203.0.113.9"))
        );
        assert_eq!(
            resolve_client_ip(ip("10.0.0.1"), &x(&["6.6.6.6", "203.0.113.9"]), &trusted),
            Ok(ip("203.0.113.9"))
        );
        // Only proxies in the chain: the left-most hop.
        assert_eq!(
            resolve_client_ip(ip("10.0.0.1"), &x(&["10.0.0.3, 10.0.0.4"]), &trusted),
            Ok(ip("10.0.0.3"))
        );
        // No header at all: the proxy itself.
        assert_eq!(
            resolve_client_ip(ip("10.0.0.1"), &[], &trusted),
            Ok(ip("10.0.0.1"))
        );
    }

    #[test]
    fn port_and_bracket_forms_are_understood() {
        let trusted = nets(&["10.0.0.0/8"]);
        for (h, want) in [
            ("203.0.113.9:5555", "203.0.113.9"),
            ("[2001:db8::1]:5555", "2001:db8::1"),
            ("[2001:db8::1]", "2001:db8::1"),
            ("2001:db8::1", "2001:db8::1"),
        ] {
            assert_eq!(
                resolve_client_ip(ip("10.0.0.1"), &x(&[h]), &trusted),
                Ok(ip(want)),
                "{h}"
            );
        }
    }

    #[test]
    fn broken_header_from_trusted_proxy_is_refused_not_guessed() {
        let trusted = nets(&["10.0.0.0/8"]);
        let bad = |lines: &[Option<&str>]| resolve_client_ip(ip("10.0.0.1"), lines, &trusted);
        assert_eq!(bad(&[None]), Err(BadForwardedFor), "not valid text");
        assert_eq!(bad(&x(&["1.2.3.4, not-an-ip"])), Err(BadForwardedFor));
        assert_eq!(bad(&x(&[""])), Err(BadForwardedFor), "empty line");
        assert_eq!(bad(&x(&["1.2.3.4,"])), Err(BadForwardedFor), "empty hop");
        assert_eq!(bad(&x(&["[2001:db8::1"])), Err(BadForwardedFor));
        assert_eq!(bad(&x(&["203.0.113.9:99999"])), Err(BadForwardedFor));
        assert_eq!(bad(&[Some("203.0.113.9"), None]), Err(BadForwardedFor));
    }

    #[test]
    fn mapped_ipv4_is_the_same_client() {
        assert_eq!(
            resolve_client_ip(ip("::ffff:198.51.100.7"), &[], &[]),
            Ok(ip("198.51.100.7"))
        );
    }

    #[test]
    fn mapped_networks_are_made_canonical() {
        let c = |s: &str| canonical_net(s.parse().unwrap()).to_string();
        assert_eq!(c("::ffff:192.0.2.0/120"), "192.0.2.0/24");
        assert_eq!(c("::ffff:192.0.2.1/128"), "192.0.2.1/32");
        assert_eq!(c("::ffff:0:0/96"), "0.0.0.0/0");
        assert_eq!(
            c("::ffff:10.0.0.0/64"),
            "::ffff:10.0.0.0/64",
            "shorter than /96 is not an IPv4 net"
        );
        assert_eq!(c("2001:db8::/32"), "2001:db8::/32");
        assert_eq!(c("10.0.0.0/8"), "10.0.0.0/8");
        // And the lists match after that.
        let n = NetworkConfig {
            deny: vec!["::ffff:192.0.2.0/120".parse().unwrap()],
            ..NetworkConfig::default()
        };
        assert!(
            is_permitted(&n, ip("192.0.2.9")),
            "uncanonical list does not match"
        );
        let n = canonical_network(n);
        assert!(!is_permitted(&n, ip("192.0.2.9")));
        // A trusted proxy given in mapped form is recognised.
        let trusted = canonical_network(NetworkConfig {
            trusted_proxies: vec!["::ffff:10.0.0.0/104".parse().unwrap()],
            ..NetworkConfig::default()
        })
        .trusted_proxies;
        assert_eq!(
            resolve_client_ip(ip("10.0.0.1"), &x(&["203.0.113.9"]), &trusted),
            Ok(ip("203.0.113.9"))
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
