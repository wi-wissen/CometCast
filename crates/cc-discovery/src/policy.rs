// SPDX-License-Identifier: GPL-3.0-or-later
//! Server policy set by an administrator (MDM), read from a file in a managed location.
//!
//! UNCERTAIN: the formats in which real device management products (Microsoft Intune, Jamf,
//! other MDM) deliver such a file are NOT verified and nothing here was tested against them.
//! What is implemented: reading one JSON file (Linux, Windows) or one property list (macOS)
//! from a fixed path that an administrator can write and ordinary users should not. Delivering
//! the file to that path (a configuration profile, a script, a package) is up to the
//! administrator. Registry values and Android managed configurations are not read.
//!
//! File format (JSON; the plist uses the same keys):
//!
//! ```json
//! {
//!   "version": 1,
//!   "servers": ["wss://relay.example.org:443", "10.0.0.5:8443"],
//!   "forbid_default_list": true,
//!   "forbid_user_override": false,
//!   "forbid_announcement": true,
//!   "mode": "internal"
//! }
//! ```
//!
//! No environment variable moves the policy path (a user could redirect it). The `--policy-file`
//! option of `cometcast-cli discover` is a development tool only.
//!
//! Unknown keys make the whole file invalid (a typo in a security setting must not be ignored
//! silently). An invalid or unreadable policy file that exists is treated as a locked, empty
//! policy ([`PolicyState::Invalid`]): the other sources are NOT used. UNCERTAIN: failing closed
//! is a design decision of this crate; the project owner has not confirmed it.

use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::address::{Trust, normalize_server};

/// Largest policy file read, in bytes.
pub const MAX_POLICY_BYTES: u64 = 64 * 1024;
/// Most servers a policy may list.
pub const MAX_POLICY_SERVERS: usize = 16;

/// A hint which server mode the organisation runs. Only a hint for the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeHint {
    /// A server open to the public internet (see docs/server.md, open mode).
    Open,
    /// An internal server (see docs/server.md, internal mode).
    Internal,
}

/// A validated policy.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServerPolicy {
    /// Servers in the order of preference, normalised ([`normalize_server`], admin trust).
    pub servers: Vec<String>,
    /// Do not use the built-in default list.
    pub forbid_default_list: bool,
    /// Use only the servers of this policy: no DNS-SRV, no announcement, no default list, and
    /// the UI must not let people type another server.
    pub forbid_user_override: bool,
    /// Do not use servers announced by display devices.
    pub forbid_announcement: bool,
    /// Which server mode the organisation runs, if stated.
    pub mode: Option<ModeHint>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPolicy {
    #[serde(default)]
    version: Option<u32>,
    #[serde(default)]
    servers: Vec<String>,
    #[serde(default)]
    forbid_default_list: bool,
    #[serde(default)]
    forbid_user_override: bool,
    #[serde(default)]
    forbid_announcement: bool,
    #[serde(default)]
    mode: Option<String>,
}

/// Why a policy was refused. Messages never contain file content.
#[derive(Debug, thiserror::Error)]
pub enum PolicyError {
    /// The file is bigger than [`MAX_POLICY_BYTES`].
    #[error("policy file is larger than {MAX_POLICY_BYTES} bytes")]
    TooLarge,
    /// The file could not be parsed.
    #[error("policy file is not valid ({0})")]
    Parse(String),
    /// A value is not acceptable.
    #[error("policy value refused: {0}")]
    Value(String),
    /// The file could not be read.
    #[error("policy file cannot be read: {0}")]
    Io(String),
    /// The file may be changed by people who are not administrators.
    #[error("policy file is not protected: {0}")]
    Unprotected(&'static str),
}

impl ServerPolicy {
    /// Parse and validate a JSON policy.
    pub fn from_json(bytes: &[u8]) -> Result<Self, PolicyError> {
        if bytes.len() as u64 > MAX_POLICY_BYTES {
            return Err(PolicyError::TooLarge);
        }
        // serde would accept `[]` (or `[1]`) as a struct made of defaults: demand an object.
        if bytes.iter().find(|b| !b.is_ascii_whitespace()) != Some(&b'{') {
            return Err(PolicyError::Parse("not a JSON object".into()));
        }
        let raw: RawPolicy = serde_json::from_slice(bytes)
            // serde_json errors name only line and column and the kind of problem.
            .map_err(|e| PolicyError::Parse(e.to_string()))?;
        Self::from_raw(raw)
    }

