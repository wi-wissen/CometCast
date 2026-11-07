// SPDX-License-Identifier: GPL-3.0-or-later
//! Windows backend: `Windows.Media.Miracast.MiracastReceiver` through the `windows` crate.
//!
//! COMPILE-CHECKED ONLY. This code has never been run. Every comment about what Windows does
//! at runtime is an assumption (UNCERTAIN) until `docs/miracast-device-test-plan.md` has been
//! done on a real Windows 11 machine.
//!
//! This file only translates between Windows and the platform independent [`Engine`]
//! (connections, consent, status, events; tested on Linux) and the [`Player`] (video).
//! Only the documented public API is used. No undocumented DLL (`WiFiDisplay.dll` and
//! friends) is touched. The input back-channel (UIBC: `InputDevices`, game controllers) and
//! the cursor channel are NOT enabled: nothing from the sender can control this PC.

use std::sync::Arc;
use std::sync::mpsc::Receiver;

use windows::ApplicationModel::Core::CoreApplicationView;
use windows::Foundation::{Deferral, TypedEventHandler};
use windows::Media::Miracast::{
    MiracastReceiver as WinReceiver, MiracastReceiverApplySettingsStatus,
    MiracastReceiverAuthorizationMethod, MiracastReceiverConnection,
    MiracastReceiverConnectionCreatedEventArgs, MiracastReceiverDisconnectReason,
    MiracastReceiverDisconnectedEventArgs, MiracastReceiverListeningStatus,
    MiracastReceiverMediaSourceCreatedEventArgs, MiracastReceiverSession,
    MiracastReceiverSessionStartStatus, MiracastReceiverWiFiStatus,
};
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
use windows::core::{HSTRING, IInspectable};

use crate::diagnosis::{AdapterSupport, FeatureState, Probe};
use crate::engine::{Conn, DisconnectKind, Engine};
use crate::frame::{FrameCounters, FrameMailbox, VideoFrame};
use crate::player::Player;
use crate::{
    ConnectionId, MiracastConfig, MiracastError, MiracastEvent, MiracastStatus, PairingPolicy,
    StreamHandle,
};

fn werr(context: &'static str) -> impl Fn(windows::core::Error) -> MiracastError {
    move |e| MiracastError::Windows(format!("{context}: {e}"))
}

/// Start the Windows Runtime on this thread (multithreaded apartment).
#[allow(unsafe_code)]
fn init_winrt() {
    // SAFETY: `RoInitialize` takes no pointers. Calling it again on a thread that is already
    // initialised is allowed. An error such as RPC_E_CHANGED_MODE means the thread is already
    // in another apartment mode, which is good enough for us, so the result is ignored.
    // UNCERTAIN: event delivery in an STA thread without a message pump was not tested.
    let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
}

/// A Windows connection for the [`Engine`].
#[derive(Clone, PartialEq)]
struct WinConn(MiracastReceiverConnection);

impl Conn for WinConn {
    type Hold = Deferral;

    fn disconnect(&self, kind: DisconnectKind) -> Result<(), String> {
        let reason = match kind {
            DisconnectKind::NotAccepted => MiracastReceiverDisconnectReason::ConnectionNotAccepted,
            DisconnectKind::Finished => MiracastReceiverDisconnectReason::Finished,
        };
        self.0.Disconnect(reason).map_err(|e| e.to_string())
    }

    fn complete(hold: &Deferral) -> Result<(), String> {
        hold.Complete().map_err(|e| e.to_string())
    }
}

/// Everything the Windows event handlers need.
struct Shared {
    engine: Arc<Engine<WinConn>>,
    player: Player,
}

impl Shared {
    fn on_connection_created(
        &self,
        args: &MiracastReceiverConnectionCreatedEventArgs,
    ) -> windows::core::Result<()> {
        let connection = WinConn(args.Connection()?);
        let raw_name = connection
            .0
            .Transmitter()
            .and_then(|t| t.Name())
            .map(|h| h.to_string_lossy())
            .unwrap_or_default();
        // UNCERTAIN: `Pin` may be empty when no PIN is used.
        let pin = args
            .Pin()
            .ok()
            .map(|h| h.to_string_lossy())
            .filter(|p| !p.is_empty());
        // Hold the connection open until a person answers (or the timeout hits). Without a
        // deferral Windows might finish the connection by itself, so a failure here makes the
        // engine reject (fail closed).
        self.engine
            .on_connection_created(&connection, &raw_name, pin, || {
                args.GetDeferral().map_err(|e| e.to_string())
            });
        Ok(())
    }

