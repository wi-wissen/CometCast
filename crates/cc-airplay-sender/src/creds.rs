// SPDX-License-Identifier: GPL-3.0-or-later
//! Credentials created by PIN pairing and the file that keeps them.
//!
//! The file holds long-term secret keys of this sender, so it is created with mode 0600 on
//! Unix and written atomically. On other systems no access restriction is set
//! (UNCERTAIN: Windows ACLs are not handled; the plan says "system key store" for releases).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::cryptoutil::{hex, random, unhex};
use crate::discovery::Device;
use crate::error::{Result, SenderError, clean};

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
        /// The receiver's long-term Ed25519 public key (64 hex digits), remembered when the
        /// pairing was made (trust on first use). `None` in files written by older versions
        /// and for receivers that announced no usable key: pair-verify then cannot check who
        /// answers (see `legacy::pair_verify`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        receiver_key: Option<String>,
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

impl Credentials {
    /// Remember the receiver's long-term Ed25519 public key in legacy credentials, so that
    /// later pair-verify runs check the receiver's signature against it. `key_hex` is the
    /// `pk` value of the receiver's mDNS record ([`Device::public_key`]); a missing value or
    /// one that is not 64 hex digits of a valid key leaves the credentials without a key.
    /// HomeKit credentials already hold the receiver's key and are returned unchanged.
    ///
    /// UNCERTAIN: the mDNS record is not authenticated. A device that answers in place of the
    /// receiver while the pairing is made can get its own key remembered (trust on first use).
    pub fn with_receiver_key(self, key_hex: Option<&str>) -> Self {
        match self {
            Self::Legacy {
                client_id, seed, ..
            } => {
                let receiver_key = key_hex
                    .and_then(unhex::<32>)
                    .filter(|k| ed25519_dalek::VerifyingKey::from_bytes(k).is_ok())
                    .map(|k| hex(&k));
                Self::Legacy {
                    client_id,
                    seed,
                    receiver_key,
                }
            }
            other => other,
        }
    }
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

/// One entry of the store file: the credentials, and the name the receiver had when it was
/// paired. The name is written next to the fields of the credentials, so files of older
/// versions (no name) still load and older versions ignore the name.
#[derive(Clone, Serialize, Deserialize)]
struct Entry {
    /// The receiver's cleaned name at pairing time; `None` = unknown (older file).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(flatten)]
    creds: Credentials,
}

/// Longest receiver name kept in the store (the length [`Device::name`] is cut to).
const MAX_NAME_CHARS: usize = 128;

/// The form in which receiver names are compared: cleaned, without outer spaces, lower case.
fn name_key(name: &str) -> String {
    clean(name, MAX_NAME_CHARS).trim().to_lowercase()
}

/// What the store knows about a receiver ([`PairingStore::lookup`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lookup {
    /// Credentials are stored for this receiver's key.
    Paired(Credentials),
    /// Nothing is stored for this receiver's key, but a receiver with the same name was
    /// paired under another key. The device id comes from mDNS, which anybody on the network
    /// can fake: this may be an impostor that copies the name of a paired receiver, so it
    /// must not be used without authentication (pair it with its PIN, or refuse).
    NameClash,
    /// Nothing is stored for this key or this name.
    Unknown,
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

    fn load_all(&self) -> Result<BTreeMap<String, Entry>> {
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
        Ok(self.load_all()?.remove(key).map(|e| e.creds))
    }

    /// What is stored for a receiver: its credentials (by [`Device::store_key`]), or the fact
    /// that another key holds credentials for a receiver of the same name. Callers must not
    /// use a [`Lookup::NameClash`] receiver without authentication.
    pub fn lookup(&self, device: &Device) -> Result<Lookup> {
        let mut all = self.load_all()?;
        if let Some(e) = all.remove(&device.store_key()) {
            return Ok(Lookup::Paired(e.creds));
        }
        let name = name_key(&device.name);
        let clash = !name.is_empty()
            && all
                .values()
                .any(|e| e.name.as_deref().is_some_and(|n| name_key(n) == name));
        Ok(if clash {
            Lookup::NameClash
        } else {
            Lookup::Unknown
        })
    }

    /// Save (or replace) the credentials for a device key. Returns whether an entry for the
    /// key existed before (the caller should tell the user: the key comes from mDNS, which
    /// anybody on the network can fake, see `docs/airplay-sender.md`).
    ///
    /// No name is stored with the entry (a name already stored for the key is kept), so
    /// [`PairingStore::lookup`] cannot warn about another device that uses this receiver's
    /// name: prefer [`PairingStore::put_named`].
    pub fn put(&self, key: &str, creds: Credentials) -> Result<bool> {
        self.put_entry(key, None, creds)
    }

    /// Like [`PairingStore::put`], and remember the receiver's name ([`Device::name`]) with
    /// the credentials, for [`PairingStore::lookup`].
    pub fn put_named(&self, key: &str, name: &str, creds: Credentials) -> Result<bool> {
        self.put_entry(key, Some(name), creds)
    }

