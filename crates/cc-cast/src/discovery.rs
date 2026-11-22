// SPDX-License-Identifier: GPL-3.0-or-later
//! Finding Cast devices with mDNS / DNS-SD (`_googlecast._tcp.local.`).
//!
//! The service id and the TXT keys (`id`, `ve`, `ca`, `st`, `fn`, `md`) are taken from
//! `cast/common/public/receiver_info.h` and `receiver_info.cc` of Open Screen (commit
//! `8b62b21479f1adb7663f182d97962b960c5c64ef`, BSD-3-Clause, Copyright 2019 The Chromium
//! Authors). The browsing itself uses the `mdns-sd` crate.
//!
//! UNCERTAIN: never run against a real device. Real devices may answer only on some
//! network interfaces, and multicast may be filtered by Wi-Fi access points.

use std::net::IpAddr;
use std::time::{Duration, Instant};

use mdns_sd::{ServiceDaemon, ServiceEvent};

use crate::error::CastError;
use crate::watch::MAX_DEVICES;

/// DNS-SD service type (with the `.local.` domain) of Cast receivers.
pub const SERVICE_TYPE: &str = "_googlecast._tcp.local.";

/// Most TXT entries read from one record.
const MAX_TXT_ENTRIES: usize = 64;
/// Longest instance name, friendly name, model and id we keep, in characters. A DNS label
/// has at most 63 bytes and a TXT entry at most 255, so real values are not cut.
const MAX_TEXT_CHARS: usize = 128;
/// Most addresses kept for one device.
const MAX_ADDRESSES: usize = 16;

/// Cut text from the network to [`MAX_TEXT_CHARS`] characters.
fn cut(s: &str) -> String {
    s.chars().take(MAX_TEXT_CHARS).collect()
}

/// Whether an address from an mDNS record may be used as a connection target: private
/// (RFC 1918), carrier-grade NAT (`100.64.0.0/10`), unique-local IPv6 (`fc00::/7`) and
/// link-local addresses. Loopback, unspecified, multicast, broadcast and public addresses are
/// not: a record is unauthenticated, and a receiver of this network has no such address. An
/// IPv4-mapped IPv6 address is judged by its IPv4 address.
pub fn is_lan_addr(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(a) => {
            let o = a.octets();
            a.is_private() || a.is_link_local() || (o[0] == 100 && (64..128).contains(&o[1]))
        }
        IpAddr::V6(a) => match a.to_ipv4_mapped() {
            Some(v4) => is_lan_addr(IpAddr::V4(v4)),
            None => {
                let first = a.segments()[0];
                first & 0xfe00 == 0xfc00 || first & 0xffc0 == 0xfe80
            }
        },
    }
}

/// A Cast receiver found on the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastDevice {
    /// DNS-SD instance name (the part before the service type).
    pub instance: String,
    /// `id`: the unique device id (a hex string).
    pub id: String,
    /// `fn`: the friendly name shown to users (falls back to the instance name).
    pub friendly_name: String,
    /// `md`: the model name, e.g. what the device calls itself.
    pub model: String,
    /// `st`: 0 idle, 1 busy (UNCERTAIN: meaning taken from `ReceiverStatus` in Open Screen).
    pub status: Option<u8>,
    /// `ca`: capability bit mask.
    pub capabilities: Option<u32>,
    /// `ve`: TXT record version.
    pub txt_version: Option<u32>,
    /// Addresses of the device (IPv4 first). [`CastDevice::from_txt`] keeps only
    /// local-network addresses ([`is_lan_addr`]).
    pub addresses: Vec<IpAddr>,
    /// TCP port of the Cast channel (normally 8009).
    pub port: u16,
}

impl CastDevice {
    /// Build a device from DNS-SD data. `txt` are the TXT key/value pairs.
    ///
    /// The data comes from the network and is untrusted: texts are cut to a fixed length,
    /// only the first TXT entries are read, and addresses that are not local-network ones
    /// ([`is_lan_addr`]) are left out.
    pub fn from_txt<'a>(
        full_name: &str,
        port: u16,
        addresses: impl IntoIterator<Item = IpAddr>,
        txt: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Self {
        let instance = cut(full_name
            .strip_suffix(SERVICE_TYPE)
            .unwrap_or(full_name)
            .trim_end_matches('.'));
        let mut dev = Self {
            instance: instance.clone(),
            id: String::new(),
            friendly_name: String::new(),
            model: String::new(),
            status: None,
            capabilities: None,
            txt_version: None,
            addresses: Vec::new(),
            port,
        };
        for (k, v) in txt.into_iter().take(MAX_TXT_ENTRIES) {
            match k {
                "id" => dev.id = cut(v),
                "fn" => dev.friendly_name = cut(v),
                "md" => dev.model = cut(v),
                "st" => dev.status = v.parse().ok(),
                "ca" => dev.capabilities = v.parse().ok(),
                "ve" => dev.txt_version = v.parse().ok(),
                _ => {}
            }
        }
        if dev.friendly_name.is_empty() {
            dev.friendly_name = instance;
        }
        let mut addrs: Vec<IpAddr> = addresses
            .into_iter()
            .take(64)
            .filter(|a| is_lan_addr(*a))
            .take(MAX_ADDRESSES)
            .collect();
        addrs.sort_by_key(|a| (a.is_ipv6(), *a));
        dev.addresses = addrs;
        dev
    }

