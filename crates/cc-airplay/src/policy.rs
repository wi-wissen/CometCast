// SPDX-License-Identifier: GPL-3.0-or-later
//! Comet Cast's own rules around AirPlay. These are plain functions, so they can be tested
//! without any UxPlay process.
//!
//! Decisions taken from the project plan (the plan text is data, the numbers are UNCERTAIN):
//! * The display device has a switch "allow foreign devices". When it is switched on, the
//!   AirPlay service runs for 10 minutes ([`FOREIGN_WINDOW`]).
//! * At most one active AirPlay session ([`MAX_ACTIVE_SESSIONS`]).
//! * After the window is over, new devices are rejected, but a mirroring that is already
//!   running continues. The service is stopped when that session ends.

use std::time::Duration;

/// How long foreign devices may start a new connection after the switch was turned on.
/// UNCERTAIN: 10 minutes is the planning decision, not measured or user-tested.
pub const FOREIGN_WINDOW: Duration = Duration::from_secs(10 * 60);

/// Largest number of AirPlay sessions that may run at the same time.
/// UNCERTAIN: planning decision. Note that UxPlay itself serves one mirroring at a time by
/// default (it does not drop the old one without `-nohold`, per its help text); we did not
/// test what a second device sees.
pub const MAX_ACTIVE_SESSIONS: usize = 1;

/// What the policy knows at the moment a device wants to start a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolicyState {
    /// Time since "allow foreign devices" was switched on. `None` = the switch is off.
    pub since_enabled: Option<Duration>,
    /// Number of sessions that are active right now.
    pub active_sessions: usize,
}

/// Why a new device was turned away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    /// "Allow foreign devices" is off.
    Disabled,
    /// The time window is over.
    WindowExpired,
    /// Another AirPlay session is active already.
    SessionActive,
}

/// Answer to "may this new device start a session?".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    /// Yes (the app may still ask the operator, see [`ConfirmHook`]).
    Allow,
    /// No.
    Reject(RejectReason),
}

/// Decide about a NEW device. A session that already runs is never touched by this function.
pub fn admit_new_session(state: PolicyState) -> Admission {
    match state.since_enabled {
        None => Admission::Reject(RejectReason::Disabled),
        Some(t) if t >= FOREIGN_WINDOW => Admission::Reject(RejectReason::WindowExpired),
        Some(_) if state.active_sessions >= MAX_ACTIVE_SESSIONS => {
            Admission::Reject(RejectReason::SessionActive)
        }
        Some(_) => Admission::Allow,
    }
}

/// What to do with the UxPlay process after the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceAction {
    /// Leave UxPlay running.
    KeepRunning,
    /// Stop UxPlay (no service is advertised any more).
    Stop,
}

/// Should the UxPlay process still run? A running mirroring continues past the window;
/// the process stops as soon as the window is over and no session is active.
pub fn service_action(state: PolicyState) -> ServiceAction {
    match state.since_enabled {
        None => {
            if state.active_sessions > 0 {
                // Switch turned off while someone mirrors. UNCERTAIN: the plan only
                // decides about the expiry of the window; we keep the picture running here
                // too, so the picture never breaks off without a clear decision.
                ServiceAction::KeepRunning
            } else {
                ServiceAction::Stop
            }
        }
        Some(t) if t >= FOREIGN_WINDOW && state.active_sessions == 0 => ServiceAction::Stop,
        Some(_) => ServiceAction::KeepRunning,
    }
}

/// What the UI can show about AirPlay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AirplayStatus {
    /// Not running (the default: foreign devices are off).
    Off,
    /// UxPlay was started, not ready yet.
    Starting,
    /// Ready; waiting for a device.
    Waiting,
    /// A device asks to connect; the PIN must be shown on the display device.
    PinShown {
        /// The four digits.
        pin: String,
    },
    /// A device is connected.
    Connected,
    /// UxPlay died and is started again.
    Restarting {
        /// Which restart attempt this is (1 = first).
        attempt: u32,
    },
    /// UxPlay cannot run (not installed, missing plugins, too many failures).
    Failed {
        /// Short English text for logs. The app shows its own translated text.
        reason: String,
    },
}

/// A device asks to connect and UxPlay shows this PIN.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmRequest {
    /// The PIN that is shown.
    pub pin: String,
}

/// The operator's answer to a [`ConfirmRequest`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Let the device go on (it must still type the PIN).
    Accept,
    /// Do not let it connect. The supervisor restarts UxPlay, which drops the pairing and
    /// the PIN. UNCERTAIN: restart is the only way we know to cancel; UxPlay 1.68 output
    /// does not name the device, so a single device cannot be blocked this way.
    Deny,
}

/// The confirmation dialog hook. The app implements this (show the PIN, ask "allow?").
///
/// It is called on its own thread, so it may block while the dialog is open. There is no
/// built-in timeout; the app should answer `Deny` itself after some time.
pub trait ConfirmHook: Send + Sync {
    /// Ask the operator.
    fn confirm(&self, request: &ConfirmRequest) -> Decision;
}

/// A hook that always says yes (for the CLI and tests). Not for real use on a display
/// device: the PIN is then the only protection.
#[derive(Debug, Clone, Copy, Default)]
pub struct AcceptAll;

impl ConfirmHook for AcceptAll {
    fn confirm(&self, _request: &ConfirmRequest) -> Decision {
        Decision::Accept
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(since: Option<u64>, active: usize) -> PolicyState {
        PolicyState {
            since_enabled: since.map(Duration::from_secs),
            active_sessions: active,
        }
    }

    #[test]
    fn allows_inside_window_without_session() {
        assert_eq!(admit_new_session(st(Some(0), 0)), Admission::Allow);
        assert_eq!(admit_new_session(st(Some(599), 0)), Admission::Allow);
    }

    #[test]
    fn rejects_when_off() {
        assert_eq!(
            admit_new_session(st(None, 0)),
            Admission::Reject(RejectReason::Disabled)
        );
    }

    #[test]
    fn rejects_after_ten_minutes() {
        assert_eq!(
            admit_new_session(st(Some(600), 0)),
            Admission::Reject(RejectReason::WindowExpired)
        );
        assert_eq!(
            admit_new_session(st(Some(3600), 0)),
            Admission::Reject(RejectReason::WindowExpired)
        );
    }

    #[test]
    fn only_one_session() {
        assert_eq!(
            admit_new_session(st(Some(10), 1)),
            Admission::Reject(RejectReason::SessionActive)
        );
    }

    #[test]
    fn running_mirroring_continues_after_window() {
        assert_eq!(service_action(st(Some(599), 0)), ServiceAction::KeepRunning);
        assert_eq!(service_action(st(Some(600), 1)), ServiceAction::KeepRunning);
        assert_eq!(
            service_action(st(Some(7200), 1)),
            ServiceAction::KeepRunning
        );
        // The session ended after the window: now the service stops.
        assert_eq!(service_action(st(Some(7200), 0)), ServiceAction::Stop);
        assert_eq!(service_action(st(None, 0)), ServiceAction::Stop);
        assert_eq!(service_action(st(None, 1)), ServiceAction::KeepRunning);
    }

    #[test]
    fn accept_all_hook() {
        let d = AcceptAll.confirm(&ConfirmRequest { pin: "1234".into() });
        assert_eq!(d, Decision::Accept);
    }
}