    fn put_entry(&self, key: &str, name: Option<&str>, creds: Credentials) -> Result<bool> {
        let mut all = self.load_all()?;
        let name = match name {
            Some(n) => Some(clean(n, MAX_NAME_CHARS)).filter(|n| !n.trim().is_empty()),
            None => all.get(key).and_then(|e| e.name.clone()),
        };
        let replaced = all.insert(key.to_owned(), Entry { name, creds }).is_some();
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
            receiver_key: None,
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
            receiver_key: None,
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
            receiver_key: None,
        };
        assert!(!format!("{c:?}").contains("secretseed"));
    }

    fn legacy() -> Credentials {
        Credentials::Legacy {
            client_id: "00AA00AA00AA00AA".into(),
            seed: "11".repeat(32),
            receiver_key: None,
        }
    }

    fn device(name: &str, id: &str) -> Device {
        crate::discovery::device_from_parts(
            &format!("{name}._airplay._tcp.local."),
            "tv.local.",
            7000,
            ["192.168.1.9".parse().unwrap()],
            [("deviceid".to_owned(), id.to_owned())],
        )
    }

    #[test]
    fn a_new_device_id_with_the_name_of_a_paired_receiver_is_a_clash() {
        let dir = temp_dir("clash");
        let store = PairingStore::at(dir.join("p.json"));
        let tv = device("Living Room", "AA:BB:CC:DD:EE:01");
        assert_eq!(store.lookup(&tv).unwrap(), Lookup::Unknown);
        assert!(
            !store
                .put_named(&tv.store_key(), &tv.name, legacy())
                .unwrap()
        );
        assert_eq!(store.lookup(&tv).unwrap(), Lookup::Paired(legacy()));
        // Same name, another device id: an impostor (or a replaced TV); never "unknown".
        let other = device("Living Room", "AA:BB:CC:DD:EE:02");
        assert_eq!(store.lookup(&other).unwrap(), Lookup::NameClash);
        // Case and outer spaces do not hide the clash.
        let disguised = device(" living ROOM ", "AA:BB:CC:DD:EE:03");
        assert_eq!(store.lookup(&disguised).unwrap(), Lookup::NameClash);
        // Another name is just an unknown device.
        let kitchen = device("Kitchen", "AA:BB:CC:DD:EE:04");
        assert_eq!(store.lookup(&kitchen).unwrap(), Lookup::Unknown);
        // A plain `put` over the entry keeps the stored name.
        assert!(store.put(&tv.store_key(), legacy()).unwrap());
        assert_eq!(store.lookup(&other).unwrap(), Lookup::NameClash);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn files_of_older_versions_load_and_have_no_names() {
        let dir = temp_dir("oldfile");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("p.json");
        // The format before names and receiver keys were stored.
        std::fs::write(
            &p,
            format!(
                r#"{{"aa:bb:cc:dd:ee:01": {{"kind": "legacy", "client_id": "00AA00AA00AA00AA", "seed": "{}"}},
                    "aa:bb:cc:dd:ee:05": {{"kind": "hap", "pairing_id": "p", "seed": "{}", "device_id": "d", "device_key": "k"}}}}"#,
                "11".repeat(32),
                "22".repeat(32)
            ),
        )
        .unwrap();
        let store = PairingStore::at(&p);
        assert_eq!(store.get("aa:bb:cc:dd:ee:01").unwrap(), Some(legacy()));
        assert!(matches!(
            store.get("aa:bb:cc:dd:ee:05").unwrap(),
            Some(Credentials::Hap { .. })
        ));
        // The name is unknown, so no clash can be reported for old entries.
        let other = device("Living Room", "AA:BB:CC:DD:EE:02");
        assert_eq!(store.lookup(&other).unwrap(), Lookup::Unknown);
        // Writing keeps the old entries readable and adds the name only to the new one.
        store
            .put_named(&other.store_key(), &other.name, legacy())
            .unwrap();
        let text = std::fs::read_to_string(&p).unwrap();
        assert_eq!(text.matches("\"name\"").count(), 1, "{text}");
        assert!(!text.contains("receiver_key"), "{text}");
        assert!(matches!(
            store.get("aa:bb:cc:dd:ee:05").unwrap(),
            Some(Credentials::Hap { .. })
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn receiver_key_is_kept_only_when_it_is_a_key() {
        let key = |c: Credentials| match c {
            Credentials::Legacy { receiver_key, .. } => receiver_key,
            Credentials::Hap { .. } => panic!("legacy expected"),
        };
        let pk = ed25519_dalek::SigningKey::from_bytes(&[7; 32])
            .verifying_key()
            .to_bytes();
        let pk_hex = hex(&pk);
        assert_eq!(
            key(legacy().with_receiver_key(Some(&pk_hex.to_uppercase()))),
            Some(pk_hex.clone())
        );
        assert_eq!(key(legacy().with_receiver_key(None)), None);
        assert_eq!(key(legacy().with_receiver_key(Some("aa"))), None);
        assert_eq!(
            key(legacy().with_receiver_key(Some(&"zz".repeat(32)))),
            None
        );
        // The key survives the store.
        let dir = temp_dir("rkey");
        let store = PairingStore::at(dir.join("p.json"));
        let c = legacy().with_receiver_key(Some(&pk_hex));
        store.put("a", c.clone()).unwrap();
        assert_eq!(store.get("a").unwrap(), Some(c));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
