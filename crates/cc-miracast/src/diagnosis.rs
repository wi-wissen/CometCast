// SPDX-License-Identifier: GPL-3.0-or-later
//! What is wrong when Miracast receiving does not work, as data the UI can translate.
//!
//! The crate never produces user-facing text; it names the problem ([`Issue`]) and the UI
//! picks a translated hint. Platform independent and unit-tested on Linux.

use crate::{MiracastError, MiracastStatus};

/// Name of the Windows optional feature ("Wireless Display") as a DISM capability.
// UNCERTAIN: written from memory of the Microsoft documentation, not checked on a machine.
// Check with `DISM /Online /Get-CapabilityInfo /CapabilityName:App.WirelessDisplay.Connect~~~~0.0.1.0`.
pub const WIRELESS_DISPLAY_CAPABILITY: &str = "App.WirelessDisplay.Connect~~~~0.0.1.0";

/// Is the optional Windows feature "Wireless Display" installed?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FeatureState {
    /// The marker file of the feature exists.
    Installed,
    /// The marker file is missing.
    NotInstalled,
    /// Could not be checked (not Windows, or the check itself failed).
    #[default]
    Unknown,
}

impl FeatureState {
    /// Map the result of a "does the marker file exist" check: `None` means the check
    /// could not be made.
    #[must_use]
    pub fn from_marker(exists: Option<bool>) -> Self {
        match exists {
            Some(true) => Self::Installed,
            Some(false) => Self::NotInstalled,
            None => Self::Unknown,
        }
    }
}

/// How well the Wi-Fi adapter supports Miracast receiving, as Windows reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AdapterSupport {
    /// Windows could not tell (yet).
    #[default]
    Undetermined,
    /// The adapter or its driver cannot receive Miracast.
    NotSupported,
    /// Supported, but Windows says the driver is not optimised for it.
    NotOptimized,
    /// Supported.
    Supported,
}

/// What a quick look at the system (without starting a session) found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Probe {
    /// State of the "Wireless Display" optional feature.
    pub feature: FeatureState,
    /// Windows' verdict on the Wi-Fi adapter. `Undetermined` also when Windows could not be asked.
    pub adapter: AdapterSupport,
    /// Windows' listening status mapped to ours, `None` if it could not be read.
    pub listening: Option<MiracastStatus>,
    /// Windows could create the `MiracastReceiver` object at all.
    pub receiver_available: bool,
}

/// What the person should be told. The UI maps each to a translated text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Issue {
    /// Not running on Windows.
    NotWindows,
    /// The optional feature "Wireless Display" seems to be missing: Windows could not
    /// provide the receiver. Hint: Settings, System, Optional features (path UNCERTAIN).
    FeatureMissing,
    /// The Wi-Fi adapter or driver cannot receive Miracast.
    AdapterUnsupported,
    /// Receiving works but the driver is not optimised: expect a poor picture.
    AdapterNotOptimized,
    /// Windows policy (group policy or MDM) switched receiving off.
    DisabledByPolicy,
    /// Windows says receiving is temporarily not possible (Wi-Fi off, in use elsewhere).
    TemporarilyUnavailable,
    /// Windows refused (access denied): not enough rights or packaging problem.
    AccessDenied,
    /// The settings given to the receiver were invalid.
    InvalidSettings,
    /// Something else went wrong; the technical text is for the log.
    Other,
}

/// Does this Windows error text look like "access denied" (`E_ACCESSDENIED`, 0x80070005)?
fn is_access_denied(text: &str) -> bool {
    text.contains("0x80070005") || text.to_ascii_lowercase().contains("access is denied")
}

/// Decide what to tell the person after a failed start.
///
/// A missing optional feature is suspected when Windows could not activate the receiver and
/// the feature was not seen installed, or when Windows says "not supported" and a check
/// found the feature missing; "unsupported" otherwise points at the adapter.
// UNCERTAIN: which error Windows really gives when the feature is missing is not known (no
// Windows machine was available); the mapping is a best guess, checked by item D2 of
// docs/miracast-device-test-plan.md.
#[must_use]
pub fn diagnose_start_error(error: &MiracastError, probe: &Probe) -> Issue {
    match error {
        MiracastError::Config(_) => Issue::InvalidSettings,
        MiracastError::UnknownConnection => Issue::Other,
        MiracastError::Unsupported => {
            if cfg!(not(windows)) {
                Issue::NotWindows
            } else if probe.feature == FeatureState::NotInstalled {
                Issue::FeatureMissing
            } else {
                Issue::AdapterUnsupported
            }
        }
        MiracastError::Windows(text) => {
            if is_access_denied(text) {
                Issue::AccessDenied
            } else if probe.feature != FeatureState::Installed && !probe.receiver_available {
                Issue::FeatureMissing
            } else {
                Issue::Other
            }
        }
    }
}

