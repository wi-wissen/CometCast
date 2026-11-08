// SPDX-License-Identifier: GPL-3.0-or-later
//! Receivers on the local network and the server they announce (mDNS / DNS-SD).
//!
//! The server ladder (step 3) reads the same signed `v=2` display records as the list of screens
//! (`crate::signed`, `docs/discovery.md` section 3): a record is used only if its id derives from
//! its `pk` and its signature verifies; `v=1` records and unverifiable records give no server.
//! The browse here keeps no `seq` memory (it is a one-shot lookup of 1.5 s), so a replayed old
//! record of a display could name the server that display used before. An announced server is
//! therefore only a suggestion of the lowest trust, as before.
//!
//! [`advertise_receiver`] (used by `cometcast-cli discover --announce` and the older
//! `start_advertising` API) announces a **locked** display record, signed with a key that is made
//! for the lifetime of the [`Advertisement`] (it joins no room; it only names a server). Dropping
//! it publishes the signed `state=gone` record before the goodbye.
//!
//! Everything read from the network is untrusted: any device on the local network can send
//! such a record, and a signature only proves that the sender holds the key of the id, not that
//! its content is true. The announced server is checked with [`Trust::Network`] (host name, or a
//! private IP literal; no scheme, path, loopback, link-local and so on) and with the TLS rule
//! of [`network_server`]: plain `ws://` is only accepted for a private IP literal or a `.local`
//! name, every other host needs `tls=1`. Names are stripped of control and format characters
//! and cut, and at most [`MAX_ADDRESSES`] addresses are kept. An announced server
//! is a suggestion of the lowest trust: it ranks after MDM and DNS-SRV, and an MDM policy can
//! switch it off (`forbid_announcement`, or `forbid_user_override`). Because anybody on the
//! network can announce, an announced server must never be used without the person seeing
//! which one it is: the signalling server sees room codes and addresses of the devices.
//!
//! UNCERTAIN: never run on a real network with several devices. Multicast may be filtered by
//! switches or access points, which is exactly the situation in which the receiver does not
//! show up (docs/measurement-points.md, A8). Apple and Android backends (NWBrowser,
//! NsdManager) are bridges in the apps (`docs/discovery.md`); this is the `mdns-sd` backend for
//! desktop.

use std::net::IpAddr;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use mdns_sd::{ResolvedService, ServiceDaemon, ServiceEvent};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::address::{clean_text, network_server, validate_hostname};
use crate::display::{AnnounceBackend, DisplayState, MdnsBackend, instance_name, txt_records};
use crate::nearby::{RawRecord, RawSink, ScreenMode};
use crate::signed::{AnnounceState, DisplayKey, SeqCounter, unix_ms, verify};

/// DNS-SD service type with domain, as `mdns-sd` wants it.
pub const MDNS_SERVICE_TYPE: &str = "_cometcast._tcp.local.";
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
    /// The server it announced, normalised, always with scheme: `wss://host:port` when
    /// `tls=1`, `ws://host:port` for a local-network host without it. `None` when it announced
    /// nothing or something that did not pass the checks (including plain `ws` for a host
    /// outside the local network).
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
    // A bounded number of TXT entries; only a record that verifies (v=2, id from pk, signature)
    // and says `live` is used. Anything else gives no server and no announced name.
    let pairs: Vec<(String, String)> = txt
        .into_iter()
        .take(32)
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
    let verified = verify(&pairs)
        .ok()
        .filter(|v| v.state == AnnounceState::Live);
    let announced_server = verified
        .as_ref()
        .and_then(|v| network_server(&v.srv, Some(v.tls)).ok().map(|(a, _)| a));
    let name = verified
        .as_ref()
        .map(|v| clean_text(&v.name, MAX_NAME_CHARS))
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
    /// The friendly name (the `name` key; cleaned and cut like a room name). The DNS-SD instance
    /// name is derived from the id of the key made for the announcement (`CometCast-xxxxxxxx`).
    pub instance_name: String,
    /// Port of the service. Comet Cast receivers do not listen for senders yet; UNCERTAIN: the
    /// value has no meaning for the protocol, mDNS only needs one. The value is not used: the
    /// display layout fixes it (`display::SERVICE_PORT`).
    pub port: u16,
    /// The server to announce: `host:port`, or with `wss://` / `ws://` (an announced `wss://`
    /// becomes `tls=1`). Required: a signed record names a server. The same rules as for
    /// received announcements apply (no loopback, no public IP literal), so a wrong value is
    /// found here, not at every sender.
    pub preferred_server: Option<String>,
    /// Host name for the `.local.` address records; random when `None`.
    pub host_label: Option<String>,
}

