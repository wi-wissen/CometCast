// SPDX-License-Identifier: GPL-3.0-or-later
//! The list of "screens on this network" (spec 14 of `docs/design/first-run-and-display.md`).
//!
//! Display devices announce `_cometcast._tcp` with the TXT layout of `docs/discovery.md`
//! (section 3). This module is the receiving side for presenter apps: it turns what a browse
//! backend saw (raw TXT pairs) into **rows keyed by the instance id of the screen**, with
//! expiry. It is pure state plus a small async driver, so tests need no network; the
//! backends (`mdns-sd` on desktops, `NsdManager` on Android, `NWBrowser` on Apple) only hand
//! over [`RawRecord`]s.
//!
//! Everything in a record is untrusted (any device on the network can send one):
//!
//! * the layout version must be `1`, the id 8 to 32 lower-case hex characters;
//! * the name is cleaned like a room name (spec 13.2) and cut at 32 characters; it may be
//!   empty (there is no fallback to the host name);
//! * the server passes [`network_server`] like every announced server (private IP literals
//!   only, TLS rule); a record without a usable server is dropped;
//! * the mode is `open`, `approval` or `locked`; for `open` and `approval` the code must be
//!   exactly 6 characters of the code alphabet, otherwise the record is dropped; a `locked`
//!   record never carries a code (a code in it is ignored);
//! * at most [`MAX_SCREENS`] rows; a row that is not refreshed within [`SCREEN_TTL`] is
//!   removed; a goodbye removes it at once. No record removes a row: an unusable record, or a
//!   record with a new id under the instance name of a row, is ignored for that row (the row
//!   leaves by goodbye or expiry);
//! * **the content of a row (server, code, mode, name) is the first content seen for its id**.
//!   A later record for the id with other content changes the row only when the shown content
//!   has not been announced for [`CHANGE_HOLD`] (the display really changed it), and such a
//!   record does not refresh the row. Only withdrawing the code (`locked`, same server and
//!   name) is applied at once: it makes the row untappable, it cannot point it elsewhere. The
//!   instance name, the id and the A/AAAA records are all chosen by the sender, so none of
//!   them proves that a record comes from the device of the row; while that device keeps
//!   announcing, another device therefore cannot point its row to another server or room.
//!   A record must also come from an instance of the row or from an address the row knew.
//!
//! What is **not** protected: a goodbye names only the instance and can be sent by anybody,
//! so a row can be removed; a device that then announces the id first decides the content of
//! the row that comes back (at the end of the list). The row always shows the code, which the
//! person can compare with the screen. UNCERTAIN: no backend hands over the source address of
//! the packets (`mdns-sd` merges several packets into one resolved service), and a source
//! address on a LAN can be forged as well (docs/measurement-points.md, A8).
//!
//! Nothing here picks a screen, remembers one or saves a server: the list is information.

use std::net::IpAddr;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::address::{MAX_ROOM_NAME_CHARS, clean_room_name, clean_text, network_server};
use crate::announce::{MdnsBrowseGuard, MdnsError, start_raw_mdns_browse};

/// Most screens in the list (spec 14.8, point 10).
pub const MAX_SCREENS: usize = 32;
/// A row leaves when its record was not refreshed for this long (spec 14.8, point 5).
pub const SCREEN_TTL: Duration = Duration::from_secs(30);
/// The only layout version this code understands.
pub const LAYOUT_VERSION: &str = "1";
/// The characters of a room code (the same as `cc_proto::RoomCode::ALPHABET`; a test in
/// `cc-ffi` keeps both equal).
pub const CODE_ALPHABET: &str = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
/// Length of a room code.
pub const CODE_LEN: usize = 6;
/// Longest value of one TXT entry that is looked at (a DNS-SD entry has at most 255 bytes).
const MAX_VALUE_BYTES: usize = 255;
/// Instance names remembered per row.
const MAX_INSTANCES_PER_ROW: usize = 4;
/// A record with other content than its row replaces the content only when the shown content
/// has not been announced for this long. Longer than the refresh of a display
/// ([`crate::display::REFRESH_EVERY`], 10 s), so a display that keeps announcing keeps its row;
/// shorter than [`SCREEN_TTL`], so a real change (a new code without a goodbye) shows in place
/// before the row would expire.
pub const CHANGE_HOLD: Duration = Duration::from_secs(15);

/// Who may join, as announced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScreenMode {
    /// Anybody who has the code joins at once.
    Open,
    /// Joining needs the approval of the person at the display.
    Approval,
    /// Nobody may join right now.
    Locked,
}

impl ScreenMode {
    /// The TXT value.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Approval => "approval",
            Self::Locked => "locked",
        }
    }

    /// Parse the TXT value (exact, lower case).
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "open" => Some(Self::Open),
            "approval" => Some(Self::Approval),
            "locked" => Some(Self::Locked),
            _ => None,
        }
    }
}

/// A record as a browse backend saw it, before any check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawRecord {
    /// DNS-SD instance name (not the full name).
    pub instance: String,
    /// Addresses the backend resolved; empty when it did not resolve any (Apple's browser
    /// hands over endpoints, not addresses).
    pub addresses: Vec<IpAddr>,
    /// The TXT pairs.
    pub txt: Vec<(String, String)>,
}