    fn on_media_source_created(
        &self,
        args: &MiracastReceiverMediaSourceCreatedEventArgs,
    ) -> windows::core::Result<()> {
        let connection = WinConn(args.Connection()?);
        if self.engine.is_closed() {
            return Ok(());
        }
        let source = args.MediaSource()?;
        let Some(id) = self.engine.active_id(&connection) else {
            // A stream for a connection nobody accepted: refuse it.
            self.engine.refuse_unconfirmed_stream(&connection);
            return Ok(());
        };
        // Take a deferral so Windows waits until the player has the source (UNCERTAIN: that
        // `GetDeferral` exists on these event arguments is from the API reference, and that
        // the stream needs it is unknown; failure to get one is not fatal).
        let deferral = args.GetDeferral().ok();
        let played = self.player.play(&source);
        if let Some(d) = deferral {
            let _ = d.Complete();
        }
        match played {
            Ok(()) => self.engine.stream_ready(id, StreamHandle { source }),
            Err(e) => {
                self.engine.emit(MiracastEvent::ConnectionFailed {
                    id: Some(id),
                    message: format!("could not play the stream: {e}"),
                });
                self.engine.disconnect_active();
            }
        }
        Ok(())
    }

    fn on_disconnected(
        &self,
        args: &MiracastReceiverDisconnectedEventArgs,
    ) -> windows::core::Result<()> {
        let connection = WinConn(args.Connection()?);
        if self.engine.on_disconnected(&connection).is_some() {
            self.player.stop();
        }
        Ok(())
    }

    fn on_status_changed(&self, receiver: &WinReceiver) -> windows::core::Result<()> {
        if self.engine.is_closed() {
            return Ok(());
        }
        let st = receiver.GetStatus()?;
        let mapped = if st.WiFiStatus()? == MiracastReceiverWiFiStatus::MiracastNotSupported {
            MiracastStatus::Unsupported
        } else {
            map_listening(st.ListeningStatus()?)
        };
        self.engine.emit(MiracastEvent::StatusChanged(mapped));
        Ok(())
    }
}

fn map_listening(s: MiracastReceiverListeningStatus) -> MiracastStatus {
    match s {
        MiracastReceiverListeningStatus::Listening => MiracastStatus::Listening,
        MiracastReceiverListeningStatus::ConnectionPending => MiracastStatus::ConnectionPending,
        MiracastReceiverListeningStatus::Connected => MiracastStatus::Connected,
        MiracastReceiverListeningStatus::DisabledByPolicy => MiracastStatus::DisabledByPolicy,
        MiracastReceiverListeningStatus::TemporarilyDisabled => MiracastStatus::Unavailable,
        _ => MiracastStatus::Off,
    }
}

/// Is a file that the optional feature "Wireless Display" installs present?
// UNCERTAIN: the file name is written from memory (the "Connect" app of the feature,
// `WirelessDisplay.exe` in System32). A missing file proves nothing, so only presence is
// reported; absence gives `None` ("could not tell"), never "not installed".
fn feature_marker() -> Option<bool> {
    let root = std::env::var_os("SystemRoot")?;
    let path = std::path::Path::new(&root)
        .join("System32")
        .join("WirelessDisplay.exe");
    path.is_file().then_some(true)
}

/// Look at the system without starting a session. Needs no consent and shows nothing.
pub(crate) fn probe() -> Probe {
    init_winrt();
    let mut p = Probe {
        feature: FeatureState::from_marker(feature_marker()),
        ..Probe::default()
    };
    if let Ok(r) = WinReceiver::new() {
        p.receiver_available = true;
        if let Ok(st) = r.GetStatus() {
            if let Ok(w) = st.WiFiStatus() {
                p.adapter = match w {
                    MiracastReceiverWiFiStatus::MiracastNotSupported => {
                        AdapterSupport::NotSupported
                    }
                    MiracastReceiverWiFiStatus::MiracastSupportNotOptimized => {
                        AdapterSupport::NotOptimized
                    }
                    MiracastReceiverWiFiStatus::MiracastSupported => AdapterSupport::Supported,
                    _ => AdapterSupport::Undetermined,
                };
            }
            p.listening = st.ListeningStatus().ok().map(map_listening);
        }
    }
    p
}

/// Registered handler tokens (needed to unsubscribe). Filled in as handlers are registered,
/// so `Drop` removes exactly what exists, also after a failed start.
#[derive(Default)]
struct Tokens {
    status: Option<i64>,
    created: Option<i64>,
    media: Option<i64>,
    disconnected: Option<i64>,
}

/// The settings Windows had before we changed them, restored on drop (best effort).
struct SavedSettings {
    friendly_name: HSTRING,
    authorization: MiracastReceiverAuthorizationMethod,
    require_known: bool,
}

