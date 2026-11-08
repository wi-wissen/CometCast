<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Signed display announcements (layout v2)

Status: specification, to be built. Replaces TXT layout `v=1` of `docs/discovery.md` section 3.
Comet Cast is not in production yet, so there is **no compatibility** with v1: v1 records are
ignored by presenters after this change, and displays announce v2 only.

## Problem

Anybody on the local network can send DNS-SD records. Today (v1) the list of screens defends
itself with "first content wins" and a 15 s change hold, but:

* a forged mDNS goodbye (TTL 0) removes any row at once, and whoever announces the id next
  decides what the row shows;
* a device can flip a row to Locked;
* nothing ties an `id` to the device that first used it;
* a device can announce a second row with the same room name ("Room A") and its own relay and
  code. Presenters cannot tell which one is real.

## Design

### Identity

* Every display has an **Ed25519 key pair**, made on first use and stored with the app's
  settings (Flutter: next to the instance id in the settings store; Apple TV: Keychain, or
  `UserDefaults` if the Keychain is not available on tvOS for this use; iPad and computers: the
  Flutter path). A data reset makes a new key, as it makes a new id today.
* The **id is derived from the public key**: `id = lowercase hex of the first 16 bytes of
  SHA-256(public key)` (32 hex digits). An id therefore cannot be used by anybody without the
  private key. The 8 to 32 hex digit rule of v1 becomes "exactly 32".
* The DNS-SD instance name stays `CometCast-` + first 8 hex digits of the id. Rows are keyed by
  the **verified id only**; the instance name is never used to match or remove a row (a name
  conflict, renamed by mDNS to `... (2)`, does not matter).
* The **short tag** of a display is the first 4 characters of the id in upper case, shown as
  `ID 3F7A`. The display shows it small on its idle screen next to the code and in its About /
  Settings; the presenter list shows it under the room name of every row.

### Record (TXT, v=2)

Keys of v1 (`srv`, `tls`, `mode`, `code`, `name`) keep their meaning and checks. New or changed:

| Key | Meaning |
| --- | --- |
| `v` | `2` |
| `id` | 32 hex digits, = derived from `pk` |
| `pk` | public key, 32 bytes, base64url without padding (43 characters) |
| `seq` | decimal `u64`, strictly increasing per key for every record the display publishes. `seq = max(unix time in ms, last seq + 1)`; the display remembers the last value while it runs, and persists it with the key so a clock that goes back after a restart cannot reuse values |
| `state` | `live` or `gone` (see "Leaving") |
| `sig` | Ed25519 signature, 64 bytes, base64url without padding (86 characters) |

**Signed bytes:** the ASCII string `cometcast-announce-v2`, a 0x00 byte, then for every key
**except `sig`** in this fixed order `v, id, pk, seq, state, srv, tls, mode, code, name` the
bytes `key=value` followed by 0x00; an absent optional key (`tls`, `code`, `name`) is written as
`key=` with an empty value (so removing a key changes the signed bytes). Values are the exact
UTF-8 bytes of the TXT value. The record must also stay within `MAX_TXT_BYTES` (raise it to 512
if needed; a normal v2 record is about 260 bytes).

### Checks on the presenter side (`crates/cc-discovery`, every platform)

A record is accepted only if all hold, otherwise it is ignored (never a reason to change or
remove a row):

1. `v=2`, all required keys present, values well-formed (v1 checks plus the new ones);
2. `id` equals the id derived from `pk`;
3. `sig` verifies over the signed bytes with `pk` (strict Ed25519 verification, e.g.
   `ed25519-dalek` `verify_strict`, which is already in `Cargo.lock`);
4. `seq` is **greater** than the highest `seq` accepted for this id (kept while the row exists
   and for 10 minutes after it is gone, at most 256 ids remembered, oldest dropped first).

An accepted record replaces the row's content **at once** (the 15 s `CHANGE_HOLD` of v1 is no
longer needed: only the key owner can make a newer record). The rule "a row's server is fixed by
the first record" is dropped too: a signed newer record may change it.

### Leaving

* A display that stops announcing (End, background, offline, policy) first publishes a signed
  record with `state=gone` (a higher `seq`), then the usual goodbye. A presenter removes a row
  **only** on an accepted `state=gone` record or by expiry (30 s without an accepted refresh,
  `SCREEN_TTL`).
