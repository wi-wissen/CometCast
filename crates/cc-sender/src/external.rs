// SPDX-License-Identifier: GPL-3.0-or-later
//! Input mode `ExternalH264`: the host (for example the Android app, whose `MediaCodec` already
//! produces H.264) pushes finished access units; this module normalises them and queues them for
//! the RTP packetiser of the transport. No capture and no encoder run in the Rust core.
//!
//! What is accepted (defensive, because the exact output format of `MediaCodec` on real devices
//! was not checked; UNCERTAIN, see docs/android.md):
//!
//! * Annex B (start codes `00 00 01` / `00 00 00 01`) or AVCC (length-prefixed NAL units, 4 byte
//!   lengths; other sizes only after an `avcC` config record announced them). Both are turned into Annex B, which is what the packetiser wants.
//! * Codec configuration separately ([`ExternalInput::set_config`]): Annex B with SPS/PPS, or an
//!   `avcC` record. SPS/PPS found inside access units update the cache too.
//! * Key frames without SPS/PPS get the cached ones in front (after a leading access unit
//!   delimiter), so a receiver that joins at any key frame can decode. A key frame that cannot be
//!   completed (no parameter sets known yet) is dropped and a new one is requested.
//!
//! Rules (same spirit as the screen pipeline in `screen.rs`):
//!
//! * One frame in flight: the sender task takes one unit from the queue and awaits the transport
//!   before it takes the next one.
//! * The queue is bounded ([`ExternalOptions::max_queue`]). A full queue, a host that pushes while
//!   no peer is connected and deltas that follow a drop are dropped and counted; after a drop all
//!   delta frames are discarded until the next key frame (a stream with a hole would show
//!   garbage), and a key frame is requested.
//! * A new IDR frame replaces everything that is still queued (nothing older is needed).
//! * Key frame requests (new peer, PLI/FIR from the receiver, the drop policy above) set a flag
//!   that the host reads with [`ExternalInput::keyframe_requested`] (poll) or waits for with
//!   [`ExternalInput::wait_keyframe_request`]. They are rate limited
//!   ([`ExternalOptions::keyframe_min_interval`]), like the X11 path.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use tokio::sync::Notify;

/// Largest access unit accepted (a 4K key frame is far below this).
/// UNCERTAIN: placeholder, not derived from a measurement.
pub const MAX_ACCESS_UNIT_BYTES: usize = 8 * 1024 * 1024;

/// Settings of the `ExternalH264` input.
#[derive(Debug, Clone)]
pub struct ExternalOptions {
    /// Most access units that wait for the transport. UNCERTAIN: 3 is a placeholder.
    pub max_queue: usize,
    /// At most one key frame request is accepted per this time (others are counted as ignored).
    pub keyframe_min_interval: Duration,
}

impl Default for ExternalOptions {
    fn default() -> Self {
        Self {
            max_queue: 3,
            keyframe_min_interval: crate::screen::KEYFRAME_MIN_INTERVAL,
        }
    }
}

/// What happened to one pushed access unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushOutcome {
    /// Queued for sending.
    Queued,
    /// No peer is connected (yet): dropped. Key frame requested for when one connects.
    NotStreaming,
    /// The queue was full: dropped; delta frames are discarded until the next key frame.
    QueueFull,
    /// A delta frame while a key frame is awaited (after a drop): dropped.
    WaitingForKeyFrame,
    /// A key frame without SPS/PPS and none cached: dropped, key frame requested.
    MissingParameterSets,
    /// Not valid H.264 in a known format, empty, or too large: dropped.
    Invalid,
    /// The input was closed (the sender stopped).
    Closed,
}

/// Counters of the external input, readable while it runs.
#[derive(Debug, Default)]
struct Counters {
    pushed: AtomicU64,
    queued: AtomicU64,
    sent: AtomicU64,
    bytes_sent: AtomicU64,
    keyframes_sent: AtomicU64,
    dropped_not_streaming: AtomicU64,
    dropped_queue_full: AtomicU64,
    dropped_waiting_key: AtomicU64,
    dropped_no_params: AtomicU64,
    dropped_superseded: AtomicU64,
    invalid: AtomicU64,
    keyframe_requests: AtomicU64,
    keyframe_requests_ignored: AtomicU64,
}

/// A copy of the counters of the external input.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExternalStats {
    /// Access units handed to [`ExternalInput::push`].
    pub pushed: u64,
    /// Of those, units that entered the queue.
    pub queued: u64,
    /// Units written to the transport.
    pub sent: u64,
    /// Bytes written to the transport (Annex B, after normalising).
    pub bytes_sent: u64,
    /// Of the sent units, key frames.
    pub keyframes_sent: u64,
    /// Dropped because no peer was connected.
    pub dropped_not_streaming: u64,
    /// Dropped because the queue was full.
    pub dropped_queue_full: u64,
    /// Delta frames dropped while a key frame was awaited.
    pub dropped_waiting_key: u64,
    /// Key frames dropped because no SPS/PPS were known.
    pub dropped_no_params: u64,
    /// Queued units replaced by a newer key frame.
    pub dropped_superseded: u64,
    /// Rejected as not valid.
    pub invalid: u64,
    /// Key frame requests (PLI/FIR, new peer, drop policy).
    pub keyframe_requests: u64,
    /// Of those, ignored: one was pending or one was accepted too recently.
    pub keyframe_requests_ignored: u64,
}