    /// Does the device have a screen (`ca` bit 0, "video out", `receiver_info.h`)? An audio-only
    /// device (a speaker) does not. Without a `ca` value the answer is yes (UNCERTAIN: a device
    /// that leaves the key out is assumed to be a TV).
    pub fn has_video_out(&self) -> bool {
        self.capabilities.is_none_or(|c| c & 1 == 1)
    }

    /// The address to connect to: the first IPv4 address, else the first address.
    pub fn connect_ip(&self) -> Option<IpAddr> {
        self.addresses.first().copied()
    }
}

/// Add a resolved device to the list of a one-shot search: a record without port or usable
/// address is not a device, a known instance is replaced, and at most [`MAX_DEVICES`] devices
/// are kept (the list comes from the network). Returns whether the device is in the list now.
fn remember(found: &mut Vec<CastDevice>, dev: CastDevice) -> bool {
    if dev.port == 0 || dev.addresses.is_empty() {
        return false;
    }
    if let Some(old) = found.iter_mut().find(|d| d.instance == dev.instance) {
        *old = dev;
    } else if found.len() < MAX_DEVICES {
        found.push(dev);
    } else {
        return false;
    }
    true
}

/// Browse for `duration` and return every device that resolved (at most [`MAX_DEVICES`]).
///
/// Blocking (the `mdns-sd` receiver is a blocking channel): call it from
/// `tokio::task::spawn_blocking` in async code, or use [`discover`].
pub fn discover_blocking(duration: Duration) -> Result<Vec<CastDevice>, CastError> {
    let daemon = ServiceDaemon::new().map_err(|e| CastError::Discovery(e.to_string()))?;
    let rx = daemon
        .browse(SERVICE_TYPE)
        .map_err(|e| CastError::Discovery(e.to_string()))?;
    let deadline = Instant::now() + duration;
    let mut found: Vec<CastDevice> = Vec::new();
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(ServiceEvent::ServiceResolved(info)) => {
                let props: Vec<(String, String)> = info
                    .get_properties()
                    .iter()
                    .map(|p| (p.key().to_owned(), p.val_str().to_owned()))
                    .collect();
                let dev = CastDevice::from_txt(
                    info.get_fullname(),
                    info.get_port(),
                    info.get_addresses()
                        .iter()
                        .map(mdns_sd::ScopedIp::to_ip_addr),
                    props.iter().map(|(k, v)| (k.as_str(), v.as_str())),
                );
                remember(&mut found, dev);
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    let _ = daemon.stop_browse(SERVICE_TYPE);
    let _ = daemon.shutdown();
    Ok(found)
}

/// Async wrapper around [`discover_blocking`].
pub async fn discover(duration: Duration) -> Result<Vec<CastDevice>, CastError> {
    tokio::task::spawn_blocking(move || discover_blocking(duration))
        .await
        .map_err(|e| CastError::Discovery(e.to_string()))?
}

/// Pick a device by (case-insensitive) friendly name, instance name or id.
pub fn find_by_name<'a>(devices: &'a [CastDevice], name: &str) -> Option<&'a CastDevice> {
    devices.iter().find(|d| {
        d.friendly_name.eq_ignore_ascii_case(name)
            || d.instance.eq_ignore_ascii_case(name)
            || d.id.eq_ignore_ascii_case(name)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_txt_record() {
        let d = CastDevice::from_txt(
            "Chromecast-0123abcd._googlecast._tcp.local.",
            8009,
            ["fe80::1".parse().unwrap(), "192.168.1.20".parse().unwrap()],
            [
                ("id", "0123abcd"),
                ("fn", "Living Room"),
                ("md", "Chromecast"),
                ("st", "0"),
                ("ca", "199172"),
                ("ve", "05"),
                ("xx", "ignored"),
            ],
        );
        assert_eq!(d.instance, "Chromecast-0123abcd");
        assert_eq!(d.id, "0123abcd");
        assert_eq!(d.friendly_name, "Living Room");
        assert_eq!(d.model, "Chromecast");
        assert_eq!(d.status, Some(0));
        assert_eq!(d.capabilities, Some(199_172));
        assert_eq!(d.txt_version, Some(5));
        assert_eq!(d.port, 8009);
        // IPv4 sorts before IPv6.
        assert_eq!(d.connect_ip(), Some("192.168.1.20".parse().unwrap()));
    }

    #[test]
    fn only_local_network_addresses_are_connection_targets() {
        let ok = [
            "10.0.0.5",
            "172.16.0.1",
            "192.168.1.20",
            "100.64.0.1",
            "169.254.10.20",
            "fe80::1",
            "fd12:3456::1",
            "::ffff:192.168.1.9",
        ];
        let bad = [
            "127.0.0.1",
            "0.0.0.0",
            "224.0.0.251",
            "255.255.255.255",
            "8.8.8.8",
            "172.32.0.1",
            "100.128.0.1",
            "::1",
            "::",
            "ff02::fb",
            "2001:db8::1",
            "::ffff:127.0.0.1",
        ];
        for a in ok {
            assert!(is_lan_addr(a.parse().unwrap()), "{a}");
        }
        for a in bad {
            assert!(!is_lan_addr(a.parse().unwrap()), "{a}");
        }
        let all = ok.iter().chain(&bad).map(|a| a.parse().unwrap());
        let d = CastDevice::from_txt("A._googlecast._tcp.local.", 8009, all, []);
        assert_eq!(d.addresses.len(), ok.len());
        assert!(d.addresses.iter().all(|a| is_lan_addr(*a)));
        let none = bad.iter().map(|a| a.parse().unwrap());
        let d = CastDevice::from_txt("A._googlecast._tcp.local.", 8009, none, []);
        assert_eq!(d.connect_ip(), None);
    }

    #[test]
    fn hostile_txt_is_cut() {
        let long = "x".repeat(10_000);
        let name = format!("{long}.{SERVICE_TYPE}");
        let d = CastDevice::from_txt(
            &name,
            8009,
            ["10.0.0.2".parse().unwrap()],
            [
                ("id", long.as_str()),
                ("fn", long.as_str()),
                ("md", long.as_str()),
            ],
        );
        for text in [&d.instance, &d.id, &d.friendly_name, &d.model] {
            assert_eq!(text.chars().count(), MAX_TEXT_CHARS);
        }
        // Cutting counts characters, so a multi-byte name is not split inside a character.
        let d = CastDevice::from_txt(
            "A._googlecast._tcp.local.",
            8009,
            ["10.0.0.2".parse().unwrap()],
            [("fn", "ä".repeat(300).as_str())],
        );
        assert_eq!(d.friendly_name, "ä".repeat(MAX_TEXT_CHARS));
        // Entries after the cap are not read.
        let junk: Vec<(String, String)> = (0..MAX_TXT_ENTRIES)
            .map(|i| (format!("k{i}"), "v".to_owned()))
            .collect();
        let d = CastDevice::from_txt(
            "A._googlecast._tcp.local.",
            8009,
            ["10.0.0.2".parse().unwrap()],
            junk.iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .chain([("id", "late")]),
        );
        assert_eq!(d.id, "");
    }

    #[test]
    fn a_one_shot_search_keeps_a_capped_list() {
        let dev = |name: &str, port: u16, addr: &str| {
            CastDevice::from_txt(
                &format!("{name}.{SERVICE_TYPE}"),
                port,
                addr.parse::<IpAddr>(),
                [("id", name)],
            )
        };
        let mut found = Vec::new();
        assert!(!remember(&mut found, dev("NoPort", 0, "10.0.0.1")));
        assert!(!remember(&mut found, dev("Public", 8009, "8.8.8.8")));
        for i in 0..MAX_DEVICES {
            assert!(remember(
                &mut found,
                dev(&format!("D{i}"), 8009, "10.0.0.1")
            ));
        }
        assert!(!remember(&mut found, dev("Extra", 8009, "10.0.0.1")));
        assert_eq!(found.len(), MAX_DEVICES);
        // A known device may still change.
        assert!(remember(&mut found, dev("D0", 8010, "10.0.0.1")));
        assert_eq!(found.len(), MAX_DEVICES);
        assert_eq!(found[0].port, 8010);
    }

    #[test]
    fn video_out_is_bit_zero_of_ca() {
        let d = |ca: Option<&str>| {
            CastDevice::from_txt(
                "A._googlecast._tcp.local.",
                8009,
                ["10.0.0.2".parse().unwrap()],
                ca.map(|c| ("ca", c)),
            )
        };
        assert!(d(Some("463365")).has_video_out()); // a Chromecast
        assert!(!d(Some("4")).has_video_out()); // audio out only: a speaker
        assert!(d(None).has_video_out());
    }

    #[test]
    fn friendly_name_falls_back_to_instance() {
        let d = CastDevice::from_txt(
            "Box-1._googlecast._tcp.local.",
            8009,
            ["10.0.0.2".parse().unwrap()],
            [("id", "x")],
        );
        assert_eq!(d.friendly_name, "Box-1");
    }

    #[test]
    fn find_by_name_is_case_insensitive() {
        let devs = vec![CastDevice::from_txt(
            "A._googlecast._tcp.local.",
            8009,
            ["10.0.0.2".parse().unwrap()],
            [("fn", "Kitchen TV"), ("id", "ab12")],
        )];
        assert!(find_by_name(&devs, "kitchen tv").is_some());
        assert!(find_by_name(&devs, "AB12").is_some());
        assert!(find_by_name(&devs, "nope").is_none());
    }
}
