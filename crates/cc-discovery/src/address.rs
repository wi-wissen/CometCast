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
    /// broadcast, link-local (IPv4 169.254.0.0/16 incl. the cloud metadata address, IPv6
    /// fe80::/10); an IP literal must be in a private range (public servers have to be given
    /// by name). See [`network_server`] for the TLS rule.
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
            if v4.is_link_local() {
                return bad("link-local addresses (169.254.0.0/16, cloud metadata) are refused");
            }
            if v4.octets() == [100, 100, 100, 200] {
                return bad("cloud metadata address");
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
            // fd00:ec2::254 is the IPv6 metadata address of AWS.
            if v6.segments()[..3] == [0xfd00, 0x0ec2, 0] && v6.segments()[7] == 0x254 {
                return bad("cloud metadata address");
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
    a.is_private() || (o[0] == 100 && (64..128).contains(&o[1]))
}

fn is_unique_local_v6(a: Ipv6Addr) -> bool {
    a.segments()[0] & 0xfe00 == 0xfc00
}

/// Is `ip` in a private range (RFC 1918, `100.64.0.0/10`, unique-local IPv6)?
pub fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(a) => is_private_v4(a),
        IpAddr::V6(a) => is_unique_local_v6(a),
    }
}

/// Check a DNS host name (letters, digits, hyphens; labels of 1 to 63 characters) and return
/// it in lower case without a trailing dot. Underscores are refused.
///
/// The last label must contain at least one letter `a` to `z` and must not start with `0x`.
/// Reason: `inet_aton` (glibc, BSD), WHATWG URL parsers and others read hosts such as
/// `127.1`, `2130706433`, `0177.0.0.1` (octal) or `0x7f000001` (hex) as IPv4 addresses, which
/// would get around the IP-literal rules. A name whose last label has a letter and does not
/// start with `0x` is never a number for those parsers.
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
    let last = name.rsplit('.').next().unwrap_or_default();
    if last.starts_with("0x") || !last.bytes().any(|c| c.is_ascii_lowercase()) {
        return Err(AddressError(
            "last label of a host name must contain a letter and not look like a number",
        ));
    }
    Ok(name)
}

/// Is `host` (IPv6 without brackets) inside the local network by its form: an IP literal
/// (already checked to be private by [`Trust::Network`]) or a name ending in `.local` (mDNS)?
fn is_local_host(host: &str) -> bool {
    host.parse::<IpAddr>().is_ok() || host.ends_with(".local")
}

/// Check a server address that came from the network and decide about TLS.
///
/// `tls` says what the source stated: `Some(true)` TLS (`wss`), `Some(false)` explicitly plain
/// `ws`, `None` nothing (DNS-SRV cannot say). Rule: plain `ws` is only accepted inside the
/// local network, that is for a private IP literal or a `.local` name. Every other host is only
/// accepted with TLS: `Some(false)` is refused, `None` becomes `wss`. Returns the canonical
/// address WITH scheme (`wss://host:port` or `ws://host:port`) and whether it is TLS.
///
/// A host NAME that resolves to a private or loopback address (DNS rebinding, `localtest.me`)
/// is NOT detected: this crate does no name resolution here.
pub fn network_server(input: &str, tls: Option<bool>) -> Result<(String, bool), AddressError> {
    let addr = normalize_server(input, Trust::Network)?;
    let host = addr.rsplit_once(':').map_or(addr.as_str(), |(h, _)| h);
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let local = is_local_host(host);
    let use_tls = match tls {
        Some(true) => true,
        Some(false) if local => false,
        Some(false) => {
            return Err(AddressError(
                "plain ws is only accepted for private IP addresses and .local names",
            ));
        }
        None => !local,
    };
    let scheme = if use_tls { "wss" } else { "ws" };
    Ok((format!("{scheme}://{addr}"), use_tls))
}

/// Characters of the Unicode categories Cf (format: bidi overrides, zero width and so on),
/// Zl and Zp. The standard library has no category lookup, so the list is explicit.
/// UNCERTAIN: written by hand from Unicode 15/16; Cf characters added later are not covered.
fn is_invisible_format(c: char) -> bool {
    matches!(c as u32,
        0x00AD | 0x0600..=0x0605 | 0x061C | 0x06DD | 0x070F | 0x0890..=0x0891 | 0x08E2
        | 0x180E | 0x200B..=0x200F | 0x2028..=0x202E | 0x2060..=0x2064 | 0x2066..=0x206F
        | 0xFEFF | 0xFFF9..=0xFFFB | 0x110BD | 0x110CD | 0x13430..=0x1343F
        | 0x1BCA0..=0x1BCA3 | 0x1D173..=0x1D17A | 0xE0001 | 0xE0020..=0xE007F)
}

/// Remove control characters (Cc), format characters (Cf: bidi overrides, zero-width
/// characters), line and paragraph separators (Zl, Zp) and surrounding blanks from text that
/// came from the network and cut it to `max_chars` characters. Used for names shown to people.
pub fn clean_text(s: &str, max_chars: usize) -> String {
    s.chars()
        .filter(|c| !c.is_control() && !is_invisible_format(*c))
        .take(max_chars)
        .collect::<String>()
        .trim()
        .to_owned()
}

/// Characters of the private-use areas (they have no agreed meaning and can be used to fake
/// symbols).
fn is_private_use(c: char) -> bool {
    matches!(c as u32, 0xE000..=0xF8FF | 0xF0000..=0xFFFFD | 0x100000..=0x10FFFD)
}

