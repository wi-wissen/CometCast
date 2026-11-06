// SPDX-License-Identifier: GPL-3.0-or-later
//! Windows backend: `Windows.Media.Miracast.MiracastReceiver` through the `windows` crate.
//!
//! COMPILE-CHECKED ONLY. This code has never been run. Every comment about what Windows does
//! at runtime is an assumption (UNCERTAIN) until `docs/miracast.md` section "Test checklist"
//! has been done on a real Windows 11 machine.
//!
//! Only the documented public API is used. No undocumented DLL (`WiFiDisplay.dll` and
//! friends) is touched.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

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

use crate::deadline::DeadlineQueue;
use crate::{
    CONFIRM_TIMEOUT, ConnectionId, ConnectionPolicy, Decision, MiracastConfig, MiracastError,
    MiracastEvent, MiracastStatus, PairingPolicy, RejectLimiter, RejectReason, StreamHandle,
    sanitize_device_name, status_after,
};

/// Capacity of the event channel. When the consumer is slower than this, further events are
/// dropped (the status in `Shared` is still updated) instead of growing without bound.
const EVENT_QUEUE: usize = 64;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

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

struct Pending {
    connection: MiracastReceiverConnection,
    deferral: Option<Deferral>,
    name: String,
}

struct Active {
    connection: MiracastReceiverConnection,
}

#[derive(Default)]
struct State {
    next_id: u64,
    pending: HashMap<u64, Pending>,
    active: HashMap<u64, Active>,
}

struct Shared {
    tx: SyncSender<MiracastEvent>,
    status: Mutex<MiracastStatus>,
    state: Mutex<State>,
    max_sessions: usize,
    /// Set first thing in `Drop`; handlers that still run afterwards do nothing.
    closed: AtomicBool,
    /// Deadlines of the pending confirmations (one timer thread for all).
    deadlines: DeadlineQueue,
    reject_limiter: Mutex<RejectLimiter>,
}

impl Shared {
    fn emit(&self, event: MiracastEvent) {
        {
            let mut s = lock(&self.status);
            *s = status_after(*s, &event);
        }
        // The receiving side may be gone, or the queue full (consumer stuck): drop the event.
        if let Err(TrySendError::Full(_)) = self.tx.try_send(event) {
            // Nothing sensible to do; a pending request is rejected by its timeout anyway.
        }
    }

    /// Report a rejected connection, at most once per interval and reason.
    fn report_rejection(&self, reason: RejectReason, device_name: Option<String>) {
        if lock(&self.reject_limiter).allow(reason, Instant::now()) {
            self.emit(MiracastEvent::ConnectionRejected {
                reason,
                device_name,
            });
        }
    }

