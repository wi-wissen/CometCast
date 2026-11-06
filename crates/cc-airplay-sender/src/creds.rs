// SPDX-License-Identifier: GPL-3.0-or-later
//! Credentials created by PIN pairing and the file that keeps them.
//!
//! The file holds long-term secret keys of this sender, so it is created with mode 0600 on
//! Unix and written atomically. On other systems no access restriction is set
//! (UNCERTAIN: Windows ACLs are not handled; the plan says "system key store" for releases).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cryptoutil::{hex, random};
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

    /// Save (or replace) the credentials for a device key. Returns whether an entry for the
    /// key existed before (the caller should tell the user: the key comes from mDNS, which
    /// anybody on the network can fake, see `docs/airplay-sender.md`).
    pub fn put(&self, key: &str, creds: Credentials) -> Result<bool> {
        let mut all = self.load_all()?;
        let replaced = all.insert(key.to_owned(), creds).is_some();
        let text =
            serde_json::to_string_pretty(&all).map_err(|e| SenderError::Store(e.to_string()))?;
        let dir = match self.path.parent() {
            Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
            _ => PathBuf::from("."),
        };
        create_private_dir(&dir)?;
        // A fresh, unpredictable name that must not exist yet (no stale file, no planted link).
        let tmp = dir.join(format!(".airplay-pairings.{}.tmp", hex(&random::<8>())));
        let _ = std::fs::remove_file(&tmp);
        let result = write_private(&tmp, text.as_bytes())
            .and_then(|()| std::fs::rename(&tmp, &self.path).map_err(store_err))
            .and_then(|()| sync_dir(&dir));
        if result.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        result.map(|()| replaced)
    }
}

fn store_err(e: std::io::Error) -> SenderError {
    SenderError::Store(e.to_string())
}

/// Create the directory (and parents) with mode 0700 where it does not exist yet.
fn create_private_dir(dir: &Path) -> Result<()> {
    let mut b = std::fs::DirBuilder::new();
    b.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        b.mode(0o700);
    }
    b.create(dir).map_err(store_err)
}

/// Make the rename durable.
#[cfg(unix)]
fn sync_dir(dir: &Path) -> Result<()> {
    std::fs::File::open(dir)
        .and_then(|d| d.sync_all())
        .map_err(store_err)
}

#[cfg(not(unix))]
fn sync_dir(_dir: &Path) -> Result<()> {
    Ok(())
}

/// Create a new file (it must not exist) with mode 0600, write, and fsync it.
fn write_private(path: &Path, data: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    let mut f = o.open(path).map_err(store_err)?;
    f.write_all(data).map_err(store_err)?;
    f.sync_all().map_err(store_err)
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

    #[cfg(unix)]
    #[test]
    fn modes_are_private_even_over_loose_existing_files() {
        use std::os::unix::fs::PermissionsExt;
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        let dir = temp_dir("modes");
        let store = PairingStore::at(dir.join("a").join("b").join("p.json"));
        let c = Credentials::Legacy {
            client_id: "00AA00AA00AA00AA".into(),
            seed: "11".repeat(32),
        };
        assert!(!store.put("k", c.clone()).unwrap());
        assert_eq!(mode(store.path().parent().unwrap()), 0o700);
        // A store file that was world readable is replaced by a private one.
        let loose = std::fs::Permissions::from_mode(0o644);
        std::fs::set_permissions(store.path(), loose).unwrap();
        assert_eq!(mode(store.path()), 0o644);
        assert!(store.put("k", c.clone()).unwrap(), "replaced");
        assert_eq!(mode(store.path()), 0o600);
        // No temporary file is left behind.
        let left: Vec<_> = std::fs::read_dir(store.path().parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name())
            .collect();
        assert_eq!(left.len(), 1, "{left:?}");
        // A planted file at a temporary name cannot be reused: create_new refuses.
        let planted = dir.join("planted");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&planted, b"x").unwrap();
        assert!(write_private(&planted, b"secret").is_err());
        assert_eq!(std::fs::read(&planted).unwrap(), b"x");
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
