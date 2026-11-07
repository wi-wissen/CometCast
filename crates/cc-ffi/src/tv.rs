// SPDX-License-Identifier: GPL-3.0-or-later
//! Sending to TVs without Comet Cast from the desktop presenter app (native-apps.md 21.5):
//! the list of Google Cast and AirPlay receivers (`watch_tvs`) and the session to one of them
//! (`start_tv_presenter`), which answers the commands of the normal presenter session
//! (`present_screen`, `present_content`, `stop_presenting`, `stop_presenter`).
//!
//! The work on a TV is behind [`TvLink`] (blocking calls, run on the blocking pool), so the
//! session logic is tested with a fake link; the real links are [`CastLink`] (`cc-cast`) and
//! [`AirplayLink`] (`cc-airplay-sender`).
//!
//! Errors: `tv_pair_wrong` and `tv_unsupported_content` are shown inline and do **not** end the
//! stream; `tv_refused` and `tv_lost` do (like every other final `Error`).

use std::collections::BTreeMap;
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use cc_airplay_sender::client::Session;
use cc_airplay_sender::creds::{Credentials, PairingStore};
use cc_airplay_sender::discovery::{Device as AirplayDevice, feature};
use cc_airplay_sender::error::SenderError;
use cc_airplay_sender::hap::{self, PinSetup};
use cc_airplay_sender::screen::{ScreenCast, ScreenOptions as AirplayScreen, start_cast};
use cc_airplay_sender::{legacy, photo};
use cc_cast::{CastDevice, DeviceAuth, MirrorConfig, TrustAnchors};
use tokio::sync::{mpsc, oneshot};

use crate::api::{
    ContentSource, LinkCheck, PresenterUiEvent, TvKind, TvUiEvent, UiContentKind, UiSendRejection,
};
use crate::frb_generated::StreamSink;
use crate::presenter::PCmd;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

// ---------------------------------------------------------------------------
// The devices
// ---------------------------------------------------------------------------

/// A TV the search found.
#[derive(Clone)]
pub(crate) enum Tv {
    Cast(CastDevice),
    AirPlay(AirplayDevice),
}

/// The id of a TV in the UI: kind and the device's own id (`cast:ab12`, `airplay:aa:bb:..`).
pub(crate) fn tv_id(tv: &Tv) -> String {
    match tv {
        Tv::Cast(d) => format!("cast:{}", cc_cast::watch::device_key(d)),
        Tv::AirPlay(d) => format!("airplay:{}", d.store_key()),
    }
}

static REGISTRY: Mutex<BTreeMap<String, Tv>> = Mutex::new(BTreeMap::new());
static CAST_ANCHORS: Mutex<Option<TrustAnchors>> = Mutex::new(None);
static DATA_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Load the PEM file with the trust anchors for Cast device authentication (`None` removes
/// them). Without anchors Cast devices cannot be verified, so they are not offered
/// (`can_screen` false): the program embeds none (docs/cast.md). Returns whether anchors are
/// loaded now.
pub(crate) fn set_cast_anchors(path: Option<&str>) -> bool {
    let anchors = path
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| TrustAnchors::from_pem(&b).ok());
    let loaded = anchors.is_some();
    *lock(&CAST_ANCHORS) = anchors;
    loaded
}

fn cast_anchors_loaded() -> bool {
    lock(&CAST_ANCHORS).is_some()
}

/// The folder for pairing credentials (`airplay-pairings.json` in it).
pub(crate) fn set_data_dir(path: Option<&str>) {
    *lock(&DATA_DIR) = path.map(PathBuf::from);
}

fn pairing_store() -> Option<PairingStore> {
    match lock(&DATA_DIR).clone() {
        Some(dir) => Some(PairingStore::at(dir.join("airplay-pairings.json"))),
        None => PairingStore::default_location().ok(),
    }
}

/// The UI event for a device.
pub(crate) fn seen_event(tv: &Tv) -> TvUiEvent {
    match tv {
        Tv::Cast(d) => TvUiEvent::Seen {
            id: tv_id(tv),
            name: cc_cast::sanitize_display(&d.friendly_name, 64),
            kind: TvKind::Cast,
            addresses: d.addresses.iter().map(ToString::to_string).collect(),
            can_screen: d.has_video_out() && cast_anchors_loaded(),
            can_photo: false,
            can_video: false,
        },
        Tv::AirPlay(d) => {
            let video = d.has_feature(feature::VIDEO_V1) || d.has_feature(feature::VIDEO_V2);
            TvUiEvent::Seen {
                id: tv_id(tv),
                name: d.name.clone(),
                kind: TvKind::AirPlay,
                addresses: d.addrs.iter().map(ToString::to_string).collect(),
                // The screen goes as a live video the TV is told to play.
                can_screen: video,
                can_photo: d.has_feature(feature::PHOTO),
                can_video: video,
            }
        }
    }
}

/// Remember a TV and return the event for the UI.
pub(crate) fn register(tv: Tv) -> TvUiEvent {
    let ev = seen_event(&tv);
    lock(&REGISTRY).insert(tv_id(&tv), tv);
    ev
}

/// Forget a TV.
pub(crate) fn forget(id: &str) {
    lock(&REGISTRY).remove(id);
}

pub(crate) fn clear_registry() {
    lock(&REGISTRY).clear();
}

