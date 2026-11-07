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
//!    for developers; it must exist, but it is not hashed here (the `openh264` crate checks
//!    the hash when it loads);
//! 2. the per-user directory ([`user_dir`], mode 0700), where downloads and imports are stored;
//! 3. the directory in [`DIR_ENV`] (`COMETCAST_CODEC_DIR`, an absolute path): for
//!    administrators of networks without Internet access. The directory holds the file under
//!    its exact name from the table (see [`CodecInfo::file_name`]), decompressed (found by
//!    [`find_library`]) or as Cisco's `.bz2` file (imported by [`ensure_codec`] only).
//!
//! The administrator's file is never loaded in place. Its bytes are read (at most
//! [`MAX_LIBRARY_BYTES`]), checked against the pinned SHA-256, copied into the user directory,
//! read back from there and checked again; only that copy's path is returned. The administrator
//! directory and file must belong to the current user or root and must not be writable by group
//! or others (Unix; Windows is not checked, UNCERTAIN). The per-user directory is created with
//! mode 0700, an existing one must belong to the current user and is set to 0700.
//!
//! Residual risk (UNCERTAIN): between our last hash check of the copy and the moment the
//! `openh264` crate opens it, a process of the same user, or root, could still replace the
//! file; the private directory only keeps other users out. `openh264-sys2` reads and hashes the
//! file again itself when it loads it (not re-verified here), which narrows the window, but
//! hashing and loading are two separate opens of the path.
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

use std::path::Path;
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
    #[error("refusing to use a symbolic link: {}", file_name_of(.0))]
    SymlinkRefused(PathBuf),
    /// A folder or file has the wrong owner or can be changed by other users.
    #[error("unsafe folder or file: {0}")]
    Unsafe(String),
    /// A setting is wrong (for example a relative [`DIR_ENV`]).
    #[error("{0}")]
    BadConfig(String),
    /// The download failed after the administrator's file had been rejected.
    #[error("{source} (the file in the administrator folder was not used: {admin})")]
    AfterAdminError {
        /// Why the administrator's file was rejected.
        admin: String,
        /// The error of the download.
        source: Box<CodecError>,
    },
    /// File system error.
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
}

/// The last component of `p` for messages: the UI must not show folders of the user.
pub(crate) fn file_name_of(p: &Path) -> String {
    p.file_name()
        .map_or_else(|| "(folder)".into(), |n| n.to_string_lossy().into_owned())
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
    /// [`DIR_ENV`]. The path is the verified copy in the per-user directory.
    AdminDir,
    /// The per-user directory.
    UserDir,
}

/// A library that can be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// Path of the library (never inside the administrator directory).
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

impl Runtime {
    const fn new() -> Self {
        Self {
            downloading: false,
            done: 0,
            total: 0,
            failed: None,
            data_dir: None,
        }
    }
}

static RUNTIME: Mutex<Runtime> = Mutex::new(Runtime::new());

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
        // An explicit choice: it is used as it is, but only if there is a file.
        return path.is_file().then_some(Found {
            path,
            origin: Origin::Env,
        });
    }
    if let Some(dir) = &user {
        let path = dir.join(artifact.name);
        if verify_user_file(dir, &path, artifact.sha256).is_ok() {
            return Some(Found {
                path,
                origin: Origin::UserDir,
            });
        }
    }
    if let (Some(admin), Some(user)) = (&admin, &user)
        && let Ok(Some(path)) = import_raw_from_admin(&artifact, admin, user)
    {
        return Some(Found {
            path,
            origin: Origin::AdminDir,
        });
    }
    None
}

/// The file in the per-user directory: not a symlink, folder and file owned by us and not
/// writable by others (Unix), small enough, with the pinned hash.
fn verify_user_file(dir: &Path, path: &Path, sha256: &str) -> Result<(), CodecError> {
    store::check_trusted(dir)?;
    store::verify_file(path, sha256, true, MAX_LIBRARY_BYTES)?;
    store::check_trusted(path)
}

/// The administrator directory as it may be used: absolute, and trusted (see the crate docs).
pub(crate) fn check_admin_dir(dir: &Path) -> Result<(), CodecError> {
    if !dir.is_absolute() {
        return Err(CodecError::BadConfig(format!(
            "{DIR_ENV} must be an absolute path (it is a relative one)"
        )));
    }
    if !dir.is_dir() {
        // Nothing there (yet): nothing to import, no mistake to report.
        return Ok(());
    }
    store::check_trusted(dir)
}

