// SPDX-License-Identifier: GPL-3.0-or-later
//! The life of a picker-based capture session, as a small state machine that every picker
//! backend (portal, Windows Graphics Capture, ScreenCaptureKit) drives with the events its
//! platform reports, plus the one-slot mailbox that carries frames from the platform's
//! callback thread to the thread that calls `next_frame`.
//!
//! Both pieces are plain Rust so they are tested here with fake events and fake frames; what
//! the platform really reports is only checked by the manual test plans.

use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::{CaptureError, RawFrame};

/// Why a session ended after it had started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    /// `stop` was called.
    Stopped,
    /// The shared window or screen is gone (window closed, monitor unplugged).
    SourceClosed,
    /// The system or the person ended it (compositor closed the session, the Windows or
    /// macOS "stop sharing" control).
    SystemStopped,
}

/// Where a session is.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Phase {
    /// Nothing started.
    #[default]
    Idle,
    /// The system picker is open.
    Picking,
    /// A source was picked; the stream is being set up.
    Starting,
    /// Frames arrive.
    Streaming,
    /// The source delivers nothing for now (minimised window, compositor paused the stream).
    Paused,
    /// Over after a start.
    Ended(EndReason),
    /// The picker was cancelled or the permission refused: maps to `capture_denied`.
    Denied,
    /// Failed with a reason.
    Failed(String),
}

/// What a platform reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// The picker opened.
    PickerShown,
    /// The person closed the picker without choosing, or the system refused permission.
    PickerCancelled,
    /// A source was picked.
    Picked,
    /// The stream delivers frames.
    StreamReady,
    /// The stream is paused.
    StreamPaused,
    /// The stream continues.
    StreamResumed,
    /// The shared window or screen disappeared.
    SourceClosed,
    /// The system ended the session.
    SystemStopped,
    /// `stop` was called.
    StopRequested,
    /// Something failed.
    Error(String),
}

impl Phase {
    /// The next phase after `event`. Events that make no sense in the current phase leave it
    /// unchanged; a finished phase (`Ended`, `Denied`, `Failed`) never changes again.
    pub fn apply(self, event: Event) -> Phase {
        use Phase::*;
        if matches!(self, Ended(_) | Denied | Failed(_)) {
            return self;
        }
        match (self, event) {
            (_, Event::StopRequested) => Ended(EndReason::Stopped),
            (_, Event::Error(e)) => Failed(e),
            (Idle, Event::PickerShown) => Picking,
            (Picking, Event::PickerCancelled) => Denied,
            (Picking, Event::Picked) => Starting,
            // A platform without a separate "picker is open" signal goes straight to Picked.
            (Idle, Event::Picked) => Starting,
            (Starting, Event::StreamReady)
            | (Paused, Event::StreamReady | Event::StreamResumed) => Streaming,
            (Streaming, Event::StreamPaused) => Paused,
            (Starting, Event::StreamPaused) => Starting,
            (Starting | Streaming | Paused, Event::SourceClosed) => Ended(EndReason::SourceClosed),
            (Starting | Streaming | Paused, Event::SystemStopped) => {
                Ended(EndReason::SystemStopped)
            }
            (Idle | Picking, Event::SourceClosed | Event::SystemStopped) => {
                Failed("the capture session ended before it started".into())
            }
            (p, _) => p,
        }
    }

    /// True once nothing more can happen.
    pub fn is_final(&self) -> bool {
        matches!(self, Phase::Ended(_) | Phase::Denied | Phase::Failed(_))
    }

    /// The error `start` must return for a phase it cannot continue from.
    pub fn start_error(&self) -> Option<CaptureError> {
        match self {
            Phase::Denied => Some(CaptureError::Denied),
            Phase::Failed(e) => Some(CaptureError::Display(e.clone())),
            Phase::Ended(_) => Some(CaptureError::Display(
                "the capture ended while it was starting".into(),
            )),
            _ => None,
        }
    }
}

/// A one-slot mailbox: the producer replaces the picture, the consumer takes the newest one.
/// Older pictures are dropped on purpose (the consumer works at a lower rate than some
/// compositors deliver; only the latest state of the screen matters).
pub struct Mailbox<T> {
    inner: Mutex<Inner<T>>,
    cv: Condvar,
}

struct Inner<T> {
    item: Option<T>,
    /// The producer is finished (stream ended).
    closed: bool,
    /// Pictures that were replaced before the consumer took them.
    dropped: u64,
}