    /// Accept or reject a pending connection.
    fn answer(&self, id: ConnectionId, accept: bool) -> Result<(), MiracastError> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(MiracastError::UnknownConnection);
        }
        // pending -> active under ONE guard, so the number of sessions in use never drops
        // to zero in between (a second sender could otherwise slip in).
        let pending = {
            let mut st = lock(&self.state);
            let pending = st
                .pending
                .remove(&id.0)
                .ok_or(MiracastError::UnknownConnection)?;
            if accept {
                st.active.insert(
                    id.0,
                    Active {
                        connection: pending.connection.clone(),
                    },
                );
            }
            pending
        };
        self.deadlines.cancel(id.0);
        if accept {
            if let Some(d) = &pending.deferral
                && let Err(e) = d.Complete()
            {
                lock(&self.state).active.remove(&id.0);
                let _ = pending
                    .connection
                    .Disconnect(MiracastReceiverDisconnectReason::ConnectionNotAccepted);
                self.emit(MiracastEvent::ConnectionFailed {
                    id: Some(id),
                    message: format!("could not complete the confirmation: {e}"),
                });
                self.emit(MiracastEvent::Disconnected { id });
                return Err(MiracastError::Windows(format!("Deferral.Complete: {e}")));
            }
            self.emit(MiracastEvent::Connected {
                id,
                device_name: pending.name,
            });
        } else {
            let r = pending
                .connection
                .Disconnect(MiracastReceiverDisconnectReason::ConnectionNotAccepted)
                .map_err(werr("Disconnect"));
            if let Some(d) = &pending.deferral {
                // UNCERTAIN: completing the deferral after Disconnect is what we assume the
                // API expects; not verified.
                let _ = d.Complete();
            }
            self.emit(MiracastEvent::Disconnected { id });
            r?;
        }
        Ok(())
    }

    fn find_id(&self, connection: &MiracastReceiverConnection) -> Option<(ConnectionId, bool)> {
        let st = lock(&self.state);
        if let Some((id, _)) = st.active.iter().find(|(_, a)| &a.connection == connection) {
            return Some((ConnectionId(*id), true));
        }
        st.pending
            .iter()
            .find(|(_, p)| &p.connection == connection)
            .map(|(id, _)| (ConnectionId(*id), false))
    }

    fn on_connection_created(
        self: &Arc<Self>,
        args: &MiracastReceiverConnectionCreatedEventArgs,
    ) -> windows::core::Result<()> {
        let connection = args.Connection()?;
        if self.closed.load(Ordering::SeqCst) {
            let _ = connection.Disconnect(MiracastReceiverDisconnectReason::ConnectionNotAccepted);
            return Ok(());
        }
        let raw_name = connection
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

        let mut st = lock(&self.state);
        let in_use = st.pending.len() + st.active.len();
        match ConnectionPolicy::evaluate(in_use, self.max_sessions, &raw_name) {
            Decision::Reject(reason) => {
                drop(st);
                let _ =
                    connection.Disconnect(MiracastReceiverDisconnectReason::ConnectionNotAccepted);
                self.report_rejection(reason, sanitize_device_name(&raw_name));
            }
            Decision::AskPerson { device_name: name } => {
                // Hold the connection open until a person answers (or the timeout hits).
                // Without a deferral Windows might finish the connection by itself, so a
                // failure here must reject (fail closed).
                let deferral = match args.GetDeferral() {
                    Ok(d) => d,
                    Err(e) => {
                        drop(st);
                        let _ = connection
                            .Disconnect(MiracastReceiverDisconnectReason::ConnectionNotAccepted);
                        self.report_rejection(
                            RejectReason::ConfirmationUnavailable,
                            Some(name.clone()),
                        );
                        self.emit(MiracastEvent::ConnectionFailed {
                            id: None,
                            message: format!("no deferral for {name:?}, connection refused: {e}"),
                        });
                        return Ok(());
                    }
                };
                st.next_id += 1;
                let id = ConnectionId(st.next_id);
                st.pending.insert(
                    id.0,
                    Pending {
                        connection,
                        deferral: Some(deferral),
                        name: name.clone(),
                    },
                );
                drop(st);
                self.deadlines.add(id.0, Instant::now() + CONFIRM_TIMEOUT);
                self.emit(MiracastEvent::ConnectionRequested {
                    id,
                    device_name: name,
                    pin,
                });
            }
        }
        Ok(())
    }

    fn on_media_source_created(
        &self,
        args: &MiracastReceiverMediaSourceCreatedEventArgs,
    ) -> windows::core::Result<()> {
        let connection = args.Connection()?;
        if self.closed.load(Ordering::SeqCst) {
            return Ok(());
        }
        let source = args.MediaSource()?;
        match self.find_id(&connection) {
            Some((id, true)) => {
                // UNCERTAIN: we do not take a Deferral here. If Windows needs the app to
                // attach the source to a player before the handler returns, the stream may
                // not start; nothing renders it yet anyway.
                self.emit(MiracastEvent::StreamReady {
                    id,
                    stream: StreamHandle { source },
                });
            }
            _ => {
                // A stream for a connection nobody accepted: refuse it.
                let _ =
                    connection.Disconnect(MiracastReceiverDisconnectReason::ConnectionNotAccepted);
                self.emit(MiracastEvent::ConnectionFailed {
                    id: None,
                    message: "media source for an unconfirmed connection ignored".into(),
                });
            }
        }
        Ok(())
    }

    fn on_disconnected(
        &self,
        args: &MiracastReceiverDisconnectedEventArgs,
    ) -> windows::core::Result<()> {
        let connection = args.Connection()?;
        if self.closed.load(Ordering::SeqCst) {
            return Ok(());
        }
        if let Some((id, _)) = self.find_id(&connection) {
            let pending = {
                let mut st = lock(&self.state);
                st.active.remove(&id.0);
                st.pending.remove(&id.0)
            };
            self.deadlines.cancel(id.0);
            if let Some(d) = pending.and_then(|p| p.deferral) {
                let _ = d.Complete();
            }
            self.emit(MiracastEvent::Disconnected { id });
        }
        Ok(())
    }

    fn on_status_changed(&self, receiver: &WinReceiver) -> windows::core::Result<()> {
        if self.closed.load(Ordering::SeqCst) {
            return Ok(());
        }
        let st = receiver.GetStatus()?;
        let mapped = if st.WiFiStatus()? == MiracastReceiverWiFiStatus::MiracastNotSupported {
            MiracastStatus::Unsupported
        } else {
            match st.ListeningStatus()? {
                MiracastReceiverListeningStatus::Listening => MiracastStatus::Listening,
                MiracastReceiverListeningStatus::ConnectionPending => {
                    MiracastStatus::ConnectionPending
                }
                MiracastReceiverListeningStatus::Connected => MiracastStatus::Connected,
                MiracastReceiverListeningStatus::DisabledByPolicy
                | MiracastReceiverListeningStatus::TemporarilyDisabled => {
                    MiracastStatus::Unavailable
                }
                _ => MiracastStatus::Off,
            }
        };
        self.emit(MiracastEvent::StatusChanged(mapped));
        Ok(())
    }
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
        let (tx, rx) = sync_channel(EVENT_QUEUE);
        let shared = Arc::new(Shared {
            tx,
            status: Mutex::new(MiracastStatus::Starting),
            state: Mutex::new(State::default()),
            max_sessions: config.max_sessions,
            closed: AtomicBool::new(false),
            deadlines: DeadlineQueue::default(),
            reject_limiter: Mutex::new(RejectLimiter::default()),
        });

        let receiver = WinReceiver::new().map_err(werr("MiracastReceiver activation"))?;
        // From here on `Drop` cleans up, also on every error path below.
        let mut backend = Self {
            shared: Arc::clone(&shared),
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
        let timer = Arc::clone(&shared);
        std::thread::spawn(move || {
            while let Some(id) = timer.deadlines.wait_next() {
                // Already answered is the normal case: the error is ignored.
                let _ = timer.answer(ConnectionId(id), false);
            }
        });

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
        shared.emit(MiracastEvent::Listening);
        Ok((backend, rx))
    }

    pub(crate) fn respond(&self, id: ConnectionId, accept: bool) -> Result<(), MiracastError> {
        self.shared.answer(id, accept)
    }

    pub(crate) fn status(&self) -> MiracastStatus {
        *lock(&self.shared.status)
    }
}

impl Drop for Backend {
    fn drop(&mut self) {
        // Handlers that are already running see this and do nothing; the timer thread ends.
        self.shared.closed.store(true, Ordering::SeqCst);
        self.shared.deadlines.close();
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

        let (pending, active) = {
            let mut st = lock(&self.shared.state);
            (
                std::mem::take(&mut st.pending),
                std::mem::take(&mut st.active),
            )
        };
        for p in pending.into_values() {
            let _ = p
                .connection
                .Disconnect(MiracastReceiverDisconnectReason::ConnectionNotAccepted);
            if let Some(d) = p.deferral {
                let _ = d.Complete();
            }
        }
        for a in active.into_values() {
            let _ = a
                .connection
                .Disconnect(MiracastReceiverDisconnectReason::Finished);
        }
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
        self.shared
            .emit(MiracastEvent::StatusChanged(MiracastStatus::Off));
    }
}
