// SPDX-License-Identifier: GPL-3.0-or-later
//! Files on disk: hash check, file type sniffing, atomic installation.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

use crate::CodecError;

/// Lower-case hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(64);
    for b in Sha256::digest(bytes) {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// What a file looks like from its first bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// bzip2 stream (`BZh1`..`BZh9`), as Cisco serves its files.
    Bzip2,
    /// A native library: ELF, PE (`MZ`) or Mach-O.
    Native,
    /// Anything else.
    Unknown,
}

/// Look at the magic bytes. Used so that both Cisco's `.bz2` files and an already
/// decompressed library (copied by an administrator) are accepted.
pub fn sniff(bytes: &[u8]) -> Format {
    if bytes.len() >= 4 && &bytes[..3] == b"BZh" && (b'1'..=b'9').contains(&bytes[3]) {
        return Format::Bzip2;
    }
    let native = bytes.starts_with(b"\x7fELF")
        || bytes.starts_with(b"MZ")
        || bytes.starts_with(&[0xfe, 0xed, 0xfa, 0xce])
        || bytes.starts_with(&[0xfe, 0xed, 0xfa, 0xcf])
        || bytes.starts_with(&[0xce, 0xfa, 0xed, 0xfe])
        || bytes.starts_with(&[0xcf, 0xfa, 0xed, 0xfe])
        || bytes.starts_with(&[0xca, 0xfe, 0xba, 0xbe]);
    if native {
        Format::Native
    } else {
        Format::Unknown
    }
}

/// Read `path` (a regular file, no symlink when `refuse_symlink`) and check that its SHA-256
/// is `expected`. Files larger than `max` bytes are rejected without being read.
pub fn verify_file(
    path: &Path,
    expected: &str,
    refuse_symlink: bool,
    max: u64,
) -> Result<(), CodecError> {
    let meta = fs::symlink_metadata(path)?;
    if refuse_symlink && meta.file_type().is_symlink() {
        return Err(CodecError::SymlinkRefused(path.to_path_buf()));
    }
    let meta = fs::metadata(path)?;
    if !meta.is_file() {
        return Err(CodecError::Io(io::Error::other("not a regular file")));
    }
    if meta.len() > max {
        return Err(CodecError::TooLarge);
    }
    let bytes = fs::read(path)?;
    check_hash(&bytes, expected)
}

/// `Ok` if the SHA-256 of `bytes` is `expected`.
pub fn check_hash(bytes: &[u8], expected: &str) -> Result<(), CodecError> {
    let actual = sha256_hex(bytes);
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(CodecError::HashMismatch {
            expected: expected.to_string(),
            actual,
        })
    }
}

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Write `data` to `dir/name` so that a reader sees either no file or the whole file:
/// temporary file in the same directory (created exclusively, so a planted file or symlink of
/// that name is never followed), `fsync`, rename, `fsync` of the directory.
///
/// The directory is created with mode 0700 and the file gets 0755 on Unix (0555 on Android,
/// whose newer versions want dynamically loaded files to be read-only). Refuses a symlink as
/// the directory or as the target. `name` must be a plain file name.
pub fn install_atomic(dir: &Path, name: &str, data: &[u8]) -> Result<PathBuf, CodecError> {
    let plain =
        !name.is_empty() && name != "." && name != ".." && !name.contains(['/', '\\', '\0']);
    if !plain {
        return Err(CodecError::Io(io::Error::other("invalid file name")));
    }
    create_private_dir(dir)?;
    let dest = dir.join(name);
    if let Ok(m) = fs::symlink_metadata(&dest)
        && m.file_type().is_symlink()
    {
        return Err(CodecError::SymlinkRefused(dest));
    }
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp = dir.join(format!(".{name}.{}.{n}.tmp", std::process::id()));
    let result = write_and_rename(&tmp, &dest, dir, data);
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result?;
    Ok(dest)
}

#[cfg(all(unix, target_os = "android"))]
const FILE_MODE: u32 = 0o555;
#[cfg(all(unix, not(target_os = "android")))]
const FILE_MODE: u32 = 0o755;

fn write_and_rename(tmp: &Path, dest: &Path, dir: &Path, data: &[u8]) -> io::Result<()> {
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(FILE_MODE);
    }
    let mut f = opts.open(tmp)?;
    f.write_all(data)?;
    f.sync_all()?;
    drop(f);
    #[cfg(unix)]
    {
        // `mode` above is filtered by the umask; set the exact mode.
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(tmp, fs::Permissions::from_mode(FILE_MODE))?;
    }
    fs::rename(tmp, dest)?;
    #[cfg(unix)]
    {
        // Make the rename durable. Best effort: some file systems refuse to sync a directory.
        if let Ok(d) = fs::File::open(dir) {
            let _ = d.sync_all();
        }
    }
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

