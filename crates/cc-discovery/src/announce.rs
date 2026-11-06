// SPDX-License-Identifier: GPL-3.0-or-later
//! Receivers on the local network and the server they announce (mDNS / DNS-SD).
//!
//! A receiver registers a DNS-SD service `_cometcast._tcp` on `local.` ([`advertise_receiver`]).
//! Its TXT record carries (UNCERTAIN: this record layout is a proposal of this crate, not an
//! agreed protocol; nothing else implements it yet):
//!
//! | key | meaning |
//! | --- | --- |
//! | `v` | layout version, `1` |
//! | `srv` | the server the receiver prefers, `host:port` |
//! | `tls` | `1`: the server speaks TLS (`wss://`), otherwise plain `ws://` |
//! | `name` | friendly name shown to people |
//!
//! The room code is NOT announced.
//!
//! Everything read from the network is untrusted: any device on the local network can send
//! such a record. The announced server is checked with [`Trust::Network`] (host name, or a
//! private IP literal; no scheme, path, loopback and so on), names are stripped of control
//! characters and cut, and at most [`MAX_ADDRESSES`] addresses are kept. An announced server
//! is a suggestion of the lowest trust: it ranks after MDM and DNS-SRV, and an MDM policy can
//! switch it off (`forbid_announcement`, or `forbid_user_override`). Because anybody on the
//! network can announce, an announced server must never be used without the person seeing
//! which one it is: the signalling server sees room codes and addresses of the devices.
//!
//! UNCERTAIN: never run on a real network with several devices. Multicast may be filtered by
//! switches or access points, which is exactly the situation in which the receiver does not
//! show up (docs/measurement-points.md, A8). Apple and Android backends (NWBrowser,
//! NsdManager) are not implemented here; this is the `mdns-sd` backend for desktop.

use std::net::IpAddr;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use mdns_sd::{ResolvedService, ServiceDaemon, ServiceEvent, ServiceInfo};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::address::{Trust, clean_text, normalize_server};

/// DNS-SD service type with domain, as `mdns-sd` wants it.
pub const MDNS_SERVICE_TYPE: &str = "_cometcast._tcp.local.";
/// TXT key: layout version.
pub const TXT_VERSION: &str = "v";
/// TXT key: announced server.
pub const TXT_SERVER: &str = "srv";
/// TXT key: TLS flag.
pub const TXT_TLS: &str = "tls";
/// TXT key: friendly name.
pub const TXT_NAME: &str = "name";
/// Most addresses kept per receiver.
pub const MAX_ADDRESSES: usize = 8;
/// Most receivers collected by one browse.
pub const MAX_RECEIVERS: usize = 64;
/// Longest friendly name in characters.
pub const MAX_NAME_CHARS: usize = 48;

/// A failure of the mDNS backend.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("mDNS: {0}")]
pub struct MdnsError(pub String);

/// A receiver found on the local network. All text is cleaned and bounded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiverInfo {
    /// DNS-SD instance name.
    pub instance: String,
    /// Friendly name (falls back to the instance name).
    pub name: String,
    /// Addresses of the receiver, IPv4 first.
    pub addresses: Vec<IpAddr>,
    /// Port of the DNS-SD service.
    pub port: u16,
    /// The server it announced, normalised, `wss://` prefix when `tls=1`. `None` when it
    /// announced nothing or something that did not pass the checks.
    pub announced_server: Option<String>,
}

/// Build a [`ReceiverInfo`] from what the network said. `fullname` is
/// `<instance>._cometcast._tcp.local.`; `txt` are the TXT pairs. `None` when the name does not
/// belong to this service or the instance name is empty.
pub fn receiver_from_parts<'a>(
    fullname: &str,
    port: u16,
    addresses: impl IntoIterator<Item = IpAddr>,
    txt: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Option<ReceiverInfo> {
    let instance = fullname
        .strip_suffix(MDNS_SERVICE_TYPE)?
        .strip_suffix('.')?;
    let instance = clean_text(instance, 63);
    if instance.is_empty() {
        return None;
    }
    let (mut version, mut srv, mut tls, mut name) = (None, None, false, None);
    // Look at a bounded number of TXT entries; the first occurrence of a key wins.
    for (k, v) in txt.into_iter().take(32) {
        match k {
            TXT_VERSION if version.is_none() => version = Some(v),
            TXT_SERVER if srv.is_none() => srv = Some(v),
            TXT_TLS => tls |= v == "1",
            TXT_NAME if name.is_none() => name = Some(v),
            _ => {}
        }
    }
    let announced_server = match (version, srv) {
        (None | Some("1"), Some(s)) => normalize_server(s, Trust::Network)
            .ok()
            .map(|a| if tls { format!("wss://{a}") } else { a }),
        _ => None,
    };
    let name = name
        .map(|n| clean_text(n, MAX_NAME_CHARS))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| instance.clone());
    let mut addresses: Vec<IpAddr> = addresses.into_iter().collect();
    addresses.sort_by_key(|a| (a.is_ipv6(), *a));
    addresses.dedup();
    addresses.truncate(MAX_ADDRESSES);
    Some(ReceiverInfo {
        instance,
        name,
        addresses,
        port,
        announced_server,
    })
}

