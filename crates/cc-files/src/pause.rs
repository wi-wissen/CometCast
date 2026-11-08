// SPDX-License-Identifier: GPL-3.0-or-later
//! The two pause flags of a presentation (docs/design/pause.md).
//!
//! * `presenter` is set only by the presenter (`cmd pause` / `cmd resume`),
//! * `display` is set only by the display (its own Pause control).
//!
//! The picture is frozen while **either** flag is set. Each side clears only its own flag.
//! The display owns this state and tells the presenter with `pause_state`
//! ([`PauseState::message`]). Pure logic: no I/O.

use crate::wire::Message;

/// The pause flags, as the display holds them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PauseState {
    /// Set by the presenter: it stopped sending video.
    pub presenter: bool,
    /// Set by the display: it keeps showing the last picture.
    pub display: bool,
}

impl PauseState {
    /// Both flags cleared.
    pub const CLEAR: Self = Self {
        presenter: false,
        display: false,
    };

    /// The picture is frozen while either flag is set.
    pub fn is_frozen(self) -> bool {
        self.presenter || self.display
    }

    /// `cmd pause` / `cmd resume` of the presenter. Returns whether the state changed
    /// (both commands are idempotent).
    pub fn set_presenter(&mut self, paused: bool) -> bool {
        std::mem::replace(&mut self.presenter, paused) != paused
    }

    /// The display's own Pause control. Returns whether the state changed.
    pub fn set_display(&mut self, paused: bool) -> bool {
        std::mem::replace(&mut self.display, paused) != paused
    }

    /// The screen stopped (stop, presentation ended, peer left, new session): both flags are
    /// cleared. Returns whether the state changed.
    pub fn clear(&mut self) -> bool {
        std::mem::take(self) != Self::CLEAR
    }

    /// The `pause_state` message for the presenter.
    pub fn message(self) -> String {
        Message::PauseState {
            presenter: self.presenter,
            display: self.display,
        }
        .to_json()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn either_flag_freezes_and_each_side_clears_only_its_own() {
        let mut s = PauseState::default();
        assert!(!s.is_frozen());
        assert!(s.set_presenter(true));
        assert!(!s.set_presenter(true), "idempotent");
        assert!(s.set_display(true));
        assert!(s.is_frozen());
        assert!(s.set_presenter(false));
        assert!(s.is_frozen(), "the display flag keeps the picture frozen");
        assert!(s.set_display(false));
        assert!(!s.is_frozen());
        assert!(!s.set_display(false));
    }

    #[test]
    fn clear_resets_both() {
        let mut s = PauseState {
            presenter: true,
            display: true,
        };
        assert!(s.clear());
        assert_eq!(s, PauseState::CLEAR);
        assert!(!s.clear());
    }

    #[test]
    fn the_message_is_the_wire_format() {
        let s = PauseState {
            presenter: true,
            display: false,
        };
        assert_eq!(
            s.message(),
            r#"{"display":false,"kind":"pause_state","presenter":true}"#
        );
    }
}
