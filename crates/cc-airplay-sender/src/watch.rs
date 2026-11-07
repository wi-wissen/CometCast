// SPDX-License-Identifier: GPL-3.0-or-later
//! A running search for AirPlay receivers (`_airplay._tcp`): a device that appears, changes or
//! goes away is reported as it happens, until the [`Watch`] is stopped or dropped. The list of
//! TVs in the presenter app is built from this (native-apps.md 21.5).
//!
//! Same limits as [`crate::discovery::discover`]: at most [`MAX_DEVICES`] devices, text capped.

use std::collections::BTreeMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;
use std::time::Duration;

use mdns_sd::{ServiceDaemon, ServiceEvent};

use crate::discovery::{Device, MAX_DEVICES, SERVICE_TYPE, device_from_parts};
use crate::error::{Result, SenderError};

/// What a [`Watch`] reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchEvent {
    /// A receiver appeared or its record changed.
    Seen(Device),
    /// The receiver with this key ([`Device::store_key`]) went away.
    Gone {
        /// The key the device had in its last `Seen`.
        key: String,
    },
}

/// Keeps the full-name to key map and turns mDNS events into [`WatchEvent`]s. Pure, for tests.
#[derive(Default)]
pub struct Tracker {
    keys: BTreeMap<String, String>,
}

impl Tracker {
    /// A resolved service.
    pub fn resolved(
        &mut self,
        fullname: &str,
        host: &str,
        port: u16,
        addrs: impl IntoIterator<Item = std::net::IpAddr>,
        txt: impl IntoIterator<Item = (String, String)>,
    ) -> Option<WatchEvent> {
        if self.keys.len() >= MAX_DEVICES && !self.keys.contains_key(fullname) {
            return None;
        }
        let dev = device_from_parts(fullname, host, port, addrs, txt);
        if port == 0 || dev.addrs.is_empty() {
            return None;
        }
        self.keys.insert(fullname.to_owned(), dev.store_key());
        Some(WatchEvent::Seen(dev))
    }

    /// A removed service.
    pub fn removed(&mut self, fullname: &str) -> Option<WatchEvent> {
        self.keys
            .remove(fullname)
            .map(|key| WatchEvent::Gone { key })
    }
}

/// A running search. Dropping it stops the search.
pub struct Watch {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Watch {
    /// Start browsing. Events arrive on the returned channel until the watch stops.
    pub fn start() -> Result<(Self, Receiver<WatchEvent>)> {
        let daemon = ServiceDaemon::new().map_err(|e| SenderError::Discovery(e.to_string()))?;
        let rx = daemon
            .browse(SERVICE_TYPE)
            .map_err(|e| SenderError::Discovery(e.to_string()))?;
        let (tx, out) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let thread = std::thread::Builder::new()
            .name("cc-airplay-watch".into())
            .spawn(move || run(&daemon, &rx, &tx, &flag))
            .map_err(|e| SenderError::Discovery(e.to_string()))?;
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
                let txt: Vec<(String, String)> = info
                    .txt_properties
                    .iter()
                    .map(|p| (p.key().to_owned(), p.val_str().to_owned()))
                    .collect();
                tracker.resolved(
                    &info.fullname,
                    &info.host,
                    info.port,
                    info.addresses.iter().map(|a| a.to_ip_addr()),
                    txt,
                )
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

    fn txt(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn seen_then_gone_by_the_key_of_the_device() {
        let mut t = Tracker::default();
        let full = format!("Living Room.{SERVICE_TYPE}");
        let ev = t
            .resolved(
                &full,
                "tv.local.",
                7000,
                ["192.168.1.9".parse().unwrap()],
                txt(&[("deviceid", "AA:BB:CC:DD:EE:FF"), ("features", "0x1,0x0")]),
            )
            .unwrap();
        let WatchEvent::Seen(dev) = ev else {
            panic!("not Seen")
        };
        assert_eq!(dev.name, "Living Room");
        assert_eq!(
            t.removed(&full),
            Some(WatchEvent::Gone {
                key: "aa:bb:cc:dd:ee:ff".into()
            })
        );
        // A second removal says nothing.
        assert_eq!(t.removed(&full), None);
    }

    #[test]
    fn a_record_without_address_or_port_is_not_a_device() {
        let mut t = Tracker::default();
        assert!(
            t.resolved(
                "A._airplay._tcp.local.",
                "h",
                0,
                ["10.0.0.1".parse().unwrap()],
                []
            )
            .is_none()
        );
        assert!(
            t.resolved("A._airplay._tcp.local.", "h", 7000, [], [])
                .is_none()
        );
    }

    #[test]
    fn the_number_of_devices_is_capped() {
        let mut t = Tracker::default();
        for i in 0..MAX_DEVICES {
            let n = format!("D{i}._airplay._tcp.local.");
            assert!(
                t.resolved(&n, "h", 7000, ["10.0.0.1".parse().unwrap()], [])
                    .is_some()
            );
        }
        assert!(
            t.resolved(
                "Extra._airplay._tcp.local.",
                "h",
                7000,
                ["10.0.0.1".parse().unwrap()],
                []
            )
            .is_none()
        );
        // A known device may still change.
        assert!(
            t.resolved(
                "D0._airplay._tcp.local.",
                "h",
                7001,
                ["10.0.0.1".parse().unwrap()],
                []
            )
            .is_some()
        );
    }
}