/// Decide what to tell the person from the status (while running, or after a probe).
/// `None` means there is nothing to warn about.
#[must_use]
pub fn diagnose_status(status: MiracastStatus, probe: &Probe) -> Option<Issue> {
    match status {
        MiracastStatus::Unsupported => Some(if probe.feature == FeatureState::NotInstalled {
            Issue::FeatureMissing
        } else {
            Issue::AdapterUnsupported
        }),
        MiracastStatus::DisabledByPolicy => Some(Issue::DisabledByPolicy),
        MiracastStatus::Unavailable => Some(Issue::TemporarilyUnavailable),
        MiracastStatus::Failed => Some(Issue::Other),
        _ if probe.adapter == AdapterSupport::NotOptimized => Some(Issue::AdapterNotOptimized),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe(feature: FeatureState, adapter: AdapterSupport, avail: bool) -> Probe {
        Probe {
            feature,
            adapter,
            listening: None,
            receiver_available: avail,
        }
    }

    #[test]
    fn marker_maps_to_state() {
        assert_eq!(
            FeatureState::from_marker(Some(true)),
            FeatureState::Installed
        );
        assert_eq!(
            FeatureState::from_marker(Some(false)),
            FeatureState::NotInstalled
        );
        assert_eq!(FeatureState::from_marker(None), FeatureState::Unknown);
    }

    #[test]
    fn capability_name_is_the_documented_one() {
        assert_eq!(
            WIRELESS_DISPLAY_CAPABILITY,
            "App.WirelessDisplay.Connect~~~~0.0.1.0"
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn not_windows_is_reported_as_such() {
        let p = Probe::default();
        assert_eq!(
            diagnose_start_error(&MiracastError::Unsupported, &p),
            Issue::NotWindows
        );
    }

    #[test]
    fn missing_feature_is_suspected_when_activation_fails() {
        let p = probe(
            FeatureState::NotInstalled,
            AdapterSupport::Undetermined,
            false,
        );
        let e = MiracastError::Windows("MiracastReceiver activation: Class not registered".into());
        assert_eq!(diagnose_start_error(&e, &p), Issue::FeatureMissing);
        // With the feature installed the same error is just "other".
        let p = probe(FeatureState::Installed, AdapterSupport::Undetermined, false);
        assert_eq!(diagnose_start_error(&e, &p), Issue::Other);
        // Activation failed and the feature was not seen: the hint names the feature (the text
        // for it is worded as "may be missing").
        let p = probe(FeatureState::Unknown, AdapterSupport::Undetermined, false);
        assert_eq!(diagnose_start_error(&e, &p), Issue::FeatureMissing);
        // Activation worked: some other problem.
        let p = probe(FeatureState::Unknown, AdapterSupport::Undetermined, true);
        assert_eq!(diagnose_start_error(&e, &p), Issue::Other);
    }

    #[test]
    fn access_denied_is_recognised() {
        let p = Probe::default();
        let e = MiracastError::Windows("CreateSession: Access is denied. (0x80070005)".into());
        assert_eq!(diagnose_start_error(&e, &p), Issue::AccessDenied);
    }

    #[test]
    fn config_errors_are_invalid_settings() {
        let e = MiracastError::Config(crate::ConfigError::EmptyName);
        assert_eq!(
            diagnose_start_error(&e, &Probe::default()),
            Issue::InvalidSettings
        );
    }

    #[test]
    fn status_hints() {
        let none = Probe::default();
        assert_eq!(
            diagnose_status(
                MiracastStatus::Unsupported,
                &probe(
                    FeatureState::NotInstalled,
                    AdapterSupport::Undetermined,
                    true
                )
            ),
            Some(Issue::FeatureMissing)
        );
        assert_eq!(
            diagnose_status(MiracastStatus::Unsupported, &none),
            Some(Issue::AdapterUnsupported)
        );
        assert_eq!(
            diagnose_status(MiracastStatus::Unavailable, &none),
            Some(Issue::TemporarilyUnavailable)
        );
        assert_eq!(
            diagnose_status(MiracastStatus::DisabledByPolicy, &none),
            Some(Issue::DisabledByPolicy)
        );
        assert_eq!(diagnose_status(MiracastStatus::Listening, &none), None);
        assert_eq!(
            diagnose_status(
                MiracastStatus::Listening,
                &probe(FeatureState::Installed, AdapterSupport::NotOptimized, true)
            ),
            Some(Issue::AdapterNotOptimized)
        );
    }
}
