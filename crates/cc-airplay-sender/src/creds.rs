// SPDX-License-Identifier: GPL-3.0-or-later
//! Credentials created by PIN pairing and the file that keeps them.
//!
//! The file holds long-term secret keys of this sender, so it is created with mode 0600 on
//! Unix and written atomically. On other systems no access restriction is set
//! (UNCERTAIN: Windows ACLs are not handled; the plan says "system key store" for releases).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Result, SenderError};

/// What we keep after a successful pairing. Seeds are secret: `Debug` hides them.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Credentials {
    /// Legacy AirPlay PIN pairing (`/pair-setup-pin`).
    Legacy {
        /// Our client id, 16 upper-case hex digits.
        client_id: String,
        /// Ed25519 seed, 64 hex digits.
        seed: String,
    },
    /// HomeKit-style PIN pairing (`/pair-setup` with TLV8).
    Hap {
        /// Our pairing id (text, as sent in the TLV).
        pairing_id: String,
        /// Our Ed25519 seed, 64 hex digits.
        seed: String,
        /// The receiver's identifier as hex.
        device_id: String,
        /// The receiver's long-term Ed25519 public key, 64 hex digits.
        device_key: String,
    },
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Legacy { client_id, .. } => f
                .debug_struct("Legacy")
                .field("client_id", client_id)
                .field("seed", &"<hidden>")
                .finish(),
            Self::Hap {
                pairing_id,
                device_id,
                ..
            } => f
                .debug_struct("Hap")
                .field("pairing_id", pairing_id)
                .field("device_id", device_id)
                .field("seed", &"<hidden>")
                .finish_non_exhaustive(),
        }
    }
}

/// A JSON file with the credentials of all paired receivers, keyed by device id.
#[derive(Debug, Clone)]
pub struct PairingStore {
    path: PathBuf,
}

/// Biggest store file we read.
const MAX_STORE_BYTES: u64 = 1024 * 1024;

impl PairingStore {
    /// Use this file.
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// The default place: `$XDG_CONFIG_HOME/cometcast/airplay-pairings.json`, else
    /// `$HOME/.config/cometcast/airplay-pairings.json`.
    pub fn default_location() -> Result<Self> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .filter(|v| !v.is_empty())
                    .map(|h| PathBuf::from(h).join(".config"))
            })
            .ok_or_else(|| SenderError::Store("neither XDG_CONFIG_HOME nor HOME is set".into()))?;
        Ok(Self::at(
            base.join("cometcast").join("airplay-pairings.json"),
        ))
    }

    /// The file path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn load_all(&self) -> Result<BTreeMap<String, Credentials>> {
        let meta = match std::fs::metadata(&self.path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
            Err(e) => return Err(SenderError::Store(e.to_string())),
        };
        if meta.len() > MAX_STORE_BYTES {
            return Err(SenderError::Store("store file is too big".into()));
        }
        let text =
            std::fs::read_to_string(&self.path).map_err(|e| SenderError::Store(e.to_string()))?;
        serde_json::from_str(&text).map_err(|e| SenderError::Store(format!("bad store file: {e}")))
    }

    /// Credentials for a device key, if any.
    pub fn get(&self, key: &str) -> Result<Option<Credentials>> {
        Ok(self.load_all()?.remove(key))
    }

    /// Save (or replace) the credentials for a device key.
    pub fn put(&self, key: &str, creds: Credentials) -> Result<()> {
        let mut all = self.load_all()?;
        all.insert(key.to_owned(), creds);
        let text =
            serde_json::to_string_pretty(&all).map_err(|e| SenderError::Store(e.to_string()))?;
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| SenderError::Store(e.to_string()))?;
        }
        let tmp = self.path.with_extension("json.tmp");
        write_private(&tmp, text.as_bytes())?;
        std::fs::rename(&tmp, &self.path).map_err(|e| SenderError::Store(e.to_string()))
    }
}

#[cfg(unix)]
fn write_private(path: &Path, data: &[u8]) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| SenderError::Store(e.to_string()))?;
    f.write_all(data)
        .map_err(|e| SenderError::Store(e.to_string()))
}

#[cfg(not(unix))]
fn write_private(path: &Path, data: &[u8]) -> Result<()> {
    std::fs::write(path, data).map_err(|e| SenderError::Store(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("cc-airplay-sender-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn store_roundtrip_with_private_mode() {
        let dir = temp_dir("store");
        let store = PairingStore::at(dir.join("sub").join("p.json"));
        assert_eq!(store.get("a").unwrap(), None);
        let c = Credentials::Legacy {
            client_id: "00AA00AA00AA00AA".into(),
            seed: "11".repeat(32),
        };
        store.put("a", c.clone()).unwrap();
        assert_eq!(store.get("a").unwrap(), Some(c));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(store.path())
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_and_huge_files_are_errors() {
        let dir = temp_dir("corrupt");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("p.json");
        std::fs::write(&p, b"{not json").unwrap();
        assert!(PairingStore::at(&p).get("a").is_err());
        std::fs::write(&p, vec![b' '; MAX_STORE_BYTES as usize + 1]).unwrap();
        assert!(PairingStore::at(&p).get("a").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn debug_hides_seed() {
        let c = Credentials::Legacy {
            client_id: "X".into(),
            seed: "secretseed".into(),
        };
        assert!(!format!("{c:?}").contains("secretseed"));
    }
}