/// Longest room name in characters (spec 13.2, 14.3).
pub const MAX_ROOM_NAME_CHARS: usize = 32;

/// Clean a room name that came from the network like the display cleans `room_name`
/// (spec 13.2): control, format (bidi, zero-width), invisible and private-use characters are
/// removed, white space runs become one blank, the result is trimmed and cut at
/// [`MAX_ROOM_NAME_CHARS`] characters. May be empty.
pub fn clean_room_name(s: &str) -> String {
    let mut out = String::new();
    let mut pending_blank = false;
    for c in s.chars() {
        if c.is_whitespace() || matches!(c, '\t' | '\n' | '\r') {
            pending_blank = !out.is_empty();
            continue;
        }
        if c.is_control() || is_invisible_format(c) || is_private_use(c) {
            continue;
        }
        if pending_blank {
            out.push(' ');
            pending_blank = false;
        }
        out.push(c);
    }
    out.chars()
        .take(MAX_ROOM_NAME_CHARS)
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
            "169.254.169.254:80",
            "169.254.1.1:80",
            "100.100.100.200:80",
            "[fd00:ec2::254]:80",
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
            "0x7f000001:80",
            "0x08080808:80",
            "0xc0a80001:80",
            "0x7f.0x1:80",
            "1.0x1:80",
            "0177.0.0.0x1:80",
            "127.1:80",
            "2130706433:80",
            "0X7F000001:80",
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
    fn numeric_and_hex_hosts_are_refused_for_every_trust() {
        for h in [
            "0x7f000001",
            "0x7f.0x1",
            "1.0x1",
            "0177.0.0.0x1",
            "127.1",
            "2130706433",
            "0300.0250.0.1",
        ] {
            assert!(validate_hostname(h).is_err(), "{h}");
            for t in [Trust::Admin, Trust::Network] {
                assert!(n(&format!("{h}:8443"), t).is_err(), "{h} {t:?}");
            }
        }
        // Names with a letter in the last label stay valid, also with digits and 0x inside.
        for h in [
            "0x7f.example",
            "a1.b2",
            "example.c0m",
            "1.2.3.abc",
            "x0x1.org",
        ] {
            assert!(validate_hostname(h).is_ok(), "{h}");
        }
    }

    #[test]
    fn network_server_decides_about_tls() {
        let ok = |s: &str, t| network_server(s, t).unwrap();
        // Private IP literals and .local names may be plain.
        assert_eq!(
            ok("192.168.1.5:8443", Some(false)),
            ("ws://192.168.1.5:8443".into(), false)
        );
        assert_eq!(
            ok("[fd00::1]:8443", Some(false)),
            ("ws://[fd00::1]:8443".into(), false)
        );
        assert_eq!(
            ok("Relay.LOCAL:80", Some(false)),
            ("ws://relay.local:80".into(), false)
        );
        assert_eq!(ok("10.0.0.5:1", None), ("ws://10.0.0.5:1".into(), false));
        assert_eq!(
            ok("10.0.0.5:1", Some(true)),
            ("wss://10.0.0.5:1".into(), true)
        );
        // Other names need TLS: explicit plain is refused, unknown becomes wss.
        assert!(network_server("relay.example.org:443", Some(false)).is_err());
        assert_eq!(
            ok("relay.example.org:443", None),
            ("wss://relay.example.org:443".into(), true)
        );
        assert_eq!(
            ok("relay.example.org:443", Some(true)),
            ("wss://relay.example.org:443".into(), true)
        );
        // The Network rules still apply.
        assert!(network_server("127.0.0.1:1", Some(true)).is_err());
        assert!(network_server("169.254.169.254:80", Some(false)).is_err());
        assert!(network_server("wss://x.example:1", Some(true)).is_err());
        // Not detected, documented: a name that resolves to a private address.
        assert!(network_server("localtest.me:443", Some(true)).is_ok());
    }

    #[test]
    fn clean_text_strips_format_characters() {
        // bidi override, zero width space, BOM, soft hyphen, line/paragraph separator, tag char.
        let s = "a\u{202E}b\u{200B}c\u{FEFF}d\u{AD}e\u{2028}f\u{2029}g\u{E0041}h\u{2066}i";
        assert_eq!(clean_text(s, 50), "abcdefghi");
        assert_eq!(clean_text("Zo\u{eb} \u{1F600}", 50), "Zo\u{eb} \u{1F600}");
    }

    #[test]
    fn clean_text_strips_controls_and_cuts() {
        assert_eq!(clean_text("  a\u{7}b\nc  ", 10), "abc");
        assert_eq!(clean_text(&"x".repeat(500), 5), "xxxxx");
    }

    #[test]
    fn room_names_are_cleaned_like_the_display_does() {
        assert_eq!(clean_room_name("  Room \t 204\n "), "Room 204");
        assert_eq!(
            clean_room_name("A\u{202e}B\u{200b}C\u{e000}D\u{7}E"),
            "ABCDE"
        );
        assert_eq!(clean_room_name("\u{200b}\u{7}"), "");
        assert_eq!(clean_room_name(&"x".repeat(100)).chars().count(), 32);
        assert_eq!(clean_room_name("a   b"), "a b");
        // The cut never leaves a trailing blank.
        assert_eq!(
            clean_room_name(&format!("{} y", "x".repeat(31))),
            "x".repeat(31)
        );
    }
}