/// An active announcement. Dropping it publishes the signed `gone` record, withdraws the
/// service and stops the mDNS daemon.
pub struct Advertisement {
    backend: MdnsBackend,
    key: DisplayKey,
    seq: SeqCounter,
    state: DisplayState,
    instance: String,
    fullname: String,
}

impl Advertisement {
    /// The full DNS-SD name that is announced.
    pub fn fullname(&self) -> &str {
        &self.fullname
    }

    /// The id of the key made for this announcement.
    pub fn id(&self) -> String {
        self.key.id()
    }
}

impl Drop for Advertisement {
    fn drop(&mut self) {
        let seq = self.seq.next(unix_ms());
        if let Ok(txt) = txt_records(&self.state, &self.key, seq, AnnounceState::Gone)
            && self.backend.publish(&self.instance, &txt).is_ok()
        {
            self.backend.settle_before_goodbye();
        }
        self.backend.withdraw();
    }
}

/// Announce a locked display record that names `preferred_server`, signed with a new key.
pub fn advertise_receiver(opts: &AdvertiseOptions) -> Result<Advertisement, MdnsError> {
    let server = opts
        .preferred_server
        .clone()
        .ok_or_else(|| MdnsError("server cannot be announced: none given".into()))?;
    if let Some(l) = &opts.host_label {
        let label = validate_hostname(l).map_err(|e| MdnsError(e.to_string()))?;
        if label.contains('.') {
            return Err(MdnsError("host label must not contain dots".into()));
        }
    }
    let key = DisplayKey::generate();
    let state = DisplayState {
        room_name: opts.instance_name.clone(),
        server,
        mode: ScreenMode::Locked,
        code: None,
    };
    let mut seq = SeqCounter::new(0);
    let txt = txt_records(&state, &key, seq.next(unix_ms()), AnnounceState::Live)
        .map_err(|e| MdnsError(format!("server cannot be announced: {e}")))?;
    let instance = instance_name(&key.id());
    let mut backend = match &opts.host_label {
        Some(l) => MdnsBackend::with_host_label(l.clone()),
        None => MdnsBackend::new(),
    };
    backend.publish(&instance, &txt)?;
    Ok(Advertisement {
        fullname: format!("{instance}.{MDNS_SERVICE_TYPE}"),
        backend,
        key,
        seq,
        state,
        instance,
    })
}

/// Lifetime in seconds of every record of an announcement (SRV, addresses, PTR, TXT).
pub const RECORD_TTL_SECS: u32 = 30;

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

/// Stops a raw `mdns-sd` browse (and its daemon) when dropped.
pub struct MdnsBrowseGuard {
    daemon: ServiceDaemon,
    task: JoinHandle<()>,
}

impl Drop for MdnsBrowseGuard {
    fn drop(&mut self) {
        self.task.abort();
        let _ = self.daemon.stop_browse(MDNS_SERVICE_TYPE);
        let _ = self.daemon.shutdown();
    }
}

