// SPDX-License-Identifier: GPL-3.0-or-later
//! A running search for Cast receivers (`_googlecast._tcp`): a device that appears, changes or
//! goes away is reported as it happens, until the [`Watch`] is stopped or dropped. The list of
//! TVs in the presenter app is built from this (native-apps.md 21.5).
//!
//! UNCERTAIN: never run against a real device (like [`crate::discovery`]).

use std::collections::BTreeMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;
use std::time::Duration;

use mdns_sd::{ServiceDaemon, ServiceEvent};

use crate::discovery::{CastDevice, SERVICE_TYPE};
use crate::error::CastError;

/// Most devices a watch keeps (the list comes from the network and is untrusted).
pub const MAX_DEVICES: usize = 256;

/// What a [`Watch`] reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchEvent {
    /// A device appeared or its record changed.
    Seen(CastDevice),
    /// The device with this id went away (`CastDevice::id`, or its instance name when the
    /// record had no id).
    Gone {
        /// The id the device had in its last `Seen`.
        id: String,
    },
}

/// The stable id of a device: its `id` TXT value, else the instance name.
pub fn device_key(d: &CastDevice) -> String {
    if d.id.is_empty() {
        d.instance.clone()
    } else {
        d.id.clone()
    }
}

/// Keeps the full-name to id map and turns mDNS events into [`WatchEvent`]s. Pure, for tests.
#[derive(Default)]
pub struct Tracker {
    keys: BTreeMap<String, String>,
}

impl Tracker {
    /// A resolved service; `None` for a record that is not usable (no port or address) or when
    /// the cap is reached.
    pub fn resolved(&mut self, dev: CastDevice, fullname: &str) -> Option<WatchEvent> {
        if dev.port == 0 || dev.addresses.is_empty() {
            return None;
        }
        if self.keys.len() >= MAX_DEVICES && !self.keys.contains_key(fullname) {
            return None;
        }
        self.keys.insert(fullname.to_owned(), device_key(&dev));
        Some(WatchEvent::Seen(dev))
    }

    /// A removed service.
    pub fn removed(&mut self, fullname: &str) -> Option<WatchEvent> {
        self.keys.remove(fullname).map(|id| WatchEvent::Gone { id })
    }
}

/// A running search. Dropping it stops the search.
pub struct Watch {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Watch {
    /// Start browsing. Events arrive on the returned channel until the watch stops.
    pub fn start() -> Result<(Self, Receiver<WatchEvent>), CastError> {
        let daemon = ServiceDaemon::new().map_err(|e| CastError::Discovery(e.to_string()))?;
        let rx = daemon
            .browse(SERVICE_TYPE)
            .map_err(|e| CastError::Discovery(e.to_string()))?;
        let (tx, out) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let thread = std::thread::Builder::new()
            .name("cc-cast-watch".into())
            .spawn(move || run(&daemon, &rx, &tx, &flag))
            .map_err(|e| CastError::Discovery(e.to_string()))?;
        Ok((
            Self {
                stop,
                thread: Some(thread),
            },
            out,
        ))
    }

    /// Stop the search and wait for its thread.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run(
    daemon: &ServiceDaemon,
    rx: &mdns_sd::Receiver<ServiceEvent>,
    tx: &Sender<WatchEvent>,
    stop: &AtomicBool,
) {
    let mut tracker = Tracker::default();
    while !stop.load(Ordering::Relaxed) {
        let ev = match rx.recv_timeout(Duration::from_millis(200)) {
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
                tracker.resolved(dev, info.get_fullname())
            }
            Ok(ServiceEvent::ServiceRemoved(_, fullname)) => tracker.removed(&fullname),
            Ok(_) => None,
            Err(mdns_sd::RecvTimeoutError::Timeout) => None,
            Err(mdns_sd::RecvTimeoutError::Disconnected) => break,
        };
        if let Some(ev) = ev
            && tx.send(ev).is_err()
        {
            break;
        }
    }
    let _ = daemon.stop_browse(SERVICE_TYPE);
    let _ = daemon.shutdown();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(name: &str, id: &str, port: u16, addr: &str) -> CastDevice {
        CastDevice::from_txt(
            &format!("{name}.{SERVICE_TYPE}"),
            port,
            addr.parse::<std::net::IpAddr>(),
            [("id", id), ("fn", "Kitchen TV")],
        )
    }

    #[test]
    fn seen_then_gone_by_id() {
        let mut t = Tracker::default();
        let full = format!("Chromecast-1.{SERVICE_TYPE}");
        assert!(matches!(
            t.resolved(dev("Chromecast-1", "ab12", 8009, "10.0.0.5"), &full),
            Some(WatchEvent::Seen(d)) if d.friendly_name == "Kitchen TV"
        ));
        assert_eq!(
            t.removed(&full),
            Some(WatchEvent::Gone { id: "ab12".into() })
        );
        assert_eq!(t.removed(&full), None);
    }

    #[test]
    fn a_device_without_id_is_known_by_its_instance_name() {
        let d = dev("Box-7", "", 8009, "10.0.0.6");
        assert_eq!(device_key(&d), "Box-7");
    }

    #[test]
    fn unusable_records_and_the_cap() {
        let mut t = Tracker::default();
        assert!(t.resolved(dev("A", "a", 0, "10.0.0.1"), "A").is_none());
        let mut none = dev("B", "b", 8009, "10.0.0.1");
        none.addresses.clear();
        assert!(t.resolved(none, "B").is_none());
        for i in 0..MAX_DEVICES {
            let n = format!("D{i}");
            assert!(t.resolved(dev(&n, &n, 8009, "10.0.0.1"), &n).is_some());
        }
        assert!(t.resolved(dev("X", "x", 8009, "10.0.0.1"), "X").is_none());
        assert!(
            t.resolved(dev("D0", "d0", 8010, "10.0.0.1"), "D0")
                .is_some()
        );
    }
}
