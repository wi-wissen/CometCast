// SPDX-License-Identifier: GPL-3.0-or-later
//! Browser origins that may use this relay from another web site (`allowed_origins`).
//!
//! The web app is static files that can live on any host (for example GitHub Pages) while the
//! relay runs somewhere else. A browser then calls `/ice-servers` and `/altcha/challenge`
//! across origins (needs CORS) and opens `/ws` with the page's `Origin` header.
//!
//! - Empty list (default): no CORS headers are ever sent (same-origin use only, as before) and
//!   `/ws` accepts any `Origin`.
//! - Non-empty list: the HTTP endpoints answer CORS for exactly those origins, and `/ws`
//!   refuses a browser origin that is not listed. Requests without `Origin` header (native
//!   clients, curl) are never affected.
//! - `*` means "any origin". It is accepted in internal mode only (`ServerConfig::validate`).
//!
//! No credentials are involved: `Access-Control-Allow-Credentials` is never sent.
//! WebSocket upgrades are not subject to CORS, so the `Origin` check there is done by hand.

use std::collections::HashSet;

use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::AppState;

/// Endpoints that a browser on another origin calls with fetch().
const CORS_PATHS: [&str; 3] = ["/ice-servers", "/altcha/challenge", "/healthz"];

/// Longest `Origin` we look at; anything longer cannot be a real one.
const MAX_ORIGIN_LEN: usize = 255;

/// Turn `https://App.Example.org:443` into the form browsers send (`https://app.example.org`):
/// lowercase scheme and host, default port removed. Only `http` and `https` with a host and an
/// optional port are accepted; a path (also a trailing `/`), query, user info or wildcard host
/// is not an origin and gives `None`.
pub(crate) fn normalize_origin(text: &str) -> Option<String> {
    if text.is_empty() || text.len() > MAX_ORIGIN_LEN || !text.is_ascii() {
        return None;
    }
    let text = text.to_ascii_lowercase();
    let (scheme, rest) = text.split_once("://")?;
    let default_port = match scheme {
        "http" => "80",
        "https" => "443",
        _ => return None,
    };
    let (host, port) = if let Some(inner) = rest.strip_prefix('[') {
        // [v6]  or  [v6]:port
        let (addr, after) = inner.split_once(']')?;
        if addr.is_empty()
            || !addr
                .chars()
                .all(|c| c.is_ascii_hexdigit() || c == ':' || c == '.')
        {
            return None;
        }
        let port = match after {
            "" => None,
            _ => Some(after.strip_prefix(':')?),
        };
        (format!("[{addr}]"), port)
    } else {
        let (host, port) = match rest.split_once(':') {
            Some((h, p)) => (h, Some(p)),
            None => (rest, None),
        };
        let ok = !host.is_empty()
            && !host.starts_with(['.', '-'])
            && !host.ends_with(['.', '-'])
            && !host.contains("..")
            && host
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.');
        if !ok {
            return None;
        }
        (host.to_string(), port)
    };
    let port = match port {
        None => None,
        Some(p) => {
            let ok = !p.is_empty() && p.len() <= 5 && p.chars().all(|c| c.is_ascii_digit());
            match p.parse::<u32>() {
                Ok(n) if ok && (1..=65535).contains(&n) => Some(n.to_string()),
                _ => return None,
            }
        }
    };
    Some(match port {
        Some(p) if p != default_port => format!("{scheme}://{host}:{p}"),
        _ => format!("{scheme}://{host}"),
    })
}

/// The compiled `allowed_origins` list.
#[derive(Debug, Default)]
pub(crate) struct OriginPolicy {
    /// The list was not empty: CORS answers and the `/ws` check are active.
    active: bool,
    /// `*` was listed.
    any: bool,
    /// Normalised origins.
    set: HashSet<String>,
}

