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

use cc_proto::RoomCode;
use cc_receiver::{ReceiverCore, ReceiverEvent, ReceiverOptions};
use cc_sender::{SenderCore, SenderEvent, SenderOptions};

const USAGE: &str = "\
Usage:
  cometcast-cli receive [--server HOST:PORT] [--bind ADDR]
  cometcast-cli send <CODE> [--server HOST:PORT] [--seconds N] [--bind ADDR]
  cometcast-cli --version

Options:
  --server HOST:PORT  Signalling server (default 127.0.0.1:8443, plain ws://, no TLS yet)
  --seconds N         send: stop after N seconds (default: until Ctrl-C)
  --bind ADDR         Local UDP address for WebRTC (default 0.0.0.0:0; use 127.0.0.1:0 for same-machine tests)

Exit codes: 0 ok, 1 error, 2 bad arguments, 3 the peer left before any video was exchanged.

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
    Version,
    Help,
}

struct Common {
    server: String,
    bind: Option<String>,
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let mut args = args.into_iter();
    let Some(cmd) = args.next() else {
        return Ok(Command::Help);
    };
    let mut common = Common {
        server: DEFAULT_SERVER.to_string(),
        bind: None,
    };
    let mut seconds = None;
    let mut positional = Vec::new();
    match cmd.as_str() {
        "--version" | "-V" => return Ok(Command::Version),
        "--help" | "-h" | "help" => return Ok(Command::Help),
        "receive" | "send" => {}
        other => return Err(format!("unknown command {other:?}")),
    }
    while let Some(a) = args.next() {
        match a.as_str() {
            "--server" => common.server = args.next().ok_or("--server needs a value")?,
            "--bind" => common.bind = Some(args.next().ok_or("--bind needs a value")?),
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
            return Err("receive takes only --server and --bind".into());
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
        Command::Receive(common) => receive(common).await,
        Command::Send {
            code,
            seconds,
            common,
        } => send(code, seconds, common).await,
    }
}

async fn receive(common: Common) -> ExitCode {
    let mut opts = ReceiverOptions::new(common.server);
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
    fn rejects_bad_input() {
        assert!(parse("send").is_err());
        assert!(parse("send ABC").is_err());
        assert!(parse("send K7M2QX --seconds 0").is_err());
        assert!(parse("receive extra").is_err());
        assert!(parse("receive --nope").is_err());
        assert!(parse("dance").is_err());
    }
}
