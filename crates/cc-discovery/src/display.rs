// SPDX-License-Identifier: GPL-3.0-or-later
//! The announcement of a display device (spec 14.8 of `docs/design/first-run-and-display.md`,
//! TXT layout in `docs/discovery.md`, section 3).
//!
//! A display announces `_cometcast._tcp` only while it shows Idle or Presenting with a valid
//! code. The caller ([`DisplayAnnouncement`]) says what is true now ([`DisplayState`], or
//! `None` for "must not announce": setup, offline, policy forbids, background); this module
//! keeps the record equal to it:
//!
//! * a change is published at once (the limit of the spec is 1 s);
//! * the record is published again every [`REFRESH_EVERY`] so that, with a record lifetime of
//!   [`RECORD_TTL`], a display that dies without a goodbye leaves the lists within 30 s;
//! * `None` (and dropping the announcement) withdraws the record (goodbye);
//! * the record is small ([`MAX_TXT_BYTES`]) and holds nothing but the fields of the layout:
//!   no presenter data, no busy flag;
//! * **Locked** never carries a code (the code of a state in mode `locked` is not written
//!   even when the caller passes one).
//!
//! The wire part is a trait ([`AnnounceBackend`]) so the logic is tested with a fake; the
//! `mdns-sd` backend is [`MdnsBackend`].
//!
//! UNCERTAIN: `mdns-sd` 0.21 has no public setter for the TTL of a service's records (they go
//! out with 4500 s for TXT/PTR and 120 s for SRV/address). The 30 s rule is therefore met by
//! the receiving side (a row is dropped after 30 s without a refresh, `nearby.rs`) and by
//! publishing again every 10 s, and not by the TTL field on the wire. Whether a
//! re-publication is visible to every browser as a refresh is not verified on real devices.

use std::time::Duration;

use mdns_sd::{ServiceDaemon, ServiceInfo};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::address::{Trust, clean_room_name, network_server, normalize_server, validate_hostname};
use crate::announce::{MDNS_SERVICE_TYPE, MdnsError};
use crate::nearby::{LAYOUT_VERSION, ScreenMode, is_valid_code, is_valid_id};

/// The lifetime a record is meant to have (spec 14.8, point 5).
pub const RECORD_TTL: Duration = Duration::from_secs(30);
/// The record is published again this often (well before [`RECORD_TTL`] runs out).
pub const REFRESH_EVERY: Duration = Duration::from_secs(10);
/// The most bytes of one TXT record (DNS-SD lengths included). Spec 14.8, point 6: "well
/// under 400"; a server name so long that the record would not fit is not announced.
pub const MAX_TXT_BYTES: usize = 400;
/// The port the service names. Displays do not listen for senders; DNS-SD only needs one.
pub const SERVICE_PORT: u16 = 9;

/// What a display announces right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayState {
    /// The stable random instance id of this installation (8 to 32 lower-case hex digits).
    pub id: String,
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
    /// The instance id is not 8 to 32 lower-case hex digits.
    #[error("announcement: bad instance id")]
    BadId,
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

