// SPDX-License-Identifier: GPL-3.0-or-later
//! Rate limits, failed-join counting and temporary bans.
//!
//! Clients are counted per IPv4 address or per IPv6 /64 network (one home or
//! office usually owns a whole /64, so counting single IPv6 addresses would be useless).
//! Failures and bans are also counted per IPv6 /48, because an attacker who owns a /48
//! can otherwise just move to the next /64.

use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, Ipv6Addr};
use std::num::NonZeroU32;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use governor::{DefaultKeyedRateLimiter, Quota, RateLimiter};

use crate::config::LimitsConfig;

type Keyed = DefaultKeyedRateLimiter<IpAddr>;

/// Why a request was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Deny {
    /// Too many requests; try again after this long.
    RateLimited(Duration),
    /// The client address is temporarily banned for this long.
    Banned(Duration),
    /// The server as a whole is full.
    Busy,
}

/// The address a client is counted under: IPv4 as is, IPv6 reduced to its /64.
pub(crate) fn limit_key(ip: IpAddr) -> IpAddr {
    mask_v6(ip, 4)
}

/// IPv6 reduced to its /48 (None for IPv4).
fn wide_key(ip: IpAddr) -> Option<IpAddr> {
    match ip {
        IpAddr::V4(_) => None,
        IpAddr::V6(_) => Some(mask_v6(ip, 3)),
    }
}

/// Keep the first `keep` 16-bit groups of an IPv6 address, zero the rest.
fn mask_v6(ip: IpAddr, keep: usize) -> IpAddr {
    match ip {
        IpAddr::V4(_) => ip,
        IpAddr::V6(v6) => {
            let mut seg = v6.segments();
            for s in &mut seg[keep..] {
                *s = 0;
            }
            IpAddr::V6(Ipv6Addr::from(seg))
        }
    }
}

/// Key of the failure table: (address or /64, is it the wide /48 entry?).
type StateKey = (IpAddr, bool);

/// What is remembered about one client address.
#[derive(Debug, Default)]
struct IpState {
    /// Failures in the current window.
    failures: u32,
    /// When the current failure window started.
    window_start: Option<Instant>,
    /// Banned until this moment.
    banned_until: Option<Instant>,
    /// How many bans this address had (picks the ban length).
    ban_level: usize,
    /// When the last ban ended or was set (for forgetting the history).
    last_ban: Option<Instant>,
}

/// The failure table and the time of its last full sweep.
#[derive(Default)]
struct StateTable {
    map: HashMap<StateKey, IpState>,
    /// When `retain` last ran over the whole table. Sweeping 200 000 entries while
    /// holding the lock is costly, so it is done at most once per `SWEEP_EVERY`.
    last_sweep: Option<Instant>,
}

/// Counts of recent events within the last minute.
#[derive(Default)]
struct EventWindow {
    times: VecDeque<Instant>,
}

impl EventWindow {
    fn push(&mut self, now: Instant) {
        self.times.push_back(now);
        while self.times.len() > EVENT_CAP {
            self.times.pop_front();
        }
    }

    fn count(&mut self, now: Instant) -> usize {
        while self
            .times
            .front()
            .is_some_and(|t| now.duration_since(*t) > EVENT_WINDOW)
        {
            self.times.pop_front();
        }
        self.times.len()
    }
}

/// All limit state of the server.
pub(crate) struct Guard {
    cfg: LimitsConfig,
    connections: Keyed,
    create: Keyed,
    join: Keyed,
    http: Keyed,
    ice: Keyed,
    state: Mutex<StateTable>,
    per_ip_connections: Mutex<HashMap<IpAddr, usize>>,
    /// All bad events (for the ALTCHA difficulty).
    events: Mutex<EventWindow>,
    /// Failed joins of the whole server (for the global brake).
    failures: Mutex<EventWindow>,
    open_connections: AtomicUsize,
    /// Start of the guard; `last_housekeeping_ms` counts from here.
    epoch: Instant,
    /// Milliseconds after `epoch` of the last housekeeping run.
    last_housekeeping_ms: AtomicU64,
    /// Full sweeps of the failure table (a metric, used by tests).
    sweeps: AtomicU64,
    /// Failures that could not be tracked because the table was full.
    untracked: AtomicU64,
}

/// Held by a connection while it is open; frees its place when dropped.
pub(crate) struct ConnSlot {
    guard: Arc<Guard>,
    key: IpAddr,
}