/// One screen in the list. All text is cleaned and bounded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NearbyScreen {
    /// The stable instance id of the screen (rows are merged by it).
    pub id: String,
    /// Cleaned room name, at most 32 characters; empty when the screen has none.
    pub name: String,
    /// The announced server with scheme (`wss://host:port` or `ws://host:port`). Not trusted:
    /// the app decides whether it may be used without asking.
    pub server: String,
    /// Who may join.
    pub mode: ScreenMode,
    /// The current code (6 characters of [`CODE_ALPHABET`]); `None` exactly when the mode is
    /// [`ScreenMode::Locked`].
    pub code: Option<String>,
    /// The IPv4 and IPv6 addresses the announcement resolved to (of every instance of this
    /// screen), as normalised text, sorted, at most [`crate::announce::MAX_ADDRESSES`]; empty
    /// when the backend resolved none. Used only to hide TV rows that are the same device
    /// (decision N105); never shown.
    pub addresses: Vec<String>,
}

/// The addresses as sorted, de-duplicated, normalised text (IPv4-mapped IPv6 as IPv4).
fn address_texts<'a>(addrs: impl IntoIterator<Item = &'a IpAddr>) -> Vec<String> {
    let mut v: Vec<IpAddr> = addrs.into_iter().map(IpAddr::to_canonical).collect();
    v.sort();
    v.dedup();
    v.truncate(crate::announce::MAX_ADDRESSES);
    v.iter().map(ToString::to_string).collect()
}

/// Is `code` exactly [`CODE_LEN`] characters of [`CODE_ALPHABET`]?
pub fn is_valid_code(code: &str) -> bool {
    code.chars().count() == CODE_LEN && code.chars().all(|c| CODE_ALPHABET.contains(c))
}

