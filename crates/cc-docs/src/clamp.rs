// SPDX-License-Identifier: GPL-3.0-or-later
//! Safe time arithmetic: a caller-supplied `Duration::MAX` must not panic `Instant + Duration`.

use std::time::{Duration, Instant};

/// Longest timeout that is honoured (one day); anything longer is cut to this.
pub(crate) const MAX_TIMEOUT: Duration = Duration::from_secs(24 * 3600);

/// `timeout` cut to [`MAX_TIMEOUT`].
pub(crate) fn clamp_timeout(timeout: Duration) -> Duration {
    timeout.min(MAX_TIMEOUT)
}

/// The point in time `timeout` from now, never panicking.
pub(crate) fn deadline(timeout: Duration) -> Instant {
    let now = Instant::now();
    now.checked_add(clamp_timeout(timeout))
        .unwrap_or_else(|| now + Duration::from_secs(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn huge_timeouts_do_not_panic() {
        let _ = deadline(Duration::MAX);
        let _ = deadline(Duration::from_secs(u64::MAX / 2));
        assert_eq!(clamp_timeout(Duration::MAX), MAX_TIMEOUT);
        assert_eq!(
            clamp_timeout(Duration::from_secs(3)),
            Duration::from_secs(3)
        );
    }
}
