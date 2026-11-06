// SPDX-License-Identifier: GPL-3.0-or-later
//! `cometcast-cli cast list` and `cast send`: the Chromecast / Google Cast compatible sender
//! (BETA, never tried on a real device; see docs/cast.md).

use std::net::{IpAddr, SocketAddr};
use std::process::ExitCode;
use std::time::Duration;

use cc_cast::{CastDevice, DeviceAuth, MirrorConfig, TrustAnchors};

/// A parsed `cast` command.
pub enum CastCmd {
    /// `cast list`.
    List {
        /// Listening time in seconds.
        seconds: u64,
    },
    /// `cast send`.
    Send(SendArgs),
}

/// Arguments of `cast send`.
pub struct SendArgs {
    target: String,
    display: Option<String>,
    fps: u32,
    bitrate_kbps: u32,
    seconds: Option<u64>,
    trust_anchors: Option<String>,
    insecure_skip: bool,
}

fn num(
    args: &mut impl Iterator<Item = String>,
    name: &str,
    lo: u64,
    hi: u64,
) -> Result<u64, String> {
    let v = args.next().ok_or(format!("{name} needs a value"))?;
    match v.parse::<u64>() {
        Ok(n) if (lo..=hi).contains(&n) => Ok(n),
        _ => Err(format!("bad {name} value {v:?} ({lo} to {hi})")),
    }
}

/// Parse the arguments after `cast`. `Ok(None)` means help was requested.
pub fn parse(mut args: impl Iterator<Item = String>) -> Result<Option<CastCmd>, String> {
    let sub = args.next().ok_or("cast needs a subcommand: list or send")?;
    let mut seconds = None;
    let mut display = None;
    let mut fps = 15;
    let mut bitrate_kbps = 2000;
    let mut trust_anchors = None;
    let mut insecure_skip = false;
    let mut positional = Vec::new();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--seconds" => seconds = Some(num(&mut args, "--seconds", 1, 86_400)?),
            "--fps" => fps = num(&mut args, "--fps", 1, 60)? as u32,
            "--bitrate-kbps" => {
                bitrate_kbps = num(&mut args, "--bitrate-kbps", 50, 100_000)? as u32
            }
            "--screen" => {}
            "--display" => display = Some(args.next().ok_or("--display needs a value")?),
            "--trust-anchors" => {
                trust_anchors = Some(args.next().ok_or("--trust-anchors needs a value")?)
            }
            "--insecure-skip-device-auth" => insecure_skip = true,
            "--help" | "-h" => return Ok(None),
            s if s.starts_with("--") => return Err(format!("unknown option {s:?}")),
            _ => positional.push(a),
        }
    }
    match sub.as_str() {
        "list" => {
            if !positional.is_empty()
                || display.is_some()
                || trust_anchors.is_some()
                || insecure_skip
            {
                return Err("cast list takes only --seconds".into());
            }
            Ok(Some(CastCmd::List {
                seconds: seconds.unwrap_or(4),
            }))
        }
        "send" => {
            let [target] = <[String; 1]>::try_from(positional)
                .map_err(|_| "cast send needs exactly one device name or IP address".to_string())?;
            Ok(Some(CastCmd::Send(SendArgs {
                target,
                display,
                fps,
                bitrate_kbps,
                seconds,
                trust_anchors,
                insecure_skip,
            })))
        }
        other => Err(format!("unknown cast subcommand {other:?} (list or send)")),
    }
}

/// Run a parsed command.
pub async fn run(cmd: CastCmd) -> ExitCode {
    match cmd {
        CastCmd::List { seconds } => list(seconds).await,
        CastCmd::Send(a) => send(a).await,
    }
}

