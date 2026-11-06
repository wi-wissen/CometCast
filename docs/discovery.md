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
are decisions of this crate. The project owner has not confirmed them. The Flutter app does
not enforce `locked` yet: it pre-fills the first server but still lets people type another
one and still appends the development fallback `127.0.0.1:8443`.

**The default list is a placeholder.** `relay.cometcast.de` does not exist yet
([server.md](server.md)); it will be set up in the last step of the project. Until then a
build that has no other source offers a host that cannot be reached.

## 1. MDM policy

File locations (first existing one is used):

| System | Path | Format |
| --- | --- | --- |
| Linux | `/etc/cometcast/policy.json` | JSON |
| macOS | `/Library/Managed Preferences/de.cometcast.cometcast.plist` | property list, same keys |
| Windows | `%ProgramData%\CometCast\policy.json` | JSON |

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
  owned by root; otherwise the policy counts as unusable (fail closed). Error messages never
  contain file content.
* There is no environment variable or setting that moves the policy path: a user could
  otherwise point it away.

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
  a public range, no `localhost`, loopback, unspecified, multicast), port 0 is refused,
  duplicates are removed. Bad records are dropped with a count in the warnings. No panics.
* The `DnsSrv` result is plain `host:port` (no scheme), like the default list. **UNCERTAIN:**
  nothing in DNS says whether the server speaks TLS; how the app decides between `ws://` and
  `wss://` for such an address is outside this crate.
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
  default list.
* An MDM policy can switch announcements off (`forbid_announcement`), and
  `forbid_user_override` ignores them as well.
* The `srv` value is checked like DNS data: no scheme, path or user name; host name or an IP
  literal. An IP literal must be in a **private range** (RFC 1918, `100.64.0.0/10`,
  link-local IPv4, unique-local IPv6 `fc00::/7`); a public server has to be given by name.
  Loopback, unspecified, multicast, broadcast, IPv4-mapped and link-local IPv6 (needs a zone
  id) are refused. UNCERTAIN: whether this is too strict for real sites is not known.
* Only the first 32 TXT entries are read, names are cleaned, at most 8 addresses and 64
  receivers are kept per browse.
* `advertise_receiver` applies the same checks to the server it is told to announce, so the
  mistake shows at the receiver.
* The announcement only feeds a *list*. The app must show which server is used and where it
  came from (`ServerSource`); it must not switch silently. UNCERTAIN: the Flutter UI does not
  show the source yet.

`advertise_receiver` is **not yet called by the receiver app or `cometcast-cli receive`**.
`cometcast-cli discover --announce HOST:PORT` advertises for the length of the command and is
the only caller.

Backends: this is the `mdns-sd` backend (desktop). NWBrowser (Apple) and NsdManager (Android)
are not implemented ([architecture.md](architecture.md)). Multicast can be blocked by
switches, access points or VLANs; then receivers do not show up and the room code or QR code
is the way ([measurement-points.md](measurement-points.md), A8).

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
  warnings. The Dart side does not call it yet.

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
