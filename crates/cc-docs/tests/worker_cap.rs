// SPDX-License-Identifier: GPL-3.0-or-later
//! The global cap on running worker processes (own test program: the cap is process-wide).

use std::time::{Duration, Instant};

use cc_docs::{
    DocError, Document, Isolation, Limits, WorkerSpec, active_workers, max_concurrent_workers,
    set_max_concurrent_workers,
};

fn data(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[test]
fn more_workers_than_the_cap_get_too_many_workers() {
    if !cfg!(unix) {
        println!("NOTE: skipped, needs `sleep`");
        return;
    }
    assert_eq!(max_concurrent_workers(), 4, "the default is 4");
    set_max_concurrent_workers(2);
    let sleeper = || Isolation::Process(WorkerSpec::new("sleep").arg("600"));
    let long = Limits {
        open_timeout: Duration::from_millis(1500),
        ..Limits::default()
    };
    let holders: Vec<_> = (0..2)
        .map(|_| {
            let l = long;
            std::thread::spawn(move || {
                Document::open_with_options(data("minimal.pdf"), l, sleeper()).unwrap_err()
            })
        })
        .collect();
    // Wait until both places are taken.
    let t = Instant::now();
    while active_workers() < 2 {
        assert!(
            t.elapsed() < Duration::from_secs(5),
            "workers did not start"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    // A third call waits at most its own timeout (300 ms), then gives the clear error, and
    // starts no process.
    let quick = Limits {
        open_timeout: Duration::from_millis(300),
        ..Limits::default()
    };
    let t = Instant::now();
    let e = Document::open_with_options(data("minimal.pdf"), quick, sleeper()).unwrap_err();
    assert_eq!(e, DocError::TooManyWorkers);
    assert!(
        t.elapsed() < Duration::from_millis(1000),
        "{:?}",
        t.elapsed()
    );
    assert_eq!(active_workers(), 2);
    for h in holders {
        assert_eq!(h.join().unwrap(), DocError::Timeout);
    }
    assert_eq!(active_workers(), 0, "places are given back");
    // With room again, a call goes through (to the error of the fake worker, not the cap).
    let e = Document::open_with_options(data("minimal.pdf"), quick, sleeper()).unwrap_err();
    assert_eq!(e, DocError::Timeout);
    set_max_concurrent_workers(4);
}