impl ExternalStats {
    /// All units that were pushed but not sent and not (any more) waiting: the sum of the
    /// drop counters.
    pub fn dropped(&self) -> u64 {
        self.dropped_not_streaming
            + self.dropped_queue_full
            + self.dropped_waiting_key
            + self.dropped_no_params
            + self.dropped_superseded
    }
}

/// One access unit in the queue (Annex B).
#[derive(Debug)]
pub(crate) struct Unit {
    pub(crate) data: Vec<u8>,
    pub(crate) pts_us: Option<u64>,
    pub(crate) key: bool,
}

#[derive(Default)]
struct State {
    sps: Vec<Vec<u8>>,
    pps: Vec<Vec<u8>>,
    /// Length field size of AVCC input; set by an `avcC` record, 4 otherwise.
    nal_length_size: Option<usize>,
    queue: VecDeque<Unit>,
    streaming: bool,
    /// Deltas are dropped until a key frame arrived.
    need_key: bool,
    last_key_request: Option<Instant>,
}

struct Inner {
    options: ExternalOptions,
    state: Mutex<State>,
    frame_ready: Notify,
    key_ready: Notify,
    key_flag: AtomicBool,
    closed: AtomicBool,
    counters: Counters,
}

/// The host side of the `ExternalH264` mode. Cheap to clone; all clones share the state.
/// Methods never block (short locks only), so they can be called from any thread.
#[derive(Clone)]
pub struct ExternalInput {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for ExternalInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExternalInput").finish_non_exhaustive()
    }
}

impl Default for ExternalInput {
    fn default() -> Self {
        Self::new(ExternalOptions::default())
    }
}

impl ExternalInput {
    /// A new input. Pass it to `SenderOptions::external`; keep a clone to push frames.
    pub fn new(options: ExternalOptions) -> Self {
        Self {
            inner: Arc::new(Inner {
                options: ExternalOptions {
                    max_queue: options.max_queue.max(1),
                    ..options
                },
                state: Mutex::new(State {
                    need_key: true,
                    ..State::default()
                }),
                frame_ready: Notify::new(),
                key_ready: Notify::new(),
                key_flag: AtomicBool::new(false),
                closed: AtomicBool::new(false),
                counters: Counters::default(),
            }),
        }
    }