fn lookup(id: &str) -> Option<Tv> {
    lock(&REGISTRY).get(id).cloned()
}

// ---------------------------------------------------------------------------
// The search
// ---------------------------------------------------------------------------

/// Turn an event of the Cast search into the UI event (and keep the device for a session).
pub(crate) fn cast_event(ev: cc_cast::watch::WatchEvent) -> TvUiEvent {
    match ev {
        cc_cast::watch::WatchEvent::Seen(d) => register(Tv::Cast(d)),
        cc_cast::watch::WatchEvent::Gone { id } => {
            let id = format!("cast:{id}");
            forget(&id);
            TvUiEvent::Gone { id }
        }
    }
}

/// Same for the AirPlay search.
pub(crate) fn airplay_event(ev: cc_airplay_sender::watch::WatchEvent) -> TvUiEvent {
    match ev {
        cc_airplay_sender::watch::WatchEvent::Seen(d) => register(Tv::AirPlay(d)),
        cc_airplay_sender::watch::WatchEvent::Gone { key } => {
            let id = format!("airplay:{key}");
            forget(&id);
            TvUiEvent::Gone { id }
        }
    }
}

/// Run both searches until `stop` fires (or its sender is dropped) or Dart stops listening.
pub(crate) fn watch(events: StreamSink<TvUiEvent>, mut stop: oneshot::Receiver<()>) {
    clear_registry();
    let cast = cc_cast::watch::Watch::start();
    let airplay = cc_airplay_sender::watch::Watch::start();
    if cast.is_err() && airplay.is_err() {
        let _ = events.add(TvUiEvent::Unavailable);
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("cc-tv-watch".into())
        .spawn(move || {
            let (mut cast_watch, cast_rx) = match cast {
                Ok((w, r)) => (Some(w), Some(r)),
                Err(_) => (None, None),
            };
            let (mut ap_watch, ap_rx) = match airplay {
                Ok((w, r)) => (Some(w), Some(r)),
                Err(_) => (None, None),
            };
            'outer: loop {
                // `Empty`: keep going. A value or a closed channel: stop.
                if !matches!(stop.try_recv(), Err(oneshot::error::TryRecvError::Empty)) {
                    break;
                }
                let mut quiet = true;
                while let Some(ev) = cast_rx.as_ref().and_then(|r| r.try_recv().ok()) {
                    quiet = false;
                    if events.add(cast_event(ev)).is_err() {
                        break 'outer;
                    }
                }
                while let Some(ev) = ap_rx.as_ref().and_then(|r| r.try_recv().ok()) {
                    quiet = false;
                    if events.add(airplay_event(ev)).is_err() {
                        break 'outer;
                    }
                }
                if quiet {
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
            if let Some(w) = cast_watch.as_mut() {
                w.stop();
            }
            if let Some(w) = ap_watch.as_mut() {
                w.stop();
            }
        });
    let _ = spawned;
}

// ---------------------------------------------------------------------------
// One TV
// ---------------------------------------------------------------------------

/// Why a call on a TV failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TvFail {
    /// The TV shows a code; call `pair` with it.
    PairingRequired,
    /// The code was wrong (a new one is shown).
    PairWrong,
    /// The TV said no (`tv_refused`, ends the session).
    Refused(String),
    /// The connection to the TV is gone (`tv_lost`, ends the session).
    Lost(String),
    /// The TV cannot show this (`tv_unsupported_content`, inline).
    Unsupported(String),
    /// Anything else (inline, `unknown`).
    Failed(String),
}

/// The calls the session makes on a TV. All blocking.
pub(crate) trait TvLink: Send {
    /// The TV's name.
    fn name(&self) -> String;
    /// Connect and authenticate; `PairingRequired` when the TV wants its code first.
    fn connect(&mut self) -> Result<(), TvFail>;
    /// Pair with the code shown on the TV; on success the link is connected.
    fn pair(&mut self, pin: &str) -> Result<(), TvFail>;
    /// Show the screen.
    fn screen(&mut self) -> Result<(), TvFail>;
    /// Show a picture (JPEG bytes).
    fn photo(&mut self, jpeg: Vec<u8>) -> Result<(), TvFail>;
    /// Play a media link.
    fn play(&mut self, url: &str) -> Result<(), TvFail>;
    /// Stop what is shown (the connection stays).
    fn stop(&mut self);
    /// Has the running presentation ended by itself (the TV stopped it, the stream failed)?
    fn ended(&mut self) -> Option<TvFail>;
    /// Whether this TV can show photos (the UI decides what is offered; this guards the call).
    fn can_photo(&self) -> bool;
}

fn map_sender(e: &SenderError) -> TvFail {
    match e {
        SenderError::Auth(_) => TvFail::Refused(e.to_string()),
        SenderError::Status { status, .. } if matches!(status, 401 | 403 | 470) => {
            TvFail::Refused(e.to_string())
        }
        SenderError::Io(_) | SenderError::Timeout(_) => TvFail::Lost(e.to_string()),
        SenderError::Unsupported(_) | SenderError::Input(_) | SenderError::TooLarge(_) => {
            TvFail::Unsupported(e.to_string())
        }
        _ => TvFail::Failed(e.to_string()),
    }
}

// --- AirPlay ----------------------------------------------------------------

enum Pending {
    Legacy,
    Hap(PinSetup),
}

struct AirplayLink {
    dev: AirplayDevice,
    store: Option<PairingStore>,
    session: Option<Session>,
    pending: Option<Pending>,
    cast: Option<ScreenCast>,
}

impl AirplayLink {
    fn new(dev: AirplayDevice) -> Self {
        Self {
            dev,
            store: pairing_store(),
            session: None,
            pending: None,
            cast: None,
        }
    }

    fn wants_pin(&self) -> bool {
        [
            feature::LEGACY_PAIRING,
            feature::HK_PAIRING,
            feature::CORE_UTILS_PAIRING,
        ]
        .iter()
        .any(|b| self.dev.has_feature(*b))
    }

    fn hap_preferred(&self) -> bool {
        self.dev.has_feature(feature::HK_PAIRING)
            || self.dev.has_feature(feature::CORE_UTILS_PAIRING)
            || self.dev.has_feature(feature::SYSTEM_PAIRING)
    }

    /// Open a connection and make the TV show its code.
    fn start_pairing(&mut self) -> Result<(), TvFail> {
        let mut s = Session::connect(&self.dev).map_err(|e| map_sender(&e))?;
        let pending = if self.hap_preferred() {
            Pending::Hap(hap::setup_pin_start(s.conn_mut()).map_err(|e| map_sender(&e))?)
        } else {
            legacy::pair_pin_start(s.conn_mut()).map_err(|e| map_sender(&e))?;
            Pending::Legacy
        };
        self.session = Some(s);
        self.pending = Some(pending);
        Ok(())
    }
}

impl TvLink for AirplayLink {
    fn name(&self) -> String {
        self.dev.name.clone()
    }

    fn connect(&mut self) -> Result<(), TvFail> {
        let creds = self
            .store
            .as_ref()
            .and_then(|s| s.get(&self.dev.store_key()).ok().flatten());
        if creds.is_none() && self.wants_pin() {
            self.start_pairing()?;
            return Err(TvFail::PairingRequired);
        }
        let mut s = Session::connect(&self.dev).map_err(|e| map_sender(&e))?;
        s.authenticate(creds.as_ref()).map_err(|e| map_sender(&e))?;
        self.session = Some(s);
        Ok(())
    }

    fn pair(&mut self, pin: &str) -> Result<(), TvFail> {
        let (Some(mut s), Some(pending)) = (self.session.take(), self.pending.take()) else {
            return Err(TvFail::Failed("not waiting for a code".into()));
        };
        let creds: Result<Credentials, SenderError> = match pending {
            Pending::Legacy => legacy::pair_finish(s.conn_mut(), pin),
            Pending::Hap(p) => hap::setup_pin_finish(s.conn_mut(), p, pin).map(|(c, _)| c),
        };
        match creds {
            Ok(c) => {
                if let Some(store) = &self.store {
                    let _ = store.put(&self.dev.store_key(), c.clone());
                }
                drop(s);
                let mut fresh = Session::connect(&self.dev).map_err(|e| map_sender(&e))?;
                fresh.authenticate(Some(&c)).map_err(|e| map_sender(&e))?;
                self.session = Some(fresh);
                Ok(())
            }
            Err(SenderError::Auth(_)) => {
                // Wrong code: the TV shows a new one for the next try.
                drop(s);
                self.start_pairing()?;
                Err(TvFail::PairWrong)
            }
            Err(e) => Err(map_sender(&e)),
        }
    }

    fn screen(&mut self) -> Result<(), TvFail> {
        let s = self
            .session
            .as_mut()
            .ok_or_else(|| TvFail::Failed("not connected".into()))?;
        let cast = start_cast(s, &AirplayScreen::default()).map_err(|e| match e {
            SenderError::Input(m) | SenderError::Io(m) => TvFail::Failed(m),
            other => map_sender(&other),
        })?;
        self.cast = Some(cast);
        Ok(())
    }

    fn photo(&mut self, jpeg: Vec<u8>) -> Result<(), TvFail> {
        self.stop();
        let s = self
            .session
            .as_mut()
            .ok_or_else(|| TvFail::Failed("not connected".into()))?;
        s.photo(jpeg).map_err(|e| map_sender(&e))
    }

    fn play(&mut self, url: &str) -> Result<(), TvFail> {
        photo::check_media_link(url).map_err(|e| TvFail::Unsupported(e.to_string()))?;
        self.stop();
        let s = self
            .session
            .as_mut()
            .ok_or_else(|| TvFail::Failed("not connected".into()))?;
        s.play(url).map_err(|e| map_sender(&e))
    }

    fn stop(&mut self) {
        if let Some(mut c) = self.cast.take() {
            c.stop();
        }
        if let Some(s) = self.session.as_mut() {
            let _ = s.stop();
        }
    }

    fn ended(&mut self) -> Option<TvFail> {
        let finished = self.cast.as_ref().is_some_and(|c| c.streamer.is_finished());
        if finished {
            self.cast = None;
            return Some(TvFail::Lost("the screen stream ended".into()));
        }
        None
    }

    fn can_photo(&self) -> bool {
        self.dev.has_feature(feature::PHOTO)
    }
}

// --- Cast -------------------------------------------------------------------

struct CastLink {
    dev: CastDevice,
    rt: tokio::runtime::Handle,
    stop: Option<oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<Result<(), String>>>,
}

impl CastLink {
    fn addr(&self) -> Option<SocketAddr> {
        self.dev
            .connect_ip()
            .map(|ip| SocketAddr::new(ip, self.dev.port))
    }
}

impl TvLink for CastLink {
    fn name(&self) -> String {
        cc_cast::sanitize_display(&self.dev.friendly_name, 64)
    }

    fn connect(&mut self) -> Result<(), TvFail> {
        let addr = self
            .addr()
            .ok_or_else(|| TvFail::Lost("no address".into()))?;
        TcpStream::connect_timeout(&addr, Duration::from_secs(5))
            .map(drop)
            .map_err(|e| TvFail::Lost(e.to_string()))
    }

    fn pair(&mut self, _pin: &str) -> Result<(), TvFail> {
        Err(TvFail::Failed("Cast needs no code".into()))
    }

    fn screen(&mut self) -> Result<(), TvFail> {
        let anchors = lock(&CAST_ANCHORS).clone();
        let Some(anchors) = anchors else {
            return Err(TvFail::Refused(
                "no trust anchors for device authentication".into(),
            ));
        };
        let addr = self
            .addr()
            .ok_or_else(|| TvFail::Lost("no address".into()))?;
        self.stop();
        let cfg = MirrorConfig {
            addr,
            auth: DeviceAuth::Verify(anchors),
            display: None,
            fps: 15,
            bitrate_kbps: 4000,
            android_rtp_hack: false,
        };
        let (tx, rx) = oneshot::channel::<()>();
        let task = self.rt.spawn(async move {
            let stop = async {
                let _ = rx.await;
            };
            cc_cast::mirror_screen(cfg, &|_| {}, stop)
                .await
                .map(drop)
                .map_err(|e| e.to_string())
        });
        self.stop = Some(tx);
        self.task = Some(task);
        Ok(())
    }

    fn photo(&mut self, _jpeg: Vec<u8>) -> Result<(), TvFail> {
        Err(TvFail::Unsupported("Cast shows the screen only".into()))
    }

    fn play(&mut self, _url: &str) -> Result<(), TvFail> {
        Err(TvFail::Unsupported("Cast shows the screen only".into()))
    }

    fn stop(&mut self) {
        if let Some(tx) = self.stop.take() {
            let _ = tx.send(());
        }
        if let Some(t) = self.task.take() {
            // The mirroring ends on its own after the stop signal; do not wait for it here.
            drop(t);
        }
    }

    fn ended(&mut self) -> Option<TvFail> {
        if self
            .task
            .as_ref()
            .is_some_and(tokio::task::JoinHandle::is_finished)
        {
            let t = self.task.take()?;
            self.stop = None;
            return Some(match self.rt.block_on(t) {
                Ok(Err(m)) => TvFail::Lost(m),
                _ => TvFail::Lost("the screen stream ended".into()),
            });
        }
        None
    }

    fn can_photo(&self) -> bool {
        false
    }
}

/// Make the link for a TV.
fn make_link(tv: Tv) -> Box<dyn TvLink> {
    #[cfg(test)]
    if let Some(f) = lock(&test_support::FACTORY).as_ref() {
        return f(&tv);
    }
    match tv {
        Tv::Cast(dev) => Box::new(CastLink {
            dev,
            rt: tokio::runtime::Handle::current(),
            stop: None,
            task: None,
        }),
        Tv::AirPlay(dev) => Box::new(AirplayLink::new(dev)),
    }
}

// ---------------------------------------------------------------------------
// The session
// ---------------------------------------------------------------------------

type SharedLink = Arc<Mutex<Box<dyn TvLink>>>;

async fn blocking<T: Send + 'static>(
    link: &SharedLink,
    f: impl FnOnce(&mut dyn TvLink) -> T + Send + 'static,
) -> Option<T> {
    let link = Arc::clone(link);
    tokio::task::spawn_blocking(move || f(lock(&link).as_mut()))
        .await
        .ok()
}

