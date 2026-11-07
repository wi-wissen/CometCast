// SPDX-License-Identifier: GPL-3.0-or-later
//! Finds, downloads and verifies Cisco's prebuilt OpenH264 library at run time.
//!
//! Releases of Comet Cast load OpenH264 as a shared library (feature `h264-libloading` of
//! `cc-media`) instead of building it. Cisco's patent licence covers the binary that Cisco
//! builds and distributes, so the program must use exactly that binary. It is not part of the
//! release files. Like Firefox, the program downloads it from Cisco once, after the user
//! agreed, and checks it against a SHA-256 value that is compiled in ([`table`]).
//!
//! Where the library is looked for, in this order ([`find_library`]):
//!
//! 1. the environment variable [`LIBRARY_ENV`] (`COMETCAST_OPENH264_LIB`): path of a library,
//!    for developers; not checked here (the `openh264` crate checks the hash when it loads);
//! 2. the directory in [`DIR_ENV`] (`COMETCAST_CODEC_DIR`): for administrators of networks
//!    without Internet access. The directory holds the file under its exact name from the
//!    table (see [`CodecInfo::file_name`]), either decompressed or as Cisco's `.bz2` file;
//! 3. the per-user directory ([`user_dir`]), where the download is stored.
//!
//! Files from 2 and 3 are only used if their SHA-256 matches the table.
//!
//! Nothing here downloads without being asked: [`ensure_codec`] (feature `download`) is called
//! by the app after the user consented, or when the administrator set [`AUTO_ENV`]
//! (`COMETCAST_CODEC_AUTO=1`).

use std::path::PathBuf;
use std::sync::Mutex;

mod store;
pub mod table;

#[cfg(feature = "download")]
mod install;

#[cfg(feature = "download")]
pub use install::ensure_codec;
pub use store::{Format, check_hash, install_atomic, sha256_hex, sniff, verify_file};
pub use table::{Artifact, CISCO_HOST, OPENH264_VERSION, artifact_for, current_artifact};

/// Environment variable with the path of a library (developers).
pub const LIBRARY_ENV: &str = "COMETCAST_OPENH264_LIB";
/// Environment variable with a directory that holds the library (administrators).
pub const DIR_ENV: &str = "COMETCAST_CODEC_DIR";
/// Environment variable: `1` lets the program download without asking the user.
pub const AUTO_ENV: &str = "COMETCAST_CODEC_AUTO";

/// Largest compressed download accepted. Cisco's files are 0.4 to 0.7 MB (2026-11).
pub const MAX_COMPRESSED_BYTES: u64 = 4 * 1024 * 1024;
/// Largest decompressed library accepted. Cisco's are 0.8 to 1.6 MB (2026-11).
pub const MAX_LIBRARY_BYTES: u64 = 8 * 1024 * 1024;

/// Errors of the codec manager.
#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    /// No OpenH264 file is offered for this platform.
    #[error("no OpenH264 library is offered for this platform ({0})")]
    Unsupported(String),
    /// There is no directory to store the library in (call [`set_data_dir`] on Android).
    #[error("no directory to store the codec in")]
    NoDataDir,
    /// Network or TLS error (may go away on a retry).
    #[error("network error: {0}")]
    Network(String),
    /// The server answered with an HTTP status other than 200.
    #[error("the server answered HTTP {0}")]
    Http(u16),
    /// The file is larger than allowed (compressed or decompressed).
    #[error("the file is larger than allowed")]
    TooLarge,
    /// The file is neither bzip2 nor a native library, or the bzip2 data is broken.
    #[error("the file has an unexpected format: {0}")]
    BadFormat(String),
    /// The SHA-256 of the library is not the pinned one.
    #[error("checksum mismatch (expected {expected}, got {actual})")]
    HashMismatch {
        /// The pinned value.
        expected: String,
        /// The value of the data.
        actual: String,
    },
    /// A symlink was found where a plain file or directory is required.
    #[error("refusing to use a symbolic link: {0}")]
    SymlinkRefused(PathBuf),
    /// File system error.
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
}

impl CodecError {
    /// Worth another attempt after a pause.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Network(_) => true,
            Self::Http(c) => *c >= 500 || *c == 429 || *c == 408,
            _ => false,
        }
    }
}

/// Where a found library came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// [`LIBRARY_ENV`].
    Env,
    /// [`DIR_ENV`].
    AdminDir,
    /// The per-user directory.
    UserDir,
}

/// A library that can be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// Path of the library.
    pub path: PathBuf,
    /// Which rule found it.
    pub origin: Origin,
}

struct Runtime {
    downloading: bool,
    done: u64,
    total: u64,
    failed: Option<String>,
    data_dir: Option<PathBuf>,
}

