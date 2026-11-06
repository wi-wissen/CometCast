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

/// DNS-SD service type (with the `.local.` domain) of Cast receivers.
pub const SERVICE_TYPE: &str = "_googlecast._tcp.local.";

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
    /// Addresses of the device (IPv4 first).
    pub addresses: Vec<IpAddr>,
    /// TCP port of the Cast channel (normally 8009).
    pub port: u16,
}

impl CastDevice {
    /// Build a device from DNS-SD data. `txt` are the TXT key/value pairs.
    pub fn from_txt<'a>(
        full_name: &str,
        port: u16,
        addresses: impl IntoIterator<Item = IpAddr>,
        txt: impl IntoIterator<Item = (&'a str, &'a str)>,
    ) -> Self {
        let instance = full_name
            .strip_suffix(SERVICE_TYPE)
            .unwrap_or(full_name)
            .trim_end_matches('.')
            .to_owned();
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
        for (k, v) in txt {
            match k {
                "id" => dev.id = v.to_owned(),
                "fn" => dev.friendly_name = v.to_owned(),
                "md" => dev.model = v.to_owned(),
                "st" => dev.status = v.parse().ok(),
                "ca" => dev.capabilities = v.parse().ok(),
                "ve" => dev.txt_version = v.parse().ok(),
                _ => {}
            }
        }
        if dev.friendly_name.is_empty() {
            dev.friendly_name = instance;
        }
        let mut addrs: Vec<IpAddr> = addresses.into_iter().collect();
        addrs.sort_by_key(|a| (a.is_ipv6(), *a));
        dev.addresses = addrs;
        dev
    }

    /// The address to connect to: the first IPv4 address, else the first address.
    pub fn connect_ip(&self) -> Option<IpAddr> {
        self.addresses.first().copied()
    }
}

/// Browse for `duration` and return every device that resolved.
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
                if dev.port != 0 && !dev.addresses.is_empty() {
                    if let Some(old) = found.iter_mut().find(|d| d.instance == dev.instance) {
                        *old = dev;
                    } else {
                        found.push(dev);
                    }
                }
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