/// Is `id` a valid instance id (8 to 32 lower-case hex characters)?
pub fn is_valid_id(id: &str) -> bool {
    (8..=32).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Check and clean one record. `None` when it is not a usable announcement (see the module
/// documentation).
pub fn screen_from_raw(rec: &RawRecord) -> Option<NearbyScreen> {
    let (mut version, mut id, mut srv, mut tls, mut mode, mut code, mut name) =
        (None, None, None, false, None, None, None);
    // A bounded number of entries; the first occurrence of a key wins (`tls`: any `1`).
    for (k, v) in rec.txt.iter().take(32) {
        if v.len() > MAX_VALUE_BYTES {
            continue;
        }
        let v = v.as_str();
        match k.as_str() {
            "v" if version.is_none() => version = Some(v),
            "id" if id.is_none() => id = Some(v),
            "srv" if srv.is_none() => srv = Some(v),
            "tls" => tls |= v == "1",
            "mode" if mode.is_none() => mode = Some(v),
            "code" if code.is_none() => code = Some(v),
            "name" if name.is_none() => name = Some(v),
            _ => {}
        }
    }
    if version != Some(LAYOUT_VERSION) {
        return None;
    }
    let id = id.filter(|i| is_valid_id(i))?.to_owned();
    let (server, _) = network_server(srv?, Some(tls)).ok()?;
    let mode = ScreenMode::parse(mode?)?;
    let code = match mode {
        ScreenMode::Locked => None,
        ScreenMode::Open | ScreenMode::Approval => {
            let c = code?;
            if !is_valid_code(c) {
                return None;
            }
            Some(c.to_owned())
        }
    };
    let name = name.map(clean_room_name).unwrap_or_default();
    debug_assert!(name.chars().count() <= MAX_ROOM_NAME_CHARS);
    Some(NearbyScreen {
        id,
        name,
        server,
        mode,
        code,
        addresses: address_texts(&rec.addresses),
    })
}

/// A change of the list, in the order it happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NearbyChange {
    /// A screen appeared (it is at the end of the list).
    Added(NearbyScreen),
    /// A screen changed (code, mode, name or server); its place in the list stays.
    Updated(NearbyScreen),
    /// A screen left: goodbye, expiry, or it announced something unusable (its id).
    Removed(String),
}

struct Entry {
    screen: NearbyScreen,
    /// The instance names (and their addresses) this id was seen under.
    instances: Vec<(String, Vec<IpAddr>)>,
    /// When the content of [`Entry::screen`] was last announced.
    last_seen: Instant,
}

/// Do two rows show the same thing (everything but the addresses)?
fn same_content(a: &NearbyScreen, b: &NearbyScreen) -> bool {
    a.server == b.server && a.code == b.code && a.mode == b.mode && a.name == b.name
}

/// Does `new` only withdraw the code of `row` (Locked, same server and name)? That cannot
/// point the row anywhere else, and a withdrawn code must leave the list at once.
fn only_withdraws_code(row: &NearbyScreen, new: &NearbyScreen) -> bool {
    new.mode == ScreenMode::Locked && new.server == row.server && new.name == row.name
}

/// The rows, with their expiry. Pure: time is passed in.
#[derive(Default)]
pub struct NearbyTracker {
    entries: Vec<Entry>,
}

impl NearbyTracker {
    /// An empty list.
    pub fn new() -> Self {
        Self::default()
    }

    /// The rows in the order they were first found.
    pub fn screens(&self) -> Vec<NearbyScreen> {
        self.entries.iter().map(|e| e.screen.clone()).collect()
    }

    /// When the next row expires, if there is one.
    pub fn next_expiry(&self) -> Option<Instant> {
        self.entries.iter().map(|e| e.last_seen + SCREEN_TTL).min()
    }

    /// Remove every row (the search stopped: an entry that is not watched could be stale).
    pub fn clear(&mut self) -> Vec<NearbyChange> {
        self.entries
            .drain(..)
            .map(|e| NearbyChange::Removed(e.screen.id))
            .collect()
    }

    /// A record was seen (new or refreshed).
    pub fn on_seen(&mut self, rec: &RawRecord, now: Instant) -> Vec<NearbyChange> {
        let instance = clean_text(&rec.instance, 63);
        // An unusable record is ignored. It does not remove a row: anybody can send one under
        // the instance name of a row; a display that stops announcing something usable leaves
        // by expiry.
        let Some(screen) = screen_from_raw(rec) else {
            return Vec::new();
        };
        if instance.is_empty() {
            return Vec::new();
        }
        let mut addresses = rec.addresses.clone();
        addresses.sort();
        addresses.dedup();
        addresses.truncate(crate::announce::MAX_ADDRESSES);
        // A record with a new id under the instance name of another row does not remove that
        // row (the instance name is public, anybody can announce it): the old row leaves by
        // goodbye or expiry like any other.
        if let Some(e) = self.entries.iter_mut().find(|e| e.screen.id == screen.id) {
            let known = e.instances.iter().position(|(i, _)| *i == instance);
            let allowed = match known {
                Some(_) => true,
                None => {
                    !addresses.is_empty()
                        && e.instances
                            .iter()
                            .any(|(_, a)| a.iter().any(|x| addresses.contains(x)))
                }
            };
            if !allowed {
                return Vec::new();
            }
            // The first content seen for the id stays while it is announced (see the module
            // documentation): other content neither changes nor refreshes the row.
            let takes_over = same_content(&e.screen, &screen)
                || only_withdraws_code(&e.screen, &screen)
                || now.duration_since(e.last_seen) >= CHANGE_HOLD;
            if !takes_over {
                return Vec::new();
            }
            match known {
                Some(n) => e.instances[n].1 = addresses,
                None if e.instances.len() < MAX_INSTANCES_PER_ROW => {
                    e.instances.push((instance, addresses));
                }
                None => {}
            }
            e.last_seen = now;
            // The addresses of the row are those of all its instances.
            let mut screen = screen;
            screen.addresses = address_texts(e.instances.iter().flat_map(|(_, a)| a.iter()));
            if e.screen == screen {
                return Vec::new();
            }
            e.screen = screen.clone();
            return vec![NearbyChange::Updated(screen)];
        }

        if self.entries.len() >= MAX_SCREENS {
            // Spec 14.8, point 10 (decision N49): the first 32 found stay; a further screen is
            // neither reported nor stored. It is added at the end when its next refresh
            // arrives after a row has left.
            return Vec::new();
        }
        self.entries.push(Entry {
            screen: screen.clone(),
            instances: vec![(instance, addresses)],
            last_seen: now,
        });
        vec![NearbyChange::Added(screen)]
    }

    /// A goodbye (or the backend lost the service).
    pub fn on_gone(&mut self, instance: &str) -> Vec<NearbyChange> {
        let instance = clean_text(instance, 63);
        let mut changes = Vec::new();
        let mut n = 0;
        while n < self.entries.len() {
            let e = &mut self.entries[n];
            if e.instances.iter().any(|(i, _)| *i == instance) {
                e.instances.retain(|(i, _)| *i != instance);
                if e.instances.is_empty() {
                    changes.push(NearbyChange::Removed(self.entries.remove(n).screen.id));
                    continue;
                }
                // No change is reported for it: the addresses only hide TV rows.
                e.screen.addresses = address_texts(e.instances.iter().flat_map(|(_, a)| a.iter()));
            }
            n += 1;
        }
        changes
    }

    /// Remove the rows that were not refreshed within [`SCREEN_TTL`].
    pub fn expire(&mut self, now: Instant) -> Vec<NearbyChange> {
        let mut changes = Vec::new();
        self.entries.retain(|e| {
            let alive = now.duration_since(e.last_seen) < SCREEN_TTL;
            if !alive {
                changes.push(NearbyChange::Removed(e.screen.id.clone()));
            }
            alive
        });
        changes
    }
}

/// Does this machine have a local network to search in: a non-loopback interface with a
/// private address (spec 14.2 "No local network"; native-apps.md 4.1, computers).
pub fn local_network_available() -> bool {
    if_addrs::get_if_addrs().is_ok_and(|list| {
        list.iter()
            .any(|i| !i.is_loopback() && crate::address::is_private_ip(i.ip()))
    })
}

/// What a backend reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RawEvent {
    /// A record was seen or refreshed.
    Seen(RawRecord),
    /// The service of this instance name went away.
    Gone(String),
    /// Forget all rows (the backend restarted or the search stopped).
    Reset,
}

/// Where a backend puts what it sees. Cheap to clone; never blocks (a flood is dropped).
#[derive(Clone)]
pub struct RawSink(mpsc::Sender<RawEvent>);

impl RawSink {
    /// A record was seen.
    pub fn seen(&self, rec: RawRecord) {
        let _ = self.0.try_send(RawEvent::Seen(rec));
    }

    /// A service went away.
    pub fn gone(&self, instance: String) {
        let _ = self.0.try_send(RawEvent::Gone(instance));
    }

    /// Forget all rows.
    pub fn reset(&self) {
        let _ = self.0.try_send(RawEvent::Reset);
    }
}

