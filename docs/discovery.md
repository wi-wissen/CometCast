# Finding the server (`cc-discovery`)

`cc-discovery` answers two questions: **which signalling server should this app use?** and
**which receivers are on this network?** The code is in `crates/cc-discovery`; the command
line tool shows the result with `cometcast-cli discover`.

Statements that are not verified are marked **UNCERTAIN**. The weak spots are collected in
[measurement-points.md](measurement-points.md), assumption A8 (discovery) and measurement
point 6 (network on site). Nothing here was tried against a real MDM product, a real managed
network, a real Android or Apple device.

## The ladder

The owner's rule is: the server search order is

1. **MDM policy**: a file that an administrator put on the device.
2. **DNS-SRV** `_cometcast._tcp` in the search domain of the network.
3. **Announcement by the display device**: the receiver says which server it prefers (mDNS; a signed v2 record, section 3).
4. **Built-in default list**: `relay.cometcast.de:443`.

Discovery of **receivers** over mDNS runs **in parallel** to this ladder; it is not a step of
it. `discover_servers()` asks DNS-SRV and the mDNS browse at the same time, and
`discover_receivers()` is an independent, endless stream of receivers.

`discover_servers()` returns the **ordered list** (best first, each entry with its source),
not just the first answer. The pre-fill of the server field is the first entry; the others
are fallbacks and suggestions. The pure function `merge()` builds the list:

| Policy | Result |
| --- | --- |
| none | DNS-SRV, then announcements, then the default list; each address once, at its best position |
| servers listed | those first, then the rest as above |
| `forbid_default_list` | the default list is left out |
| `forbid_announcement` | announced servers are left out |
| `forbid_user_override` | **only** the policy servers; the result is `locked` (the UI must not offer another server); the network is not even asked |
| file exists but is unusable | **fail closed**: no server, `locked`, a warning |

**UNCERTAIN:** failing closed on a broken policy file, and what `locked` means for the UI,
are decisions of this crate. The project owner has not confirmed them. The Flutter app
enforces `locked` (see "In the Flutter app" below).

**The default list is a placeholder.** `relay.cometcast.de` does not exist yet
([server.md](server.md)); it will be set up in the last step of the project. Until then a
build that has no other source offers a host that cannot be reached.

## 1. MDM policy

File locations (first existing one is used):

| System | Path | Format |
| --- | --- | --- |
| Linux | `/etc/cometcast/policy.json` | JSON |
| macOS | `/Library/Managed Preferences/de.cometcast.cometcast.plist` | property list, same keys |
| Windows | `C:\ProgramData\CometCast\policy.json` (fixed) | JSON |

```json
{
  "version": 1,
  "servers": ["wss://relay.example.org:443", "10.0.0.5:8443"],
  "forbid_default_list": true,
  "forbid_user_override": false,
  "forbid_announcement": true,
  "mode": "internal"
}
```

* All keys are optional. Unknown keys make the whole file invalid (a typo must not switch off
  a security setting without notice). `mode` (`open` or `internal`) is only a hint.
* Limits: file at most 64 KiB, at most 16 servers, each address at most 300 bytes. An
  address is `host:port`, or with `ws://`, `wss://`, `http://`, `https://`; no path, no user
  name. Admin entries may use loopback and any IP.
* On Unix the file must not be writable by group or others, and the system paths must be
  owned by root; otherwise the policy counts as unusable (fail closed). The same holds for
  the **directory that holds the file** (its direct parent): not writable by group or others,
  and owned by root for the system paths, because whoever can write the directory can replace
  the file. Parents further up are **not** checked (UNCERTAIN: a writable ancestor could
  rename the parent directory). The file is looked at with `lstat` first and opened with
  `O_NONBLOCK`; a FIFO, socket or device at the path is refused (fail closed) and cannot hang
  the start-up. Error messages never contain file content.
* **No environment variable and no setting moves the policy path.** In particular the Windows
  path is the fixed `C:\ProgramData`; the `ProgramData` variable is deliberately not read
  because a standard user can set it for their own processes. (UNCERTAIN: if an
  administrator moved the ProgramData folder, the policy is not found; the exact API,
  `SHGetKnownFolderPath`, needs `unsafe`, which this crate forbids.) The `--policy-file`
  option of `cometcast-cli discover` exists for development only; apps do not offer it.

**UNCERTAIN, not verified, not tested:**