/// Create `dir` (and missing parents) with mode 0700 on Unix; refuse a symlink in its place.
fn create_private_dir(dir: &Path) -> Result<(), CodecError> {
    match fs::symlink_metadata(dir) {
        Ok(m) if m.file_type().is_symlink() => {
            return Err(CodecError::SymlinkRefused(dir.to_path_buf()));
        }
        Ok(m) if m.is_dir() => return Ok(()),
        Ok(_) => return Err(CodecError::Io(io::Error::other("not a directory"))),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    let mut b = fs::DirBuilder::new();
    b.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        b.mode(0o700);
    }
    b.create(dir)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_known_value() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn tampered_bytes_fail_the_hash() {
        let good = sha256_hex(b"library");
        assert!(check_hash(b"library", &good).is_ok());
        assert!(matches!(
            check_hash(b"librarx", &good),
            Err(CodecError::HashMismatch { .. })
        ));
    }

    #[test]
    fn sniffing() {
        assert_eq!(sniff(b"BZh91AY&SY"), Format::Bzip2);
        assert_eq!(sniff(b"BZh0"), Format::Unknown);
        assert_eq!(sniff(b"\x7fELF\x02"), Format::Native);
        assert_eq!(sniff(b"MZ\x90\x00"), Format::Native);
        assert_eq!(sniff(&[0xcf, 0xfa, 0xed, 0xfe, 0]), Format::Native);
        assert_eq!(sniff(b"<html>"), Format::Unknown);
        assert_eq!(sniff(b""), Format::Unknown);
    }

    #[test]
    fn atomic_install_writes_and_replaces() {
        let t = tempfile::tempdir().unwrap();
        let dir = t.path().join("a").join("codecs");
        let p = install_atomic(&dir, "lib.so", b"one").unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"one");
        install_atomic(&dir, "lib.so", b"two").unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"two");
        // No temporary file is left behind.
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let fm = fs::metadata(&p).unwrap().permissions().mode() & 0o777;
            let dm = fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
            assert_eq!(fm, FILE_MODE);
            assert_eq!(dm, 0o700);
        }
    }

    #[test]
    fn rejects_path_like_names() {
        let t = tempfile::tempdir().unwrap();
        for bad in ["", ".", "..", "a/b", "a\\b", "../x"] {
            assert!(install_atomic(t.path(), bad, b"x").is_err(), "{bad:?}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn refuses_symlinked_target_and_directory() {
        use std::os::unix::fs::symlink;
        let t = tempfile::tempdir().unwrap();
        let victim = t.path().join("victim");
        fs::write(&victim, b"keep").unwrap();
        let dir = t.path().join("codecs");
        fs::create_dir(&dir).unwrap();
        symlink(&victim, dir.join("lib.so")).unwrap();
        assert!(matches!(
            install_atomic(&dir, "lib.so", b"new"),
            Err(CodecError::SymlinkRefused(_))
        ));
        assert_eq!(fs::read(&victim).unwrap(), b"keep");

        let real = t.path().join("real");
        fs::create_dir(&real).unwrap();
        let link = t.path().join("linkdir");
        symlink(&real, &link).unwrap();
        assert!(matches!(
            install_atomic(&link, "lib.so", b"new"),
            Err(CodecError::SymlinkRefused(_))
        ));
        assert!(!real.join("lib.so").exists());
    }

    #[cfg(unix)]
    #[test]
    fn verify_refuses_symlink_when_asked() {
        use std::os::unix::fs::symlink;
        let t = tempfile::tempdir().unwrap();
        let real = t.path().join("real");
        fs::write(&real, b"x").unwrap();
        let link = t.path().join("link");
        symlink(&real, &link).unwrap();
        let h = sha256_hex(b"x");
        assert!(verify_file(&real, &h, true, 100).is_ok());
        assert!(matches!(
            verify_file(&link, &h, true, 100),
            Err(CodecError::SymlinkRefused(_))
        ));
        assert!(verify_file(&link, &h, false, 100).is_ok());
    }

    #[test]
    fn verify_enforces_size_cap_and_hash() {
        let t = tempfile::tempdir().unwrap();
        let f = t.path().join("f");
        fs::write(&f, vec![1u8; 100]).unwrap();
        let h = sha256_hex(&[1u8; 100]);
        assert!(matches!(
            verify_file(&f, &h, true, 99),
            Err(CodecError::TooLarge)
        ));
        assert!(verify_file(&f, &h, true, 100).is_ok());
        assert!(matches!(
            verify_file(&f, &sha256_hex(b"other"), true, 100),
            Err(CodecError::HashMismatch { .. })
        ));
    }
}
