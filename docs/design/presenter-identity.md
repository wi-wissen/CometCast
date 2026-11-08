<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Presenter device identity in join requests

Status: built (see "As built" at the end); it came after `signed-announcements.md` and `pause.md`.
Not in production yet: no compatibility needed beyond "a join without the new field still works".

## Problem

A join request shows the presenter's name only ("Julian's iPad"). Anybody can give their device
the same name. A host who has allowed "Julian's iPad" many times has no way to see that this
request comes from a different device.

## Goal

The display can tell, without any setup by anybody: **this name was used here before by a
different device**. It is shown as neutral information in the request dialog (and in the
"connected" toast in open mode), not as an alarm: in companies many devices carry the same name
("iPhone"), and every installation has its own key, so the note can be harmless.

## Design

### Presenter key

* Every presenter installation has an **Ed25519 key pair**, made on first use:
  * Flutter / native: the same mechanism as the display key of `signed-announcements.md`
    (32 secret bytes stored by the app with its settings, crypto in the Rust core). A device
    that is both display and presenter may use one key for both.
  * iOS broadcast extension: uses the key of the app through the App Group (it joins on the
    app's behalf; `cc-ios`).
  * Web: WebCrypto `Ed25519` (`crypto.subtle.generateKey({name:'Ed25519'}, false, ['sign'])`),
    the non-extractable `CryptoKeyPair` kept in IndexedDB. Browsers without WebCrypto Ed25519
    send no proof (the join works as today, with no device note). UNCERTAIN: browser support
    (Chrome 137+, Firefox 129+, Safari 17+) to be checked in the e2e browsers.
* **Device id** = lowercase hex of the first 16 bytes of SHA-256(public key), as for displays.

### Proof in `join_room`

New optional field `device` in `join_room`:

```json
{"type":"join_room","code":"K7QM2W","name":"Julian's iPad",
 "device":{"pk":"<base64url 32 bytes>","ts":1767225600000,"sig":"<base64url 64 bytes>"}}
```

* `sig` is Ed25519 over the bytes `cometcast-join-v1` 0x00 `<room code, upper case>` 0x00
  `<ts as decimal>`. The **name is not signed**: the server cleans names, and the proof only has
  to show "the holder of this key asked to join this room now".
* The **server does not verify** (it stays simple and keeps no device data). It checks only the
  shape (`pk` 43 and `sig` 86 base64url characters, `ts` a `u64`, nothing else in the object) and
  relays the object unchanged in `join_request` and `peer_joined` to the owner. A malformed
  `device` is dropped (the join goes on without it). cc-proto, cc-server, docs/protocol.md.
* The **display verifies**: signature valid for the current room code, `|ts − now| ≤ 120 s`.
  Invalid or missing proof → the request is shown as today, without any device note (never
  as "verified"). Valid → the request carries the device id.

### Memory on the display

* The display stores pairs `(cleaned name, device id)` for every request it **allowed** and every
  presenter that joined in open mode with a valid proof. At most 200 pairs, the least recently
  used dropped first, stored with the display's settings (Flutter settings, Apple TV
  `UserDefaults`, web display `localStorage` wrapped in try/catch like the existing setup).
* Names the server generates (`Guest N`, docs/protocol.md) are never stored or compared.
* A request (or open-mode join) with a valid proof whose cleaned name (case-insensitive) is
  stored **only with other device ids** gets the note. A name stored with this device id, or a
  name never seen, gets no note.

### UI

* Request dialog (web display, Flutter display, Apple TV), under the name, in the secondary
  text style with an info icon (not a warning colour):
  `This name was used here before by a different device.`
  `Dieser Name wurde hier schon von einem anderen Gerät benutzt.` (du and Sie identical)
* Open mode "connected" toast: the same sentence appended.
* Nothing else changes: allow / deny / block work as today.

### What it does not protect

* A host who never allowed the real device has nothing to compare.
* A new installation (data reset, new phone) is a new device; the note then appears once for a
  legitimate person. The wording is neutral for that reason.
* A malicious relay sees the proof and could replay it into the same room code within 120 s; the
  relay can already do worse (it carries the media path set-up), and the connection check code
  covers that case.

## Shared vectors

