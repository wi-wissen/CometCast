<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Presenter device identity in join requests

Status: specification, to be built after `signed-announcements.md` and `pause.md` are merged.
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
