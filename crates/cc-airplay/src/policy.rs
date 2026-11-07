// SPDX-License-Identifier: GPL-3.0-or-later
//! Comet Cast's own rules around AirPlay. These are plain functions, so they can be tested
//! without any UxPlay process.
//!
//! Decisions (docs/design/first-run-and-display.md, N87 and N88):
//! * Admission follows the display's **join mode**, not a time window: Open lets a device in
//!   with the PIN, Approval on asks the operator first, Locked turns new devices away. The
//!   switch in Settings is the permission. (This replaces the 10-minute "allow foreign devices"
//!   window of the first version.)
//! * A PIN is always required (`-pin`, never `-reg`, see [`crate::config`]).
//! * At most one active AirPlay session ([`MAX_ACTIVE_SESSIONS`]).
//! * While a Comet Cast presenter presents, new pairings are refused without any question
//!   ("Comet Cast first").
//! * Locked stops the service as soon as no session is active; a running mirroring stays until
//!   it ends.

/// Largest number of AirPlay sessions that may run at the same time.
/// UNCERTAIN: planning decision. Note that UxPlay itself serves one mirroring at a time by
/// default (it does not drop the old one without `-nohold`, per its help text); we did not
/// test what a second device sees.
pub const MAX_ACTIVE_SESSIONS: usize = 1;

/// The join mode of the display, as far as AirPlay is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinMode {
    /// Open: a device gets in with the PIN, no question.
    Open,
    /// Approval on: the operator is asked first, then the PIN.
    Approval,
    /// Locked: no new device; a running mirror stays.
    Locked,
}

/// What the policy knows at the moment a device wants to start a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolicyState {
    /// The join mode of the display.
    pub join_mode: JoinMode,
    /// Number of sessions that are active right now.
    pub active_sessions: usize,
    /// A Comet Cast presenter presents right now (decision N88: Comet Cast first).
    pub comet_cast_presenting: bool,
}

/// Why a new device was turned away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    /// The display is Locked.
    Locked,
    /// Another AirPlay session is active already.
    SessionActive,
    /// A Comet Cast presenter presents (no dialog, the device shows its own failure).
    CometCastFirst,
}

/// Answer to "may this new device start a session?".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    /// Yes, the PIN follows at once (Open).
    Allow,
    /// Ask the operator first, then show the PIN (Approval on).
    Ask,
    /// No.
    Reject(RejectReason),
}

/// Decide about a NEW device. A session that already runs is never touched by this function.
pub fn admit_new_session(state: PolicyState) -> Admission {
    if state.join_mode == JoinMode::Locked {
        return Admission::Reject(RejectReason::Locked);
    }
    if state.comet_cast_presenting {
        return Admission::Reject(RejectReason::CometCastFirst);
    }
    if state.active_sessions >= MAX_ACTIVE_SESSIONS {
        return Admission::Reject(RejectReason::SessionActive);
    }
    match state.join_mode {
        JoinMode::Open => Admission::Allow,
        JoinMode::Approval | JoinMode::Locked => Admission::Ask,
    }
}

/// What to do with the UxPlay process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceAction {
    /// Leave UxPlay running.
    KeepRunning,
    /// Stop UxPlay (no service is advertised any more).
    Stop,
}

/// Should the UxPlay process still run? Locked stops it, but a running mirroring continues:
/// the process stops as soon as the display is Locked and no session is active.
pub fn service_action(state: PolicyState) -> ServiceAction {
    if state.join_mode == JoinMode::Locked && state.active_sessions == 0 {
        ServiceAction::Stop
    } else {
        ServiceAction::KeepRunning
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

    fn st(join_mode: JoinMode, active: usize, presenting: bool) -> PolicyState {
        PolicyState {
            join_mode,
            active_sessions: active,
            comet_cast_presenting: presenting,
        }
    }

    #[test]
    fn open_lets_a_device_in_without_a_question() {
        assert_eq!(
            admit_new_session(st(JoinMode::Open, 0, false)),
            Admission::Allow
        );
    }

    #[test]
    fn approval_on_asks_first() {
        assert_eq!(
            admit_new_session(st(JoinMode::Approval, 0, false)),
            Admission::Ask
        );
    }

    #[test]
    fn locked_turns_new_devices_away() {
        assert_eq!(
            admit_new_session(st(JoinMode::Locked, 0, false)),
            Admission::Reject(RejectReason::Locked)
        );
    }

    #[test]
    fn only_one_session() {
        for m in [JoinMode::Open, JoinMode::Approval] {
            assert_eq!(
                admit_new_session(st(m, 1, false)),
                Admission::Reject(RejectReason::SessionActive)
            );
        }
    }

    #[test]
    fn comet_cast_comes_first_in_every_mode() {
        for m in [JoinMode::Open, JoinMode::Approval] {
            assert_eq!(
                admit_new_session(st(m, 0, true)),
                Admission::Reject(RejectReason::CometCastFirst)
            );
        }
        assert_eq!(
            admit_new_session(st(JoinMode::Locked, 0, true)),
            Admission::Reject(RejectReason::Locked)
        );
    }

    #[test]
    fn locked_stops_the_service_once_no_session_is_active() {
        // A running mirroring stays until it ends ("Connected stay").
        assert_eq!(
            service_action(st(JoinMode::Locked, 1, false)),
            ServiceAction::KeepRunning
        );
        assert_eq!(
            service_action(st(JoinMode::Locked, 0, false)),
            ServiceAction::Stop
        );
        assert_eq!(
            service_action(st(JoinMode::Locked, 0, true)),
            ServiceAction::Stop
        );
        for m in [JoinMode::Open, JoinMode::Approval] {
            assert_eq!(service_action(st(m, 0, false)), ServiceAction::KeepRunning);
            assert_eq!(service_action(st(m, 1, true)), ServiceAction::KeepRunning);
        }
    }

    #[test]
    fn accept_all_hook() {
        let d = AcceptAll.confirm(&ConfirmRequest { pin: "1234".into() });
        assert_eq!(d, Decision::Accept);
    }
}
