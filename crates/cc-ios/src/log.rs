// SPDX-License-Identifier: GPL-3.0-or-later
//! A tiny log file for the extension. Plain text, one line per event, kept below
//! [`MAX_BYTES`]: when the file grows past it, it is renamed to `cc-ios.log.1` (replacing an
//! older one) and a new file starts. No timestamps from a clock crate: seconds since the log
//! was opened are enough to read a session.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

/// Size at which the file is rotated. Two files make at most twice this.
pub const MAX_BYTES: u64 = 64 * 1024;

/// Name of the log file inside the directory passed to `cc_ios_start`.
pub const FILE_NAME: &str = "cc-ios.log";

/// An append-only, size-bounded log file. Failures to write are ignored (a log must never
/// break the broadcast).
pub struct Log {
    inner: Mutex<Inner>,
    opened: Instant,
}

struct Inner {
    dir: PathBuf,
    file: Option<File>,
    written: u64,
}

impl Log {
    /// Open (append to) `dir/cc-ios.log`. A missing or unwritable directory gives a log that
    /// discards everything.
    pub fn open(dir: Option<PathBuf>) -> Self {
        let dir = dir.unwrap_or_default();
        let (file, written) = Self::open_file(&dir);
        Self {
            inner: Mutex::new(Inner { dir, file, written }),
            opened: Instant::now(),
        }
    }

    fn open_file(dir: &std::path::Path) -> (Option<File>, u64) {
        if dir.as_os_str().is_empty() {
            return (None, 0);
        }
        let path = dir.join(FILE_NAME);
        let written = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let file = OpenOptions::new().create(true).append(true).open(path).ok();
        (file, written)
    }

    /// Write one line.
    pub fn line(&self, text: &str) {
        let mut g = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        let line = format!("[{:>7.1}s] {}\n", self.opened.elapsed().as_secs_f64(), text);
        if g.file.is_none() {
            return;
        }
        if g.written + line.len() as u64 > MAX_BYTES {
            let from = g.dir.join(FILE_NAME);
            let to = g.dir.join(format!("{FILE_NAME}.1"));
            g.file = None;
            let _ = fs::rename(&from, &to);
            let (file, written) = Self::open_file(&g.dir);
            g.file = file;
            g.written = written;
        }
        if let Some(f) = g.file.as_mut()
            && f.write_all(line.as_bytes()).is_ok()
        {
            g.written += line.len() as u64;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_lines_and_rotates() {
        let dir = tempfile::tempdir().unwrap();
        let log = Log::open(Some(dir.path().to_owned()));
        log.line("hello");
        let text = fs::read_to_string(dir.path().join(FILE_NAME)).unwrap();
        assert!(text.contains("hello"));
        let big = "x".repeat(1000);
        for _ in 0..200 {
            log.line(&big);
        }
        let main = fs::metadata(dir.path().join(FILE_NAME)).unwrap().len();
        assert!(main <= MAX_BYTES, "{main}");
        assert!(dir.path().join("cc-ios.log.1").exists());
    }

    #[test]
    fn no_directory_discards() {
        let log = Log::open(None);
        log.line("ignored");
        let log = Log::open(Some(PathBuf::from("/nonexistent/dir/for/test")));
        log.line("ignored");
    }
}
