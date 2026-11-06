// SPDX-License-Identifier: GPL-3.0-or-later
//! `cometcast-cli airplay-send ...`: the AirPlay sender (no FairPlay). See
//! docs/airplay-sender.md. All the work is blocking network code of `cc-airplay-sender`; it
//! runs on a blocking thread so that Ctrl-C can stop it cleanly.

use std::io::Read;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use cc_airplay_sender::discovery::{self, feature};
use cc_airplay_sender::screen::{ScreenOptions, start_cast};
use cc_airplay_sender::{
    Credentials, Device, MirroringProvider, PairingStore, Result as SendResult, SenderError,
    Session, Unsupported, hap, legacy,
};

pub(crate) const USAGE: &str = "\
airplay-send commands:
  cometcast-cli airplay-send list [--seconds N]
  cometcast-cli airplay-send pair <device> [--pin XXXX] [--mode legacy|hap]
  cometcast-cli airplay-send play <device> <url> [--seconds N]
  cometcast-cli airplay-send photo <device> <file.jpg>
  cometcast-cli airplay-send screen <device> [--display :N] [--seconds N] [--fps N] [--bitrate-kbps N]

airplay-send (AirPlay sender without FairPlay, see docs/airplay-sender.md): <device> is the name
or deviceid from 'list', or an address HOST:PORT / HOST (port 7000). 'pair' shows the PIN request
and stores the result in ~/.config/cometcast/airplay-pairings.json (mode 0600). Without --pin (or
with '--pin -') the PIN is read from standard input; --pin XXXX is visible in the process list.
'play' plays a
media link, 'photo' shows a JPEG, 'screen' shows the screen as an HLS live stream with a delay of
several seconds. Real AirPlay screen mirroring needs FairPlay and is not available.";

pub(crate) enum Sub {
    List {
        seconds: u64,
    },
    Pair {
        device: String,
        pin: Option<String>,
        mode: Option<String>,
    },
    Play {
        device: String,
        url: String,
        seconds: Option<u64>,
    },
    Photo {
        device: String,
        file: String,
    },
    Screen {
        device: String,
        display: Option<String>,
        seconds: Option<u64>,
        fps: u32,
        bitrate_kbps: u32,
    },
}

fn num<T: std::str::FromStr>(flag: &str, v: Option<String>) -> Result<T, String> {
    let v = v.ok_or(format!("{flag} needs a value"))?;
    v.parse().map_err(|_| format!("bad {flag} value {v:?}"))
}

pub(crate) fn parse(mut args: impl Iterator<Item = String>) -> Result<Sub, String> {
    let sub = args
        .next()
        .ok_or("airplay-send needs list, pair, play, photo or screen")?;
    let mut positional = Vec::new();
    let (mut seconds, mut pin, mut mode, mut display) = (None, None, None, None);
    let (mut fps, mut kbps) = (10u32, 2000u32);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--seconds" => {
                let n: u64 = num("--seconds", args.next())?;
                if n == 0 {
                    return Err("--seconds must be at least 1".into());
                }
                seconds = Some(n);
            }
            "--pin" => pin = Some(args.next().ok_or("--pin needs a value")?),
            "--mode" => mode = Some(args.next().ok_or("--mode needs a value")?),
            "--display" => display = Some(args.next().ok_or("--display needs a value")?),
            "--fps" => fps = num("--fps", args.next())?,
            "--bitrate-kbps" => kbps = num("--bitrate-kbps", args.next())?,
            s if s.starts_with("--") => {
                return Err(format!("unexpected option {s:?} for airplay-send"));
            }
            _ => positional.push(a),
        }
    }
    let mut it = positional.into_iter();
    let mut need = |what: &str| it.next().ok_or(format!("airplay-send {sub} needs {what}"));
    let parsed = match sub.as_str() {
        "list" => Sub::List {
            seconds: seconds.unwrap_or(4),
        },
        "pair" => Sub::Pair {
            device: need("<device>")?,
            pin,
            mode,
        },
        "play" => Sub::Play {
            device: need("<device>")?,
            url: need("<url>")?,
            seconds,
        },
        "photo" => Sub::Photo {
            device: need("<device>")?,
            file: need("<file>")?,
        },
        "screen" => Sub::Screen {
            device: need("<device>")?,
            display,
            seconds,
            fps,
            bitrate_kbps: kbps,
        },
        other => return Err(format!("unknown airplay-send command {other:?}")),
    };
    if it.next().is_some() {
        return Err("too many arguments".into());
    }
    Ok(parsed)
}