/// Browse with `mdns-sd` and hand the raw records to `sink` (the checks happen in
/// `crate::nearby`). Must be called inside a Tokio runtime.
pub(crate) fn start_raw_mdns_browse(sink: RawSink) -> Result<MdnsBrowseGuard, MdnsError> {
    let daemon = ServiceDaemon::new().map_err(|e| MdnsError(e.to_string()))?;
    let mut events = daemon
        .browse(MDNS_SERVICE_TYPE)
        .map_err(|e| MdnsError(e.to_string()))?;
    let browse_daemon = daemon.clone();
    let task = tokio::spawn(async move {
        // `mdns-sd` reports a record only when it is new or changed, but a display refreshes
        // its record without changing it, and a row is dropped after 30 s without a refresh.
        // So the browse is started again every `RE_BROWSE_EVERY`: the displays answer the new
        // query and the answers are reported again.
        let mut tick = tokio::time::interval_at(
            tokio::time::Instant::now() + RE_BROWSE_EVERY,
            RE_BROWSE_EVERY,
        );
        loop {
            tokio::select! {
                ev = events.recv_async() => {
                    let Ok(ev) = ev else { break };
                    forward_event(&sink, ev);
                }
                _ = tick.tick() => {
                    let _ = browse_daemon.stop_browse(MDNS_SERVICE_TYPE);
                    match browse_daemon.browse(MDNS_SERVICE_TYPE) {
                        Ok(e) => events = e,
                        Err(_) => break,
                    }
                }
            }
        }
    });
    Ok(MdnsBrowseGuard { daemon, task })
}

/// How often the raw browse is started again (see [`start_raw_mdns_browse`]).
const RE_BROWSE_EVERY: Duration = Duration::from_secs(10);

fn forward_event(sink: &RawSink, ev: ServiceEvent) {
    match ev {
        ServiceEvent::ServiceResolved(info) => {
            let Some(instance) = info
                .get_fullname()
                .strip_suffix(MDNS_SERVICE_TYPE)
                .and_then(|i| i.strip_suffix('.'))
            else {
                return;
            };
            let mut addresses: Vec<IpAddr> = info
                .get_addresses()
                .iter()
                .map(mdns_sd::ScopedIp::to_ip_addr)
                .collect();
            addresses.sort();
            addresses.truncate(MAX_ADDRESSES);
            sink.seen(RawRecord {
                instance: instance.to_owned(),
                addresses,
                txt: info
                    .get_properties()
                    .iter()
                    .take(32)
                    .map(|p| (p.key().to_owned(), p.val_str().to_owned()))
                    .collect(),
            });
        }
        ServiceEvent::ServiceRemoved(_, fullname) => {
            if let Some(i) = fullname
                .strip_suffix(MDNS_SERVICE_TYPE)
                .and_then(|i| i.strip_suffix('.'))
            {
                sink.gone(i.to_owned());
            }
        }
        _ => {}
    }
}

/// Remember `r` in `list` (at most [`MAX_RECEIVERS`] entries, one per instance name).
///
/// Anybody on the network can announce under any instance name, and the addresses of a record
/// (its A/AAAA records) are written by the sender as well, so neither proves which device sent
/// it. The FIRST announcement seen for a name is kept, and **its server is never replaced**: a
/// later record for the name with another server (or with a server where the first had none,
/// or none where it had one) is ignored. A later record with the same server, addresses and
/// port may update the entry (a new friendly name); it can point the entry nowhere else.
/// Returns whether the list changed.
pub fn record_receiver(list: &mut Vec<ReceiverInfo>, r: ReceiverInfo) -> bool {
    if let Some(old) = list.iter_mut().find(|o| o.instance == r.instance) {
        if old.announced_server == r.announced_server
            && old.addresses == r.addresses
            && old.port == r.port
            && *old != r
        {
            *old = r;
            return true;
        }
        return false;
    }
    if list.len() < MAX_RECEIVERS {
        list.push(r);
        return true;
    }
    false
}

