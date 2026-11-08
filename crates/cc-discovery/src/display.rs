// SPDX-License-Identifier: GPL-3.0-or-later
//! The announcement of a display device (spec 14.8 of `docs/design/first-run-and-display.md`,
//! TXT layout `v=2` in `docs/discovery.md`, section 3, `docs/design/signed-announcements.md`).
//!
//! A display announces `_cometcast._tcp` only while it shows Idle or Presenting with a valid
//! code. The caller ([`DisplayAnnouncement`]) says what is true now ([`DisplayState`], or
//! `None` for "must not announce": setup, offline, policy forbids, background); this module
//! keeps the record equal to it:
//!
//! * every record is **signed** with the display's Ed25519 key ([`DisplayKey`], held by the
//!   app) and carries its public key, a strictly increasing `seq` and `state=live`; the id is
//!   derived from the key ([`crate::signed`]);
//! * a change is published at once (the limit of the spec is 1 s);
//! * the record is published again every [`REFRESH_EVERY`] (with a new `seq`) so that, with a
//!   record lifetime of [`RECORD_TTL`], a display that dies without a goodbye leaves the lists
//!   within 30 s;
//! * `None` (and dropping the announcement) first publishes a signed `state=gone` record with
//!   a higher `seq` and then withdraws the record (goodbye). Presenters remove a row only on the
//!   signed record, never on the goodbye alone;
//! * the record is small ([`MAX_TXT_BYTES`]) and holds nothing but the fields of the layout:
//!   no presenter data, no busy flag;
//! * **Locked** never carries a code (the code of a state in mode `locked` is not written
//!   even when the caller passes one).
//!
//! The wire part is a trait ([`AnnounceBackend`]) so the logic is tested with a fake; the
//! `mdns-sd` backend is [`MdnsBackend`].
//!
//! UNCERTAIN: whether a re-publication is visible to every browser as a refresh is not
//! verified on real devices (`docs/measurement-points.md`, A8).

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use mdns_sd::{ServiceDaemon, ServiceInfo};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::address::{Trust, clean_room_name, network_server, normalize_server, validate_hostname};
use crate::announce::{MDNS_SERVICE_TYPE, MdnsError};
use crate::nearby::{ScreenMode, is_valid_code};
use crate::signed::{AnnounceState, DisplayKey, SeqCounter, sign_record, unix_ms};

/// The lifetime a record is meant to have (spec 14.8, point 5).
pub const RECORD_TTL: Duration = Duration::from_secs(30);
/// The record is published again this often (well before [`RECORD_TTL`] runs out).
pub const REFRESH_EVERY: Duration = Duration::from_secs(10);
/// The most bytes of one TXT record (DNS-SD lengths included). A normal v2 record is about
/// 260 bytes; a server name so long that the record would not fit is not announced.
pub const MAX_TXT_BYTES: usize = 512;
/// The port the service names. Displays do not listen for senders; DNS-SD only needs one.
pub const SERVICE_PORT: u16 = 9;
/// How long the `mdns-sd` backend waits between the signed `gone` record and the goodbye.
const GONE_SETTLE: Duration = Duration::from_millis(400);
/// Added to the last `seq` when the app is told what to persist
/// ([`DisplayAnnouncement::seq_to_persist`]), so that a persist that lags behind the last
/// publish still covers every value used.
pub const SEQ_RESERVE: u64 = 120_000;

/// What a display announces right now. The id comes from the key of the announcer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayState {
    /// The room name as set in setup. Cleaned and cut here; may be empty (no fallback).
    pub room_name: String,
    /// The server the display uses: `host:port`, or with `wss://` / `ws://`. Checked like any
    /// announced server; a `wss://` or `https://` scheme becomes `tls=1`.
    pub server: String,
    /// Who may join.
    pub mode: ScreenMode,
    /// The code the display shows. Written only for `open` and `approval`.
    pub code: Option<String>,
}

/// Why a state cannot be announced.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AnnounceError {
    /// The server fails the checks of announced servers.
    #[error("announcement: server cannot be announced")]
    BadServer,
    /// `open` or `approval` without a valid code: nothing to show to anybody.
    #[error("announcement: no valid code")]
    NoCode,
    /// The record would be larger than [`MAX_TXT_BYTES`].
    #[error("announcement: record too large")]
    TooLarge,
}