/// Copy the administrator's decompressed library into the per-user directory (after the
/// checks of the crate docs) and return the path of the copy. `Ok(None)`: there is no such
/// file. The `.bz2` variant needs the `download` feature, see `ensure_codec`.
pub(crate) fn import_raw_from_admin(
    art: &Artifact,
    admin: &Path,
    user: &Path,
) -> Result<Option<PathBuf>, CodecError> {
    check_admin_dir(admin)?;
    let path = admin.join(art.name);
    match std::fs::symlink_metadata(&path) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    }
    store::check_trusted(&path)?;
    let bytes = store::read_bounded(&path, false, MAX_LIBRARY_BYTES)?;
    store::install_verified(user, art.name, art.sha256, &bytes).map(Some)
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
    if let Some(p) = env_path(LIBRARY_ENV)
        && !p.is_file()
    {
        return CodecStatus::Failed {
            reason: format!(
                "{LIBRARY_ENV} names a file that does not exist ({})",
                file_name_of(&p)
            ),
        };
    }
    if let Some(f) = find_library() {
        return CodecStatus::Ready {
            path: f.path.to_string_lossy().into_owned(),
        };
    }
    if let Some(reason) = runtime().failed.clone() {
        return CodecStatus::Failed { reason };
    }
    // A wrong administrator folder is a mistake the person cannot see otherwise.
    if let Some(dir) = admin_dir()
        && let Err(e) = check_admin_dir(&dir)
    {
        return CodecStatus::Failed {
            reason: e.to_string(),
        };
    }
    CodecStatus::Missing
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

/// Marks a download as running for as long as it lives. Dropping it without [`finish`]
/// (a panic) ends the download and records a failure, so that the state cannot stay at
/// "downloading" for ever and the UI cannot hang.
///
/// [`finish`]: DownloadGuard::finish
#[cfg(feature = "download")]
pub(crate) struct DownloadGuard {
    rt: &'static Mutex<Runtime>,
    finished: bool,
}

#[cfg(feature = "download")]
impl DownloadGuard {
    /// `None` if a download already runs.
    pub(crate) fn begin() -> Option<Self> {
        Self::begin_in(&RUNTIME)
    }

    fn begin_in(rt: &'static Mutex<Runtime>) -> Option<Self> {
        let mut r = rt.lock().unwrap_or_else(|e| e.into_inner());
        if r.downloading {
            return None;
        }
        r.downloading = true;
        r.done = 0;
        r.total = 0;
        r.failed = None;
        drop(r);
        Some(Self {
            rt,
            finished: false,
        })
    }

    /// End the download; `error` is the reason if it failed.
    pub(crate) fn finish(mut self, error: Option<String>) {
        self.end(error);
    }

    fn end(&mut self, error: Option<String>) {
        let mut r = self.rt.lock().unwrap_or_else(|e| e.into_inner());
        r.downloading = false;
        r.failed = error;
        self.finished = true;
    }
}

#[cfg(feature = "download")]
impl Drop for DownloadGuard {
    fn drop(&mut self) {
        if !self.finished {
            self.end(Some(
                "the download stopped unexpectedly (internal error)".into(),
            ));
        }
    }
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
    fn env_wins_then_user_then_admin_copy() {
        let t = tempfile::tempdir().unwrap();
        let admin = t.path().join("admin");
        let user = t.path().join("user");
        fs::create_dir_all(&admin).unwrap();
        let art = fake(b"good");
        fs::write(admin.join(art.name), b"good").unwrap();
        let dev = t.path().join("dev.so");
        fs::write(&dev, b"anything").unwrap();

        let f = find_in(
            art,
            Some(dev.clone()),
            Some(admin.clone()),
            Some(user.clone()),
        );
        assert_eq!(f.as_ref().unwrap().origin, Origin::Env);
        assert_eq!(f.unwrap().path, dev);
        // Only the administrator's file exists: it is copied, and the copy is what is found.
        assert!(!user.exists());
        let f = find_in(art, None, Some(admin.clone()), Some(user.clone())).unwrap();
        assert_eq!(f.origin, Origin::AdminDir);
        assert_eq!(f.path, user.join(art.name));
        assert!(!f.path.starts_with(&admin));
        assert_eq!(fs::read(&f.path).unwrap(), b"good");
        // Now the user directory answers (even if the administrator's file is gone).
        fs::remove_file(admin.join(art.name)).unwrap();
        let f = find_in(art, None, Some(admin), Some(user.clone())).unwrap();
        assert_eq!(f.origin, Origin::UserDir);
        assert!(find_in(art, None, None, None).is_none());
        // The administrator's folder cannot be used without a place to copy to.
        assert!(find_in(art, None, Some(t.path().join("admin")), None).is_none());
    }