`tests/fixtures/join-proof.json`: fixed seed → pk, device id, signed bytes, sig for a room code
and ts; refused cases (wrong code, ts outside the window, bad sig, malformed shapes). Rust, web
(WebCrypto in vitest via Node's `crypto.webcrypto`, which supports Ed25519) and Swift read it.

## Acceptance

* Server tests: `device` relayed unchanged; malformed dropped; size bound.
* Core / web / Swift tests from the shared vectors.
* Display tests: note appears for a known name with another device, not for the same device,
  not for `Guest N`, not without a proof; memory bound.
* `cargo clippy --workspace --all-targets -- -D warnings`, `flutter analyze`, `flutter test`,
  web typecheck + vitest + i18n check.

## As built

| Part | Built |
| --- | --- |
| `cc-proto` | `JoinDevice {pk, ts, sig}` (`deny_unknown_fields`), `JoinDevice::is_well_formed` (43 / 86 canonical base64url characters, spare bits zero, no crate needed), optional `device` in `ClientMsg::JoinRoom`, `ServerMsg::PeerJoined` and `ServerMsg::JoinRequest`. The field is read **leniently** (`lenient_device`): a value that is not a well-formed device (wrong type, extra key, bad length or spelling, `ts` negative, fractional, a string or above `u64`) becomes `None` and the message still parses |
| `cc-server` | Passes `device` on in `join_request` and (open mode) `peer_joined`; nothing is stored. After an allow in `confirm` mode `peer_joined` has no `device` (the display keeps the request's). Tests in `crates/cc-server/tests/join_approval.rs`: relayed unchanged (both messages), a dozen malformed shapes dropped with the join going on, a 4000-character `sig` dropped (larger frames are closed by the 64 KiB frame limit as for every message) |
| Core crypto | `cc_transport::device` (not `cc-proto`, whose dependency list stays serde and thiserror for WebAssembly): `join_signed_bytes`, `sign_join`, `verify_join` (strict, `verify_strict`), `device_id`, `DeviceKey` (32 secret bytes, `Debug` shows the id only), `unix_ms`. `cc-discovery`'s `DisplayKey` was left as it is; the two id derivations are the same SHA-256 rule and the shared vectors check both |
| Vectors | `tests/fixtures/join-proof.json` by `tools/gen-join-proof-vectors.py` (Python `cryptography`): 2 keys, 7 valid, 26 refused (wrong code, ts 121 s away, ts changed, bad and foreign signatures, name or domain or lower-case code in the signed bytes, small-order key, S + L, malformed shapes). Read by Rust (`cc-transport/tests/join_proof_vectors.rs`, also byte for byte against the signatures), web (`joinProof.test.ts`, also signing from the seed through PKCS#8) and Swift (`JoinProofTests.swift`) |
| `cc-sender`, `cc-transport` | `join_room_with(.., device: Option<&DeviceKey>, ..)` signs the room code and the current time just before it sends; `SenderOptions`, `PresenterOptions` and `ContentSenderOptions` have `device_key` |
| `cc-ios` | `Config::device_key` (base64url secret), `cc_ios_start(..., device_key, ...)`, C ABI 4. The app puts the secret into `config.json` in the App Group (`BroadcastConfig.deviceKey`; the extension deletes the file after reading it, as for the hand-over token). The iOS **share** extension joins without a proof (not part of this work) |
| `cc-receiver` | The core verifies the proof of `join_request` and of an open-mode `peer_joined` for its own room code and clock; `ReceiverEvent::JoinRequest` and `PeerJoined` carry `device` and `other_device`; `DeviceMemory` (`device_memory.rs`: pairs, LRU, 200, `Guest N` skipped, `to_json` / `from_json`) lives in the core; `ReceiverEvent::DeviceMemoryChanged(json)` after every change; `ReceiverOptions::device_memory` is the start state. A pair of an **allowed** request is remembered when the server's `peer_joined` shows the presenter is in (not at the click, so a `room_full` refusal remembers nothing); a pair of a request let in by a hand-over token is remembered the same way |
| `cc-ffi` | `set_presenter_device_key(secret)` and `set_display_device_memory(json)` (sync; process-wide, the apps call them before they start a presenter or a receiver), `ReceiverUiEvent::DeviceMemoryChanged`, `device` and `other_device` (default `false` in Dart) on `PeerJoined` and `JoinRequest`. Bindings regenerated |
| `apps/web` | `core/deviceIdentity.ts` (WebCrypto Ed25519, non-extractable key pair in IndexedDB database `cometcast-device`, feature detection by trying `generateKey`; no proof where that fails; every storage access in try/catch; verification with WebCrypto plus explicit small-order and S < L checks), `core/deviceMemory.ts` (localStorage key `cometcast.deviceMemory`, loaded at every use), `SenderSession` dependency `deviceProof`, `ReceiverSession` dependencies `deviceMemory` and `verifyDevice`, `request.otherDevice` in `RequestDialog.vue` (info icon, `--mut` text) and in the open-mode "connected" toast |
| `apps/flutter` | `AppShell` loads the stored display key (or makes and stores it the same way) and gives it to the core (`setPresenterDeviceKey`) and to `PresenterRoot` for the iOS broadcast (`IosBroadcastRequest.deviceKey`, base64url); `DisplayStore.loadDeviceMemory` / `saveDeviceMemory` (`display.deviceMemory`, forgotten by reset); `RequestDialog` note with the design's info icon; `NoticeJoined.otherDevice` appended to the toast; ARB `requestOtherDevice` (en, de, sie) |
| `apps/apple-tv` | `JoinProof.swift` (parse, strict verify with small-order and S checks of its own, `DeviceMemory`), `ReceiverMachine` verifies at `join_request` and open-mode `peer_joined`, remembers on the presenter's arrival, `HostEvent.senderJoined(.., otherDevice:)`, `takeDeviceMemoryChange()`; `ConfigStore.deviceMemory` (UserDefaults), `JoinRequestView` note, `request.otherDevice` in both catalogs (`tools/gen-appletv-strings.mjs`). The Apple TV app is display only, so it has no presenter key. Not compiled here |
| `cometcast-cli` | Presents without a proof (it has no stored key); as a display it ignores `DeviceMemoryChanged` and keeps no memory |
| Docs | `docs/protocol.md` (fields and "Presenter device proof"), `docs/ios.md`, `tests/fixtures/README.md`, `docs/measurement-points.md` (A25) |

### Decisions

* **Where the memory lives.** One implementation per language, because the three displays are written in three languages: Rust (`cc-receiver`, used by the Flutter apps through the FFI), TypeScript (web) and Swift (Apple TV). The rules are the same word for word (case-insensitive name, ids of 32 hex digits, `Guest ` plus digits is never stored, 200 pairs, LRU by "allowed again").
* **When a request is "allowed".** Remembered when the presenter arrives (`peer_joined`), not at the click. A denied request is never remembered; the note for a later request therefore compares with devices the host really let in.
* **A name that is stored with this id as well as with another.** No note (the display has seen this device under this name).
* **Generated names.** The server's `Guest N` cannot be told from a person who typed exactly that text; both are treated as generated (never stored, never compared).
* **Verification on the web is asynchronous.** The request dialog opens at once and gets its note a few milliseconds later; the toast of an open-mode join is shown after the check. A proof that cannot be checked (no WebCrypto) is a request without a note.
* **`ts` range.** `u64` on the wire; JavaScript and Swift (JSON numbers as `Double`) accept 0 to 2^53 - 1, which is far beyond any real time. The vector for the largest value is 2^53 - 1 for that reason; Rust has its own unit test for `u64::MAX`.
* **Strict verification outside Rust.** The vector file contains a small-order key and a signature with S + L. Node's WebCrypto refused both without help; the web and Swift code check them explicitly anyway, because that is not documented behaviour of the libraries.
* **Key on the web.** The key pair is made on the first join, not at page load. If IndexedDB can read but not write, no proof is sent (a key that would be new on every visit is no identity). A browser that clears site data gets a new key: a new device for the display.
* **One key per installation.** Flutter apps: the display key slot of `DisplayStore.key()` serves both roles; the presenter never makes a second key. The web has two separate keys (display: none, it only verifies; presenter: IndexedDB).
* **Strings.** One sentence, du and Sie identical, so no row in the copy tables of `first-run-and-display.md` was needed.
* **Style.** The note is secondary text (`--mut` / `s.tokens.mut`, `p.mut`) with the design's `info` icon, no warning colour; the toast appends the sentence after a space.

### UNCERTAIN

* Support of WebCrypto `Ed25519` and persistence of a non-extractable key pair in IndexedDB in the browsers people use, and whether CryptoKit / swift-crypto `isValidSignature` agrees with the vectors: see A25 in [../measurement-points.md](../measurement-points.md). Rust and the web code (Node's WebCrypto) ran against the vectors; the Swift code and the Swift tests were never compiled.
* No browser end-to-end test was run (the vitest suites use Node's WebCrypto and an in-memory key store; IndexedDB itself is covered only by code reading).
* The iOS broadcast extension was not run on a device with the new `config.json` field; the Rust side is covered by the loopback test of `cc-ios` (the display sees the extension's device id).
* Keeping the key in app preferences (Flutter) rather than a keystore is the same open question as for the display key (A8).
