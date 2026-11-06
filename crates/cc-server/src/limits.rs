// SPDX-License-Identifier: GPL-3.0-or-later
//! Rate limits, failed-join counting and temporary bans.
//!
//! Clients are counted per IPv4 address or per IPv6 /64 network (one home or
//! school usually owns a whole /64, so counting single IPv6 addresses would be useless).

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
    match ip {
        IpAddr::V4(_) => ip,
        IpAddr::V6(v6) => {
            let mut seg = v6.segments();
            for s in &mut seg[4..] {
                *s = 0;
            }
            IpAddr::V6(Ipv6Addr::from(seg))
        }
    }
}

/// What is remembered about one client address.
#[derive(Debug, Default)]
struct IpState {
    /// Failed joins in the current window.
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

/// Counts of recent bad events, for the adaptive ALTCHA difficulty.
#[derive(Default)]
struct EventWindow {
    times: VecDeque<Instant>,
}

/// All limit state of the server.
pub(crate) struct Guard {
    cfg: LimitsConfig,
    connections: Keyed,
    create: Keyed,
    join: Keyed,
    http: Keyed,
    state: Mutex<HashMap<IpAddr, IpState>>,
    events: Mutex<EventWindow>,
    open_connections: AtomicUsize,
    calls: AtomicU64,
}

/// Held by a connection while it is open; frees its place when dropped.
pub(crate) struct ConnSlot(Arc<Guard>);

impl Drop for ConnSlot {
    fn drop(&mut self) {
        self.0.open_connections.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Entries older than this many are cleaned out when the table grows.
const STATE_SOFT_LIMIT: usize = 50_000;
/// How long a bad event counts as "recent" for the difficulty.
const EVENT_WINDOW: Duration = Duration::from_secs(60);
/// Remembered events are capped to keep memory small.
const EVENT_CAP: usize = 1_000;

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
            cfg,
            state: Mutex::new(HashMap::new()),
            events: Mutex::new(EventWindow::default()),
            open_connections: AtomicUsize::new(0),
            calls: AtomicU64::new(0),
        })
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, HashMap<IpAddr, IpState>> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Now and then, drop memory about addresses that are no longer interesting.
    fn housekeeping(&self) {
        // Only every 1024th call does the cleanup.
        if self.calls.fetch_add(1, Ordering::Relaxed) % 1024 != 0 {
            return;
        }
        for l in [&self.connections, &self.create, &self.join, &self.http] {
            l.retain_recent();
            l.shrink_to_fit();
        }
        let now = Instant::now();
        let mut map = self.lock_state();
        if map.len() > STATE_SOFT_LIMIT {
            map.retain(|_, s| Self::still_relevant(&self.cfg, s, now));
        }
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
        let mut ev = self.events.lock().unwrap_or_else(|e| e.into_inner());
        ev.times.push_back(now);
        while ev.times.len() > EVENT_CAP {
            ev.times.pop_front();
        }
    }

    /// Number of bad events in the last minute.
    pub(crate) fn recent_events(&self, now: Instant) -> usize {
        let mut ev = self.events.lock().unwrap_or_else(|e| e.into_inner());
        while ev
            .times
            .front()
            .is_some_and(|t| now.duration_since(*t) > EVENT_WINDOW)
        {
            ev.times.pop_front();
        }
        ev.times.len()
    }

    /// Does this client currently have failed joins in the window?
    pub(crate) fn has_recent_failures(&self, ip: IpAddr, now: Instant) -> bool {
        let key = limit_key(ip);
        self.lock_state().get(&key).is_some_and(|s| {
            s.failures > 0
                && s.window_start
                    .is_some_and(|w| now.duration_since(w) < self.cfg.failed_join_window)
        })
    }

    /// Remaining ban time, if this client is banned.
    pub(crate) fn banned_for(&self, ip: IpAddr, now: Instant) -> Option<Duration> {
        let key = limit_key(ip);
        self.lock_state()
            .get(&key)
            .and_then(|s| s.banned_until)
            .and_then(|until| until.checked_duration_since(now))
            .filter(|d| !d.is_zero())
    }