pub(crate) struct Backend {
    shared: Arc<Shared>,
    mailbox: Arc<FrameMailbox>,
    receiver: WinReceiver,
    session: Option<MiracastReceiverSession>,
    tokens: Tokens,
    saved: Option<SavedSettings>,
}

impl Backend {
    pub(crate) fn start(
        config: &MiracastConfig,
    ) -> Result<(Self, Receiver<MiracastEvent>), MiracastError> {
        init_winrt();
        let (engine, rx) = Engine::<WinConn>::new(config.max_sessions);
        let mailbox = Arc::new(FrameMailbox::default());
        let failed_engine = Arc::clone(&engine);
        let player = Player::new(Arc::clone(&mailbox), move |text| {
            failed_engine.emit(MiracastEvent::ConnectionFailed {
                id: None,
                message: format!("playback failed: {text}"),
            });
        })
        .map_err(werr("MediaPlayer"))?;
        let shared = Arc::new(Shared {
            engine: Arc::clone(&engine),
            player,
        });

        let receiver = WinReceiver::new().map_err(werr("MiracastReceiver activation"))?;
        // From here on `Drop` cleans up, also on every error path below.
        let mut backend = Self {
            shared: Arc::clone(&shared),
            mailbox,
            receiver,
            session: None,
            tokens: Tokens::default(),
            saved: None,
        };
        let receiver = &backend.receiver;

        // Settings: start from what Windows has now, else from the defaults.
        let settings = receiver
            .GetCurrentSettings()
            .or_else(|_| receiver.GetDefaultSettings())
            .map_err(werr("GetCurrentSettings/GetDefaultSettings"))?;
        let saved = SavedSettings {
            friendly_name: settings.FriendlyName().unwrap_or_default(),
            authorization: settings
                .AuthorizationMethod()
                .unwrap_or(MiracastReceiverAuthorizationMethod::ConfirmConnection),
            require_known: settings
                .RequireAuthorizationFromKnownTransmitters()
                .unwrap_or(true),
        };
        settings
            .SetFriendlyName(&HSTRING::from(config.display_name.trim()))
            .map_err(werr("SetFriendlyName"))?;
        settings
            .SetAuthorizationMethod(match config.pairing {
                PairingPolicy::ConfirmConnection => {
                    MiracastReceiverAuthorizationMethod::ConfirmConnection
                }
                PairingPolicy::PinIfRequested => {
                    MiracastReceiverAuthorizationMethod::PinDisplayIfRequested
                }
                PairingPolicy::RequirePin => {
                    MiracastReceiverAuthorizationMethod::PinDisplayRequired
                }
            })
            .map_err(werr("SetAuthorizationMethod"))?;
        // UNCERTAIN: we assume this makes Windows ask again even for known devices.
        settings
            .SetRequireAuthorizationFromKnownTransmitters(true)
            .map_err(werr("SetRequireAuthorizationFromKnownTransmitters"))?;
        // UNCERTAIN: this disconnects Miracast sessions of OTHER apps machine-wide and the new
        // settings may persist after we exit (docs/miracast.md). We remember the old values
        // and try to put them back on drop.
        let applied = receiver
            .DisconnectAllAndApplySettings(&settings)
            .map_err(werr("DisconnectAllAndApplySettings"))?;
        backend.saved = Some(saved);
        let status = applied.Status().map_err(werr("ApplySettings status"))?;
        if status != MiracastReceiverApplySettingsStatus::Success {
            return Err(match status {
                MiracastReceiverApplySettingsStatus::MiracastNotSupported => {
                    MiracastError::Unsupported
                }
                other => MiracastError::Windows(format!(
                    "applying settings failed: status {}, error {:?}",
                    other.0,
                    applied.ExtendedError()
                )),
            });
        }

        let s = Arc::clone(&shared);
        backend.tokens.status = Some(
            backend
                .receiver
                .StatusChanged(&TypedEventHandler::<WinReceiver, IInspectable>::new(
                    move |sender, _| {
                        if let Some(r) = sender.as_ref() {
                            s.on_status_changed(r)?;
                        }
                        Ok(())
                    },
                ))
                .map_err(werr("StatusChanged"))?,
        );

        // The plan says `CreateSession(None)`: no CoreApplicationView (unpackaged app).
        // UNCERTAIN: whether this is accepted outside a packaged app is assumption A4.
        let session = backend
            .receiver
            .CreateSession(None::<&CoreApplicationView>)
            .map_err(werr("CreateSession"))?;
        backend.session = Some(session.clone());
        session
            .SetAllowConnectionTakeover(false)
            .map_err(werr("SetAllowConnectionTakeover"))?;
        session
            .SetMaxSimultaneousConnections(1)
            .map_err(werr("SetMaxSimultaneousConnections"))?;

        let s = Arc::clone(&shared);
        backend.tokens.created = Some(
            session
                .ConnectionCreated(&TypedEventHandler::<
                    MiracastReceiverSession,
                    MiracastReceiverConnectionCreatedEventArgs,
                >::new(move |_, args| {
                    s.on_connection_created(args.ok()?)
                }))
                .map_err(werr("ConnectionCreated"))?,
        );
        let s = Arc::clone(&shared);
        backend.tokens.media = Some(
            session
                .MediaSourceCreated(&TypedEventHandler::<
                    MiracastReceiverSession,
                    MiracastReceiverMediaSourceCreatedEventArgs,
                >::new(move |_, args| {
                    s.on_media_source_created(args.ok()?)
                }))
                .map_err(werr("MediaSourceCreated"))?,
        );
        let s = Arc::clone(&shared);
        backend.tokens.disconnected = Some(
            session
                .Disconnected(&TypedEventHandler::<
                    MiracastReceiverSession,
                    MiracastReceiverDisconnectedEventArgs,
                >::new(move |_, args| {
                    s.on_disconnected(args.ok()?)
                }))
                .map_err(werr("Disconnected"))?,
        );

        // One timer thread for all confirmation timeouts. It ends when `Drop` closes the queue.
        engine.spawn_timer();

        let started = session.Start().map_err(werr("Session.Start"))?;
        match started.Status().map_err(werr("Start status"))? {
            MiracastReceiverSessionStartStatus::Success => {}
            MiracastReceiverSessionStartStatus::MiracastNotSupported => {
                return Err(MiracastError::Unsupported);
            }
            other => {
                return Err(MiracastError::Windows(format!(
                    "starting the session failed: status {}, error {:?}",
                    other.0,
                    started.ExtendedError()
                )));
            }
        }
        engine.emit(MiracastEvent::Listening);
        Ok((backend, rx))
    }

