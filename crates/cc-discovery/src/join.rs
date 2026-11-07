// SPDX-License-Identifier: GPL-3.0-or-later
//! The screen's join address (`{dom}`) and the address a person types (spec 15.7, decision N85,
//! native-apps.md 21.6).
//!
//! * [`join_address`]: where a browser finds the web app for a server. The built-in relay
//!   deliberately serves no web app, so it maps to `cometcast.de/app`; every other server is its
//!   own host (without `:443`).
//! * [`server_for_address`]: the reverse for the "via {dom} · Change" field: text typed by a
//!   person (or the address a screen shows) to the server the app connects to.

use crate::DEFAULT_SERVERS;
use crate::address::{AddressError, Trust, normalize_server};

/// The join address of the built-in relay: where the static web app lives.
pub const BUILT_IN_JOIN_ADDRESS: &str = "cometcast.de/app";

/// The built-in relay (`relay.cometcast.de:443`).
pub fn built_in_relay() -> &'static str {
    DEFAULT_SERVERS[0]
}

/// Host part of `server` as shown to people: scheme, trailing slashes and the default port 443
/// removed (decision N12). Lower case is not forced here; callers pass canonical text.
fn display_host(server: &str) -> String {
    let v = server.trim();
    let v = match v.split_once("://") {
        Some((s, r))
            if s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"+.-".contains(&b)) =>
        {
            r
        }
        _ => v,
    };
    let v = v.trim_end_matches('/');
    v.strip_suffix(":443").unwrap_or(v).to_string()
}

/// Is `server` the built-in relay (any spelling of the same address)?
fn is_built_in(server: &str) -> bool {
    display_host(server).eq_ignore_ascii_case(&display_host(built_in_relay()))
}

/// The join address (`{dom}`) for `server`: `cometcast.de/app` for the built-in relay, else
/// the server's host with `:port` only when the port is not 443.
pub fn join_address(server: &str) -> String {
    if is_built_in(server) {
        BUILT_IN_JOIN_ADDRESS.to_string()
    } else {
        display_host(server)
    }
}

/// Result of [`server_for_address`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddressParse {
    /// The server the app should use (`host:port`, built-in relay as in the built-in list).
    Ok(String),
    /// Not an address we can use.
    Invalid,
}

/// Turn the text of the address field into a server.
///
/// Trims; drops a scheme (`https://`, `wss://` ...), a trailing `/` and any path except `/app`;
/// drops `:443`. `cometcast.de` and `cometcast.de/app` give the built-in relay (also
/// `relay.cometcast.de`). Names, IP literals and `.local` names are accepted; a missing port
/// means 443. Anything with a user name, query or fragment is invalid.
pub fn server_for_address(input: &str) -> AddressParse {
    let t = input.trim();
    let t = match t.split_once("://") {
        Some((_, r)) => r,
        None => t,
    };
    // Everything after the first '/' is a path: only "/app" (and "/") are meaningful.
    let (authority, path) = match t.find('/') {
        Some(i) => t.split_at(i),
        None => (t, ""),
    };
    let path = path.trim_end_matches('/');
    if !(path.is_empty() || path.eq_ignore_ascii_case("/app")) {
        // "CAST.Musterfirma.de:443/x": the path is dropped (a person pasted a link).
        if path.contains(['?', '#']) {
            return AddressParse::Invalid;
        }
    }
    if authority.is_empty() || authority.contains(['@', '?', '#', '\\', '%', ' ']) {
        return AddressParse::Invalid;
    }
    let host_port = authority.to_ascii_lowercase();
    let with_port = if has_port(&host_port) {
        host_port.clone()
    } else {
        format!("{host_port}:443")
    };
    let server: Result<String, AddressError> = normalize_server(&with_port, Trust::Admin);
    let Ok(server) = server else {
        return AddressParse::Invalid;
    };
    // The website host stands for the built-in relay (its join address is `cometcast.de/app`).
    if server == "cometcast.de:443" || is_built_in(&server) {
        return AddressParse::Ok(built_in_relay().to_string());
    }
    AddressParse::Ok(server)
}

fn has_port(authority: &str) -> bool {
    match authority.strip_prefix('[') {
        Some(rest) => rest
            .split_once(']')
            .is_some_and(|(_, a)| a.starts_with(':')),
        None => authority
            .rsplit_once(':')
            .is_some_and(|(h, _)| !h.contains(':')),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(s: &str) -> AddressParse {
        AddressParse::Ok(s.to_string())
    }

    #[test]
    fn built_in_relay_has_the_website_as_join_address() {
        assert_eq!(join_address("relay.cometcast.de:443"), "cometcast.de/app");
        assert_eq!(join_address("wss://relay.cometcast.de"), "cometcast.de/app");
        assert_eq!(join_address("Relay.CometCast.de:443/"), "cometcast.de/app");
    }

    #[test]
    fn other_servers_keep_their_host() {
        assert_eq!(
            join_address("cast.musterfirma.de:443"),
            "cast.musterfirma.de"
        );
        assert_eq!(
            join_address("https://cast.musterfirma.de/"),
            "cast.musterfirma.de"
        );
        assert_eq!(join_address("192.168.1.5:8443"), "192.168.1.5:8443");
        assert_eq!(join_address("cast-box.local:443"), "cast-box.local");
    }

    #[test]
    fn typed_addresses_become_servers() {
        assert_eq!(
            server_for_address("cometcast.de"),
            ok("relay.cometcast.de:443")
        );
        assert_eq!(
            server_for_address("https://cometcast.de/app/"),
            ok("relay.cometcast.de:443")
        );
        assert_eq!(
            server_for_address(" cometcast.de/app "),
            ok("relay.cometcast.de:443")
        );
        assert_eq!(
            server_for_address("relay.cometcast.de"),
            ok("relay.cometcast.de:443")
        );
        assert_eq!(
            server_for_address("CAST.Musterfirma.de:443/x"),
            ok("cast.musterfirma.de:443")
        );
        assert_eq!(
            server_for_address("cast.musterfirma.de"),
            ok("cast.musterfirma.de:443")
        );
        assert_eq!(
            server_for_address("192.168.1.5:8443"),
            ok("192.168.1.5:8443")
        );
        assert_eq!(
            server_for_address("cast-box.local"),
            ok("cast-box.local:443")
        );
        assert_eq!(server_for_address("[fd00::1]:8443"), ok("[fd00::1]:8443"));
    }

    #[test]
    fn nonsense_is_invalid() {
        for bad in [
            "",
            "   ",
            "https://",
            "a b",
            "user@host.de",
            "host.de?x=1",
            "host:0",
            "-.de",
            "/app",
        ] {
            assert_eq!(server_for_address(bad), AddressParse::Invalid, "{bad:?}");
        }
    }
}