/// The DNS-SD instance name of a display: derived from the id, never from a host or room name.
pub fn instance_name(id: &str) -> String {
    format!("CometCast-{}", &id[..id.len().min(8)])
}

/// The checked, unsigned content of a record (everything the display decides).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Content {
    srv: String,
    tls: bool,
    mode: ScreenMode,
    code: Option<String>,
    name: String,
}

fn content_of(state: &DisplayState) -> Result<Content, AnnounceError> {
    let full =
        normalize_server(&state.server, Trust::Admin).map_err(|_| AnnounceError::BadServer)?;
    let (tls, authority) = match full.split_once("://") {
        Some(("wss" | "https", a)) => (true, a),
        Some((_, a)) => (false, a),
        None => (false, full.as_str()),
    };
    let (checked, _) =
        network_server(authority, Some(tls)).map_err(|_| AnnounceError::BadServer)?;
    let srv = checked
        .split_once("://")
        .map_or(checked.as_str(), |(_, a)| a)
        .to_owned();
    let code = match state.mode {
        // Never a code (and never a token) while locked.
        ScreenMode::Locked => None,
        ScreenMode::Open | ScreenMode::Approval => {
            let code = state.code.as_deref().ok_or(AnnounceError::NoCode)?;
            if !is_valid_code(code) {
                return Err(AnnounceError::NoCode);
            }
            Some(code.to_owned())
        }
    };
    Ok(Content {
        srv,
        tls,
        mode: state.mode,
        code,
        name: clean_room_name(&state.room_name),
    })
}

fn sign_content(
    c: &Content,
    key: &DisplayKey,
    seq: u64,
    state: AnnounceState,
) -> Result<Vec<(String, String)>, AnnounceError> {
    let mut fields: Vec<(&str, &str)> = vec![("srv", c.srv.as_str())];
    if c.tls {
        fields.push(("tls", "1"));
    }
    fields.push(("mode", c.mode.as_str()));
    if let Some(code) = &c.code {
        fields.push(("code", code.as_str()));
    }
    if !c.name.is_empty() {
        fields.push(("name", c.name.as_str()));
    }
    let txt = sign_record(key, seq, state, &fields);
    let size: usize = txt.iter().map(|(k, v)| 1 + k.len() + 1 + v.len()).sum();
    if size > MAX_TXT_BYTES {
        return Err(AnnounceError::TooLarge);
    }
    Ok(txt)
}

/// Build the signed TXT pairs of `state` for `key` with the given `seq` and announcement state.
/// The keys come in the order `v, id, pk, seq, state, srv, tls, mode, code, name, sig`; absent
/// optional keys (`tls`, `code`, `name`) are left out.
pub fn txt_records(
    state: &DisplayState,
    key: &DisplayKey,
    seq: u64,
    announce: AnnounceState,
) -> Result<Vec<(String, String)>, AnnounceError> {
    sign_content(&content_of(state)?, key, seq, announce)
}

/// The wire side of an announcement. A trait so tests need no network.
pub trait AnnounceBackend: Send {
    /// Publish the record, or replace the record published before (and announce it).
    fn publish(&mut self, instance: &str, txt: &[(String, String)]) -> Result<(), MdnsError>;
    /// Withdraw the record (goodbye). Does nothing when none is published.
    fn withdraw(&mut self);
    /// Called after the signed `gone` record was published and before [`AnnounceBackend::withdraw`]:
    /// a wire backend gives the record a moment to reach the listeners, because a cache that
    /// sees the update and the goodbye at once may report only the removal. Default: no wait.
    fn settle_before_goodbye(&mut self) {}
}

/// The `mdns-sd` backend (desktops; also used on Android tablets and TVs only if the platform
/// bridge is not used).
#[derive(Default)]
pub struct MdnsBackend {
    daemon: Option<ServiceDaemon>,
    fullname: Option<String>,
    host: Option<String>,
}

impl MdnsBackend {
    /// A backend that has published nothing yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// A backend that uses `label` for its `.local.` host name (checked when publishing).
    pub fn with_host_label(label: String) -> Self {
        let mut b = Self::default();
        b.host = Some(label);
        b
    }
}

