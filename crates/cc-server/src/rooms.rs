// SPDX-License-Identifier: GPL-3.0-or-later
//! In-memory room table. All functions are quick and never wait, so a plain
//! `std::sync::Mutex` around it is fine (it is never held across an `.await`).

use std::collections::HashMap;
use std::net::IpAddr;

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
    /// Send a WebSocket ping (liveness check).
    Ping,
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
    /// Who made the room (client address, as counted by the limits).
    owner: IpAddr,
    /// When the room was made.
    created: Instant,
    /// When something was last relayed (or someone joined).
    last_activity: Instant,
    /// Bytes relayed through this room so far.
    relayed: u64,
}

#[derive(Default)]
pub(crate) struct Rooms {
    map: HashMap<RoomCode, Room>,
    /// Rooms per owner address.
    per_owner: HashMap<IpAddr, usize>,
}

/// Why a room could not be made.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CreateError {
    /// The whole server has too many rooms.
    ServerFull,
    /// This client address owns too many rooms.
    OwnerFull,
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

    /// Remove a room and fix the per-owner count.
    fn remove(&mut self, code: &RoomCode) -> Option<Room> {
        let room = self.map.remove(code)?;
        if let Some(n) = self.per_owner.get_mut(&room.owner) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                self.per_owner.remove(&room.owner);
            }
        }
        Some(room)
    }

    /// Create a room with a fresh random code (retry on collision).
    /// Also returns a signal that fires when the room is removed.
    /// Fails when `max_rooms` rooms exist already, or `owner` has `max_per_owner`.
    pub(crate) fn create(
        &mut self,
        receiver_id: u64,
        receiver: Tx,
        owner: IpAddr,
        max_rooms: usize,
        max_per_owner: usize,
    ) -> Result<(RoomCode, Arc<Notify>), CreateError> {
        if self.map.len() >= max_rooms {
            return Err(CreateError::ServerFull);
        }
        if self.per_owner.get(&owner).copied().unwrap_or(0) >= max_per_owner {
            return Err(CreateError::OwnerFull);
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
                    owner,
                    created: Instant::now(),
                    last_activity: Instant::now(),
                    relayed: 0,
                },
            );
            *self.per_owner.entry(owner).or_insert(0) += 1;
            return Ok((code, cancel));
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
        room.last_activity = Instant::now();
        Ok(room.receiver.clone())
    }

    /// When the room was made and when something was last relayed.
    pub(crate) fn times(&self, code: &RoomCode) -> Option<(Instant, Instant)> {
        self.map.get(code).map(|r| (r.created, r.last_activity))
    }

    /// Count `bytes` relayed through the room and mark it active.
    /// Returns false when this would pass `max_bytes` (nothing should be relayed then).
    pub(crate) fn add_relayed(&mut self, code: &RoomCode, bytes: u64, max_bytes: u64) -> bool {
        let Some(room) = self.map.get_mut(code) else {
            return true;
        };
        let total = room.relayed.saturating_add(bytes);
        if total > max_bytes {
            return false;
        }
        room.relayed = total;
        room.last_activity = Instant::now();
        true
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
            let room = self.remove(code)?;
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
            if let Some(room) = self.remove(code) {
                room.receiver.send(Outgoing::Close("room_expired"));
            }
            return Err(Gone);
        }
        Ok(remaining)
    }
}

/// The room is gone (removed or expired).
pub(crate) struct Gone;
