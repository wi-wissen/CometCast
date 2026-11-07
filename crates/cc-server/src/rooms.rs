// SPDX-License-Identifier: GPL-3.0-or-later
//! In-memory room table. All functions are quick and never wait, so a plain
//! `std::sync::Mutex` around it is fine (it is never held across an `.await`).

use std::collections::HashMap;
use std::net::IpAddr;

use cc_proto::{JoinMode, RoomCode, ServerMsg, error_codes, sanitize_display_name};
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

/// A sender that waits for the owner's decision. Holds only what the server needs; the owner
/// is told the sanitised `name` and the room-local `req` id, nothing else.
struct Pending {
    req: u64,
    /// Two-digit number shown to both sides.
    check: u8,
    conn_id: u64,
    tx: Tx,
    /// Address the joiner is counted under (for the per-address pending limit).
    ip: IpAddr,
    name: String,
}

/// An entry of a room's block list: blocked for the rest of the room session.
struct Block {
    id: u64,
    /// The address bucket (IPv4 address or IPv6 /64) that may not join this room any more.
    bucket: IpAddr,
}

/// Most entries on one room's block list.
const MAX_BLOCKS: usize = 32;

struct Room {
    mode: JoinMode,
    blocks: Vec<Block>,
    next_block: u64,
    /// Address bucket and name of the sender who is in the room (for "remove and block").
    sender_info: Option<(IpAddr, String)>,
    pending: Vec<Pending>,
    /// Next request id (room-local, starts at 1).
    next_req: u64,
    /// Counter for "Guest N" names.
    guests: u32,
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
    /// Waiting join requests per client address (all rooms).
    pending_per_ip: HashMap<IpAddr, usize>,
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
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum JoinError {
    NotFound,
    Full,
    /// Join mode `locked`.
    Locked,
    /// Too many requests wait in this room.
    RoomPendingFull,
    /// Too many requests wait from this address.
    IpPendingFull,
    /// This address bucket is on the room's block list.
    Blocked,
}

/// A successful join.
pub(crate) enum Joined {
    /// The sender is in the room now.
    Member { receiver: Tx, name: String },
    /// The sender waits for the owner's decision.
    Pending {
        owner: Tx,
        req: u64,
        check: u8,
        name: String,
    },
}

/// Where a connection stands in a room it tried to join.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Standing {
    Member,
    Pending,
    Gone,
}

/// Why the owner's decision or mode change was refused.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum OwnerError {
    /// The caller does not own this room (or is not in it).
    NotOwner,
    /// No such waiting request.
    NoSuchRequest,
    /// No such block list entry.
    NoSuchBlock,
    /// The block list is full.
    BlockFull,
}

/// Result of `decide`.
pub(crate) struct Decided {
    /// The approved sender: (its channel, owner's channel, name).
    pub(crate) approved: Option<(Tx, Tx, String)>,
    /// A block list entry that was added: (id, name).
    pub(crate) blocked: Option<(u64, String)>,
    /// Requests that ended without joining: (joiner's channel, request id, error code to the
    /// joiner, reason for the owner).
    pub(crate) dropped: Vec<(Tx, u64, &'static str, &'static str, IpAddr)>,
}

/// What a leaving connection leaves behind.
#[derive(Default)]
pub(crate) struct Left {
    /// Remaining peer that gets `peer_left`.
    pub(crate) peer: Option<Tx>,
    /// A waiting request ended: tell the owner.
    pub(crate) owner_cancel: Option<(Tx, u64)>,
    /// Waiting joiners of a room that ended with its owner.
    pub(crate) orphaned: Vec<Tx>,
}

impl Rooms {
    /// How many rooms exist.
    pub(crate) fn len(&self) -> usize {
        self.map.len()
    }

