// SPDX-License-Identifier: GPL-3.0-or-later
//! Comet Cast server program: signalling over WebSocket (`/ws`), `/healthz`,
//! and optionally the web app as static files. No TLS yet.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;

use cc_server::{ServerConfig, VERSION};

const USAGE: &str = "Usage: cometcast-server [--listen <addr:port>] [--web-dir <path>]\n\
  --listen <addr:port>  address to listen on (default 127.0.0.1:8443)\n\
  --web-dir <path>      serve this directory (the web app) from the same address\n\
  --help                show this text";

/// Read the command line by hand (no extra library needed for two flags).
fn parse_args(args: impl Iterator<Item = String>) -> Result<Option<ServerConfig>, String> {
    let mut config = ServerConfig::default();
    let mut args = args;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => return Ok(None),
            "--listen" => {
                let v = args.next().ok_or("--listen needs a value")?;
                config.listen_addr = v
                    .parse::<SocketAddr>()
                    .map_err(|e| format!("bad --listen value {v:?}: {e}"))?;
            }
            "--web-dir" => {
                let v = args.next().ok_or("--web-dir needs a value")?;
                config.web_dir = Some(PathBuf::from(v));
            }
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    Ok(Some(config))
}

#[tokio::main]
async fn main() -> ExitCode {
    let config = match parse_args(std::env::args().skip(1)) {
        Ok(Some(c)) => c,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("error: {e}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let listener = match tokio::net::TcpListener::bind(config.listen_addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("error: cannot listen on {}: {e}", config.listen_addr);
            return ExitCode::FAILURE;
        }
    };
    // With port 0 the OS picks the port, so print the real address.
    let addr = listener.local_addr().unwrap_or(config.listen_addr);
    println!("cometcast-server {VERSION}");
    println!("listening on http://{addr} (WebSocket: ws://{addr}/ws, health: /healthz)");
    if let Some(dir) = &config.web_dir {
        println!("serving web files from {}", dir.display());
    }
    println!("press Ctrl-C to stop");

    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
        println!("shutting down");
    };
    match cc_server::run(listener, config, shutdown).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
