// SPDX-License-Identifier: GPL-3.0-or-later
//! "Static refresh": after the screen has been unchanged for a short time, the last picture is
//! encoded again a few times so the encoder can refine it (text gets sharp), and then nothing
//! more is sent until the screen changes (docs/measurement-points.md, A10).
//!
//! This is only the timing logic, with the clock passed in so it can be tested exactly. It does
//! not touch the encoder.
//!
//! UNCERTAIN: that re-encoding an unchanged picture really makes OpenH264 output sharper text
//! is the idea from the plan; docs/capture.md says what was measured here and what was not.

use std::time::{Duration, Instant};

/// Settings of [`StaticRefresh`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefreshConfig {
    /// How long the picture must stay unchanged before the first refresh.
    pub quiet: Duration,
    /// How many refreshes are made at most per quiet period.
    pub repeats: u32,
    /// Minimum time between two refreshes.
    pub interval: Duration,
}

impl Default for RefreshConfig {
    /// UNCERTAIN: 100 ms quiet time is the value named in the plan; 3 repeats every 100 ms are
    /// placeholders ("a few times"), not tuned.
    fn default() -> Self {
        Self {
            quiet: Duration::from_millis(100),
            repeats: 3,
            interval: Duration::from_millis(100),
        }
    }
}

/// Decides when the last picture should be encoded again.
#[derive(Debug, Clone)]
pub struct StaticRefresh {
    config: RefreshConfig,
    /// When the picture last changed (or was last encoded because of a change).
    changed_at: Option<Instant>,
    /// When the last refresh was made.
    last_refresh: Option<Instant>,
    done: u32,
}

impl StaticRefresh {
    /// A new helper with nothing to refresh yet.
    pub fn new(config: RefreshConfig) -> Self {
        Self {
            config,
            changed_at: None,
            last_refresh: None,
            done: 0,
        }
    }

    /// The picture changed (and was encoded) at `now`: start a new quiet period.
    pub fn picture_changed(&mut self, now: Instant) {
        self.changed_at = Some(now);
        self.last_refresh = None;
        self.done = 0;
    }

    /// Ask whether the last picture should be encoded again now. Returns `true` at most
    /// `repeats` times per quiet period; the caller then re-encodes and must not call
    /// [`StaticRefresh::picture_changed`] for that.
    pub fn poll(&mut self, now: Instant) -> bool {
        let Some(changed) = self.changed_at else {
            return false;
        };
        if self.done >= self.config.repeats {
            return false;
        }
        let due = match self.last_refresh {
            None => changed + self.config.quiet,
            Some(last) => last + self.config.interval,
        };
        if now >= due {
            self.done += 1;
            self.last_refresh = Some(now);
            true
        } else {
            false
        }
    }

    /// When the next refresh would be due, or `None` if there is nothing left to do.
    /// Useful to sleep until then.
    pub fn next_due(&self) -> Option<Instant> {
        let changed = self.changed_at?;
        if self.done >= self.config.repeats {
            return None;
        }
        Some(match self.last_refresh {
            None => changed + self.config.quiet,
            Some(last) => last + self.config.interval,
        })
    }

    /// Forget everything (for example after a resolution change).
    pub fn reset(&mut self) {
        self.changed_at = None;
        self.last_refresh = None;
        self.done = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn nothing_to_refresh_before_a_picture() {
        let mut r = StaticRefresh::new(RefreshConfig::default());
        assert!(!r.poll(Instant::now()));
        assert_eq!(r.next_due(), None);
    }

    #[test]
    fn refreshes_a_limited_number_of_times_then_stops() {
        let t0 = Instant::now();
        let mut r = StaticRefresh::new(RefreshConfig::default());
        r.picture_changed(t0);
        assert!(!r.poll(t0 + ms(99)), "not before the quiet time");
        assert!(r.poll(t0 + ms(100)));
        assert!(!r.poll(t0 + ms(150)), "interval not over");
        assert!(r.poll(t0 + ms(210)));
        assert!(r.poll(t0 + ms(320)));
        // The budget of 3 is used up: silence, however long we wait.
        assert!(!r.poll(t0 + ms(500)));
        assert!(!r.poll(t0 + Duration::from_secs(60)));
        assert_eq!(r.next_due(), None);
    }

    #[test]
    fn a_change_starts_a_new_period() {
        let t0 = Instant::now();
        let mut r = StaticRefresh::new(RefreshConfig::default());
        r.picture_changed(t0);
        assert!(r.poll(t0 + ms(100)));
        r.picture_changed(t0 + ms(120));
        assert!(!r.poll(t0 + ms(150)));
        assert_eq!(r.next_due(), Some(t0 + ms(220)));
        assert!(r.poll(t0 + ms(220)));
    }
}