impl AnnounceBackend for MdnsBackend {
    fn publish(&mut self, instance: &str, txt: &[(String, String)]) -> Result<(), MdnsError> {
        let label = match &self.host {
            Some(h) => validate_hostname(h).map_err(|e| MdnsError(e.to_string()))?,
            None => {
                use rand::Rng;
                let l = format!("cometcast-{:08x}", rand::rng().random::<u32>());
                validate_hostname(&l).map_err(|e| MdnsError(e.to_string()))?;
                self.host = Some(l.clone());
                l
            }
        };
        let host = format!("{label}.local.");
        let info = ServiceInfo::new(MDNS_SERVICE_TYPE, instance, &host, "", SERVICE_PORT, txt)
            .map_err(|e| MdnsError(e.to_string()))?
            .enable_addr_auto();
        let mut info = info;
        // Decision N50: records live 30 s, so a platform cache cannot keep a dead display in a
        // list for longer than the 35 s of spec 14.8, point 5.
        info.set_host_ttl(crate::announce::RECORD_TTL_SECS);
        info.set_other_ttl(crate::announce::RECORD_TTL_SECS);
        let fullname = info.get_fullname().to_owned();
        if self.daemon.is_none() {
            self.daemon = Some(ServiceDaemon::new().map_err(|e| MdnsError(e.to_string()))?);
        }
        let daemon = self
            .daemon
            .as_ref()
            .ok_or_else(|| MdnsError("no daemon".into()))?;
        // Registering the same full name again replaces the record and announces it.
        daemon
            .register(info)
            .map_err(|e| MdnsError(e.to_string()))?;
        self.fullname = Some(fullname);
        Ok(())
    }

    fn settle_before_goodbye(&mut self) {
        std::thread::sleep(GONE_SETTLE);
    }

    fn withdraw(&mut self) {
        if let (Some(d), Some(name)) = (&self.daemon, self.fullname.take()) {
            // Wait a moment for the goodbye to be sent before the daemon goes away.
            if let Ok(rx) = d.unregister(&name) {
                let _ = rx.recv_timeout(Duration::from_millis(300));
            }
        }
        if let Some(d) = self.daemon.take() {
            let _ = d.shutdown();
        }
    }
}

impl Drop for MdnsBackend {
    fn drop(&mut self) {
        self.withdraw();
    }
}

/// Keeps the published record equal to the wanted state. Pure apart from the backend and the
/// clock; time is passed in.
pub struct DisplayAnnouncer<B: AnnounceBackend> {
    backend: B,
    key: DisplayKey,
    instance: String,
    seq: SeqCounter,
    /// Last `seq` used, for the app to persist ([`DisplayAnnouncement::seq_to_persist`]).
    seq_cell: Arc<AtomicU64>,
    clock: Box<dyn Fn() -> u64 + Send>,
    wanted: Option<Content>,
    /// What the published record says (the `gone` record repeats it).
    published_content: Option<Content>,
    published: bool,
    last_publish: Option<Instant>,
}

impl<B: AnnounceBackend> DisplayAnnouncer<B> {
    /// An announcer that announces nothing yet. `seq_floor` is the highest `seq` that may
    /// have been used before with this key (persisted by the app; 0 for a new key).
    pub fn new(backend: B, key: DisplayKey, seq_floor: u64) -> Self {
        Self::with_clock(backend, key, seq_floor, Box::new(unix_ms))
    }

    /// Like [`DisplayAnnouncer::new`] with a clock in Unix milliseconds (tests).
    pub fn with_clock(
        backend: B,
        key: DisplayKey,
        seq_floor: u64,
        clock: Box<dyn Fn() -> u64 + Send>,
    ) -> Self {
        let instance = instance_name(&key.id());
        Self {
            backend,
            key,
            instance,
            seq: SeqCounter::new(seq_floor),
            seq_cell: Arc::new(AtomicU64::new(seq_floor)),
            clock,
            wanted: None,
            published_content: None,
            published: false,
            last_publish: None,
        }
    }

    /// The backend (tests look at what was published).
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// The id of the display (from its key).
    pub fn id(&self) -> String {
        self.key.id()
    }

