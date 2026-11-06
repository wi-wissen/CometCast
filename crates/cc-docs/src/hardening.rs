// SPDX-License-Identifier: GPL-3.0-or-later
//! The worker restricts itself **before it reads the PDF**: resource limits (mandatory),
//! then, where the kernel offers them, Landlock (no file system access, no TCP) and a seccomp
//! allowlist (no `fork`, `exec`, `socket`, `open` ...).
//!
//! Policy (see `docs/documents.md`):
//!
//! * The resource limits are **fail closed**: if one cannot be set the worker refuses the job
//!   (`DocError::WorkerFailed`) and never parses.
//! * Landlock and seccomp are **best effort**: on an old kernel, in a container that forbids
//!   them, or on a platform without them (macOS, Windows, Android, other CPUs) the worker
//!   still runs with the limits only. The reply tells the parent what was applied
//!   ([`Protection`]). An application that wants to refuse such a platform sets
//!   [`Limits::require_os_sandbox`](crate::Limits::require_os_sandbox).
//! * Nothing here needs `unsafe` in this crate; the `rlimit`, `landlock` and `seccompiler`
//!   crates contain the system calls.

use crate::error::DocError;
use crate::protection::Mask;
#[cfg(unix)]
use crate::protection::Protection;

/// Files the worker may have open: stdin, stdout, stderr, the `/proc` handle for its own
/// peak memory, and a little headroom. Everything else was closed by the parent (`std` opens
/// with close-on-exec).
#[cfg(unix)]
const MAX_OPEN_FILES: u64 = 16;

/// What the worker needs to know from the request.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Plan {
    pub memory_bytes: u64,
    pub cpu_secs: u64,
    pub require_os_sandbox: bool,
}

#[cfg(unix)]
mod limits {
    use rlimit::{Resource, getrlimit, setrlimit};

    /// Lower both limits to `want` (or to the existing hard limit when that is lower).
    pub(super) fn lower(res: Resource, want: u64) -> std::io::Result<()> {
        let (_, hard) = getrlimit(res)?;
        let v = want.min(hard);
        setrlimit(res, v, v)
    }

    /// CPU: the soft limit sends `SIGXCPU`, the hard limit one second later `SIGKILL`.
    pub(super) fn cpu(secs: u64) -> std::io::Result<()> {
        let (_, hard) = getrlimit(Resource::CPU)?;
        let soft = secs.max(1).min(hard);
        setrlimit(Resource::CPU, soft, soft.saturating_add(1).min(hard))
    }

    pub(super) const CORE: Resource = Resource::CORE;
    pub(super) const FSIZE: Resource = Resource::FSIZE;
    pub(super) const NOFILE: Resource = Resource::NOFILE;
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub(super) const AS: Resource = Resource::AS;
}

/// Apply every protection the platform offers. `Err` only when a mandatory limit failed (or
/// `require_os_sandbox` is set and Landlock or seccomp is missing).
pub(crate) fn apply(plan: Plan) -> Result<Mask, DocError> {
    #[allow(unused_mut)]
    let mut mask = Mask::default();
    // Opened before the file system is closed; read later with `pread`.
    open_proc_status();

    #[cfg(unix)]
    {
        let fail = DocError::WorkerFailed;
        limits::lower(limits::CORE, 0).map_err(|_| fail("core dumps could not be disabled"))?;
        mask = mask.with(Protection::NoCoreDumps);
        limits::lower(limits::FSIZE, 0)
            .map_err(|_| fail("the file size limit could not be applied"))?;
        mask = mask.with(Protection::NoFileWrites);
        limits::lower(limits::NOFILE, MAX_OPEN_FILES)
            .map_err(|_| fail("the open files limit could not be applied"))?;
        mask = mask.with(Protection::FdLimit);
        limits::cpu(plan.cpu_secs).map_err(|_| fail("the CPU time limit could not be applied"))?;
        mask = mask.with(Protection::CpuLimit);
        // `RLIMIT_AS` is not enforced on macOS; the parent's watchdog is the cap there.
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            limits::lower(limits::AS, plan.memory_bytes)
                .map_err(|_| fail("the memory limit could not be applied"))?;
            mask = mask.with(Protection::MemoryLimit);
        }
    }

    #[cfg(all(
        target_os = "linux",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    {
        mask = linux::apply(mask);
    }

    #[cfg(unix)]
    let both = mask.has(Protection::Landlock) && mask.has(Protection::Seccomp);
    #[cfg(not(unix))]
    let both = false;
    if plan.require_os_sandbox && !both {
        return Err(DocError::SandboxUnavailable(
            "Landlock and seccomp are required but not both available",
        ));
    }
    let _ = plan.memory_bytes;
    Ok(mask)
}