fn receiver_from_resolved(info: &ResolvedService) -> Option<ReceiverInfo> {
    let props: Vec<(String, String)> = info
        .get_properties()
        .iter()
        .take(32)
        .map(|p| (p.key().to_owned(), p.val_str().to_owned()))
        .collect();
    receiver_from_parts(
        info.get_fullname(),
        info.get_port(),
        info.get_addresses()
            .iter()
            .map(mdns_sd::ScopedIp::to_ip_addr),
        props.iter().map(|(k, v)| (k.as_str(), v.as_str())),
    )
}

/// What [`advertise_receiver`] announces.
#[derive(Debug, Clone)]
pub struct AdvertiseOptions {
    /// DNS-SD instance name (cleaned, cut to 63 bytes).
    pub instance_name: String,
    /// Port of the service. Comet Cast receivers do not listen for senders yet; UNCERTAIN: the
    /// value has no meaning for the protocol, mDNS only needs one.
    pub port: u16,
    /// The server to announce: `host:port`, or with `wss://` / `ws://` (an announced `wss://`
    /// becomes `tls=1`). The same rules as for received announcements apply (no loopback, no
    /// public IP literal), so a wrong value is found at the receiver, not at every sender.
    pub preferred_server: Option<String>,
    /// Host name for the `.local.` address records; random when `None`.
    pub host_label: Option<String>,
}

/// An active announcement. Dropping it withdraws the service and stops the mDNS daemon.
pub struct Advertisement {
    daemon: ServiceDaemon,
    fullname: String,
}

impl Advertisement {
    /// The full DNS-SD name that is announced.
    pub fn fullname(&self) -> &str {
        &self.fullname
    }
}

impl Drop for Advertisement {
    fn drop(&mut self) {
        let _ = self.daemon.unregister(&self.fullname);
        let _ = self.daemon.shutdown();
    }
}

/// Announce this receiver and its preferred server on the local network.
pub fn advertise_receiver(opts: &AdvertiseOptions) -> Result<Advertisement, MdnsError> {
    let instance = clean_text(&opts.instance_name, 63);
    if instance.is_empty() || instance.len() > 63 {
        return Err(MdnsError("instance name is empty or too long".into()));
    }
    let mut props: Vec<(String, String)> = vec![
        (TXT_VERSION.into(), "1".into()),
        (
            TXT_NAME.into(),
            clean_text(&opts.instance_name, MAX_NAME_CHARS),
        ),
    ];
    if let Some(server) = &opts.preferred_server {
        let full = normalize_server(server, Trust::Admin).map_err(|e| MdnsError(e.to_string()))?;
        let (tls, authority) = match full.split_once("://") {
            Some(("wss" | "https", a)) => (true, a),
            Some((_, a)) => (false, a),
            None => (false, full.as_str()),
        };
        // The same checks as the senders will apply.
        let authority = normalize_server(authority, Trust::Network)
            .map_err(|e| MdnsError(format!("server cannot be announced: {e}")))?;
        props.push((TXT_SERVER.into(), authority));
        if tls {
            props.push((TXT_TLS.into(), "1".into()));
        }
    }
    let label = match &opts.host_label {
        Some(l) => crate::address::validate_hostname(l).map_err(|e| MdnsError(e.to_string()))?,
        None => {
            use rand::Rng;
            format!("cometcast-{:08x}", rand::rng().random::<u32>())
        }
    };
    if label.contains('.') {
        return Err(MdnsError("host label must not contain dots".into()));
    }
    let host = format!("{label}.local.");
    // An empty address list plus `enable_addr_auto` lets mdns-sd use the addresses of this
    // machine's interfaces.
    let info = ServiceInfo::new(
        MDNS_SERVICE_TYPE,
        &instance,
        &host,
        "",
        opts.port,
        props.as_slice(),
    )
    .map_err(|e| MdnsError(e.to_string()))?
    .enable_addr_auto();
    let fullname = info.get_fullname().to_owned();
    let daemon = ServiceDaemon::new().map_err(|e| MdnsError(e.to_string()))?;
    if let Err(e) = daemon.register(info) {
        let _ = daemon.shutdown();
        return Err(MdnsError(e.to_string()));
    }
    Ok(Advertisement { daemon, fullname })
}

