// SPDX-License-Identifier: GPL-3.0-or-later
//! Comet Cast command line tool (developer tooling for now).
//!
//! `receive` creates a room and prints packet statistics of the incoming video.
//! `send <CODE>` joins a room and sends a synthetic VP8 test stream.
//!
//! Note: the output text of this tool is English only. It is a developer tool, not a user
//! interface, so it is not translated (the i18n rules in docs/i18n.md are for the apps).

use std::process::ExitCode;
use std::time::Duration;

use cc_airplay::{AirplayConfig, AirplayEvent, AirplayStatus, Supervisor, SupervisorOptions};
use cc_proto::RoomCode;
use cc_receiver::{ReceiverCore, ReceiverEvent, ReceiverOptions};
use cc_sender::{SenderCore, SenderEvent, SenderOptions};
use cc_transport::{IceServerConfig, TlsTrust};

const USAGE: &str = "\
Usage:
  cometcast-cli receive [--server HOST:PORT] [--bind ADDR] [--ca-file PEM]
  cometcast-cli send <CODE> [--server HOST:PORT] [--seconds N] [--bind ADDR] [--ca-file PEM]
  cometcast-cli airplay [--name NAME] [--uxplay-path PATH] [--video-sink SINK] [--seconds N]
  cometcast-cli --version