/// A running search: raw events in, [`NearbyChange`]s out. Dropping it stops the search (and
/// the `mdns-sd` browse when it was started with [`NearbySearch::with_mdns`]).
pub struct NearbySearch {
    rx: mpsc::Receiver<NearbyChange>,
    sink: RawSink,
    task: JoinHandle<()>,
    _guard: Option<MdnsBrowseGuard>,
}

impl NearbySearch {
    /// A search without a backend; feed it through [`NearbySearch::sink`]. Must be called in
    /// a Tokio runtime.
    pub fn new() -> Self {
        Self::build(None)
    }

    /// A search that browses with `mdns-sd` (desktops). Must be called in a Tokio runtime.
    pub fn with_mdns() -> Result<Self, MdnsError> {
        let (raw_tx, raw_rx) = mpsc::channel(256);
        let sink = RawSink(raw_tx);
        let guard = start_raw_mdns_browse(sink.clone())?;
        Ok(Self::build_with(sink, raw_rx, Some(guard)))
    }

    fn build(guard: Option<MdnsBrowseGuard>) -> Self {
        let (raw_tx, raw_rx) = mpsc::channel(256);
        Self::build_with(RawSink(raw_tx), raw_rx, guard)
    }

    fn build_with(
        sink: RawSink,
        mut raw_rx: mpsc::Receiver<RawEvent>,
        guard: Option<MdnsBrowseGuard>,
    ) -> Self {
        let (tx, rx) = mpsc::channel(64);
        let task = tokio::spawn(async move {
            let mut tracker = NearbyTracker::new();
            loop {
                let expiry = tracker.next_expiry();
                let changes = tokio::select! {
                    ev = raw_rx.recv() => match ev {
                        None => break,
                        Some(RawEvent::Seen(rec)) => tracker.on_seen(&rec, Instant::now()),
                        Some(RawEvent::Gone(i)) => tracker.on_gone(&i),
                        Some(RawEvent::Reset) => tracker.clear(),
                    },
                    () = async {
                        match expiry {
                            Some(t) => tokio::time::sleep_until(t).await,
                            None => std::future::pending().await,
                        }
                    } => tracker.expire(Instant::now()),
                };
                for c in changes {
                    if tx.send(c).await.is_err() {
                        return;
                    }
                }
            }
        });
        Self {
            rx,
            sink,
            task,
            _guard: guard,
        }
    }

    /// Where a backend reports what it sees.
    pub fn sink(&self) -> RawSink {
        self.sink.clone()
    }

    /// The next change, or `None` when the search has ended.
    pub async fn next(&mut self) -> Option<NearbyChange> {
        self.rx.recv().await
    }
}

impl Default for NearbySearch {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for NearbySearch {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    const ID1: &str = "0123456789abcdef";
    const ID2: &str = "fedcba9876543210";