/// Where the session sends its events: the bridge's stream sink, or a channel in tests.
pub(crate) trait Emit {
    /// Send one event; `false` when nobody listens any more.
    fn emit(&self, ev: PresenterUiEvent) -> bool;
}

impl Emit for StreamSink<PresenterUiEvent> {
    fn emit(&self, ev: PresenterUiEvent) -> bool {
        self.add(ev).is_ok()
    }
}

/// Tell the app about a failure; `true` when the failure ends the session.
fn report(events: &impl Emit, fail: &TvFail) -> bool {
    let (code, message, last) = match fail {
        TvFail::PairWrong => ("tv_pair_wrong", String::new(), false),
        TvFail::Unsupported(m) => ("tv_unsupported_content", m.clone(), false),
        TvFail::Failed(m) => ("unknown", m.clone(), false),
        TvFail::Refused(m) => ("tv_refused", m.clone(), true),
        TvFail::Lost(m) => ("tv_lost", m.clone(), true),
        TvFail::PairingRequired => return false,
    };
    events.emit(PresenterUiEvent::Error {
        code: code.into(),
        message,
    });
    last
}

/// A picture file by its extension.
fn is_image_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    [".png", ".jpg", ".jpeg", ".webp", ".gif"]
        .iter()
        .any(|e| n.ends_with(e))
}