Options:
  --server ADDRESS    Signalling server: HOST:PORT (plain ws://), or ws://, wss://, http://,
                      https:// URL (default 127.0.0.1:8443). Servers that require proof-of-work
                      (open mode) are handled automatically; STUN/TURN servers are fetched
                      from the server when it offers them.
  --ca-file PEM       Also trust the certificate(s) in this PEM file for wss:// / https://
                      (for a self-signed test server; the system store is always used too)
  --seconds N         send: stop after N seconds (default: until Ctrl-C)
  --bind ADDR         Local UDP address for WebRTC (default 0.0.0.0:0; use 127.0.0.1:0 for same-machine tests)

  --name NAME         airplay: receiver name shown on iPhone, iPad and Mac (default: Comet Cast)
  --uxplay-path PATH  airplay: the uxplay program (default: uxplay from PATH)
  --video-sink SINK   airplay: GStreamer video sink, e.g. fakesink for a headless test
  (--seconds N also works with airplay: stop after N seconds)

Exit codes: 0 ok, 1 error, 2 bad arguments, 3 the peer left before any video was exchanged.

airplay starts UxPlay (a separate GPL program that must be installed) and prints what it
reports: server ready, the PIN of each connection, connects, errors. See docs/airplay.md.

receive prints the room code, then one 'stats' line per second. It exits when the sender
leaves or on Ctrl-C. send streams a built-in VP8 test pattern (no screen capture yet).";

const DEFAULT_SERVER: &str = "127.0.0.1:8443";

enum Command {
    Receive(Common),
    Send {
        code: RoomCode,
        seconds: Option<u64>,
        common: Common,
    },
    Airplay(AirplayArgs),
    Version,
    Help,
}

struct AirplayArgs {
    name: Option<String>,
    uxplay_path: Option<String>,
    video_sink: Option<String>,
    seconds: Option<u64>,
}

fn parse_airplay(mut args: impl Iterator<Item = String>) -> Result<Command, String> {
    let mut out = AirplayArgs {
        name: None,
        uxplay_path: None,
        video_sink: None,
        seconds: None,
    };
    while let Some(a) = args.next() {
        match a.as_str() {
            "--name" => out.name = Some(args.next().ok_or("--name needs a value")?),
            "--uxplay-path" => {
                out.uxplay_path = Some(args.next().ok_or("--uxplay-path needs a value")?);
            }
            "--video-sink" => {
                out.video_sink = Some(args.next().ok_or("--video-sink needs a value")?);
            }
            "--seconds" => {
                let v = args.next().ok_or("--seconds needs a value")?;
                match v.parse::<u64>() {
                    Ok(n) if n > 0 => out.seconds = Some(n),
                    _ => return Err(format!("bad --seconds value {v:?}")),
                }
            }
            "--help" | "-h" => return Ok(Command::Help),
            other => return Err(format!("unexpected argument {other:?} for airplay")),
        }
    }
    Ok(Command::Airplay(out))
}

struct Common {
    server: String,
    bind: Option<String>,
    ca_file: Option<String>,
}

/// Read the optional extra trusted certificates.
fn load_trust(ca_file: Option<&str>) -> Result<TlsTrust, String> {
    let mut trust = TlsTrust::system();
    if let Some(path) = ca_file {
        let pem = std::fs::read(path).map_err(|e| format!("cannot read --ca-file {path}: {e}"))?;
        trust
            .add_pem(&pem)
            .map_err(|e| format!("--ca-file {path}: {e}"))?;
    }
    Ok(trust)
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let mut args = args.into_iter();
    let Some(cmd) = args.next() else {
        return Ok(Command::Help);
    };
    let mut common = Common {
        server: DEFAULT_SERVER.to_string(),
        bind: None,
        ca_file: None,
    };
    let mut seconds = None;
    let mut positional = Vec::new();
    match cmd.as_str() {
        "--version" | "-V" => return Ok(Command::Version),
        "--help" | "-h" | "help" => return Ok(Command::Help),
        "airplay" => return parse_airplay(args),
        "receive" | "send" => {}
        other => return Err(format!("unknown command {other:?}")),
    }
    while let Some(a) = args.next() {
        match a.as_str() {
            "--server" => common.server = args.next().ok_or("--server needs a value")?,
            "--bind" => common.bind = Some(args.next().ok_or("--bind needs a value")?),
            "--ca-file" => common.ca_file = Some(args.next().ok_or("--ca-file needs a value")?),
            "--seconds" => {
                let v = args.next().ok_or("--seconds needs a value")?;
                let n = v
                    .parse::<u64>()
                    .map_err(|_| format!("bad --seconds value {v:?}"))?;
                if n == 0 {
                    return Err("--seconds must be at least 1".into());
                }
                seconds = Some(n);
            }
            "--help" | "-h" => return Ok(Command::Help),
            s if s.starts_with("--") => return Err(format!("unknown option {s:?}")),
            _ => positional.push(a),
        }
    }
    if cmd == "receive" {
        if !positional.is_empty() || seconds.is_some() {
            return Err("receive takes only --server, --bind and --ca-file".into());
        }
        return Ok(Command::Receive(common));
    }
    let [code] = positional.as_slice() else {
        return Err("send needs exactly one room code".into());
    };
    let code = RoomCode::parse(code).map_err(|e| format!("bad room code: {e}"))?;
    Ok(Command::Send {
        code,
        seconds,
        common,
    })
}

#[tokio::main]
async fn main() -> ExitCode {
    let cmd = match parse_args(std::env::args().skip(1)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match cmd {
        Command::Version => {
            println!("cometcast-cli {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Command::Help => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Command::Airplay(a) => airplay(a).await,
        Command::Receive(common) => receive(common).await,
        Command::Send {
            code,
            seconds,
            common,
        } => send(code, seconds, common).await,
    }
}

/// Run UxPlay under the supervisor and print its events until Ctrl-C (or --seconds).
async fn airplay(args: AirplayArgs) -> ExitCode {
    let mut config = AirplayConfig::default();
    if let Some(n) = args.name {
        config.name = n;
    }
    if let Some(p) = args.uxplay_path {
        config.uxplay_path = p.into();
    }
    config.video_sink = args.video_sink;
    // Ask the installed program which of our flags it does not list in its help text.
    // The probe gets 3 seconds; a program that hangs is killed.
    if let Ok(flags) = config.build_args() {
        let path = config.uxplay_path.clone();
        let help = tokio::task::spawn_blocking(move || help_text(&path))
            .await
            .ok()
            .flatten();
        if let Some(help) = help {
            let missing = cc_airplay::unsupported_flags(&help, &flags);
            if !missing.is_empty() {
                eprintln!("warning: this uxplay does not list these options in -h: {missing:?}");
            }
        }
    }
    let (mut sup, rx) = match Supervisor::start(config, SupervisorOptions::default()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    // The supervisor uses plain threads; hand its events over to this async code.
    let (tx, mut events) = tokio::sync::mpsc::unbounded_channel();
    std::thread::spawn(move || {
        while let Ok(ev) = rx.recv() {
            if tx.send(ev).is_err() {
                break;
            }
        }
    });
    println!("Starting UxPlay (Ctrl-C to quit)...");
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);
    let limit = args
        .seconds
        .map_or(Duration::from_secs(60 * 60 * 24 * 365), Duration::from_secs);
    let deadline = tokio::time::sleep(limit);
    tokio::pin!(deadline);
    let mut status = AirplayStatus::Off;
    let mut failed = false;
    let mut stopping = false;
    loop {
        tokio::select! {
            () = &mut deadline, if !stopping => {
                stopping = true;
                sup.request_stop();
            }
            _ = &mut ctrl_c, if !stopping => {
                println!("Interrupted, stopping UxPlay.");
                stopping = true;
                sup.request_stop();
            }
            ev = events.recv() => {
                let Some(ev) = ev else { break };
                println!("event: {ev:?}");
                let next = cc_airplay::status_after(&status, &ev);
                if next != status {
                    println!("status: {next:?}");
                    status = next;
                }
                match ev {
                    AirplayEvent::Failed(_) => failed = true,
                    AirplayEvent::Stopped => break,
                    _ => {}
                }
            }
        }
    }
    // Waits until the process is gone and reaped.
    sup.stop();
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Run `PROGRAM -h` and return its output, or `None` if it fails or takes over 3 seconds.
fn help_text(path: &std::path::Path) -> Option<String> {
    use std::io::Read;
    use std::process::Stdio;
    let mut child = std::process::Command::new(path)
        .arg("-h")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let end = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < end => {
                std::thread::sleep(Duration::from_millis(20));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    // The help text is small, so it fits in the pipes while we waited.
    let mut text = String::new();
    if let Some(mut o) = child.stdout.take() {
        let _ = o.read_to_string(&mut text);
    }
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut text);
    }
    Some(text)
}

/// One line about the STUN/TURN servers the signalling server handed out (nothing if none).
fn print_ice_servers(list: &[IceServerConfig]) {
    if list.is_empty() {
        return;
    }
    let turn = list
        .iter()
        .filter(|s| s.username.is_some() && s.credential.is_some())
        .count();
    println!(
        "ICE servers from the server: {} ({} with TURN credentials)",
        list.len(),
        turn
    );
}

async fn receive(common: Common) -> ExitCode {
    let mut opts = ReceiverOptions::new(common.server);
    opts.trust = match load_trust(common.ca_file.as_deref()) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    if let Some(b) = common.bind {
        opts.session.udp_bind = vec![b];
    }
    let mut core = match ReceiverCore::start(opts).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!("Room code: {}", core.room_code());
    print_ice_servers(core.ice_servers());
    println!("Waiting for a sender (Ctrl-C to quit)...");
    let mut last_packets = 0;
    let mut got_video = false;
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);
    loop {
        tokio::select! {
            _ = &mut ctrl_c => {
                println!("Interrupted, closing.");
                core.stop().await;
                return ExitCode::SUCCESS;
            }
            ev = core.next_event() => match ev {
                Some(ReceiverEvent::PeerJoined) => println!("Sender joined."),
                Some(ReceiverEvent::State(s)) => println!("Connection: {s:?}"),
                Some(ReceiverEvent::VideoTrack { mime, .. }) => {
                    got_video = true;
                    println!("Video track: {mime}");
                }
                Some(ReceiverEvent::Stats(s)) => {
                    println!(
                        "stats packets={} bytes={} frames={} keyframes={} (+{} packets/s)",
                        s.packets, s.bytes, s.frames, s.keyframes, s.packets - last_packets
                    );
                    last_packets = s.packets;
                }
                Some(ReceiverEvent::PeerLeft) => {
                    println!("Sender left.");
                    core.stop().await;
                    return if got_video { ExitCode::SUCCESS } else { ExitCode::from(3) };
                }
                Some(ReceiverEvent::Error(e)) => {
                    eprintln!("error: {e}");
                    core.stop().await;
                    return ExitCode::FAILURE;
                }
                None => return ExitCode::FAILURE,
            }
        }
    }
}

async fn send(code: RoomCode, seconds: Option<u64>, common: Common) -> ExitCode {
    let mut opts = SenderOptions::new(common.server, code);
    opts.duration = seconds.map(Duration::from_secs);
    opts.trust = match load_trust(common.ca_file.as_deref()) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    if let Some(b) = common.bind {
        opts.session.udp_bind = vec![b];
    }
    let mut core = match SenderCore::start(opts).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!("Joined the room, offer sent.");
    print_ice_servers(core.ice_servers());
    let mut connected = false;
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);
    loop {
        tokio::select! {
            _ = &mut ctrl_c => {
                println!("Interrupted, closing.");
                core.stop().await;
                return ExitCode::SUCCESS;
            }
            ev = core.next_event() => match ev {
                Some(SenderEvent::State(s)) => {
                    connected |= s == cc_transport::ConnectionState::Connected;
                    println!("Connection: {s:?}");
                }
                Some(SenderEvent::Streaming { frames_sent }) => println!("sent frames={frames_sent}"),
                Some(SenderEvent::Finished { frames_sent }) => {
                    println!("Done, sent {frames_sent} frames.");
                    core.stop().await;
                    return ExitCode::SUCCESS;
                }
                Some(SenderEvent::PeerLeft) => {
                    println!("Receiver left.");
                    core.stop().await;
                    return if connected { ExitCode::SUCCESS } else { ExitCode::from(3) };
                }
                Some(SenderEvent::Error(e)) => {
                    eprintln!("error: {e}");
                    core.stop().await;
                    return ExitCode::FAILURE;
                }
                None => return ExitCode::FAILURE,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<Command, String> {
        parse_args(s.split_whitespace().map(String::from))
    }

    #[test]
    fn parses_receive_and_send() {
        assert!(matches!(
            parse("receive --server a:1"),
            Ok(Command::Receive(c)) if c.server == "a:1"
        ));
        let Ok(Command::Send { code, seconds, .. }) = parse("send k7m2qx --seconds 5") else {
            panic!("send did not parse");
        };
        assert_eq!(code.as_str(), "K7M2QX");
        assert_eq!(seconds, Some(5));
    }

    #[test]
    fn parses_ca_file() {
        assert!(matches!(
            parse("receive --server https://h --ca-file /x/ca.pem"),
            Ok(Command::Receive(c)) if c.ca_file.as_deref() == Some("/x/ca.pem")
        ));
        assert!(parse("receive --ca-file").is_err());
        assert!(load_trust(Some("/nonexistent/ca.pem")).is_err());
        assert!(load_trust(None).is_ok());
    }

    #[test]
    fn parses_airplay() {
        let Ok(Command::Airplay(a)) =
            parse("airplay --name Room1 --uxplay-path /x/uxplay --seconds 3")
        else {
            panic!("airplay did not parse");
        };
        assert_eq!(a.name.as_deref(), Some("Room1"));
        assert_eq!(a.uxplay_path.as_deref(), Some("/x/uxplay"));
        assert_eq!(a.seconds, Some(3));
        assert!(parse("airplay --nope").is_err());
        assert!(parse("airplay --name").is_err());
        assert!(parse("airplay --seconds 0").is_err());
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse("send").is_err());
        assert!(parse("send ABC").is_err());
        assert!(parse("send K7M2QX --seconds 0").is_err());
        assert!(parse("receive extra").is_err());
        assert!(parse("receive --nope").is_err());
        assert!(parse("dance").is_err());
    }
}
