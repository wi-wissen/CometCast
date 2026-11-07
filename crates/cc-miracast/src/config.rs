// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed settings of the Miracast receiver.

use crate::policy::{DEFAULT_MAX_SESSIONS, MAX_NAME_CHARS};

/// How a foreign device must be authorised before its stream is used.
///
/// There is deliberately no "accept everyone" variant: every foreign device is confirmed
/// by a person (see [`crate::policy::ConnectionPolicy`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PairingPolicy {
    /// The person at the receiver confirms each connection (Windows value
    /// `ConfirmConnection`). Default.
    #[default]
    ConfirmConnection,
    /// Like [`PairingPolicy::ConfirmConnection`], and a PIN is shown at the receiver when
    /// the sender asks for one (Windows value `PinDisplayIfRequested`).
    // UNCERTAIN: what "if requested" means in practice (which senders request a PIN) was not
    // observed; checklist item F2 of docs/miracast-device-test-plan.md.
    PinIfRequested,
    /// Like [`PairingPolicy::ConfirmConnection`], and a PIN must be shown at the receiver
    /// and typed on the sending device (Windows value `PinDisplayRequired`).
    // UNCERTAIN: which side draws the PIN on screen (Windows UI or our app) is exactly
    // measurement point 5 / assumption A4. The backend reports the PIN it gets from
    // `ConnectionCreated`; the app must display it.
    RequirePin,
}

/// Settings for [`crate::MiracastReceiver::start`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiracastConfig {
    /// Receiver name shown in the sender's device list (Windows `FriendlyName`).
    pub display_name: String,
    /// Authorisation rule.
    pub pairing: PairingPolicy,
    /// Upper limit of simultaneous sessions. The policy never lets this exceed 1.
    pub max_sessions: usize,
}

impl Default for MiracastConfig {
    fn default() -> Self {
        Self {
            display_name: "Comet Cast".to_string(),
            pairing: PairingPolicy::default(),
            max_sessions: DEFAULT_MAX_SESSIONS,
        }
    }
}

/// A configuration problem found before anything is started.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    /// The display name is empty or only white space.
    #[error("the display name is empty")]
    EmptyName,
    /// The display name is longer than [`MAX_NAME_CHARS`] characters.
    #[error("the display name is longer than {MAX_NAME_CHARS} characters")]
    NameTooLong,
    /// The display name contains a control character.
    #[error("the display name contains a control character")]
    ControlCharacter,
}

impl MiracastConfig {
    /// Check the settings. Windows has the final say; it also reports `FriendlyNameTooLong`.
    pub fn validate(&self) -> Result<(), ConfigError> {
        let name = self.display_name.trim();
        if name.is_empty() {
            return Err(ConfigError::EmptyName);
        }
        if name.chars().count() > MAX_NAME_CHARS {
            return Err(ConfigError::NameTooLong);
        }
        if name.chars().any(char::is_control) {
            return Err(ConfigError::ControlCharacter);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_valid_and_confirms() {
        let c = MiracastConfig::default();
        assert_eq!(c.validate(), Ok(()));
        assert_eq!(c.pairing, PairingPolicy::ConfirmConnection);
        assert_eq!(c.max_sessions, 1);
    }

    #[test]
    fn rejects_bad_names() {
        let mut c = MiracastConfig {
            display_name: "  ".into(),
            ..Default::default()
        };
        assert_eq!(c.validate(), Err(ConfigError::EmptyName));
        c.display_name = "x".repeat(MAX_NAME_CHARS + 1);
        assert_eq!(c.validate(), Err(ConfigError::NameTooLong));
        c.display_name = "a\nb".into();
        assert_eq!(c.validate(), Err(ConfigError::ControlCharacter));
        c.display_name = "x".repeat(MAX_NAME_CHARS);
        assert_eq!(c.validate(), Ok(()));
    }
}