impl Drop for ConnSlot {
    fn drop(&mut self) {
        self.guard.open_connections.fetch_sub(1, Ordering::Relaxed);
        let mut map = self
            .guard
            .per_ip_connections
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(n) = map.get_mut(&self.key) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                map.remove(&self.key);
            }
        }
    }
}

/// Entries older than this many are cleaned out when the table grows.
const STATE_SOFT_LIMIT: usize = 50_000;
/// The table never grows beyond this; further new addresses are not tracked.
/// UNCERTAIN: placeholder.
const STATE_HARD_LIMIT: usize = 200_000;
/// How long a bad event counts as "recent" for the difficulty.
const EVENT_WINDOW: Duration = Duration::from_secs(60);
/// Remembered events are capped to keep memory small. `global_failed_joins_per_min`
/// must not be above this, or the global brake could never trigger (checked in
/// `ServerConfig::validate`).
pub(crate) const EVENT_CAP: usize = 10_000;
/// The failure table is swept at most this often (the sweep locks the table).
const SWEEP_EVERY: Duration = Duration::from_secs(1);
/// The keyed rate limiters are pruned on this timer.
/// UNCERTAIN: 10 seconds is a placeholder.
const HOUSEKEEPING_EVERY: Duration = Duration::from_secs(10);

fn per_minute(n: u32) -> Keyed {
    RateLimiter::keyed(Quota::per_minute(
        NonZeroU32::new(n.max(1)).unwrap_or(NonZeroU32::MIN),
    ))
}

fn retry_after(per_min: u32) -> Duration {
    // One "cell" is refilled every 60/n seconds; round up to whole seconds, at least 1.
    Duration::from_secs(u64::from(60u32.div_ceil(per_min.max(1))).max(1))
}