impl OriginPolicy {
    /// An entry that is not a valid origin never matches (it is reported by `validate()`);
    /// it still counts as "a list was given", so a typo cannot silently switch the check off.
    /// An empty list means "not configured". A value that was written down but names no origin
    /// (`""`, `","`) never gets here: the configuration loader refuses it at start
    /// (`check_allowed_origins_text`), so "empty" can only mean "absent".
    pub(crate) fn new(list: &[String]) -> Self {
        let mut p = Self {
            active: !list.is_empty(),
            ..Self::default()
        };
        for e in list {
            let e = e.trim();
            if e == "*" {
                p.any = true;
            } else if let Some(o) = normalize_origin(e) {
                p.set.insert(o);
            }
        }
        p
    }

    /// May a page with this `Origin` header value use the relay?
    pub(crate) fn allows(&self, origin: &str) -> bool {
        self.any || normalize_origin(origin).is_some_and(|o| self.set.contains(&o))
    }

    /// `/ws`: false = refuse the upgrade. No `Origin` header (not a browser) is always fine.
    pub(crate) fn allows_ws(&self, headers: &HeaderMap) -> bool {
        if !self.active {
            return true;
        }
        match headers.get(header::ORIGIN) {
            None => true,
            Some(v) => v.to_str().is_ok_and(|o| self.allows(o)),
        }
    }
}

fn is_cors_path(path: &str) -> bool {
    CORS_PATHS.contains(&path)
}

/// Adds CORS headers to the answers of [`CORS_PATHS`] and answers their preflight requests.
/// It is the outermost layer, so refusals (429, 403) can be read by an allowed page too.
pub(crate) async fn cors_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let policy = &state.origins;
    if !policy.active || !is_cors_path(req.uri().path()) {
        return next.run(req).await;
    }
    let origin = req.headers().get(header::ORIGIN).cloned();
    let Some(origin) = origin else {
        // Not a cross-origin browser request. Caches must still keep the variants apart.
        let mut resp = next.run(req).await;
        vary_origin(&mut resp);
        return resp;
    };
    let allowed = origin.to_str().is_ok_and(|o| policy.allows(o));
    let preflight = req.method() == Method::OPTIONS
        && req
            .headers()
            .contains_key(header::ACCESS_CONTROL_REQUEST_METHOD);
    let pna_requested = req
        .headers()
        .get("access-control-request-private-network")
        .is_some_and(|v| v.as_bytes() == b"true");
    if preflight {
        let mut resp = if allowed {
            let mut r = StatusCode::NO_CONTENT.into_response();
            let h = r.headers_mut();
            h.insert(
                header::ACCESS_CONTROL_ALLOW_METHODS,
                HeaderValue::from_static("GET, HEAD, OPTIONS"),
            );
            // Only plain tokens are echoed back (what the browser asked to send).
            if let Some(req_headers) = req
                .headers()
                .get(header::ACCESS_CONTROL_REQUEST_HEADERS)
                .and_then(|v| v.to_str().ok())
                .filter(|v| {
                    v.len() <= 200
                        && v.chars()
                            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | ',' | ' '))
                })
                && let Ok(v) = HeaderValue::from_str(req_headers)
            {
                h.insert(header::ACCESS_CONTROL_ALLOW_HEADERS, v);
            }
            h.insert(
                header::ACCESS_CONTROL_MAX_AGE,
                HeaderValue::from_static("600"),
            );
            // Private Network Access: only for an allowed origin and only on a relay that is
            // meant for a private network. UNCERTAIN: not tried in a real Chrome PNA flow.
            if pna_requested && private_network_relay(&state.config) {
                h.insert(
                    HeaderName::from_static("access-control-allow-private-network"),
                    HeaderValue::from_static("true"),
                );
            }
            r
        } else {
            (StatusCode::FORBIDDEN, "origin not allowed\n").into_response()
        };
        if allowed {
            allow_origin(&mut resp, policy.any, &origin);
        }
        vary_origin(&mut resp);
        return resp;
    }
    let mut resp = next.run(req).await;
    if allowed {
        allow_origin(&mut resp, policy.any, &origin);
        // Without this a page on another origin cannot read Retry-After of a 429 or a ban.
        resp.headers_mut().insert(
            header::ACCESS_CONTROL_EXPOSE_HEADERS,
            HeaderValue::from_static("Retry-After"),
        );
    }
    vary_origin(&mut resp);
    resp
}