/// Build the TXT pairs of `state`, in a fixed order. Key `name` is left out when the room name
/// is empty after cleaning.
pub fn txt_records(state: &DisplayState) -> Result<Vec<(String, String)>, AnnounceError> {
    if !is_valid_id(&state.id) {
        return Err(AnnounceError::BadId);
    }
    let full =
        normalize_server(&state.server, Trust::Admin).map_err(|_| AnnounceError::BadServer)?;
    let (tls, authority) = match full.split_once("://") {
        Some(("wss" | "https", a)) => (true, a),
        Some((_, a)) => (false, a),
        None => (false, full.as_str()),
    };
    let (checked, _) =
        network_server(authority, Some(tls)).map_err(|_| AnnounceError::BadServer)?;
    let authority = checked
        .split_once("://")
        .map_or(checked.as_str(), |(_, a)| a)
        .to_owned();
    let mut txt: Vec<(String, String)> = vec![
        ("v".into(), LAYOUT_VERSION.into()),
        ("id".into(), state.id.clone()),
        ("srv".into(), authority),
    ];
    if tls {
        txt.push(("tls".into(), "1".into()));
    }
    txt.push(("mode".into(), state.mode.as_str().into()));
    match state.mode {
        // Never a code (and never a token) while locked.
        ScreenMode::Locked => {}
        ScreenMode::Open | ScreenMode::Approval => {
            let code = state.code.as_deref().ok_or(AnnounceError::NoCode)?;
            if !is_valid_code(code) {
                return Err(AnnounceError::NoCode);
            }
            txt.push(("code".into(), code.into()));
        }
    }
    let name = clean_room_name(&state.room_name);
    if !name.is_empty() {
        txt.push(("name".into(), name));
    }
    let size: usize = txt.iter().map(|(k, v)| 1 + k.len() + 1 + v.len()).sum();
    if size > MAX_TXT_BYTES {
        return Err(AnnounceError::TooLarge);
    }
    Ok(txt)
}

/// The wire side of an announcement. A trait so tests need no network.
pub trait AnnounceBackend: Send {
    /// Publish the record, or replace the record published before (and announce it).
    fn publish(&mut self, instance: &str, txt: &[(String, String)]) -> Result<(), MdnsError>;
    /// Withdraw the record (goodbye). Does nothing when none is published.
    fn withdraw(&mut self);
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
}

