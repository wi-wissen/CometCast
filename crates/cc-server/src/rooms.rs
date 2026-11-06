// SPDX-License-Identifier: GPL-3.0-or-later
//! In-memory room table. All functions are quick and never wait, so a plain
//! `std::sync::Mutex` around it is fine (it is never held across an `.await`).

use std::collections::HashMap;

use cc_proto::{RoomCode, ServerMsg};
use rand::Rng;
use std::sync::Arc;

use tokio::sync::{Notify, mpsc};
use tokio::time::Instant;

/// What the writer task of a connection can be told to do.
#[derive(Debug)]
pub(crate) enum Outgoing {
    /// Send this message as a JSON text frame.
    Msg(ServerMsg),
    /// Send a close frame with this reason text, then stop.
    Close(&'static str),
}

/// Handle to one connection's outgoing queue. The queue is bounded: if a client
/// does not read fast enough and the queue is full, the connection is told to
/// close (via `kill`) instead of letting memory grow without limit.
#[derive(Clone)]
pub(crate) struct Tx {
    queue: mpsc::Sender<Outgoing>,
    kill: Arc<Notify>,
}

impl Tx {
    /// New queue with room for `len` messages. Returns the handle, the receiving end
    /// for the writer task, and the signal the connection waits on to be closed.
    pub(crate) fn new(len: usize) -> (Self, mpsc::Receiver<Outgoing>, Arc<Notify>) {
        let (queue, rx) = mpsc::channel(len.max(1));
        let kill = Arc::new(Notify::new());
        (
            Self {
                queue,
                kill: kill.clone(),
            },
            rx,
            kill,
        )
    }

    /// Queue a message. Never waits. If the queue is full, the connection is killed.
    pub(crate) fn send(&self, out: Outgoing) {
        if let Err(mpsc::error::TrySendError::Full(_)) = self.queue.try_send(out) {
            // `notify_one` remembers the signal even if nobody waits yet.
            self.kill.notify_one();
        }
    }
}

struct Room {
    receiver_id: u64,
    receiver: Tx,
    sender: Option<(u64, Tx)>,
    /// When the room last became sender-less (set at creation). `None` while a sender is in.
    empty_since: Option<Instant>,
    /// Wakes the expiry task so it stops when the room is removed.
    cancel: Arc<Notify>,
}

#[derive(Default)]
pub(crate) struct Rooms {
    map: HashMap<RoomCode, Room>,
}

/// Why a join failed.
pub(crate) enum JoinError {
    NotFound,
    Full,
}

impl Rooms {
    /// How many rooms exist.
    pub(crate) fn len(&self) -> usize {
        self.map.len()
    }

    /// Create a room with a fresh random code (retry on collision).
    /// Also returns a signal that fires when the room is removed.
    /// Returns `None` when `max_rooms` rooms exist already.
    pub(crate) fn create(
        &mut self,
        receiver_id: u64,
        receiver: Tx,
        max_rooms: usize,
    ) -> Option<(RoomCode, Arc<Notify>)> {
        if self.map.len() >= max_rooms {
            return None;
        }
        let mut rng = rand::rng();
        loop {
            let mut idx = [0u8; RoomCode::LEN];
            for i in &mut idx {
                // The alphabet has 32 symbols, so indexes are 0..32.
                *i = rng.random_range(0..32);
            }
            let Some(code) = RoomCode::from_indices(idx) else {
                continue;
            };
            if self.map.contains_key(&code) {
                continue;
            }
            let cancel = Arc::new(Notify::new());
            self.map.insert(
                code.clone(),
                Room {
                    receiver_id,
                    receiver,
                    sender: None,
                    empty_since: Some(Instant::now()),
                    cancel: cancel.clone(),
                },
            );
            return Some((code, cancel));
        }
    }

    /// Put a sender into a room. On success returns the receiver's channel.
    pub(crate) fn join(&mut self, code: &RoomCode, id: u64, tx: Tx) -> Result<Tx, JoinError> {
        let room = self.map.get_mut(code).ok_or(JoinError::NotFound)?;
        if room.sender.is_some() {
            return Err(JoinError::Full);
        }
        room.sender = Some((id, tx));
        room.empty_since = None;
        Ok(room.receiver.clone())
    }

    /// Is this connection still part of this room?
    pub(crate) fn is_member(&self, code: &RoomCode, id: u64) -> bool {
        self.map.get(code).is_some_and(|room| {
            room.receiver_id == id || room.sender.as_ref().is_some_and(|(sid, _)| *sid == id)
        })
    }

    /// Channel of the other side of the room, if both sides are present.
    pub(crate) fn peer(&self, code: &RoomCode, from_id: u64) -> Option<Tx> {
        let room = self.map.get(code)?;
        if room.receiver_id == from_id {
            room.sender.as_ref().map(|(_, tx)| tx.clone())
        } else if room.sender.as_ref().is_some_and(|(id, _)| *id == from_id) {
            Some(room.receiver.clone())
        } else {
            None
        }
    }

    /// A connection went away. Returns the channel of the remaining peer, if any.
    pub(crate) fn leave(&mut self, code: &RoomCode, id: u64) -> Option<Tx> {
        let room = self.map.get_mut(code)?;
        if room.receiver_id == id {
            // The room ends with its receiver.
            let room = self.map.remove(code)?;
            room.cancel.notify_one();
            room.sender.map(|(_, tx)| tx)
        } else if room.sender.as_ref().is_some_and(|(sid, _)| *sid == id) {
            room.sender = None;
            room.empty_since = Some(Instant::now());
            Some(room.receiver.clone())
        } else {
            None
        }
    }

    /// For the expiry task. `Ok(wait)`: check again after `wait`.
    /// `Err(Gone)`: the room no longer exists. Removes the room when it is idle too long
    /// and tells its receiver to close.
    pub(crate) fn check_expiry(
        &mut self,
        code: &RoomCode,
        idle: std::time::Duration,
    ) -> Result<std::time::Duration, Gone> {
        let room = self.map.get(code).ok_or(Gone)?;
        let Some(since) = room.empty_since else {
            return Ok(idle);
        };
        let remaining = (since + idle).saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            if let Some(room) = self.map.remove(code) {
                room.receiver.send(Outgoing::Close("room_expired"));
            }
            return Err(Gone);
        }
        Ok(remaining)
    }
}

/// The room is gone (removed or expired).
pub(crate) struct Gone;
