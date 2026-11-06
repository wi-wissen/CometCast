// SPDX-License-Identifier: GPL-3.0-or-later
//! Comet Cast server program: signalling over WebSocket (`/ws`), `/healthz`,
//! ALTCHA puzzles, optional TURN credentials, optional HTTPS, and optionally the web
//! app as static files. Settings: config file, environment, command line (docs/server.md).

use std::path::PathBuf;
use std::process::ExitCode;

use cc_server::{
    AltchaMode, DEFAULT_CONFIG_TOML, Mode, ServerConfig, VERSION, load_config, overrides_from_env,
};

const USAGE: &str = "Usage: cometcast-server [options]\n\
  --config <file>        TOML configuration file (or env COMETCAST_CONFIG)\n\
  --mode <open|internal> operating mode (default internal)\n\
  --listen <addr:port>   address to listen on (default 127.0.0.1:8443)\n\
  --web-dir <path>       serve this directory (the web app) from the same address\n\
  --tls-cert <file>      certificate chain (PEM); use with --tls-key to serve HTTPS\n\
  --tls-key <file>       private key (PEM)\n\
  --tls-self-signed      DEVELOPMENT ONLY: make a throw-away self-signed certificate\n\
  --set <key=value>      set any setting, e.g. --set limits.max_rooms=500 (repeatable)\n\
  --print-default-config print the default configuration file and exit\n\
  --version              print the version and exit\n\
  --help                 show this text\n\
Order of precedence: config file, then COMETCAST_* environment variables, then these options.";

/// What the command line asked for.
#[derive(Debug, Default, PartialEq, Eq)]
struct Cli {
    config_file: Option<PathBuf>,
    overrides: Vec<(String, String)>,
    print_default_config: bool,
    version: bool,
    help: bool,
}

/// Read the command line by hand (a few flags do not need an extra library).
fn parse_args(args: impl Iterator<Item = String>) -> Result<Cli, String> {
    let mut cli = Cli::default();
    let mut args = args;
    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "--help" | "-h" => cli.help = true,
            "--version" | "-V" => cli.version = true,
            "--print-default-config" => cli.print_default_config = true,
            "--config" => cli.config_file = Some(PathBuf::from(value("--config")?)),
            "--mode" => cli.overrides.push(("mode".into(), value("--mode")?)),
            "--listen" => cli.overrides.push(("listen".into(), value("--listen")?)),
            "--web-dir" => cli.overrides.push(("web_dir".into(), value("--web-dir")?)),
            "--tls-cert" => cli
                .overrides
                .push(("tls.cert_file".into(), value("--tls-cert")?)),
            "--tls-key" => cli
                .overrides
                .push(("tls.key_file".into(), value("--tls-key")?)),
            "--tls-self-signed" => cli
                .overrides
                .push(("tls.self_signed".into(), "true".into())),
            "--set" => {
                let v = value("--set")?;
                let (k, val) = v
                    .split_once('=')
                    .ok_or(format!("--set needs key=value, got {v:?}"))?;
                cli.overrides.push((k.trim().into(), val.trim().into()));
            }
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    Ok(cli)
}

/// Combine file, environment and command line into the final settings.
fn build_config(cli: &Cli) -> Result<ServerConfig, String> {
    let env_file = std::env::var_os("COMETCAST_CONFIG").map(PathBuf::from);
    let file = cli.config_file.clone().or(env_file);
    let mut overrides = overrides_from_env(std::env::vars());
    overrides.extend(cli.overrides.iter().cloned());
    load_config(file.as_deref(), &overrides).map_err(|e| e.to_string())
}

fn describe(config: &ServerConfig, addr: std::net::SocketAddr) {
    let https = config.tls.enabled();
    let (http, ws) = if https {
        ("https", "wss")
    } else {
        ("http", "ws")
    };
    println!("cometcast-server {VERSION}");
    println!("listening on {http}://{addr} (WebSocket: {ws}://{addr}/ws, health: /healthz)");
    let mode = match config.mode {
        Mode::Open => "open (mediation only)",
        Mode::Internal => "internal",
    };
    let altcha = match config.altcha.mode {
        AltchaMode::Always => "required",
        AltchaMode::Optional => "optional",
    };
    println!("mode: {mode}; ALTCHA for create_room: {altcha}");
    if !https {
        println!(
            "TLS: off (plain HTTP). Browsers need HTTPS for screen sharing except on localhost."
        );
    }
    if config.turn.is_some() {
        println!("TURN credentials: on (GET /ice-servers)");
    }
    if let Some(dir) = &config.web_dir {
        println!("serving web files from {}", dir.display());
    }
    if !config.network.trusted_proxies.is_empty() {
        println!(
            "trusting X-Forwarded-For from {} proxy network(s)",
            config.network.trusted_proxies.len()
        );
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = match parse_args(std::env::args().skip(1)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    if cli.help {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if cli.version {
        println!("cometcast-server {VERSION}");
        return ExitCode::SUCCESS;
    }
    if cli.print_default_config {
        print!("{DEFAULT_CONFIG_TOML}");
        return ExitCode::SUCCESS;
    }
    let config = match build_config(&cli) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {e}");
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
    describe(&config, addr);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> impl Iterator<Item = String> {
        list.iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn flags_become_overrides() {
        let cli = parse_args(args(&[
            "--config",
            "a.toml",
            "--mode",
            "open",
            "--listen",
            "0.0.0.0:1",
            "--tls-self-signed",
            "--set",
            "limits.max_rooms=5",
        ]))
        .unwrap();
        assert_eq!(cli.config_file, Some(PathBuf::from("a.toml")));
        assert_eq!(
            cli.overrides,
            vec![
                ("mode".to_string(), "open".to_string()),
                ("listen".to_string(), "0.0.0.0:1".to_string()),
                ("tls.self_signed".to_string(), "true".to_string()),
                ("limits.max_rooms".to_string(), "5".to_string()),
            ]
        );
    }

    #[test]
    fn bad_flags_are_errors() {
        assert!(parse_args(args(&["--nope"])).is_err());
        assert!(parse_args(args(&["--listen"])).is_err());
        assert!(parse_args(args(&["--set", "nokey"])).is_err());
    }
}