/// A change in the set of receivers on the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiverEvent {
    /// A receiver appeared or changed.
    Found(ReceiverInfo),
    /// A receiver went away (its instance name).
    Lost(String),
}

/// A running browse for receivers. Dropping it stops the browse.
pub struct ReceiverBrowser {
    rx: mpsc::Receiver<ReceiverEvent>,
    daemon: ServiceDaemon,
    task: JoinHandle<()>,
}

impl ReceiverBrowser {
    /// The next event, or `None` when the browse has ended.
    pub async fn next(&mut self) -> Option<ReceiverEvent> {
        self.rx.recv().await
    }
}

impl Drop for ReceiverBrowser {
    fn drop(&mut self) {
        self.task.abort();
        let _ = self.daemon.stop_browse(MDNS_SERVICE_TYPE);
        let _ = self.daemon.shutdown();
    }
}

/// Start browsing for receivers. The events come until the browser is dropped. Must be called
/// inside a Tokio runtime. This is meant to run in parallel with the server lookup.
pub fn discover_receivers() -> Result<ReceiverBrowser, MdnsError> {
    let daemon = ServiceDaemon::new().map_err(|e| MdnsError(e.to_string()))?;
    let events = daemon
        .browse(MDNS_SERVICE_TYPE)
        .map_err(|e| MdnsError(e.to_string()))?;
    let (tx, rx) = mpsc::channel(64);
    let task = tokio::spawn(async move {
        while let Ok(ev) = events.recv_async().await {
            let out = match ev {
                ServiceEvent::ServiceResolved(info) => {
                    receiver_from_resolved(&info).map(ReceiverEvent::Found)
                }
                ServiceEvent::ServiceRemoved(_, fullname) => fullname
                    .strip_suffix(MDNS_SERVICE_TYPE)
                    .and_then(|i| i.strip_suffix('.'))
                    .map(|i| ReceiverEvent::Lost(clean_text(i, 63))),
                _ => None,
            };
            if let Some(out) = out
                && tx.send(out).await.is_err()
            {
                break;
            }
        }
    });
    Ok(ReceiverBrowser { rx, daemon, task })
}

/// Where announcements come from. A trait so that tests need no network.
#[async_trait]
pub trait AnnouncementSource: Send + Sync {
    /// Listen for `wait` and return the receivers seen (at most [`MAX_RECEIVERS`]).
    async fn browse(&self, wait: Duration) -> Result<Vec<ReceiverInfo>, MdnsError>;
}

/// The `mdns-sd` backend.
#[derive(Debug, Default, Clone, Copy)]
pub struct MdnsSource;

#[async_trait]
impl AnnouncementSource for MdnsSource {
    async fn browse(&self, wait: Duration) -> Result<Vec<ReceiverInfo>, MdnsError> {
        let daemon = ServiceDaemon::new().map_err(|e| MdnsError(e.to_string()))?;
        let events = match daemon.browse(MDNS_SERVICE_TYPE) {
            Ok(e) => e,
            Err(e) => {
                let _ = daemon.shutdown();
                return Err(MdnsError(e.to_string()));
            }
        };
        let deadline = Instant::now() + wait;
        let mut found: Vec<ReceiverInfo> = Vec::new();
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            match tokio::time::timeout(left, events.recv_async()).await {
                Ok(Ok(ServiceEvent::ServiceResolved(info))) => {
                    if let Some(r) = receiver_from_resolved(&info) {
                        if let Some(old) = found.iter_mut().find(|o| o.instance == r.instance) {
                            *old = r;
                        } else if found.len() < MAX_RECEIVERS {
                            found.push(r);
                        }
                    }
                }
                Ok(Ok(_)) => {}
                Ok(Err(_)) | Err(_) => break,
            }
        }
        let _ = daemon.stop_browse(MDNS_SERVICE_TYPE);
        let _ = daemon.shutdown();
        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    fn parts(fullname: &str, txt: &[(&str, &str)]) -> Option<ReceiverInfo> {
        receiver_from_parts(
            fullname,
            9,
            [ip("fd00::2"), ip("192.168.0.9"), ip("192.168.0.9")],
            txt.iter().copied(),
        )
    }