impl AnnounceBackend for MdnsBackend {
    fn publish(&mut self, instance: &str, txt: &[(String, String)]) -> Result<(), MdnsError> {
        let label = match &self.host {
            Some(h) => h.clone(),
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

/// Keeps the published record equal to the wanted state. Pure apart from the backend; time is
/// passed in.
pub struct DisplayAnnouncer<B: AnnounceBackend> {
    backend: B,
    wanted: Option<(String, Vec<(String, String)>)>,
    published: bool,
    last_publish: Option<Instant>,
}

impl<B: AnnounceBackend> DisplayAnnouncer<B> {
    /// An announcer that announces nothing yet.
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            wanted: None,
            published: false,
            last_publish: None,
        }
    }

    /// The backend (tests look at what was published).
    pub fn backend(&self) -> &B {
        &self.backend
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
        let txt = match txt_records(state) {
            Ok(t) => t,
            Err(e) => {
                self.wanted = None;
                self.withdraw();
                return Err(e);
            }
        };
        let instance = instance_name(&state.id);
        let unchanged = self.published
            && self
                .wanted
                .as_ref()
                .is_some_and(|(i, t)| *i == instance && *t == txt);
        if let Some((old, _)) = &self.wanted
            && *old != instance
        {
            // Another id (a data reset) is another service.
            self.withdraw();
        }
        self.wanted = Some((instance, txt));
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

    fn publish(&mut self, now: Instant) {
        let Some((instance, txt)) = &self.wanted else {
            return;
        };
        // A failed publish is tried again at the next refresh; nothing is shown to anybody.
        self.published = self.backend.publish(instance, txt).is_ok();
        self.last_publish = Some(now);
    }

    fn withdraw(&mut self) {
        if self.published {
            self.backend.withdraw();
        }
        self.published = false;
        self.last_publish = None;
    }
}

impl<B: AnnounceBackend> Drop for DisplayAnnouncer<B> {
    fn drop(&mut self) {
        self.withdraw();
    }
}

/// A running announcement of a display. Dropping it (or calling [`DisplayAnnouncement::stop`])
/// withdraws the record.
pub struct DisplayAnnouncement {
    tx: watch::Sender<Option<DisplayState>>,
    task: JoinHandle<()>,
}

impl DisplayAnnouncement {
    /// Start the driver with `backend`. Must be called in a Tokio runtime.
    pub fn start<B: AnnounceBackend + 'static>(backend: B) -> Self {
        let (tx, mut rx) = watch::channel(None::<DisplayState>);
        let task = tokio::spawn(async move {
            let mut announcer = DisplayAnnouncer::new(backend);
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
            // The announcer is dropped here: goodbye.
        });
        Self { tx, task }
    }

    /// Start with the `mdns-sd` backend.
    pub fn start_mdns() -> Self {
        Self::start(MdnsBackend::new())
    }

    /// Say what is true now (`None`: do not announce).
    pub fn set(&self, state: Option<DisplayState>) {
        self.tx.send_replace(state);
    }

    /// Withdraw the record and wait until it is done.
    pub async fn stop(self) {
        let Self { tx, task } = self;
        drop(tx);
        let _ = task.await;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    const ID: &str = "0123456789abcdef";

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

    fn state(mode: ScreenMode, code: Option<&str>) -> DisplayState {
        DisplayState {
            id: ID.into(),
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

    #[test]
    fn open_and_approval_carry_the_code() {
        for mode in [ScreenMode::Open, ScreenMode::Approval] {
            let t = txt_records(&state(mode, Some("P4X9TR"))).unwrap();
            assert_eq!(get(&t, "v"), Some("1"));
            assert_eq!(get(&t, "id"), Some(ID));
            assert_eq!(get(&t, "srv"), Some("10.0.0.5:8443"));
            assert_eq!(get(&t, "mode"), Some(mode.as_str()));
            assert_eq!(get(&t, "code"), Some("P4X9TR"));
            assert_eq!(get(&t, "name"), Some("Room 204"));
            assert_eq!(get(&t, "tls"), None);
            // The record has nothing else.
            let keys: Vec<_> = t.iter().map(|(k, _)| k.as_str()).collect();
            assert_eq!(keys, ["v", "id", "srv", "mode", "code", "name"]);
        }
    }

    #[test]
    fn locked_never_announces_a_code() {
        // Even when the caller still passes the code.
        let t = txt_records(&state(ScreenMode::Locked, Some("P4X9TR"))).unwrap();
        assert_eq!(get(&t, "mode"), Some("locked"));
        assert_eq!(get(&t, "code"), None);
        assert!(t.iter().all(|(k, v)| k != "token" && !v.contains("P4X9TR")));
        let t = txt_records(&state(ScreenMode::Locked, None)).unwrap();
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
                txt_records(&state(ScreenMode::Open, code)),
                Err(AnnounceError::NoCode)
            );
        }
    }

    #[test]
    fn name_is_cleaned_cut_and_never_falls_back() {
        let mut s = state(ScreenMode::Open, Some("P4X9TR"));
        s.room_name = format!("\u{202e}A\u{7} B{}", "x".repeat(100));
        let t = txt_records(&s).unwrap();
        let n = get(&t, "name").unwrap();
        assert_eq!(n.chars().count(), 32);
        assert!(n.starts_with("A Bxxx"));
        s.room_name = "\u{200b}  ".into();
        assert_eq!(get(&txt_records(&s).unwrap(), "name"), None);
        s.room_name = String::new();
        assert_eq!(get(&txt_records(&s).unwrap(), "name"), None);
    }

    #[test]
    fn tls_flag_and_server_checks() {
        let mut s = state(ScreenMode::Open, Some("P4X9TR"));
        s.server = "wss://relay.example.org:443".into();
        let t = txt_records(&s).unwrap();
        assert_eq!(get(&t, "srv"), Some("relay.example.org:443"));
        assert_eq!(get(&t, "tls"), Some("1"));
        for bad in [
            "127.0.0.1:8443",
            "8.8.8.8:443",
            "relay.example.org:443",
            "x",
            "a b:1",
            "",
        ] {
            s.server = bad.into();
            assert_eq!(txt_records(&s), Err(AnnounceError::BadServer), "{bad}");
        }
    }

    #[test]
    fn bad_ids_and_oversize_are_refused() {
        let mut s = state(ScreenMode::Open, Some("P4X9TR"));
        for bad in [
            "",
            "abc",
            "0123456789ABCDEF",
            "g123456789abcdef",
            &"a".repeat(40),
        ] {
            s.id = bad.into();
            assert_eq!(txt_records(&s), Err(AnnounceError::BadId), "{bad}");
        }
        s.id = ID.into();
        let label = |c: &str| c.repeat(60);
        s.server = format!(
            "wss://{}.{}.{}.{}.org:443",
            label("a"),
            label("b"),
            label("c"),
            label("d")
        );
        s.room_name = "\u{1F600}".repeat(32);
        assert_eq!(txt_records(&s), Err(AnnounceError::TooLarge));
        // A normal record is far below the limit.
        let s = state(ScreenMode::Open, Some("P4X9TR"));
        let size: usize = txt_records(&s)
            .unwrap()
            .iter()
            .map(|(k, v)| 2 + k.len() + v.len())
            .sum();
        assert!(size < 120, "{size}");
    }

    #[test]
    fn instance_name_comes_from_the_id() {
        assert_eq!(instance_name(ID), "CometCast-01234567");
    }

    #[tokio::test(start_paused = true)]
    async fn publishes_changes_at_once_and_refreshes_before_expiry() {
        let fake = Fake::default();
        let mut a = DisplayAnnouncer::new(fake.clone());
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
        let Some(Call::Publish(_, txt)) = fake.calls().last().cloned() else {
            panic!()
        };
        assert_eq!(get(&txt, "code"), None);
        // Goodbye.
        a.set(None, t0 + Duration::from_secs(12)).unwrap();
        assert_eq!(fake.calls().last(), Some(&Call::Withdraw));
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
    async fn an_unannounceable_state_withdraws_the_old_record() {
        let fake = Fake::default();
        let mut a = DisplayAnnouncer::new(fake.clone());
        let t0 = Instant::now();
        a.set(Some(&state(ScreenMode::Open, Some("P4X9TR"))), t0)
            .unwrap();
        assert_eq!(
            a.set(Some(&state(ScreenMode::Open, None)), t0),
            Err(AnnounceError::NoCode)
        );
        assert_eq!(fake.calls().last(), Some(&Call::Withdraw));
        assert!(!a.is_published());
    }

    #[tokio::test(start_paused = true)]
    async fn a_failed_publish_is_retried() {
        let fake = Fake::default();
        *fake.fail.lock().unwrap() = true;
        let mut a = DisplayAnnouncer::new(fake.clone());
        let t0 = Instant::now();
        a.set(Some(&state(ScreenMode::Open, Some("P4X9TR"))), t0)
            .unwrap();
        assert!(!a.is_published());
        *fake.fail.lock().unwrap() = false;
        a.refresh(t0 + REFRESH_EVERY);
        assert!(a.is_published());
        assert_eq!(fake.calls().len(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn a_new_id_withdraws_the_old_service() {
        let fake = Fake::default();
        let mut a = DisplayAnnouncer::new(fake.clone());
        let t0 = Instant::now();
        a.set(Some(&state(ScreenMode::Open, Some("P4X9TR"))), t0)
            .unwrap();
        let mut s = state(ScreenMode::Open, Some("P4X9TR"));
        s.id = "fedcba9876543210".into();
        a.set(Some(&s), t0).unwrap();
        let calls = fake.calls();
        assert_eq!(calls[1], Call::Withdraw);
        assert!(matches!(&calls[2], Call::Publish(i, _) if i == "CometCast-fedcba98"));
    }

    #[tokio::test(start_paused = true)]
    async fn the_driver_follows_the_state_refreshes_and_says_goodbye() {
        let fake = Fake::default();
        let ann = DisplayAnnouncement::start(fake.clone());
        ann.set(Some(state(ScreenMode::Open, Some("P4X9TR"))));
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(fake.calls().len(), 1);
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
