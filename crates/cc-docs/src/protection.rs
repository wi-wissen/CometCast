// SPDX-License-Identifier: GPL-3.0-or-later
//! Which protections around the render worker were really in place.
//!
//! The worker is a **resource-limited child process, not a sandbox in the full sense**: some
//! protections are mandatory (the resource limits; without them the worker does not parse
//! anything) and some are applied only where the operating system offers them (Landlock and
//! seccomp on Linux). [`Document::protections`](crate::Document::protections) tells which
//! ones a document's worker actually reported, so an application can show or log it.

/// One protection around the render worker. The list can grow; match with a `_` arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Protection {
    /// Parent side: the worker runs in its own process group and the whole group is killed on
    /// timeout, cancel, memory cap or drop (Unix).
    ProcessGroupKill,
    /// Parent side: the worker starts with an empty environment and `/` as working directory.
    CleanEnvironment,
    /// Worker side: address-space limit (`RLIMIT_AS`, Linux and Android).
    MemoryLimit,
    /// Parent side: the parent polls the worker's resident memory and kills it above the cap
    /// (every platform with a way to read it).
    MemoryWatchdog,
    /// Worker side: CPU-time limit (`RLIMIT_CPU`).
    CpuLimit,
    /// Worker side: no core dumps (`RLIMIT_CORE` 0).
    NoCoreDumps,
    /// Worker side: files cannot grow (`RLIMIT_FSIZE` 0).
    NoFileWrites,
    /// Worker side: few open files (`RLIMIT_NOFILE`).
    FdLimit,
    /// Worker side: Landlock denies all file system access (Linux 5.13 or newer, when enabled).
    Landlock,
    /// Worker side: Landlock also denies TCP `bind` and `connect` (Landlock ABI 4, Linux 6.7).
    LandlockNetwork,
    /// Worker side: a seccomp allowlist denies `fork`, `exec`, `socket`, `open` and everything
    /// else the renderer does not need (Linux x86_64 and aarch64).
    Seccomp,
}

impl Protection {
    /// Every protection, in a fixed order.
    pub const ALL: [Protection; 11] = [
        Protection::ProcessGroupKill,
        Protection::CleanEnvironment,
        Protection::MemoryLimit,
        Protection::MemoryWatchdog,
        Protection::CpuLimit,
        Protection::NoCoreDumps,
        Protection::NoFileWrites,
        Protection::FdLimit,
        Protection::Landlock,
        Protection::LandlockNetwork,
        Protection::Seccomp,
    ];

    pub(crate) fn bit(self) -> u64 {
        1u64 << (self as u8)
    }

    /// A short stable name for logs.
    pub fn name(self) -> &'static str {
        match self {
            Protection::ProcessGroupKill => "process-group-kill",
            Protection::CleanEnvironment => "clean-environment",
            Protection::MemoryLimit => "memory-limit",
            Protection::MemoryWatchdog => "memory-watchdog",
            Protection::CpuLimit => "cpu-limit",
            Protection::NoCoreDumps => "no-core-dumps",
            Protection::NoFileWrites => "no-file-writes",
            Protection::FdLimit => "fd-limit",
            Protection::Landlock => "landlock",
            Protection::LandlockNetwork => "landlock-network",
            Protection::Seccomp => "seccomp",
        }
    }
}

/// Bit mask of protections (what travels in the reply head and is merged in the parent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Mask(pub u64);

impl Mask {
    pub(crate) fn with(mut self, p: Protection) -> Self {
        self.0 |= p.bit();
        self
    }

    pub(crate) fn has(self, p: Protection) -> bool {
        self.0 & p.bit() != 0
    }

    pub(crate) fn list(self) -> Vec<Protection> {
        Protection::ALL
            .into_iter()
            .filter(|p| self.has(*p))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_round_trips() {
        let m = Mask::default()
            .with(Protection::Seccomp)
            .with(Protection::CpuLimit);
        assert_eq!(m.list(), vec![Protection::CpuLimit, Protection::Seccomp]);
        assert!(!m.has(Protection::Landlock));
        // Every protection has its own bit and a name.
        let all = Protection::ALL
            .into_iter()
            .fold(Mask::default(), Mask::with);
        assert_eq!(all.list().len(), Protection::ALL.len());
        assert!(Protection::ALL.iter().all(|p| !p.name().is_empty()));
    }
}