    /// The last `seq` used (or the floor).
    pub fn last_seq(&self) -> u64 {
        self.seq.last()
    }

    /// A handle that always holds the last `seq` used.
    pub fn seq_cell(&self) -> Arc<AtomicU64> {
        self.seq_cell.clone()
    }

    /// Is a record published now?
    pub fn is_published(&self) -> bool {
        self.published
    }

    /// Say what is true now. `None` withdraws the record. A state that cannot be announced
    /// (bad server, no code, too large) also withdraws it and returns the reason: a stale
    /// record must not stay when the new state is not announceable.
    pub fn set(&mut self, state: Option<&DisplayState>, now: Instant) -> Result<(), AnnounceError> {
        let Some(state) = state else {
            self.wanted = None;
            self.withdraw();
            return Ok(());
        };
        // Validate (and check the size) without using a `seq`.
        let content = match content_of(state)
            .and_then(|c| sign_content(&c, &self.key, 0, AnnounceState::Live).map(|_| c))
        {
            Ok(c) => c,
            Err(e) => {
                self.wanted = None;
                self.withdraw();
                return Err(e);
            }
        };
        let unchanged = self.published && self.wanted.as_ref() == Some(&content);
        self.wanted = Some(content);
        if !unchanged {
            self.publish(now);
        }
        Ok(())
    }

    /// When the record has to be published again, if one is published (or wanted but failed).
    pub fn next_refresh(&self) -> Option<Instant> {
        self.wanted.as_ref()?;
        Some(
            self.last_publish
                .map_or_else(Instant::now, |t| t + REFRESH_EVERY),
        )
    }

    /// Publish the record again if it is due (or if the last attempt failed).
    pub fn refresh(&mut self, now: Instant) {
        if self.wanted.is_some() && self.next_refresh().is_some_and(|t| t <= now) {
            self.publish(now);
        }
    }

    fn next_seq(&mut self) -> u64 {
        let n = self.seq.next((self.clock)());
        self.seq_cell.store(n, Ordering::SeqCst);
        n
    }

    fn publish(&mut self, now: Instant) {
        let Some(content) = self.wanted.clone() else {
            return;
        };
        let seq = self.next_seq();
        // A failed publish is tried again at the next refresh; nothing is shown to anybody.
        self.published = match sign_content(&content, &self.key, seq, AnnounceState::Live) {
            Ok(txt) => self.backend.publish(&self.instance, &txt).is_ok(),
            Err(_) => false,
        };
        if self.published {
            self.published_content = Some(content);
        }
        self.last_publish = Some(now);
    }

    fn withdraw(&mut self) {
        if self.published {
            // The signed `gone` record first (a higher `seq`): presenters remove the row on
            // it, not on the goodbye. A failure here changes nothing: the row then expires.
            if let Some(content) = self.published_content.clone() {
                let seq = self.next_seq();
                if let Ok(txt) = sign_content(&content, &self.key, seq, AnnounceState::Gone)
                    && self.backend.publish(&self.instance, &txt).is_ok()
                {
                    self.backend.settle_before_goodbye();
                }
            }
            self.backend.withdraw();
        }
        self.published = false;
        self.published_content = None;
        self.last_publish = None;
    }
}

impl<B: AnnounceBackend> Drop for DisplayAnnouncer<B> {
    fn drop(&mut self) {
        self.withdraw();
    }
}

/// A running announcement of a display. Dropping it (or calling [`DisplayAnnouncement::stop`])
/// says goodbye (a signed `gone` record, then the withdrawal).
pub struct DisplayAnnouncement {
    tx: watch::Sender<Option<DisplayState>>,
    task: JoinHandle<()>,
    seq_cell: Arc<AtomicU64>,
    id: String,
}

