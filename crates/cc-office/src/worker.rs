// SPDX-License-Identifier: GPL-3.0-or-later
//! The worker side: runs in the child process that the parent (`runner`) starts.
//!
//! The worker lowers its own resource limits first (they are inherited by anything it
//! starts), then runs the pure-Rust engine in-process. It prints `protection:<name>` lines for what really took effect and
//! `warning:<text>` lines for conversion warnings; the parent trusts only the output file,
//! after checking it.
//!
//! Command line (all values are produced by the parent, nothing here is a user interface):
//! `--format EXT --work DIR --timeout SECS --mem BYTES --fsize BYTES`. Input is `DIR/in/input.EXT`; output goes to `DIR/out/`.

use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use crate::format::OfficeFormat;

/// Name of the file the engines write in `DIR/out/`.
pub(crate) const OUTPUT_NAME: &str = "input.pdf";

struct Args {
    format: OfficeFormat,
    work: PathBuf,
    timeout: Duration,
    mem: u64,
    fsize: u64,
}

fn parse(args: impl Iterator<Item = OsString>) -> Result<Args, String> {
    let mut it = args;
    let (mut format, mut work) = (None, None);
    let (mut timeout, mut mem, mut fsize) = (None, None, None);
    while let Some(a) = it.next() {
        let a = a.to_string_lossy().into_owned();
        let mut val = || it.next().ok_or_else(|| format!("{a} needs a value"));
        match a.as_str() {
            "--format" => {
                format = OfficeFormat::from_extension(&val()?.to_string_lossy());
                if format.is_none() {
                    return Err("bad --format".into());
                }
            }
            "--work" => work = Some(PathBuf::from(val()?)),
            "--timeout" => timeout = val()?.to_string_lossy().parse::<u64>().ok(),
            "--mem" => mem = val()?.to_string_lossy().parse::<u64>().ok(),
            "--fsize" => fsize = val()?.to_string_lossy().parse::<u64>().ok(),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(Args {
        format: format.ok_or("--format missing")?,
        work: work.ok_or("--work missing")?,
        timeout: Duration::from_secs(timeout.ok_or("--timeout missing")?),
        mem: mem.ok_or("--mem missing")?,
        fsize: fsize.ok_or("--fsize missing")?,
    })
}

fn say(line: &str) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{}", line.replace(['\n', '\r'], " "));
    let _ = out.flush();
}

/// The value to set: what we want, but never more than the limit already in force. A parent
/// that started us with a stricter soft limit keeps it (raising it back up to the hard limit
/// would silently undo the caller's protection).
#[cfg(unix)]
fn limit_value(want: u64, soft: u64, hard: u64) -> u64 {
    want.min(soft).min(hard)
}

/// Lower one resource limit (soft and hard together, so it cannot be raised again).
#[cfg(unix)]
fn lower(res: rlimit::Resource, want: u64) -> Result<(), String> {
    let (soft, hard) = rlimit::getrlimit(res).map_err(|e| format!("getrlimit: {e}"))?;
    let v = limit_value(want, soft, hard);
    rlimit::setrlimit(res, v, v).map_err(|e| format!("setrlimit: {e}"))
}

/// Resource limits (fail closed: if one cannot be set, nothing is converted) and the
/// best-effort network block. Returns the protections that took effect.
#[cfg(unix)]
fn harden(a: &Args) -> Result<Vec<&'static str>, String> {
    use rlimit::Resource;
    let mut got = Vec::new();
    // CPU time of this process.
    lower(Resource::CPU, a.timeout.as_secs() + 2).map_err(|e| format!("CPU limit: {e}"))?;
    got.push("rlimit-cpu");
    lower(Resource::CORE, 0).map_err(|e| format!("core limit: {e}"))?;
    got.push("rlimit-core");
    // Largest file this process may write: the output cap.
    lower(Resource::FSIZE, a.fsize).map_err(|e| format!("file size limit: {e}"))?;
    got.push("rlimit-fsize");
    // Best effort, not fail closed: open files (never raised, see `lower`).
    if lower(Resource::NOFILE, 2048).is_ok() {
        got.push("rlimit-nofile");
    }
    // RLIMIT_AS is not enforced by the kernel on macOS; setting it may fail there. On Linux
    // it is mandatory. UNCERTAIN: the macOS behaviour was not run.
    match lower(Resource::AS, a.mem) {
        Ok(()) => got.push("rlimit-as"),
        Err(e) if cfg!(target_os = "linux") => return Err(format!("memory limit: {e}")),
        Err(_) => {}
    }
    if deny_tcp() {
        got.push("landlock-no-tcp");
    }
    Ok(got)
}