    /// Remove a room and fix the per-owner count.
    fn remove(&mut self, code: &RoomCode) -> Option<Room> {
        let room = self.map.remove(code)?;
        for p in &room.pending {
            Self::dec(&mut self.pending_per_ip, p.ip);
        }
        if let Some(n) = self.per_owner.get_mut(&room.owner) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                self.per_owner.remove(&room.owner);
            }
        }
        Some(room)
    }

    fn dec(map: &mut HashMap<IpAddr, usize>, ip: IpAddr) {
        if let Some(n) = map.get_mut(&ip) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                map.remove(&ip);
            }
        }
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
        mode: JoinMode,
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
                    mode,
                    blocks: Vec::new(),
                    next_block: 1,
                    sender_info: None,
                    pending: Vec::new(),
                    next_req: 1,
                    guests: 0,
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

    /// A sender wants into a room. In `open` mode it is in at once; in `confirm` mode (and
    /// when it announced the feature) it waits. `raw_name` is untrusted and gets sanitised.
    /// `ip` is the joiner's counting address; `max_*` are the pending limits.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn join(
        &mut self,
        code: &RoomCode,
        id: u64,
        tx: Tx,
        raw_name: Option<&str>,
        ip: IpAddr,
        max_pending_room: usize,
        max_pending_ip: usize,
    ) -> Result<Joined, JoinError> {
        let room = self.map.get_mut(code).ok_or(JoinError::NotFound)?;
        if room.sender.is_some() {
            return Err(JoinError::Full);
        }
        match room.mode {
            JoinMode::Locked => return Err(JoinError::Locked),
            _ if room.blocks.iter().any(|b| b.bucket == ip) => return Err(JoinError::Blocked),
            JoinMode::Open => {
                room.guests += 1;
                let name = sanitize_display_name(raw_name, room.guests);
                room.sender_info = Some((ip, name.clone()));
                room.sender = Some((id, tx));
                room.empty_since = None;
                room.last_activity = Instant::now();
                Ok(Joined::Member {
                    receiver: room.receiver.clone(),
                    name,
                })
            }
            JoinMode::Confirm => {
                if room.pending.len() >= max_pending_room {
                    return Err(JoinError::RoomPendingFull);
                }
                if self.pending_per_ip.get(&ip).copied().unwrap_or(0) >= max_pending_ip {
                    return Err(JoinError::IpPendingFull);
                }
                room.guests += 1;
                let name = sanitize_display_name(raw_name, room.guests);
                let req = room.next_req;
                room.next_req += 1;
                let check = fresh_check(&room.pending);
                room.pending.push(Pending {
                    req,
                    check,
                    conn_id: id,
                    tx,
                    ip,
                    name: name.clone(),
                });
                *self.pending_per_ip.entry(ip).or_insert(0) += 1;
                Ok(Joined::Pending {
                    owner: room.receiver.clone(),
                    req,
                    check,
                    name,
                })
            }
        }
    }

    /// Is this connection a member, waiting, or neither?
    pub(crate) fn standing(&self, code: &RoomCode, id: u64) -> Standing {
        let Some(room) = self.map.get(code) else {
            return Standing::Gone;
        };
        if room.sender.as_ref().is_some_and(|(sid, _)| *sid == id) {
            Standing::Member
        } else if room.pending.iter().any(|p| p.conn_id == id) {
            Standing::Pending
        } else {
            Standing::Gone
        }
    }

    /// The joiner stops waiting (cancel, timeout, disconnect). Returns the owner's channel and
    /// the request id if it was still waiting.
    pub(crate) fn cancel_pending(&mut self, code: &RoomCode, id: u64) -> Option<(Tx, u64)> {
        let room = self.map.get_mut(code)?;
        let at = room.pending.iter().position(|p| p.conn_id == id)?;
        let p = room.pending.remove(at);
        let owner = room.receiver.clone();
        Self::dec(&mut self.pending_per_ip, p.ip);
        Some((owner, p.req))
    }

    /// The owner decides. `req` `None` with `allow == false` denies everybody who waits.
    pub(crate) fn decide(
        &mut self,
        code: &RoomCode,
        owner_id: u64,
        req: Option<u64>,
        allow: bool,
        block: bool,
    ) -> Result<Decided, OwnerError> {
        let room = self.map.get_mut(code).ok_or(OwnerError::NotOwner)?;
        if room.receiver_id != owner_id {
            return Err(OwnerError::NotOwner);
        }
        let mut out = Decided {
            blocked: None,
            approved: None,
            dropped: Vec::new(),
        };
        let taken: Vec<Pending> = match (req, allow) {
            (None, false) => std::mem::take(&mut room.pending),
            (None, true) => return Err(OwnerError::NoSuchRequest),
            (Some(r), _) => {
                let at = room
                    .pending
                    .iter()
                    .position(|p| p.req == r)
                    .ok_or(OwnerError::NoSuchRequest)?;
                vec![room.pending.remove(at)]
            }
        };
        for p in &taken {
            Self::dec(&mut self.pending_per_ip, p.ip);
        }
        let room = self.map.get_mut(code).ok_or(OwnerError::NotOwner)?;
        // "No and block": the denied request's address bucket is blocked, and every other
        // request of the same bucket ends with it.
        if block && !allow {
            if let Some(p) = taken.first() {
                let (ip, name) = (p.ip, p.name.clone());
                match Self::add_block(room, ip, &name) {
                    Ok(b) => out.blocked = b,
                    Err(e) => {
                        // Not decided: put the request back so the host can decide again.
                        let mut back = taken;
                        back.append(&mut room.pending);
                        room.pending = back;
                        for q in &room.pending {
                            *self.pending_per_ip.entry(q.ip).or_insert(0) += 1;
                        }
                        return Err(e);
                    }
                }
                let (same, rest): (Vec<Pending>, Vec<Pending>) = std::mem::take(&mut room.pending)
                    .into_iter()
                    .partition(|q| q.ip == ip);
                room.pending = rest;
                for q in same {
                    Self::dec(&mut self.pending_per_ip, q.ip);
                    out.dropped
                        .push((q.tx, q.req, error_codes::JOIN_DENIED, "blocked", q.ip));
                }
            }
        }
        for p in taken {
            if !allow {
                out.dropped
                    .push((p.tx, p.req, error_codes::JOIN_DENIED, "denied", p.ip));
            } else if room.sender.is_some() {
                out.dropped
                    .push((p.tx, p.req, "room_full", "room_full", p.ip));
            } else {
                room.sender = Some((p.conn_id, p.tx.clone()));
                room.sender_info = Some((p.ip, p.name.clone()));
                room.empty_since = None;
                room.last_activity = Instant::now();
                out.approved = Some((p.tx, room.receiver.clone(), p.name));
            }
        }
        if out.approved.is_some() {
            // Only one sender per room: everybody else who waits cannot be let in any more.
            let rest = std::mem::take(&mut room.pending);
            for p in rest {
                Self::dec(&mut self.pending_per_ip, p.ip);
                out.dropped
                    .push((p.tx, p.req, "room_full", "room_full", p.ip));
            }
        }
        Ok(out)
    }

    /// Owner changes the mode. Switching to `locked` drops every waiting request (returned).
    pub(crate) fn set_mode(
        &mut self,
        code: &RoomCode,
        owner_id: u64,
        mode: JoinMode,
    ) -> Result<Vec<(Tx, u64)>, OwnerError> {
        let room = self.map.get_mut(code).ok_or(OwnerError::NotOwner)?;
        if room.receiver_id != owner_id {
            return Err(OwnerError::NotOwner);
        }
        room.mode = mode;
        let mut dropped = Vec::new();
        if mode == JoinMode::Locked {
            for p in std::mem::take(&mut room.pending) {
                Self::dec(&mut self.pending_per_ip, p.ip);
                dropped.push((p.tx, p.req));
            }
        }
        Ok(dropped)
    }

    /// Add a block list entry for `bucket` (not twice). Returns the new entry's id and name,
    /// or `None` if the bucket was blocked already.
    fn add_block(
        room: &mut Room,
        bucket: IpAddr,
        name: &str,
    ) -> Result<Option<(u64, String)>, OwnerError> {
        if room.blocks.iter().any(|b| b.bucket == bucket) {
            return Ok(None);
        }
        if room.blocks.len() >= MAX_BLOCKS {
            return Err(OwnerError::BlockFull);
        }
        let id = room.next_block;
        room.next_block += 1;
        room.blocks.push(Block { id, bucket });
        Ok(Some((id, name.to_string())))
    }

    /// Owner removes the connected sender, optionally blocking its address bucket. Returns the
    /// sender's channel and the new block list entry.
    #[allow(clippy::type_complexity)]
    pub(crate) fn remove_peer(
        &mut self,
        code: &RoomCode,
        owner_id: u64,
        block: bool,
    ) -> Result<Option<(Tx, Option<(u64, String)>)>, OwnerError> {
        let room = self.map.get_mut(code).ok_or(OwnerError::NotOwner)?;
        if room.receiver_id != owner_id {
            return Err(OwnerError::NotOwner);
        }
        if room.sender.is_none() {
            return Ok(None);
        }
        let mut entry = None;
        if block && let Some((bucket, name)) = room.sender_info.clone() {
            entry = Self::add_block(room, bucket, &name)?;
        }
        let removed = room.sender.take().map(|(_, tx)| tx);
        room.sender_info = None;
        room.empty_since = Some(Instant::now());
        Ok(removed.map(|tx| (tx, entry)))
    }

    /// Owner takes an entry off the block list.
    pub(crate) fn unblock(
        &mut self,
        code: &RoomCode,
        owner_id: u64,
        id: u64,
    ) -> Result<(), OwnerError> {
        let room = self.map.get_mut(code).ok_or(OwnerError::NotOwner)?;
        if room.receiver_id != owner_id {
            return Err(OwnerError::NotOwner);
        }
        let at = room
            .blocks
            .iter()
            .position(|b| b.id == id)
            .ok_or(OwnerError::NoSuchBlock)?;
        room.blocks.remove(at);
        Ok(())
    }

    /// Owner ends the room session: the room is removed. Returns who is left to be told.
    pub(crate) fn end_room(&mut self, code: &RoomCode, owner_id: u64) -> Result<Left, OwnerError> {
        let room = self.map.get(code).ok_or(OwnerError::NotOwner)?;
        if room.receiver_id != owner_id {
            return Err(OwnerError::NotOwner);
        }
        Ok(self.leave(code, owner_id))
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

    /// A connection went away (member, owner or waiting joiner).
    pub(crate) fn leave(&mut self, code: &RoomCode, id: u64) -> Left {
        let Some(room) = self.map.get_mut(code) else {
            return Left::default();
        };
        if room.receiver_id == id {
            // The room ends with its receiver.
            let Some(room) = self.remove(code) else {
                return Left::default();
            };
            room.cancel.notify_one();
            Left {
                peer: room.sender.map(|(_, tx)| tx),
                owner_cancel: None,
                orphaned: room.pending.into_iter().map(|p| p.tx).collect(),
            }
        } else if room.sender.as_ref().is_some_and(|(sid, _)| *sid == id) {
            room.sender = None;
            room.sender_info = None;
            room.empty_since = Some(Instant::now());
            Left {
                peer: Some(room.receiver.clone()),
                ..Left::default()
            }
        } else {
            Left {
                owner_cancel: self.cancel_pending(code, id),
                ..Left::default()
            }
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
                for p in &room.pending {
                    p.tx.send(Outgoing::Msg(ServerMsg::Error {
                        code: "room_not_found".into(),
                        message: "the room was closed".into(),
                    }));
                }
                room.receiver.send(Outgoing::Close("room_expired"));
            }
            return Err(Gone);
        }
        Ok(remaining)
    }
}

/// The room is gone (removed or expired).
pub(crate) struct Gone;

/// A two-digit number (10..=99) that no waiting request of the room uses yet.
fn fresh_check(pending: &[Pending]) -> u8 {
    let mut rng = rand::rng();
    loop {
        let n: u8 = rng.random_range(10..=99);
        if pending.iter().all(|p| p.check != n) {
            return n;
        }
    }
}
