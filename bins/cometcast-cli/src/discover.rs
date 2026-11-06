// SPDX-License-Identifier: GPL-3.0-or-later
//! `cometcast-cli discover`: print where the server list comes from (docs/discovery.md).

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use cc_discovery::{
    AdvertiseOptions, DiscoverOptions, PolicyState, ReceiverEvent, advertise_receiver,
    discover_receivers, discover_servers, policy::explicit_location,
};

/// Arguments of `discover`.
pub struct DiscoverArgs {
    seconds: u64,
    policy_file: Option<PathBuf>,
    announce: Option<String>,
}

/// Parse the arguments after `discover`. `Ok(None)` means help was asked for.
pub fn parse(mut args: impl Iterator<Item = String>) -> Result<Option<DiscoverArgs>, String> {
    let mut out = DiscoverArgs {
        seconds: 3,
        policy_file: None,
        announce: None,
    };
    while let Some(a) = args.next() {
        match a.as_str() {
            "--seconds" => {
                let v = args.next().ok_or("--seconds needs a value")?;
                out.seconds = v
                    .parse::<u64>()
                    .ok()
                    .filter(|n| (1..=3600).contains(n))
                    .ok_or(format!("bad --seconds value {v:?} (1 to 3600)"))?;
            }
            "--policy-file" => {
                out.policy_file = Some(args.next().ok_or("--policy-file needs a value")?.into());
            }
            "--announce" => out.announce = Some(args.next().ok_or("--announce needs a value")?),
            "--help" | "-h" => return Ok(None),
            other => return Err(format!("unknown argument {other:?} for discover")),
        }
    }
    Ok(Some(out))
}

/// Run `discover`.
pub async fn run(a: DiscoverArgs) -> ExitCode {
    // Keep the announcement alive while we look around (also makes a one-machine test easy).
    let _ad = match &a.announce {
        Some(server) => match advertise_receiver(&AdvertiseOptions {
            instance_name: "Comet Cast CLI".into(),
            port: 9,
            preferred_server: Some(server.clone()),
            host_label: None,
        }) {
            Ok(ad) => {
                println!("announcing {server} as {}", ad.fullname());
                Some(ad)
            }
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::from(1);
            }
        },
        None => None,
    };
    let opts = DiscoverOptions {
        policy_locations: a.policy_file.as_deref().map(|p| vec![explicit_location(p)]),
        mdns_wait: Duration::from_secs(a.seconds),
        ..DiscoverOptions::default()
    };
    // The receiver browse runs at the same time as the server lookup.
    let browser = discover_receivers();
    let mut receivers = Vec::new();
    let report = discover_servers(&opts);
    // Listen for receivers for the same time as the announcement browse.
    let listen = async {
        if let Ok(mut b) = browser {
            let _ = tokio::time::timeout(Duration::from_secs(a.seconds), async {
                while let Some(ev) = b.next().await {
                    receivers.push(ev);
                }
            })
            .await;
        }
    };
    let (report, ()) = tokio::join!(report, listen);

    match &report.policy {
        PolicyState::Absent => println!("policy: none"),
        PolicyState::Active(_) => println!("policy: active (mode hint {:?})", report.mode_hint),
        PolicyState::Invalid(why) => println!("policy: UNUSABLE, failing closed ({why})"),
    }
    if report.locked {
        println!("locked: other servers are not allowed");
    }
    println!("servers (best first):");
    for (i, s) in report.servers.iter().enumerate() {
        match &s.detail {
            Some(d) => println!("  {}. {}  [{}]  {d}", i + 1, s.address, s.source.as_str()),
            None => println!("  {}. {}  [{}]", i + 1, s.address, s.source.as_str()),
        }
    }
    if report.servers.is_empty() {
        println!("  (none)");
    }
    for w in &report.warnings {
        println!("note: {w}");
    }
    println!("receivers:");
    let mut n = 0;
    for ev in &receivers {
        if let ReceiverEvent::Found(r) = ev {
            n += 1;
            println!(
                "  {}  addresses {:?}  announced server {}",
                r.name,
                r.addresses,
                r.announced_server.as_deref().unwrap_or("-")
            );
        }
    }
    if n == 0 {
        println!("  (none in {} s)", a.seconds);
    }
    ExitCode::SUCCESS
}
