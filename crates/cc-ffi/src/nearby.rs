// SPDX-License-Identifier: GPL-3.0-or-later
//! State behind the "screens on this network" API (spec 14): the announcement of the running
//! display and the feed of the native browse bridges. The types and functions that FRB sees
//! are in `api.rs`.

use std::sync::Mutex;

use cc_discovery::{
    AnnounceBackend, DisplayAnnouncement, DisplayState, MdnsError, RawRecord, RawSink, ScreenMode,
};
use cc_proto::JoinMode;

use crate::api::{NativeAnnounceEvent, NearbyUiEvent};
use crate::frb_generated::StreamSink;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

pub(crate) fn screen_mode(m: JoinMode) -> ScreenMode {
    match m {
        JoinMode::Open => ScreenMode::Open,
        JoinMode::Confirm => ScreenMode::Approval,
        JoinMode::Locked => ScreenMode::Locked,
    }
}

// ---------------------------------------------------------------------------
// The display side
// ---------------------------------------------------------------------------

/// What the running receiver knows (the announcement is built from it, so the announced code is
/// always the code the display shows).
#[derive(Clone)]
struct ReceiverFacts {
    generation: u64,
    server: String,
    room_name: String,
    code: String,
    mode: ScreenMode,
}

#[derive(Default)]
struct AnnounceCtl {
    /// The UI says the display shows Idle or Presenting in the foreground.
    enabled: bool,
    /// The stable id of this installation (checked by `cc-discovery`).
    id: Option<String>,
    receiver: Option<ReceiverFacts>,
    announcement: Option<DisplayAnnouncement>,
}

static ANNOUNCE: Mutex<Option<AnnounceCtl>> = Mutex::new(None);

/// Why nothing is announced / what is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Wanted {
    Yes,
    NotEnabled,
    NoReceiver,
    Forbidden,
    BadId,
}

fn policy_allows() -> bool {
    let report = cc_discovery::discover_servers_offline(&cc_discovery::DiscoverOptions::default());
    crate::api::announcements_allowed(&report)
}

impl AnnounceCtl {
    fn wanted(&self, policy_ok: bool) -> Wanted {
        if !self.enabled {
            Wanted::NotEnabled
        } else if !policy_ok {
            Wanted::Forbidden
        } else if self
            .id
            .as_deref()
            .is_none_or(|i| !cc_discovery::nearby::is_valid_id(i))
        {
            Wanted::BadId
        } else if self.receiver.is_none() {
            Wanted::NoReceiver
        } else {
            Wanted::Yes
        }
    }

    /// Bring the announcement in line with the facts. Must run inside a Tokio runtime.
    /// What is to be announced now, if anything.
    fn state(&self, policy_ok: bool) -> Option<DisplayState> {
        match (self.wanted(policy_ok), &self.receiver, &self.id) {
            (Wanted::Yes, Some(r), Some(id)) => Some(DisplayState {
                id: id.clone(),
                room_name: r.room_name.clone(),
                server: r.server.clone(),
                mode: r.mode,
                code: Some(r.code.clone()),
            }),
            _ => None,
        }
    }

    fn apply(&mut self, policy_ok: bool) -> Wanted {
        let w = self.wanted(policy_ok);
        match self.state(policy_ok) {
            Some(s) => self
                .announcement
                .get_or_insert_with(start_announcement)
                .set(Some(s)),
            None => {
                // Drop it: the record is withdrawn (goodbye) and the sockets are freed.
                self.announcement = None;
            }
        }
        w
    }
}

/// The platform that announces on its own (iPadOS, `NWListener`): where the core's announcement
/// is sent instead of to `mdns-sd` (decision N50).
static NATIVE_ANNOUNCE: Mutex<Option<StreamSink<NativeAnnounceEvent>>> = Mutex::new(None);

/// Forwards what would go on the wire to the platform.
struct NativeBackend(StreamSink<NativeAnnounceEvent>);