pub(crate) async fn run(sub: Sub) -> ExitCode {
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = Arc::clone(&stop);
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            stop2.store(true, Ordering::Relaxed);
        }
    });
    let result = tokio::task::spawn_blocking(move || run_blocking(sub, &stop)).await;
    match result {
        Ok(Ok(())) => ExitCode::SUCCESS,
        Ok(Err(e)) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("error: worker failed: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run_blocking(sub: Sub, stop: &AtomicBool) -> SendResult<()> {
    match sub {
        Sub::List { seconds } => {
            let found = discovery::discover(Duration::from_secs(seconds))?;
            if found.is_empty() {
                println!("no AirPlay receivers found in {seconds} s");
            }
            for d in &found {
                println!("{}", d.summary());
            }
            Ok(())
        }
        Sub::Pair { device, pin, mode } => pair(&device, pin, mode.as_deref()),
        Sub::Play {
            device,
            url,
            seconds,
        } => {
            let mut s = connect(&device)?;
            s.play(&url)?;
            println!("playing {url}");
            watch(&mut s, seconds, stop, || None)
        }
        Sub::Photo { device, file } => {
            // Read at most one byte more than the cap: a huge file is never loaded whole.
            let io_err = |e: std::io::Error| SenderError::Input(format!("{file}: {e}"));
            let mut data = Vec::new();
            std::fs::File::open(&file)
                .map_err(io_err)?
                .take(cc_airplay_sender::client::MAX_PHOTO_BYTES as u64 + 1)
                .read_to_end(&mut data)
                .map_err(io_err)?;
            let mut s = connect(&device)?;
            s.photo(data)?;
            println!("photo sent");
            Ok(())
        }
        Sub::Screen {
            device,
            display,
            seconds,
            fps,
            bitrate_kbps,
        } => {
            let mut s = connect(&device)?;
            if let Err(e) = Unsupported.start(s.device()) {
                println!("note: {e}");
            }
            let opts = ScreenOptions {
                display,
                fps,
                bitrate_bps: bitrate_kbps.saturating_mul(1000),
                ..ScreenOptions::default()
            };
            let mut cast = start_cast(&mut s, &opts)?;
            println!(
                "screen {}x{} as HLS: {} (delay of several seconds expected)",
                cast.streamer.size.0, cast.streamer.size.1, cast.url
            );
            let stats = Arc::clone(&cast.streamer.stats);
            let r = watch(&mut s, seconds, stop, || {
                Some(format!(
                    "frames={} segments={}",
                    stats.frames.load(Ordering::Relaxed),
                    stats.segments.load(Ordering::Relaxed)
                ))
            });
            cast.stop();
            let _ = s.stop();
            r
        }
    }
}

fn store() -> SendResult<PairingStore> {
    PairingStore::default_location()
}

/// Connect and authenticate with stored credentials if there are any.
fn connect(selector: &str) -> SendResult<Session> {
    let device = discovery::find(selector, Duration::from_secs(4))?;
    println!("device: {}", device.summary());
    let creds = store()
        .ok()
        .and_then(|s| s.get(&device.store_key()).ok().flatten());
    let mut s = Session::connect(&device)?;
    let kind = s.authenticate(creds.as_ref())?;
    println!("authentication: {kind:?}");
    Ok(s)
}

fn pair(selector: &str, pin: Option<String>, mode: Option<&str>) -> SendResult<()> {
    let device = discovery::find(selector, Duration::from_secs(4))?;
    println!("device: {}", device.summary());
    let use_hap = match mode {
        Some("hap") => true,
        Some("legacy") => false,
        Some(other) => {
            return Err(SenderError::Input(format!(
                "--mode must be legacy or hap, not {other:?}"
            )));
        }
        None => hap_preferred(&device),
    };
    let mut s = Session::connect(&device)?;
    let creds: Credentials = if use_hap {
        let pending = hap::setup_pin_start(s.conn_mut())?;
        let pin = ask_pin(pin)?;
        let (creds, ok) = hap::setup_pin_finish(s.conn_mut(), pending, &pin)?;
        if !ok {
            println!(
                "warning: the receiver's signature could not be verified (UNCERTAIN, see docs)"
            );
        }
        creds
    } else {
        legacy::pair_pin_start(s.conn_mut())?;
        let pin = ask_pin(pin)?;
        legacy::pair_finish(s.conn_mut(), &pin)?
    };
    let st = store()?;
    if st.put(&device.store_key(), creds)? {
        println!(
            "warning: this replaced the stored credentials for {} (the key comes from the \
             network announcement, which another device can fake)",
            device.store_key()
        );
    }
    println!("paired; credentials stored in {}", st.path().display());
    Ok(())
}

fn hap_preferred(d: &Device) -> bool {
    d.has_feature(feature::HK_PAIRING)
        || d.has_feature(feature::CORE_UTILS_PAIRING)
        || d.has_feature(feature::SYSTEM_PAIRING)
}

fn ask_pin(given: Option<String>) -> SendResult<String> {
    // `--pin XXXX` is visible to other users in the process list; without it (or with
    // `--pin -`) the PIN is read from standard input.
    if let Some(p) = given.filter(|p| p != "-") {
        return Ok(p);
    }
    println!("Enter the PIN shown on the receiver, then Enter:");
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .map_err(|e| SenderError::Input(e.to_string()))?;
    Ok(line.trim().to_owned())
}

/// Poll `/playback-info` once a second until the media ended, the time is over, Ctrl-C, or
/// the receiver fails. Then stop playback.
fn watch(
    s: &mut Session,
    seconds: Option<u64>,
    stop: &AtomicBool,
    extra: impl Fn() -> Option<String>,
) -> SendResult<()> {
    let start = Instant::now();
    let mut started = false;
    let mut idle_polls = 0;
    loop {
        if stop.load(Ordering::Relaxed) {
            println!("stopping (Ctrl-C)");
            break;
        }
        if seconds.is_some_and(|n| start.elapsed() >= Duration::from_secs(n)) {
            println!("stopping (time is over)");
            break;
        }
        match s.playback_info() {
            Ok(info) => {
                if let Some(e) = info.error {
                    return Err(SenderError::Protocol(format!(
                        "receiver reported an error: {e}"
                    )));
                }
                if info.duration.is_some() {
                    started = true;
                    idle_polls = 0;
                } else {
                    idle_polls += 1;
                }
                println!(
                    "position={:?} duration={:?} rate={:?} {}",
                    info.position,
                    info.duration,
                    info.rate,
                    extra().unwrap_or_default()
                );
                // Same rule as pyatv: after playback started, no duration means it ended; if it
                // never starts, give up after 15 polls.
                if (started && idle_polls >= 2) || (!started && idle_polls >= 15) {
                    println!("playback ended or never started");
                    break;
                }
            }
            Err(e) => {
                println!("playback-info failed: {e}");
                break;
            }
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    let _ = s.stop();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Result<Sub, String> {
        parse(s.split_whitespace().map(str::to_owned))
    }

    #[test]
    fn parses_subcommands() {
        assert!(matches!(
            p("list --seconds 2"),
            Ok(Sub::List { seconds: 2 })
        ));
        assert!(matches!(
            p("pair TV --pin 1234 --mode hap"),
            Ok(Sub::Pair {
                pin: Some(_),
                mode: Some(_),
                ..
            })
        ));
        assert!(matches!(p("play TV http://x/y.m3u8"), Ok(Sub::Play { .. })));
        assert!(matches!(p("photo TV a.jpg"), Ok(Sub::Photo { .. })));
        let Ok(Sub::Screen { display, fps, .. }) = p("screen TV --display :99 --fps 5") else {
            panic!("screen did not parse");
        };
        assert_eq!((display.as_deref(), fps), (Some(":99"), 5));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(p("").is_err());
        assert!(p("play TV").is_err());
        assert!(p("pair").is_err());
        assert!(p("list --seconds 0").is_err());
        assert!(p("list --bogus").is_err());
        assert!(p("play A B C").is_err());
        assert!(p("nothing").is_err());
    }
}