    #[test]
    fn a_missing_env_library_is_not_ready() {
        let t = tempfile::tempdir().unwrap();
        let art = fake(b"good");
        assert!(find_in(art, Some(t.path().join("nope.so")), None, None).is_none());
        // A folder is no library either.
        assert!(find_in(art, Some(t.path().to_path_buf()), None, None).is_none());
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
        assert!(find_in(art, None, Some(admin), Some(user.clone())).is_none());
        // Nothing was copied over the bad file.
        assert_eq!(fs::read(user.join(art.name)).unwrap(), b"evil");
    }

    #[cfg(unix)]
    #[test]
    fn symlink_in_user_dir_is_ignored_admin_symlink_is_copied() {
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
        assert!(find_in(art, None, None, Some(user.clone())).is_none());
        // The administrator's symlink is read; the user directory's symlink is in the way.
        assert!(find_in(art, None, Some(admin.clone()), Some(user)).is_none());
        let user2 = t.path().join("user2");
        let f = find_in(art, None, Some(admin), Some(user2.clone())).unwrap();
        assert_eq!(f.path, user2.join(art.name));
    }

    #[cfg(unix)]
    #[test]
    fn unsafe_admin_dir_and_user_files_are_not_used() {
        use std::os::unix::fs::PermissionsExt;
        let t = tempfile::tempdir().unwrap();
        let art = fake(b"good");
        let admin = t.path().join("admin");
        let user = t.path().join("user");
        fs::create_dir_all(&admin).unwrap();
        fs::write(admin.join(art.name), b"good").unwrap();
        let mode =
            |p: &Path, m: u32| fs::set_permissions(p, fs::Permissions::from_mode(m)).unwrap();
        mode(&admin, 0o777);
        assert!(find_in(art, None, Some(admin.clone()), Some(user.clone())).is_none());
        assert!(!user.exists());
        mode(&admin, 0o755);
        mode(&admin.join(art.name), 0o664);
        assert!(find_in(art, None, Some(admin.clone()), Some(user.clone())).is_none());
        mode(&admin.join(art.name), 0o644);
        assert!(find_in(art, None, Some(admin), Some(user.clone())).is_some());
        // A library in the user directory that others can write is not trusted either.
        mode(&user.join(art.name), 0o666);
        assert!(find_in(art, None, None, Some(user.clone())).is_none());
        mode(&user.join(art.name), 0o755);
        mode(&user, 0o777);
        assert!(find_in(art, None, None, Some(user)).is_none());
    }

    #[test]
    fn relative_admin_dir_is_a_config_error() {
        let e = check_admin_dir(Path::new("codecs")).unwrap_err();
        assert!(
            matches!(&e, CodecError::BadConfig(m) if m.contains("absolute")),
            "{e}"
        );
        // A folder that does not exist is just empty.
        assert!(check_admin_dir(Path::new("/definitely/not/here")).is_ok());
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

    #[cfg(feature = "download")]
    mod guard {
        use super::super::*;
        use std::panic::{AssertUnwindSafe, catch_unwind};

        fn status_of(rt: &Mutex<Runtime>) -> (bool, Option<String>) {
            let r = rt.lock().unwrap_or_else(|e| e.into_inner());
            (r.downloading, r.failed.clone())
        }

        #[test]
        fn finish_ends_the_download_with_or_without_error() {
            static RT: Mutex<Runtime> = Mutex::new(Runtime::new());
            let g = DownloadGuard::begin_in(&RT).unwrap();
            assert!(DownloadGuard::begin_in(&RT).is_none(), "one at a time");
            g.finish(Some("boom".into()));
            assert_eq!(status_of(&RT), (false, Some("boom".into())));
            let g = DownloadGuard::begin_in(&RT).unwrap();
            assert_eq!(
                status_of(&RT),
                (true, None),
                "a new start clears the failure"
            );
            g.finish(None);
            assert_eq!(status_of(&RT), (false, None));
        }

        #[test]
        fn a_panic_does_not_leave_the_flag_set() {
            static RT: Mutex<Runtime> = Mutex::new(Runtime::new());
            let r = catch_unwind(AssertUnwindSafe(|| {
                let _g = DownloadGuard::begin_in(&RT).unwrap();
                assert!(status_of(&RT).0);
                panic!("worker crashed");
            }));
            assert!(r.is_err());
            let (downloading, failed) = status_of(&RT);
            assert!(
                !downloading,
                "the guard must reset the flag while unwinding"
            );
            assert!(failed.unwrap().contains("unexpectedly"));
            // And a new download can start.
            assert!(DownloadGuard::begin_in(&RT).is_some());
        }
    }
}
