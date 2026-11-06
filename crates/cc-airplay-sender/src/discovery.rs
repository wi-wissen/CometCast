// SPDX-License-Identifier: GPL-3.0-or-later
//! Finding AirPlay receivers: mDNS service `_airplay._tcp` and its TXT record.
//!
//! Everything that comes from the network is untrusted: the number of devices, the number
//! and length of TXT entries and the text of names are capped, control characters in names
//! are replaced.
//!
//! TXT keys used (names from pyatv and the public AirPlay notes): `deviceid`, `model`,
//! `features` (or `ft`; `0xLOW,0xHIGH`), `flags` (or `sf`), `pk`, `pw`, `acl`, `srcvers`.
//! UNCERTAIN: the meaning of the feature bits is taken from pyatv's table (itself
//! "imported" from two public documents that it says are partly inconsistent).

use std::collections::BTreeMap;
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

use mdns_sd::{ServiceDaemon, ServiceEvent};

use crate::error::{Result, SenderError, clean};

/// The DNS-SD service type of AirPlay receivers.
pub const SERVICE_TYPE: &str = "_airplay._tcp.local.";
/// The older RAOP service type; its instance name starts with the MAC address. Used only as
/// a fallback port (see [`Device::raop_port`]).
pub const RAOP_TYPE: &str = "_raop._tcp.local.";
/// Default control port if a device is given as a bare address.
pub const DEFAULT_PORT: u16 = 7000;
/// Most devices we keep.
pub const MAX_DEVICES: usize = 256;
const MAX_TXT_ENTRIES: usize = 64;
const MAX_TXT_LEN: usize = 512;

/// Feature bits of the `features` TXT value that matter here (bit numbers).
pub mod feature {
    /// Video (AirPlay 1 style `/play`).
    pub const VIDEO_V1: u32 = 0;
    /// Photos.
    pub const PHOTO: u32 = 1;
    /// Screen mirroring (needs FairPlay; this crate cannot use it).
    pub const SCREEN: u32 = 7;
    /// Audio.
    pub const AUDIO: u32 = 9;
    /// Legacy pairing.
    pub const LEGACY_PAIRING: u32 = 27;
    /// Unified media control (AirPlay 2).
    pub const UNIFIED_MEDIA_CONTROL: u32 = 38;
    /// System pairing (transient).
    pub const SYSTEM_PAIRING: u32 = 43;
    /// HomeKit pairing and access control.
    pub const HK_PAIRING: u32 = 46;
    /// CoreUtils pairing and encryption.
    pub const CORE_UTILS_PAIRING: u32 = 48;
    /// Video v2.
    pub const VIDEO_V2: u32 = 49;
}

/// An AirPlay receiver found on the network (or built from an address).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    /// Name shown to people (the service instance name).
    pub name: String,
    /// DNS host name, if known.
    pub host: String,
    /// Addresses, IPv4 first.
    pub addrs: Vec<IpAddr>,
    /// Control port (usually 7000).
    pub port: u16,
    /// Port of the `_raop._tcp` service of the same device, if seen. UxPlay 1.68 advertises
    /// `_airplay._tcp` with a port one higher than the one it listens on (observed: listens
    /// 44545, advertises 44546; with `-p` listens 7000, advertises 7001) but advertises
    /// the listening port for `_raop._tcp`; the sender tries this port when the advertised
    /// one refuses the connection. UNCERTAIN: whether other receivers behave so.
    pub raop_port: Option<u16>,
    /// `deviceid` (a MAC address as text).
    pub device_id: Option<String>,
    /// `model`, for example `AppleTV6,2`.
    pub model: Option<String>,
    /// All feature bits (`features` / `ft`).
    pub features: u64,
    /// `flags` / `sf` value.
    pub flags: u64,
    /// `pk` (hex of the receiver's public key).
    pub public_key: Option<String>,
    /// `pw`: a password is required.
    pub password_required: bool,
    /// `acl` access control type, as text.
    pub acl: Option<String>,
    /// `srcvers` source version.
    pub source_version: Option<String>,
}

impl Device {
    /// Whether a feature bit is set.
    pub fn has_feature(&self, bit: u32) -> bool {
        bit < 64 && (self.features >> bit) & 1 == 1
    }