impl DisplayAnnouncement {
    /// Start the driver with `backend` and the key of the display. `seq_floor` is the
    /// persisted `seq` (see [`DisplayAnnouncement::seq_to_persist`]). Must be called in a Tokio
    /// runtime.
    pub fn start<B: AnnounceBackend + 'static>(
        backend: B,
        key: DisplayKey,
        seq_floor: u64,
    ) -> Self {
        Self::start_with(DisplayAnnouncer::new(backend, key, seq_floor))
    }

    /// Start the driver with a prepared announcer (tests use a fixed clock).
    pub fn start_with<B: AnnounceBackend + 'static>(mut announcer: DisplayAnnouncer<B>) -> Self {
        let (tx, mut rx) = watch::channel(None::<DisplayState>);
        let seq_cell = announcer.seq_cell();
        let id = announcer.id();
        let task = tokio::spawn(async move {
            loop {
                let due = announcer.next_refresh();
                tokio::select! {
                    changed = rx.changed() => {
                        if changed.is_err() {
                            break;
                        }
                        let state = rx.borrow_and_update().clone();
                        let _ = announcer.set(state.as_ref(), Instant::now());
                    }
                    () = async {
                        match due {
                            Some(t) => tokio::time::sleep_until(t).await,
                            None => std::future::pending().await,
                        }
                    } => announcer.refresh(Instant::now()),
                }
            }
            // The announcer is dropped here: `gone` record, then goodbye.
        });
        Self {
            tx,
            task,
            seq_cell,
            id,
        }
    }

    /// Start with the `mdns-sd` backend.
    pub fn start_mdns(key: DisplayKey, seq_floor: u64) -> Self {
        Self::start(MdnsBackend::new(), key, seq_floor)
    }

    /// The id of the announced display.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The value the app should persist with the key: the last `seq` used plus
    /// [`SEQ_RESERVE`]. Pass it back as `seq_floor` after a restart.
    pub fn seq_to_persist(&self) -> u64 {
        self.seq_cell
            .load(Ordering::SeqCst)
            .saturating_add(SEQ_RESERVE)
    }

    /// Say what is true now (`None`: do not announce).
    pub fn set(&self, state: Option<DisplayState>) {
        self.tx.send_replace(state);
    }

    /// Withdraw the record and wait until it is done.
    pub async fn stop(self) {
        let Self { tx, task, .. } = self;
        drop(tx);
        let _ = task.await;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::signed::{Reject, verify};

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Call {
        Publish(String, Vec<(String, String)>),
        Withdraw,
    }

    #[derive(Clone, Default)]
    struct Fake {
        calls: Arc<Mutex<Vec<Call>>>,
        fail: Arc<Mutex<bool>>,
    }

    impl Fake {
        fn calls(&self) -> Vec<Call> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl AnnounceBackend for Fake {
        fn publish(&mut self, instance: &str, txt: &[(String, String)]) -> Result<(), MdnsError> {
            if *self.fail.lock().unwrap() {
                return Err(MdnsError("down".into()));
            }
            self.calls
                .lock()
                .unwrap()
                .push(Call::Publish(instance.into(), txt.to_vec()));
            Ok(())
        }
        fn withdraw(&mut self) {
            self.calls.lock().unwrap().push(Call::Withdraw);
        }
    }

    fn key() -> DisplayKey {
        DisplayKey::from_secret(&[7; 32]).unwrap()
    }

    fn state(mode: ScreenMode, code: Option<&str>) -> DisplayState {
        DisplayState {
            room_name: "Room 204".into(),
            server: "10.0.0.5:8443".into(),
            mode,
            code: code.map(str::to_owned),
        }
    }

    fn get<'a>(txt: &'a [(String, String)], k: &str) -> Option<&'a str> {
        txt.iter()
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.as_str())
    }

    fn live(s: &DisplayState) -> Vec<(String, String)> {
        txt_records(s, &key(), 100, AnnounceState::Live).unwrap()
    }

    fn announcer(fake: &Fake) -> DisplayAnnouncer<Fake> {
        DisplayAnnouncer::with_clock(fake.clone(), key(), 0, Box::new(|| 1_000))
    }

    fn published(c: &Call) -> &Vec<(String, String)> {
        match c {
            Call::Publish(_, t) => t,
            Call::Withdraw => panic!("not a publish"),
        }
    }

    #[test]
    fn open_and_approval_carry_the_code_and_are_signed() {
        for mode in [ScreenMode::Open, ScreenMode::Approval] {
            let t = live(&state(mode, Some("P4X9TR")));
            assert_eq!(get(&t, "v"), Some("2"));
            assert_eq!(get(&t, "id"), Some(key().id().as_str()));
            assert_eq!(get(&t, "pk"), Some(key().public_key_b64().as_str()));
            assert_eq!(get(&t, "seq"), Some("100"));
            assert_eq!(get(&t, "state"), Some("live"));
            assert_eq!(get(&t, "srv"), Some("10.0.0.5:8443"));
            assert_eq!(get(&t, "mode"), Some(mode.as_str()));
            assert_eq!(get(&t, "code"), Some("P4X9TR"));
            assert_eq!(get(&t, "name"), Some("Room 204"));
            assert_eq!(get(&t, "tls"), None);
            // The record has nothing else.
            let keys: Vec<_> = t.iter().map(|(k, _)| k.as_str()).collect();
            assert_eq!(
                keys,
                [
                    "v", "id", "pk", "seq", "state", "srv", "mode", "code", "name", "sig"
                ]
            );
            assert_eq!(verify(&t).unwrap().code, "P4X9TR");
        }
    }

    #[test]
    fn locked_never_announces_a_code() {
        // Even when the caller still passes the code.
        let t = live(&state(ScreenMode::Locked, Some("P4X9TR")));
        assert_eq!(get(&t, "mode"), Some("locked"));
        assert_eq!(get(&t, "code"), None);
        assert!(t.iter().all(|(k, v)| k != "token" && !v.contains("P4X9TR")));
        assert!(verify(&t).is_ok());
        let t = live(&state(ScreenMode::Locked, None));
        assert_eq!(get(&t, "name"), Some("Room 204"));
        assert_eq!(get(&t, "srv"), Some("10.0.0.5:8443"));
    }

    #[test]
    fn open_without_a_valid_code_is_not_announced() {
        for code in [
            None,
            Some(""),
            Some("p4x9tr"),
            Some("P4X9T"),
            Some("P4X9T0"),
        ] {
            assert_eq!(
                txt_records(
                    &state(ScreenMode::Open, code),
                    &key(),
                    1,
                    AnnounceState::Live
                ),
                Err(AnnounceError::NoCode)
            );
        }
    }

    #[test]
    fn name_is_cleaned_cut_and_never_falls_back() {
        let mut s = state(ScreenMode::Open, Some("P4X9TR"));
        s.room_name = format!("\u{202e}A\u{7} B{}", "x".repeat(100));
        let t = live(&s);
        let n = get(&t, "name").unwrap();
        assert_eq!(n.chars().count(), 32);
        assert!(n.starts_with("A Bxxx"));
        s.room_name = "\u{200b}  ".into();
        let t = live(&s);
        assert_eq!(get(&t, "name"), None);
        // A record without a name still verifies (the empty name is signed).
        assert!(verify(&t).is_ok());
        s.room_name = String::new();
        assert_eq!(get(&live(&s), "name"), None);
    }

    #[test]
    fn tls_flag_and_server_checks() {
        let mut s = state(ScreenMode::Open, Some("P4X9TR"));
        s.server = "wss://relay.example.org:443".into();
        let t = live(&s);
        assert_eq!(get(&t, "srv"), Some("relay.example.org:443"));
        assert_eq!(get(&t, "tls"), Some("1"));
        assert!(verify(&t).unwrap().tls);
        for bad in [
            "127.0.0.1:8443",
            "8.8.8.8:443",
            "relay.example.org:443",
            "x",
            "a b:1",
            "",
        ] {
            s.server = bad.into();
            assert_eq!(
                txt_records(&s, &key(), 1, AnnounceState::Live),
                Err(AnnounceError::BadServer),
                "{bad}"
            );
        }
    }

    #[test]
    fn oversize_is_refused_and_a_normal_record_is_small() {
        let mut s = state(ScreenMode::Open, Some("P4X9TR"));
        let label = |c: &str| c.repeat(60);
        s.server = format!(
            "wss://{}.{}.{}.{}.org:443",
            label("a"),
            label("b"),
            label("c"),
            label("d")
        );
        s.room_name = "\u{1F600}".repeat(32);
        assert_eq!(
            txt_records(&s, &key(), 1, AnnounceState::Live),
            Err(AnnounceError::TooLarge)
        );
        let s = state(ScreenMode::Open, Some("P4X9TR"));
        let size: usize = live(&s).iter().map(|(k, v)| 2 + k.len() + v.len()).sum();
        assert!((200..320).contains(&size), "{size}");
        assert!(size < MAX_TXT_BYTES);
    }

    #[test]
    fn instance_name_comes_from_the_id() {
        assert_eq!(instance_name("0123456789abcdef"), "CometCast-01234567");
        let fake = Fake::default();
        let mut a = announcer(&fake);
        a.set(
            Some(&state(ScreenMode::Open, Some("P4X9TR"))),
            Instant::now(),
        )
        .unwrap();
        assert!(
            matches!(&fake.calls()[0], Call::Publish(i, _) if *i == format!("CometCast-{}", &key().id()[..8]))
        );
    }

    #[tokio::test(start_paused = true)]
    async fn publishes_changes_at_once_and_refreshes_before_expiry() {
        let fake = Fake::default();
        let mut a = announcer(&fake);
        let t0 = Instant::now();
        a.set(Some(&state(ScreenMode::Open, Some("P4X9TR"))), t0)
            .unwrap();
        assert_eq!(fake.calls().len(), 1);
        // Same state again: nothing.
        a.set(Some(&state(ScreenMode::Open, Some("P4X9TR"))), t0)
            .unwrap();
        assert_eq!(fake.calls().len(), 1);
        // New code, mode and name: one publish each, immediately.
        a.set(Some(&state(ScreenMode::Open, Some("K7Q2MW"))), t0)
            .unwrap();
        a.set(Some(&state(ScreenMode::Approval, Some("K7Q2MW"))), t0)
            .unwrap();
        let mut s = state(ScreenMode::Approval, Some("K7Q2MW"));
        s.room_name = "Elbe".into();
        a.set(Some(&s), t0).unwrap();
        assert_eq!(fake.calls().len(), 4);
        // Refresh: not before 10 s, then again; always well inside the 30 s lifetime.
        a.refresh(t0 + Duration::from_secs(9));
        assert_eq!(fake.calls().len(), 4);
        a.refresh(t0 + Duration::from_secs(10));
        assert_eq!(fake.calls().len(), 5);
        assert!(REFRESH_EVERY * 2 < RECORD_TTL);
        // Locked: the record is replaced by one without a code.
        a.set(
            Some(&state(ScreenMode::Locked, Some("K7Q2MW"))),
            t0 + Duration::from_secs(11),
        )
        .unwrap();
        let calls = fake.calls();
        assert_eq!(get(published(calls.last().unwrap()), "code"), None);
        // Goodbye: a signed gone record, then the withdrawal.
        a.set(None, t0 + Duration::from_secs(12)).unwrap();
        let calls = fake.calls();
        assert_eq!(calls.last(), Some(&Call::Withdraw));
        let gone = verify(published(&calls[calls.len() - 2])).unwrap();
        assert_eq!(gone.state, AnnounceState::Gone);
        assert!(a.next_refresh().is_none());
        a.set(None, t0).unwrap();
        assert_eq!(
            fake.calls()
                .iter()
                .filter(|c| **c == Call::Withdraw)
                .count(),
            1
        );
    }

    #[tokio::test(start_paused = true)]
    async fn every_record_has_a_higher_seq_and_a_valid_signature() {
        let fake = Fake::default();
        // A clock that does not move: seq still grows by one.
        let mut a = announcer(&fake);
        let t0 = Instant::now();
        for (i, code) in ["P4X9TR", "K7Q2MW", "AAAAAA"].into_iter().enumerate() {
            a.set(Some(&state(ScreenMode::Open, Some(code))), t0)
                .unwrap();
            a.refresh(t0 + REFRESH_EVERY * (i as u32 + 1));
        }
        a.set(None, t0).unwrap();
        let seqs: Vec<u64> = fake
            .calls()
            .iter()
            .filter(|c| matches!(c, Call::Publish(..)))
            .map(|c| verify(published(c)).unwrap().seq)
            .collect();
        assert!(seqs.len() >= 4, "{seqs:?}");
        assert_eq!(seqs[0], 1_000);
        assert!(seqs.windows(2).all(|w| w[0] < w[1]), "{seqs:?}");
        assert_eq!(a.last_seq(), *seqs.last().unwrap());
    }

    #[tokio::test(start_paused = true)]
    async fn the_persisted_seq_floor_survives_a_clock_that_went_back() {
        let fake = Fake::default();
        // The clock says 5, the persisted floor says 9000.
        let mut a = DisplayAnnouncer::with_clock(fake.clone(), key(), 9_000, Box::new(|| 5));
        a.set(
            Some(&state(ScreenMode::Open, Some("P4X9TR"))),
            Instant::now(),
        )
        .unwrap();
        let first = verify(published(&fake.calls()[0])).unwrap().seq;
        assert_eq!(first, 9_001);
    }

    #[tokio::test(start_paused = true)]
    async fn an_unannounceable_state_withdraws_the_old_record() {
        let fake = Fake::default();
        let mut a = announcer(&fake);
        let t0 = Instant::now();
        a.set(Some(&state(ScreenMode::Open, Some("P4X9TR"))), t0)
            .unwrap();
        assert_eq!(
            a.set(Some(&state(ScreenMode::Open, None)), t0),
            Err(AnnounceError::NoCode)
        );
        assert_eq!(fake.calls().last(), Some(&Call::Withdraw));
        assert!(!a.is_published());
        // The gone record was published before the goodbye.
        let calls = fake.calls();
        assert_eq!(
            verify(published(&calls[calls.len() - 2])).unwrap().state,
            AnnounceState::Gone
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_failed_publish_is_retried_and_a_failed_gone_is_harmless() {
        let fake = Fake::default();
        *fake.fail.lock().unwrap() = true;
        let mut a = announcer(&fake);
        let t0 = Instant::now();
        a.set(Some(&state(ScreenMode::Open, Some("P4X9TR"))), t0)
            .unwrap();
        assert!(!a.is_published());
        *fake.fail.lock().unwrap() = false;
        a.refresh(t0 + REFRESH_EVERY);
        assert!(a.is_published());
        assert_eq!(fake.calls().len(), 1);
        // The backend fails while leaving: the goodbye still happens.
        *fake.fail.lock().unwrap() = true;
        a.set(None, t0).unwrap();
        assert_eq!(fake.calls().last(), Some(&Call::Withdraw));
    }

    #[tokio::test(start_paused = true)]
    async fn a_forged_key_cannot_use_the_id_of_the_display() {
        let fake = Fake::default();
        let mut a = announcer(&fake);
        a.set(
            Some(&state(ScreenMode::Open, Some("P4X9TR"))),
            Instant::now(),
        )
        .unwrap();
        let mut t = published(&fake.calls()[0]).clone();
        let other = DisplayKey::from_secret(&[8; 32]).unwrap();
        t.iter_mut().find(|(k, _)| k == "pk").unwrap().1 = other.public_key_b64();
        assert_eq!(verify(&t), Err(Reject::IdMismatch));
    }

    #[tokio::test(start_paused = true)]
    async fn the_driver_follows_the_state_refreshes_and_says_goodbye() {
        let fake = Fake::default();
        let ann = DisplayAnnouncement::start(fake.clone(), key(), 0);
        assert_eq!(ann.id(), key().id());
        ann.set(Some(state(ScreenMode::Open, Some("P4X9TR"))));
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(fake.calls().len(), 1);
        let persisted = ann.seq_to_persist();
        assert!(persisted > SEQ_RESERVE);
        // A change is published within a second (here: at once).
        ann.set(Some(state(ScreenMode::Open, Some("K7Q2MW"))));
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(fake.calls().len(), 2);
        // 25 s later the record has been refreshed twice: it never ages past 10 s.
        tokio::time::sleep(Duration::from_secs(25)).await;
        assert_eq!(fake.calls().len(), 4);
        ann.set(None);
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(fake.calls().last(), Some(&Call::Withdraw));
        // Nothing is refreshed while nothing is announced.
        let n = fake.calls().len();
        tokio::time::sleep(Duration::from_secs(60)).await;
        assert_eq!(fake.calls().len(), n);
        ann.set(Some(state(ScreenMode::Open, Some("P4X9TR"))));
        tokio::time::sleep(Duration::from_millis(10)).await;
        ann.stop().await;
        assert_eq!(fake.calls().last(), Some(&Call::Withdraw));
    }
}
