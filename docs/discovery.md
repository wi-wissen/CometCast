# Finding the server (`cc-discovery`)

`cc-discovery` answers two questions: **which signalling server should this app use?** and
**which receivers are on this network?** The code is in `crates/cc-discovery`; the command
line tool shows the result with `cometcast-cli discover`.

Statements that are not verified are marked **UNCERTAIN**. The weak spots are collected in
[measurement-points.md](measurement-points.md), assumption A8 (discovery) and measurement
point 6 (network on site). Nothing here was tried against a real MDM product, a real school
network, a real Android or Apple device.

## The ladder

The owner's rule is: the server search order is

1. **MDM policy**: a file that an administrator put on the device.
2. **DNS-SRV** `_cometcast._tcp` in the search domain of the network.
3. **Announcement by the display device**: the receiver says which server it prefers (mDNS).
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

## 3. Announcement by the display device

A receiver registers a DNS-SD service `_cometcast._tcp` on `local.` (`advertise_receiver`).
**UNCERTAIN:** the TXT layout is a proposal of this crate. Nothing else speaks it.

| TXT key | Meaning |
| --- | --- |
| `v` | layout version, `1` (a different version: the announcement is ignored) |
| `srv` | preferred server, `host:port` |
| `tls` | `1`: the server speaks TLS, the sender uses `wss://` |
| `name` | friendly name (at most 48 characters, control characters removed) |

The room code is **not** announced. The service port is only there because DNS-SD needs one;
receivers do not listen for senders yet.

**Trust policy.** Anybody on the local network can send such a record, and the server a
sender is pointed to sees room codes and the addresses of the devices. Therefore:

* An announced server has the **lowest rank** after MDM and DNS-SRV, and above only the
  default list. Receivers with the same DNS-SD instance name: the **first one seen is kept**;
  a later announcement replaces it only if it comes from the same addresses and port (the
  same device updating its record). Otherwise a second device could take over the entry.
  Browses keep at most 64 receivers; the command line tool also drops an entry on `Lost`.
* An MDM policy can switch announcements off (`forbid_announcement`), and
  `forbid_user_override` ignores them as well.
* The `srv` value is checked like DNS data: no scheme, path or user name; host name or an IP
  literal. An IP literal must be in a **private range** (RFC 1918, `100.64.0.0/10`, unique-local
  IPv6 `fc00::/7`); a public server has to be given by name. Loopback, unspecified, multicast,
  broadcast, IPv4-mapped, **link-local IPv4 (`169.254.0.0/16`)** and link-local IPv6 (needs a
  zone id) are refused, as are the cloud metadata addresses (`169.254.169.254` is covered by
  the link-local rule, plus `100.100.100.200` and `fd00:ec2::254`). UNCERTAIN: whether this is
  too strict for real sites is not known.
* Host names are checked so that no spelling of an IPv4 address gets through as a "name": the
  last label must contain a letter and must not start with `0x`. So `0x7f000001`, `0x7f.0x1`,
  `1.0x1`, `0177.0.0.1`, `127.1` and `2130706433`, which `inet_aton` and URL parsers read as
  IPv4, are refused for every source, MDM policy included.
* Only the first 32 TXT entries are read, names are cleaned (control characters, Unicode
  format characters such as bidi overrides and zero-width characters, and line/paragraph
  separators are removed), at most 8 addresses and 64 receivers are kept per browse.
* `advertise_receiver` applies the same checks to the server it is told to announce, so the
  mistake shows at the receiver.
* The announcement only feeds a *list*. The app shows which server is used and where it
  came from (`ServerSource`) and does not switch silently once the person has chosen.

The Flutter receiver view calls `advertise_receiver` while it is open (through `cc-ffi`
`start_advertising`); `cometcast-cli receive` does not. `cometcast-cli discover --announce
HOST:PORT` advertises for the length of the command.

Backends: this is the `mdns-sd` backend (desktop). NWBrowser (Apple) and NsdManager (Android)
are not implemented ([architecture.md](architecture.md)). Multicast can be blocked by
switches, access points or VLANs; then receivers do not show up and the room code or QR code
is the way ([measurement-points.md](measurement-points.md), A8).

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
  translates. Also `start_advertising(name, preferred_server)` / `stop_advertising()` and
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
* **Receiver view.** While open it announces itself with `start_advertising(name,
  preferred_server)`: the name is the host name of the device (UNCERTAIN: visible to everybody
  in the network; a chosen or random name might be better) and the server in the field (left
  out if `cc-discovery` refuses it, for example `127.0.0.1`). Starting to receive announces the
  server really used. Closing the view stops it. Nothing is announced when the policy
  forbids announcements or locks the server (and a ban learned late stops a running
  announcement). Starting and stopping the announcement, and the listening, are queued in
  Dart (first in, first out), so a stop never overtakes a start that is still running and a
  closed view never starts the fallback announcement. UNCERTAIN: the Rust core keeps one global announcement and one global watch; if a
  second screen starts before the first one is disposed, the late stop of the first still ends
  the second's. A failure (no multicast) shows nothing.
* **Send view.** "Nearby receivers" lists what `watch_receivers()` finds (name and announced
  server, each marked "Announced by a display on this network, not verified"). A tap takes
  over the announced server (the field then names the source) and moves the focus to the room
  code field;
  the person types the code, because **an announcement never contains the room code**. The
  list is not shown, and mDNS is not even started, when the policy locks the server or
  forbids announcements, also when that is only learned from the late network answer (the
  listening then stops). At most 32 receivers are kept; when more are announced the oldest
  entry makes room for the newest and the list says "and more ...". UNCERTAIN: the list is only as
  trustworthy as the local network; the person sees the server before sending anything.
* **Not done:** the receiver does not listen for senders (the mDNS port is a placeholder),
  nothing is picked automatically from announcements (only a tap selects an announced
  server; the pre-fill comes from the policy, DNS-SRV, the saved server or the built-in
  list), no QR code, no Android or Apple discovery
  backends (the Dart side would work unchanged once Rust has them).

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
* Not tested: a real DNS server with SRV records, a real MDM, Windows, macOS, Android, iOS.