    /// Where to connect: the first address (IPv4 preferred) and the port.
    pub fn socket_addr(&self) -> Option<SocketAddr> {
        self.addrs.first().map(|a| SocketAddr::new(*a, self.port))
    }

    /// Key under which pairing credentials are stored.
    pub fn store_key(&self) -> String {
        match (&self.device_id, self.socket_addr()) {
            (Some(id), _) => id.to_ascii_lowercase(),
            (None, Some(a)) => a.to_string(),
            (None, None) => self.name.clone(),
        }
    }

    /// A device made from an address only (`host:port` or `host`).
    pub fn from_address(text: &str) -> Result<Self> {
        let addr: SocketAddr = match text.parse() {
            Ok(a) => a,
            Err(_) => {
                let ip: IpAddr = text
                    .parse()
                    .map_err(|_| SenderError::Input(format!("not an address: {text:?}")))?;
                SocketAddr::new(ip, DEFAULT_PORT)
            }
        };
        Ok(Self {
            name: addr.to_string(),
            host: String::new(),
            addrs: vec![addr.ip()],
            port: addr.port(),
            raop_port: None,
            device_id: None,
            model: None,
            features: 0,
            flags: 0,
            public_key: None,
            password_required: false,
            acl: None,
            source_version: None,
        })
    }

    /// One line for people.
    pub fn summary(&self) -> String {
        let mut caps = Vec::new();
        for (bit, name) in [
            (feature::VIDEO_V1, "video"),
            (feature::VIDEO_V2, "video2"),
            (feature::PHOTO, "photo"),
            (feature::SCREEN, "screen-mirroring(FairPlay)"),
            (feature::AUDIO, "audio"),
        ] {
            if self.has_feature(bit) {
                caps.push(name);
            }
        }
        let addr = self
            .socket_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|| "no address".into());
        format!(
            "{}\t{}\tmodel={}\tid={}\tfeatures=0x{:x}\t[{}]",
            self.name,
            addr,
            self.model.as_deref().unwrap_or("?"),
            self.device_id.as_deref().unwrap_or("?"),
            self.features,
            caps.join(",")
        )
    }
}

/// Parse a `features` TXT value: `0xLOW` or `0xLOW,0xHIGH`.
pub fn parse_features(text: &str) -> Option<u64> {
    fn part(s: &str) -> Option<u64> {
        let hex = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"))?;
        if hex.is_empty() || hex.len() > 8 {
            return None;
        }
        u64::from_str_radix(hex, 16).ok()
    }
    match text.trim().split_once(',') {
        Some((low, high)) => Some(part(low)? | (part(high)? << 32)),
        None => part(text.trim()),
    }
}

fn parse_flags(text: &str) -> u64 {
    let t = text.trim();
    let t = t
        .strip_prefix("0x")
        .or_else(|| t.strip_prefix("0X"))
        .unwrap_or(t);
    u64::from_str_radix(t, 16).unwrap_or(0)
}

/// Build a [`Device`] from the parts of a resolved mDNS service. Pure, for tests.
pub fn device_from_parts(
    fullname: &str,
    host: &str,
    port: u16,
    addrs: impl IntoIterator<Item = IpAddr>,
    txt: impl IntoIterator<Item = (String, String)>,
) -> Device {
    let mut map: BTreeMap<String, String> = BTreeMap::new();
    for (k, v) in txt.into_iter().take(MAX_TXT_ENTRIES) {
        if k.len() <= MAX_TXT_LEN && v.len() <= MAX_TXT_LEN {
            map.insert(k.to_ascii_lowercase(), v);
        }
    }
    let name = fullname
        .strip_suffix(&format!(".{SERVICE_TYPE}"))
        .unwrap_or(fullname);
    let mut addrs: Vec<IpAddr> = addrs.into_iter().take(16).collect();
    addrs.sort_by_key(|a| (!a.is_ipv4(), *a));
    let features = map
        .get("features")
        .or_else(|| map.get("ft"))
        .and_then(|f| parse_features(f))
        .unwrap_or(0);
    let flags = map
        .get("flags")
        .or_else(|| map.get("sf"))
        .map(|f| parse_flags(f))
        .unwrap_or(0);
    Device {
        name: clean(name, 128),
        host: clean(host, 255),
        addrs,
        port,
        raop_port: None,
        device_id: map.get("deviceid").map(|s| clean(s, 64)),
        model: map.get("model").map(|s| clean(s, 64)),
        features,
        flags,
        public_key: map.get("pk").map(|s| clean(s, 128)),
        password_required: map
            .get("pw")
            .is_some_and(|v| v.eq_ignore_ascii_case("true")),
        acl: map.get("acl").map(|s| clean(s, 16)),
        source_version: map.get("srcvers").map(|s| clean(s, 32)),
    }
}