impl Guard {
    pub(crate) fn new(cfg: LimitsConfig) -> Arc<Self> {
        Arc::new(Self {
            connections: per_minute(cfg.connections_per_min),
            create: per_minute(cfg.create_room_per_min),
            join: per_minute(cfg.join_attempts_per_min),
            http: per_minute(cfg.http_per_min),
            ice: per_minute(cfg.ice_per_min),
            cfg,
            state: Mutex::new(StateTable::default()),
            per_ip_connections: Mutex::new(HashMap::new()),
            events: Mutex::new(EventWindow::default()),
            failures: Mutex::new(EventWindow::default()),
            open_connections: AtomicUsize::new(0),
            epoch: Instant::now(),
            last_housekeeping_ms: AtomicU64::new(0),
            sweeps: AtomicU64::new(0),
            untracked: AtomicU64::new(0),
        })
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, StateTable> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Every `HOUSEKEEPING_EVERY` (a timer, not a call counter): prune the keyed limiters
    /// and drop memory about addresses that are no longer interesting.
    fn housekeeping(&self) {
        let ms_now = u64::try_from(self.epoch.elapsed().as_millis()).unwrap_or(u64::MAX);
        let last = self.last_housekeeping_ms.load(Ordering::Relaxed);
        let every = u64::try_from(HOUSEKEEPING_EVERY.as_millis()).unwrap_or(u64::MAX);
        if ms_now.saturating_sub(last) < every
            || self
                .last_housekeeping_ms
                .compare_exchange(last, ms_now, Ordering::Relaxed, Ordering::Relaxed)
                .is_err()
        {
            return;
        }
        for l in [
            &self.connections,
            &self.create,
            &self.join,
            &self.http,
            &self.ice,
        ] {
            l.retain_recent();
            l.shrink_to_fit();
        }
        let now = Instant::now();
        let mut table = self.lock_state();
        if table.map.len() > STATE_SOFT_LIMIT {
            self.sweep(&mut table, now);
        }
    }

    /// Drop table entries that no longer matter. At most once per `SWEEP_EVERY`;
    /// returns whether the sweep ran.
    fn sweep(&self, table: &mut StateTable, now: Instant) -> bool {
        if table
            .last_sweep
            .is_some_and(|t| now.saturating_duration_since(t) < SWEEP_EVERY)
        {
            return false;
        }
        table.last_sweep = Some(now);
        self.sweeps.fetch_add(1, Ordering::Relaxed);
        table
            .map
            .retain(|_, s| Self::still_relevant(&self.cfg, s, now));
        true
    }

    fn still_relevant(cfg: &LimitsConfig, s: &IpState, now: Instant) -> bool {
        let banned = s.banned_until.is_some_and(|u| u > now);
        let window_open = s
            .window_start
            .is_some_and(|w| now.duration_since(w) < cfg.failed_join_window);
        let remembered = s
            .last_ban
            .is_some_and(|b| now.duration_since(b) < cfg.ban_memory);
        banned || window_open || remembered
    }

    /// Record that something bad happened (for the difficulty).
    pub(crate) fn note_event(&self, now: Instant) {
        self.events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(now);
    }

    /// Number of bad events in the last minute.
    pub(crate) fn recent_events(&self, now: Instant) -> usize {
        self.events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .count(now)
    }

    /// How many failures could not be tracked because the table was full (a metric).
    #[cfg(test)]
    pub(crate) fn untracked_failures(&self) -> u64 {
        self.untracked.load(Ordering::Relaxed)
    }

    /// Does this client currently have failures in the window?
    pub(crate) fn has_recent_failures(&self, ip: IpAddr, now: Instant) -> bool {
        let key = (limit_key(ip), false);
        self.lock_state().map.get(&key).is_some_and(|s| {
            s.failures > 0
                && s.window_start
                    .is_some_and(|w| now.duration_since(w) < self.cfg.failed_join_window)
        })
    }

    /// Remaining ban time, if this client (or its IPv6 /48) is banned.
    pub(crate) fn banned_for(&self, ip: IpAddr, now: Instant) -> Option<Duration> {
        let table = self.lock_state();
        let left = |key: StateKey| {
            table
                .map
                .get(&key)
                .and_then(|s| s.banned_until)
                .and_then(|until| until.checked_duration_since(now))
        };
        let narrow = left((limit_key(ip), false));
        let wide = wide_key(ip).and_then(|w| left((w, true)));
        narrow.max(wide).filter(|d| !d.is_zero())
    }

    /// A new WebSocket connection: banned? too fast? too many from this address? server full?
    /// On success returns a slot that must be kept while the connection is open.
    pub(crate) fn admit_connection(
        self: &Arc<Self>,
        ip: IpAddr,
        now: Instant,
    ) -> Result<ConnSlot, Deny> {
        self.housekeeping();
        if let Some(left) = self.banned_for(ip, now) {
            return Err(Deny::Banned(left));
        }
        let key = limit_key(ip);
        if self.connections.check_key(&key).is_err() {
            self.note_event(now);
            return Err(Deny::RateLimited(retry_after(self.cfg.connections_per_min)));
        }
        {
            let mut map = self
                .per_ip_connections
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let n = map.entry(key).or_insert(0);
            if *n >= self.cfg.max_connections_per_ip {
                if *n == 0 {
                    map.remove(&key);
                }
                self.note_event(now);
                return Err(Deny::RateLimited(Duration::from_secs(5)));
            }
            *n += 1;
        }
        // Reserve a place, undo if the server is full.
        let before = self.open_connections.fetch_add(1, Ordering::Relaxed);
        let slot = ConnSlot {
            guard: self.clone(),
            key,
        };
        if before >= self.cfg.max_connections {
            // Dropping the slot gives both counters back.
            drop(slot);
            return Err(Deny::Busy);
        }
        Ok(slot)
    }

    /// Number of open connections (used by tests).
    #[cfg(test)]
    pub(crate) fn open_connections(&self) -> usize {
        self.open_connections.load(Ordering::Relaxed)
    }

    /// `create_room` from this client (banned? too many?).
    pub(crate) fn check_create(&self, ip: IpAddr, now: Instant) -> Result<(), Deny> {
        self.housekeeping();
        if let Some(left) = self.banned_for(ip, now) {
            return Err(Deny::Banned(left));
        }
        if self.create.check_key(&limit_key(ip)).is_err() {
            self.note_event(now);
            return Err(Deny::RateLimited(retry_after(self.cfg.create_room_per_min)));
        }
        Ok(())
    }

    /// `join_room` from this client (banned? too many attempts? global brake?).
    pub(crate) fn check_join(&self, ip: IpAddr, now: Instant) -> Result<(), Deny> {
        self.housekeeping();
        if let Some(left) = self.banned_for(ip, now) {
            return Err(Deny::Banned(left));
        }
        let global = self
            .failures
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .count(now);
        if global >= self.cfg.global_failed_joins_per_min as usize {
            self.note_event(now);
            return Err(Deny::RateLimited(Duration::from_secs(10)));
        }
        if self.join.check_key(&limit_key(ip)).is_err() {
            self.note_event(now);
            return Err(Deny::RateLimited(retry_after(
                self.cfg.join_attempts_per_min,
            )));
        }
        Ok(())
    }

    /// A plain HTTP request to `/altcha/challenge`.
    pub(crate) fn check_http(&self, ip: IpAddr, now: Instant) -> Result<(), Deny> {
        self.check_keyed(&self.http, self.cfg.http_per_min, ip, now)
    }

    /// A request to `/ice-servers`: its own bucket, separate from `check_http`.
    pub(crate) fn check_ice(&self, ip: IpAddr, now: Instant) -> Result<(), Deny> {
        self.check_keyed(&self.ice, self.cfg.ice_per_min, ip, now)
    }

    fn check_keyed(
        &self,
        bucket: &Keyed,
        per_min: u32,
        ip: IpAddr,
        now: Instant,
    ) -> Result<(), Deny> {
        self.housekeeping();
        if let Some(left) = self.banned_for(ip, now) {
            return Err(Deny::Banned(left));
        }
        if bucket.check_key(&limit_key(ip)).is_err() {
            self.note_event(now);
            return Err(Deny::RateLimited(retry_after(per_min)));
        }
        Ok(())
    }

    /// A join failed (unknown code, full room, malformed code). Counts it harder against
    /// the join limit and bans the client after too many. Returns `Some(duration)` when
    /// the client was banned just now.
    pub(crate) fn record_failed_join(&self, ip: IpAddr, now: Instant) -> Option<Duration> {
        // The failed attempt costs `weight` attempts in total; one was already paid.
        // If not enough is left, nothing is taken, but the client is then nearly
        // blocked anyway.
        if let Some(extra) = NonZeroU32::new(self.cfg.failed_join_weight.saturating_sub(1)) {
            let _ = self.join.check_key_n(&limit_key(ip), extra);
        }
        // Only real failed joins (code guessing) feed the server-wide brake.
        self.failures
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(now);
        self.record_failure(ip, now)
    }

    /// Any kind of abuse (failed join, message spam, too fast sending, refused join).
    /// Counts toward a ban of the address and of its IPv6 /48. Not toward the global brake:
    /// that one counts failed joins only (`record_failed_join`).
    pub(crate) fn record_failure(&self, ip: IpAddr, now: Instant) -> Option<Duration> {
        self.note_event(now);
        let narrow = self.fail_one((limit_key(ip), false), self.cfg.ban_after_failed_joins, now);
        let wide = wide_key(ip).and_then(|w| {
            self.fail_one(
                (w, true),
                self.cfg
                    .ban_after_failed_joins
                    .saturating_mul(self.cfg.wide_failure_factor),
                now,
            )
        });
        narrow.or(wide)
    }

    /// Count one failure in one table entry; ban when `threshold` is reached.
    fn fail_one(&self, key: StateKey, threshold: u32, now: Instant) -> Option<Duration> {
        let mut table = self.lock_state();
        if !table.map.contains_key(&key) && table.map.len() >= STATE_HARD_LIMIT {
            // Table full: drop entries that no longer matter (rate-limited, because a
            // sweep over the whole table under the lock is expensive), else do not track
            // this one.
            self.sweep(&mut table, now);
            if table.map.len() >= STATE_HARD_LIMIT {
                self.untracked.fetch_add(1, Ordering::Relaxed);
                return None;
            }
        }
        let s = table.map.entry(key).or_default();
        let window = self.cfg.failed_join_window;
        if s.window_start
            .is_none_or(|w| now.duration_since(w) >= window)
        {
            s.window_start = Some(now);
            s.failures = 0;
        }
        s.failures += 1;
        if s.failures < threshold {
            return None;
        }
        // Ban. Forget an old history first.
        if s.last_ban
            .is_some_and(|b| now.duration_since(b) >= self.cfg.ban_memory)
        {
            s.ban_level = 0;
        }
        let idx = s
            .ban_level
            .min(self.cfg.ban_durations.len().saturating_sub(1));
        let len = self.cfg.ban_durations.get(idx).copied().unwrap_or_default();
        s.ban_level += 1;
        s.banned_until = Some(now + len);
        s.last_ban = Some(now + len);
        s.failures = 0;
        s.window_start = None;
        Some(len)
    }

    /// Server-wide cap on rooms (checked together with the room table).
    pub(crate) fn max_rooms(&self) -> usize {
        self.cfg.max_rooms
    }

    /// Per-address cap on rooms.
    pub(crate) fn max_rooms_per_ip(&self) -> usize {
        self.cfg.max_rooms_per_ip
    }

    /// The limits (timeouts etc.) for the connection loop.
    pub(crate) fn cfg(&self) -> &LimitsConfig {
        &self.cfg
    }
}

/// A token bucket: `rate` tokens per second up to `capacity`.
struct Bucket {
    capacity: f64,
    rate: f64,
    tokens: f64,
    last: Instant,
}

impl Bucket {
    fn new(rate: u32, capacity: u32, now: Instant) -> Self {
        Self {
            capacity: f64::from(capacity),
            rate: f64::from(rate),
            tokens: f64::from(capacity),
            last: now,
        }
    }

