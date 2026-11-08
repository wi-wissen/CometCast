// SPDX-License-Identifier: GPL-3.0-or-later
//! A display announces itself with `DisplayAnnouncement` (mdns-sd) and the nearby list sees the
//! row, its code change and its goodbye on the same machine over real multicast DNS.
//!
//! Like `mdns_loopback.rs` it skips itself (with a note) where multicast is not available, so a
//! green run in such a sandbox proves nothing.

use std::time::Duration;

use cc_discovery::{DisplayAnnouncement, DisplayState, NearbyChange, NearbySearch, ScreenMode};

fn state(id: &str, code: &str, mode: ScreenMode) -> DisplayState {
    DisplayState {
        id: id.into(),
        room_name: "Loopback room".into(),
        server: "10.99.0.5:8443".into(),
        mode,
        code: Some(code.into()),
    }
}

async fn next_for(
    search: &mut NearbySearch,
    id: &str,
    secs: u64,
    pred: impl Fn(&NearbyChange) -> bool,
) -> Option<NearbyChange> {
    tokio::time::timeout(Duration::from_secs(secs), async {
        while let Some(c) = search.next().await {
            let mine = match &c {
                NearbyChange::Added(s) | NearbyChange::Updated(s) => s.id == id,
                NearbyChange::Removed(i) => i == id,
            };
            if mine && pred(&c) {
                return Some(c);
            }
        }
        None
    })
    .await
    .ok()
    .flatten()
}

#[tokio::test(flavor = "multi_thread")]
async fn announced_display_is_listed_updated_and_removed() {
    let id = format!(
        "{:016x}",
        u64::from(std::process::id()) * 7919 + 0xabc0_0000_0000
    );
    let mut search = match NearbySearch::with_mdns() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("SKIPPED: cannot browse here: {e}");
            return;
        }
    };
    let ann = DisplayAnnouncement::start_mdns();
    ann.set(Some(state(&id, "P4X9TR", ScreenMode::Open)));
    let Some(NearbyChange::Added(s)) = next_for(&mut search, &id, 10, |_| true).await else {
        eprintln!("SKIPPED: own announcement not seen within 10 s (no multicast loopback?)");
        return;
    };
    assert_eq!(s.code.as_deref(), Some("P4X9TR"));
    assert_eq!(s.name, "Loopback room");
    assert_eq!(s.server, "ws://10.99.0.5:8443");

    // A new code is seen as an update. The list takes other content for an id only once the
    // old content has not been announced for `CHANGE_HOLD` (15 s), so this takes 15 to 25 s here;
    // the spec's 1 s is the publishing side, and is tested with the fake backend.
    ann.set(Some(state(&id, "K7Q2MW", ScreenMode::Approval)));
    // (An update of the addresses alone may come first; wait for the one with the new code.)
    let upd = next_for(
        &mut search,
        &id,
        35,
        |c| matches!(c, NearbyChange::Updated(s) if s.code.as_deref() == Some("K7Q2MW")),
    )
    .await;
    match upd {
        Some(NearbyChange::Updated(s)) => {
            assert_eq!(s.code.as_deref(), Some("K7Q2MW"));
            assert_eq!(s.mode, ScreenMode::Approval);
        }
        other => panic!("no update seen: {other:?}"),
    }

    // Locked: the row stays, without a code (withdrawing the code is shown at once).
    ann.set(Some(state(&id, "K7Q2MW", ScreenMode::Locked)));
    let upd = next_for(
        &mut search,
        &id,
        10,
        |c| matches!(c, NearbyChange::Updated(s) if s.mode == ScreenMode::Locked),
    )
    .await;
    match upd {
        Some(NearbyChange::Updated(s)) => assert_eq!((s.mode, s.code), (ScreenMode::Locked, None)),
        other => panic!("no locked update seen: {other:?}"),
    }

    // Goodbye: the row leaves.
    ann.stop().await;
    let gone = next_for(&mut search, &id, 10, |c| {
        matches!(c, NearbyChange::Removed(_))
    })
    .await;
    assert!(gone.is_some(), "no removal after goodbye");
    eprintln!("nearby loopback: OK");
}

/// Slow (45 s): the row of a screen that keeps announcing without a change must not expire.
/// Run with `cargo test -p cc-discovery --test nearby_loopback -- --ignored`.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "takes 45 s"]
async fn refreshed_row_does_not_expire() {
    let id = format!(
        "{:016x}",
        u64::from(std::process::id()) * 104_729 + 0xdef0_0000_0000
    );
    let Ok(mut search) = NearbySearch::with_mdns() else {
        eprintln!("SKIPPED: cannot browse here");
        return;
    };
    let ann = DisplayAnnouncement::start_mdns();
    ann.set(Some(state(&id, "P4X9TR", ScreenMode::Open)));
    if next_for(&mut search, &id, 10, |_| true).await.is_none() {
        eprintln!("SKIPPED: own announcement not seen");
        return;
    }
    // 45 s without any change: a removal means the refresh was not seen as a refresh.
    let removed = next_for(&mut search, &id, 45, |c| {
        matches!(c, NearbyChange::Removed(_))
    })
    .await;
    assert!(
        removed.is_none(),
        "row expired although the display kept announcing"
    );
    ann.stop().await;
}