* How Intune, Jamf or another MDM delivers such a file. Only reading a file from the paths
  above is implemented. The macOS file name assumes a configuration profile with the preference
  domain `de.cometcast.cometcast` (the bundle id of the Flutter app) ends up as
  `/Library/Managed Preferences/<domain>.plist`; this was not checked on a Mac.
* The Windows path is **compile-checked only** (`cargo check --target x86_64-pc-windows-gnu`),
  never run on Windows. There is **no** registry (GPO/Intune ADMX) reader and no ACL check on
  Windows: whoever can write the file can change the policy.
* Android managed configurations (`RestrictionsManager`) and Apple tvOS/iOS managed app
  configuration (`com.apple.configuration.managed`) are not read. Those need platform code in
  the apps.

## 2. DNS-SRV

For every search domain of the system (at most 4) the name `_cometcast._tcp.<domain>` is asked
for SRV records ([RFC 2782](https://www.rfc-editor.org/rfc/rfc2782)). The first domain with
usable answers wins. Records are ordered by priority; inside one priority by weighted random
choice, weight 0 first as the RFC describes (the random source can be injected, so tests are
deterministic). A single record with target `.` means "service not available here" and stops
the search.

* Library: `hickory-resolver` 0.26.3 (MIT OR Apache-2.0), features `system-config` and
  `tokio` only (no DNSSEC, DoT, DoH, DoQ). It reads the system DNS configuration
  (`/etc/resolv.conf`, the Windows registry, macOS system configuration) including search
  domains. One attempt per name server, 3 s per request (placeholder, not measured).
* DNS answers are untrusted: at most 32 records are looked at, at most 8 servers returned,
  targets must be valid host names (letters, digits, hyphens; no underscore, no IP literal in
  a public range, no `localhost`, loopback, unspecified, multicast, link-local incl. the
  cloud metadata address), port 0 is refused, duplicates are removed. Bad records are dropped
  with a count in the warnings. No panics.
* Time: each request is limited by the request timeout and **all search domains together**
  by twice that (a total budget); then the search stops with a warning.
* **TLS rule for network data** (see "Network servers" below): DNS cannot say whether a server
  speaks TLS, so the result carries a scheme: `ws://host:port` for a private IP literal or a
  `.local` name, `wss://host:port` for every other target.
* **UNCERTAIN, not covered:** Android's `getDomains` and iOS `lb._dns-sd._udp` browsing
  domains (RFC 6763 section 11). On those systems the hickory system configuration may be
  empty, in which case this step finds nothing. Split-DNS and VLAN effects are the subject of
  measurement point 6.
* The tests use a fake resolver. The real resolver is only checked to build or to report an
  error (the sandbox has no DNS server with a `_cometcast._tcp` record).

## 3. Announcement by the display device (layout v2, signed)

A display registers a DNS-SD service `_cometcast._tcp` on `local.`. Two things use it: the
**server ladder** (the `srv` field of an announcement is a server suggestion of the lowest
rank, see "Trust policy") and the **list of screens on this network** in the presenter apps
(`docs/design/first-run-and-display.md`, section 14). The design is
[design/signed-announcements.md](design/signed-announcements.md). The code is `crates/cc-discovery`:
`signed.rs` (key, id derivation, signed bytes, signature check, `seq` counter), `display.rs` (what a
display announces), `nearby.rs` (what a presenter app lists), `announce.rs` (the server ladder's
browse, and `advertise_receiver`, which announces a locked, signed record for
`cometcast-cli discover --announce`).

There is **no compatibility with layout v1** (Comet Cast is not in production). Records with
`v=1` are ignored by presenters and by the server ladder; displays announce v2 only.

**UNCERTAIN:** the TXT layout is a proposal of this crate. Nothing else speaks it. The web
pages cannot browse mDNS, so they do not read it.

### What a record looks like

| TXT key | Meaning | Checks |
| --- | --- | --- |
| `v` | layout version, `2` | required; anything else: ignored |
| `id` | the id of the display: lower-case hex of the first 16 bytes of SHA-256(public key), exactly 32 digits | required; must equal the id derived from `pk` |
| `pk` | the Ed25519 public key, 32 bytes, base64url without padding (43 characters) | required; canonical spelling only |
| `seq` | decimal `u64` (no sign, no leading zero), strictly increasing per key for every record the display publishes: `seq = max(unix time in ms, last seq + 1)`. The app persists the last value (plus a reserve) with the key, so a clock that goes back after a restart cannot reuse values | required; must be greater than the highest `seq` accepted for the id |
| `state` | `live`, or `gone` in the record that is published before the goodbye | required |
| `srv` | the server the display uses, `host:port` | required; checked like any announced server (below) |
| `tls` | `1`: the server speaks TLS (`wss://`) | written only when set; only `1` or empty is accepted |
| `mode` | who may join: `open`, `approval` or `locked` | required |
| `code` | the current 6-character room code (alphabet `ABCDEFGHJKLMNPQRSTUVWXYZ23456789`) | written for `open` and `approval` only; required there, else the whole record is dropped; ignored in a `locked` record |
| `name` | the room name as set in setup, cleaned like `room_name` (control, bidi, zero-width, invisible and private-use characters removed, white space collapsed), at most 32 characters. **No fallback** to the host or device name: none set means the key is absent | optional |
| `sig` | Ed25519 signature, 64 bytes, base64url without padding (86 characters) | required; verified with `verify_strict` |

**Signed bytes.** The ASCII string `cometcast-announce-v2`, a 0x00 byte, then for every key except `sig`, in
this fixed order `v, id, pk, seq, state, srv, tls, mode, code, name`, the bytes `key=value` followed by a
0x00 byte. An absent optional key (`tls`, `code`, `name`) is written as `key=` with an empty value, so removing a
key changes the signed bytes. Values are the exact UTF-8 bytes of the TXT value (values with a NUL byte or over 255
bytes are refused). Where a key occurs twice, the first occurrence is signed and read. Only the first 32 TXT entries
are looked at. `tests/fixtures/announce-v2.json` pins every byte (keys, ids, signed bytes, signatures, TXT, and the
records that must be refused); it was made with an independent implementation, and Rust and the Swift tests read it.

**Identity.** Every display has an Ed25519 key pair, made on first use from the operating system's randomness and
stored by the app with its settings (Flutter: next to the display setup in the preferences; Apple TV: Keychain, with
`UserDefaults` as a fallback). The core never writes a key: the app holds the 32 secret bytes and passes them to
`set_display_announcing`. A data reset makes a new key, so a new id. The DNS-SD **instance name** is `CometCast-` plus
the first 8 digits of the id, never a host or room name; rows are keyed by the **verified id only**, and the instance
name is never used to match or remove a row (a name conflict that mDNS renames to `... (2)` does not matter). The
service port is `9`; DNS-SD only needs one, displays do not listen for senders.

**The short tag** is the first 4 characters of the id in upper case, shown as `ID 3F7A`. It is deliberately
discreet: the display shows it small, in the secondary text colour, in the footer of its idle screen next to the
server line (not next to the code) and in About; the presenter list shows it **only on rows that carry a note**.

### What a display publishes, and when

* Everything is built **from the running receiver** (`cc-ffi`, `nearby.rs`), not from what the UI last said, so the
  announced code cannot differ from the shown one. A new code (End, restart, reconnect) means a new receiver and a
  new record.
* **Locked never announces a code** (and there is no token). A locked display keeps announcing name, server and
  `mode=locked`. The code is taken out of the record when the UI asks for Locked, without waiting for the server.
* A display announces only while it shows **Idle or Presenting with a code** and the app is in the foreground: not
  during setup, offline or connecting, not when the administrator's policy forbids announcements
  (`forbid_announcement` or `forbid_user_override`, checked again in the core on every start).
* **Timing.** A change (code, mode, name) is published at once (the limit of the spec is 1 s). The record is signed
  and published again every 10 s (`REFRESH_EVERY`), each time with a new `seq`; the presenter drops a row after 30 s
  without an accepted record (`SCREEN_TTL`).
* **Leaving.** A display that stops announcing (End, background, offline, policy, key change) first publishes a signed
  `state=gone` record (a higher `seq`), waits a moment (0.4 s; **UNCERTAIN**, a guess) and then sends the usual mDNS
  goodbye. A presenter removes a row **only** on an accepted `gone` record or by expiry. An mDNS goodbye on its own
  (TTL 0, `Lost`, `NsdManager` lost, `NWBrowser` removed) never removes a row: the platform bridges still call
  `nearby_native_gone`, but it only marks the row "unconfirmed" inside the core and has no visible effect.
* **Record lifetime (decision N50).** `mdns-sd` 0.21.7 announces with 120 s (SRV, address) and 4500 s (PTR, TXT)
  and has no public setter, and Android's `NsdManager` and Apple's `NWBrowser` answer from the system's cache until a
  record's lifetime has passed, so a display that dies without a goodbye would stay in a list for minutes. The
  repository therefore carries a patched copy, `vendor/mdns-sd` (the published crate plus two public setters, see
  `vendor/mdns-sd/COMETCAST-PATCH.md`, wired in by `[patch.crates-io]`), and every announcement sets both lifetimes to
  **30 s** (`RECORD_TTL_SECS`) (acceptance of spec 14.8 point 5: at most 35 s). The receiving side keeps its expiry as a
  second guard (a browse started again every 10 s on desktops). Checked on one machine over real multicast
  (`tests/nearby_loopback.rs`, including a 45 s run in which a refreshed row stays); **not** checked: a display that
  dies without any goodbye on a real network, Android's `NsdManager` and Apple's `NWBrowser` caches, and whether those
  caches pass a record that changes (`seq`) every 10 s on as a refresh (device test plans).
* **Size.** The record has at most 512 bytes (`MAX_TXT_BYTES`, DNS-SD lengths included); a normal record is about 260
  bytes. A server name so long that it would not fit is not announced. Nothing else is in it: no presenter name, no
  presenter data, no busy flag.
* **Why the code is announced.** The announced code is the same string the display shows large on its screen, to
  everybody in the room. Announcing it gives a device on the same network nothing that a person in the room does not
  see. What it enables (a list the person taps instead of typing) is the point of the feature.

**Who announces how (N50).** Computers and Android (TV, tablet, phone in the display role): the Rust core with
`mdns-sd`; Android holds a `WifiManager.MulticastLock` while it announces. iPad in the display role: the core builds
and **signs** the record and hands it to Swift (`watch_native_announcements`, `NativeAnnounceEvent`;
`AnnounceBridge.swift`), which only publishes it with `NWListener` (no crypto in Swift on this path; it cancels the
listener 0.4 s after the withdraw so the `gone` record is on the wire first); the multicast entitlement is not counted
on. Apple TV: its own Swift announcer (`Announcer.swift`, with the pure record builder `displayAnnouncementTxt` in
`CometCastCore`: CryptoKit `Curve25519.Signing`, same id derivation, `seq` and signed bytes, `state=gone` before
stopping), no room name unless the setup wizard sets one. macOS browses with `NWBrowser` through the iOS bridge; Linux
and Windows browse in the core. **UNCERTAIN:** the Swift code was never compiled or run; whether CryptoKit's Ed25519
signature is deterministic like Rust's (the Swift tests compare bytes only if it is), and whether `isValidSignature` is
as strict as `verify_strict`, is not known (A8).

### What signing protects, and what it does not

Protected (a record that does not pass the four checks of "Nearby screens" below changes nothing):

* A device cannot remove a row with a forged goodbye or a forged `gone` record: only the key owner can sign.
* A device cannot take over the id, the row, the code, the server or the name of another display: the id derives from
  the public key, so a record with the victim's id needs the victim's key.
* A replayed old record (live or gone) has a lower or equal `seq` and is ignored while the id is remembered.
* A device cannot make a row Locked or point it elsewhere.

**Not protected, and the docs say so plainly:**

* A device can still announce its **own** id with **any name, relay and code**. It can use the name "Room A", its own
  relay and its own code; presenters cannot tell by signature alone which "Room A" is the real one. The defences are the
  short tag, the same-name note, the remembered screens (trust on first use) and the connection check code after
  joining; they depend on a person comparing the ID with the screen. A relay that is not trusted still needs the
  confirmation page.
* Records can be **withheld** (multicast blocked, flooding, a jammed network); then a row expires after 30 s. Not
  preventable.
* A record can be **replayed** to a presenter that has no `seq` memory for the id (the app was just started, or the
  display has been gone for over 10 minutes, or 256 other ids pushed it out): the row then shows the old content for up
  to 30 s, until the display's next record (a higher `seq`) replaces it or the row expires. `seq` is a time in
  milliseconds, but presenters do not compare it with their own clock (clocks on these devices are not trusted;
  UNCERTAIN, a window check could be added). A replayed old `gone` record is harmless. The server ladder (below) has no
  `seq` memory at all.
* Whoever floods the network with valid records under many keys can fill the 32 rows ("the first 32 found stay") or push
  ids out of the `seq` memory. Not preventable on a network without port security.
* The key lives in the app's preferences on phones, tablets and computers (UNCERTAIN: not a hardware-backed keystore;
  someone with the app's data has the key and so the identity) and in the Keychain on Apple TV (UNCERTAIN: not tried).

### Trust policy

Anybody on the local network can send such a record, and the server a sender is pointed to sees room codes and the
addresses of the devices. Therefore:

* An announced server has the **lowest rank** after MDM and DNS-SRV, and above only the default list. In the list of
  screens it is **never trusted because it was announced** (even if signed): only the default relay, a policy server, a
  DNS-SRV server and a server the person confirmed on this device are joined at once. The server ladder's browse
  (`announce.rs`, and `announcedReceiver` on the Apple TV) uses a record only if it verifies (`v=2`, id from `pk`,
  signature) and says `live`; the first announcement seen for an instance name is kept and **its server is never
  replaced** by a later one (`record_receiver`); browses keep at most 64 receivers; the command line tool drops an entry
  on `Lost`.
* An MDM policy can switch announcements off (`forbid_announcement`), and `forbid_user_override` ignores them as well.
* The `srv` value is checked like DNS data: no scheme, path or user name; host name or an IP literal. An IP literal must
  be in a **private range** (RFC 1918, `100.64.0.0/10`, unique-local IPv6 `fc00::/7`); a public server has to be given by
  name. Loopback, unspecified, multicast, broadcast, IPv4-mapped, **link-local IPv4 (`169.254.0.0/16`)** and link-local
  IPv6 (needs a zone id) are refused, as are the cloud metadata addresses (`169.254.169.254` is covered by the
  link-local rule, plus `100.100.100.200` and `fd00:ec2::254`). UNCERTAIN: whether this is too strict for real sites is
  not known.
* Host names are checked so that no spelling of an IPv4 address gets through as a "name": the last label must contain a
  letter and must not start with `0x`. So `0x7f000001`, `0x7f.0x1`, `1.0x1`, `0177.0.0.1`, `127.1` and `2130706433`,
  which `inet_aton` and URL parsers read as IPv4, are refused for every source, MDM policy included.
* Names are cleaned (control characters, Unicode format characters such as bidi overrides and zero-width characters,
  and line/paragraph separators are removed), at most 8 addresses and 64 receivers are kept per browse.
* The announcement only feeds a *list*. The app shows which server is used and where it came from (`ServerSource`) and
  does not switch silently once the person has chosen. A valid signature is **not** a statement that the server or the
  code is right.

`cometcast-cli receive` does not announce. `cometcast-cli discover --announce HOST:PORT` advertises, for the length of
the command, a **locked** display record (no code) that names that server, signed with a key made for that run; its
goodbye is a signed `gone` record first.

Backends: the display announcement and the browse of the desktop apps use `mdns-sd` (`DisplayAnnouncement::start_mdns`,
`NearbySearch::with_mdns`). Android browses with `NsdManager` and iOS / iPadOS with `NWBrowser` in Kotlin and Swift
(`NearbyBridge`), which only hand the raw TXT records to the Rust core (`nearby_native_seen` ...); the core verifies and
keeps the list on every platform, so no platform has its own check. **Not built:** the announcement of a display with
`NsdManager.registerService` (Android TV and tablets): displays on all platforms that run the Flutter app announce
through the core's `mdns-sd`, which on iOS needs the multicast entitlement ([ios.md](ios.md)) and probably fails without
it (the status `AnnounceStatus` is then still `Announcing`; the failure is silent). Multicast can be blocked by switches,
access points or VLANs; then screens do not show up and the room code or QR code is the way
([measurement-points.md](measurement-points.md), A8).

## Nearby screens (the list in the presenter apps)

`crates/cc-discovery/src/nearby.rs`; the API for Flutter is `watch_nearby_screens`, `set_remembered_screens` and the
`nearby_native_*` calls of `cc-ffi` ([flutter-bridge.md](flutter-bridge.md)). Rules (spec 14.3, 14.8,
signed-announcements.md):

* **Four checks, all in Rust, on every platform.** A record is accepted only if (1) `v=2`, all required keys are
  present and every value is well-formed (`srv` passes `network_server`; `mode` is one of the three; for `open` and
  `approval` the code is exactly 6 characters of the code alphabet, otherwise the whole record is dropped; a code in a
  `locked` record is ignored; the name is cleaned and cut; values over 255 bytes or with a NUL byte are refused; only
  the first 32 TXT entries are read, the first occurrence of a key wins); (2) `id` equals the id derived from `pk`;
  (3) `sig` verifies over the signed bytes with `pk` (strict Ed25519); (4) `seq` is **greater** than the highest `seq`
  accepted for this id. Otherwise the record is **ignored**: it never changes or removes a row, and it does not refresh
  one.
* **`seq` memory.** The highest accepted `seq` per id is kept while the row exists and for **10 minutes** after it is
  gone (a signed goodbye or expiry), at most **256** ids, the oldest dropped first (an id with a row is dropped only if
  there is no other). A record with the same `seq` as the one accepted (a cache that reports the same record again) is
  not newer and does not keep a row alive. A search that is stopped or restarted forgets everything.
* **Rows are keyed by the verified id only**, in the order they were first found (new ones at the end, never re-sorted).
  The instance name, the id as text and the A/AAAA records prove nothing and are never used to match or remove a row.
  The same screen under two instance names (two interfaces, or a name that mDNS renamed to `... (2)`) is one row; its
  addresses are the union (they only hide TV rows of the same device, N105).
* **An accepted record replaces the row's content at once** (server, code, mode, name): only the key owner can make a
  newer record. The 15 s `CHANGE_HOLD` of layout v1 and the rule "a row's server is fixed by the first record" are gone.
* **A row leaves** only on an accepted `state=gone` record or when no record was accepted for 30 s (`SCREEN_TTL`). A
  goodbye from the platform (`nearby_native_gone`) only marks the row unconfirmed inside the core and changes nothing
  that is shown. A replayed old `gone` record has a lower `seq` and is ignored.
* **Same name, different id.** When two rows have the same name (cleaned, compared case-insensitively; empty names never
  match), both rows get `same_name`; the app shows `nearby.sameName` and the tag `nearby.tag` under both. **Remembered
  screens (trust on first use):** when a join from a row succeeds, the app stores `(room name, id)` locally (at most 50,
  oldest dropped) and passes the pairs to the core with `set_remembered_screens` (before and while a search runs); the
  core keeps no list of its own and writes nothing. A row whose name is remembered but whose id is not gets `new_device`;
  the app shows `nearby.newDevice` and the tag, and the row stays tappable. Rows without a note show no tag.
* At most 32 rows: the **first 32 found stay**; a 33rd and later screen is neither reported nor stored until a row
  leaves, then it is added at the end with its next record (spec 14.8 point 10, decision N49).
* **What is not protected** is described under "What signing protects, and what it does not" above: a device can
  announce its own id with any name, relay and code; records can be withheld or replayed to a presenter with no memory.
  The row always shows the code, so the person can compare it with the screen, and a server that is not trusted still
  needs the relay confirmation (14.4).
* Nothing is picked, remembered (apart from the app's remembered screens) or saved here. In the Flutter app a row is
  tapped by the person; the announced server is used for that one join and is **not** saved as "last used" (a server the
  person confirmed is saved in the list of confirmed servers only).
* **Policy.** With `forbid_announcement` or `forbid_user_override` (or an unusable policy file) the section is not shown,
  no search runs (`watch_nearby_screens` answers `Forbidden`) and a display does not announce (`set_display_announcing`
  answers `ForbiddenByPolicy`).
* **Permission and network.** `NearbyPermission` (`notAsked`, `granted`, `denied`) comes from the native bridge (Apple:
  `kDNSServiceErr_PolicyDenied` means `denied`). `local_network_available()` (a private, non-loopback interface) is used
  on computers; phones and tablets ask `ConnectivityManager` / `NWPathMonitor`.

### The API for the apps (`cc-ffi`)

* `new_display_secret()` (sync): 32 random bytes from the operating system; the app stores them.
* `display_identity(secret)` (sync): id, tag and public key of a key (nothing is announced).
* `set_display_announcing(active, secret_key, seq_floor)`: announce (or not); `seq_floor` is the persisted value.
* `display_seq_to_persist()` (sync): the last `seq` plus a reserve of 120 000; the Flutter display stores it after
  every status change and every minute.
* `set_remembered_screens(screens)` (sync): the app's remembered `(name, id)` pairs.
* `NearbyScreenInfo` carries `id`, `tag`, `same_name` and `new_device` besides the old fields.

## Network servers: TLS and the local network

Servers from DNS-SRV and from announcements are not trusted just because they are "on the
LAN". Rule (`address::network_server`): **plain `ws://` is only accepted inside the local
network**, that is for a private IP literal or a name ending in `.local`. Every other host is
only usable with TLS (`wss://`):

* An announcement whose host is outside the local network needs `tls=1` in the TXT record;
  without it the announcement is dropped. With `tls=1` it is honoured for every host
  (`wss://`), including private literals.
* A DNS-SRV target outside the local network becomes `wss://host:port`; a private literal or
  `.local` target becomes `ws://host:port`.
* The result entries (`DiscoveredServer`) carry the scheme in `address` and a `tls` flag
  (`Some(true)`/`Some(false)`; `None` only for policy and default entries without scheme).
  The UI should show a plain `ws` entry as "unencrypted, local network".

**Not checked:** a host *name* that resolves to a private or loopback address (DNS rebinding,
names like `localtest.me`, or a `.local` name that someone on the LAN answers). This crate does
no name resolution for the check. The transport and the app must not rely on the name alone.
TLS also does not prove that the server is the *right* one: any host with a valid certificate
passes. That is why the UI must show the server and its source.

## 4. The built-in list

`DEFAULT_SERVERS = ["relay.cometcast.de:443"]`, marked `DefaultList`, with a detail text that
says the host does not exist yet.

## API

```rust
let report = cc_discovery::discover_servers(&DiscoverOptions::default()).await;
for s in &report.servers { println!("{} [{}]", s.address, s.source.as_str()); }
let mut receivers = cc_discovery::discover_receivers()?;       // parallel stream
while let Some(ReceiverEvent::Found(r)) = receivers.next().await { /* r.announced_server ... */ }
```

* `discover_servers_offline()`: policy file and default list only, no network; fast.
* `discover_servers_with()`: same as `discover_servers()` with injected DNS and mDNS access.
* Nothing fails as a whole: a source that does not work leaves a warning (no network content
  in it).
* `cc-ffi`: `server_defaults()` (sync) uses the offline variant, so the pre-fill order is
  "policy, then default list" and the development fallback stays last. `discover_servers()`
  (async) asks everything and returns `ServerCandidate { address, source }`, `locked` and
  warnings (English, for logs) and `notices`: a code per kind of warning that the UI
  translates. Also `start_advertising(name, preferred_server)` (the server is required: the announced record is signed and
  names a server; it is a locked display record with a key made for the announcement) / `stop_advertising()` and
  `watch_receivers()` / `stop_watching_receivers()`, see below.

## In the Flutter app

Both screens (receive, send) show the server field with these rules
(`apps/flutter/lib/settings.dart`, `widgets.dart`, `discovery.dart`):

* **Pre-fill.** At once, from the offline answer (`server_defaults()`); a moment later the
  network answer (`discover_servers()`) replaces the list and, if the person has not typed or
  tapped anything, the pre-fill. The order of the list is the owner's: policy servers, DNS-SRV,
  announcements, built-in list. **The pre-fill is a different rule: it never takes an
  announced server** (any device in the local network can announce one, and the room code
  would be sent to it). It is the first policy server; without one, the server used last
  time (saved with `shared_preferences`); otherwise the first entry that is not an
  announcement (DNS-SRV, built-in list, development server). Announced servers are chips that
  the person has to tap. If only announced servers exist, the field gets `127.0.0.1:8443`. The saved
  server is shown as a chip "Last used" right after the policy servers; `127.0.0.1:8443` stays
  last ("This computer (development)"). UNCERTAIN: putting the saved server before DNS-SRV
  results is a product decision that the owner has not confirmed.
* **Suggestion chips** carry a translated label for the source: set by your administrator
  (`mdm`), found in the network (DNS) (`dns-srv`), announced by a display
  (`display-announcement`), default list (`default-list`). An unknown source name gets "Other
  source" and, like an announcement, is never pre-filled. The announced label reads "Announced
  by a display on this network, not verified". Whenever the value of the field was taken from
  a chip or a nearby receiver, a line next to the field names its source ("Source of this
  address: ..."); typing removes it.
* **Last used.** A server whose source is an announcement is not saved as "last used", so an
  unverified announcement cannot become the trusted pre-fill of the next start. Servers from a
  policy, DNS-SRV, the built-in list or typed by the person are saved (not under a lock).
* **`locked`.** The field is read-only (it can still be selected), a line says "Your
  administrator has set the server.", only the policy servers are chips, the saved server, the
  development override `COMETCAST_SERVER` and the development server are ignored, the server
  is not saved, the model refuses other values. UNCERTAIN: with several policy servers the
  person may still choose between them (chips); the owner has not said whether that is wanted.
  An unusable policy file locks with an empty list: the start button then asks for a server
  that cannot be given; the note "The policy file of your administrator could not be read, so
  no network server is offered." explains it.
* **Notes.** The Rust side maps each warning text to a code (`DiscoveryWarning`); the UI shows
  one translated, calm sentence per code and never the raw text (it may contain network
  content). UNCERTAIN: `cc-discovery` has no codes, so the mapping matches the beginning of its
  English texts; a changed text becomes the general note "Not every source of servers could be
  asked." The unit tests in `cc-ffi` build real warnings to notice such a change.
* **Progress.** While the search runs a still hint "Looking for servers in the network …" is
  shown (no spinner: it would keep `pumpAndSettle` and screen readers busy). Nothing is
  blocked. The time limits (DNS 3 s, announcements 1.5 s) are those of
  `DiscoverOptions::default()`; Dart has no timer of its own. A Rust future cannot be
  cancelled from Dart: closing the screen makes the answer harmless (`cancelDiscovery`); the
  search ends by itself within its Rust time limits.
* **Receiver and send views (old).** `start_advertising`, `stop_advertising`, `watch_receivers` and
  the `NearbyReceivers` class (a "Nearby receivers" list that took over an announced server and
  moved the focus to the code field) are **no longer called by the UI** since the designed pages
  (docs/design/native-apps.md); they remain in `cc-ffi` and in `lib/discovery.dart`. The designed
  replacement is the list of "screens on this network" (section "Nearby screens" above):
  the display announces through `set_display_announcing`, the presenter start page lists
  through `watch_nearby_screens`. A tap on a row **does connect** (with the announced code and the
  name from the field), where the old list only took over a server.

`address` carries the scheme for network servers (`ws://` or `wss://`); the `tls` flag of
`DiscoveredServer` is not passed through `cc-ffi` yet.

## Minimum Rust version

The workspace `rust-version` is **1.92** (raised for `hayro`; `hickory-resolver` 0.26 needs 1.88, and the code
uses let-chains, stable since 1.88). UNCERTAIN: CI uses the stable toolchain, so no job builds
with exactly 1.92.

## Tests

* Unit tests: address validation with hostile input; policy files in temp directories (size,
  typos, permissions, directory instead of file, plist); RFC 2782 ordering with a fixed random
  source and a distribution check; fake resolver (timeouts, errors, `.` target, hostile
  records, hostile search domains); TXT parsing; exhaustive `merge()` cases (each source
  alone, every flag, all flags, duplicates, fail closed).
* `tests/mdns_loopback.rs`: advertises a receiver and finds it again with real multicast DNS
  on the same machine. It passed in the development sandbox. It skips itself with a note
  when multicast is unavailable, so a green run in such an environment proves nothing about
  mDNS. It does not prove behaviour on a network with several devices.
* `signed.rs` unit tests: id derivation, the layout of the signed bytes, every changed or removed field breaks the
  signature, id from another key, wrong key order, malformed values, first key wins, the `seq` counter, keys.
* `display.rs` unit tests with a fake announce backend and a fixed clock: signed TXT building (Locked never carries a
  code, bounds, hostile server, the order of keys), changes published at once, refresh every 10 s with a higher `seq`
  every time, the persisted `seq` floor against a clock that went back, the signed `gone` record before the goodbye,
  retry after a failed publish.
* `nearby.rs` unit tests (the acceptance list of the design): a forged goodbye does nothing; a forged `gone` record, a
  record with the victim's id and another key, and a tampered record are ignored and do not refresh a row; a replay of
  an older signed record (live or gone) is ignored; a signed newer record (new code, new server, locked, renamed, gone)
  applies at once; the same `seq` again does not refresh; expiry after 30 s; the `seq` memory (10 minutes, 256 ids);
  rows keyed by the verified id, not by the instance name; the 32-row limit; same name marks both rows; the
  remembered-screen mark; layout v1 is ignored.
* `tests/announce_v2_vectors.rs`: the shared vectors `tests/fixtures/announce-v2.json` (made by an independent
  implementation): ids, tags, signed bytes, signatures, the refused records with their reasons, a `seq` sequence at the
  presenter, the `seq` counter. The Swift tests of the Apple TV core read the same file (not run here).
* `tests/nearby_loopback.rs`: a display announces with `mdns-sd`, the list sees the row, a new code and mode at once,
  Locked without a code, and the signed goodbye (real multicast, skips itself where there is none);
  `refreshed_row_does_not_expire` (45 s, run with `-- --ignored`: the record changes its `seq` every 10 s and the row
  stays). `cc-ffi` tests cover the key API and the rows' tag and marks; the Flutter tests cover the store of the key
  and `seq`, the tag on the idle screen, the notes and remembered screens in the list, and the real core through
  `rust_core_test.dart` (needs `COMETCAST_FFI_LIB`).
* Not tested: a real DNS server with SRV records, a real MDM, Windows, macOS, Android, iOS, tvOS (the Swift code was
  never compiled), a real network with several devices.