    pub(crate) fn respond(&self, id: ConnectionId, accept: bool) -> Result<(), MiracastError> {
        self.shared.engine.answer(id, accept)
    }

    pub(crate) fn disconnect(&self) {
        if self.shared.engine.disconnect_active() > 0 {
            self.shared.player.stop();
        }
    }

    pub(crate) fn status(&self) -> MiracastStatus {
        self.shared.engine.status()
    }

    pub(crate) fn take_frame(&self) -> Option<VideoFrame> {
        self.mailbox.take()
    }

    pub(crate) fn peek_frame_size(&self) -> Option<(u32, u32)> {
        self.mailbox.peek_size()
    }

    pub(crate) fn frame_counters(&self) -> FrameCounters {
        self.mailbox.counters()
    }

    pub(crate) fn set_max_frame_size(&self, w: u32, h: u32) {
        self.shared.player.set_max_size(w, h);
    }

    pub(crate) fn set_muted(&self, muted: bool) {
        self.shared.player.set_muted(muted);
    }
}

impl Drop for Backend {
    fn drop(&mut self) {
        // Unsubscribe first so no handler runs while we tear down.
        if let Some(t) = self.tokens.status.take() {
            let _ = self.receiver.RemoveStatusChanged(t);
        }
        if let Some(session) = &self.session {
            if let Some(t) = self.tokens.created.take() {
                let _ = session.RemoveConnectionCreated(t);
            }
            if let Some(t) = self.tokens.media.take() {
                let _ = session.RemoveMediaSourceCreated(t);
            }
            if let Some(t) = self.tokens.disconnected.take() {
                let _ = session.RemoveDisconnected(t);
            }
        }
        // Handlers that are already running see `closed` and do nothing; the timer ends;
        // every connection is ended.
        self.shared.engine.shutdown();
        self.shared.player.stop();
        // UNCERTAIN: Close() is assumed to stop listening and release the Wi-Fi Direct role.
        if let Some(session) = self.session.take() {
            let _ = session.Close();
        }
        // UNCERTAIN: put back the settings Windows had before (best effort). This again
        // disconnects every Miracast session on the machine, and whether the values we set
        // persist beyond our process was not measured.
        if let Some(saved) = self.saved.take() {
            let restore = || -> windows::core::Result<()> {
                let settings = self.receiver.GetCurrentSettings()?;
                settings.SetFriendlyName(&saved.friendly_name)?;
                settings.SetAuthorizationMethod(saved.authorization)?;
                settings.SetRequireAuthorizationFromKnownTransmitters(saved.require_known)?;
                self.receiver.DisconnectAllAndApplySettings(&settings)?;
                Ok(())
            };
            let _ = restore();
        }
        // The engine is closed, so `emit` here only updates the status for `status()`.
        self.shared
            .engine
            .emit(MiracastEvent::StatusChanged(MiracastStatus::Off));
    }
}