    /// Take `n` tokens; false (and nothing taken) if not enough are left.
    fn take(&mut self, n: f64, now: Instant) -> bool {
        let dt = now.saturating_duration_since(self.last).as_secs_f64();
        self.last = now;
        self.tokens = (self.tokens + dt * self.rate).min(self.capacity);
        if self.tokens >= n {
            self.tokens -= n;
            true
        } else {
            false
        }
    }
}

/// How much one connection may send: messages per second and bytes per second.
pub(crate) struct ConnBudget {
    msgs: Bucket,
    bytes: Bucket,
}

impl ConnBudget {
    pub(crate) fn new(cfg: &LimitsConfig, now: Instant) -> Self {
        Self {
            msgs: Bucket::new(cfg.msg_rate_per_sec, cfg.msg_burst, now),
            bytes: Bucket::new(cfg.bytes_per_sec, cfg.bytes_burst, now),
        }
    }

    /// May a message of `len` bytes pass now? Both buckets must have room.
    pub(crate) fn allow(&mut self, len: usize, now: Instant) -> bool {
        // Bytes are only taken if the message count also fits, and vice versa.
        let m = self.msgs.take(1.0, now);
        let b = self.bytes.take(len as f64, now);
        m && b
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LimitsConfig, Mode};

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    fn limits() -> LimitsConfig {
        LimitsConfig {
            connections_per_min: 3,
            create_room_per_min: 2,
            join_attempts_per_min: 6,
            failed_join_weight: 3,
            http_per_min: 2,
            ice_per_min: 2,
            max_rooms: 10,
            max_connections: 2,
            ban_after_failed_joins: 3,
            failed_join_window: Duration::from_secs(600),
            ban_durations: vec![Duration::from_secs(60), Duration::from_secs(300)],
            ban_memory: Duration::from_secs(3600),
            max_connections_per_ip: 100,
            global_failed_joins_per_min: 1000,
            ..LimitsConfig::for_mode(Mode::Internal)
        }
    }

