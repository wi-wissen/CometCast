// SPDX-License-Identifier: GPL-3.0-or-later
//! The list of "screens on this network" (spec 14 of `docs/design/first-run-and-display.md`,
//! trust rules of `docs/design/signed-announcements.md`).
//!
//! Display devices announce `_cometcast._tcp` with the signed TXT layout `v=2` of
//! `docs/discovery.md` (section 3). This module is the receiving side for presenter apps: it
//! turns what a browse backend saw (raw TXT pairs) into **rows keyed by the verified id of the
//! screen**, with expiry. It is pure state plus a small async driver, so tests need no network;
//! the backends (`mdns-sd` on desktops, `NsdManager` on Android, `NWBrowser` on Apple) only hand
//! over [`RawRecord`]s. Every check is here, in Rust, for every platform.
//!
//! A record is accepted only if **all** of these hold, otherwise it is ignored (it never
//! changes or removes a row):
//!
//! 1. `v=2`, all required keys present, values well-formed; `srv` passes [`network_server`]
//!    (private IP literals only, TLS rule); `mode` is `open`, `approval` or `locked`; for `open`
//!    and `approval` the code is exactly 6 characters of the code alphabet; a `locked` record's
//!    code is ignored; the name is cleaned like a room name and cut at 32 characters;
//! 2. `id` equals the id derived from `pk` (first 16 bytes of SHA-256, 32 hex digits);
//! 3. `sig` verifies over the signed bytes with `pk` (strict Ed25519, [`crate::signed`]);
//! 4. `seq` is greater than the highest `seq` accepted for this id (kept while the row exists
//!    and for [`SEQ_MEMORY_AFTER_GONE`] after it is gone, at most [`MAX_SEQ_MEMORY`] ids, oldest
//!    dropped first).
//!
//! An accepted record replaces the row at once (only the key owner can make a newer record).
//! A row is removed **only** by an accepted `state=gone` record or by expiry ([`SCREEN_TTL`]
//! without an accepted record). A goodbye from the network ([`NearbyTracker::on_gone`]) only
//! marks the row unconfirmed internally and has no visible effect. The instance name is never
//! used to match or remove a row.
//!
//! The list also marks rows: **same name** (two rows with the same cleaned name, compared
//! case-insensitively) and **new device with a known name** (a remembered `(name, id)` pair of
//! the app has the name but not this id). Nothing here picks a screen, remembers one or saves a
//! server: the list is information; the app passes the remembered pairs in
//! ([`NearbyTracker::set_remembered`]).
//!
//! What signing does **not** protect: a device can announce its **own** id with any name,
//! relay and code (see `docs/discovery.md`); a record can be withheld, and then the row expires.

use std::net::IpAddr;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::address::{MAX_ROOM_NAME_CHARS, clean_room_name, clean_text, network_server};
use crate::announce::{MdnsBrowseGuard, MdnsError, start_raw_mdns_browse};
use crate::signed::{self, AnnounceState, Reject};

/// Most screens in the list (spec 14.8, point 10).
pub const MAX_SCREENS: usize = 32;
/// A row leaves when no record was accepted for it for this long (spec 14.8, point 5).
pub const SCREEN_TTL: Duration = Duration::from_secs(30);
/// The only layout version this code understands.
pub const LAYOUT_VERSION: &str = signed::LAYOUT_VERSION_V2;
/// The characters of a room code (the same as `cc_proto::RoomCode::ALPHABET`; a test in
/// `cc-ffi` keeps both equal).
pub const CODE_ALPHABET: &str = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
/// Length of a room code.
pub const CODE_LEN: usize = 6;
/// Instance names remembered per row (only to collect addresses and to mark a goodbye).
const MAX_INSTANCES_PER_ROW: usize = 4;
/// How long the highest accepted `seq` of an id is kept after its row is gone.
pub const SEQ_MEMORY_AFTER_GONE: Duration = Duration::from_secs(600);
/// Most ids whose highest `seq` is remembered.
pub const MAX_SEQ_MEMORY: usize = 256;
/// Most remembered `(name, id)` pairs the app may pass.
pub const MAX_REMEMBERED: usize = 50;

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
    /// The verified id of the screen (32 hex digits, derived from its public key); rows are
    /// keyed by it.
    pub id: String,
    /// The short tag: the first 4 characters of the id in upper case (`3F7A`).
    pub tag: String,
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
    /// Another row has the same name (cleaned, compared case-insensitively).
    pub same_name: bool,
    /// The app remembers a screen with this name but another id.
    pub new_device: bool,
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

/// Is `id` a valid id (exactly 32 lower-case hex digits)?
pub fn is_valid_id(id: &str) -> bool {
    signed::is_valid_id(id)
}

/// A record that passed every check except the `seq` memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checked {
    /// The row the record describes (marks are not set).
    pub screen: NearbyScreen,
    /// The sequence number.
    pub seq: u64,
    /// `live` or `gone`.
    pub state: AnnounceState,
}

/// Check one record: version, shape, id derivation, signature and the content rules (see the
/// module documentation). The `seq` is not compared with anything here.
pub fn check_record(rec: &RawRecord) -> Result<Checked, Reject> {
    let v = signed::verify(&rec.txt)?;
    let (server, _) = network_server(&v.srv, Some(v.tls)).map_err(|_| Reject::Malformed)?;
    let mode = ScreenMode::parse(&v.mode).ok_or(Reject::Malformed)?;
    let code = match mode {
        ScreenMode::Locked => None,
        ScreenMode::Open | ScreenMode::Approval => {
            if !is_valid_code(&v.code) {
                return Err(Reject::Malformed);
            }
            Some(v.code.clone())
        }
    };
    let name = clean_room_name(&v.name);
    debug_assert!(name.chars().count() <= MAX_ROOM_NAME_CHARS);
    Ok(Checked {
        screen: NearbyScreen {
            tag: signed::tag_of(&v.id),
            id: v.id,
            name,
            server,
            mode,
            code,
            addresses: address_texts(&rec.addresses),
            same_name: false,
            new_device: false,
        },
        seq: v.seq,
        state: v.state,
    })
}