/// Connect to the TV and run its session until the person stops it or it ends.
/// `None` for an unknown id is an immediate `tv_lost`.
pub(crate) async fn run(
    id: String,
    events: impl Emit,
    mut stop: oneshot::Receiver<()>,
    install: impl FnOnce(mpsc::UnboundedSender<PCmd>),
) {
    let Some(tv) = lookup(&id) else {
        events.emit(PresenterUiEvent::Error {
            code: "tv_lost".into(),
            message: "unknown TV".into(),
        });
        return;
    };
    let link: SharedLink = Arc::new(Mutex::new(make_link(tv)));
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<PCmd>();
    install(cmd_tx);
    let name = lock(&link).name();

    // Connect (and pair).
    let first = tokio::select! {
        r = blocking(&link, |l| l.connect()) => r.unwrap_or(Err(TvFail::Lost("task failed".into()))),
        _ = &mut stop => { close(&link).await; return; }
    };
    let mut joined = false;
    match first {
        Ok(()) => joined = true,
        Err(TvFail::PairingRequired) => {
            events.emit(PresenterUiEvent::PairingRequired);
        }
        Err(f) => {
            report(&events, &f);
            close(&link).await;
            return;
        }
    }
    if joined
        && !events.emit(PresenterUiEvent::Joined {
            room_name: Some(name.clone()),
        })
    {
        close(&link).await;
        return;
    }
    let mut showing = false;
    let mut tick = tokio::time::interval(Duration::from_millis(500));
    loop {
        tokio::select! {
            biased;
            _ = &mut stop => break,
            cmd = cmd_rx.recv() => {
                let Some(cmd) = cmd else { break };
                match cmd {
                    PCmd::Pair { pin, reply } => {
                        if joined {
                            let _ = reply.send(false);
                            continue;
                        }
                        let r = blocking(&link, move |l| l.pair(&pin)).await
                            .unwrap_or(Err(TvFail::Lost("task failed".into())));
                        match r {
                            Ok(()) => {
                                joined = true;
                                let _ = reply.send(true);
                                events.emit(PresenterUiEvent::Joined { room_name: Some(name.clone()) });
                            }
                            Err(TvFail::PairWrong) => {
                                let _ = reply.send(false);
                                report(&events, &TvFail::PairWrong);
                            }
                            Err(f) => {
                                let _ = reply.send(false);
                                if report(&events, &f) { break; }
                            }
                        }
                    }
                    _ if !joined => {
                        // Nothing can be shown before the TV is paired.
                        report(&events, &TvFail::Unsupported("not paired".into()));
                    }
                    PCmd::Screen(_) => {
                        match blocking(&link, |l| l.screen()).await.unwrap_or(Err(TvFail::Lost("task failed".into()))) {
                            Ok(()) => {
                                showing = true;
                                events.emit(PresenterUiEvent::Presenting);
                            }
                            Err(f) => { if report(&events, &f) { break; } }
                        }
                    }
                    PCmd::Content(source) => {
                        match show_content(&link, &events, source).await {
                            Ok(true) => showing = true,
                            Ok(false) => {}
                            Err(f) => { if report(&events, &f) { break; } }
                        }
                    }
                    PCmd::StopPresenting => {
                        if showing {
                            let _ = blocking(&link, |l| l.stop()).await;
                            showing = false;
                            events.emit(PresenterUiEvent::PresentingStopped);
                        }
                    }
                    // Not for a TV: no encoded input, no page commands, no hand-over.
                    PCmd::Encoded | PCmd::Cmd(_) | PCmd::Handover(_) => {
                        report(&events, &TvFail::Unsupported("not available for a TV".into()));
                    }
                }
            }
            _ = tick.tick(), if showing => {
                if let Some(f) = blocking(&link, |l| l.ended()).await.flatten() {
                    showing = false;
                    if report(&events, &f) { break; }
                    events.emit(PresenterUiEvent::PresentingStopped);
                }
            }
        }
    }
    close(&link).await;
}

