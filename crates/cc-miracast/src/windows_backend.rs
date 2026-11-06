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
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

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

use crate::{
    CONFIRM_TIMEOUT, ConnectionId, ConnectionPolicy, Decision, MiracastConfig, MiracastError,
    MiracastEvent, MiracastStatus, PairingPolicy, StreamHandle, status_after,
};

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
    tx: Mutex<Sender<MiracastEvent>>,
    status: Mutex<MiracastStatus>,
    state: Mutex<State>,
    max_sessions: usize,
}

impl Shared {
    fn emit(&self, event: MiracastEvent) {
        {
            let mut s = lock(&self.status);
            *s = status_after(*s, &event);
        }
        // The receiving side may be gone already; nothing to do then.
        let _ = lock(&self.tx).send(event);
    }

    /// Accept or reject a pending connection.
    fn answer(&self, id: ConnectionId, accept: bool) -> Result<(), MiracastError> {
        let pending = lock(&self.state)
            .pending
            .remove(&id.0)
            .ok_or(MiracastError::UnknownConnection)?;
        if accept {
            lock(&self.state).active.insert(
                id.0,
                Active {
                    connection: pending.connection.clone(),
                },
            );
            if let Some(d) = &pending.deferral {
                d.Complete().map_err(werr("Deferral.Complete"))?;
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
        let name = connection
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
        match ConnectionPolicy::evaluate(in_use, self.max_sessions, &name) {
            Decision::Reject(reason) => {
                drop(st);
                let _ =
                    connection.Disconnect(MiracastReceiverDisconnectReason::ConnectionNotAccepted);
                self.emit(MiracastEvent::Error(format!(
                    "connection from {name:?} rejected: {reason:?}"
                )));
            }
            Decision::AskPerson => {
                // Hold the connection open until a person answers (or the timeout hits).
                let deferral = args.GetDeferral().ok();
                st.next_id += 1;
                let id = ConnectionId(st.next_id);
                st.pending.insert(
                    id.0,
                    Pending {
                        connection,
                        deferral,
                        name: name.clone(),
                    },
                );
                drop(st);
                self.emit(MiracastEvent::ConnectionRequested {
                    id,
                    device_name: name,
                    pin,
                });
                let shared = Arc::clone(self);
                std::thread::spawn(move || {
                    std::thread::sleep(CONFIRM_TIMEOUT);
                    // Already answered is the normal case: the error is ignored.
                    let _ = shared.answer(id, false);
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
                self.emit(MiracastEvent::Error(
                    "media source for an unconfirmed connection ignored".into(),
                ));
            }
        }
        Ok(())
    }

    fn on_disconnected(
        &self,
        args: &MiracastReceiverDisconnectedEventArgs,
    ) -> windows::core::Result<()> {
        let connection = args.Connection()?;
        if let Some((id, _)) = self.find_id(&connection) {
            let pending = {
                let mut st = lock(&self.state);
                st.active.remove(&id.0);
                st.pending.remove(&id.0)
            };
            if let Some(d) = pending.and_then(|p| p.deferral) {
                let _ = d.Complete();
            }
            self.emit(MiracastEvent::Disconnected { id });
        }
        Ok(())
    }

    fn on_status_changed(&self, receiver: &WinReceiver) -> windows::core::Result<()> {
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

/// Registered handler tokens (needed to unsubscribe).
struct Tokens {
    status: i64,
    created: i64,
    media: i64,
    disconnected: i64,
}

pub(crate) struct Backend {
    shared: Arc<Shared>,
    receiver: WinReceiver,
    session: MiracastReceiverSession,
    tokens: Tokens,
}

impl Backend {
    pub(crate) fn start(
        config: &MiracastConfig,
    ) -> Result<(Self, Receiver<MiracastEvent>), MiracastError> {
        init_winrt();
        let (tx, rx) = channel();
        let shared = Arc::new(Shared {
            tx: Mutex::new(tx),
            status: Mutex::new(MiracastStatus::Starting),
            state: Mutex::new(State::default()),
            max_sessions: config.max_sessions,
        });

        let receiver = WinReceiver::new().map_err(werr("MiracastReceiver activation"))?;

        // Settings: start from what Windows has now, else from the defaults.
        let settings = receiver
            .GetCurrentSettings()
            .or_else(|_| receiver.GetDefaultSettings())
            .map_err(werr("GetCurrentSettings/GetDefaultSettings"))?;
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
        let applied = receiver
            .DisconnectAllAndApplySettings(&settings)
            .map_err(werr("DisconnectAllAndApplySettings"))?;
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
        let status_token = receiver
            .StatusChanged(&TypedEventHandler::<WinReceiver, IInspectable>::new(
                move |sender, _| {
                    if let Some(r) = sender.as_ref() {
                        s.on_status_changed(r)?;
                    }
                    Ok(())
                },
            ))
            .map_err(werr("StatusChanged"))?;

        // The plan says `CreateSession(None)`: no CoreApplicationView (unpackaged app).
        // UNCERTAIN: whether this is accepted outside a packaged app is assumption A4.
        let session = receiver
            .CreateSession(None::<&CoreApplicationView>)
            .map_err(werr("CreateSession"))?;
        session
            .SetAllowConnectionTakeover(false)
            .map_err(werr("SetAllowConnectionTakeover"))?;
        session
            .SetMaxSimultaneousConnections(1)
            .map_err(werr("SetMaxSimultaneousConnections"))?;

        let s = Arc::clone(&shared);
        let created = session
            .ConnectionCreated(&TypedEventHandler::<
                MiracastReceiverSession,
                MiracastReceiverConnectionCreatedEventArgs,
            >::new(move |_, args| {
                s.on_connection_created(args.ok()?)
            }))
            .map_err(werr("ConnectionCreated"))?;
        let s = Arc::clone(&shared);
        let media = session
            .MediaSourceCreated(&TypedEventHandler::<
                MiracastReceiverSession,
                MiracastReceiverMediaSourceCreatedEventArgs,
            >::new(move |_, args| {
                s.on_media_source_created(args.ok()?)
            }))
            .map_err(werr("MediaSourceCreated"))?;
        let s = Arc::clone(&shared);
        let disconnected = session
            .Disconnected(&TypedEventHandler::<
                MiracastReceiverSession,
                MiracastReceiverDisconnectedEventArgs,
            >::new(move |_, args| {
                s.on_disconnected(args.ok()?)
            }))
            .map_err(werr("Disconnected"))?;

        let tokens = Tokens {
            status: status_token,
            created,
            media,
            disconnected,
        };
        // From here on `Drop` cleans up, also on the error paths below.
        let backend = Self {
            shared: Arc::clone(&shared),
            receiver,
            session,
            tokens,
        };

        let started = backend.session.Start().map_err(werr("Session.Start"))?;
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
        // Unsubscribe first so no handler runs while we tear down.
        let _ = self.receiver.RemoveStatusChanged(self.tokens.status);
        let _ = self.session.RemoveConnectionCreated(self.tokens.created);
        let _ = self.session.RemoveMediaSourceCreated(self.tokens.media);
        let _ = self.session.RemoveDisconnected(self.tokens.disconnected);

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
        let _ = self.session.Close();
        self.shared
            .emit(MiracastEvent::StatusChanged(MiracastStatus::Off));
    }
}
