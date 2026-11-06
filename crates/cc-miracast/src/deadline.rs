// SPDX-License-Identifier: GPL-3.0-or-later
//! One timer for all pending confirmations (instead of one sleeping thread per request).

// Only the Windows backend uses the queue; the unit tests run everywhere.
#![cfg_attr(not(windows), allow(dead_code))]

use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Instant;

#[derive(Default)]
struct Inner {
    items: Vec<(Instant, u64)>,
    closed: bool,
}

/// A set of `(deadline, id)` pairs. One thread calls [`DeadlineQueue::wait_next`] in a loop.
#[derive(Default)]
pub(crate) struct DeadlineQueue {
    inner: Mutex<Inner>,
    cv: Condvar,
}

impl DeadlineQueue {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Register a deadline for `id`.
    pub(crate) fn add(&self, id: u64, at: Instant) {
        self.lock().items.push((at, id));
        self.cv.notify_all();
    }

    /// Forget the deadline of `id` (the request was answered).
    pub(crate) fn cancel(&self, id: u64) {
        self.lock().items.retain(|(_, i)| *i != id);
        self.cv.notify_all();
    }

    /// Wake the waiting thread for good; `wait_next` returns `None` from now on.
    pub(crate) fn close(&self) {
        let mut g = self.lock();
        g.closed = true;
        g.items.clear();
        drop(g);
        self.cv.notify_all();
    }

    /// Block until a deadline has passed and return its id (removing it), or `None` once closed.
    pub(crate) fn wait_next(&self) -> Option<u64> {
        let mut g = self.lock();
        loop {
            if g.closed {
                return None;
            }
            let now = Instant::now();
            if let Some(pos) = g.items.iter().position(|(at, _)| *at <= now) {
                return Some(g.items.swap_remove(pos).1);
            }
            let next = g.items.iter().map(|(at, _)| *at).min();
            g = match next {
                Some(at) => {
                    self.cv
                        .wait_timeout(g, at.saturating_duration_since(now))
                        .unwrap_or_else(PoisonError::into_inner)
                        .0
                }
                None => self.cv.wait(g).unwrap_or_else(PoisonError::into_inner),
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn fires_in_deadline_order_and_cancel_works() {
        let q = DeadlineQueue::default();
        let now = Instant::now();
        q.add(2, now + Duration::from_millis(60));
        q.add(1, now + Duration::from_millis(10));
        q.add(3, now + Duration::from_millis(20));
        q.cancel(3);
        assert_eq!(q.wait_next(), Some(1));
        assert_eq!(q.wait_next(), Some(2));
    }

    #[test]
    fn close_wakes_a_waiter() {
        let q = Arc::new(DeadlineQueue::default());
        let q2 = Arc::clone(&q);
        let h = std::thread::spawn(move || q2.wait_next());
        std::thread::sleep(Duration::from_millis(30));
        q.close();
        assert_eq!(h.join().unwrap(), None);
    }

    #[test]
    fn add_wakes_a_waiter_with_an_earlier_deadline() {
        let q = Arc::new(DeadlineQueue::default());
        q.add(9, Instant::now() + Duration::from_secs(3600));
        let q2 = Arc::clone(&q);
        let h = std::thread::spawn(move || q2.wait_next());
        std::thread::sleep(Duration::from_millis(30));
        q.add(1, Instant::now());
        assert_eq!(h.join().unwrap(), Some(1));
    }
}