/// Is this relay meant for a private network (so a public page may reach it, PNA)?
/// Internal mode always is; otherwise the listen address must be loopback or private.
fn private_network_relay(config: &crate::ServerConfig) -> bool {
    use std::net::IpAddr;
    config.mode == crate::Mode::Internal
        || match config.listen_addr.ip() {
            IpAddr::V4(a) => a.is_loopback() || a.is_private() || a.is_link_local(),
            IpAddr::V6(a) => {
                a.is_loopback()
                    || (a.segments()[0] & 0xfe00) == 0xfc00
                    || (a.segments()[0] & 0xffc0) == 0xfe80
            }
        }
}

fn allow_origin(resp: &mut Response, any: bool, origin: &HeaderValue) {
    let v = if any {
        HeaderValue::from_static("*")
    } else {
        origin.clone()
    };
    resp.headers_mut()
        .insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, v);
}

fn vary_origin(resp: &mut Response) {
    resp.headers_mut()
        .append(header::VARY, HeaderValue::from_static("Origin"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origins_are_normalised() {
        let n = |s| normalize_origin(s);
        assert_eq!(
            n("https://cometcast.de"),
            Some("https://cometcast.de".into())
        );
        assert_eq!(
            n("HTTPS://CometCast.DE:443"),
            Some("https://cometcast.de".into())
        );
        assert_eq!(n("http://localhost:80"), Some("http://localhost".into()));
        assert_eq!(
            n("http://localhost:5173"),
            Some("http://localhost:5173".into())
        );
        assert_eq!(n("https://[::1]:8443"), Some("https://[::1]:8443".into()));
        assert_eq!(n("http://[::1]:80"), Some("http://[::1]".into()));
    }

    #[test]
    fn non_origins_are_refused() {
        for bad in [
            "",
            "*",
            "cometcast.de",
            "https://cometcast.de/",
            "https://cometcast.de/app",
            "https://*.cometcast.de",
            "https://user@cometcast.de",
            "ftp://cometcast.de",
            "null",
            "https://cometcast.de:0",
            "https://cometcast.de:99999",
            "https://cometcast.de:",
            "https://",
            "https://-a.de",
            "https://a..de",
            "https://[]",
            "https://[::1]x",
            "https://exämple.de",
        ] {
            assert_eq!(normalize_origin(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn policy_matches_exactly() {
        let p = OriginPolicy::new(&[
            "https://cometcast.de".into(),
            "http://localhost:8080".into(),
        ]);
        assert!(p.allows("https://cometcast.de"));
        assert!(p.allows("https://CometCast.de:443"));
        assert!(!p.allows("http://cometcast.de"), "scheme differs");
        assert!(!p.allows("https://www.cometcast.de"), "host differs");
        assert!(!p.allows("https://cometcast.de:8443"), "port differs");
        assert!(!p.allows("https://cometcast.de.evil.org"));
        assert!(!p.allows("null"));
        assert!(p.allows("http://localhost:8080"));
    }

    #[test]
    fn empty_list_is_inactive_and_typo_list_is_not() {
        assert!(!OriginPolicy::new(&[]).active);
        let typo = OriginPolicy::new(&["https://cometcast.de/".into()]);
        assert!(typo.active, "a bad entry must not switch the check off");
        assert!(!typo.allows("https://cometcast.de"));
    }

    #[test]
    fn ws_check() {
        let mut h = HeaderMap::new();
        let off = OriginPolicy::new(&[]);
        h.insert(header::ORIGIN, "https://evil.org".parse().unwrap());
        assert!(off.allows_ws(&h));
        let on = OriginPolicy::new(&["https://cometcast.de".into()]);
        assert!(!on.allows_ws(&h));
        h.insert(header::ORIGIN, "https://cometcast.de".parse().unwrap());
        assert!(on.allows_ws(&h));
        assert!(on.allows_ws(&HeaderMap::new()), "no Origin = not a browser");
        let any = OriginPolicy::new(&["*".into()]);
        h.insert(header::ORIGIN, "https://evil.org".parse().unwrap());
        assert!(any.allows_ws(&h));
    }
}
