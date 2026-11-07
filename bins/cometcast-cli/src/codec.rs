// SPDX-License-Identifier: GPL-3.0-or-later
//! `cometcast-cli codec status|install`: look for, or fetch, Cisco's OpenH264 library
//! (release builds with `h264-libloading`; docs/capture.md, "H.264 licensing").
//!
//! Needed once on a machine where the CLI is used without the app: the CLI itself only finds a
//! library that is already there (raw file in `COMETCAST_CODEC_DIR`, a previous download, or
//! `COMETCAST_OPENH264_LIB`). `install` is what the app does after the person agreed; here the
//! agreement is `--yes` (or the administrator's `COMETCAST_CODEC_AUTO=1`).
//!
//! `ensure_codec` is blocking and needs no async runtime, so it runs on a blocking thread of
//! the runtime that `main` already started (nothing here must run before the runtime).

use std::process::ExitCode;

/// Text of `codec --help`.
pub const USAGE: &str = "\
Usage:
  cometcast-cli codec status            show whether Cisco's OpenH264 library is available
  cometcast-cli codec install [--yes]   download it from Cisco (or import it from the
                                        administrator folder), after you agreed

Cisco's patent licence covers the library that Cisco builds and distributes, so this program
downloads that file from Cisco (https://ciscobinary.openh264.org) instead of shipping one, and
checks it against a SHA-256 value compiled into the program. `install` prints the host, the
file and the size and downloads only with --yes (or when COMETCAST_CODEC_AUTO=1 is set).

Where it looks (details: docs/capture.md): COMETCAST_OPENH264_LIB (a path), the per-user folder
of an earlier download, then COMETCAST_CODEC_DIR (absolute path; raw file for `send`/`receive`;
a Cisco .bz2 file is imported by `codec install`, which then needs no network).
Exit codes: 0 ok, 1 not available or failed, 2 bad arguments or no agreement given.";

/// What to do.
#[derive(Debug, PartialEq, Eq)]
pub enum CodecCmd {
    /// Report the state.
    Status,
    /// Make the codec available; `yes` is the person's agreement to the download.
    Install { yes: bool },
}

/// Parse the arguments after `codec`. `Ok(None)` means help was asked for.
pub fn parse(mut args: impl Iterator<Item = String>) -> Result<Option<CodecCmd>, String> {
    let Some(sub) = args.next() else {
        return Ok(None);
    };
    let mut yes = false;
    let cmd = match sub.as_str() {
        "--help" | "-h" | "help" => return Ok(None),
        "status" => CodecCmd::Status,
        "install" => CodecCmd::Install { yes: false },
        other => return Err(format!("unknown codec command {other:?} (status, install)")),
    };
    for a in args {
        match a.as_str() {
            "--yes" | "-y" if matches!(cmd, CodecCmd::Install { .. }) => yes = true,
            "--help" | "-h" => return Ok(None),
            other => return Err(format!("unexpected argument {other:?} for codec {sub}")),
        }
    }
    Ok(Some(match cmd {
        CodecCmd::Install { .. } => CodecCmd::Install { yes },
        other => other,
    }))
}

/// May the download start? The person said `--yes`, or the administrator allows it.
/// Only the download manager (feature `h264-libloading`) calls it; the tests cover it in every build.
#[cfg(any(feature = "h264-libloading", test))]
pub fn may_download(yes: bool, admin_allows: bool) -> bool {
    yes || admin_allows
}

/// Run the command.
#[cfg(feature = "h264-libloading")]
pub async fn run(cmd: CodecCmd) -> ExitCode {
    tokio::task::spawn_blocking(move || imp::run(cmd))
        .await
        .unwrap_or(ExitCode::FAILURE)
}

/// Run the command: this build has no download manager.
#[cfg(not(feature = "h264-libloading"))]
pub async fn run(_cmd: CodecCmd) -> ExitCode {
    eprintln!(
        "this build of cometcast-cli does not download a codec: it was built with the OpenH264 \
         source build (h264-source) or without H.264. The release variant (--no-default-features \
         --features h264-libloading) has this command."
    );
    ExitCode::from(2)
}

#[cfg(feature = "h264-libloading")]
mod imp {
    use std::process::ExitCode;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use cc_codec::CodecStatus;

    use super::{CodecCmd, may_download};

    pub fn run(cmd: CodecCmd) -> ExitCode {
        match cmd {
            CodecCmd::Status => report(&cc_codec::codec_status()),
            CodecCmd::Install { yes } => install(yes),
        }
    }

    /// Print a status; exit 0 only when the library is available.
    fn report(s: &CodecStatus) -> ExitCode {
        match s {
            CodecStatus::Ready { path } => {
                println!("ready: {path}");
                ExitCode::SUCCESS
            }
            CodecStatus::Missing => {
                println!("missing: run `cometcast-cli codec install` to get it");
                ExitCode::FAILURE
            }
            CodecStatus::Downloading { progress } => {
                println!("downloading: {} %", (progress * 100.0).round());
                ExitCode::FAILURE
            }
            CodecStatus::Failed { reason } => {
                eprintln!("failed: {reason}");
                ExitCode::FAILURE
            }
            CodecStatus::Unsupported { reason } => {
                eprintln!("unsupported: {reason}");
                ExitCode::FAILURE
            }
        }
    }

    fn install(yes: bool) -> ExitCode {
        let current = cc_codec::codec_status();
        if matches!(
            current,
            CodecStatus::Ready { .. } | CodecStatus::Unsupported { .. }
        ) {
            return report(&current);
        }
        let Some(info) = cc_codec::codec_info() else {
            return report(&current);
        };
        if !may_download(yes, info.auto_download) {
            println!(
                "Comet Cast needs Cisco's OpenH264 library ({}, {} KB) for screen sharing.\n\
                 It is downloaded from https://{} and checked against a fixed SHA-256 value.\n\
                 Run `cometcast-cli codec install --yes` to agree and download it.",
                info.file_name,
                info.download_bytes / 1024,
                info.host
            );
            return ExitCode::from(2);
        }
        let stop = Arc::new(AtomicBool::new(false));
        let printer = {
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                let mut last = u32::MAX;
                while !stop.load(Ordering::Relaxed) {
                    if let CodecStatus::Downloading { progress } = cc_codec::codec_status() {
                        let pct = (progress * 100.0).round() as u32;
                        if pct != last {
                            eprintln!("downloading: {pct} %");
                            last = pct;
                        }
                    }
                    std::thread::sleep(Duration::from_millis(250));
                }
            })
        };
        let result = cc_codec::ensure_codec();
        stop.store(true, Ordering::Relaxed);
        let _ = printer.join();
        report(&result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Result<Option<CodecCmd>, String> {
        parse(s.split_whitespace().map(String::from))
    }

    #[test]
    fn parses_the_subcommands() {
        assert_eq!(p("status").unwrap(), Some(CodecCmd::Status));
        assert_eq!(
            p("install").unwrap(),
            Some(CodecCmd::Install { yes: false })
        );
        assert_eq!(
            p("install --yes").unwrap(),
            Some(CodecCmd::Install { yes: true })
        );
        assert_eq!(
            p("install -y").unwrap(),
            Some(CodecCmd::Install { yes: true })
        );
        assert_eq!(p("").unwrap(), None);
        assert_eq!(p("--help").unwrap(), None);
        assert_eq!(p("install --help").unwrap(), None);
    }

    #[test]
    fn rejects_unknown_input() {
        assert!(p("download").is_err());
        assert!(p("status --yes").is_err());
        assert!(p("install --force").is_err());
    }

    #[test]
    fn download_needs_an_agreement() {
        assert!(!may_download(false, false));
        assert!(may_download(true, false));
        assert!(may_download(false, true));
    }
}
