// SPDX-License-Identifier: GPL-3.0-or-later
//! In-memory room table. All functions are quick and never wait, so a plain
//! `std::sync::Mutex` around it is fine (it is never held across an `.await`).

use std::collections::HashMap;

use cc_proto::{RoomCode, ServerMsg};
use rand::Rng;
use tokio::sync::mpsc::UnboundedSender;
use tokio::time::Instant;

/// What the writer task of a connection can be told to do.
#[derive(Debug)]
pub(crate) enum Outgoing {
    /// Send this message as a JSON text frame.
    Msg(ServerMsg),
    /// Send a close frame with this reason text, then stop.
    Close(&'static str),
}

pub(crate) type Tx = UnboundedSender<Outgoing>;

struct Room {
    receiver_id: u64,
    receiver: Tx,
    sender: Option<(u64, Tx)>,
    /// When the room last became sender-less (set at creation). `None` while a sender is in.
    empty_since: Option<Instant>,
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
    /// Create a room with a fresh random code (retry on collision).
    pub(crate) fn create(&mut self, receiver_id: u64, receiver: Tx) -> RoomCode {
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
            self.map.insert(
                code.clone(),
                Room {
                    receiver_id,
                    receiver,
                    sender: None,
                    empty_since: Some(Instant::now()),
                },
            );
            return code;
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
                let _ = room.receiver.send(Outgoing::Close("room_expired"));
            }
            return Err(Gone);
        }
        Ok(remaining)
    }
}

/// The room is gone (removed or expired).
pub(crate) struct Gone;