    /// Parse and validate a property list policy (macOS managed preferences).
    pub fn from_plist(bytes: &[u8]) -> Result<Self, PolicyError> {
        if bytes.len() as u64 > MAX_POLICY_BYTES {
            return Err(PolicyError::TooLarge);
        }
        let raw: RawPolicy =
            plist::from_bytes(bytes).map_err(|e| PolicyError::Parse(e.to_string()))?;
        Self::from_raw(raw)
    }

    fn from_raw(raw: RawPolicy) -> Result<Self, PolicyError> {
        match raw.version {
            None | Some(1) => {}
            Some(_) => return Err(PolicyError::Value("unknown version".into())),
        }
        if raw.servers.len() > MAX_POLICY_SERVERS {
            return Err(PolicyError::Value(format!(
                "more than {MAX_POLICY_SERVERS} servers"
            )));
        }
        let mut servers: Vec<String> = Vec::with_capacity(raw.servers.len());
        for (i, s) in raw.servers.iter().enumerate() {
            let n = normalize_server(s, Trust::Admin)
                .map_err(|e| PolicyError::Value(format!("servers[{i}]: {e}")))?;
            if !servers.contains(&n) {
                servers.push(n);
            }
        }
        let mode = match raw.mode.as_deref() {
            None => None,
            Some("open") => Some(ModeHint::Open),
            Some("internal") => Some(ModeHint::Internal),
            Some(_) => return Err(PolicyError::Value("mode must be open or internal".into())),
        };
        Ok(Self {
            servers,
            forbid_default_list: raw.forbid_default_list,
            forbid_user_override: raw.forbid_user_override,
            forbid_announcement: raw.forbid_announcement,
            mode,
        })
    }
}

/// What the lookup found about policy.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PolicyState {
    /// No policy file exists.
    #[default]
    Absent,
    /// A valid policy.
    Active(ServerPolicy),
    /// A policy file exists but cannot be used. Fails closed: no server from other sources.
    Invalid(String),
}

/// File format of a policy location.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyFormat {
    /// JSON.
    Json,
    /// Property list (XML or binary).
    Plist,
}

/// Where a policy file may be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyLocation {
    /// File path.
    pub path: PathBuf,
    /// Format of the file.
    pub format: PolicyFormat,
    /// On Unix: demand that root owns the file. (Always checked: not writable by group/others.)
    pub require_root_owner: bool,
}

/// The operating systems with a known policy location.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyOs {
    /// Linux and other Unix systems.
    Linux,
    /// macOS.
    Macos,
    /// Windows.
    Windows,
}

impl PolicyOs {
    /// The system this program was built for.
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Macos
        } else if cfg!(windows) {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}

/// The managed locations for `os`, first match wins. The paths are fixed: no environment
/// variable is read.
///
/// UNCERTAIN: the macOS file name uses the bundle id of the Flutter app
/// (`de.cometcast.cometcast`); a configuration profile with that preference domain is expected
/// to produce `/Library/Managed Preferences/<domain>.plist`, which is NOT verified. The Windows
/// path `C:\ProgramData\CometCast\policy.json` is compile-checked only, never run on Windows.
pub fn policy_locations_for(os: PolicyOs) -> Vec<PolicyLocation> {
    match os {
        PolicyOs::Linux => vec![PolicyLocation {
            path: PathBuf::from("/etc/cometcast/policy.json"),
            format: PolicyFormat::Json,
            require_root_owner: true,
        }],
        PolicyOs::Macos => vec![PolicyLocation {
            path: PathBuf::from("/Library/Managed Preferences/de.cometcast.cometcast.plist"),
            format: PolicyFormat::Plist,
            require_root_owner: true,
        }],
        PolicyOs::Windows => {
            // The `ProgramData` environment variable is NOT used: a standard user can set it for
            // their own processes and would redirect the policy. The fixed default is right
            // on a normal installation.
            // UNCERTAIN: SHGetKnownFolderPath(FOLDERID_ProgramData) would be exact if the
            // folder was moved, but needs unsafe code, which this crate forbids.
            vec![PolicyLocation {
                path: PathBuf::from(r"C:\ProgramData")
                    .join("CometCast")
                    .join("policy.json"),
                format: PolicyFormat::Json,
                require_root_owner: false,
            }]
        }
    }
}

/// The locations for the running system.
pub fn default_policy_locations() -> Vec<PolicyLocation> {
    policy_locations_for(PolicyOs::current())
}

/// Load the first policy file that exists. Blocking (file access).
pub fn load_policy(locations: &[PolicyLocation]) -> PolicyState {
    for loc in locations {
        match read_location(loc) {
            Ok(None) => continue,
            Ok(Some(p)) => return PolicyState::Active(p),
            Err(e) => return PolicyState::Invalid(e.to_string()),
        }
    }
    PolicyState::Absent
}