#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod linux {
    use std::collections::BTreeMap;

    use landlock::{
        ABI, Access, AccessFs, AccessNet, LandlockStatus, Ruleset, RulesetAttr, RulesetStatus,
        Scope,
    };
    use seccompiler::{
        BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter,
        SeccompRule, TargetArch,
    };

    use crate::protection::{Mask, Protection};

    pub(super) fn apply(mut mask: Mask) -> Mask {
        let (fs, net) = landlock_all();
        if fs {
            mask = mask.with(Protection::Landlock);
        }
        if net {
            mask = mask.with(Protection::LandlockNetwork);
        }
        if seccomp().is_ok() {
            mask = mask.with(Protection::Seccomp);
        }
        mask
    }

    /// Landlock with no rule at all: nothing in the file system may be opened, read, written,
    /// executed, created or removed; with ABI 4 or newer no TCP port may be bound or
    /// connected; with ABI 6 or newer no abstract Unix socket and no signal to a process
    /// outside. Returns (file system enforced, network enforced).
    fn landlock_all() -> (bool, bool) {
        let abi = ABI::V6;
        let built = Ruleset::default()
            .handle_access(AccessFs::from_all(abi))
            .and_then(|r| r.handle_access(AccessNet::from_all(abi)))
            .and_then(|r| r.scope(Scope::from_all(abi)))
            .and_then(Ruleset::create);
        let Ok(created) = built else {
            return (false, false);
        };
        let Ok(status) = created.restrict_self() else {
            return (false, false);
        };
        if status.ruleset == RulesetStatus::NotEnforced {
            return (false, false);
        }
        let net = matches!(
            status.landlock,
            LandlockStatus::Available { effective_abi, .. } if effective_abi >= ABI::V4
        );
        (true, net)
    }

    fn arch() -> TargetArch {
        if cfg!(target_arch = "x86_64") {
            TargetArch::x86_64
        } else {
            TargetArch::aarch64
        }
    }

    /// Compare a 64-bit argument (flags).
    fn cond(arg: u8, op: SeccompCmpOp, v: u64) -> SeccompCondition {
        // The argument number is below 6 in every use here.
        SeccompCondition::new(arg, SeccompCmpArgLen::Qword, op, v).expect("argument index below 6")
    }

    /// Compare an `int` argument (file descriptor, option): only the low 32 bits count, the
    /// upper half of the register is not defined by the calling convention.
    fn cond32(arg: u8, v: u64) -> SeccompCondition {
        SeccompCondition::new(arg, SeccompCmpArgLen::Dword, SeccompCmpOp::Eq, v)
            .expect("argument index below 6")
    }

    fn rule(c: Vec<SeccompCondition>) -> SeccompRule {
        SeccompRule::new(c).expect("at least one condition")
    }

    /// The allowlist. Everything not listed fails with `ENOSYS` (so that glibc falls back
    /// from `clone3` to `clone`, which the filter can check). Written from `strace` of the
    /// worker rendering the test PDFs; see `docs/documents.md`.
    fn seccomp() -> Result<(), ()> {
        let m = |n: i64| (n, vec![]);
        let thread_flags = (libc::CLONE_VM
            | libc::CLONE_FS
            | libc::CLONE_FILES
            | libc::CLONE_SIGHAND
            | libc::CLONE_THREAD
            | libc::CLONE_SYSVSEM) as u64;
        let namespace_flags = (libc::CLONE_NEWNS
            | libc::CLONE_NEWUTS
            | libc::CLONE_NEWIPC
            | libc::CLONE_NEWUSER
            | libc::CLONE_NEWPID
            | libc::CLONE_NEWNET
            | libc::CLONE_NEWCGROUP) as u64;
        let std_out_or_err = || vec![rule(vec![cond32(0, 1)]), rule(vec![cond32(0, 2)])];
        #[allow(unused_mut)]
        let mut rules: Vec<(i64, Vec<SeccompRule>)> = vec![
            // Memory.
            m(libc::SYS_brk),
            m(libc::SYS_mmap),
            m(libc::SYS_munmap),
            m(libc::SYS_mremap),
            m(libc::SYS_mprotect),
            m(libc::SYS_madvise),
            // Threads and locks (pthread_create, parking, thread exit).
            m(libc::SYS_futex),
            m(libc::SYS_set_robust_list),
            m(libc::SYS_rseq),
            m(libc::SYS_sched_yield),
            m(libc::SYS_sched_getaffinity),
            m(libc::SYS_gettid),
            m(libc::SYS_getpid),
            m(libc::SYS_set_tid_address),
            m(libc::SYS_exit),
            m(libc::SYS_exit_group),
            // `clone` only for a thread (same address space, files, signal handlers) and
            // never with a namespace flag. A process (`fork`, `vfork`, `clone` without
            // CLONE_THREAD) is refused.
            (
                libc::SYS_clone,
                vec![rule(vec![
                    cond(0, SeccompCmpOp::MaskedEq(thread_flags), thread_flags),
                    cond(0, SeccompCmpOp::MaskedEq(namespace_flags), 0),
                ])],
            ),
            // Signals (the stack overflow handler of the Rust runtime and its alternate stack).
            m(libc::SYS_rt_sigaction),
            m(libc::SYS_rt_sigprocmask),
            m(libc::SYS_rt_sigreturn),
            m(libc::SYS_sigaltstack),
            m(libc::SYS_restart_syscall),
            // Time and randomness (hash map seeds).
            m(libc::SYS_clock_gettime),
            m(libc::SYS_clock_nanosleep),
            m(libc::SYS_nanosleep),
            m(libc::SYS_gettimeofday),
            m(libc::SYS_getrandom),
            // Reading the request, the open `/proc/self/status` handle, closing handles.
            m(libc::SYS_read),
            m(libc::SYS_pread64),
            m(libc::SYS_lseek),
            m(libc::SYS_close),
            // Writing: only to stdout and stderr.
            (libc::SYS_write, std_out_or_err()),
            (libc::SYS_writev, std_out_or_err()),
            // Thread names only.
            (
                libc::SYS_prctl,
                vec![rule(vec![cond32(0, libc::PR_SET_NAME as u64)])],
            ),
        ];
        #[cfg(target_arch = "x86_64")]
        rules.push(m(libc::SYS_arch_prctl));
        let map: BTreeMap<i64, Vec<SeccompRule>> = rules.into_iter().collect();
        let filter = SeccompFilter::new(
            map,
            SeccompAction::Errno(libc::ENOSYS as u32),
            SeccompAction::Allow,
            arch(),
        )
        .map_err(|_| ())?;
        let program: BpfProgram = filter.try_into().map_err(|_| ())?;
        seccompiler::apply_filter_all_threads(&program).map_err(|_| ())
    }
}

/// Handle to `/proc/self/status`, opened before the file system is closed.
static PROC_STATUS: std::sync::OnceLock<Option<std::fs::File>> = std::sync::OnceLock::new();

fn open_proc_status() {
    PROC_STATUS.get_or_init(|| std::fs::File::open("/proc/self/status").ok());
}

/// Peak resident memory of this process in KiB (`VmHWM`), or 0 where `/proc` has none. Works
/// after the file system was closed because the handle was opened first.
#[cfg(unix)]
pub(crate) fn own_peak_kib() -> u64 {
    use std::os::unix::fs::FileExt;
    let Some(Some(f)) = PROC_STATUS.get() else {
        return 0;
    };
    let mut buf = [0u8; 4096];
    let Ok(n) = f.read_at(&mut buf, 0) else {
        return 0;
    };
    String::from_utf8_lossy(&buf[..n])
        .lines()
        .find(|l| l.starts_with("VmHWM:"))
        .and_then(|l| l.split_whitespace().nth(1)?.parse().ok())
        .unwrap_or(0)
}

/// No `/proc` handle on this platform.
#[cfg(not(unix))]
pub(crate) fn own_peak_kib() -> u64 {
    0
}
