// SPDX-License-Identifier: GPL-3.0-or-later
//! Memory of another process, read without `unsafe`: `/proc` on Linux and Android, `ps` on
//! macOS and other Unix, `tasklist` on Windows.

/// `VmHWM` (peak resident set) in KiB from a `/proc/<pid>/status` style file.
#[cfg_attr(not(any(target_os = "linux", target_os = "android")), allow(dead_code))]
pub(crate) fn read_vm_hwm_kib(path: &str) -> Option<u64> {
    let s = std::fs::read_to_string(path).ok()?;
    let line = s.lines().find(|l| l.starts_with("VmHWM:"))?;
    line.split_whitespace().nth(1)?.parse().ok()
}

/// Resident memory of process `pid` in KiB (the peak on Linux), or `None` if it cannot be
/// read (process gone, tool missing).
pub(crate) fn rss_kib_of(pid: u32) -> Option<u64> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        read_vm_hwm_kib(&format!("/proc/{pid}/status"))
    }
    #[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
    {
        let out = std::process::Command::new("ps")
            .args(["-o", "rss=", "-p", &pid.to_string()])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()?;
        String::from_utf8_lossy(&out.stdout).trim().parse().ok()
    }
    #[cfg(windows)]
    {
        // CSV line: "name.exe","1234","Console","1","12,345 K"; digits of the last field.
        let out = std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        let line = text.lines().find(|l| l.starts_with('"'))?;
        let last = line.rsplit("\",\"").next()?;
        let digits: String = last.chars().filter(char::is_ascii_digit).collect();
        digits.parse().ok()
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = pid;
        None
    }
}

/// Milliseconds between memory polls of the child (a poll costs a process start on non-Linux).
pub(crate) const POLL_MS: u64 = if cfg!(any(target_os = "linux", target_os = "android")) {
    10
} else if cfg!(windows) {
    250
} else {
    100
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_process_memory_is_readable_where_supported() {
        if cfg!(any(target_os = "linux", target_os = "android")) {
            assert!(rss_kib_of(std::process::id()).is_some_and(|k| k > 0));
        }
    }
}