/// Open for reading without ever blocking: a FIFO at the policy path must not hang the
/// start-up (opening a FIFO for reading waits for a writer unless `O_NONBLOCK` is set).
fn open_nonblocking(path: &Path) -> std::io::Result<std::fs::File> {
    let mut o = std::fs::OpenOptions::new();
    o.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.custom_flags(libc::O_NONBLOCK);
    }
    o.open(path)
}

fn read_location(loc: &PolicyLocation) -> Result<Option<ServerPolicy>, PolicyError> {
    // Look at the path itself first: a FIFO, socket or device (not a symlink, which may lead
    // to a regular file) is refused without opening it.
    match std::fs::symlink_metadata(&loc.path) {
        Ok(m) if !m.is_file() && !m.file_type().is_symlink() => {
            return Err(PolicyError::Unprotected("not a regular file"));
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(PolicyError::Io(e.kind().to_string())),
    }
    check_parent(loc)?;
    let file = match open_nonblocking(&loc.path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(PolicyError::Io(e.kind().to_string())),
    };
    let meta = file
        .metadata()
        .map_err(|e| PolicyError::Io(e.kind().to_string()))?;
    if !meta.is_file() {
        return Err(PolicyError::Unprotected("not a regular file"));
    }
    check_protection(&meta, loc)?;
    if meta.len() > MAX_POLICY_BYTES {
        return Err(PolicyError::TooLarge);
    }
    let mut buf = Vec::new();
    file.take(MAX_POLICY_BYTES + 1)
        .read_to_end(&mut buf)
        .map_err(|e| PolicyError::Io(e.kind().to_string()))?;
    match loc.format {
        PolicyFormat::Json => ServerPolicy::from_json(&buf),
        PolicyFormat::Plist => ServerPolicy::from_plist(&buf),
    }
    .map(Some)
}

/// The directory that holds the file decides who can replace the file: it must not be
/// writable by group or others, and for the system paths it must be owned by root.
/// Only the direct parent is checked. UNCERTAIN: a writable directory further up could still
/// rename the parent; ancestors are not walked because sticky directories such as `/tmp` are
/// legitimate ancestors in tests and the system paths are root-owned by default.
#[cfg(unix)]
fn check_parent(loc: &PolicyLocation) -> Result<(), PolicyError> {
    use std::os::unix::fs::MetadataExt;
    let parent = match loc.path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    let meta = std::fs::metadata(parent).map_err(|e| PolicyError::Io(e.kind().to_string()))?;
    if !meta.is_dir() {
        return Err(PolicyError::Unprotected("parent is not a directory"));
    }
    if meta.mode() & 0o022 != 0 {
        return Err(PolicyError::Unprotected(
            "directory writable by group or others",
        ));
    }
    if loc.require_root_owner && meta.uid() != 0 {
        return Err(PolicyError::Unprotected("directory not owned by root"));
    }
    Ok(())
}

#[cfg(not(unix))]
fn check_parent(_loc: &PolicyLocation) -> Result<(), PolicyError> {
    Ok(())
}

#[cfg(unix)]
fn check_protection(meta: &std::fs::Metadata, loc: &PolicyLocation) -> Result<(), PolicyError> {
    use std::os::unix::fs::MetadataExt;
    if meta.mode() & 0o022 != 0 {
        return Err(PolicyError::Unprotected("writable by group or others"));
    }
    if loc.require_root_owner && meta.uid() != 0 {
        return Err(PolicyError::Unprotected("not owned by root"));
    }
    Ok(())
}

// UNCERTAIN: no access-control-list check on Windows; whoever can write the file can change
// the policy. The ProgramData folder is writable by administrators only by default, but that
// was not verified here.
#[cfg(not(unix))]
fn check_protection(_meta: &std::fs::Metadata, _loc: &PolicyLocation) -> Result<(), PolicyError> {
    Ok(())
}

/// Convenience for tests and tools: a location for an explicit file (format from extension).
pub fn explicit_location(path: &Path) -> PolicyLocation {
    let plist = path.extension().is_some_and(|e| e == "plist");
    PolicyLocation {
        path: path.to_path_buf(),
        format: if plist {
            PolicyFormat::Plist
        } else {
            PolicyFormat::Json
        },
        require_root_owner: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn json(s: &str) -> Result<ServerPolicy, PolicyError> {
        ServerPolicy::from_json(s.as_bytes())
    }

    #[test]
    fn full_policy_parses() {
        let p = json(
            r#"{"version":1,"servers":["WSS://Relay.example.org:443","10.0.0.5:8443","10.0.0.5:8443"],
                "forbid_default_list":true,"forbid_user_override":true,"forbid_announcement":true,
                "mode":"internal"}"#,
        )
        .unwrap();
        assert_eq!(p.servers, ["wss://relay.example.org:443", "10.0.0.5:8443"]);
        assert!(p.forbid_default_list && p.forbid_user_override && p.forbid_announcement);
        assert_eq!(p.mode, Some(ModeHint::Internal));
    }

    #[test]
    fn empty_object_is_a_policy_that_changes_nothing() {
        assert_eq!(json("{}").unwrap(), ServerPolicy::default());
    }

    #[test]
    fn typos_and_bad_values_are_errors() {
        assert!(json(r#"{"forbid_default_lst":true}"#).is_err());
        assert!(json(r#"{"servers":"a:1"}"#).is_err());
        assert!(json(r#"{"servers":["bad host:1"]}"#).is_err());
        assert!(json(r#"{"servers":["x.example"]}"#).is_err());
        assert!(json(r#"{"mode":"both"}"#).is_err());
        assert!(json(r#"{"version":2}"#).is_err());
        assert!(json("not json").is_err());
        assert!(json("[]").is_err());
        assert!(json("").is_err());
    }

    #[test]
    fn size_caps() {
        let many = format!(
            r#"{{"servers":[{}]}}"#,
            vec![r#""a.example:1""#; MAX_POLICY_SERVERS + 1].join(",")
        );
        assert!(matches!(json(&many), Err(PolicyError::Value(_))));
        let big = format!(r#"{{"servers":["{}"]}}"#, "a".repeat(70_000));
        assert!(matches!(json(&big), Err(PolicyError::TooLarge)));
    }

    #[test]
    fn error_messages_do_not_leak_values() {
        let e = json(r#"{"servers":["secret@host:1"]}"#).unwrap_err();
        assert!(!e.to_string().contains("secret"));
    }

    #[test]
    fn plist_policy_parses() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>servers</key><array><string>relay.example.org:443</string></array>
<key>forbid_default_list</key><true/>
<key>mode</key><string>open</string>
</dict></plist>"#;
        let p = ServerPolicy::from_plist(xml).unwrap();
        assert_eq!(p.servers, ["relay.example.org:443"]);
        assert!(p.forbid_default_list);
        assert_eq!(p.mode, Some(ModeHint::Open));
        assert!(ServerPolicy::from_plist(b"garbage").is_err());
    }

    #[test]
    fn locations_per_os() {
        let l = policy_locations_for(PolicyOs::Linux);
        assert_eq!(l[0].path, Path::new("/etc/cometcast/policy.json"));
        let m = policy_locations_for(PolicyOs::Macos);
        assert_eq!(m[0].format, PolicyFormat::Plist);
        let w = policy_locations_for(PolicyOs::Windows);
        assert_eq!(
            w[0].path,
            Path::new(r"C:\ProgramData")
                .join("CometCast")
                .join("policy.json")
        );
        // The environment is not consulted, whatever it says.
        // (Safe to check on every system: the function takes no environment.)
        assert_eq!(w, policy_locations_for(PolicyOs::Windows));
    }

    #[test]
    fn missing_file_is_absent() {
        let dir = tempfile::tempdir().unwrap();
        let loc = explicit_location(&dir.path().join("nope.json"));
        assert_eq!(load_policy(&[loc]), PolicyState::Absent);
        assert_eq!(load_policy(&[]), PolicyState::Absent);
    }

    #[cfg(unix)]
    mod files {
        use super::*;
        use std::os::unix::fs::PermissionsExt;

        fn write(dir: &Path, name: &str, body: &[u8], mode: u32) -> PolicyLocation {
            let p = dir.join(name);
            std::fs::write(&p, body).unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(mode)).unwrap();
            explicit_location(&p)
        }

        #[test]
        fn valid_file_is_active() {
            let d = tempfile::tempdir().unwrap();
            let l = write(
                d.path(),
                "p.json",
                br#"{"servers":["a.example:443"]}"#,
                0o644,
            );
            match load_policy(&[l]) {
                PolicyState::Active(p) => assert_eq!(p.servers, ["a.example:443"]),
                other => panic!("{other:?}"),
            }
        }

        #[test]
        fn first_existing_location_wins() {
            let d = tempfile::tempdir().unwrap();
            let missing = explicit_location(&d.path().join("none.json"));
            let l = write(
                d.path(),
                "p.json",
                br#"{"forbid_default_list":true}"#,
                0o600,
            );
            let other = write(
                d.path(),
                "q.json",
                br#"{"forbid_user_override":true}"#,
                0o600,
            );
            match load_policy(&[missing, l, other]) {
                PolicyState::Active(p) => assert!(p.forbid_default_list && !p.forbid_user_override),
                other => panic!("{other:?}"),
            }
        }

        #[test]
        fn writable_by_others_fails_closed() {
            let d = tempfile::tempdir().unwrap();
            let l = write(d.path(), "p.json", b"{}", 0o666);
            assert!(matches!(load_policy(&[l]), PolicyState::Invalid(_)));
            let l = write(d.path(), "g.json", b"{}", 0o664);
            assert!(matches!(load_policy(&[l]), PolicyState::Invalid(_)));
        }

        #[test]
        fn fifo_does_not_hang_and_fails_closed() {
            let d = tempfile::tempdir().unwrap();
            let p = d.path().join("fifo.json");
            let st = std::process::Command::new("mkfifo").arg(&p).status();
            let Ok(st) = st else {
                eprintln!("mkfifo not available, test skipped");
                return;
            };
            assert!(st.success());
            // Also through a symlink: the open must not block either.
            let link = d.path().join("link.json");
            std::os::unix::fs::symlink(&p, &link).unwrap();
            for path in [p, link] {
                let (tx, rx) = std::sync::mpsc::channel();
                let loc = explicit_location(&path);
                std::thread::spawn(move || {
                    let _ = tx.send(load_policy(&[loc]));
                });
                let got = rx
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .expect("load_policy blocked on a FIFO");
                assert!(matches!(got, PolicyState::Invalid(_)), "{got:?}");
            }
        }

        #[test]
        fn writable_directory_fails_closed() {
            let d = tempfile::tempdir().unwrap();
            let l = write(d.path(), "p.json", b"{}", 0o644);
            assert!(matches!(
                load_policy(std::slice::from_ref(&l)),
                PolicyState::Active(_)
            ));
            for mode in [0o777, 0o770, 0o757] {
                std::fs::set_permissions(d.path(), std::fs::Permissions::from_mode(mode)).unwrap();
                match load_policy(std::slice::from_ref(&l)) {
                    PolicyState::Invalid(why) => assert!(why.contains("directory"), "{why}"),
                    other => panic!("{mode:o}: {other:?}"),
                }
            }
            std::fs::set_permissions(d.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
            assert!(matches!(load_policy(&[l]), PolicyState::Active(_)));
        }

        #[test]
        fn directory_owner_is_demanded_with_the_file_owner() {
            let d = tempfile::tempdir().unwrap();
            let mut l = write(d.path(), "p.json", b"{}", 0o644);
            l.require_root_owner = true;
            let is_root = {
                use std::os::unix::fs::MetadataExt;
                std::fs::metadata(d.path()).unwrap().uid() == 0
            };
            match (load_policy(&[l]), is_root) {
                (PolicyState::Active(_), true) | (PolicyState::Invalid(_), false) => {}
                other => panic!("{other:?}"),
            }
        }

        #[test]
        fn broken_and_oversized_files_fail_closed() {
            let d = tempfile::tempdir().unwrap();
            let l = write(d.path(), "p.json", b"{oops", 0o644);
            assert!(matches!(load_policy(&[l]), PolicyState::Invalid(_)));
            let big = vec![b' '; 70_000];
            let l = write(d.path(), "b.json", &big, 0o644);
            assert!(matches!(load_policy(&[l]), PolicyState::Invalid(_)));
        }

        #[test]
        fn directory_is_not_a_policy() {
            let d = tempfile::tempdir().unwrap();
            let sub = d.path().join("p.json");
            std::fs::create_dir(&sub).unwrap();
            assert!(matches!(
                load_policy(&[explicit_location(&sub)]),
                PolicyState::Invalid(_)
            ));
        }

        #[test]
        fn root_owner_is_demanded_when_asked() {
            let d = tempfile::tempdir().unwrap();
            let mut l = write(d.path(), "p.json", b"{}", 0o644);
            l.require_root_owner = true;
            // Passes only when the test runs as root, otherwise fails closed. Both are right.
            let is_root = std::fs::metadata(&l.path).map(|m| {
                use std::os::unix::fs::MetadataExt;
                m.uid() == 0
            });
            match (load_policy(&[l]), is_root) {
                (PolicyState::Active(_), Ok(true)) | (PolicyState::Invalid(_), Ok(false)) => {}
                other => panic!("{other:?}"),
            }
        }
    }
}