    #[test]
    fn ipv6_is_counted_per_slash_64_and_48() {
        assert_eq!(
            limit_key(ip("2001:db8:1:2:aaaa:bbbb:cccc:dddd")),
            ip("2001:db8:1:2::")
        );
        assert_eq!(
            limit_key(ip("2001:db8:1:2::1")),
            limit_key(ip("2001:db8:1:2::2"))
        );
        assert_ne!(
            limit_key(ip("2001:db8:1:2::1")),
            limit_key(ip("2001:db8:1:3::1"))
        );
        assert_eq!(limit_key(ip("192.0.2.1")), ip("192.0.2.1"));
        assert_eq!(wide_key(ip("2001:db8:1:2::1")), Some(ip("2001:db8:1::")));
        assert_eq!(wide_key(ip("192.0.2.1")), None);
    }

    #[test]
    fn create_room_rate_limit_and_ban_check() {
        let g = Guard::new(limits());
        let now = Instant::now();
        assert!(g.check_create(ip("192.0.2.1"), now).is_ok());
        assert!(g.check_create(ip("192.0.2.1"), now).is_ok());
        assert!(matches!(
            g.check_create(ip("192.0.2.1"), now),
            Err(Deny::RateLimited(_))
        ));
        assert!(g.check_create(ip("192.0.2.2"), now).is_ok());
        // A banned address cannot create rooms either.
        for _ in 0..3 {
            g.record_failure(ip("192.0.2.3"), now);
        }
        assert!(matches!(
            g.check_create(ip("192.0.2.3"), now),
            Err(Deny::Banned(_))
        ));
    }

    #[test]
    fn http_rate_limit() {
        let g = Guard::new(limits());
        let now = Instant::now();
        assert!(g.check_http(ip("192.0.2.1"), now).is_ok());
        assert!(g.check_http(ip("192.0.2.1"), now).is_ok());
        assert!(g.check_http(ip("192.0.2.1"), now).is_err());
    }