impl AnnounceBackend for NativeBackend {
    fn publish(&mut self, instance: &str, txt: &[(String, String)]) -> Result<(), MdnsError> {
        self.0
            .add(NativeAnnounceEvent::Publish {
                instance: instance.to_owned(),
                txt: txt.to_vec(),
            })
            .map_err(|_| MdnsError("the platform announcer is gone".into()))
    }

    fn withdraw(&mut self) {
        let _ = self.0.add(NativeAnnounceEvent::Withdraw);
    }
}

fn start_announcement() -> DisplayAnnouncement {
    match lock(&NATIVE_ANNOUNCE).clone() {
        Some(sink) => DisplayAnnouncement::start(NativeBackend(sink)),
        None => DisplayAnnouncement::start_mdns(),
    }
}

/// The platform takes over the announcement ([`crate::api::watch_native_announcements`]).
pub(crate) fn set_native_announce(sink: Option<StreamSink<NativeAnnounceEvent>>) {
    *lock(&NATIVE_ANNOUNCE) = sink;
    // An announcement that already runs goes on the wire it was started with; the app calls this
    // before the display announces.
}

fn with_ctl<R>(f: impl FnOnce(&mut AnnounceCtl) -> R) -> R {
    let mut g = lock(&ANNOUNCE);
    f(g.get_or_insert_with(AnnounceCtl::default))
}

/// The UI says whether the display may announce (Idle or Presenting, in the foreground).
pub(crate) fn set_enabled(enabled: bool, id: String) -> Wanted {
    let policy_ok = policy_allows();
    with_ctl(|c| {
        c.enabled = enabled;
        c.id = Some(id);
        c.apply(policy_ok)
    })
}

/// A receiver got its room: from now on its code is announced (if allowed).
pub(crate) fn receiver_started(
    generation: u64,
    server: String,
    room_name: Option<String>,
    code: String,
    mode: JoinMode,
) {
    let policy_ok = policy_allows();
    with_ctl(|c| {
        c.receiver = Some(ReceiverFacts {
            generation,
            server,
            room_name: room_name.unwrap_or_default(),
            code,
            mode: screen_mode(mode),
        });
        c.apply(policy_ok);
    });
}

/// The join mode changed (asked by the UI, or confirmed by the server). Locked removes the code
/// from the record at once.
pub(crate) fn receiver_mode(generation: u64, mode: JoinMode) {
    let policy_ok = policy_allows();
    with_ctl(|c| {
        if let Some(r) = c.receiver.as_mut().filter(|r| r.generation == generation) {
            r.mode = screen_mode(mode);
            c.apply(policy_ok);
        }
    });
}

/// The receiver ended (stop, room ended, server lost, error): goodbye.
pub(crate) fn receiver_stopped(generation: u64) {
    with_ctl(|c| {
        if c.receiver
            .as_ref()
            .is_some_and(|r| r.generation == generation)
        {
            c.receiver = None;
            c.apply(true);
        }
    });
}

/// Withdraws the announcement of a receiver when dropped (the receiver task ended).
pub(crate) struct ReceiverGuard(u64);

impl ReceiverGuard {
    pub(crate) fn new(generation: u64) -> Self {
        Self(generation)
    }
}

impl Drop for ReceiverGuard {
    fn drop(&mut self) {
        receiver_stopped(self.0);
    }
}

// ---------------------------------------------------------------------------
// The sender side: the native browse bridges push records here
// ---------------------------------------------------------------------------

struct Feed {
    generation: u64,
    sink: RawSink,
    events: StreamSink<NearbyUiEvent>,
}

static FEED: Mutex<Option<Feed>> = Mutex::new(None);

pub(crate) fn set_feed(generation: u64, sink: RawSink, events: StreamSink<NearbyUiEvent>) {
    *lock(&FEED) = Some(Feed {
        generation,
        sink,
        events,
    });
}

