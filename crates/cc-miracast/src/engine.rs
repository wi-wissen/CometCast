// SPDX-License-Identifier: GPL-3.0-or-later
//! The platform independent core of the receiver: which connections are waiting or active,
//! how a request is answered or times out, which status and events follow.
//!
//! The Windows backend only translates Windows events into calls on [`Engine`] and the
//! answers back into Windows calls. Everything Windows-specific is behind the [`Conn`]
//! trait, so this logic is tested on Linux with a fake connection.

// Only the Windows backend uses the engine outside tests.
#![cfg_attr(not(windows), allow(dead_code))]

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

use crate::deadline::DeadlineQueue;
use crate::{
    CONFIRM_TIMEOUT, ConnectionId, ConnectionPolicy, Decision, MiracastError, MiracastEvent,
    MiracastStatus, RejectLimiter, RejectReason, StreamHandle, sanitize_device_name, status_after,
};

/// Capacity of the event channel. When the consumer is slower than this, further events are
/// dropped (the status is still updated) instead of growing without bound.
pub(crate) const EVENT_QUEUE: usize = 64;

/// Why we end a connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DisconnectKind {
    /// Refused, timed out or never confirmed (Windows `ConnectionNotAccepted`).
    NotAccepted,
    /// Ended on purpose after it was active (Windows `Finished`).
    Finished,
}

/// One connection as the engine sees it (on Windows: a `MiracastReceiverConnection` and its
/// `Deferral`).
pub(crate) trait Conn: Clone + PartialEq + Send + Sync + 'static {
    /// What keeps the connection waiting for the person's answer (Windows `Deferral`).
    type Hold: Send + 'static;
    /// End the connection.
    fn disconnect(&self, kind: DisconnectKind) -> Result<(), String>;
    /// Let the connection go on (the person said yes) or release the wait.
    fn complete(hold: &Self::Hold) -> Result<(), String>;
}

struct Pending<C: Conn> {
    connection: C,
    hold: Option<C::Hold>,
    name: String,
}

struct State<C: Conn> {
    next_id: u64,
    pending: HashMap<u64, Pending<C>>,
    active: HashMap<u64, C>,
}

impl<C: Conn> Default for State<C> {
    fn default() -> Self {
        Self {
            next_id: 0,
            pending: HashMap::new(),
            active: HashMap::new(),
        }
    }
}