static RUNTIME: Mutex<Runtime> = Mutex::new(Runtime {
    downloading: false,
    done: 0,
    total: 0,
    failed: None,
    data_dir: None,
});

fn runtime() -> std::sync::MutexGuard<'static, Runtime> {
    RUNTIME.lock().unwrap_or_else(|e| e.into_inner())
}

/// Set the base directory for the downloaded library (Android: the app-private directory the
/// app passes in). Without it the platform's per-user data directory is used.
pub fn set_data_dir(dir: Option<PathBuf>) {
    runtime().data_dir = dir;
}

/// `<base>/CometCast/codecs/openh264-<version>` where `<base>` is the directory from
/// [`set_data_dir`], else `$XDG_DATA_HOME` or `~/.local/share` (Linux),
/// `~/Library/Application Support` (macOS), `%LOCALAPPDATA%` (Windows).
pub fn user_dir() -> Option<PathBuf> {
    let base = runtime().data_dir.clone().or_else(dirs::data_local_dir)?;
    Some(
        base.join("CometCast")
            .join("codecs")
            .join(format!("openh264-{OPENH264_VERSION}")),
    )
}

/// The directory named by [`DIR_ENV`], if set and not empty.
pub fn admin_dir() -> Option<PathBuf> {
    env_path(DIR_ENV)
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Does the administrator allow downloads without asking ([`AUTO_ENV`] is `1`)?
pub fn auto_download_policy() -> bool {
    std::env::var(AUTO_ENV).is_ok_and(|v| v.trim() == "1")
}

/// Look for a usable library (see the crate documentation for the order).
pub fn find_library() -> Option<Found> {
    find_in(
        current_artifact()?,
        env_path(LIBRARY_ENV),
        admin_dir(),
        user_dir(),
    )
}

/// [`find_library`] with the inputs given.
pub(crate) fn find_in(
    artifact: Artifact,
    env_lib: Option<PathBuf>,
    admin: Option<PathBuf>,
    user: Option<PathBuf>,
) -> Option<Found> {
    if let Some(path) = env_lib {
        return Some(Found {
            path,
            origin: Origin::Env,
        });
    }
    // The admin directory is the administrator's own; a symlink there is accepted.
    if let Some(dir) = admin {
        let path = dir.join(artifact.name);
        if store::verify_file(&path, artifact.sha256, false, MAX_LIBRARY_BYTES).is_ok() {
            return Some(Found {
                path,
                origin: Origin::AdminDir,
            });
        }
    }
    if let Some(dir) = user {
        let path = dir.join(artifact.name);
        if store::verify_file(&path, artifact.sha256, true, MAX_LIBRARY_BYTES).is_ok() {
            return Some(Found {
                path,
                origin: Origin::UserDir,
            });
        }
    }
    None
}

/// State of the codec, for the UI.
#[derive(Debug, Clone, PartialEq)]
pub enum CodecStatus {
    /// A library is available at this path.
    Ready {
        /// Path of the library.
        path: String,
    },
    /// No library yet; [`ensure_codec`] can fetch it.
    Missing,
    /// A download runs. `progress` is 0.0 to 1.0.
    Downloading {
        /// Fraction done.
        progress: f64,
    },
    /// The last attempt failed.
    Failed {
        /// Human-readable reason (English, technical).
        reason: String,
    },
    /// This platform has no library to offer.
    Unsupported {
        /// Why.
        reason: String,
    },
}

/// Current [`CodecStatus`].
pub fn codec_status() -> CodecStatus {
    {
        let rt = runtime();
        if rt.downloading {
            let progress = if rt.total > 0 {
                (rt.done as f64 / rt.total as f64).clamp(0.0, 1.0)
            } else {
                0.0
            };
            return CodecStatus::Downloading { progress };
        }
    }
    if current_artifact().is_none() {
        return CodecStatus::Unsupported {
            reason: format!(
                "no OpenH264 library for {} / {}",
                std::env::consts::OS,
                std::env::consts::ARCH
            ),
        };
    }
    if let Some(f) = find_library() {
        return CodecStatus::Ready {
            path: f.path.to_string_lossy().into_owned(),
        };
    }
    match &runtime().failed {
        Some(reason) => CodecStatus::Failed {
            reason: reason.clone(),
        },
        None => CodecStatus::Missing,
    }
}

/// What the UI tells the user before a download.
#[derive(Debug, Clone, PartialEq)]
pub struct CodecInfo {
    /// Exact file name the library must have (also inside the administrator directory).
    pub file_name: String,
    /// OpenH264 version.
    pub version: String,
    /// Host the file is downloaded from.
    pub host: String,
    /// Approximate download size in bytes.
    pub download_bytes: u64,
    /// The administrator allows downloads without asking.
    pub auto_download: bool,
    /// The administrator directory ([`DIR_ENV`]) if set.
    pub admin_dir: Option<String>,
    /// Where a download is stored, if known.
    pub user_dir: Option<String>,
}

/// [`CodecInfo`] for this platform, `None` if the platform has no library.
pub fn codec_info() -> Option<CodecInfo> {
    let a = current_artifact()?;
    Some(CodecInfo {
        file_name: a.name.to_string(),
        version: OPENH264_VERSION.to_string(),
        host: CISCO_HOST.to_string(),
        download_bytes: a.compressed_bytes,
        auto_download: auto_download_policy(),
        admin_dir: admin_dir().map(|p| p.to_string_lossy().into_owned()),
        user_dir: user_dir().map(|p| p.to_string_lossy().into_owned()),
    })
}

#[cfg(feature = "download")]
pub(crate) fn set_progress(done: u64, total: u64) {
    let mut rt = runtime();
    rt.done = done;
    rt.total = total;
}

/// Mark a download as running. `false` if one already runs.
#[cfg(feature = "download")]
pub(crate) fn begin_download() -> bool {
    let mut rt = runtime();
    if rt.downloading {
        return false;
    }
    rt.downloading = true;
    rt.done = 0;
    rt.total = 0;
    rt.failed = None;
    true
}

#[cfg(feature = "download")]
pub(crate) fn end_download(error: Option<String>) {
    let mut rt = runtime();
    rt.downloading = false;
    rt.failed = error;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fake(bytes: &[u8]) -> Artifact {
        Artifact {
            name: "libfake.so",
            sha256: Box::leak(sha256_hex(bytes).into_boxed_str()),
            compressed_bytes: 1,
        }
    }

    #[test]
    fn env_wins_then_admin_then_user() {
        let t = tempfile::tempdir().unwrap();
        let admin = t.path().join("admin");
        let user = t.path().join("user");
        fs::create_dir_all(&admin).unwrap();
        fs::create_dir_all(&user).unwrap();
        let art = fake(b"good");
        fs::write(admin.join(art.name), b"good").unwrap();
        fs::write(user.join(art.name), b"good").unwrap();

        let f = find_in(
            art,
            Some("/x/y".into()),
            Some(admin.clone()),
            Some(user.clone()),
        );
        assert_eq!(f.unwrap().origin, Origin::Env);
        let f = find_in(art, None, Some(admin.clone()), Some(user.clone())).unwrap();
        assert_eq!(f.origin, Origin::AdminDir);
        let f = find_in(art, None, None, Some(user.clone())).unwrap();
        assert_eq!(f.origin, Origin::UserDir);
        assert!(find_in(art, None, None, None).is_none());
    }

    #[test]
    fn tampered_files_are_ignored() {
        let t = tempfile::tempdir().unwrap();
        let admin = t.path().join("admin");
        let user = t.path().join("user");
        fs::create_dir_all(&admin).unwrap();
        fs::create_dir_all(&user).unwrap();
        let art = fake(b"good");
        fs::write(admin.join(art.name), b"evil").unwrap();
        fs::write(user.join(art.name), b"evil").unwrap();
        assert!(find_in(art, None, Some(admin), Some(user)).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn symlink_in_user_dir_is_ignored_but_not_in_admin_dir() {
        use std::os::unix::fs::symlink;
        let t = tempfile::tempdir().unwrap();
        let real = t.path().join("real.so");
        fs::write(&real, b"good").unwrap();
        let art = fake(b"good");
        let admin = t.path().join("admin");
        let user = t.path().join("user");
        fs::create_dir_all(&admin).unwrap();
        fs::create_dir_all(&user).unwrap();
        symlink(&real, admin.join(art.name)).unwrap();
        symlink(&real, user.join(art.name)).unwrap();
        assert!(find_in(art, None, None, Some(user)).is_none());
        assert!(find_in(art, None, Some(admin), None).is_some());
    }

    #[test]
    fn data_dir_override_sets_user_dir() {
        // Single test that touches the global, to avoid races with other tests.
        set_data_dir(Some(PathBuf::from("/data/app")));
        let d = user_dir().unwrap();
        assert!(d.starts_with("/data/app/CometCast/codecs"));
        assert!(d.ends_with(format!("openh264-{OPENH264_VERSION}")));
        set_data_dir(None);
    }

    #[test]
    fn retryable_errors() {
        assert!(CodecError::Network("x".into()).is_retryable());
        assert!(CodecError::Http(503).is_retryable());
        assert!(!CodecError::Http(404).is_retryable());
        assert!(!CodecError::TooLarge.is_retryable());
    }
}