pub(crate) fn clear_feed(generation: u64) {
    let mut f = lock(&FEED);
    if f.as_ref().is_some_and(|f| f.generation == generation) {
        *f = None;
    }
}

/// A native bridge saw a service. Addresses that do not parse are dropped.
pub(crate) fn native_seen(instance: String, addresses: Vec<String>, txt: Vec<(String, String)>) {
    if let Some(f) = lock(&FEED).as_ref() {
        f.sink.seen(RawRecord {
            instance,
            addresses: addresses
                .iter()
                .filter_map(|a| a.parse().ok())
                .take(cc_discovery::announce::MAX_ADDRESSES)
                .collect(),
            txt: txt.into_iter().take(32).collect(),
        });
    }
}

pub(crate) fn native_gone(instance: String) {
    if let Some(f) = lock(&FEED).as_ref() {
        f.sink.gone(instance);
    }
}

pub(crate) fn native_reset() {
    if let Some(f) = lock(&FEED).as_ref() {
        f.sink.reset();
    }
}

pub(crate) fn native_event(ev: NearbyUiEvent) {
    if let Some(f) = lock(&FEED).as_ref() {
        let _ = f.events.add(ev);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "0123456789abcdef";

    fn ctl(enabled: bool, id: Option<&str>, receiver: bool, mode: ScreenMode) -> AnnounceCtl {
        AnnounceCtl {
            enabled,
            id: id.map(str::to_owned),
            receiver: receiver.then(|| ReceiverFacts {
                generation: 1,
                server: "10.0.0.5:8443".into(),
                room_name: "Elbe".into(),
                code: "P4X9TR".into(),
                mode,
            }),
            announcement: None,
        }
    }

    #[test]
    fn announces_only_when_everything_allows_it() {
        let ok = ctl(true, Some(ID), true, ScreenMode::Open);
        assert_eq!(ok.wanted(true), Wanted::Yes);
        let s = ok.state(true).unwrap();
        assert_eq!((s.id.as_str(), s.room_name.as_str()), (ID, "Elbe"));
        assert_eq!(s.code.as_deref(), Some("P4X9TR"));
        // Setup, offline and background (the UI says no).
        assert_eq!(
            ctl(false, Some(ID), true, ScreenMode::Open).wanted(true),
            Wanted::NotEnabled
        );
        // The administrator's policy wins over everything.
        assert_eq!(ok.wanted(false), Wanted::Forbidden);
        assert!(ok.state(false).is_none());
        // No room yet (offline: no valid code).
        assert_eq!(
            ctl(true, Some(ID), false, ScreenMode::Open).wanted(true),
            Wanted::NoReceiver
        );
        // A bad or missing id is never announced.
        for id in [None, Some(""), Some("ABC"), Some("0123456789ABCDEF")] {
            assert_eq!(
                ctl(true, id, true, ScreenMode::Open).wanted(true),
                Wanted::BadId
            );
        }
    }

    #[test]
    fn join_modes_map_to_announced_modes() {
        assert_eq!(screen_mode(JoinMode::Open), ScreenMode::Open);
        assert_eq!(screen_mode(JoinMode::Confirm), ScreenMode::Approval);
        assert_eq!(screen_mode(JoinMode::Locked), ScreenMode::Locked);
    }

    #[test]
    fn locked_state_has_no_code_in_the_record() {
        let c = ctl(true, Some(ID), true, ScreenMode::Locked);
        let s = c.state(true).unwrap();
        let txt = cc_discovery::display::txt_records(&s).unwrap();
        assert!(txt.iter().all(|(k, v)| k != "code" && v != "P4X9TR"));
    }

    #[test]
    fn code_alphabet_is_the_one_of_the_protocol() {
        assert_eq!(
            cc_discovery::nearby::CODE_ALPHABET,
            cc_proto::RoomCode::ALPHABET
        );
        assert_eq!(cc_discovery::nearby::CODE_LEN, cc_proto::RoomCode::LEN);
    }
}