/// Browse for `window` and return the receivers found, without duplicates.
pub fn discover(window: Duration) -> Result<Vec<Device>> {
    let daemon = ServiceDaemon::new().map_err(|e| SenderError::Discovery(e.to_string()))?;
    let rx = daemon
        .browse(SERVICE_TYPE)
        .map_err(|e| SenderError::Discovery(e.to_string()))?;
    let raop_rx = daemon
        .browse(RAOP_TYPE)
        .map_err(|e| SenderError::Discovery(e.to_string()))?;
    let end = Instant::now() + window;
    let mut found: BTreeMap<String, Device> = BTreeMap::new();
    // MAC address (lower-case hex, no colons) to RAOP port.
    let mut raop: BTreeMap<String, u16> = BTreeMap::new();
    while let Some(left) = end.checked_duration_since(Instant::now()) {
        // Poll both receivers in short slices so that neither starves the other.
        let slice = left.min(Duration::from_millis(100));
        if let Ok(ServiceEvent::ServiceResolved(info)) = rx.recv_timeout(slice) {
            if found.len() >= MAX_DEVICES && !found.contains_key(&info.fullname) {
                continue;
            }
            let txt: Vec<(String, String)> = info
                .txt_properties
                .iter()
                .map(|p| (p.key().to_owned(), p.val_str().to_owned()))
                .collect();
            let dev = device_from_parts(
                &info.fullname,
                &info.host,
                info.port,
                info.addresses.iter().map(|a| a.to_ip_addr()),
                txt,
            );
            found.insert(info.fullname.clone(), dev);
        }
        while let Ok(ev) = raop_rx.try_recv() {
            if let ServiceEvent::ServiceResolved(info) = ev
                && let Some((mac, _)) = info.fullname.split_once('@')
                && raop.len() < MAX_DEVICES
                && mac.len() <= 12
            {
                raop.insert(mac.to_ascii_lowercase(), info.port);
            }
        }
    }
    let _ = daemon.stop_browse(SERVICE_TYPE);
    let _ = daemon.stop_browse(RAOP_TYPE);
    let _ = daemon.shutdown();
    for d in found.values_mut() {
        let key: String = d
            .device_id
            .iter()
            .flat_map(|id| id.chars())
            .filter(|c| *c != ':')
            .collect::<String>()
            .to_ascii_lowercase();
        d.raop_port = raop.get(&key).copied().filter(|p| *p != d.port);
    }
    Ok(found.into_values().collect())
}

/// Find one device by name (exact, then prefix, case-insensitive), by `deviceid`, or take an
/// address (`host:port` or `host`) without any discovery.
pub fn find(selector: &str, window: Duration) -> Result<Device> {
    if let Ok(d) = Device::from_address(selector) {
        return Ok(d);
    }
    let all = discover(window)?;
    pick(&all, selector)
}

