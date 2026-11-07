// SPDX-License-Identifier: GPL-3.0-or-later
//! The worker side: runs in the child process that the parent (`runner`) starts.
//!
//! The worker lowers its own resource limits first (they are inherited by anything it
//! starts), then either runs the pure-Rust engine in-process or **supervises LibreOffice** as
//! its own child. It prints `protection:<name>` lines for what really took effect and
//! `warning:<text>` lines for conversion warnings; the parent trusts only the output file,
//! after checking it.
//!
//! Command line (all values are produced by the parent, nothing here is a user interface):
//! `--engine rust|soffice --format EXT --work DIR --timeout SECS --mem BYTES --fsize BYTES
//! [--soffice PATH]`. Input is `DIR/in/input.EXT`; output goes to `DIR/out/`.

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::format::OfficeFormat;

/// Name of the file the engines write in `DIR/out/`.
pub(crate) const OUTPUT_NAME: &str = "input.pdf";

struct Args {
    engine: String,
    format: OfficeFormat,
    work: PathBuf,
    timeout: Duration,
    mem: u64,
    fsize: u64,
    soffice: Option<PathBuf>,
}

fn parse(args: impl Iterator<Item = OsString>) -> Result<Args, String> {
    let mut it = args;
    let (mut engine, mut format, mut work) = (None, None, None);
    let (mut timeout, mut mem, mut fsize, mut soffice) = (None, None, None, None);
    while let Some(a) = it.next() {
        let a = a.to_string_lossy().into_owned();
        let mut val = || it.next().ok_or_else(|| format!("{a} needs a value"));
        match a.as_str() {
            "--engine" => engine = Some(val()?.to_string_lossy().into_owned()),
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
            "--soffice" => soffice = Some(PathBuf::from(val()?)),
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(Args {
        engine: engine.ok_or("--engine missing")?,
        format: format.ok_or("--format missing")?,
        work: work.ok_or("--work missing")?,
        timeout: Duration::from_secs(timeout.ok_or("--timeout missing")?),
        mem: mem.ok_or("--mem missing")?,
        fsize: fsize.ok_or("--fsize missing")?,
        soffice,
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
    // CPU time of this process and, by inheritance, of LibreOffice and its helpers.
    lower(Resource::CPU, a.timeout.as_secs() + 2).map_err(|e| format!("CPU limit: {e}"))?;
    got.push("rlimit-cpu");
    lower(Resource::CORE, 0).map_err(|e| format!("core limit: {e}"))?;
    got.push("rlimit-core");
    // Largest file any of them may write: the output cap (also caps LibreOffice's profile).
    // Like AS and CPU this is **per process**, not a total for the process tree.
    lower(Resource::FSIZE, a.fsize).map_err(|e| format!("file size limit: {e}"))?;
    got.push("rlimit-fsize");
    // Best effort, not fail closed: open files (never raised, see `lower`) and, for the
    // LibreOffice supervisor only, processes. RLIMIT_NPROC counts every process and thread of
    // the *user*, not of this tree, so a low value would also stop the pure-Rust engine's
    // threads and a busy desktop session; 8192 is a fork-bomb brake, not a precise cap.
    // UNCERTAIN: neither value was tuned against real LibreOffice runs beyond the tiny
    // fixtures; the root user and CAP_SYS_RESOURCE are exempt from NPROC.
    if lower(Resource::NOFILE, 2048).is_ok() {
        got.push("rlimit-nofile");
    }
    if a.engine == "soffice" && lower(Resource::NPROC, 8192).is_ok() {
        got.push("rlimit-nproc");
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
    let r = match a.engine.as_str() {
        "rust" => run_rust(&a),
        "soffice" => run_soffice(&a),
        other => Err(format!("unknown engine {other}")),
    };
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
        _ => return Err("the pure-Rust engine cannot convert this format".into()),
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

/// A minimal LibreOffice profile: macros off at the highest security level, no updating of
/// external links in Calc, Writer, Impress and Draw. UNCERTAIN: the option paths and values
/// are from LibreOffice's configuration schema as remembered, and were not each tested for
/// effect; the Impress and Draw `Content/Update/Link` keys in particular may not exist (an
/// unknown key is ignored, which would mean *no* protection there); headless conversion
/// does not run document macros by default either. This profile does **not** stop
/// LibreOffice from reading local files that a document links or includes (images, text
/// sections, OLE links) and printing their content into the PDF; see docs/documents.md.
const PROFILE_XCU: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<oor:items xmlns:oor="http://openoffice.org/2001/registry" xmlns:xs="http://www.w3.org/2001/XMLSchema" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
<item oor:path="/org.openoffice.Office.Common/Security/Scripting"><prop oor:name="MacroSecurityLevel" oor:op="fuse"><value>3</value></prop></item>
<item oor:path="/org.openoffice.Office.Common/Security/Scripting"><prop oor:name="DisableMacrosExecution" oor:op="fuse"><value>true</value></prop></item>
<item oor:path="/org.openoffice.Office.Calc/Content/Update"><prop oor:name="Link" oor:op="fuse"><value>2</value></prop></item>
<item oor:path="/org.openoffice.Office.Writer/Content/Update"><prop oor:name="Link" oor:op="fuse"><value>2</value></prop></item>
<item oor:path="/org.openoffice.Office.Impress/Content/Update"><prop oor:name="Link" oor:op="fuse"><value>2</value></prop></item>
<item oor:path="/org.openoffice.Office.Draw/Content/Update"><prop oor:name="Link" oor:op="fuse"><value>2</value></prop></item>
</oor:items>
"#;

fn file_url(p: &Path) -> String {
    // The path is ours (`runner::WorkDir`), but the temp directory above it can contain
    // spaces; percent-encode everything but the unreserved characters and `/`.
    // UNCERTAIN: Windows drive paths (`C:\...`) would need `file:///C:/...`; not run.
    let mut s = String::from("file://");
    for b in p.to_string_lossy().bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.' | b'~') {
            s.push(char::from(b));
        } else {
            s.push_str(&format!("%{b:02X}"));
        }
    }
    s
}

fn run_soffice(a: &Args) -> Result<(), String> {
    use std::process::{Command, Stdio};
    let soffice = a
        .soffice
        .as_ref()
        .ok_or("--soffice missing for the soffice engine")?;
    let home = a.work.join("home");
    let tmp = a.work.join("tmp");
    let profile = a.work.join("profile");
    for d in [&home, &tmp, &profile.join("user")] {
        std::fs::create_dir_all(d).map_err(|e| format!("cannot create {}: {e}", d.display()))?;
    }
    std::fs::write(
        profile.join("user").join("registrymodifications.xcu"),
        PROFILE_XCU,
    )
    .map_err(|e| format!("cannot write the profile: {e}"))?;
    let input = a
        .work
        .join("in")
        .join(format!("input.{}", a.format.extension()));
    let outdir = a.work.join("out");

    let mut c = Command::new(soffice);
    c.env_clear()
        .env("PATH", "/usr/local/bin:/usr/bin:/bin")
        .env("HOME", &home)
        .env("TMPDIR", &tmp)
        .env("LANG", "C.UTF-8")
        .env("SAL_USE_VCLPLUGIN", "svp")
        .current_dir(&a.work)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .arg("--headless")
        .arg("--invisible")
        .arg("--norestore")
        .arg("--nologo")
        .arg("--nodefault")
        .arg("--nolockcheck")
        .arg("--nofirststartwizard")
        .arg(format!("-env:UserInstallation={}", file_url(&profile)))
        .arg("--convert-to")
        .arg("pdf")
        .arg("--outdir")
        .arg(&outdir)
        .arg(&input);
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("SystemRoot") {
        c.env("SystemRoot", root);
    }
    let mut child = c
        .spawn()
        .map_err(|e| format!("cannot start LibreOffice: {e}"))?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(st)) => {
                return if st.success() {
                    Ok(())
                } else {
                    Err(format!("LibreOffice exited with {st}"))
                };
            }
            Ok(None) => {}
            Err(e) => return Err(format!("waiting for LibreOffice: {e}")),
        }
        if start.elapsed() > a.timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err("LibreOffice did not finish in time".into());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
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
