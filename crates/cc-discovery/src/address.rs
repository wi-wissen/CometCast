// SPDX-License-Identifier: GPL-3.0-or-later
//! Checking and normalising server addresses that come from outside.
//!
//! Every address that enters the server list goes through [`normalize_server`]. Two trust
//! levels exist ([`Trust`]): an administrator (MDM policy) may name anything, while data from
//! the network (DNS-SRV answers, announcements) is untrusted and is held to stricter rules.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Longest accepted address text in bytes.
pub const MAX_SERVER_LEN: usize = 300;
/// Longest host name (RFC 1035, without the trailing dot).
const MAX_HOST_LEN: usize = 253;

/// How far the source of an address is trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trust {
    /// An administrator wrote it (MDM policy). Schemes, loopback and any IP are allowed.
    Admin,
    /// It came over the network (DNS, mDNS). No scheme; no loopback, unspecified, multicast,
    /// broadcast or link-local IPv6; an IP literal must be in a private range (public servers
    /// have to be given by name).
    Network,
}

/// Why an address was refused. The text never contains the input.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid server address: {0}")]
pub struct AddressError(pub &'static str);

/// Check `input` and return its canonical text: `[scheme://]host:port`, host in lower case,
/// IPv6 in brackets. A port is required unless an admin gave a scheme.
///
/// The result can be parsed by `cc_transport::ServerAddr::parse` (no path is allowed here).
pub fn normalize_server(input: &str, trust: Trust) -> Result<String, AddressError> {
    if input.len() > MAX_SERVER_LEN {
        return Err(AddressError("too long"));
    }
    let input = input.trim();
    if input.is_empty() {
        return Err(AddressError("empty"));
    }
    if !input.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(AddressError(
            "contains spaces, control or non-ASCII characters",
        ));
    }
    let (scheme, rest) = match input.split_once("://") {
        Some((s, r)) => {
            if trust == Trust::Network {
                return Err(AddressError("a scheme is not allowed here"));
            }
            let s = s.to_ascii_lowercase();
            if !matches!(s.as_str(), "ws" | "wss" | "http" | "https") {
                return Err(AddressError("unknown scheme"));
            }
            (Some(s), r)
        }
        None => (None, input),
    };
    if rest.contains(['/', '@', '?', '#', '\\', '%']) {
        return Err(AddressError(
            "no path, user name, query or zone id is allowed",
        ));
    }
    let (host, port) = split_host_port(rest)?;
    let port = match port {
        Some(p) => p,
        None if scheme.is_some() => 0,
        None => return Err(AddressError("a port is required")),
    };
    let host = canonical_host(host, trust)?;
    let authority = match (port, host.contains(':')) {
        (0, true) => format!("[{host}]"),
        (0, false) => host,
        (p, true) => format!("[{host}]:{p}"),
        (p, false) => format!("{host}:{p}"),
    };
    Ok(match scheme {
        Some(s) => format!("{s}://{authority}"),
        None => authority,
    })
}

fn split_host_port(s: &str) -> Result<(&str, Option<u16>), AddressError> {
    if let Some(rest) = s.strip_prefix('[') {
        let (host, after) = rest
            .split_once(']')
            .ok_or(AddressError("unterminated bracket"))?;
        let port = match after {
            "" => None,
            a => Some(parse_port(
                a.strip_prefix(':').ok_or(AddressError("bad port"))?,
            )?),
        };
        return Ok((host, port));
    }
    match s.rsplit_once(':') {
        Some((host, _)) if host.contains(':') => Err(AddressError("IPv6 needs brackets")),
        Some((host, port)) => Ok((host, Some(parse_port(port)?))),
        None => Ok((s, None)),
    }
}

fn parse_port(s: &str) -> Result<u16, AddressError> {
    if s.is_empty() || s.len() > 5 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(AddressError("bad port"));
    }
    match s.parse::<u16>() {
        Ok(p) if p != 0 => Ok(p),
        _ => Err(AddressError("port out of range")),
    }
}

fn canonical_host(host: &str, trust: Trust) -> Result<String, AddressError> {
    if host.is_empty() {
        return Err(AddressError("empty host"));
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        if trust == Trust::Network {
            check_network_ip(ip)?;
        }
        return Ok(ip.to_string());
    }
    if host.contains(':') {
        return Err(AddressError("bad IPv6 address"));
    }
    let name = validate_hostname(host)?;
    if trust == Trust::Network && (name == "localhost" || name.ends_with(".localhost")) {
        return Err(AddressError("localhost is not allowed here"));
    }
    Ok(name)
}

fn check_network_ip(ip: IpAddr) -> Result<(), AddressError> {
    let bad = |m| Err(AddressError(m));
    match ip {
        IpAddr::V4(v4) => {
            if v4.is_unspecified() || v4.is_loopback() || v4.is_multicast() || v4.is_broadcast() {
                return bad("loopback, unspecified, multicast or broadcast address");
            }
            if !is_private_v4(v4) {
                return bad("public IP addresses must be given as a host name");
            }
        }
        IpAddr::V6(v6) => {
            if v6.is_unspecified() || v6.is_loopback() || v6.is_multicast() {
                return bad("loopback, unspecified or multicast address");
            }
            if v6.to_ipv4_mapped().is_some() {
                return bad("IPv4-mapped IPv6 addresses are not allowed");
            }
            // fe80::/10 needs a zone id that we do not accept.
            if v6.segments()[0] & 0xffc0 == 0xfe80 {
                return bad("link-local IPv6 addresses need a zone id");
            }
            if !is_unique_local_v6(v6) {
                return bad("public IP addresses must be given as a host name");
            }
        }
    }
    Ok(())
}