/// Stops the browse and the daemon when dropped (also when the future is cancelled).
struct DaemonGuard(ServiceDaemon);

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        let _ = self.0.stop_browse(MDNS_SERVICE_TYPE);
        let _ = self.0.shutdown();
    }
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
        let daemon = DaemonGuard(ServiceDaemon::new().map_err(|e| MdnsError(e.to_string()))?);
        let events = daemon
            .0
            .browse(MDNS_SERVICE_TYPE)
            .map_err(|e| MdnsError(e.to_string()))?;
        let deadline = Instant::now() + wait;
        let mut found: Vec<ReceiverInfo> = Vec::new();
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            match tokio::time::timeout(left, events.recv_async()).await {
                Ok(Ok(ServiceEvent::ServiceResolved(info))) => {
                    if let Some(r) = receiver_from_resolved(&info) {
                        record_receiver(&mut found, r);
                    }
                }
                Ok(Ok(_)) => {}
                Ok(Err(_)) | Err(_) => break,
            }
        }
        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signed::sign_record;
    use mdns_sd::ServiceInfo;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    fn key() -> DisplayKey {
        DisplayKey::from_secret(&[9; 32]).unwrap()
    }

    /// A signed live record with the given extra fields.
    fn signed(fields: &[(&str, &str)]) -> Vec<(String, String)> {
        sign_record(&key(), 5, AnnounceState::Live, fields)
    }

    fn parts_txt(fullname: &str, txt: &[(String, String)]) -> Option<ReceiverInfo> {
        receiver_from_parts(
            fullname,
            9,
            [ip("fd00::2"), ip("192.168.0.9"), ip("192.168.0.9")],
            txt.iter().map(|(k, v)| (k.as_str(), v.as_str())),
        )
    }

    fn srv_of(fields: &[(&str, &str)]) -> Option<String> {
        let mut f = vec![("mode", "locked")];
        f.extend_from_slice(fields);
        parts_txt(FULL, &signed(&f)).unwrap().announced_server
    }

    const FULL: &str = "Room 1._cometcast._tcp.local.";

    #[test]
    fn parses_a_signed_announcement() {
        let r = parts_txt(
            FULL,
            &signed(&[
                ("srv", "Relay.Example.org:443"),
                ("tls", "1"),
                ("mode", "locked"),
                ("name", "Room 1 TV"),
            ]),
        )
        .unwrap();
        // `srv` is signed as written; a name with capitals is not a valid host for the display,
        // but a display never writes it; the checks lower-case it like for any announced server.
        assert_eq!(r.instance, "Room 1");
        assert_eq!(r.name, "Room 1 TV");
        assert_eq!(
            r.announced_server.as_deref(),
            Some("wss://relay.example.org:443")
        );
        assert_eq!(r.addresses, [ip("192.168.0.9"), ip("fd00::2")]);
        let r = parts_txt(
            FULL,
            &signed(&[("srv", "10.0.0.5:8443"), ("mode", "locked")]),
        )
        .unwrap();
        assert_eq!(r.announced_server.as_deref(), Some("ws://10.0.0.5:8443"));
        assert_eq!(r.name, "Room 1", "no name: the instance name");
    }

    #[test]
    fn unsigned_v1_and_tampered_records_give_no_server() {
        let v1 = [("v", "1"), ("srv", "10.0.0.5:8443"), ("name", "TV")];
        let txt: Vec<(String, String)> = v1
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        let r = parts_txt(FULL, &txt).unwrap();
        assert_eq!(r.announced_server, None);
        assert_eq!(r.name, "Room 1", "an unverified name is not used");
        // Changing the server of a signed record breaks the signature.
        let mut t = signed(&[("srv", "10.0.0.5:8443"), ("mode", "locked")]);
        t.iter_mut().find(|(k, _)| k == "srv").unwrap().1 = "10.66.0.1:1".into();
        assert_eq!(parts_txt(FULL, &t).unwrap().announced_server, None);
        // A signed `gone` record names no server.
        let g = sign_record(
            &key(),
            6,
            AnnounceState::Gone,
            &[("srv", "10.0.0.5:8443"), ("mode", "locked")],
        );
        assert_eq!(parts_txt(FULL, &g).unwrap().announced_server, None);
    }

    #[test]
    fn plain_ws_only_inside_the_local_network() {
        // A public name without tls=1 is refused, with tls=1 accepted.
        assert_eq!(srv_of(&[("srv", "relay.example.org:443")]), None);
        assert_eq!(
            srv_of(&[("srv", "relay.example.org:443"), ("tls", "1")]).as_deref(),
            Some("wss://relay.example.org:443")
        );
        // .local names and private literals may be plain, and tls=1 is honoured for them too.
        assert_eq!(
            srv_of(&[("srv", "relay.local:80")]).as_deref(),
            Some("ws://relay.local:80")
        );
        assert_eq!(
            srv_of(&[("srv", "192.168.0.2:80"), ("tls", "1")]).as_deref(),
            Some("wss://192.168.0.2:80")
        );
        // Link-local and metadata addresses are never accepted.
        assert_eq!(srv_of(&[("srv", "169.254.169.254:80")]), None);
        assert_eq!(srv_of(&[("srv", "169.254.10.10:80"), ("tls", "1")]), None);
        // Numeric/hex host spellings of loopback do not get through.
        assert_eq!(srv_of(&[("srv", "0x7f000001:80"), ("tls", "1")]), None);
    }

    fn rinfo(instance: &str, addr: &str, name: &str) -> ReceiverInfo {
        ReceiverInfo {
            instance: instance.into(),
            name: name.into(),
            addresses: vec![ip(addr)],
            port: 9,
            announced_server: None,
        }
    }

    #[test]
    fn first_seen_announcement_is_kept() {
        let mut l = Vec::new();
        assert!(record_receiver(&mut l, rinfo("tv", "10.0.0.1", "A")));
        // Another device claiming the same instance name does not replace it.
        assert!(!record_receiver(&mut l, rinfo("tv", "10.0.0.66", "Evil")));
        assert_eq!(l[0].name, "A");
        // The same addresses may update the record.
        assert!(record_receiver(&mut l, rinfo("tv", "10.0.0.1", "A2")));
        assert_eq!(l[0].name, "A2");
        assert!(!record_receiver(&mut l, rinfo("tv", "10.0.0.1", "A2")));
        assert_eq!(l.len(), 1);
    }

    #[test]
    fn the_server_of_an_entry_is_never_replaced() {
        let with = |srv: Option<&str>| ReceiverInfo {
            announced_server: srv.map(str::to_owned),
            ..rinfo("tv", "10.0.0.1", "A")
        };
        let mut l = Vec::new();
        assert!(record_receiver(&mut l, with(Some("ws://10.0.0.5:8443"))));
        // A device that copies the instance name AND the address (A records are written by
        // the sender) still cannot point the entry to its own server, nor remove the server.
        assert!(!record_receiver(&mut l, with(Some("ws://10.66.0.1:1"))));
        assert!(!record_receiver(&mut l, with(None)));
        let mut renamed = with(Some("ws://10.66.0.1:1"));
        renamed.name = "Evil".into();
        assert!(!record_receiver(&mut l, renamed));
        assert_eq!(l[0].announced_server.as_deref(), Some("ws://10.0.0.5:8443"));
        assert_eq!(l[0].name, "A");
        // An entry without a server does not get one later either.
        let mut l = vec![with(None)];
        assert!(!record_receiver(&mut l, with(Some("ws://10.66.0.1:1"))));
        assert_eq!(l[0].announced_server, None);
    }

    #[test]
    fn receiver_list_is_bounded() {
        let mut l = Vec::new();
        for i in 0..(MAX_RECEIVERS + 20) {
            record_receiver(&mut l, rinfo(&format!("tv{i}"), "10.0.0.1", "x"));
        }
        assert_eq!(l.len(), MAX_RECEIVERS);
    }

    #[test]
    fn no_announcement_is_fine() {
        let r = parts_txt(FULL, &[]).unwrap();
        assert_eq!(r.announced_server, None);
    }

    #[test]
    fn hostile_servers_are_refused_even_when_signed() {
        for bad in [
            "127.0.0.1:8443",
            "localhost:8443",
            "8.8.8.8:443",
            "[::1]:1",
            "wss://x.example:443",
            "x.example:443/ws",
            "relay.example.org:443",
            "user@x.example:443",
            "x.example",
            "x.example:0",
            "x y.example:1",
            "169.254.169.254:80",
            "0x7f000001:8443",
            "2130706433:8443",
        ] {
            assert_eq!(srv_of(&[("srv", bad)]), None, "{bad:?}");
        }
        let huge = format!("{}.example:1", "a".repeat(5000));
        assert_eq!(srv_of(&[("srv", &huge)]), None);
    }

    #[test]
    fn txt_is_bounded() {
        // The signed record hidden behind 40 junk entries is not looked at.
        let mut txt: Vec<(String, String)> =
            (0..40).map(|i| (format!("k{i}"), "x".into())).collect();
        txt.extend(signed(&[("srv", "10.0.0.5:8443"), ("mode", "locked")]));
        assert_eq!(parts_txt(FULL, &txt).unwrap().announced_server, None);
    }

    #[test]
    fn names_are_cleaned_and_bounded() {
        let long = "n".repeat(500);
        let name = format!("\u{1b}[31mTV{long}");
        let t = signed(&[
            ("srv", "10.0.0.5:8443"),
            ("mode", "locked"),
            ("name", &name),
        ]);
        // A value over 255 bytes is not accepted at all (and the record then has no server).
        let r = parts_txt(FULL, &t).unwrap();
        assert_eq!(r.announced_server, None);
        let name = format!("\u{1b}[31mTV{}", "n".repeat(100));
        let t = signed(&[
            ("srv", "10.0.0.5:8443"),
            ("mode", "locked"),
            ("name", &name),
        ]);
        let r = parts_txt(FULL, &t).unwrap();
        assert!(!r.name.contains('\u{1b}'));
        assert!(r.name.chars().count() <= MAX_NAME_CHARS);
        let t = signed(&[
            ("srv", "10.0.0.5:8443"),
            ("mode", "locked"),
            ("name", "\u{7}\u{7}"),
        ]);
        assert_eq!(
            parts_txt(FULL, &t).unwrap().name,
            "Room 1",
            "an empty name falls back to the instance"
        );
    }

    #[test]
    fn foreign_or_empty_instance_is_dropped() {
        assert!(parts_txt("x._http._tcp.local.", &[]).is_none());
        assert!(parts_txt("._cometcast._tcp.local.", &[]).is_none());
        assert!(parts_txt("_cometcast._tcp.local.", &[]).is_none());
        assert!(parts_txt("", &[]).is_none());
        let many: Vec<IpAddr> = (1..=50).map(|i| ip(&format!("10.0.0.{i}"))).collect();
        let r = receiver_from_parts(FULL, 1, many, []).unwrap();
        assert_eq!(r.addresses.len(), MAX_ADDRESSES);
    }

    #[test]
    fn announcements_use_a_record_lifetime_of_at_most_30_seconds() {
        // Decision N50: spec 14.8 point 5 wants a dead display gone within 35 s; a platform
        // cache keeps answering for the lifetime of the record.
        const { assert!(RECORD_TTL_SECS <= 30) };
        let mut info = ServiceInfo::new(
            MDNS_SERVICE_TYPE,
            "x",
            "h.local.",
            "10.0.0.2",
            1,
            &[("a", "b")][..],
        )
        .unwrap();
        info.set_host_ttl(RECORD_TTL_SECS);
        info.set_other_ttl(RECORD_TTL_SECS);
        assert_eq!(info.get_host_ttl(), 30);
        assert_eq!(info.get_other_ttl(), 30);
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
        // A signed record names a server: none given is an error.
        let o = AdvertiseOptions {
            instance_name: "".into(),
            port: 9,
            preferred_server: None,
            host_label: None,
        };
        assert!(advertise_receiver(&o).is_err());
        let o = AdvertiseOptions {
            instance_name: "T".into(),
            port: 9,
            preferred_server: Some("10.0.0.5:8443".into()),
            host_label: Some("a.b".into()),
        };
        assert!(advertise_receiver(&o).is_err());
    }
}