/// What [`Mailbox::take_timeout`] found.
#[derive(Debug, PartialEq, Eq)]
pub enum Taken<T> {
    /// The newest picture.
    Item(T),
    /// Nothing new arrived in time.
    Empty,
    /// The producer closed the mailbox and nothing is left.
    Closed,
}

impl<T> Default for Mailbox<T> {
    fn default() -> Self {
        Self {
            inner: Mutex::new(Inner {
                item: None,
                closed: false,
                dropped: 0,
            }),
            cv: Condvar::new(),
        }
    }
}

impl<T> Mailbox<T> {
    /// An empty, open mailbox.
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner<T>> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Put a picture in, replacing an untaken one.
    pub fn put(&self, item: T) {
        let mut g = self.lock();
        if g.item.replace(item).is_some() {
            g.dropped += 1;
        }
        self.cv.notify_all();
    }

    /// Mark the producer finished; a waiting consumer wakes up.
    pub fn close(&self) {
        self.lock().closed = true;
        self.cv.notify_all();
    }

    /// How many pictures were replaced before they were taken.
    pub fn dropped(&self) -> u64 {
        self.lock().dropped
    }

    /// Wait up to `timeout` for a picture. A picture that is already there is returned even
    /// if the mailbox is closed.
    pub fn take_timeout(&self, timeout: Duration) -> Taken<T> {
        let deadline = Instant::now() + timeout;
        let mut g = self.lock();
        loop {
            if let Some(item) = g.item.take() {
                return Taken::Item(item);
            }
            if g.closed {
                return Taken::Closed;
            }
            let now = Instant::now();
            if now >= deadline {
                return Taken::Empty;
            }
            g = self
                .cv
                .wait_timeout(g, deadline - now)
                .unwrap_or_else(|p| p.into_inner())
                .0;
        }
    }
}

/// State shared between the platform's callback thread (producer) and the thread of
/// `next_frame` (consumer): the newest picture, the phase and the latest picture size.
#[derive(Default)]
pub struct Shared {
    /// The newest picture.
    pub mailbox: Mailbox<RawFrame>,
    phase: Mutex<Phase>,
    /// Latest size in pixels from the stream format.
    size: Mutex<Option<(u32, u32)>>,
}

impl Shared {
    /// A fresh shared state in phase `Idle`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed an event into the state machine. Closing events also close the mailbox so a
    /// waiting consumer wakes up.
    pub fn event(&self, e: Event) {
        let mut p = self.phase.lock().unwrap_or_else(|p| p.into_inner());
        let next = std::mem::take(&mut *p).apply(e);
        let done = next.is_final();
        *p = next;
        drop(p);
        if done {
            self.mailbox.close();
        }
    }