    fn txt(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    fn rec(instance: &str, addr: &str, pairs: &[(&str, &str)]) -> RawRecord {
        RawRecord {
            instance: instance.into(),
            addresses: if addr.is_empty() {
                vec![]
            } else {
                vec![ip(addr)]
            },
            txt: txt(pairs),
        }
    }

    fn open(id: &str, code: &str) -> Vec<(&'static str, String)> {
        vec![
            ("v", "1".into()),
            ("id", id.into()),
            ("srv", "10.0.0.5:8443".into()),
            ("mode", "open".into()),
            ("code", code.into()),
            ("name", "Room 204".into()),
        ]
    }

    fn pairs<'a>(v: &'a [(&'static str, String)]) -> Vec<(&'static str, &'a str)> {
        v.iter().map(|(k, s)| (*k, s.as_str())).collect()
    }

    fn screen(v: &[(&'static str, String)]) -> Option<NearbyScreen> {
        screen_from_raw(&rec("x", "10.0.0.9", &pairs(v)))
    }

    #[test]
    fn parses_a_good_record() {
        let s = screen(&open(ID1, "P4X9TR")).unwrap();
        assert_eq!(s.id, ID1);
        assert_eq!(s.name, "Room 204");
        assert_eq!(s.server, "ws://10.0.0.5:8443");
        assert_eq!(s.mode, ScreenMode::Open);
        assert_eq!(s.code.as_deref(), Some("P4X9TR"));
        // TLS flag and approval.
        let mut v = open(ID1, "P4X9TR");
        v[2] = ("srv", "relay.example.org:443".into());
        v[3] = ("mode", "approval".into());
        v.push(("tls", "1".into()));
        let s = screen(&v).unwrap();
        assert_eq!(s.server, "wss://relay.example.org:443");
        assert_eq!(s.mode, ScreenMode::Approval);
    }

    #[test]
    fn the_row_carries_the_normalised_addresses_of_the_announcement() {
        let r = RawRecord {
            instance: "a".into(),
            addresses: vec![ip("192.168.1.20"), ip("::ffff:192.168.1.20"), ip("fd00::2")],
            txt: txt(&pairs(&open(ID1, "P4X9TR"))),
        };
        let s = screen_from_raw(&r).unwrap();
        // An IPv4-mapped address is the IPv4 address; sorted, no duplicates.
        assert_eq!(s.addresses, ["192.168.1.20", "fd00::2"]);
        // A backend that resolves nothing (Apple's browser) gives an empty list.
        let s = screen_from_raw(&rec("a", "", &pairs(&open(ID1, "P4X9TR")))).unwrap();
        assert!(s.addresses.is_empty());
    }

    #[test]
    fn a_row_with_two_instances_has_the_addresses_of_both() {
        let mut t = NearbyTracker::new();
        let now = Instant::now();
        let v = open(ID1, "P4X9TR");
        t.on_seen(&rec("a", "192.168.1.20", &pairs(&v)), now);
        let r2 = RawRecord {
            instance: "b".into(),
            addresses: vec![ip("192.168.1.20"), ip("192.168.1.21")],
            txt: txt(&pairs(&v)),
        };
        let ch = t.on_seen(&r2, now);
        assert!(
            matches!(ch.as_slice(), [NearbyChange::Updated(s)] if s.addresses == ["192.168.1.20", "192.168.1.21"])
        );
        assert_eq!(t.screens()[0].addresses, ["192.168.1.20", "192.168.1.21"]);
        // One instance goes: its addresses leave the row, without a change event.
        assert!(t.on_gone("b").is_empty());
        assert_eq!(t.screens()[0].addresses, ["192.168.1.20"]);
    }

    #[test]
    fn locked_never_has_a_code() {
        let mut v = open(ID1, "P4X9TR");
        v[3] = ("mode", "locked".into());
        // A code in a locked record is ignored, not shown.
        assert_eq!(screen(&v).unwrap().code, None);
        v.retain(|(k, _)| *k != "code");
        let s = screen(&v).unwrap();
        assert_eq!((s.mode, s.code), (ScreenMode::Locked, None));
    }

    #[test]
    fn bad_codes_drop_the_record() {
        for bad in [
            "",
            "P4X9T",
            "P4X9TRR",
            "p4x9tr",
            "P4X9T0",
            "P4X9TI",
            "P4X9TO",
            "P4X9T1",
            "P4X 9T",
            "P4X9T\u{200b}",
            "ÄÖÜÄÖÜ",
        ] {
            assert!(screen(&open(ID1, bad)).is_none(), "{bad:?}");
        }
        let mut v = open(ID1, "P4X9TR");
        v.retain(|(k, _)| *k != "code");
        assert!(screen(&v).is_none(), "open without code");
    }

    #[test]
    fn hostile_fields_are_refused() {
        for bad_id in [
            "",
            "short",
            "ZZZZZZZZZZZZZZZZ",
            "0123456789ABCDEF",
            &"a".repeat(33),
            "0123456789abcde\n",
        ] {
            assert!(screen(&open(bad_id, "P4X9TR")).is_none(), "{bad_id:?}");
        }
        for bad_srv in [
            "127.0.0.1:8443",
            "8.8.8.8:443",
            "relay.example.org:443",
            "x.example",
            "a b:1",
            "169.254.169.254:80",
            "0x7f000001:1",
            "",
        ] {
            let mut v = open(ID1, "P4X9TR");
            v[2] = ("srv", bad_srv.into());
            assert!(screen(&v).is_none(), "{bad_srv:?}");
        }
        for bad_mode in ["", "Open", "busy", "open ", "OPEN"] {
            let mut v = open(ID1, "P4X9TR");
            v[3] = ("mode", bad_mode.into());
            assert!(screen(&v).is_none(), "{bad_mode:?}");
        }
        let mut v = open(ID1, "P4X9TR");
        v[0] = ("v", "2".into());
        assert!(screen(&v).is_none(), "other layout version");
        v.remove(0);
        assert!(screen(&v).is_none(), "no layout version");
        // A huge value is ignored as if it was absent.
        let mut v = open(ID1, "P4X9TR");
        v[2] = ("srv", format!("{}.example:1", "a".repeat(5000)));
        assert!(screen(&v).is_none());
    }

    #[test]
    fn names_are_cleaned_and_may_be_empty() {
        let mut v = open(ID1, "P4X9TR");
        v[5] = (
            "name",
            format!("\u{202e}Ra\tum\u{7}  204{}", "x".repeat(100)),
        );
        let s = screen(&v).unwrap();
        assert!(s.name.starts_with("Ra um 204x"));
        assert_eq!(s.name.chars().count(), 32);
        assert!(!s.name.contains('\u{202e}'));
        v[5] = ("name", "\u{7}\u{200b}".into());
        assert_eq!(screen(&v).unwrap().name, "");
        v.remove(5);
        assert_eq!(screen(&v).unwrap().name, "", "no fallback to the host name");
    }

    #[test]
    fn first_key_wins_and_txt_is_bounded() {
        let mut v = open(ID1, "P4X9TR");
        v.insert(1, ("id", ID2.into()));
        assert_eq!(screen(&v).unwrap().id, ID2.to_owned());
        let mut junk: Vec<(String, String)> =
            (0..40).map(|i| (format!("k{i}"), "x".into())).collect();
        junk.extend(txt(&pairs(&open(ID1, "P4X9TR"))));
        assert!(
            screen_from_raw(&RawRecord {
                instance: "x".into(),
                addresses: vec![],
                txt: junk
            })
            .is_none()
        );
    }

    fn t0() -> Instant {
        Instant::now()
    }

    fn seen(
        t: &mut NearbyTracker,
        inst: &str,
        addr: &str,
        v: &[(&'static str, String)],
        now: Instant,
    ) -> Vec<NearbyChange> {
        t.on_seen(&rec(inst, addr, &pairs(v)), now)
    }

    #[test]
    fn rows_are_merged_by_id_and_keep_their_place() {
        let mut t = NearbyTracker::new();
        let now = t0();
        let a = open(ID1, "P4X9TR");
        let b = open(ID2, "K7Q2MW");
        assert!(matches!(
            seen(&mut t, "a", "10.0.0.1", &a, now)[..],
            [NearbyChange::Added(_)]
        ));
        assert!(matches!(
            seen(&mut t, "b", "10.0.0.2", &b, now)[..],
            [NearbyChange::Added(_)]
        ));
        // A refresh without a change is silent.
        assert!(seen(&mut t, "a", "10.0.0.1", &a, now).is_empty());
        // A new code (the display restarted its room without a goodbye) updates in place (the
        // row does not move to the end) once the old code was not announced for CHANGE_HOLD.
        let a2 = open(ID1, "AAAAAA");
        let ch = seen(&mut t, "a", "10.0.0.1", &a2, now + CHANGE_HOLD);
        assert!(
            matches!(&ch[..], [NearbyChange::Updated(s)] if s.code.as_deref() == Some("AAAAAA"))
        );
        let ids: Vec<_> = t.screens().into_iter().map(|s| s.id).collect();
        assert_eq!(ids, [ID1, ID2]);
    }

    #[test]
    fn equal_names_are_two_rows() {
        let mut t = NearbyTracker::new();
        let now = t0();
        seen(&mut t, "a", "10.0.0.1", &open(ID1, "P4X9TR"), now);
        seen(&mut t, "b", "10.0.0.2", &open(ID2, "K7Q2MW"), now);
        let s = t.screens();
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].name, s[1].name);
    }

    #[test]
    fn same_screen_on_two_interfaces_is_one_row() {
        let mut t = NearbyTracker::new();
        let now = t0();
        let a = open(ID1, "P4X9TR");
        seen(&mut t, "a", "10.0.0.1", &a, now);
        // The same id under another instance name, from an address the row knows.
        let mut r = rec("a (2)", "10.0.0.1", &pairs(&a));
        r.addresses.push(ip("192.168.1.1"));
        // Only the addresses of the row grew: it is reported as updated (N105).
        let ch = t.on_seen(&r, now);
        assert!(
            matches!(ch.as_slice(), [NearbyChange::Updated(s)] if s.addresses == ["10.0.0.1", "192.168.1.1"])
        );
        assert_eq!(t.screens().len(), 1);
        // One goodbye does not remove the row, the second does.
        assert!(t.on_gone("a").is_empty());
        assert_eq!(t.on_gone("a (2)"), [NearbyChange::Removed(ID1.into())]);
        assert!(t.screens().is_empty());
    }

    #[test]
    fn a_second_device_cannot_take_over_an_id() {
        let mut t = NearbyTracker::new();
        let now = t0();
        seen(&mut t, "a", "10.0.0.1", &open(ID1, "P4X9TR"), now);
        let mut evil = open(ID1, "K7Q2MW");
        evil[2] = ("srv", "10.66.0.1:1".into());
        assert!(seen(&mut t, "evil", "10.0.0.66", &evil, now).is_empty());
        assert!(seen(&mut t, "evil", "", &evil, now).is_empty());
        assert_eq!(t.screens()[0].server, "ws://10.0.0.5:8443");
        assert_eq!(t.screens()[0].code.as_deref(), Some("P4X9TR"));
    }

    #[test]
    fn a_second_device_cannot_take_over_a_row_under_its_instance_name() {
        // The attacker copies everything public: the instance name, the id, the name and even
        // the address of the display (A/AAAA records are written by the sender).
        let mut t = NearbyTracker::new();
        let now = t0();
        let inst = "CometCast-01234567";
        seen(&mut t, inst, "10.0.0.1", &open(ID1, "P4X9TR"), now);
        let mut other_relay = open(ID1, "K7Q2MW");
        other_relay[2] = ("srv", "10.66.0.1:1".into());
        // The same relay with another room (an attacker's room on the default relay).
        let other_room = open(ID1, "K7Q2MW");
        let mut renamed = open(ID1, "P4X9TR");
        renamed[5] = ("name", "Room 205".into());
        let mut approval = open(ID1, "P4X9TR");
        approval[3] = ("mode", "approval".into());
        for s in 0..90u64 {
            let at = now + Duration::from_secs(s);
            // The display refreshes every 10 s ...
            if s % 10 == 0 {
                assert!(seen(&mut t, inst, "10.0.0.1", &open(ID1, "P4X9TR"), at).is_empty());
            }
            // ... the attacker every second, with every variant.
            for (addr, v) in [
                ("10.0.0.1", &other_relay),
                ("10.0.0.66", &other_relay),
                ("", &other_relay),
                ("10.0.0.1", &other_room),
                ("10.0.0.1", &renamed),
                ("10.0.0.1", &approval),
            ] {
                assert!(
                    seen(&mut t, inst, addr, v, at).is_empty(),
                    "taken over at {s} s"
                );
            }
            assert!(t.expire(at).is_empty());
        }
        let row = &t.screens()[0];
        assert_eq!(row.server, "ws://10.0.0.5:8443");
        assert_eq!(row.code.as_deref(), Some("P4X9TR"));
        assert_eq!(
            (row.mode, row.name.as_str()),
            (ScreenMode::Open, "Room 204")
        );
        // The attacker's records do not refresh the row: when the display stops, it expires.
        // (Once the display has been quiet for CHANGE_HOLD, other content is taken as a real
        // change; that is the documented limit.)
        let last = now + Duration::from_secs(80);
        for s in 81..95u64 {
            seen(
                &mut t,
                inst,
                "10.0.0.1",
                &other_room,
                now + Duration::from_secs(s),
            );
        }
        assert_eq!(
            t.expire(last + SCREEN_TTL),
            [NearbyChange::Removed(ID1.into())]
        );
    }

    #[test]
    fn a_real_change_shows_once_the_old_content_is_quiet() {
        let mut t = NearbyTracker::new();
        let now = t0();
        seen(&mut t, "a", "10.0.0.1", &open(ID1, "P4X9TR"), now);
        // A new code right after the last refresh of the old one is held ...
        let new = open(ID1, "K7Q2MW");
        let one = Duration::from_secs(1);
        assert!(seen(&mut t, "a", "10.0.0.1", &new, now + one).is_empty());
        assert!(seen(&mut t, "a", "10.0.0.1", &new, now + 11 * one).is_empty());
        assert_eq!(t.screens()[0].code.as_deref(), Some("P4X9TR"));
        // ... and shown in place with the first refresh after CHANGE_HOLD.
        let ch = seen(&mut t, "a", "10.0.0.1", &new, now + 21 * one);
        assert!(
            matches!(&ch[..], [NearbyChange::Updated(s)] if s.code.as_deref() == Some("K7Q2MW"))
        );
        // From now on the old code is the one that is held.
        assert!(
            seen(
                &mut t,
                "a",
                "10.0.0.1",
                &open(ID1, "P4X9TR"),
                now + 22 * one
            )
            .is_empty()
        );
        // A new server on the display is applied the same way (it is not pinned for ever).
        let mut moved = open(ID1, "K7Q2MW");
        moved[2] = ("srv", "10.0.0.6:8443".into());
        assert!(seen(&mut t, "a", "10.0.0.1", &moved, now + 23 * one).is_empty());
        let ch = seen(
            &mut t,
            "a",
            "10.0.0.1",
            &moved,
            now + 21 * one + CHANGE_HOLD,
        );
        assert!(matches!(&ch[..], [NearbyChange::Updated(s)] if s.server == "ws://10.0.0.6:8443"));
        assert!(CHANGE_HOLD > crate::display::REFRESH_EVERY && CHANGE_HOLD < SCREEN_TTL);
    }

    #[test]
    fn locking_shows_at_once_unlocking_after_the_hold() {
        let mut t = NearbyTracker::new();
        let now = t0();
        let mut locked = open(ID1, "P4X9TR");
        locked[3] = ("mode", "locked".into());
        locked.retain(|(k, _)| *k != "code");
        seen(&mut t, "a", "10.0.0.1", &open(ID1, "P4X9TR"), now);
        // Withdrawing the code cannot point the row anywhere: at once.
        let ch = seen(&mut t, "a", "10.0.0.1", &locked, now);
        assert!(matches!(&ch[..], [NearbyChange::Updated(s)] if s.code.is_none()));
        // Open again (possibly with a code chosen by somebody else): only after the hold.
        let again = open(ID1, "K7Q2MW");
        assert!(
            seen(
                &mut t,
                "a",
                "10.0.0.1",
                &again,
                now + Duration::from_secs(5)
            )
            .is_empty()
        );
        let ch = seen(&mut t, "a", "10.0.0.1", &again, now + CHANGE_HOLD);
        assert!(
            matches!(&ch[..], [NearbyChange::Updated(s)] if s.code.as_deref() == Some("K7Q2MW"))
        );
        // "Locked" with another name or server is a change like any other.
        let mut locked_elsewhere = locked.clone();
        locked_elsewhere[2] = ("srv", "10.66.0.1:1".into());
        assert!(
            seen(
                &mut t,
                "a",
                "10.0.0.1",
                &locked_elsewhere,
                now + CHANGE_HOLD
            )
            .is_empty()
        );
    }

    #[test]
    fn goodbye_removes_the_row_an_unusable_record_does_not() {
        let mut t = NearbyTracker::new();
        let now = t0();
        seen(&mut t, "a", "10.0.0.1", &open(ID1, "P4X9TR"), now);
        assert_eq!(t.on_gone("a"), [NearbyChange::Removed(ID1.into())]);
        assert!(t.on_gone("a").is_empty());
        seen(&mut t, "a", "10.0.0.1", &open(ID1, "P4X9TR"), now);
        // A broken record under the instance name of the row (anybody can send one) neither
        // removes nor refreshes it; the row leaves by expiry.
        let later = now + Duration::from_secs(20);
        assert!(seen(&mut t, "a", "10.0.0.1", &open(ID1, "bad"), later).is_empty());
        assert_eq!(t.screens().len(), 1);
        assert_eq!(
            t.expire(now + SCREEN_TTL),
            [NearbyChange::Removed(ID1.into())]
        );
    }

    #[test]
    fn rows_expire_after_30_seconds_without_refresh() {
        let mut t = NearbyTracker::new();
        let now = t0();
        seen(&mut t, "a", "10.0.0.1", &open(ID1, "P4X9TR"), now);
        seen(
            &mut t,
            "b",
            "10.0.0.2",
            &open(ID2, "K7Q2MW"),
            now + Duration::from_secs(20),
        );
        assert_eq!(t.next_expiry(), Some(now + SCREEN_TTL));
        assert!(t.expire(now + Duration::from_secs(29)).is_empty());
        assert_eq!(
            t.expire(now + Duration::from_secs(30)),
            [NearbyChange::Removed(ID1.into())]
        );
        // A refresh keeps a row alive.
        seen(
            &mut t,
            "b",
            "10.0.0.2",
            &open(ID2, "K7Q2MW"),
            now + Duration::from_secs(45),
        );
        assert!(t.expire(now + Duration::from_secs(60)).is_empty());
        assert_eq!(t.expire(now + Duration::from_secs(75)).len(), 1);
    }

    #[test]
    fn the_33rd_screen_does_not_push_a_row_out() {
        let mut t = NearbyTracker::new();
        let now = t0();
        let id = |i: u32| format!("{i:016x}");
        for i in 0..32u32 {
            let ch = seen(
                &mut t,
                &format!("s{i}"),
                "10.0.0.1",
                &open(&id(i), "P4X9TR"),
                now,
            );
            assert_eq!(ch.len(), 1);
        }
        // The 33rd and later are not reported and not stored.
        for i in 32..40u32 {
            let ch = seen(
                &mut t,
                &format!("s{i}"),
                "10.0.0.1",
                &open(&id(i), "P4X9TR"),
                now,
            );
            assert!(ch.is_empty(), "screen {i} was reported");
        }
        let s = t.screens();
        assert_eq!(s.len(), MAX_SCREENS);
        for (n, row) in s.iter().enumerate() {
            assert_eq!(row.id, id(n as u32), "row {n} changed");
        }
        // Row 5 leaves: the 33rd is reported at the end with its next refresh.
        assert_eq!(t.on_gone("s5").len(), 1);
        let ch = seen(&mut t, "s32", "10.0.0.1", &open(&id(32), "P4X9TR"), now);
        assert!(matches!(&ch[..], [NearbyChange::Added(a)] if a.id == id(32)));
        let s = t.screens();
        assert_eq!(s.len(), MAX_SCREENS);
        assert_eq!(s[31].id, id(32));
        assert_eq!(s[5].id, id(6));
    }

    #[test]
    fn clear_removes_everything() {
        let mut t = NearbyTracker::new();
        let now = t0();
        seen(&mut t, "a", "10.0.0.1", &open(ID1, "P4X9TR"), now);
        seen(&mut t, "b", "10.0.0.2", &open(ID2, "K7Q2MW"), now);
        assert_eq!(t.clear().len(), 2);
        assert!(t.screens().is_empty() && t.next_expiry().is_none());
    }

    #[test]
    fn a_new_id_under_the_instance_name_of_a_row_does_not_evict_it() {
        let mut t = NearbyTracker::new();
        let now = t0();
        let inst = "CometCast-01234567";
        seen(&mut t, inst, "10.0.0.1", &open(ID1, "P4X9TR"), now);
        // Another device (or the same one after a data reset) under the same instance name.
        let ch = seen(&mut t, inst, "10.0.0.1", &open(ID2, "K7Q2MW"), now);
        assert!(matches!(&ch[..], [NearbyChange::Added(s)] if s.id == ID2));
        let ids: Vec<_> = t.screens().into_iter().map(|s| s.id).collect();
        assert_eq!(ids, [ID1, ID2]);
        // The first row stays while it is refreshed, and leaves by expiry once it is not.
        let at = now + Duration::from_secs(20);
        seen(&mut t, inst, "10.0.0.1", &open(ID1, "P4X9TR"), at);
        seen(
            &mut t,
            inst,
            "10.0.0.1",
            &open(ID2, "K7Q2MW"),
            at + SCREEN_TTL,
        );
        assert!(t.expire(at + Duration::from_secs(29)).is_empty());
        assert_eq!(
            t.expire(at + SCREEN_TTL),
            [NearbyChange::Removed(ID1.into())]
        );
        assert_eq!(t.screens()[0].id, ID2);
    }

    #[tokio::test(start_paused = true)]
    async fn the_driver_expires_rows_by_itself() {
        let mut s = NearbySearch::new();
        let sink = s.sink();
        sink.seen(rec("a", "10.0.0.1", &pairs(&open(ID1, "P4X9TR"))));
        assert!(matches!(s.next().await, Some(NearbyChange::Added(_))));
        // No refresh: the driver removes the row when the clock passes 30 s.
        let start = Instant::now();
        let next = s.next().await;
        assert_eq!(next, Some(NearbyChange::Removed(ID1.into())));
        assert!(start.elapsed() >= SCREEN_TTL);
        // Reset and goodbye through the sink.
        sink.seen(rec("a", "10.0.0.1", &pairs(&open(ID1, "P4X9TR"))));
        assert!(matches!(s.next().await, Some(NearbyChange::Added(_))));
        sink.gone("a".into());
        assert_eq!(s.next().await, Some(NearbyChange::Removed(ID1.into())));
        sink.seen(rec("a", "10.0.0.1", &pairs(&open(ID1, "P4X9TR"))));
        assert!(matches!(s.next().await, Some(NearbyChange::Added(_))));
        sink.reset();
        assert_eq!(s.next().await, Some(NearbyChange::Removed(ID1.into())));
    }
}