/// All state of one running receiver.
pub(crate) struct Engine<C: Conn> {
    tx: SyncSender<MiracastEvent>,
    status: Mutex<MiracastStatus>,
    state: Mutex<State<C>>,
    max_sessions: usize,
    /// Set first thing when shutting down; handlers that still run afterwards do nothing.
    closed: AtomicBool,
    /// Deadlines of the pending confirmations (one timer thread for all).
    pub(crate) deadlines: DeadlineQueue,
    reject_limiter: Mutex<RejectLimiter>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl<C: Conn> Engine<C> {
    /// A new engine in status `Starting` and the channel its events arrive on.
    pub(crate) fn new(max_sessions: usize) -> (Arc<Self>, Receiver<MiracastEvent>) {
        let (tx, rx) = sync_channel(EVENT_QUEUE);
        (
            Arc::new(Self {
                tx,
                status: Mutex::new(MiracastStatus::Starting),
                state: Mutex::new(State::default()),
                max_sessions,
                closed: AtomicBool::new(false),
                deadlines: DeadlineQueue::default(),
                reject_limiter: Mutex::new(RejectLimiter::default()),
            }),
            rx,
        )
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    pub(crate) fn status(&self) -> MiracastStatus {
        *lock(&self.status)
    }

    /// Update the status and deliver the event.
    pub(crate) fn emit(&self, event: MiracastEvent) {
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

    /// A foreign device wants to connect.
    ///
    /// `take_hold` is called only when the person is to be asked; it must produce what keeps
    /// the connection waiting (Windows `Deferral`). If it fails the connection is refused
    /// (fail closed): without a hold Windows might finish the connection by itself.
    pub(crate) fn on_connection_created(
        &self,
        connection: &C,
        raw_name: &str,
        pin: Option<String>,
        take_hold: impl FnOnce() -> Result<C::Hold, String>,
    ) {
        if self.is_closed() {
            let _ = connection.disconnect(DisconnectKind::NotAccepted);
            return;
        }
        let mut st = lock(&self.state);
        let in_use = st.pending.len() + st.active.len();
        match ConnectionPolicy::evaluate(in_use, self.max_sessions, raw_name) {
            Decision::Reject(reason) => {
                drop(st);
                let _ = connection.disconnect(DisconnectKind::NotAccepted);
                self.report_rejection(reason, sanitize_device_name(raw_name));
            }
            Decision::AskPerson { device_name: name } => {
                let hold = match take_hold() {
                    Ok(h) => h,
                    Err(e) => {
                        drop(st);
                        let _ = connection.disconnect(DisconnectKind::NotAccepted);
                        self.report_rejection(
                            RejectReason::ConfirmationUnavailable,
                            Some(name.clone()),
                        );
                        self.emit(MiracastEvent::ConnectionFailed {
                            id: None,
                            message: format!("no deferral for {name:?}, connection refused: {e}"),
                        });
                        return;
                    }
                };
                st.next_id += 1;
                let id = ConnectionId(st.next_id);
                st.pending.insert(
                    id.0,
                    Pending {
                        connection: connection.clone(),
                        hold: Some(hold),
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
    }

    /// Accept or reject a pending connection.
    pub(crate) fn answer(&self, id: ConnectionId, accept: bool) -> Result<(), MiracastError> {
        if self.is_closed() {
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
                st.active.insert(id.0, pending.connection.clone());
            }
            pending
        };
        self.deadlines.cancel(id.0);
        if accept {
            if let Some(h) = &pending.hold
                && let Err(e) = C::complete(h)
            {
                lock(&self.state).active.remove(&id.0);
                let _ = pending.connection.disconnect(DisconnectKind::NotAccepted);
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
            Ok(())
        } else {
            let r = pending
                .connection
                .disconnect(DisconnectKind::NotAccepted)
                .map_err(|e| MiracastError::Windows(format!("Disconnect: {e}")));
            if let Some(h) = &pending.hold {
                // UNCERTAIN: completing the deferral after Disconnect is what we assume the
                // API expects; not verified.
                let _ = C::complete(h);
            }
            self.emit(MiracastEvent::Disconnected { id });
            r
        }
    }

    /// The id of an ACCEPTED connection, if `connection` is one.
    pub(crate) fn active_id(&self, connection: &C) -> Option<ConnectionId> {
        lock(&self.state)
            .active
            .iter()
            .find(|(_, c)| *c == connection)
            .map(|(id, _)| ConnectionId(*id))
    }

    /// A stream arrived for a connection nobody accepted: refuse it.
    pub(crate) fn refuse_unconfirmed_stream(&self, connection: &C) {
        let _ = connection.disconnect(DisconnectKind::NotAccepted);
        self.emit(MiracastEvent::ConnectionFailed {
            id: None,
            message: "media source for an unconfirmed connection ignored".into(),
        });
    }

    /// The stream of an accepted connection is playing.
    pub(crate) fn stream_ready(&self, id: ConnectionId, stream: StreamHandle) {
        self.emit(MiracastEvent::StreamReady { id, stream });
    }

    /// Windows says a connection ended.
    ///
    /// Returns the id if it was one of ours (so the backend can stop the player).
    pub(crate) fn on_disconnected(&self, connection: &C) -> Option<ConnectionId> {
        if self.is_closed() {
            return None;
        }
        let (id, pending) = {
            let mut st = lock(&self.state);
            let id = st
                .active
                .iter()
                .find(|(_, c)| *c == connection)
                .map(|(id, _)| *id)
                .or_else(|| {
                    st.pending
                        .iter()
                        .find(|(_, p)| &p.connection == connection)
                        .map(|(id, _)| *id)
                })?;
            st.active.remove(&id);
            (id, st.pending.remove(&id))
        };
        self.deadlines.cancel(id);
        if let Some(h) = pending.and_then(|p| p.hold) {
            let _ = C::complete(&h);
        }
        self.emit(MiracastEvent::Disconnected {
            id: ConnectionId(id),
        });
        Some(ConnectionId(id))
    }

    /// The person pressed "Disconnect": end every active connection (waiting requests stay).
    /// Returns how many were ended.
    pub(crate) fn disconnect_active(&self) -> usize {
        if self.is_closed() {
            return 0;
        }
        let active: Vec<(u64, C)> = lock(&self.state).active.drain().collect();
        for (id, c) in &active {
            let _ = c.disconnect(DisconnectKind::Finished);
            self.deadlines.cancel(*id);
            self.emit(MiracastEvent::Disconnected {
                id: ConnectionId(*id),
            });
        }
        active.len()
    }

    /// Stop everything: no handler does anything afterwards, every connection is ended.
    pub(crate) fn shutdown(&self) {
        self.closed.store(true, Ordering::SeqCst);
        self.deadlines.close();
        let (pending, active) = {
            let mut st = lock(&self.state);
            (
                std::mem::take(&mut st.pending),
                std::mem::take(&mut st.active),
            )
        };
        for p in pending.into_values() {
            let _ = p.connection.disconnect(DisconnectKind::NotAccepted);
            if let Some(h) = p.hold {
                let _ = C::complete(&h);
            }
        }
        for c in active.into_values() {
            let _ = c.disconnect(DisconnectKind::Finished);
        }
    }

    /// Start the one timer thread that rejects requests nobody answered. It ends when
    /// [`Engine::shutdown`] closes the queue.
    pub(crate) fn spawn_timer(self: &Arc<Self>) {
        let timer = Arc::clone(self);
        std::thread::spawn(move || {
            while let Some(id) = timer.deadlines.wait_next() {
                // Already answered is the normal case: the error is ignored.
                let _ = timer.answer(ConnectionId(id), false);
            }
        });
    }
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[derive(Clone, Debug)]
    struct Fake {
        id: u32,
        log: Arc<Mutex<Vec<String>>>,
        fail_complete: bool,
        _live: Arc<AtomicUsize>,
    }
    impl PartialEq for Fake {
        fn eq(&self, o: &Self) -> bool {
            self.id == o.id
        }
    }
    struct FakeHold(Arc<Mutex<Vec<String>>>, bool);
    impl Conn for Fake {
        type Hold = FakeHold;
        fn disconnect(&self, kind: DisconnectKind) -> Result<(), String> {
            self.log
                .lock()
                .unwrap()
                .push(format!("disconnect {} {kind:?}", self.id));
            Ok(())
        }
        fn complete(h: &FakeHold) -> Result<(), String> {
            h.0.lock().unwrap().push("complete".into());
            if h.1 { Err("boom".into()) } else { Ok(()) }
        }
    }

    fn fake(id: u32, log: &Arc<Mutex<Vec<String>>>) -> Fake {
        Fake {
            id,
            log: Arc::clone(log),
            fail_complete: false,
            _live: Arc::default(),
        }
    }
    fn hold(f: &Fake) -> Result<FakeHold, String> {
        Ok(FakeHold(Arc::clone(&f.log), f.fail_complete))
    }
    fn events(rx: &Receiver<MiracastEvent>) -> Vec<MiracastEvent> {
        rx.try_iter().collect()
    }
    fn requested_id(ev: &[MiracastEvent]) -> ConnectionId {
        ev.iter()
            .find_map(|e| match e {
                MiracastEvent::ConnectionRequested { id, .. } => Some(*id),
                _ => None,
            })
            .expect("a connection request")
    }

    #[test]
    fn request_accept_stream_disconnect() {
        let log = Arc::default();
        let (e, rx) = Engine::<Fake>::new(1);
        let c = fake(1, &log);
        e.on_connection_created(&c, "Laptop", Some("1234".into()), || hold(&c));
        let ev = events(&rx);
        let id = requested_id(&ev);
        assert_eq!(e.status(), MiracastStatus::ConnectionPending);
        assert!(
            matches!(&ev[0], MiracastEvent::ConnectionRequested { device_name, pin, .. }
            if device_name == "Laptop" && pin.as_deref() == Some("1234"))
        );
        assert!(e.active_id(&c).is_none(), "not active before the answer");
        e.answer(id, true).unwrap();
        assert_eq!(e.status(), MiracastStatus::Connected);
        assert_eq!(e.active_id(&c), Some(id));
        e.stream_ready(id, StreamHandle {});
        assert_eq!(e.on_disconnected(&c), Some(id));
        assert_eq!(e.status(), MiracastStatus::Listening);
        // A second Disconnected for the same connection does nothing.
        assert_eq!(e.on_disconnected(&c), None);
        assert!(log.lock().unwrap().contains(&"complete".to_string()));
    }

    #[test]
    fn no_accept_without_a_person() {
        // Every path to `active` goes through `answer(id, true)`.
        let log = Arc::default();
        let (e, rx) = Engine::<Fake>::new(1);
        let c = fake(1, &log);
        e.on_connection_created(&c, "Laptop", None, || hold(&c));
        assert!(e.active_id(&c).is_none());
        // A media source for it is refused.
        e.refuse_unconfirmed_stream(&c);
        assert!(
            log.lock()
                .unwrap()
                .iter()
                .any(|l| l.contains("NotAccepted"))
        );
        let _ = events(&rx);
    }

    #[test]
    fn second_sender_is_rejected_and_first_stays() {
        let log = Arc::default();
        let (e, rx) = Engine::<Fake>::new(1);
        let (a, b) = (fake(1, &log), fake(2, &log));
        e.on_connection_created(&a, "A", None, || hold(&a));
        let id = requested_id(&events(&rx));
        e.on_connection_created(&b, "B", None, || hold(&b));
        // Busy while the first one is only waiting.
        let ev = events(&rx);
        assert!(matches!(
            ev[0],
            MiracastEvent::ConnectionRejected {
                reason: RejectReason::Busy,
                ..
            }
        ));
        assert_eq!(e.status(), MiracastStatus::ConnectionPending);
        e.answer(id, true).unwrap();
        e.on_connection_created(&b, "B", None, || hold(&b));
        // Same reason again within 5 s: no second report.
        assert!(
            events(&rx)
                .iter()
                .all(|e| !matches!(e, MiracastEvent::ConnectionRejected { .. }))
        );
        assert_eq!(e.status(), MiracastStatus::Connected);
        assert!(
            log.lock()
                .unwrap()
                .iter()
                .filter(|l| l.starts_with("disconnect 2"))
                .count()
                == 2
        );
    }

    #[test]
    fn rejecting_answers_the_sender_and_returns_to_listening() {
        let log = Arc::default();
        let (e, rx) = Engine::<Fake>::new(1);
        e.emit(MiracastEvent::Listening);
        let c = fake(1, &log);
        e.on_connection_created(&c, "Laptop", None, || hold(&c));
        let id = requested_id(&events(&rx));
        e.answer(id, false).unwrap();
        assert_eq!(e.status(), MiracastStatus::Listening);
        assert!(matches!(
            e.answer(id, true),
            Err(MiracastError::UnknownConnection)
        ));
        assert!(
            log.lock()
                .unwrap()
                .iter()
                .any(|l| l == "disconnect 1 NotAccepted")
        );
    }

    #[test]
    fn missing_hold_fails_closed() {
        let log = Arc::default();
        let (e, rx) = Engine::<Fake>::new(1);
        let c = fake(1, &log);
        e.on_connection_created(&c, "Laptop", None, || Err("no deferral".into()));
        let ev = events(&rx);
        assert!(ev.iter().any(|x| matches!(
            x,
            MiracastEvent::ConnectionRejected {
                reason: RejectReason::ConfirmationUnavailable,
                ..
            }
        )));
        assert!(
            ev.iter()
                .all(|x| !matches!(x, MiracastEvent::ConnectionRequested { .. }))
        );
        assert!(
            log.lock()
                .unwrap()
                .iter()
                .any(|l| l == "disconnect 1 NotAccepted")
        );
        assert!(e.active_id(&c).is_none());
    }

    #[test]
    fn failing_complete_on_accept_ends_the_connection() {
        let log = Arc::default();
        let (e, rx) = Engine::<Fake>::new(1);
        let mut c = fake(1, &log);
        c.fail_complete = true;
        e.on_connection_created(&c, "Laptop", None, || hold(&c));
        let id = requested_id(&events(&rx));
        assert!(e.answer(id, true).is_err());
        assert!(e.active_id(&c).is_none());
        let ev = events(&rx);
        assert!(
            ev.iter()
                .any(|x| matches!(x, MiracastEvent::ConnectionFailed { .. }))
        );
        assert!(
            ev.iter()
                .any(|x| matches!(x, MiracastEvent::Disconnected { .. }))
        );
    }

    #[test]
    fn timeout_rejects_through_the_timer() {
        let log: Arc<Mutex<Vec<String>>> = Arc::default();
        let (e, rx) = Engine::<Fake>::new(1);
        e.spawn_timer();
        let c = fake(1, &log);
        e.on_connection_created(&c, "Laptop", None, || hold(&c));
        let id = requested_id(&events(&rx));
        // Force the deadline now instead of waiting 30 s.
        e.deadlines.add(id.0, Instant::now());
        for _ in 0..100 {
            if log
                .lock()
                .unwrap()
                .iter()
                .any(|l| l == "disconnect 1 NotAccepted")
            {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(
            log.lock()
                .unwrap()
                .iter()
                .any(|l| l == "disconnect 1 NotAccepted")
        );
        assert!(
            events(&rx)
                .iter()
                .any(|x| matches!(x, MiracastEvent::Disconnected { .. }))
        );
        e.shutdown();
    }

    #[test]
    fn disconnect_active_keeps_pending_and_listens_again() {
        let log = Arc::default();
        let (e, rx) = Engine::<Fake>::new(1);
        let c = fake(1, &log);
        e.on_connection_created(&c, "Laptop", None, || hold(&c));
        let id = requested_id(&events(&rx));
        e.answer(id, true).unwrap();
        assert_eq!(e.disconnect_active(), 1);
        assert_eq!(e.status(), MiracastStatus::Listening);
        assert_eq!(e.disconnect_active(), 0);
        assert!(
            log.lock()
                .unwrap()
                .iter()
                .any(|l| l == "disconnect 1 Finished")
        );
        // The sender's own Disconnected event afterwards is ignored (already removed).
        assert_eq!(e.on_disconnected(&c), None);
    }

    #[test]
    fn shutdown_ends_everything_and_silences_handlers() {
        let log = Arc::default();
        let (e, rx) = Engine::<Fake>::new(1);
        let (a, b) = (fake(1, &log), fake(2, &log));
        e.on_connection_created(&a, "A", None, || hold(&a));
        let id = requested_id(&events(&rx));
        e.answer(id, true).unwrap();
        e.shutdown();
        assert!(
            log.lock()
                .unwrap()
                .iter()
                .any(|l| l == "disconnect 1 Finished")
        );
        e.on_connection_created(&b, "B", None, || hold(&b));
        assert!(
            log.lock()
                .unwrap()
                .iter()
                .any(|l| l == "disconnect 2 NotAccepted")
        );
        assert!(matches!(
            e.answer(id, true),
            Err(MiracastError::UnknownConnection)
        ));
        assert_eq!(e.on_disconnected(&a), None);
        let _ = events(&rx);
    }

    #[test]
    fn a_stuck_consumer_does_not_block() {
        let (e, _rx) = Engine::<Fake>::new(1);
        for _ in 0..(EVENT_QUEUE * 3) {
            e.emit(MiracastEvent::Listening);
        }
        assert_eq!(e.status(), MiracastStatus::Listening);
    }

    #[test]
    fn hostile_names_are_cleaned_before_the_person_sees_them() {
        let log = Arc::default();
        let (e, rx) = Engine::<Fake>::new(1);
        let c = fake(1, &log);
        e.on_connection_created(&c, "Bob\u{202E}\nAccept?", None, || hold(&c));
        let ev = events(&rx);
        assert!(
            matches!(&ev[0], MiracastEvent::ConnectionRequested { device_name, .. }
            if device_name == "Bob Accept?")
        );
    }
}