/// A change of the list, in the order it happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NearbyChange {
    /// A screen appeared (it is at the end of the list).
    Added(NearbyScreen),
    /// A screen changed (code, mode, name, server or a mark); its place in the list stays.
    Updated(NearbyScreen),
    /// A screen left: a signed `gone` record, or expiry (its id).
    Removed(String),
}

struct Entry {
    screen: NearbyScreen,
    /// The instance names (and their addresses) this id was seen under. Used only to collect
    /// addresses and to mark a goodbye; never to match or remove a row.
    instances: Vec<(String, Vec<IpAddr>)>,
    /// When the last record was accepted for this row.
    last_seen: Instant,
    /// A goodbye from the network was seen and no accepted record since (internal; no visible
    /// effect).
    unconfirmed: bool,
}

/// The highest `seq` accepted for an id.
struct SeqMemory {
    id: String,
    seq: u64,
    /// Since when the row is gone (`None` while it exists).
    gone_at: Option<Instant>,
}

/// A name for comparing: cleaned, lower case.
fn name_key(name: &str) -> String {
    name.to_lowercase()
}

/// The rows, with their expiry and the `seq` memory. Pure: time is passed in.
#[derive(Default)]
pub struct NearbyTracker {
    entries: Vec<Entry>,
    /// Ordered by the last update: the oldest is first.
    seqs: Vec<SeqMemory>,
    /// `(name key, id)` pairs of screens the app remembers.
    remembered: Vec<(String, String)>,
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

    /// The highest `seq` accepted for `id`, if it is remembered.
    pub fn highest_seq(&self, id: &str) -> Option<u64> {
        self.seqs.iter().find(|m| m.id == id).map(|m| m.seq)
    }

    /// Did a goodbye come from the network for this row, with no accepted record since? This
    /// has no visible effect; it is kept for the diagnostics of the app.
    pub fn is_unconfirmed(&self, id: &str) -> bool {
        self.entries
            .iter()
            .any(|e| e.screen.id == id && e.unconfirmed)
    }

    /// Remove every row and forget every `seq` (the search stopped or the backend restarted: a
    /// new start knows nothing, like an app that was just opened).
    pub fn clear(&mut self) -> Vec<NearbyChange> {
        self.seqs.clear();
        self.entries
            .drain(..)
            .map(|e| NearbyChange::Removed(e.screen.id))
            .collect()
    }

    /// The app passes the `(room name, id)` pairs it remembers (at most [`MAX_REMEMBERED`];
    /// names are compared case-insensitively, a pair without a name is ignored). Rows are marked
    /// "new device" when a pair has their name but not their id.
    pub fn set_remembered(&mut self, pairs: &[(String, String)]) -> Vec<NearbyChange> {
        let before = self.screens();
        let mut out: Vec<(String, String)> = Vec::new();
        for (name, id) in pairs.iter().take(MAX_REMEMBERED) {
            let key = name_key(&clean_room_name(name));
            if key.is_empty() || !is_valid_id(id) {
                continue;
            }
            let pair = (key, id.clone());
            if !out.contains(&pair) {
                out.push(pair);
            }
        }
        self.remembered = out;
        self.finish(&before)
    }

    /// A record was seen (new or refreshed).
    pub fn on_seen(&mut self, rec: &RawRecord, now: Instant) -> Vec<NearbyChange> {
        let before = self.screens();
        self.apply(rec, now);
        self.finish(&before)
    }

    fn apply(&mut self, rec: &RawRecord, now: Instant) {
        let instance = clean_text(&rec.instance, 63);
        if instance.is_empty() {
            return;
        }
        self.purge_seqs(now);
        // An unusable or unverifiable record is ignored: it never changes or removes a row.
        let Ok(checked) = check_record(rec) else {
            return;
        };
        let id = checked.screen.id.clone();
        if self.highest_seq(&id).is_some_and(|h| checked.seq <= h) {
            // A replay (or a repeat of the record already accepted).
            return;
        }
        let mut addresses = rec.addresses.clone();
        addresses.sort();
        addresses.dedup();
        addresses.truncate(crate::announce::MAX_ADDRESSES);
        match checked.state {
            AnnounceState::Gone => {
                self.entries.retain(|e| e.screen.id != id);
                self.remember_seq(&id, checked.seq, Some(now));
            }
            AnnounceState::Live => {
                if let Some(e) = self.entries.iter_mut().find(|e| e.screen.id == id) {
                    match e.instances.iter().position(|(i, _)| *i == instance) {
                        Some(n) => e.instances[n].1 = addresses,
                        None if e.instances.len() < MAX_INSTANCES_PER_ROW => {
                            e.instances.push((instance, addresses));
                        }
                        None => {}
                    }
                    e.last_seen = now;
                    e.unconfirmed = false;
                    let mut screen = checked.screen;
                    screen.addresses =
                        address_texts(e.instances.iter().flat_map(|(_, a)| a.iter()));
                    e.screen = screen;
                } else if self.entries.len() < MAX_SCREENS {
                    // Spec 14.8, point 10 (decision N49): the first 32 found stay; a further
                    // screen is neither reported nor stored. It is added at the end when its
                    // next record arrives after a row has left.
                    self.entries.push(Entry {
                        screen: checked.screen,
                        instances: vec![(instance, addresses)],
                        last_seen: now,
                        unconfirmed: false,
                    });
                } else {
                    return;
                }
                self.remember_seq(&id, checked.seq, None);
            }
        }
    }

    /// A goodbye (or the backend lost the service). It proves nothing: anybody can send one. It
    /// only marks the rows seen under this instance name as unconfirmed, internally; the list,
    /// the codes and the expiry do not change.
    pub fn on_gone(&mut self, instance: &str) {
        let instance = clean_text(instance, 63);
        for e in &mut self.entries {
            if e.instances.iter().any(|(i, _)| *i == instance) {
                e.unconfirmed = true;
            }
        }
    }