* An mDNS goodbye (TTL 0 / `Lost` / `NsdManager` lost / `NWBrowser` removed) on its own **never**
  removes a row. Android and Apple hand "lost" to the core today (`nearby_native_lost`); keep the
  call, but it only marks the row "unconfirmed" internally and has no visible effect.
* A replayed old `gone` record has a lower `seq` and is ignored.

### Same name, different id

* When two rows have the same room name (after cleaning, case-insensitive), both rows show the
  tag prominently and a note "Two screens use this name. Compare the ID with the screen."
* **Remembered screens (trust on first use):** when a presenter joins a room from a row, the app
  stores `(room name, id)` locally (at most 50 entries, oldest dropped). A later row with a stored
  name but a different id is marked "New device with a known name. Compare the ID with the
  screen." It stays tappable. The core keeps no list; the app passes the remembered pairs to the
  core (or marks rows itself, whichever fits the existing nearby API better), and it is covered
  by tests.

### What signing does not protect

* A device can still announce its **own** id with any name, relay and code. The tag, the
  same-name note, the remembered screens and the connection check code after joining are the
  defences; the docs must say so plainly.
* Records can be withheld (multicast blocked, flooding); then a row expires. Not preventable.

## Where it is built

| Part | Work |
| --- | --- |
| `crates/cc-discovery` | `display.rs`: key, id derivation, seq, signing, `state=gone` before goodbye; `nearby.rs`: checks above, seq memory, removal rules, same-name marking; `announce.rs` (older `advertise_receiver` record used by the server ladder): sign and check it the same way, or remove that record if nothing needs it any more (check `cometcast-cli discover --announce` and the server ladder) |
| `crates/cc-ffi` | API to create/load the key (the app stores 32 secret bytes; the core never writes files), pass it to the announcement, expose the tag, the same-name and remembered-screen marks on rows; regenerate bindings (flutter_rust_bridge_codegen 2.13.0, Flutter 3.47.9 at `/home/user/tools/flutter/bin`) |
| iPad announce path (`AnnounceBridge.swift`) | the core builds the signed record; Swift only publishes it (no crypto in Swift there) |
| `apps/flutter` | store the key with the settings (replacing the stored random id); show the tag on the display idle screen and About; show tag, same-name note and "new device" note in the nearby list; remember joined screens |
| `apps/apple-tv` | `Announcer.swift` / `displayAnnouncementTxt`: CryptoKit `Curve25519.Signing` (Ed25519) key in the Keychain, same id derivation, seq and signed bytes, `state=gone` before stopping; show the tag on the idle screen. Cannot be compiled here: keep it small and test it with the shared vectors |
| Shared vectors | new `tests/fixtures/announce-v2.json`: fixed seeds → public key, id, signed bytes, signature, TXT; and records that must be refused (bad sig, id mismatch, wrong key order, removed `code`, lower seq). Ed25519 is deterministic, so Rust and Swift must produce identical bytes |
| Docs | rewrite `docs/discovery.md` section 3 and "Nearby screens" for v2 (trust policy, what is and is not protected); `docs/design/first-run-and-display.md` (tag, notes); copy tables for new strings |

## Strings (en / de du / de Sie)

* `nearby.tag` ID {tag} / ID {tag} / ID {tag}
* `nearby.sameName` Two screens use this name. Compare the ID with the screen. / Zwei Bildschirme haben diesen Namen. Vergleiche die ID mit dem Bildschirm. / Zwei Bildschirme haben diesen Namen. Vergleichen Sie die ID mit dem Bildschirm.
* `nearby.newDevice` New device with a known name. Compare the ID with the screen. / Neues Gerät mit bekanntem Namen. Vergleiche die ID mit dem Bildschirm. / Neues Gerät mit bekanntem Namen. Vergleichen Sie die ID mit dem Bildschirm.
* `display.tag` ID {tag} (display idle screen and About)

## Acceptance

* Unit tests in `nearby.rs`: forged goodbye does nothing; forged record with the victim's id and
  another key is ignored; replay of an older signed record (live or gone) is ignored; a signed
  newer record (new code, new server, locked, gone) applies at once; expiry still works; same
  name marks both rows; remembered-screen mark.
* `tests/nearby_loopback.rs` over real multicast passes with v2.
* Shared vectors pass in Rust; the Swift tests read the same file.
* `cargo clippy --workspace --all-targets -- -D warnings`, `flutter analyze`, `flutter test`.