/// Choose a device from a list by name or id. Ambiguous prefixes are an error.
pub fn pick(all: &[Device], selector: &str) -> Result<Device> {
    let sel = selector.to_lowercase();
    if let Some(d) = all.iter().find(|d| {
        d.name.to_lowercase() == sel
            || d.device_id.as_deref().map(str::to_lowercase) == Some(sel.clone())
    }) {
        return Ok(d.clone());
    }
    let prefix: Vec<&Device> = all
        .iter()
        .filter(|d| d.name.to_lowercase().starts_with(&sel))
        .collect();
    match prefix.as_slice() {
        [one] => Ok((*one).clone()),
        [] => Err(SenderError::Discovery(format!(
            "no AirPlay receiver named {selector:?} found ({} seen)",
            all.len()
        ))),
        many => Err(SenderError::Discovery(format!(
            "{selector:?} matches {} receivers; use the full name",
            many.len()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn txt(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).into(), (*v).into()))
            .collect()
    }

    #[test]
    fn features_parse_like_pyatv() {
        assert_eq!(
            parse_features("0x4A7FDFD5,0x3C155FDE"),
            Some(0x3C155FDE_4A7FDFD5)
        );
        assert_eq!(parse_features("0x5A7FFFF7"), Some(0x5A7FFFF7));
        assert_eq!(parse_features("5A7FFFF7"), None);
        assert_eq!(parse_features("0x123456789"), None);
        assert_eq!(parse_features("0x"), None);
        assert_eq!(parse_features("0x1,0x"), None);
        assert_eq!(parse_features(""), None);
    }

    #[test]
    fn device_from_a_txt_record() {
        // TXT values taken from the sample record printed in pyatv's protocol documentation
        // (an Apple TV 4K, with the key replaced by a dummy).
        let d = device_from_parts(
            "Living Room._airplay._tcp.local.",
            "Living-Room.local.",
            7000,
            ["fe80::1".parse().unwrap(), "10.0.0.5".parse().unwrap()],
            txt(&[
                ("deviceid", "AA:BB:CC:DD:EE:FF"),
                ("features", "0x4A7FDFD5,0x3C155FDE"),
                ("flags", "0x244"),
                ("model", "AppleTV6,2"),
                ("pk", "aa"),
                ("acl", "0"),
                ("srcvers", "550.10"),
            ]),
        );
        assert_eq!(d.name, "Living Room");
        assert_eq!(d.addrs[0], "10.0.0.5".parse::<IpAddr>().unwrap());
        assert_eq!(d.model.as_deref(), Some("AppleTV6,2"));
        assert!(d.has_feature(feature::SCREEN));
        assert!(d.has_feature(feature::VIDEO_V1));
        assert!(d.has_feature(feature::CORE_UTILS_PAIRING));
        assert_eq!(d.flags, 0x244);
        assert!(!d.password_required);
        assert_eq!(d.store_key(), "aa:bb:cc:dd:ee:ff");
        assert!(d.summary().contains("AppleTV6,2"));
    }

    #[test]
    fn hostile_txt_is_capped_and_cleaned() {
        let mut pairs = txt(&[("model", "A\nB\u{7}")]);
        for i in 0..500 {
            pairs.push((format!("k{i}"), "v".into()));
        }
        pairs.push(("deviceid".into(), "x".repeat(10_000)));
        let d = device_from_parts("N\n._airplay._tcp.local.", "h", 1, [], pairs);
        assert_eq!(d.model.as_deref(), Some("A?B?"));
        assert_eq!(d.name, "N?");
        assert_eq!(d.device_id, None);
        assert_eq!(d.features, 0);
        assert_eq!(d.store_key(), d.name);
    }

    #[test]
    fn selecting_devices() {
        let mk = |n: &str, id: &str| {
            device_from_parts(
                &format!("{n}._airplay._tcp.local."),
                "h",
                7000,
                ["10.0.0.1".parse().unwrap()],
                txt(&[("deviceid", id)]),
            )
        };
        let all = [
            mk("Living Room", "AA:00"),
            mk("Lab TV", "BB:00"),
            mk("Lab Box", "CC:00"),
        ];
        assert_eq!(pick(&all, "living room").unwrap().name, "Living Room");
        assert_eq!(pick(&all, "bb:00").unwrap().name, "Lab TV");
        assert_eq!(pick(&all, "liv").unwrap().name, "Living Room");
        assert!(pick(&all, "lab").is_err());
        assert!(pick(&all, "none").is_err());
        assert_eq!(Device::from_address("10.1.2.3").unwrap().port, 7000);
        assert_eq!(Device::from_address("10.1.2.3:7100").unwrap().port, 7100);
        assert!(Device::from_address("not an address").is_err());
    }
}
