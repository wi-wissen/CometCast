# Comet Cast patch of mdns-sd 0.21.7

Upstream: https://github.com/keepsimple1/mdns-sd (Apache-2.0 OR MIT, files LICENSE-APACHE and
LICENSE-MIT in this folder). The source is the published 0.21.7 crate. Changes:

* `src/service_info.rs`: `ServiceInfo::set_host_ttl` and `ServiceInfo::set_other_ttl` are public
  (upstream has the same setters as `pub(crate)` for its own tests).
* `Cargo.toml`: the `[[example]]` and `[[test]]` tables are removed (those files are not copied).

Why: a screen announcement must disappear from every list at most 35 s after the display loses
power without a goodbye (docs/design/first-run-and-display.md, 14.8 point 5, decision N50). The
platform browse APIs of Android and iOS answer from their cache until a record's lifetime has
passed, and mdns-sd announces with 120 s (SRV, addresses) and 4500 s (PTR, TXT). The display
(`crates/cc-discovery/src/announce.rs`) sets both to 30 s.

Remove this folder and the `[patch.crates-io]` entry of the root `Cargo.toml` when a released
mdns-sd offers the setters.