    #[test]
    fn ice_servers_have_their_own_bucket() {
        let g = Guard::new(limits());
        let now = Instant::now();
        let a = ip("192.0.2.1");
        // Use up the challenge bucket: the ICE list is still served.
        assert!(g.check_http(a, now).is_ok());
        assert!(g.check_http(a, now).is_ok());
        assert!(g.check_http(a, now).is_err());
        assert!(g.check_ice(a, now).is_ok());
        assert!(g.check_ice(a, now).is_ok());
        assert!(g.check_ice(a, now).is_err());
        // And the other way round for another address.
        let b = ip("192.0.2.2");
        assert!(g.check_ice(b, now).is_ok());
        assert!(g.check_ice(b, now).is_ok());
        assert!(g.check_ice(b, now).is_err());
        assert!(g.check_http(b, now).is_ok());
    }

    #[test]
    fn connection_rate_and_global_cap() {
        let g = Guard::new(limits());
        let now = Instant::now();
        let a = g.admit_connection(ip("192.0.2.1"), now).unwrap();
        let b = g.admit_connection(ip("192.0.2.2"), now).unwrap();
        assert_eq!(
            g.admit_connection(ip("192.0.2.3"), now).err(),
            Some(Deny::Busy)
        );
        assert_eq!(g.open_connections(), 2);
        drop(a);
        assert_eq!(g.open_connections(), 1);
        assert!(g.admit_connection(ip("192.0.2.3"), now).is_ok());
        drop(b);
        let g = Guard::new(LimitsConfig {
            max_connections: 100,
            ..limits()
        });
        let ok = (0..3)
            .map(|_| g.admit_connection(ip("192.0.2.9"), now))
            .filter(Result::is_ok)
            .count();
        assert_eq!(ok, 3);
        assert!(matches!(
            g.admit_connection(ip("192.0.2.9"), now),
            Err(Deny::RateLimited(_))
        ));
    }

    #[test]
    fn concurrent_connections_per_ip_are_capped_and_freed() {
        let g = Guard::new(LimitsConfig {
            max_connections: 100,
            connections_per_min: 1000,
            max_connections_per_ip: 2,
            ..limits()
        });
        let now = Instant::now();
        let a = g.admit_connection(ip("192.0.2.1"), now).unwrap();
        let _b = g.admit_connection(ip("192.0.2.1"), now).unwrap();
        assert!(matches!(
            g.admit_connection(ip("192.0.2.1"), now),
            Err(Deny::RateLimited(_))
        ));
        // Another address is fine, and the whole /64 of an IPv6 client counts as one.
        assert!(g.admit_connection(ip("192.0.2.2"), now).is_ok());
        let _c = g.admit_connection(ip("2001:db8::1"), now).unwrap();
        let _d = g.admit_connection(ip("2001:db8::2"), now).unwrap();
        assert!(g.admit_connection(ip("2001:db8::3"), now).is_err());
        // A closed connection frees its place.
        drop(a);
        assert!(g.admit_connection(ip("192.0.2.1"), now).is_ok());
    }

    #[test]
    fn failed_joins_count_harder_than_good_ones() {
        let g = Guard::new(LimitsConfig {
            ban_after_failed_joins: 100,
            ..limits()
        });
        let now = Instant::now();
        let who = ip("192.0.2.1");
        let mut allowed = 0;
        while g.check_join(who, now).is_ok() {
            g.record_failed_join(who, now);
            allowed += 1;
            assert!(allowed < 10, "must stop");
        }
        assert_eq!(allowed, 2);
        let g = Guard::new(limits());
        let good = (0..10).filter(|_| g.check_join(who, now).is_ok()).count();
        assert_eq!(good, 6);
    }

    #[test]
    fn repeated_failures_cause_escalating_bans() {
        let g = Guard::new(LimitsConfig {
            join_attempts_per_min: 1000,
            ..limits()
        });
        let t0 = Instant::now();
        let who = ip("192.0.2.1");
        assert_eq!(g.record_failed_join(who, t0), None);
        assert_eq!(g.record_failed_join(who, t0), None);
        assert_eq!(g.record_failed_join(who, t0), Some(Duration::from_secs(60)));
        assert!(matches!(g.check_join(who, t0), Err(Deny::Banned(_))));
        assert!(g.admit_connection(who, t0).is_err());
        assert!(g.banned_for(who, t0 + Duration::from_secs(59)).is_some());
        assert!(g.check_join(ip("192.0.2.2"), t0).is_ok());
        let t1 = t0 + Duration::from_secs(61);
        assert!(g.check_join(who, t1).is_ok());
        for _ in 0..2 {
            assert_eq!(g.record_failed_join(who, t1), None);
        }
        assert_eq!(
            g.record_failed_join(who, t1),
            Some(Duration::from_secs(300))
        );
        let t2 = t1 + Duration::from_secs(301);
        for _ in 0..2 {
            g.record_failed_join(who, t2);
        }
        assert_eq!(
            g.record_failed_join(who, t2),
            Some(Duration::from_secs(300))
        );
        let t3 = t2 + Duration::from_secs(300 + 3601);
        for _ in 0..2 {
            g.record_failed_join(who, t3);
        }
        assert_eq!(g.record_failed_join(who, t3), Some(Duration::from_secs(60)));
    }