    /// True if `other` is a clone of this input.
    pub fn same_as(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Give the codec configuration: Annex B with SPS/PPS, or an `avcC` record (which also
    /// sets the length size for AVCC access units). Returns `false` if neither SPS nor PPS could
    /// be read from `data`.
    pub fn set_config(&self, data: &[u8]) -> bool {
        let mut st = self.state();
        apply_config(&mut st, data)
    }

    /// Push one access unit (copied; the caller keeps its buffer). `is_key` is the host's
    /// knowledge (for example `MediaCodec.BUFFER_FLAG_KEY_FRAME`); an IDR slice in the data counts
    /// as a key frame too. `pts_us` (microseconds, presentation time) sets the RTP timestamp
    /// step; pass `None` if unknown.
    pub fn push(&self, data: &[u8], is_key: bool, pts_us: Option<u64>) -> PushOutcome {
        self.push_at(data, is_key, pts_us, Instant::now())
    }

    fn push_at(&self, data: &[u8], is_key: bool, pts_us: Option<u64>, now: Instant) -> PushOutcome {
        let c = &self.inner.counters;
        c.pushed.fetch_add(1, Ordering::Relaxed);
        if self.inner.closed.load(Ordering::Relaxed) {
            return PushOutcome::Closed;
        }
        let mut st = self.state();
        // Normalise before anything else: parameter sets in the unit update the cache even if
        // the unit itself is dropped below.
        let au = match normalize(&mut st, data) {
            Ok(au) => au,
            Err(_) => {
                c.invalid.fetch_add(1, Ordering::Relaxed);
                return PushOutcome::Invalid;
            }
        };
        let has_idr = au.has_idr;
        let key = is_key || has_idr;
        if !st.streaming {
            c.dropped_not_streaming.fetch_add(1, Ordering::Relaxed);
            return PushOutcome::NotStreaming;
        }
        if st.need_key && !key {
            c.dropped_waiting_key.fetch_add(1, Ordering::Relaxed);
            self.request_locked(&mut st, now);
            return PushOutcome::WaitingForKeyFrame;
        }
        let out = if key {
            match complete_key_frame(&st, au) {
                Some(out) => out,
                None => {
                    c.dropped_no_params.fetch_add(1, Ordering::Relaxed);
                    st.need_key = true;
                    self.request_locked(&mut st, now);
                    return PushOutcome::MissingParameterSets;
                }
            }
        } else {
            au.annex_b
        };
        if key && has_idr {
            // An IDR frame resets the decoder: everything queued before it is obsolete.
            let old = st.queue.len() as u64;
            st.queue.clear();
            c.dropped_superseded.fetch_add(old, Ordering::Relaxed);
        }
        if st.queue.len() >= self.inner.options.max_queue {
            c.dropped_queue_full.fetch_add(1, Ordering::Relaxed);
            st.need_key = true;
            self.request_locked(&mut st, now);
            return PushOutcome::QueueFull;
        }
        if key {
            st.need_key = false;
            // The pending request is answered.
            self.inner.key_flag.store(false, Ordering::Relaxed);
        }
        st.queue.push_back(Unit {
            data: out,
            pts_us,
            key,
        });
        c.queued.fetch_add(1, Ordering::Relaxed);
        drop(st);
        self.inner.frame_ready.notify_one();
        PushOutcome::Queued
    }

    /// Ask the host for a key frame (rate limited). Returns whether the request was accepted.
    /// Called for PLI/FIR from the receiver.
    pub fn request_keyframe(&self) -> bool {
        let mut st = self.state();
        self.request_locked(&mut st, Instant::now())
    }

    /// Like [`Self::request_keyframe`] with the clock passed in (for tests).
    pub fn request_keyframe_at(&self, now: Instant) -> bool {
        let mut st = self.state();
        self.request_locked(&mut st, now)
    }

    fn request_locked(&self, st: &mut State, now: Instant) -> bool {
        let c = &self.inner.counters;
        c.keyframe_requests.fetch_add(1, Ordering::Relaxed);
        let too_soon = st.last_key_request.is_some_and(|t| {
            now.saturating_duration_since(t) < self.inner.options.keyframe_min_interval
        });
        // A request while one is pending is covered by that one.
        let pending = self.inner.key_flag.load(Ordering::Relaxed);
        if too_soon || pending {
            c.keyframe_requests_ignored.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        st.last_key_request = Some(now);
        self.inner.key_flag.store(true, Ordering::Relaxed);
        self.inner.key_ready.notify_one();
        true
    }

    /// True once per key frame request: the host should make a sync frame now. Reading clears
    /// the flag (a new request is accepted after the rate limit).
    pub fn keyframe_requested(&self) -> bool {
        self.inner.key_flag.swap(false, Ordering::Relaxed)
    }

    /// Wait until a key frame is requested (and clear the request). Returns `false` once the
    /// input is closed.
    pub async fn wait_keyframe_request(&self) -> bool {
        loop {
            if self.inner.closed.load(Ordering::Relaxed) {
                return false;
            }
            if self.keyframe_requested() {
                return true;
            }
            self.inner.key_ready.notified().await;
        }
    }

    /// A peer is connected: from now on units are accepted. Clears the queue and asks for a
    /// key frame, bypassing the rate limit (the peer has seen nothing yet).
    pub(crate) fn begin_streaming(&self) {
        let mut st = self.state();
        st.streaming = true;
        st.need_key = true;
        st.queue.clear();
        st.last_key_request = Some(Instant::now());
        self.inner
            .counters
            .keyframe_requests
            .fetch_add(1, Ordering::Relaxed);
        self.inner.key_flag.store(true, Ordering::Relaxed);
        self.inner.key_ready.notify_one();
    }

    /// Stop accepting units; wakes the waiting sender task and key frame waiters.
    pub fn close(&self) {
        self.inner.closed.store(true, Ordering::Relaxed);
        self.state().queue.clear();
        self.inner.frame_ready.notify_one();
        self.inner.key_ready.notify_one();
    }

    /// The next unit to send; `None` once the input is closed. Cancel safe.
    pub(crate) async fn next_unit(&self) -> Option<Unit> {
        loop {
            if self.inner.closed.load(Ordering::Relaxed) {
                return None;
            }
            if let Some(u) = self.state().queue.pop_front() {
                return Some(u);
            }
            self.inner.frame_ready.notified().await;
        }
    }

    /// The transport took `unit`: update the counters.
    pub(crate) fn count_sent(&self, unit_len: usize, key: bool) {
        let c = &self.inner.counters;
        c.sent.fetch_add(1, Ordering::Relaxed);
        c.bytes_sent.fetch_add(unit_len as u64, Ordering::Relaxed);
        if key {
            c.keyframes_sent.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Number of units waiting for the transport.
    pub fn queue_len(&self) -> usize {
        self.state().queue.len()
    }

    /// A copy of the counters.
    pub fn stats(&self) -> ExternalStats {
        let c = &self.inner.counters;
        let g = |a: &AtomicU64| a.load(Ordering::Relaxed);
        ExternalStats {
            pushed: g(&c.pushed),
            queued: g(&c.queued),
            sent: g(&c.sent),
            bytes_sent: g(&c.bytes_sent),
            keyframes_sent: g(&c.keyframes_sent),
            dropped_not_streaming: g(&c.dropped_not_streaming),
            dropped_queue_full: g(&c.dropped_queue_full),
            dropped_waiting_key: g(&c.dropped_waiting_key),
            dropped_no_params: g(&c.dropped_no_params),
            dropped_superseded: g(&c.dropped_superseded),
            invalid: g(&c.invalid),
            keyframe_requests: g(&c.keyframe_requests),
            keyframe_requests_ignored: g(&c.keyframe_requests_ignored),
        }
    }
}

/// The RTP time step for a frame: the difference of the presentation times when both are known
/// and plausible (1 ms to 1 s), otherwise the wall-clock time since the previous frame, clamped
/// the same way.
pub(crate) fn rtp_step(prev_pts: Option<u64>, pts: Option<u64>, wall: Duration) -> Duration {
    const MIN: Duration = Duration::from_millis(1);
    const MAX: Duration = Duration::from_secs(1);
    let from_pts = match (prev_pts, pts) {
        (Some(a), Some(b)) if b > a => Some(Duration::from_micros(b - a)),
        _ => None,
    };
    from_pts.unwrap_or(wall).clamp(MIN, MAX)
}

// ---------------------------------------------------------------------------
// Format handling
// ---------------------------------------------------------------------------

/// Why bytes could not be read as H.264.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    /// Empty input.
    Empty,
    /// Larger than [`MAX_ACCESS_UNIT_BYTES`].
    TooLarge,
    /// Neither Annex B nor AVCC with consistent lengths.
    UnknownFormat,
}

/// A normalised access unit.
struct AccessUnit {
    /// Annex B with 4-byte start codes.
    annex_b: Vec<u8>,
    has_idr: bool,
    has_sps: bool,
    has_pps: bool,
    /// The first NAL is an access unit delimiter (type 9).
    leading_aud: bool,
}

/// NAL units (without start codes or length prefixes) of an Annex B stream.
fn split_annex_b(data: &[u8]) -> Vec<&[u8]> {
    let mut starts = Vec::new();
    let mut i = 0;
    while i + 3 <= data.len() {
        if data[i] == 0 && data[i + 1] == 0 && data[i + 2] == 1 {
            starts.push(i + 3);
            i += 3;
        } else {
            i += 1;
        }
    }
    let mut out = Vec::with_capacity(starts.len());
    for (n, &s) in starts.iter().enumerate() {
        let mut e = starts.get(n + 1).map_or(data.len(), |next| next - 3);
        // Trailing zero bytes belong to the next start code (4-byte form) or are padding.
        while e > s && data[e - 1] == 0 {
            e -= 1;
        }
        if e > s {
            out.push(&data[s..e]);
        }
    }
    out
}

fn is_plausible_nal(nal: &[u8]) -> bool {
    // forbidden_zero_bit must be 0; type 0 and 24..=31 are not used in H.264 byte streams.
    // (Types 24..=31 exist only inside RTP payloads.)
    let Some(h) = nal.first() else { return false };
    let t = h & 0x1f;
    h & 0x80 == 0 && (1..=23).contains(&t)
}

/// NAL units of a length-prefixed (AVCC) access unit, or `None` if the lengths are not exactly
/// consistent with the data.
fn split_avcc(data: &[u8], length_size: usize) -> Option<Vec<&[u8]>> {
    if !(1..=4).contains(&length_size) {
        return None;
    }
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < data.len() {
        let prefix = data.get(pos..pos + length_size)?;
        let len = prefix
            .iter()
            .fold(0usize, |acc, b| (acc << 8) | usize::from(*b));
        pos += length_size;
        if len == 0 {
            return None;
        }
        let nal = data.get(pos..pos.checked_add(len)?)?;
        if !is_plausible_nal(nal) {
            return None;
        }
        out.push(nal);
        pos += len;
    }
    (!out.is_empty()).then_some(out)
}

/// Read `data` as AVCC (tried first: its lengths must add up exactly, which an Annex B stream
/// does not do by accident) and then as Annex B. A 4-byte length 256..=511 looks like the start
/// code `00 00 01 xx`, which is why Annex B is not tried first.
fn split_any(data: &[u8], preferred_length_size: usize) -> Option<Vec<&[u8]>> {
    // Other length sizes are only used when an `avcC` record announced them: with 1 or 2 byte
    // lengths short garbage would pass the exact-length check by accident.
    let mut sizes = vec![preferred_length_size];
    if preferred_length_size != 4 {
        sizes.push(4);
    }
    for s in sizes {
        if let Some(nals) = split_avcc(data, s) {
            return Some(nals);
        }
    }
    let nals = split_annex_b(data);
    (!nals.is_empty() && nals.iter().all(|n| is_plausible_nal(n))).then_some(nals)
}

const START_CODE: [u8; 4] = [0, 0, 0, 1];

fn normalize(st: &mut State, data: &[u8]) -> Result<AccessUnit, FormatError> {
    if data.is_empty() {
        return Err(FormatError::Empty);
    }
    if data.len() > MAX_ACCESS_UNIT_BYTES {
        return Err(FormatError::TooLarge);
    }
    let nals =
        split_any(data, st.nal_length_size.unwrap_or(4)).ok_or(FormatError::UnknownFormat)?;
    let mut annex_b = Vec::with_capacity(data.len() + 4 * nals.len());
    let (mut has_idr, mut has_sps, mut has_pps) = (false, false, false);
    let mut sps = Vec::new();
    let mut pps = Vec::new();
    for nal in &nals {
        match nal[0] & 0x1f {
            5 => has_idr = true,
            7 => {
                has_sps = true;
                sps.push(nal.to_vec());
            }
            8 => {
                has_pps = true;
                pps.push(nal.to_vec());
            }
            _ => {}
        }
        annex_b.extend_from_slice(&START_CODE);
        annex_b.extend_from_slice(nal);
    }
    // Parameter sets inside the stream replace the cached ones.
    if has_sps {
        st.sps = sps;
    }
    if has_pps {
        st.pps = pps;
    }
    Ok(AccessUnit {
        annex_b,
        has_idr,
        has_sps,
        has_pps,
        leading_aud: nals[0][0] & 0x1f == 9,
    })
}

/// The unit with SPS/PPS in front if it lacks them. `None` if they are needed but unknown.
fn complete_key_frame(st: &State, au: AccessUnit) -> Option<Vec<u8>> {
    if au.has_sps && au.has_pps {
        return Some(au.annex_b);
    }
    if (!au.has_sps && st.sps.is_empty()) || (!au.has_pps && st.pps.is_empty()) {
        return None;
    }
    // Rebuild: delimiter first (it must stay first in the unit), then SPS and PPS (the cache
    // already holds any inline ones, see `normalize`), then everything else.
    let nals = split_annex_b(&au.annex_b);
    let mut out = Vec::with_capacity(au.annex_b.len() + 64);
    let mut push = |nal: &[u8]| {
        out.extend_from_slice(&START_CODE);
        out.extend_from_slice(nal);
    };
    if au.leading_aud {
        push(nals[0]);
    }
    st.sps.iter().for_each(|n| push(n));
    st.pps.iter().for_each(|n| push(n));
    for (i, n) in nals.iter().enumerate() {
        let t = n[0] & 0x1f;
        if t != 7 && t != 8 && !(i == 0 && au.leading_aud) {
            push(n);
        }
    }
    Some(out)
}

/// An `avcC` record (ISO 14496-15): version 1, profile, compatibility, level, `111111xx` with
/// the length size minus one, `111xxxxx` count of SPS, SPS (2-byte length each), count of PPS,
/// PPS (2-byte length each).
/// NAL units of one kind (all SPS, or all PPS).
type NalSets = Vec<Vec<u8>>;

fn parse_avcc_record(d: &[u8]) -> Option<(usize, NalSets, NalSets)> {
    if d.len() < 7 || d[0] != 1 {
        return None;
    }
    let length_size = usize::from(d[4] & 3) + 1;
    let mut pos = 5;
    let read_sets = |count: usize, pos: &mut usize| -> Option<Vec<Vec<u8>>> {
        let mut v = Vec::new();
        for _ in 0..count {
            let len = usize::from(u16::from_be_bytes([*d.get(*pos)?, *d.get(*pos + 1)?]));
            *pos += 2;
            let nal = d.get(*pos..*pos + len)?;
            if !is_plausible_nal(nal) {
                return None;
            }
            v.push(nal.to_vec());
            *pos += len;
        }
        Some(v)
    };
    let n_sps = usize::from(d[pos] & 0x1f);
    pos += 1;
    let sps = read_sets(n_sps, &mut pos)?;
    let n_pps = usize::from(*d.get(pos)?);
    pos += 1;
    let pps = read_sets(n_pps, &mut pos)?;
    Some((length_size, sps, pps))
}

fn apply_config(st: &mut State, data: &[u8]) -> bool {
    // An avcC record starts with the version byte 1; Annex B starts with a zero byte.
    if let Some((length_size, sps, pps)) = parse_avcc_record(data) {
        if sps.is_empty() && pps.is_empty() {
            return false;
        }
        st.nal_length_size = Some(length_size);
        if !sps.is_empty() {
            st.sps = sps;
        }
        if !pps.is_empty() {
            st.pps = pps;
        }
        return true;
    }
    let nals = split_annex_b(data);
    let sps: Vec<Vec<u8>> = nals
        .iter()
        .filter(|n| n[0] & 0x1f == 7)
        .map(|n| n.to_vec())
        .collect();
    let pps: Vec<Vec<u8>> = nals
        .iter()
        .filter(|n| n[0] & 0x1f == 8)
        .map(|n| n.to_vec())
        .collect();
    if sps.is_empty() && pps.is_empty() {
        return false;
    }
    if !sps.is_empty() {
        st.sps = sps;
    }
    if !pps.is_empty() {
        st.pps = pps;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPS: &[u8] = &[0x67, 0x42, 0xc0, 0x1f, 0xda, 0x05];
    const PPS: &[u8] = &[0x68, 0xce, 0x3c, 0x80];
    const IDR: &[u8] = &[0x65, 0x88, 0x84, 0x00, 0x10];
    const DELTA: &[u8] = &[0x41, 0x9a, 0x02, 0x03];

    fn annex_b(nals: &[&[u8]]) -> Vec<u8> {
        let mut v = Vec::new();
        for n in nals {
            v.extend_from_slice(&START_CODE);
            v.extend_from_slice(n);
        }
        v
    }

    fn avcc(nals: &[&[u8]], size: usize) -> Vec<u8> {
        let mut v = Vec::new();
        for n in nals {
            v.extend_from_slice(&(n.len() as u32).to_be_bytes()[4 - size..]);
            v.extend_from_slice(n);
        }
        v
    }

    fn avcc_record(length_size: usize) -> Vec<u8> {
        let mut v = vec![
            1,
            SPS[1],
            SPS[2],
            SPS[3],
            0xfc | (length_size as u8 - 1),
            0xe1,
        ];
        v.extend_from_slice(&(SPS.len() as u16).to_be_bytes());
        v.extend_from_slice(SPS);
        v.push(1);
        v.extend_from_slice(&(PPS.len() as u16).to_be_bytes());
        v.extend_from_slice(PPS);
        v
    }

    fn streaming(opts: ExternalOptions) -> ExternalInput {
        let e = ExternalInput::new(opts);
        e.begin_streaming();
        assert!(e.keyframe_requested(), "a new peer asks for a key frame");
        e
    }

    fn pop(e: &ExternalInput) -> Option<Unit> {
        e.state().queue.pop_front()
    }

    #[test]
    fn annex_b_with_three_and_four_byte_start_codes_is_normalised() {
        let mut data = vec![0, 0, 1];
        data.extend_from_slice(SPS);
        data.extend_from_slice(&[0, 0, 0, 1]);
        data.extend_from_slice(PPS);
        data.extend_from_slice(&[0, 0, 1]);
        data.extend_from_slice(IDR);
        let e = streaming(ExternalOptions::default());
        assert_eq!(e.push(&data, false, Some(0)), PushOutcome::Queued);
        let u = pop(&e).unwrap();
        assert_eq!(u.data, annex_b(&[SPS, PPS, IDR]));
        assert!(u.key, "an IDR slice makes it a key frame without the flag");
    }

    #[test]
    fn avcc_with_four_byte_lengths_becomes_annex_b_and_other_sizes_need_a_config() {
        let e = streaming(ExternalOptions::default());
        let data = avcc(&[SPS, PPS, IDR], 4);
        assert_eq!(e.push(&data, true, None), PushOutcome::Queued);
        assert_eq!(pop(&e).unwrap().data, annex_b(&[SPS, PPS, IDR]));
        for size in [3, 2, 1] {
            let e = streaming(ExternalOptions::default());
            assert!(e.set_config(&avcc_record(size)));
            let data = avcc(&[IDR], size);
            assert_eq!(
                e.push(&data, true, None),
                PushOutcome::Queued,
                "size {size}"
            );
            assert_eq!(pop(&e).unwrap().data, annex_b(&[SPS, PPS, IDR]));
        }
    }

    #[test]
    fn avcc_length_that_looks_like_a_start_code_is_not_mistaken() {
        // A 300-byte NAL with a 4-byte length: 00 00 01 2c, the same bytes as a 3-byte start
        // code followed by 0x2c.
        let mut nal = vec![0x41u8];
        nal.extend((0..299).map(|i| (i % 250 + 1) as u8));
        assert_eq!(nal.len(), 300);
        let data = avcc(&[&nal], 4);
        assert_eq!(&data[..4], &[0, 0, 1, 0x2c]);
        let e = streaming(ExternalOptions::default());
        e.set_config(&annex_b(&[SPS, PPS]));
        // Delta while a key frame is awaited: first a key frame.
        assert_eq!(e.push(&avcc(&[IDR], 4), true, None), PushOutcome::Queued);
        pop(&e);
        assert_eq!(e.push(&data, false, None), PushOutcome::Queued);
        assert_eq!(pop(&e).unwrap().data, annex_b(&[&nal]));
    }

    #[test]
    fn garbage_empty_and_oversize_are_invalid() {
        let e = streaming(ExternalOptions::default());
        assert_eq!(e.push(&[], true, None), PushOutcome::Invalid);
        assert_eq!(e.push(&[1, 2, 3, 4, 5], true, None), PushOutcome::Invalid);
        // Length prefix longer than the data.
        assert_eq!(
            e.push(&[0, 0, 0, 9, 0x65, 1, 2], true, None),
            PushOutcome::Invalid
        );
        // Forbidden bit set.
        assert_eq!(
            e.push(&annex_b(&[&[0xe5, 1, 2]]), true, None),
            PushOutcome::Invalid
        );
        let big = vec![0u8; MAX_ACCESS_UNIT_BYTES + 1];
        assert_eq!(e.push(&big, true, None), PushOutcome::Invalid);
        assert_eq!(e.stats().invalid, 5);
    }

    #[test]
    fn key_frames_get_the_cached_parameter_sets_in_front() {
        let e = streaming(ExternalOptions::default());
        // No parameter sets known yet: the key frame cannot be completed.
        assert_eq!(
            e.push(&annex_b(&[IDR]), true, None),
            PushOutcome::MissingParameterSets
        );
        e.set_config(&annex_b(&[SPS, PPS]));
        assert_eq!(e.push(&annex_b(&[IDR]), true, None), PushOutcome::Queued);
        assert_eq!(pop(&e).unwrap().data, annex_b(&[SPS, PPS, IDR]));
        // Only the PPS is missing.
        assert_eq!(
            e.push(&annex_b(&[SPS, IDR]), true, None),
            PushOutcome::Queued
        );
        assert_eq!(pop(&e).unwrap().data, annex_b(&[SPS, PPS, IDR]));
    }

    #[test]
    fn parameter_sets_go_after_a_leading_access_unit_delimiter() {
        let aud: &[u8] = &[0x09, 0xf0];
        let e = streaming(ExternalOptions::default());
        e.set_config(&annex_b(&[SPS, PPS]));
        assert_eq!(
            e.push(&annex_b(&[aud, IDR]), true, None),
            PushOutcome::Queued
        );
        assert_eq!(pop(&e).unwrap().data, annex_b(&[aud, SPS, PPS, IDR]));
    }

    #[test]
    fn avcc_config_record_sets_parameter_sets_and_length_size() {
        let e = streaming(ExternalOptions::default());
        assert!(e.set_config(&avcc_record(2)));
        // Two-byte lengths now preferred; the key frame has no parameter sets.
        assert_eq!(e.push(&avcc(&[IDR], 2), true, None), PushOutcome::Queued);
        assert_eq!(pop(&e).unwrap().data, annex_b(&[SPS, PPS, IDR]));
        assert!(!e.set_config(&[9, 9, 9]), "garbage is not a config");
    }

    #[test]
    fn inline_parameter_sets_update_the_cache() {
        let e = streaming(ExternalOptions::default());
        let sps2: &[u8] = &[0x67, 0x42, 0xc0, 0x28, 0xaa];
        assert_eq!(
            e.push(&annex_b(&[SPS, PPS, IDR]), true, None),
            PushOutcome::Queued
        );
        pop(&e);
        // A later unit brings a new SPS; a following bare key frame uses it.
        assert_eq!(
            e.push(&annex_b(&[sps2, PPS, IDR]), true, None),
            PushOutcome::Queued
        );
        pop(&e);
        assert_eq!(e.push(&annex_b(&[IDR]), true, None), PushOutcome::Queued);
        assert_eq!(pop(&e).unwrap().data, annex_b(&[sps2, PPS, IDR]));
    }

    #[test]
    fn nothing_is_accepted_before_a_peer_is_connected() {
        let e = ExternalInput::default();
        assert_eq!(
            e.push(&annex_b(&[SPS, PPS, IDR]), true, None),
            PushOutcome::NotStreaming
        );
        assert_eq!(e.stats().dropped_not_streaming, 1);
        assert_eq!(e.queue_len(), 0);
        // The parameter sets were still learned.
        e.begin_streaming();
        assert_eq!(e.push(&annex_b(&[IDR]), true, None), PushOutcome::Queued);
    }

    #[test]
    fn deltas_before_the_first_key_frame_are_dropped() {
        let e = streaming(ExternalOptions::default());
        assert_eq!(
            e.push(&annex_b(&[DELTA]), false, None),
            PushOutcome::WaitingForKeyFrame
        );
        assert_eq!(
            e.push(&annex_b(&[SPS, PPS, IDR]), true, None),
            PushOutcome::Queued
        );
        assert_eq!(e.push(&annex_b(&[DELTA]), false, None), PushOutcome::Queued);
        assert_eq!(e.stats().dropped_waiting_key, 1);
    }

    #[test]
    fn a_full_queue_drops_the_new_unit_counts_it_and_waits_for_a_key_frame() {
        let e = streaming(ExternalOptions {
            max_queue: 2,
            keyframe_min_interval: Duration::ZERO,
        });
        assert_eq!(
            e.push(&annex_b(&[SPS, PPS, IDR]), true, None),
            PushOutcome::Queued
        );
        assert_eq!(e.push(&annex_b(&[DELTA]), false, None), PushOutcome::Queued);
        assert_eq!(
            e.push(&annex_b(&[DELTA]), false, None),
            PushOutcome::QueueFull
        );
        assert_eq!(e.queue_len(), 2, "bounded");
        // The transport takes one; deltas are still refused until a key frame comes.
        pop(&e);
        assert_eq!(
            e.push(&annex_b(&[DELTA]), false, None),
            PushOutcome::WaitingForKeyFrame
        );
        assert!(e.keyframe_requested(), "the host is asked for a sync frame");
        // A new IDR frame replaces what was queued.
        assert_eq!(e.push(&annex_b(&[IDR]), true, None), PushOutcome::Queued);
        assert_eq!(e.queue_len(), 1);
        let s = e.stats();
        assert_eq!(s.dropped_queue_full, 1);
        assert_eq!(s.dropped_waiting_key, 1);
        assert_eq!(s.dropped_superseded, 1);
        assert_eq!(s.dropped(), 3);
        assert_eq!(s.queued, 3);
        assert_eq!(s.pushed, 5);
    }

    #[test]
    fn key_frame_requests_are_rate_limited_and_coalesced() {
        let e = ExternalInput::default();
        let t0 = Instant::now();
        assert!(e.request_keyframe_at(t0));
        // 999 more within half a second: all ignored (pending, then rate limit).
        let accepted = (1..1000u64)
            .filter(|i| e.request_keyframe_at(t0 + Duration::from_micros(i * 500)))
            .count();
        assert_eq!(accepted, 0);
        let s = e.stats();
        assert_eq!(s.keyframe_requests, 1000);
        assert_eq!(s.keyframe_requests_ignored, 999);
        assert!(e.keyframe_requested());
        assert!(!e.keyframe_requested(), "reading clears the flag");
        // Cleared, but the window still blocks; afterwards a request is accepted again.
        assert!(!e.request_keyframe_at(t0 + Duration::from_millis(900)));
        assert!(e.request_keyframe_at(t0 + Duration::from_millis(1001)));
    }

    #[test]
    fn a_key_frame_answers_the_pending_request() {
        let e = streaming(ExternalOptions::default());
        e.request_keyframe_at(Instant::now() + Duration::from_secs(5));
        assert_eq!(
            e.push(&annex_b(&[SPS, PPS, IDR]), true, None),
            PushOutcome::Queued
        );
        assert!(!e.keyframe_requested());
    }

    #[test]
    fn rtp_step_prefers_pts_and_clamps() {
        let wall = Duration::from_millis(40);
        assert_eq!(
            rtp_step(Some(1_000), Some(34_000), wall),
            Duration::from_micros(33_000)
        );
        assert_eq!(rtp_step(None, Some(5), wall), wall);
        assert_eq!(
            rtp_step(Some(9), Some(9), wall),
            wall,
            "no progress: wall clock"
        );
        assert_eq!(
            rtp_step(Some(0), Some(60_000_000), wall),
            Duration::from_secs(1)
        );
        assert_eq!(rtp_step(Some(0), Some(10), wall), Duration::from_millis(1));
    }

    #[tokio::test]
    async fn next_unit_waits_and_close_ends_it() {
        let e = streaming(ExternalOptions::default());
        let e2 = e.clone();
        let waiter = tokio::spawn(async move { e2.next_unit().await });
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert_eq!(
            e.push(&annex_b(&[SPS, PPS, IDR]), true, Some(7)),
            PushOutcome::Queued
        );
        let u = waiter.await.unwrap().unwrap();
        assert_eq!(u.pts_us, Some(7));
        let e3 = e.clone();
        let waiter = tokio::spawn(async move { e3.next_unit().await });
        tokio::time::sleep(Duration::from_millis(30)).await;
        e.close();
        assert!(waiter.await.unwrap().is_none());
        assert_eq!(e.push(&annex_b(&[IDR]), true, None), PushOutcome::Closed);
        assert!(!e.wait_keyframe_request().await);
    }

    #[tokio::test]
    async fn wait_keyframe_request_wakes_up() {
        let e = ExternalInput::default();
        let e2 = e.clone();
        let waiter = tokio::spawn(async move { e2.wait_keyframe_request().await });
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert!(e.request_keyframe());
        assert!(waiter.await.unwrap());
    }
}
