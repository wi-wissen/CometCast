// SPDX-License-Identifier: GPL-3.0-or-later
//! The app's device identity for the core (`docs/design/presenter-identity.md`): the key that
//! signs this installation's joins as a presenter, and what the display remembers about names
//! and devices. Both are set by the app (from its settings) before it starts a presenter or a
//! receiver; the core never writes either anywhere.

use std::sync::Mutex;

use cc_receiver::DeviceMemory;
use cc_transport::device::DeviceKey;

static PRESENTER_KEY: Mutex<Option<DeviceKey>> = Mutex::new(None);
static MEMORY: Mutex<Option<DeviceMemory>> = Mutex::new(None);

/// Use `secret` (32 bytes) to sign joins as a presenter; `false` (and no key) for any other
/// length.
pub(crate) fn set_presenter_key(secret: &[u8]) -> bool {
    let key = DeviceKey::from_secret(secret);
    let ok = key.is_some();
    *PRESENTER_KEY.lock().unwrap_or_else(|p| p.into_inner()) = key;
    ok
}

/// The key that signs joins, if the app set one.
pub(crate) fn presenter_key() -> Option<DeviceKey> {
    PRESENTER_KEY
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
}

/// Set what the display remembers (the text the app stored, see `DeviceMemory::from_json`).
pub(crate) fn set_memory(json: &str) {
    *MEMORY.lock().unwrap_or_else(|p| p.into_inner()) = Some(DeviceMemory::from_json(json));
}

/// What the display remembers (empty when the app set nothing).
pub(crate) fn memory() -> DeviceMemory {
    MEMORY
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone()
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_and_memory_are_taken_from_the_app() {
        assert!(!set_presenter_key(&[1; 31]));
        assert!(presenter_key().is_none());
        assert!(set_presenter_key(&[1; 32]));
        assert!(presenter_key().is_some());
        set_memory("not json");
        assert!(memory().is_empty());
        let id = "0123456789abcdef0123456789abcdef";
        set_memory(&format!(r#"[["Ana","{id}"]]"#));
        assert_eq!(memory().len(), 1);
    }
}