    const FULL: &str = "Room 1._cometcast._tcp.local.";

    #[test]
    fn parses_announcement() {
        let r = parts(
            FULL,
            &[
                ("v", "1"),
                ("srv", "Relay.School.example:443"),
                ("tls", "1"),
                ("name", "Room 1 TV"),
            ],
        )
        .unwrap();
        assert_eq!(r.instance, "Room 1");
        assert_eq!(r.name, "Room 1 TV");
        assert_eq!(
            r.announced_server.as_deref(),
            Some("wss://relay.school.example:443")
        );
        assert_eq!(r.addresses, [ip("192.168.0.9"), ip("fd00::2")]);
        let r = parts(FULL, &[("srv", "10.0.0.5:8443")]).unwrap();
        assert_eq!(r.announced_server.as_deref(), Some("10.0.0.5:8443"));
        assert_eq!(r.name, "Room 1");
    }

    #[test]
    fn no_announcement_is_fine() {
        let r = parts(FULL, &[]).unwrap();
        assert_eq!(r.announced_server, None);
    }

    #[test]
    fn hostile_txt_is_refused() {
        for bad in [
            "127.0.0.1:8443",
            "localhost:8443",
            "8.8.8.8:443",
            "[::1]:1",
            "wss://x.example:443",
            "x.example:443/ws",
            "user@x.example:443",
            "x.example",
            "x.example:0",
            "x y.example:1",
            "x.example:1\r\nHost: evil",
            "",
        ] {
            let r = parts(FULL, &[("srv", bad)]).unwrap();
            assert_eq!(r.announced_server, None, "{bad:?}");
        }
        let huge = format!("{}.example:1", "a".repeat(5000));
        assert_eq!(
            parts(FULL, &[("srv", &huge)]).unwrap().announced_server,
            None
        );
        // A different layout version is not understood.
        let r = parts(FULL, &[("v", "2"), ("srv", "x.example:1")]).unwrap();
        assert_eq!(r.announced_server, None);
    }

    #[test]
    fn first_srv_wins_and_txt_is_bounded() {
        let r = parts(FULL, &[("srv", "a.example:1"), ("srv", "b.example:2")]).unwrap();
        assert_eq!(r.announced_server.as_deref(), Some("a.example:1"));
        // The announcement hidden behind 40 junk entries is not looked at.
        let mut txt: Vec<(String, String)> =
            (0..40).map(|i| (format!("k{i}"), "x".into())).collect();
        txt.push(("srv".into(), "late.example:1".into()));
        let r = receiver_from_parts(
            FULL,
            1,
            [],
            txt.iter().map(|(k, v)| (k.as_str(), v.as_str())),
        )
        .unwrap();
        assert_eq!(r.announced_server, None);
    }

    #[test]
    fn names_are_cleaned_and_bounded() {
        let long = "n".repeat(500);
        let r = parts(FULL, &[("name", &format!("\u{1b}[31mTV{long}"))]).unwrap();
        assert!(!r.name.contains('\u{1b}'));
        assert!(r.name.chars().count() <= MAX_NAME_CHARS);
        let r = parts(FULL, &[("name", "\u{7}\u{7}")]).unwrap();
        assert_eq!(r.name, "Room 1", "an empty name falls back to the instance");
    }

    #[test]
    fn foreign_or_empty_instance_is_dropped() {
        assert!(parts("x._http._tcp.local.", &[]).is_none());
        assert!(parts("._cometcast._tcp.local.", &[]).is_none());
        assert!(parts("_cometcast._tcp.local.", &[]).is_none());
        assert!(parts("", &[]).is_none());
        let many: Vec<IpAddr> = (1..=50).map(|i| ip(&format!("10.0.0.{i}"))).collect();
        let r = receiver_from_parts(FULL, 1, many, []).unwrap();
        assert_eq!(r.addresses.len(), MAX_ADDRESSES);
    }

    #[test]
    fn advertise_refuses_unannounceable_servers() {
        for bad in [
            "127.0.0.1:8443",
            "localhost:1",
            "8.8.8.8:443",
            "x.example",
            "a b:1",
        ] {
            let o = AdvertiseOptions {
                instance_name: "T".into(),
                port: 9,
                preferred_server: Some(bad.into()),
                host_label: None,
            };
            assert!(advertise_receiver(&o).is_err(), "{bad}");
        }
        let o = AdvertiseOptions {
            instance_name: "".into(),
            port: 9,
            preferred_server: None,
            host_label: None,
        };
        assert!(advertise_receiver(&o).is_err());
    }
}