    /// Remove the rows that had no accepted record within [`SCREEN_TTL`].
    pub fn expire(&mut self, now: Instant) -> Vec<NearbyChange> {
        let before = self.screens();
        let mut gone: Vec<String> = Vec::new();
        self.entries.retain(|e| {
            let alive = now.duration_since(e.last_seen) < SCREEN_TTL;
            if !alive {
                gone.push(e.screen.id.clone());
            }
            alive
        });
        for id in gone {
            if let Some(m) = self.seqs.iter_mut().find(|m| m.id == id) {
                m.gone_at = Some(now);
            }
        }
        self.purge_seqs(now);
        self.finish(&before)
    }

    /// Forget the `seq` of ids that have been gone for a long time.
    fn purge_seqs(&mut self, now: Instant) {
        self.seqs.retain(|m| {
            m.gone_at
                .is_none_or(|t| now.saturating_duration_since(t) < SEQ_MEMORY_AFTER_GONE)
        });
    }

    fn remember_seq(&mut self, id: &str, seq: u64, gone_at: Option<Instant>) {
        self.seqs.retain(|m| m.id != id);
        if self.seqs.len() >= MAX_SEQ_MEMORY {
            // Oldest first; an id that still has a row is dropped only if there is no other.
            let n = self
                .seqs
                .iter()
                .position(|m| !self.entries.iter().any(|e| e.screen.id == m.id))
                .unwrap_or(0);
            self.seqs.remove(n);
        }
        self.seqs.push(SeqMemory {
            id: id.to_owned(),
            seq,
            gone_at,
        });
    }