async fn close(link: &SharedLink) {
    let _ = blocking(link, |l| l.stop()).await;
}

/// Show a file or a link. `Ok(true)` = something is shown now.
async fn show_content(
    link: &SharedLink,
    events: &impl Emit,
    source: ContentSource,
) -> Result<bool, TvFail> {
    match source {
        ContentSource::File { path } => {
            let name = std::path::Path::new(&path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if !is_image_name(&name) || !lock(link).can_photo() {
                return Err(TvFail::Unsupported(format!("cannot show {name}")));
            }
            let bytes = tokio::task::spawn_blocking(move || std::fs::read(&path))
                .await
                .map_err(|e| TvFail::Failed(e.to_string()))?
                .map_err(|_| TvFail::Failed("unreadable".into()))?;
            let size = bytes.len() as u64;
            let jpeg = tokio::task::spawn_blocking(move || photo::jpeg_for_photo(&bytes))
                .await
                .map_err(|e| TvFail::Failed(e.to_string()))?
                .map_err(|e| TvFail::Unsupported(e.to_string()))?;
            events.emit(PresenterUiEvent::ContentPrepared {
                name,
                kind: UiContentKind::Image,
                size,
                pages: None,
                host: None,
            });
            blocking(link, move |l| l.photo(jpeg))
                .await
                .unwrap_or(Err(TvFail::Lost("task failed".into())))?;
            events.emit(PresenterUiEvent::ContentSent);
            events.emit(PresenterUiEvent::Presenting);
            Ok(true)
        }
        ContentSource::Link { url } => {
            let LinkCheck::Valid { url: norm, host } = crate::content::check_link(&url) else {
                events.emit(PresenterUiEvent::ContentRejected {
                    reason: UiSendRejection::BadLink,
                });
                return Ok(false);
            };
            photo::check_media_link(&norm).map_err(|e| TvFail::Unsupported(e.to_string()))?;
            let u = norm.clone();
            events.emit(PresenterUiEvent::ContentPrepared {
                name: host.clone(),
                kind: UiContentKind::Link,
                size: 0,
                pages: None,
                host: Some(host),
            });
            blocking(link, move |l| l.play(&u))
                .await
                .unwrap_or(Err(TvFail::Lost("task failed".into())))?;
            events.emit(PresenterUiEvent::Presenting);
            Ok(true)
        }
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    pub(crate) type Factory = Box<dyn Fn(&Tv) -> Box<dyn TvLink> + Send>;
    pub(crate) static FACTORY: Mutex<Option<Factory>> = Mutex::new(None);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// Tests share the factory and the registry: one at a time.
    static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    #[derive(Clone)]
    struct Sink(mpsc::UnboundedSender<PresenterUiEvent>);

    impl Emit for Sink {
        fn emit(&self, ev: PresenterUiEvent) -> bool {
            self.0.send(ev).is_ok()
        }
    }

    /// What the fake TV was asked to do, and how it answers.
    #[derive(Default)]
    struct Script {
        calls: Vec<String>,
        needs_pin: bool,
        right_pin: &'static str,
        photo: bool,
        screen_result: Option<TvFail>,
        ended: Option<TvFail>,
    }

    struct Fake(Arc<Mutex<Script>>, Arc<AtomicBool>);

    impl TvLink for Fake {
        fn name(&self) -> String {
            "Living Room".into()
        }
        fn connect(&mut self) -> Result<(), TvFail> {
            let mut s = lock(&self.0);
            s.calls.push("connect".into());
            if s.needs_pin {
                Err(TvFail::PairingRequired)
            } else {
                Ok(())
            }
        }
        fn pair(&mut self, pin: &str) -> Result<(), TvFail> {
            let mut s = lock(&self.0);
            s.calls.push(format!("pair {pin}"));
            if pin == s.right_pin {
                s.needs_pin = false;
                Ok(())
            } else {
                Err(TvFail::PairWrong)
            }
        }
        fn screen(&mut self) -> Result<(), TvFail> {
            let mut s = lock(&self.0);
            s.calls.push("screen".into());
            s.screen_result.take().map_or(Ok(()), Err)
        }
        fn photo(&mut self, jpeg: Vec<u8>) -> Result<(), TvFail> {
            lock(&self.0)
                .calls
                .push(format!("photo {}", jpeg.len() > 3));
            Ok(())
        }
        fn play(&mut self, url: &str) -> Result<(), TvFail> {
            lock(&self.0).calls.push(format!("play {url}"));
            Ok(())
        }
        fn stop(&mut self) {
            lock(&self.0).calls.push("stop".into());
            self.1.store(true, Ordering::SeqCst);
        }
        fn ended(&mut self) -> Option<TvFail> {
            lock(&self.0).ended.take()
        }
        fn can_photo(&self) -> bool {
            lock(&self.0).photo
        }
    }

    struct Harness {
        events: mpsc::UnboundedReceiver<PresenterUiEvent>,
        cmds: mpsc::UnboundedReceiver<mpsc::UnboundedSender<PCmd>>,
        script: Arc<Mutex<Script>>,
        stop: Option<oneshot::Sender<()>>,
        done: Option<tokio::task::JoinHandle<()>>,
        cmd: Option<mpsc::UnboundedSender<PCmd>>,
        _guard: tokio::sync::MutexGuard<'static, ()>,
    }

    fn airplay(features_hex: &str) -> Tv {
        let d = cc_airplay_sender::discovery::device_from_parts(
            "Living Room._airplay._tcp.local.",
            "tv.local.",
            7000,
            ["192.168.1.9".parse().unwrap()],
            [
                ("deviceid".to_owned(), "AA:BB:CC:DD:EE:FF".to_owned()),
                ("features".to_owned(), features_hex.to_owned()),
            ],
        );
        Tv::AirPlay(d)
    }

    async fn start(script: Script) -> Harness {
        let guard = ONE_AT_A_TIME.lock().await;
        let script = Arc::new(Mutex::new(script));
        let s2 = Arc::clone(&script);
        *lock(&test_support::FACTORY) = Some(Box::new(move |_| {
            Box::new(Fake(Arc::clone(&s2), Arc::new(AtomicBool::new(false))))
        }));
        let tv = airplay("0x2,0x0");
        let id = tv_id(&tv);
        register(tv);
        let (etx, events) = mpsc::unbounded_channel();
        let (ctx_tx, cmds) = mpsc::unbounded_channel();
        let (stop_tx, stop_rx) = oneshot::channel();
        let done = tokio::spawn(async move {
            run(id, Sink(etx), stop_rx, move |tx| {
                let _ = ctx_tx.send(tx);
            })
            .await;
        });
        Harness {
            events,
            cmds,
            script,
            stop: Some(stop_tx),
            done: Some(done),
            cmd: None,
            _guard: guard,
        }
    }

    impl Harness {
        async fn next(&mut self) -> PresenterUiEvent {
            tokio::time::timeout(Duration::from_secs(20), self.events.recv())
                .await
                .expect("no event in time")
                .expect("the session ended")
        }

        async fn cmd(&mut self, c: PCmd) {
            if self.cmd.is_none() {
                self.cmd = self.cmds.recv().await;
            }
            self.cmd.as_ref().unwrap().send(c).unwrap();
        }

        async fn finish(mut self) -> Vec<String> {
            drop(self.stop.take());
            if let Some(d) = self.done.take() {
                let _ = tokio::time::timeout(Duration::from_secs(20), d).await;
            }
            *lock(&test_support::FACTORY) = None;
            clear_registry();
            let calls = lock(&self.script).calls.clone();
            calls
        }
    }

    #[tokio::test]
    async fn a_paired_tv_joins_with_its_name_and_shows_the_screen() {
        let mut h = start(Script::default()).await;
        assert_eq!(
            h.next().await,
            PresenterUiEvent::Joined {
                room_name: Some("Living Room".into())
            }
        );
        h.cmd(PCmd::Screen(None)).await;
        assert_eq!(h.next().await, PresenterUiEvent::Presenting);
        h.cmd(PCmd::StopPresenting).await;
        assert_eq!(h.next().await, PresenterUiEvent::PresentingStopped);
        let calls = h.finish().await;
        assert_eq!(calls[..2], ["connect", "screen"]);
        assert!(calls.contains(&"stop".to_owned()));
    }

    #[tokio::test]
    async fn the_first_time_asks_for_the_code_and_a_wrong_one_is_inline() {
        let mut h = start(Script {
            needs_pin: true,
            right_pin: "1234",
            ..Script::default()
        })
        .await;
        assert_eq!(h.next().await, PresenterUiEvent::PairingRequired);
        // Nothing can be shown before pairing.
        h.cmd(PCmd::Screen(None)).await;
        assert!(matches!(
            h.next().await,
            PresenterUiEvent::Error { code, .. } if code == "tv_unsupported_content"
        ));
        let (tx, rx) = oneshot::channel();
        h.cmd(PCmd::Pair {
            pin: "0000".into(),
            reply: tx,
        })
        .await;
        assert!(!rx.await.unwrap());
        assert!(matches!(
            h.next().await,
            PresenterUiEvent::Error { code, .. } if code == "tv_pair_wrong"
        ));
        // The session is still open: the right code now works.
        let (tx, rx) = oneshot::channel();
        h.cmd(PCmd::Pair {
            pin: "1234".into(),
            reply: tx,
        })
        .await;
        assert!(rx.await.unwrap());
        assert_eq!(
            h.next().await,
            PresenterUiEvent::Joined {
                room_name: Some("Living Room".into())
            }
        );
        h.finish().await;
    }

    #[tokio::test]
    async fn a_refused_screen_ends_the_session_but_an_unsupported_one_does_not() {
        let mut h = start(Script {
            screen_result: Some(TvFail::Unsupported("x".into())),
            ..Script::default()
        })
        .await;
        h.next().await; // Joined
        h.cmd(PCmd::Screen(None)).await;
        assert!(matches!(
            h.next().await,
            PresenterUiEvent::Error { code, .. } if code == "tv_unsupported_content"
        ));
        lock(&h.script).screen_result = Some(TvFail::Refused("no".into()));
        h.cmd(PCmd::Screen(None)).await;
        assert!(matches!(
            h.next().await,
            PresenterUiEvent::Error { code, .. } if code == "tv_refused"
        ));
        // The session ends after a refusal.
        let done = h.done.take().unwrap();
        assert!(
            tokio::time::timeout(Duration::from_secs(20), done)
                .await
                .is_ok()
        );
        h.finish().await;
    }

    #[tokio::test]
    async fn a_picture_is_prepared_and_a_page_link_is_not_playable() {
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("pic.png");
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(8, 8, image::Rgb([1, 2, 3])))
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        std::fs::write(&png, bytes).unwrap();
        let pdf = dir.path().join("a.pdf");
        std::fs::write(&pdf, b"%PDF-1.7").unwrap();
        let mut h = start(Script {
            photo: true,
            ..Script::default()
        })
        .await;
        h.next().await; // Joined
        h.cmd(PCmd::Content(ContentSource::File {
            path: png.to_string_lossy().into_owned(),
        }))
        .await;
        assert!(matches!(
            h.next().await,
            PresenterUiEvent::ContentPrepared {
                kind: UiContentKind::Image,
                ..
            }
        ));
        assert_eq!(h.next().await, PresenterUiEvent::ContentSent);
        assert_eq!(h.next().await, PresenterUiEvent::Presenting);
        // A PDF cannot go to a TV.
        h.cmd(PCmd::Content(ContentSource::File {
            path: pdf.to_string_lossy().into_owned(),
        }))
        .await;
        assert!(matches!(
            h.next().await,
            PresenterUiEvent::Error { code, .. } if code == "tv_unsupported_content"
        ));
        // A web page is no media link; a media link plays.
        h.cmd(PCmd::Content(ContentSource::Link {
            url: "https://www.youtube.com/watch?v=abc".into(),
        }))
        .await;
        assert!(matches!(
            h.next().await,
            PresenterUiEvent::Error { code, .. } if code == "tv_unsupported_content"
        ));
        h.cmd(PCmd::Content(ContentSource::Link {
            url: "https://example.org/clip.mp4".into(),
        }))
        .await;
        assert!(matches!(
            h.next().await,
            PresenterUiEvent::ContentPrepared {
                kind: UiContentKind::Link,
                ..
            }
        ));
        assert_eq!(h.next().await, PresenterUiEvent::Presenting);
        let calls = h.finish().await;
        assert!(calls.contains(&"photo true".to_owned()), "{calls:?}");
        assert!(calls.contains(&"play https://example.org/clip.mp4".to_owned()));
    }

    #[tokio::test]
    async fn a_stream_that_ends_by_itself_is_a_lost_tv() {
        let mut h = start(Script::default()).await;
        h.next().await;
        h.cmd(PCmd::Screen(None)).await;
        assert_eq!(h.next().await, PresenterUiEvent::Presenting);
        lock(&h.script).ended = Some(TvFail::Lost("gone".into()));
        assert!(matches!(
            h.next().await,
            PresenterUiEvent::Error { code, .. } if code == "tv_lost"
        ));
        h.finish().await;
    }

    #[tokio::test]
    async fn an_unknown_tv_is_lost() {
        let _g = ONE_AT_A_TIME.lock().await;
        clear_registry();
        let (etx, mut events) = mpsc::unbounded_channel();
        let (_stop_tx, stop_rx) = oneshot::channel();
        run("cast:nope".into(), Sink(etx), stop_rx, |_| {}).await;
        assert!(matches!(
            events.recv().await,
            Some(PresenterUiEvent::Error { code, .. }) if code == "tv_lost"
        ));
    }

    #[test]
    fn what_each_way_offers() {
        let _g = ONE_AT_A_TIME.try_lock();
        // AirPlay: bit 0 video, bit 1 photo.
        let TvUiEvent::Seen {
            kind,
            can_screen,
            can_photo,
            can_video,
            id,
            name,
            ..
        } = seen_event(&airplay("0x3,0x0"))
        else {
            panic!("not Seen")
        };
        assert_eq!(kind, TvKind::AirPlay);
        assert!(can_screen && can_photo && can_video);
        assert_eq!(id, "airplay:aa:bb:cc:dd:ee:ff");
        assert_eq!(name, "Living Room");
        // Neither video nor photos: the app hides it.
        let TvUiEvent::Seen {
            can_screen,
            can_photo,
            can_video,
            ..
        } = seen_event(&airplay("0x0,0x0"))
        else {
            panic!("not Seen")
        };
        assert!(!can_screen && !can_photo && !can_video);
        // Cast: a screen, no photos or links; no trust anchors, no screen.
        set_cast_anchors(None);
        let d = CastDevice::from_txt(
            "Chromecast-1._googlecast._tcp.local.",
            8009,
            ["192.168.1.7".parse::<std::net::IpAddr>().unwrap()],
            [("id", "ab12"), ("fn", "Kitchen TV"), ("ca", "463365")],
        );
        let TvUiEvent::Seen {
            kind,
            can_screen,
            can_photo,
            can_video,
            id,
            ..
        } = seen_event(&Tv::Cast(d))
        else {
            panic!("not Seen")
        };
        assert_eq!(kind, TvKind::Cast);
        assert_eq!(id, "cast:ab12");
        assert!(!can_screen && !can_photo && !can_video);
    }

    #[test]
    fn the_search_events_keep_the_registry() {
        let _g = ONE_AT_A_TIME.try_lock();
        clear_registry();
        let tv = airplay("0x1,0x0");
        let Tv::AirPlay(dev) = tv.clone() else {
            unreachable!()
        };
        let id = tv_id(&tv);
        assert!(matches!(
            airplay_event(cc_airplay_sender::watch::WatchEvent::Seen(dev.clone())),
            TvUiEvent::Seen { .. }
        ));
        assert!(lookup(&id).is_some());
        assert_eq!(
            airplay_event(cc_airplay_sender::watch::WatchEvent::Gone {
                key: dev.store_key()
            }),
            TvUiEvent::Gone { id: id.clone() }
        );
        assert!(lookup(&id).is_none());
        assert_eq!(
            cast_event(cc_cast::watch::WatchEvent::Gone { id: "ab12".into() }),
            TvUiEvent::Gone {
                id: "cast:ab12".into()
            }
        );
    }

    #[test]
    fn sender_errors_map_to_the_failure_kinds() {
        assert!(matches!(
            map_sender(&SenderError::Io("x".into())),
            TvFail::Lost(_)
        ));
        assert!(matches!(
            map_sender(&SenderError::Auth("x".into())),
            TvFail::Refused(_)
        ));
        assert!(matches!(
            map_sender(&SenderError::Status {
                status: 470,
                what: "x".into()
            }),
            TvFail::Refused(_)
        ));
        assert!(matches!(
            map_sender(&SenderError::Input("x".into())),
            TvFail::Unsupported(_)
        ));
        assert!(matches!(
            map_sender(&SenderError::Protocol("x".into())),
            TvFail::Failed(_)
        ));
    }

    #[test]
    fn picture_names() {
        assert!(is_image_name("A.PNG") && is_image_name("b.jpeg") && is_image_name("c.gif"));
        assert!(!is_image_name("d.pdf") && !is_image_name("e.docx"));
    }
}