    #[test]
    fn failures_outside_the_window_do_not_add_up() {
        let g = Guard::new(limits());
        let t0 = Instant::now();
        let who = ip("192.0.2.1");
        g.record_failed_join(who, t0);
        g.record_failed_join(who, t0);
        let later = t0 + Duration::from_secs(601);
        assert_eq!(g.record_failed_join(who, later), None);
        assert!(g.has_recent_failures(who, later));
        assert!(!g.has_recent_failures(who, later + Duration::from_secs(700)));
    }

    #[test]
    fn moving_between_slash_64s_does_not_escape_the_ban() {
        // Threshold 3 per /64, 12 per /48. Every failure comes from a new /64 of one /48.
        let g = Guard::new(LimitsConfig {
            wide_failure_factor: 4,
            ..limits()
        });
        let t0 = Instant::now();
        let mut banned = None;
        for i in 0..12u16 {
            let who: IpAddr = format!("2001:db8:1:{i}::1").parse().unwrap();
            banned = g.record_failure(who, t0);
        }
        assert_eq!(banned, Some(Duration::from_secs(60)));
        // The whole /48 is banned, including /64s that never failed.
        assert!(g.banned_for(ip("2001:db8:1:ffff::5"), t0).is_some());
        assert!(g.banned_for(ip("2001:db8:2::5"), t0).is_none());
    }

    #[test]
    fn global_failed_join_brake_slows_everybody() {
        let g = Guard::new(LimitsConfig {
            global_failed_joins_per_min: 5,
            ban_after_failed_joins: 1000,
            join_attempts_per_min: 1000,
            ..limits()
        });
        let now = Instant::now();
        // Five different addresses fail once each.
        for i in 1..=5 {
            let a: IpAddr = format!("192.0.2.{i}").parse().unwrap();
            assert!(g.check_join(a, now).is_ok());
            g.record_failed_join(a, now);
        }
        // Now even a new, innocent address is slowed down.
        assert!(matches!(
            g.check_join(ip("192.0.2.99"), now),
            Err(Deny::RateLimited(_))
        ));
        // A minute later it is over.
        assert!(
            g.check_join(ip("192.0.2.99"), now + Duration::from_secs(61))
                .is_ok()
        );
    }

    #[test]
    fn other_failures_do_not_feed_the_global_brake() {
        let g = Guard::new(LimitsConfig {
            global_failed_joins_per_min: 5,
            ban_after_failed_joins: 1000,
            join_attempts_per_min: 1000,
            ..limits()
        });
        let now = Instant::now();
        // Bad messages, floods, refused joins: many, from many addresses.
        for i in 1..=50 {
            let a: IpAddr = format!("192.0.2.{i}").parse().unwrap();
            g.record_failure(a, now);
        }
        assert!(g.check_join(ip("192.0.2.99"), now).is_ok());
    }

    #[test]
    fn event_window_forgets_old_events() {
        let g = Guard::new(LimitsConfig::for_mode(Mode::Open));
        let t0 = Instant::now();
        g.note_event(t0);
        g.note_event(t0);
        assert_eq!(g.recent_events(t0), 2);
        assert_eq!(g.recent_events(t0 + Duration::from_secs(61)), 0);
    }

    #[test]
    fn failure_table_has_a_hard_cap() {
        let g = Guard::new(LimitsConfig {
            failed_join_window: Duration::from_secs(600),
            ..limits()
        });
        let now = Instant::now();
        // Fill the table to its limit with live entries.
        {
            let mut table = g.lock_state();
            for i in 0..STATE_HARD_LIMIT {
                let ip = IpAddr::V6(Ipv6Addr::from((i as u128 + 1) << 64));
                table.map.insert(
                    (ip, false),
                    IpState {
                        failures: 1,
                        window_start: Some(now),
                        ..IpState::default()
                    },
                );
            }
        }
        // A new address is not tracked (and counted in the metric) ...
        assert_eq!(g.record_failure(ip("192.0.2.1"), now), None);
        assert!(g.untracked_failures() >= 1);
        assert_eq!(g.lock_state().map.len(), STATE_HARD_LIMIT);
        // ... but once old entries expire, room is made again.
        let later = now + Duration::from_secs(700);
        g.record_failure(ip("192.0.2.1"), later);
        assert!(g.lock_state().map.len() < STATE_HARD_LIMIT);
    }