    /// A new WebSocket connection: banned? too fast? server full?
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
        // Reserve a place, undo if the server is full.
        let before = self.open_connections.fetch_add(1, Ordering::Relaxed);
        if before >= self.cfg.max_connections {
            self.open_connections.fetch_sub(1, Ordering::Relaxed);
            return Err(Deny::Busy);
        }
        Ok(ConnSlot(self.clone()))
    }

    /// Number of open connections (used by tests).
    #[cfg(test)]
    pub(crate) fn open_connections(&self) -> usize {
        self.open_connections.load(Ordering::Relaxed)
    }

    /// `create_room` from this client.
    pub(crate) fn check_create(&self, ip: IpAddr, now: Instant) -> Result<(), Deny> {
        self.housekeeping();
        if self.create.check_key(&limit_key(ip)).is_err() {
            self.note_event(now);
            return Err(Deny::RateLimited(retry_after(self.cfg.create_room_per_min)));
        }
        Ok(())
    }

    /// `join_room` from this client (banned? too many attempts?).
    pub(crate) fn check_join(&self, ip: IpAddr, now: Instant) -> Result<(), Deny> {
        self.housekeeping();
        if let Some(left) = self.banned_for(ip, now) {
            return Err(Deny::Banned(left));
        }
        if self.join.check_key(&limit_key(ip)).is_err() {
            self.note_event(now);
            return Err(Deny::RateLimited(retry_after(
                self.cfg.join_attempts_per_min,
            )));
        }
        Ok(())
    }

    /// A plain HTTP request (`/altcha/challenge`, `/ice-servers`).
    pub(crate) fn check_http(&self, ip: IpAddr, now: Instant) -> Result<(), Deny> {
        self.housekeeping();
        if let Some(left) = self.banned_for(ip, now) {
            return Err(Deny::Banned(left));
        }
        if self.http.check_key(&limit_key(ip)).is_err() {
            self.note_event(now);
            return Err(Deny::RateLimited(retry_after(self.cfg.http_per_min)));
        }
        Ok(())
    }

    /// A join failed (unknown code, full room, malformed code). Counts it harder against
    /// the join limit and bans the client after too many. Returns `Some(duration)` when
    /// the client was banned just now.
    pub(crate) fn record_failed_join(&self, ip: IpAddr, now: Instant) -> Option<Duration> {
        let key = limit_key(ip);
        self.note_event(now);
        // The failed attempt costs `weight` attempts in total; one was already paid.
        // If not enough is left, nothing is taken, but the client is then nearly
        // blocked anyway.
        if let Some(extra) = NonZeroU32::new(self.cfg.failed_join_weight.saturating_sub(1)) {
            let _ = self.join.check_key_n(&key, extra);
        }
        let mut map = self.lock_state();
        let s = map.entry(key).or_default();
        let window = self.cfg.failed_join_window;
        if s.window_start
            .is_none_or(|w| now.duration_since(w) >= window)
        {
            s.window_start = Some(now);
            s.failures = 0;
        }
        s.failures += 1;
        if s.failures < self.cfg.ban_after_failed_joins {
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
            max_rooms: 10,
            max_connections: 2,
            ban_after_failed_joins: 3,
            failed_join_window: Duration::from_secs(600),
            ban_durations: vec![Duration::from_secs(60), Duration::from_secs(300)],
            ban_memory: Duration::from_secs(3600),
        }
    }

    #[test]
    fn ipv6_is_counted_per_slash_64() {
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
    }

    #[test]
    fn create_room_rate_limit() {
        let g = Guard::new(limits());
        let now = Instant::now();
        assert!(g.check_create(ip("192.0.2.1"), now).is_ok());
        assert!(g.check_create(ip("192.0.2.1"), now).is_ok());
        assert!(matches!(
            g.check_create(ip("192.0.2.1"), now),
            Err(Deny::RateLimited(_))
        ));
        // Another client is not affected.
        assert!(g.check_create(ip("192.0.2.2"), now).is_ok());
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
    fn connection_rate_and_global_cap() {
        let g = Guard::new(limits());
        let now = Instant::now();
        let a = g.admit_connection(ip("192.0.2.1"), now).unwrap();
        let b = g.admit_connection(ip("192.0.2.2"), now).unwrap();
        // Global cap of 2 reached.
        assert_eq!(
            g.admit_connection(ip("192.0.2.3"), now).err(),
            Some(Deny::Busy)
        );
        assert_eq!(g.open_connections(), 2);
        drop(a);
        assert_eq!(g.open_connections(), 1);
        assert!(g.admit_connection(ip("192.0.2.3"), now).is_ok());
        drop(b);
        // Per-IP rate: 3 per minute; the first client used one, two more are fine, the 4th is not.
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
    fn failed_joins_count_harder_than_good_ones() {
        // 6 attempts per minute, a failure costs 3: only two failures fit.
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
        // Good joins would have allowed 6 attempts.
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
        // Third failure: first ban, 60 s.
        assert_eq!(g.record_failed_join(who, t0), Some(Duration::from_secs(60)));
        assert!(matches!(g.check_join(who, t0), Err(Deny::Banned(_))));
        assert!(g.admit_connection(who, t0).is_err());
        assert!(g.banned_for(who, t0 + Duration::from_secs(59)).is_some());
        // Other clients are fine, and the same IPv6 /64 is one client.
        assert!(g.check_join(ip("192.0.2.2"), t0).is_ok());
        // After the ban: free again.
        let t1 = t0 + Duration::from_secs(61);
        assert!(g.check_join(who, t1).is_ok());
        // Second ban is longer (300 s), and it stays 300 s afterwards.
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
        // After a long quiet time the history is forgotten: back to 60 s.
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
    fn event_window_forgets_old_events() {
        let g = Guard::new(LimitsConfig::for_mode(Mode::Open));
        let t0 = Instant::now();
        g.note_event(t0);
        g.note_event(t0);
        assert_eq!(g.recent_events(t0), 2);
        assert_eq!(g.recent_events(t0 + Duration::from_secs(61)), 0);
    }
}