#[cfg(not(unix))]
fn harden(_a: &Args) -> Result<Vec<&'static str>, String> {
    // UNCERTAIN: Windows has no resource limits here; only the parent's timeout applies.
    Ok(Vec::new())
}

/// Landlock: no TCP connect or bind (ABI 4+, Linux 6.7+). Inherited by children. UDP and
/// Unix sockets are not covered.
#[cfg(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
fn deny_tcp() -> bool {
    use landlock::{ABI, Access, AccessNet, LandlockStatus, Ruleset, RulesetAttr, RulesetStatus};
    let built = Ruleset::default()
        .handle_access(AccessNet::from_all(ABI::V4))
        .and_then(Ruleset::create);
    let Ok(created) = built else {
        return false;
    };
    let Ok(status) = created.restrict_self() else {
        return false;
    };
    status.ruleset != RulesetStatus::NotEnforced
        && matches!(
            status.landlock,
            LandlockStatus::Available { effective_abi, .. } if effective_abi >= ABI::V4
        )
}

#[cfg(not(all(
    target_os = "linux",
    any(target_arch = "x86_64", target_arch = "aarch64")
)))]
fn deny_tcp() -> bool {
    false
}

/// Run as the worker. Returns the process exit code (0 = the PDF is in `DIR/out/`).
pub fn worker_main_args(args: impl Iterator<Item = OsString>) -> i32 {
    let a = match parse(args) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("cc-office-worker: {e}");
            return 64;
        }
    };
    match harden(&a) {
        Ok(p) => {
            for name in p {
                say(&format!("protection:{name}"));
            }
        }
        Err(e) => {
            eprintln!("cc-office-worker: {e}");
            return 70;
        }
    }
    let r = run_rust(&a);
    match r {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

/// Run as the worker with this process's own arguments (after the program name).
pub fn worker_main() -> i32 {
    worker_main_args(std::env::args_os().skip(1))
}

#[cfg(feature = "engine-rust")]
fn run_rust(a: &Args) -> Result<(), String> {
    use office2pdf::config::{ConvertOptions, Format};
    let fmt = match a.format {
        OfficeFormat::Docx => Format::Docx,
        OfficeFormat::Pptx => Format::Pptx,
        OfficeFormat::Xlsx => Format::Xlsx,
    };
    let input = a
        .work
        .join("in")
        .join(format!("input.{}", a.format.extension()));
    let data = std::fs::read(&input).map_err(|e| format!("cannot read the input: {e}"))?;
    let res = office2pdf::convert_bytes(&data, fmt, &ConvertOptions::default())
        .map_err(|e| format!("office2pdf: {e}"))?;
    for w in res.warnings.iter().take(50) {
        say(&format!("warning:{w}"));
    }
    std::fs::write(a.work.join("out").join(OUTPUT_NAME), &res.pdf)
        .map_err(|e| format!("cannot write the result: {e}"))
}

#[cfg(not(feature = "engine-rust"))]
fn run_rust(_a: &Args) -> Result<(), String> {
    Err("the pure-Rust engine is not compiled into this build".into())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn limit_value_never_raises() {
        // Wanting more than the inherited soft limit keeps the soft limit.
        assert_eq!(limit_value(1000, 10, 500), 10);
        assert_eq!(limit_value(5, 10, 500), 5);
        // Hard limit below the soft one cannot happen on Linux, but still bounds the value.
        assert_eq!(limit_value(1000, 600, 500), 500);
        // Unlimited inherited (u64::MAX) is lowered to the wish.
        assert_eq!(limit_value(7, u64::MAX, u64::MAX), 7);
    }

    #[test]
    fn lower_keeps_a_stricter_inherited_soft_limit() {
        // RLIMIT_CORE is only touched by this test in this process (the workers run as
        // separate processes), and 0 is what the worker wants for it anyway.
        use rlimit::Resource;
        lower(Resource::CORE, 0).expect("lower to 0");
        // Asking for more later must not raise it again.
        lower(Resource::CORE, 1_000_000).expect("lower with a bigger wish");
        let (soft, hard) = rlimit::getrlimit(Resource::CORE).unwrap();
        assert_eq!((soft, hard), (0, 0));
    }
}