    #[test]
    fn full_table_is_swept_at_most_once_per_second() {
        let g = Guard::new(LimitsConfig {
            ban_after_failed_joins: u32::MAX,
            ..limits()
        });
        let now = Instant::now();
        {
            let mut table = g.lock_state();
            for i in 0..STATE_HARD_LIMIT {
                let ip = IpAddr::V6(Ipv6Addr::from((i as u128 + 1) << 64));
                table.map.insert(
                    (ip, false),
                    IpState {
                        failures: 1,
                        window_start: Some(now),
                        ..IpState::default()
                    },
                );
            }
        }
        // 500 failures of new addresses in the same second: none fits, but the 200 000
        // entries were walked only once, not 500 times.
        for i in 0..500u32 {
            let a = IpAddr::from(std::net::Ipv4Addr::from(0xC000_0200 + i));
            assert_eq!(g.record_failure(a, now), None);
        }
        assert_eq!(g.untracked_failures(), 500);
        assert_eq!(g.sweeps.load(Ordering::Relaxed), 1);
        // A second later it may sweep again.
        g.record_failure(ip("198.51.100.1"), now + Duration::from_millis(1500));
        assert_eq!(g.sweeps.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn wide_threshold_is_configurable() {
        // Threshold 3 per /64; with factor 2 a /48 bans at 6 failures from different /64s.
        let g = Guard::new(LimitsConfig {
            wide_failure_factor: 2,
            ..limits()
        });
        let t0 = Instant::now();
        let mut banned = None;
        for i in 0..6u16 {
            let who: IpAddr = format!("2001:db8:1:{i}::1").parse().unwrap();
            assert!(banned.is_none(), "banned too early");
            banned = g.record_failure(who, t0);
        }
        assert!(banned.is_some());
        let g = Guard::new(LimitsConfig {
            wide_failure_factor: 100,
            ..limits()
        });
        for i in 0..12u16 {
            let who: IpAddr = format!("2001:db8:1:{i}::1").parse().unwrap();
            assert_eq!(g.record_failure(who, t0), None);
        }
    }

    #[test]
    fn limiters_are_pruned_on_a_timer_not_a_call_count() {
        let g = Guard::new(limits());
        // Within the timer interval of creation: many calls, no pruning run.
        for _ in 0..3000 {
            g.housekeeping();
        }
        assert_eq!(g.last_housekeeping_ms.load(Ordering::Relaxed), 0);
        // A guard that is older than the interval prunes on the next call, once.
        let old = Guard {
            epoch: Instant::now() - HOUSEKEEPING_EVERY - Duration::from_secs(1),
            ..Arc::try_unwrap(Guard::new(limits())).ok().unwrap()
        };
        old.housekeeping();
        let stamp = old.last_housekeeping_ms.load(Ordering::Relaxed);
        assert!(stamp >= 10_000, "{stamp}");
        old.housekeeping();
        assert_eq!(old.last_housekeeping_ms.load(Ordering::Relaxed), stamp);
    }

    #[test]
    fn connection_budget_limits_messages_and_bytes() {
        let cfg = LimitsConfig {
            msg_rate_per_sec: 2,
            msg_burst: 3,
            bytes_per_sec: 100,
            bytes_burst: 250,
            ..limits()
        };
        let t0 = Instant::now();
        let mut b = ConnBudget::new(&cfg, t0);
        // Burst of 3 small messages passes, the 4th does not.
        assert!(b.allow(10, t0) && b.allow(10, t0) && b.allow(10, t0));
        assert!(!b.allow(10, t0));
        // After 1 s: 2 more messages are allowed.
        let t1 = t0 + Duration::from_secs(1);
        assert!(b.allow(10, t1) && b.allow(10, t1));
        assert!(!b.allow(10, t1));
        // Bytes: a big message uses the byte budget even when the count is fine.
        let mut b = ConnBudget::new(&cfg, t0);
        assert!(b.allow(200, t0));
        assert!(!b.allow(200, t0), "only 50 bytes left");
        assert!(b.allow(200, t0 + Duration::from_secs(2)), "refilled");
    }
}