fn is_private_v4(a: Ipv4Addr) -> bool {
    let o = a.octets();
    a.is_private() || a.is_link_local() || (o[0] == 100 && (64..128).contains(&o[1]))
}

fn is_unique_local_v6(a: Ipv6Addr) -> bool {
    a.segments()[0] & 0xfe00 == 0xfc00
}

/// Check a DNS host name (letters, digits, hyphens; labels of 1 to 63 characters; the last
/// label not all digits so that it cannot be mistaken for a malformed IPv4 address) and return
/// it in lower case without a trailing dot. Underscores are refused.
pub fn validate_hostname(name: &str) -> Result<String, AddressError> {
    let name = name.strip_suffix('.').unwrap_or(name);
    if name.is_empty() || name.len() > MAX_HOST_LEN {
        return Err(AddressError("bad host name length"));
    }
    let name = name.to_ascii_lowercase();
    for label in name.split('.') {
        let b = label.as_bytes();
        if b.is_empty() || b.len() > 63 {
            return Err(AddressError("bad host name label length"));
        }
        if !b.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'-')
            || b[0] == b'-'
            || b[b.len() - 1] == b'-'
        {
            return Err(AddressError("bad character in host name"));
        }
    }
    if name
        .rsplit('.')
        .next()
        .is_some_and(|l| l.bytes().all(|c| c.is_ascii_digit()))
    {
        return Err(AddressError(
            "last label of a host name must not be numeric",
        ));
    }
    Ok(name)
}

/// Remove control characters and surrounding blanks from text that came from the network and
/// cut it to `max_chars` characters. Used for names shown to people.
pub fn clean_text(s: &str, max_chars: usize) -> String {
    s.chars()
        .filter(|c| !c.is_control())
        .take(max_chars)
        .collect::<String>()
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(s: &str, t: Trust) -> Result<String, AddressError> {
        normalize_server(s, t)
    }

    #[test]
    fn accepts_and_canonicalises() {
        assert_eq!(
            n("Relay.Example.ORG:443", Trust::Network).unwrap(),
            "relay.example.org:443"
        );
        assert_eq!(
            n("relay.example.org.:8443", Trust::Network).unwrap(),
            "relay.example.org:8443"
        );
        assert_eq!(
            n("192.168.1.5:8443", Trust::Network).unwrap(),
            "192.168.1.5:8443"
        );
        assert_eq!(
            n("[fd00::1]:8443", Trust::Network).unwrap(),
            "[fd00::1]:8443"
        );
        assert_eq!(
            n("WSS://Relay.example.org", Trust::Admin).unwrap(),
            "wss://relay.example.org"
        );
        assert_eq!(
            n("https://[::1]:9", Trust::Admin).unwrap(),
            "https://[::1]:9"
        );
        assert_eq!(n("127.0.0.1:8443", Trust::Admin).unwrap(), "127.0.0.1:8443");
        assert_eq!(
            n("host:80\n", Trust::Admin).unwrap(),
            "host:80",
            "surrounding blanks are trimmed"
        );
    }

    #[test]
    fn network_trust_is_strict() {
        for bad in [
            "wss://a.example:443",
            "localhost:8443",
            "x.localhost:1",
            "127.0.0.1:8443",
            "0.0.0.0:1",
            "224.0.0.1:1",
            "255.255.255.255:1",
            "8.8.8.8:443",
            "[::1]:1",
            "[::]:1",
            "[ff02::1]:1",
            "[fe80::1]:1",
            "[2001:db8::1]:1",
            "[::ffff:192.168.0.1]:1",
        ] {
            assert!(n(bad, Trust::Network).is_err(), "{bad}");
        }
    }

    #[test]
    fn rejects_malformed_for_everybody() {
        let long = format!("{}.example:1", "a".repeat(64));
        let longer = "a".repeat(MAX_SERVER_LEN + 1);
        for bad in [
            "",
            "   ",
            "host",
            "host:0",
            "host:65536",
            "host:+80",
            "host:8 0",
            "host:",
            ":80",
            "a b:80",
            "host:80/ws",
            "user@host:80",
            "host:80?x",
            "host\u{0}:80",
            "höst.example:80",
            "a_b.example:80",
            "-a.example:80",
            "a-.example:80",
            "a..example:80",
            "1.2.3:80",
            "999.1.1.1:80",
            "::1:80",
            "[::1:80",
            "[::1]80",
            "ftp://host:1",
            long.as_str(),
            longer.as_str(),
        ] {
            assert!(n(bad, Trust::Admin).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn errors_do_not_echo_input() {
        let e = n("secret-token@host:80", Trust::Admin).unwrap_err();
        assert!(!e.to_string().contains("secret"));
    }

    #[test]
    fn clean_text_strips_controls_and_cuts() {
        assert_eq!(clean_text("  a\u{7}b\nc  ", 10), "abc");
        assert_eq!(clean_text(&"x".repeat(500), 5), "xxxxx");
    }
}