    /// Set the marks of every row and report what differs from `before`.
    fn finish(&mut self, before: &[NearbyScreen]) -> Vec<NearbyChange> {
        let keys: Vec<String> = self
            .entries
            .iter()
            .map(|e| name_key(&e.screen.name))
            .collect();
        for (n, e) in self.entries.iter_mut().enumerate() {
            let key = &keys[n];
            let named = !key.is_empty();
            e.screen.same_name = named && keys.iter().filter(|k| *k == key).count() > 1;
            e.screen.new_device = named
                && self.remembered.iter().any(|(k, _)| k == key)
                && !self
                    .remembered
                    .iter()
                    .any(|(k, id)| k == key && *id == e.screen.id);
        }
        let mut changes = Vec::new();
        for b in before {
            if !self.entries.iter().any(|e| e.screen.id == b.id) {
                changes.push(NearbyChange::Removed(b.id.clone()));
            }
        }
        for e in &self.entries {
            match before.iter().find(|b| b.id == e.screen.id) {
                Some(b) if *b != e.screen => changes.push(NearbyChange::Updated(e.screen.clone())),
                Some(_) => {}
                None => changes.push(NearbyChange::Added(e.screen.clone())),
            }
        }
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
    /// The service of this instance name went away (it only marks the row unconfirmed).
    Gone(String),
    /// Forget all rows (the backend restarted or the search stopped).
    Reset,
    /// The remembered `(room name, id)` pairs of the app.
    Remembered(Vec<(String, String)>),
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

    /// The app's remembered pairs changed.
    pub fn remembered(&self, pairs: Vec<(String, String)>) {
        let _ = self.0.try_send(RawEvent::Remembered(pairs));
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
                        Some(RawEvent::Gone(i)) => {
                            tracker.on_gone(&i);
                            Vec::new()
                        }
                        Some(RawEvent::Reset) => tracker.clear(),
                        Some(RawEvent::Remembered(p)) => tracker.set_remembered(&p),
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
    use crate::signed::{DisplayKey, sign_record};

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    fn key(n: u8) -> DisplayKey {
        DisplayKey::from_secret(&[n; 32]).unwrap()
    }

    /// What a signed record says.
    #[derive(Clone)]
    struct Say {
        srv: &'static str,
        mode: &'static str,
        code: Option<&'static str>,
        name: Option<&'static str>,
    }

    fn say(code: &'static str) -> Say {
        Say {
            srv: "10.0.0.5:8443",
            mode: "open",
            code: Some(code),
            name: Some("Room 204"),
        }
    }

    fn txt_of(k: &DisplayKey, seq: u64, state: AnnounceState, s: &Say) -> Vec<(String, String)> {
        let mut fields: Vec<(&str, &str)> = vec![("srv", s.srv), ("mode", s.mode)];
        if let Some(c) = s.code {
            fields.push(("code", c));
        }
        if let Some(n) = s.name {
            fields.push(("name", n));
        }
        sign_record(k, seq, state, &fields)
    }

    fn rec_txt(inst: &str, addr: &str, txt: Vec<(String, String)>) -> RawRecord {
        RawRecord {
            instance: inst.into(),
            addresses: if addr.is_empty() {
                vec![]
            } else {
                vec![ip(addr)]
            },
            txt,
        }
    }

    fn rec(k: &DisplayKey, seq: u64, s: &Say) -> RawRecord {
        let inst = format!("CometCast-{}", &k.id()[..8]);
        rec_txt(&inst, "10.0.0.1", txt_of(k, seq, AnnounceState::Live, s))
    }

    fn gone(k: &DisplayKey, seq: u64) -> RawRecord {
        let inst = format!("CometCast-{}", &k.id()[..8]);
        rec_txt(
            &inst,
            "10.0.0.1",
            txt_of(k, seq, AnnounceState::Gone, &say("P4X9TR")),
        )
    }

    fn t0() -> Instant {
        Instant::now()
    }

    fn s(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    fn only_updated(ch: &[NearbyChange]) -> &NearbyScreen {
        match ch {
            [NearbyChange::Updated(s)] => s,
            other => panic!("expected one update, got {other:?}"),
        }
    }

    // ----- one record -----

    #[test]
    fn parses_a_good_record() {
        let k = key(1);
        let c = check_record(&rec(&k, 5, &say("P4X9TR"))).unwrap();
        assert_eq!(c.screen.id, k.id());
        assert_eq!(c.screen.tag, k.tag());
        assert_eq!(c.screen.name, "Room 204");
        assert_eq!(c.screen.server, "ws://10.0.0.5:8443");
        assert_eq!(c.screen.mode, ScreenMode::Open);
        assert_eq!(c.screen.code.as_deref(), Some("P4X9TR"));
        assert_eq!((c.seq, c.state), (5, AnnounceState::Live));
        assert!(!c.screen.same_name && !c.screen.new_device);
        // TLS flag and approval.
        let txt = sign_record(
            &k,
            6,
            AnnounceState::Live,
            &[
                ("srv", "relay.example.org:443"),
                ("tls", "1"),
                ("mode", "approval"),
                ("code", "P4X9TR"),
            ],
        );
        let c = check_record(&rec_txt("x", "", txt)).unwrap();
        assert_eq!(c.screen.server, "wss://relay.example.org:443");
        assert_eq!(c.screen.mode, ScreenMode::Approval);
        assert_eq!(c.screen.name, "");
    }

    #[test]
    fn the_row_carries_the_normalised_addresses_of_the_announcement() {
        let k = key(1);
        let mut r = rec(&k, 1, &say("P4X9TR"));
        r.addresses = vec![ip("192.168.1.20"), ip("::ffff:192.168.1.20"), ip("fd00::2")];
        let c = check_record(&r).unwrap();
        // An IPv4-mapped address is the IPv4 address; sorted, no duplicates.
        assert_eq!(c.screen.addresses, ["192.168.1.20", "fd00::2"]);
        r.addresses.clear();
        assert!(check_record(&r).unwrap().screen.addresses.is_empty());
    }

    #[test]
    fn a_v1_record_is_ignored_there_is_no_compatibility() {
        let v1 = vec![
            ("v".to_owned(), "1".to_owned()),
            ("id".into(), "0123456789abcdef".into()),
            ("srv".into(), "10.0.0.5:8443".into()),
            ("mode".into(), "open".into()),
            ("code".into(), "P4X9TR".into()),
        ];
        assert_eq!(
            check_record(&rec_txt("x", "", v1.clone())),
            Err(Reject::Version)
        );
        let mut t = NearbyTracker::new();
        assert!(t.on_seen(&rec_txt("x", "10.0.0.1", v1), t0()).is_empty());
        assert!(t.screens().is_empty());
    }

    #[test]
    fn locked_never_has_a_code() {
        let k = key(1);
        let mut say = say("P4X9TR");
        say.mode = "locked";
        // A code in a locked record is ignored, not shown.
        let c = check_record(&rec(&k, 1, &say)).unwrap();
        assert_eq!((c.screen.mode, c.screen.code), (ScreenMode::Locked, None));
        say.code = None;
        let c = check_record(&rec(&k, 2, &say)).unwrap();
        assert_eq!((c.screen.mode, c.screen.code), (ScreenMode::Locked, None));
    }

    #[test]
    fn bad_codes_drop_the_record_even_when_signed() {
        let k = key(1);
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
            let mut sy = say("P4X9TR");
            sy.code = Some(bad);
            assert!(check_record(&rec(&k, 1, &sy)).is_err(), "{bad:?}");
        }
        let mut sy = say("P4X9TR");
        sy.code = None;
        assert!(check_record(&rec(&k, 1, &sy)).is_err(), "open without code");
    }

    #[test]
    fn hostile_content_is_refused_even_when_signed() {
        let k = key(1);
        for bad_srv in [
            "127.0.0.1:8443",
            "8.8.8.8:443",
            "relay.example.org:443",
            "x.example",
            "a b:1",
            "169.254.169.254:80",
            "0x7f000001:1",
        ] {
            let mut sy = say("P4X9TR");
            sy.srv = bad_srv;
            assert!(check_record(&rec(&k, 1, &sy)).is_err(), "{bad_srv:?}");
        }
        for bad_mode in ["Open", "busy", "open ", "OPEN"] {
            let mut sy = say("P4X9TR");
            sy.mode = bad_mode;
            assert!(check_record(&rec(&k, 1, &sy)).is_err(), "{bad_mode:?}");
        }
    }

    #[test]
    fn names_are_cleaned_and_may_be_empty() {
        let k = key(1);
        let long = format!("\u{202e}Ra\tum\u{7}  204{}", "x".repeat(100));
        let txt = sign_record(
            &k,
            1,
            AnnounceState::Live,
            &[
                ("srv", "10.0.0.5:8443"),
                ("mode", "locked"),
                ("name", long.as_str()),
            ],
        );
        let c = check_record(&rec_txt("x", "", txt)).unwrap();
        assert!(c.screen.name.starts_with("Ra um 204x"));
        assert_eq!(c.screen.name.chars().count(), 32);
        assert!(!c.screen.name.contains('\u{202e}'));
        let mut sy = say("P4X9TR");
        sy.name = None;
        assert_eq!(
            check_record(&rec(&k, 2, &sy)).unwrap().screen.name,
            "",
            "no fallback to the host name"
        );
    }

    // ----- the forgery cases of the spec -----

    #[test]
    fn a_forged_goodbye_does_nothing() {
        let k = key(1);
        let mut t = NearbyTracker::new();
        let now = t0();
        t.on_seen(&rec(&k, 10, &say("P4X9TR")), now);
        let inst = format!("CometCast-{}", &k.id()[..8]);
        // The mDNS goodbye of anybody: no change, no removal, and the row still expires on time.
        t.on_gone(&inst);
        assert_eq!(t.screens().len(), 1);
        assert!(t.is_unconfirmed(&k.id()));
        assert_eq!(t.next_expiry(), Some(now + SCREEN_TTL));
        assert_eq!(t.screens()[0].code.as_deref(), Some("P4X9TR"));
        // A refresh clears the mark.
        t.on_seen(&rec(&k, 11, &say("P4X9TR")), now + s(10));
        assert!(!t.is_unconfirmed(&k.id()));
        // A goodbye for an unknown name does nothing either.
        t.on_gone("nobody");
        assert_eq!(t.screens().len(), 1);
    }

    #[test]
    fn a_forged_gone_record_does_not_remove_the_row() {
        let k = key(1);
        let evil = key(2);
        let mut t = NearbyTracker::new();
        let now = t0();
        t.on_seen(&rec(&k, 10, &say("P4X9TR")), now);
        // Unsigned, bad signature, wrong key with the victim's id, tampered state.
        let mut bad_sig = gone(&k, 11);
        bad_sig.txt.iter_mut().find(|(k, _)| k == "seq").unwrap().1 = "12".into();
        let mut stolen = gone(&evil, 11);
        stolen.txt.iter_mut().find(|(k, _)| k == "id").unwrap().1 = k.id();
        let mut tampered = rec(&k, 11, &say("P4X9TR"));
        tampered
            .txt
            .iter_mut()
            .find(|(k, _)| k == "state")
            .unwrap()
            .1 = "gone".into();
        for bad in [bad_sig, stolen, tampered] {
            assert!(t.on_seen(&bad, now).is_empty());
        }
        assert_eq!(t.screens().len(), 1);
        assert_eq!(t.highest_seq(&k.id()), Some(10));
    }

    #[test]
    fn a_record_with_the_victims_id_and_another_key_is_ignored() {
        let victim = key(1);
        let evil = key(2);
        let mut t = NearbyTracker::new();
        let now = t0();
        t.on_seen(&rec(&victim, 10, &say("P4X9TR")), now);
        // The attacker signs correctly with their own key but claims the victim's id.
        let mut forged = rec(&evil, 99, &say("K7Q2MW"));
        forged.txt.iter_mut().find(|(k, _)| k == "id").unwrap().1 = victim.id();
        assert!(t.on_seen(&forged, now).is_empty());
        // Or with the victim's id and the victim's pk but the attacker's content: bad signature.
        let mut forged2 = rec(&victim, 99, &say("P4X9TR"));
        forged2.txt.iter_mut().find(|(k, _)| k == "code").unwrap().1 = "K7Q2MW".into();
        assert!(t.on_seen(&forged2, now).is_empty());
        assert_eq!(t.screens()[0].code.as_deref(), Some("P4X9TR"));
        assert_eq!(t.screens()[0].id, victim.id());
        // The attacker's own id is a separate row (they own it), never the victim's.
        let mut own_say = say("K7Q2MW");
        own_say.name = Some("Evil room");
        let own = rec(&evil, 1, &own_say);
        assert!(matches!(&t.on_seen(&own, now)[..], [NearbyChange::Added(s)] if s.id == evil.id()));
        assert_eq!(t.screens()[0].code.as_deref(), Some("P4X9TR"));
    }

    #[test]
    fn a_replay_of_an_older_record_is_ignored() {
        let k = key(1);
        let mut t = NearbyTracker::new();
        let now = t0();
        let old = rec(&k, 10, &say("P4X9TR"));
        t.on_seen(&old, now);
        t.on_seen(&rec(&k, 20, &say("K7Q2MW")), now + s(1));
        // The old live record again (lower seq) and the same one (equal seq).
        assert!(t.on_seen(&old, now + s(2)).is_empty());
        assert!(
            t.on_seen(&rec(&k, 20, &say("K7Q2MW")), now + s(3))
                .is_empty()
        );
        assert_eq!(t.screens()[0].code.as_deref(), Some("K7Q2MW"));
        // An old gone record (captured earlier) does not remove the row either.
        let old_gone = gone(&k, 15);
        assert!(t.on_seen(&old_gone, now + s(4)).is_empty());
        assert_eq!(t.screens().len(), 1);
        // The real goodbye removes it; replaying the old live record afterwards does not bring
        // the row back.
        assert_eq!(
            t.on_seen(&gone(&k, 30), now + s(5)),
            [NearbyChange::Removed(k.id())]
        );
        assert!(t.on_seen(&old, now + s(6)).is_empty());
        assert!(
            t.on_seen(&rec(&k, 25, &say("K7Q2MW")), now + s(7))
                .is_empty()
        );
        assert!(t.screens().is_empty());
        // Replaying the gone record again is harmless.
        assert!(t.on_seen(&gone(&k, 30), now + s(8)).is_empty());
        // A newer live record brings the row back (the display restarted).
        assert!(matches!(
            &t.on_seen(&rec(&k, 31, &say("AAAAAA")), now + s(9))[..],
            [NearbyChange::Added(_)]
        ));
    }

    #[test]
    fn a_repeat_of_the_same_record_does_not_refresh_the_row() {
        let k = key(1);
        let mut t = NearbyTracker::new();
        let now = t0();
        let r = rec(&k, 10, &say("P4X9TR"));
        t.on_seen(&r, now);
        // A backend cache that keeps reporting the same record is not a sign of life.
        for i in 1..30 {
            assert!(t.on_seen(&r, now + s(i)).is_empty());
        }
        assert_eq!(t.expire(now + SCREEN_TTL), [NearbyChange::Removed(k.id())]);
    }

    // ----- newer signed records apply at once -----

    #[test]
    fn a_newer_signed_record_applies_at_once() {
        let k = key(1);
        let mut t = NearbyTracker::new();
        let now = t0();
        t.on_seen(&rec(&k, 10, &say("P4X9TR")), now);
        // New code, one second later (no 15 s hold any more).
        let ch = t.on_seen(&rec(&k, 11, &say("K7Q2MW")), now + s(1));
        assert_eq!(only_updated(&ch).code.as_deref(), Some("K7Q2MW"));
        // New server: a signed newer record may change it (the first record does not fix it).
        let mut moved = say("K7Q2MW");
        moved.srv = "10.0.0.6:8443";
        let ch = t.on_seen(&rec(&k, 12, &moved), now + s(2));
        assert_eq!(only_updated(&ch).server, "ws://10.0.0.6:8443");
        // Approval, then locked (no code).
        let mut ap = moved.clone();
        ap.mode = "approval";
        assert_eq!(
            only_updated(&t.on_seen(&rec(&k, 13, &ap), now + s(3))).mode,
            ScreenMode::Approval
        );
        let mut locked = moved.clone();
        locked.mode = "locked";
        locked.code = None;
        let ch = t.on_seen(&rec(&k, 14, &locked), now + s(4));
        let row = only_updated(&ch);
        assert_eq!((row.mode, row.code.clone()), (ScreenMode::Locked, None));
        // Unlocking again is a newer record like any other.
        let ch = t.on_seen(&rec(&k, 15, &say("AAAAAA")), now + s(5));
        assert_eq!(only_updated(&ch).code.as_deref(), Some("AAAAAA"));
        // Renamed.
        let mut renamed = say("AAAAAA");
        renamed.name = Some("Room 205");
        assert_eq!(
            only_updated(&t.on_seen(&rec(&k, 16, &renamed), now + s(6))).name,
            "Room 205"
        );
        // Signed gone: the row leaves at once.
        assert_eq!(
            t.on_seen(&gone(&k, 17), now + s(7)),
            [NearbyChange::Removed(k.id())]
        );
        assert!(t.screens().is_empty());
        // The place of a changed row stays.
        let a = key(3);
        let b = key(4);
        t.on_seen(&rec(&a, 1, &say("P4X9TR")), now);
        t.on_seen(&rec(&b, 1, &say("K7Q2MW")), now);
        let mut other = say("P4X9TR");
        other.name = Some("Other");
        t.on_seen(&rec(&a, 2, &other), now + s(1));
        let ids: Vec<_> = t.screens().into_iter().map(|s| s.id).collect();
        assert_eq!(ids, [a.id(), b.id()]);
    }

    #[test]
    fn rows_are_keyed_by_the_verified_id_not_by_the_instance_name() {
        let k = key(1);
        let evil = key(2);
        let mut t = NearbyTracker::new();
        let now = t0();
        let inst = format!("CometCast-{}", &k.id()[..8]);
        t.on_seen(&rec(&k, 10, &say("P4X9TR")), now);
        // An attacker announces its own id under the instance name of the row: its own row.
        let mut evil_say = say("K7Q2MW");
        evil_say.name = Some("Evil room");
        let evil_rec = rec_txt(
            &inst,
            "10.0.0.1",
            txt_of(&evil, 1, AnnounceState::Live, &evil_say),
        );
        let ch = t.on_seen(&evil_rec, now);
        assert!(matches!(&ch[..], [NearbyChange::Added(s)] if s.id == evil.id()));
        assert_eq!(t.screens()[0].code.as_deref(), Some("P4X9TR"));
        // The same screen under a renamed instance ("... (2)") is the same row.
        let renamed = rec_txt(
            &format!("{inst} (2)"),
            "10.0.0.1",
            txt_of(&k, 11, AnnounceState::Live, &say("P4X9TR")),
        );
        t.on_seen(&renamed, now + s(1));
        assert_eq!(t.screens().len(), 2);
        // A goodbye of the evil instance name marks, but removes nothing of either.
        t.on_gone(&inst);
        assert_eq!(t.screens().len(), 2);
    }

    #[test]
    fn same_screen_on_two_interfaces_is_one_row_with_both_addresses() {
        let k = key(1);
        let mut t = NearbyTracker::new();
        let now = t0();
        let txt1 = txt_of(&k, 10, AnnounceState::Live, &say("P4X9TR"));
        t.on_seen(&rec_txt("a", "192.168.1.20", txt1), now);
        let txt2 = txt_of(&k, 11, AnnounceState::Live, &say("P4X9TR"));
        let r2 = RawRecord {
            instance: "a (2)".into(),
            addresses: vec![ip("192.168.1.20"), ip("192.168.1.21")],
            txt: txt2,
        };
        let ch = t.on_seen(&r2, now + s(1));
        assert_eq!(
            only_updated(&ch).addresses,
            ["192.168.1.20", "192.168.1.21"]
        );
        assert_eq!(t.screens().len(), 1);
        // A goodbye of one instance changes nothing visible, not even the addresses.
        t.on_gone("a (2)");
        assert_eq!(t.screens()[0].addresses, ["192.168.1.20", "192.168.1.21"]);
    }

    // ----- expiry and limits -----

    #[test]
    fn rows_expire_after_30_seconds_without_an_accepted_record() {
        let a = key(1);
        let b = key(2);
        let mut t = NearbyTracker::new();
        let now = t0();
        let mut unnamed = say("P4X9TR");
        unnamed.name = None;
        t.on_seen(&rec(&a, 1, &unnamed), now);
        t.on_seen(&rec(&b, 1, &unnamed), now + s(20));
        assert_eq!(t.next_expiry(), Some(now + SCREEN_TTL));
        assert!(t.expire(now + s(29)).is_empty());
        assert_eq!(t.expire(now + s(30)), [NearbyChange::Removed(a.id())]);
        // A refresh with a new seq keeps a row alive.
        t.on_seen(&rec(&b, 2, &unnamed), now + s(45));
        assert!(t.expire(now + s(60)).is_empty());
        assert_eq!(t.expire(now + s(75)).len(), 1);
    }

    #[test]
    fn a_forged_record_does_not_refresh_a_row() {
        let k = key(1);
        let evil = key(2);
        let mut t = NearbyTracker::new();
        let now = t0();
        t.on_seen(&rec(&k, 10, &say("P4X9TR")), now);
        let mut forged = rec(&evil, 50, &say("P4X9TR"));
        forged.txt.iter_mut().find(|(k, _)| k == "id").unwrap().1 = k.id();
        for i in 1..30 {
            t.on_seen(&forged, now + s(i));
        }
        assert_eq!(t.expire(now + SCREEN_TTL), [NearbyChange::Removed(k.id())]);
    }

    #[test]
    fn the_33rd_screen_does_not_push_a_row_out() {
        let mut t = NearbyTracker::new();
        let now = t0();
        let keys: Vec<DisplayKey> = (0..40u8).map(|i| key(i + 1)).collect();
        let mut unnamed = say("P4X9TR");
        unnamed.name = None;
        for k in &keys[..32] {
            assert_eq!(t.on_seen(&rec(k, 1, &unnamed), now).len(), 1);
        }
        // The 33rd and later are not reported and not stored.
        for k in &keys[32..] {
            assert!(t.on_seen(&rec(k, 1, &unnamed), now).is_empty());
            assert_eq!(t.highest_seq(&k.id()), None);
        }
        let rows = t.screens();
        assert_eq!(rows.len(), MAX_SCREENS);
        for (n, row) in rows.iter().enumerate() {
            assert_eq!(row.id, keys[n].id(), "row {n} changed");
        }
        // Row 5 leaves (signed): the 33rd is reported at the end with its next record.
        assert_eq!(t.on_seen(&gone(&keys[5], 2), now).len(), 1);
        let ch = t.on_seen(&rec(&keys[32], 2, &unnamed), now);
        assert!(matches!(&ch[..], [NearbyChange::Added(a)] if a.id == keys[32].id()));
        let rows = t.screens();
        assert_eq!(rows.len(), MAX_SCREENS);
        assert_eq!(rows[31].id, keys[32].id());
        assert_eq!(rows[5].id, keys[6].id());
    }

    #[test]
    fn clear_removes_everything_and_forgets_the_seqs() {
        let a = key(1);
        let b = key(2);
        let mut t = NearbyTracker::new();
        let now = t0();
        t.on_seen(&rec(&a, 5, &say("P4X9TR")), now);
        t.on_seen(&rec(&b, 5, &say("K7Q2MW")), now);
        assert_eq!(t.clear().len(), 2);
        assert!(t.screens().is_empty() && t.next_expiry().is_none());
        assert_eq!(t.highest_seq(&a.id()), None);
        // The same record is accepted again after a restart of the search.
        assert_eq!(t.on_seen(&rec(&a, 5, &say("P4X9TR")), now).len(), 1);
    }

    // ----- the seq memory -----

    #[test]
    fn seq_is_kept_for_ten_minutes_after_the_row_is_gone() {
        let k = key(1);
        let mut t = NearbyTracker::new();
        let now = t0();
        t.on_seen(&rec(&k, 10, &say("P4X9TR")), now);
        // Kept while the row exists, however long.
        assert_eq!(t.highest_seq(&k.id()), Some(10));
        // Expired: still remembered for 10 minutes ...
        t.expire(now + SCREEN_TTL);
        assert!(t.screens().is_empty());
        let later = now + SCREEN_TTL + SEQ_MEMORY_AFTER_GONE - s(1);
        assert!(t.on_seen(&rec(&k, 10, &say("P4X9TR")), later).is_empty());
        assert_eq!(t.highest_seq(&k.id()), Some(10));
        // ... and dropped after.
        let after = now + SCREEN_TTL + SEQ_MEMORY_AFTER_GONE + s(1);
        t.expire(after);
        assert_eq!(t.highest_seq(&k.id()), None);
        assert_eq!(t.on_seen(&rec(&k, 10, &say("P4X9TR")), after).len(), 1);
        // A signed goodbye starts the same 10 minutes.
        let at = after + s(1);
        t.on_seen(&gone(&k, 11), at);
        assert_eq!(t.highest_seq(&k.id()), Some(11));
        t.expire(at + SEQ_MEMORY_AFTER_GONE + s(1));
        assert_eq!(t.highest_seq(&k.id()), None);
    }

    #[test]
    fn at_most_256_ids_are_remembered_oldest_first() {
        let mut t = NearbyTracker::new();
        let now = t0();
        // 300 screens that said goodbye: only the last 256 are remembered.
        let keys: Vec<DisplayKey> = (0..300u32)
            .map(|i| {
                let mut seed = [0u8; 32];
                seed[..4].copy_from_slice(&(i + 1).to_le_bytes());
                DisplayKey::from_secret(&seed).unwrap()
            })
            .collect();
        for k in &keys {
            t.on_seen(&rec(k, 1, &say("P4X9TR")), now);
            t.on_seen(&gone(k, 2), now);
        }
        assert_eq!(t.seqs.len(), MAX_SEQ_MEMORY);
        assert_eq!(t.highest_seq(&keys[0].id()), None);
        assert_eq!(t.highest_seq(&keys[299].id()), Some(2));
        assert_eq!(t.highest_seq(&keys[43].id()), None);
        assert_eq!(t.highest_seq(&keys[44].id()), Some(2));
    }

    // ----- same name and remembered screens -----

    #[test]
    fn the_same_name_marks_both_rows() {
        let a = key(1);
        let b = key(2);
        let mut t = NearbyTracker::new();
        let now = t0();
        let ch = t.on_seen(&rec(&a, 1, &say("P4X9TR")), now);
        assert!(matches!(&ch[..], [NearbyChange::Added(s)] if !s.same_name));
        // The second row appears with the mark, and the first row is reported as updated.
        let mut other = say("K7Q2MW");
        other.name = Some("room 204");
        let ch = t.on_seen(&rec(&b, 1, &other), now);
        assert_eq!(ch.len(), 2, "{ch:?}");
        assert!(matches!(&ch[0], NearbyChange::Updated(s) if s.id == a.id() && s.same_name));
        assert!(matches!(&ch[1], NearbyChange::Added(s) if s.id == b.id() && s.same_name));
        assert!(t.screens().iter().all(|s| s.same_name));
        // A different name does not mark.
        let c = key(3);
        let mut third = say("AAAAAA");
        third.name = Some("Elbe");
        t.on_seen(&rec(&c, 1, &third), now);
        assert!(!t.screens()[2].same_name);
        // One of the two renames: both marks go.
        let mut renamed = say("K7Q2MW");
        renamed.name = Some("Room 205");
        let ch = t.on_seen(&rec(&b, 2, &renamed), now + s(1));
        assert_eq!(ch.len(), 2);
        assert!(t.screens().iter().all(|s| !s.same_name));
        // Rows without a name never match each other.
        let d = key(4);
        let e = key(5);
        let mut none = say("AAAAAA");
        none.name = None;
        t.on_seen(&rec(&d, 1, &none), now);
        t.on_seen(&rec(&e, 1, &none), now);
        assert!(t.screens().iter().all(|s| !s.same_name));
        // The mark goes when the other row leaves.
        let mut same = say("P4X9TR");
        same.name = Some("Elbe");
        t.on_seen(&rec(&a, 2, &same), now + s(2));
        assert!(t.screens()[0].same_name && t.screens()[2].same_name);
        let ch = t.on_seen(&gone(&a, 3), now + s(3));
        assert!(ch.contains(&NearbyChange::Removed(a.id())));
        assert!(t.screens().iter().all(|s| !s.same_name));
    }

    #[test]
    fn a_remembered_name_with_another_id_is_marked_as_a_new_device() {
        let known = key(1);
        let newcomer = key(2);
        let mut t = NearbyTracker::new();
        let now = t0();
        t.on_seen(&rec(&known, 1, &say("P4X9TR")), now);
        // Nothing is remembered: no mark.
        assert!(!t.screens()[0].new_device);
        // The app remembers (Room 204, known): the known row stays unmarked.
        let pairs = vec![("Room 204".to_owned(), known.id())];
        assert!(t.set_remembered(&pairs).is_empty());
        let mut other = say("K7Q2MW");
        other.name = Some("ROOM 204");
        let ch = t.on_seen(&rec(&newcomer, 1, &other), now);
        // Also a same-name row: both marks.
        assert!(matches!(
            ch.last(),
            Some(NearbyChange::Added(s)) if s.new_device && s.same_name
        ));
        assert!(!t.screens()[0].new_device);
        assert!(t.screens()[0].same_name);
        // The row stays tappable: it is a normal row with the code.
        assert_eq!(t.screens()[1].code.as_deref(), Some("K7Q2MW"));
        // The known one leaves: only the new-device mark is left.
        t.on_seen(&gone(&known, 2), now + s(1));
        assert!(t.screens()[0].new_device && !t.screens()[0].same_name);
        // Joining the new one adds a pair: the mark goes.
        let pairs = vec![
            ("Room 204".to_owned(), known.id()),
            ("Room 204".to_owned(), newcomer.id()),
        ];
        let ch = t.set_remembered(&pairs);
        assert!(!only_updated(&ch).new_device);
        // Forgetting everything clears marks.
        let mut t2 = NearbyTracker::new();
        t2.on_seen(&rec(&newcomer, 1, &other), now);
        let ch = t2.set_remembered(&[("room 204".into(), known.id())]);
        assert!(only_updated(&ch).new_device);
        let ch = t2.set_remembered(&[]);
        assert!(!only_updated(&ch).new_device);
    }

    #[test]
    fn remembered_pairs_are_bounded_and_checked() {
        let k = key(1);
        let mut t = NearbyTracker::new();
        let now = t0();
        t.on_seen(&rec(&k, 1, &say("P4X9TR")), now);
        let other = key(2).id();
        // Pairs with an empty name or a bad id are ignored.
        assert!(
            t.set_remembered(&[
                ("".into(), other.clone()),
                ("Room 204".into(), "nope".into())
            ])
            .is_empty()
        );
        // Only the first 50 count.
        let mut many: Vec<(String, String)> = (0..MAX_REMEMBERED)
            .map(|i| (format!("Other {i}"), other.clone()))
            .collect();
        many.push(("Room 204".into(), other.clone()));
        assert!(t.set_remembered(&many).is_empty());
        assert!(!t.screens()[0].new_device);
        // Within the first 50 it counts (the name is the same, the id differs).
        many.insert(0, ("room 204".into(), other));
        let ch = t.set_remembered(&many);
        assert!(only_updated(&ch).new_device);
    }

    // ----- the driver -----

    #[tokio::test(start_paused = true)]
    async fn the_driver_expires_rows_by_itself_and_ignores_forged_goodbyes() {
        let k = key(1);
        let mut search = NearbySearch::new();
        let sink = search.sink();
        sink.seen(rec(&k, 1, &say("P4X9TR")));
        assert!(matches!(search.next().await, Some(NearbyChange::Added(_))));
        // A forged goodbye changes nothing; no refresh: the driver removes the row at 30 s.
        let inst = format!("CometCast-{}", &k.id()[..8]);
        sink.gone(inst.clone());
        let start = Instant::now();
        assert_eq!(search.next().await, Some(NearbyChange::Removed(k.id())));
        assert!(start.elapsed() >= SCREEN_TTL);
        // The signed goodbye removes a row at once; reset clears.
        sink.seen(rec(&k, 2, &say("P4X9TR")));
        assert!(matches!(search.next().await, Some(NearbyChange::Added(_))));
        sink.seen(gone(&k, 3));
        assert_eq!(search.next().await, Some(NearbyChange::Removed(k.id())));
        sink.seen(rec(&k, 4, &say("P4X9TR")));
        assert!(matches!(search.next().await, Some(NearbyChange::Added(_))));
        sink.reset();
        assert_eq!(search.next().await, Some(NearbyChange::Removed(k.id())));
        // Remembered pairs through the sink.
        let mut other = say("K7Q2MW");
        other.name = Some("Elbe");
        sink.seen(rec(&k, 5, &other));
        assert!(matches!(search.next().await, Some(NearbyChange::Added(_))));
        sink.remembered(vec![("Elbe".into(), key(9).id())]);
        assert!(matches!(search.next().await, Some(NearbyChange::Updated(s)) if s.new_device));
    }
}
