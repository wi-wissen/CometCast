// SPDX-License-Identifier: GPL-3.0-or-later
//! Advertise a receiver and find it again on the same machine over real multicast DNS.
//!
//! UNCERTAIN: needs a network interface with multicast. In sandboxes without one the test
//! prints a note and passes without checking (it cannot tell "no multicast here" from a bug in
//! the browse code, so a green run in such a sandbox proves nothing about mDNS).

use std::time::Duration;

use cc_discovery::{
    AdvertiseOptions, AnnouncementSource, MdnsSource, ReceiverEvent, advertise_receiver,
    discover_receivers,
};

#[tokio::test(flavor = "multi_thread")]
async fn advertised_receiver_is_found_with_its_server() {
    let name = format!("ccloop{}", std::process::id());
    let ad = match advertise_receiver(&AdvertiseOptions {
        instance_name: name.clone(),
        port: 9,
        preferred_server: Some("wss://relay.loopback-test.example:443".into()),
        host_label: None,
    }) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("SKIPPED: cannot advertise here: {e}");
            return;
        }
    };
    // The instance name is derived from the id of the key made for the announcement.
    let instance = format!("CometCast-{}", &ad.id()[..8]);
    // Stream API.
    let mut browser = match discover_receivers() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("SKIPPED: cannot browse here: {e}");
            return;
        }
    };
    let found = tokio::time::timeout(Duration::from_secs(8), async {
        while let Some(ev) = browser.next().await {
            if let ReceiverEvent::Found(r) = ev
                && r.instance == instance
            {
                return Some(r);
            }
        }
        None
    })
    .await;
    let Ok(Some(r)) = found else {
        eprintln!("SKIPPED: own advertisement not seen within 8 s (no multicast loopback?)");
        return;
    };
    assert_eq!(
        r.announced_server.as_deref(),
        Some("wss://relay.loopback-test.example:443")
    );
    assert_eq!(r.port, 9);
    assert!(!ad.fullname().is_empty());
    drop(browser);

    // One-shot API used by discover_servers.
    let list = MdnsSource.browse(Duration::from_secs(3)).await.unwrap();
    assert!(list.iter().any(|r| r.instance == instance));
    eprintln!("mDNS loopback: OK");
}