    /// The current phase.
    pub fn phase(&self) -> Phase {
        self.phase.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// Record the pixel size from a (new) stream format.
    pub fn set_size(&self, w: u32, h: u32) {
        *self.size.lock().unwrap_or_else(|p| p.into_inner()) = Some((w, h));
    }

    /// The latest pixel size.
    pub fn size(&self) -> Option<(u32, u32)> {
        *self.size.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Publish a finished BGRA picture: remember its size, put it in the mailbox and tell the
    /// state machine the stream delivers.
    pub fn push_frame(&self, frame: RawFrame) {
        self.set_size(frame.width, frame.height);
        self.mailbox.put(frame);
        self.event(Event::StreamReady);
    }
}

/// Decides when the next picture is looked at. Pure: the caller passes the clock.
#[derive(Debug, Clone)]
pub struct Pacer {
    interval: Duration,
    next_at: Option<Instant>,
}

impl Pacer {
    /// A pacer for `fps` pictures per second (1..=240; 0 counts as 1).
    pub fn new(fps: u32) -> Self {
        Self {
            interval: Duration::from_secs_f64(1.0 / f64::from(fps.clamp(1, 240))),
            next_at: None,
        }
    }

    /// The time between two looks.
    pub fn interval(&self) -> Duration {
        self.interval
    }

    /// How long to wait from `now` until the next look. The first look is immediate. After a
    /// late look the schedule is not caught up with a burst: the next look is one interval
    /// after `now`, not after the missed slot.
    pub fn wait(&self, now: Instant) -> Duration {
        match self.next_at {
            Some(t) => t.saturating_duration_since(now),
            None => Duration::ZERO,
        }
    }

    /// Record that a look happened at `now`.
    pub fn looked(&mut self, now: Instant) {
        self.next_at = Some(match self.next_at {
            Some(p) if now >= p && now - p <= self.interval => p + self.interval,
            _ => now + self.interval,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn run(events: &[Event]) -> Phase {
        events.iter().cloned().fold(Phase::Idle, |p, e| p.apply(e))
    }

    #[test]
    fn happy_path() {
        let p = run(&[
            Event::PickerShown,
            Event::Picked,
            Event::StreamReady,
            Event::StreamPaused,
            Event::StreamResumed,
        ]);
        assert_eq!(p, Phase::Streaming);
    }

    #[test]
    fn cancelled_picker_is_denied() {
        let p = run(&[Event::PickerShown, Event::PickerCancelled]);
        assert_eq!(p, Phase::Denied);
        assert_eq!(p.start_error(), Some(CaptureError::Denied));
        assert!(p.is_final());
    }

    #[test]
    fn final_phases_stay() {
        let p = run(&[Event::PickerShown, Event::PickerCancelled, Event::Picked]);
        assert_eq!(p, Phase::Denied);
        let p = run(&[
            Event::Picked,
            Event::StreamReady,
            Event::SourceClosed,
            Event::StreamReady,
        ]);
        assert_eq!(p, Phase::Ended(EndReason::SourceClosed));
    }

    #[test]
    fn system_stop_and_stop_request() {
        let p = run(&[Event::Picked, Event::StreamReady, Event::SystemStopped]);
        assert_eq!(p, Phase::Ended(EndReason::SystemStopped));
        let p = run(&[Event::Picked, Event::StopRequested]);
        assert_eq!(p, Phase::Ended(EndReason::Stopped));
    }

    #[test]
    fn errors_fail_the_session() {
        let p = run(&[Event::Picked, Event::Error("boom".into())]);
        assert_eq!(p, Phase::Failed("boom".into()));
        assert!(matches!(p.start_error(), Some(CaptureError::Display(m)) if m == "boom"));
    }

    #[test]
    fn closing_before_start_is_a_failure() {
        let p = run(&[Event::PickerShown, Event::SystemStopped]);
        assert!(matches!(p, Phase::Failed(_)));
    }

    #[test]
    fn pause_before_ready_is_ignored() {
        let p = run(&[Event::Picked, Event::StreamPaused, Event::StreamReady]);
        assert_eq!(p, Phase::Streaming);
    }

    #[test]
    fn mailbox_keeps_only_the_newest() {
        let m = Mailbox::new();
        m.put(1);
        m.put(2);
        m.put(3);
        assert_eq!(m.dropped(), 2);
        assert_eq!(m.take_timeout(Duration::ZERO), Taken::Item(3));
        assert_eq!(m.take_timeout(Duration::from_millis(5)), Taken::Empty);
    }

    #[test]
    fn mailbox_wakes_a_waiting_consumer_and_closes() {
        let m = Arc::new(Mailbox::new());
        let m2 = Arc::clone(&m);
        let t = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            m2.put("a");
            std::thread::sleep(Duration::from_millis(20));
            m2.close();
        });
        assert_eq!(m.take_timeout(Duration::from_secs(5)), Taken::Item("a"));
        assert_eq!(m.take_timeout(Duration::from_secs(5)), Taken::Closed);
        t.join().unwrap();
    }

    #[test]
    fn a_picture_survives_close() {
        let m = Mailbox::new();
        m.put(7);
        m.close();
        assert_eq!(m.take_timeout(Duration::ZERO), Taken::Item(7));
        assert_eq!(m.take_timeout(Duration::ZERO), Taken::Closed);
    }

    #[test]
    fn pacer_keeps_rhythm_and_does_not_burst() {
        let mut p = Pacer::new(10);
        let t0 = Instant::now();
        assert_eq!(p.wait(t0), Duration::ZERO);
        p.looked(t0);
        assert_eq!(p.wait(t0), Duration::from_millis(100));
        // A look 30 ms late keeps the grid: the next look is due 70 ms later.
        let t1 = t0 + Duration::from_millis(130);
        p.looked(t1);
        assert_eq!(p.wait(t1), Duration::from_millis(70));
        // A look several intervals late restarts from now.
        let t2 = t1 + Duration::from_millis(900);
        p.looked(t2);
        assert_eq!(p.wait(t2), Duration::from_millis(100));
    }

    #[test]
    fn pacer_clamps_fps() {
        assert_eq!(Pacer::new(0).interval(), Duration::from_secs(1));
        assert!(Pacer::new(10_000).interval() > Duration::ZERO);
    }
}
