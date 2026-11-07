// SPDX-License-Identifier: GPL-3.0-or-later
//! Restore tokens of the ScreenCast portal, kept in the per-user data directory.
//!
//! A restore token lets the portal start the same share again without showing its picker.
//! Whether the app may do that is a design decision that has not been made (the designer's
//! text says the system picker is where the person chooses), so the backend does **not** use
//! this store unless [`crate::PickerOptions::use_restore_token`] is true, which nothing sets
//! yet. The store is here, tested, so that decision costs no new code.
//!
//! The file is `<data dir>/CometCast/screencast/restore-token`, mode 0600 in a 0700 folder.

use std::io;
use std::path::{Path, PathBuf};

/// Longest token accepted, in bytes (portal tokens are short UUID-like strings).
pub const MAX_TOKEN_LEN: usize = 256;

/// The per-user data directory: `$XDG_DATA_HOME` (absolute) or `$HOME/.local/share`.
pub fn user_data_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("XDG_DATA_HOME") {
        let p = PathBuf::from(d);
        if p.is_absolute() {
            return Some(p);
        }
    }
    let home = PathBuf::from(std::env::var_os("HOME")?);
    home.is_absolute().then(|| home.join(".local/share"))
}

/// A token is non-empty, short, and printable ASCII without whitespace.
pub fn valid_token(token: &str) -> bool {
    !token.is_empty() && token.len() <= MAX_TOKEN_LEN && token.bytes().all(|b| b.is_ascii_graphic())
}

/// Where tokens are stored.
#[derive(Debug, Clone)]
pub struct RestoreStore {
    dir: PathBuf,
}

impl RestoreStore {
    /// A store inside `dir` (created on first save).
    pub fn in_dir(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The default store in the per-user data directory, if there is one.
    pub fn default_location() -> Option<Self> {
        Some(Self::in_dir(
            user_data_dir()?.join("CometCast").join("screencast"),
        ))
    }

    fn file(&self) -> PathBuf {
        self.dir.join("restore-token")
    }

    /// The stored token, if there is a valid one. A broken file counts as none.
    pub fn load(&self) -> Option<String> {
        let text = std::fs::read_to_string(self.file()).ok()?;
        let t = text.trim();
        valid_token(t).then(|| t.to_owned())
    }

    /// Store a token (atomic replace). Invalid tokens are refused.
    pub fn save(&self, token: &str) -> io::Result<()> {
        if !valid_token(token) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a valid restore token",
            ));
        }
        create_private_dir(&self.dir)?;
        let tmp = self.dir.join("restore-token.tmp");
        write_private(&tmp, token.as_bytes())?;
        std::fs::rename(&tmp, self.file())
    }

    /// Forget the token (also what to do when the portal refused it).
    pub fn clear(&self) {
        let _ = std::fs::remove_file(self.file());
    }
}

#[cfg(unix)]
fn create_private_dir(dir: &Path) -> io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn create_private_dir(dir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dir)
}

#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(bytes)
}

#[cfg(not(unix))]
fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    std::fs::write(path, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cc-capture-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn save_load_clear() {
        let dir = temp("restore");
        let s = RestoreStore::in_dir(dir.join("a/b"));
        assert_eq!(s.load(), None);
        s.save("tok-123_ABC").unwrap();
        assert_eq!(s.load().as_deref(), Some("tok-123_ABC"));
        s.save("other").unwrap();
        assert_eq!(s.load().as_deref(), Some("other"));
        s.clear();
        assert_eq!(s.load(), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn files_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp("perm");
        let s = RestoreStore::in_dir(dir.join("x"));
        s.save("abc").unwrap();
        let f = std::fs::metadata(dir.join("x/restore-token")).unwrap();
        assert_eq!(f.permissions().mode() & 0o777, 0o600);
        let d = std::fs::metadata(dir.join("x")).unwrap();
        assert_eq!(d.permissions().mode() & 0o777, 0o700);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn bad_tokens_are_refused_and_bad_files_ignored() {
        let dir = temp("bad");
        let s = RestoreStore::in_dir(&dir);
        assert!(s.save("").is_err());
        assert!(s.save("has space").is_err());
        assert!(s.save(&"a".repeat(MAX_TOKEN_LEN + 1)).is_err());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("restore-token"), "two words").unwrap();
        assert_eq!(s.load(), None);
        let _ = std::fs::remove_dir_all(dir);
    }
}