async fn list(seconds: u64) -> ExitCode {
    match cc_cast::discover(Duration::from_secs(seconds)).await {
        Ok(devs) if devs.is_empty() => {
            println!("no Cast devices found in {seconds} s");
            ExitCode::SUCCESS
        }
        Ok(devs) => {
            for d in devs {
                let ips: Vec<String> = d.addresses.iter().map(ToString::to_string).collect();
                println!(
                    "{}\tmodel={}\t{}:{}\tid={}",
                    d.friendly_name,
                    d.model,
                    ips.join(","),
                    d.port,
                    d.id
                );
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("cast list: {e}");
            ExitCode::FAILURE
        }
    }
}

/// An IP address (optionally with `:port`) or a name.
fn parse_target(t: &str) -> Option<SocketAddr> {
    if let Ok(sa) = t.parse::<SocketAddr>() {
        return Some(sa);
    }
    t.parse::<IpAddr>().ok().map(|ip| SocketAddr::new(ip, 8009))
}

async fn send(a: SendArgs) -> ExitCode {
    let auth = match (&a.trust_anchors, a.insecure_skip) {
        (_, true) => {
            eprintln!(
                "WARNING: --insecure-skip-device-auth: the Cast device will NOT be verified. \
                 Anybody on the network who answers there receives your screen picture."
            );
            DeviceAuth::InsecureSkip
        }
        (Some(path), false) => match std::fs::read(path)
            .map_err(|e| e.to_string())
            .and_then(|b| TrustAnchors::from_pem(&b).map_err(|e| e.to_string()))
        {
            Ok(t) => DeviceAuth::Verify(t),
            Err(e) => {
                eprintln!("cast send: cannot read trust anchors {path:?}: {e}");
                return ExitCode::from(2);
            }
        },
        (None, false) => {
            eprintln!(
                "cast send: device authentication needs trust anchors (--trust-anchors PEM), which this \
                 program does not embed. To try a device anyway, pass --insecure-skip-device-auth. See docs/cast.md."
            );
            return ExitCode::from(2);
        }
    };

    let addr = match parse_target(&a.target) {
        Some(sa) => sa,
        None => {
            eprintln!("looking for {:?} on the network ...", a.target);
            let devs: Vec<CastDevice> = match cc_cast::discover(Duration::from_secs(4)).await {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("cast send: {e}");
                    return ExitCode::FAILURE;
                }
            };
            let Some(d) = cc_cast::find_by_name(&devs, &a.target) else {
                eprintln!(
                    "cast send: no device named {:?} found (try `cast list`)",
                    a.target
                );
                return ExitCode::FAILURE;
            };
            let Some(ip) = d.connect_ip() else {
                eprintln!("cast send: device has no address");
                return ExitCode::FAILURE;
            };
            SocketAddr::new(ip, d.port)
        }
    };

    let cfg = MirrorConfig {
        addr,
        auth,
        display: a.display,
        fps: a.fps,
        bitrate_kbps: a.bitrate_kbps,
        android_rtp_hack: false,
    };
    let stop = async {
        match a.seconds {
            Some(s) => tokio::time::sleep(Duration::from_secs(s)).await,
            None => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    };
    match cc_cast::mirror_screen(cfg, &|m| eprintln!("{m}"), stop).await {
        Ok(stats) => {
            println!(
                "done: {} frames, {} packets, {} resent, {} picture-loss requests",
                stats.frames, stats.packets, stats.resent_packets, stats.picture_loss
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("cast send: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> Result<Option<CastCmd>, String> {
        parse(s.split_whitespace().map(str::to_owned))
    }

    #[test]
    fn parses_list_and_send() {
        assert!(matches!(p("list"), Ok(Some(CastCmd::List { seconds: 4 }))));
        assert!(matches!(
            p("list --seconds 9"),
            Ok(Some(CastCmd::List { seconds: 9 }))
        ));
        let Ok(Some(CastCmd::Send(a))) =
            p("send Living --screen --display :99 --insecure-skip-device-auth --fps 30")
        else {
            panic!()
        };
        assert_eq!(a.target, "Living");
        assert_eq!(a.display.as_deref(), Some(":99"));
        assert!(a.insecure_skip);
        assert_eq!(a.fps, 30);
        assert!(p("send").is_err());
        assert!(p("send a b").is_err());
        assert!(p("list --display :1").is_err());
        assert!(p("bogus").is_err());
        assert!(matches!(p("send x --help"), Ok(None)));
    }

    #[test]
    fn target_forms() {
        assert_eq!(
            parse_target("192.168.1.5"),
            Some("192.168.1.5:8009".parse().unwrap())
        );
        assert_eq!(
            parse_target("192.168.1.5:9000"),
            Some("192.168.1.5:9000".parse().unwrap())
        );
        assert_eq!(parse_target("Living Room"), None);
    }
}
