# AirPlay sender without FairPlay (M7, light)

Status: library `crates/cc-airplay-sender` and the CLI command `cometcast-cli airplay-send`. Tested against the real **UxPlay 1.68** (as far as UxPlay implements things) and against a **test fake** written for the tests. **No Apple TV, HomePod or AirPlay TV was available**, so nothing here is verified against Apple hardware. Related measurement points: **A6**, **A16** and point **9** in [measurement-points.md](measurement-points.md). They stay open: this work does not replace them.

## What it is, and what it is not

A desktop program that finds AirPlay receivers, pairs with them, and then

1. plays a **media link** on the receiver (`/play`, polling `/playback-info`, `/scrub`, `/stop`),
2. shows a **photo** (`/photo`),
3. shows the **screen as a live video stream**: Comet Cast captures the screen (X11, `cc-capture`), encodes H.264 (OpenH264 through `cc-media`), cuts it into MPEG-TS segments, serves an **HLS live playlist** on plain HTTP in the local network and tells the receiver to play that URL. The delay is several seconds.

It is **not** AirPlay screen mirroring. Real mirroring needs the FairPlay handshake (`fp-setup`). The project owner forbids taking over FairPlay material, so this crate does not implement or call `fp-setup`, contains no FairPlay tables or keys, and uses nothing from AirParrot, AirServer or the AGPL RustDesk code. No keys or certificates are extracted and no DRM is circumvented. The crate only has the empty socket `MirroringProvider` (trait without an implementation) and the type `Unsupported`, whose error says why. Receivers that offer only mirroring (no HLS/URL playback) cannot be used.

## How to use it

```
cometcast-cli airplay-send list [--seconds N]
cometcast-cli airplay-send pair <device> [--pin XXXX] [--mode legacy|hap]
cometcast-cli airplay-send play <device> <url> [--seconds N]
cometcast-cli airplay-send photo <device> <file.jpg>
cometcast-cli airplay-send screen <device> [--display :99] [--seconds N] [--fps N] [--bitrate-kbps N]
```

`<device>` is the name (exact, or a unique prefix) or `deviceid` shown by `list`, or an address `HOST:PORT` / `HOST` (port 7000) which skips discovery. `pair` asks the receiver to show a PIN (`pair-pin-start`), reads it from standard input (or `--pin -`), or from `--pin XXXX`, and stores the result in `$XDG_CONFIG_HOME/cometcast/airplay-pairings.json` (else `~/.config/...`), mode 0600 on Unix. Later `play`, `photo` and `screen` use the stored credentials; without them they try transient pairing if the receiver advertises it. `--pin XXXX` is visible in the process list to other local users, so prefer the prompt; the PIN is a short-lived value shown on the receiver's screen. The `screen` command needs an X11 display (`--display` or `$DISPLAY`); Wayland-only systems get the error of `cc-capture`.

## Parts of the crate

| Module | Content |
| --- | --- |
| `discovery` | mDNS browse of `_airplay._tcp` (crate `mdns-sd`), TXT fields `deviceid`, `model`, `features`/`ft`, `flags`/`sf`, `pk`, `pw`, `acl`, `srcvers`; also `_raop._tcp` for a fallback port (see below). Caps on device count, TXT entries and lengths; control characters in names are replaced |
| `http` | Own small HTTP/1.1 **and RTSP/1.0** (with `CSeq`) message code for both sides; caps: head 16 KiB, body 4 MiB, 100 headers, 20 s per message, chunked bodies supported, header-injection checks |
| `framed` | The encrypted control channel of HomeKit-style pairing (ChaCha20-Poly1305 frames of at most 1024 bytes, length as additional data, counter nonce) |
| `srp` | SRP-6a on `num-bigint`: legacy variant (2048-bit group, SHA-1) and HomeKit variant (3072-bit group, SHA-512); checks for `B = 0` mod N and bad lengths |
| `legacy` | Legacy PIN pairing (`/pair-pin-start`, `/pair-setup-pin` x3, `/pair-verify` x2) |
| `hap` | HomeKit-style transient pairing (PIN 3939), PIN pairing M1 to M6, pair-verify M1 to M4, TLV8 (`tlv`) |
| `client` | `Session`: connect with dialect detection, `authenticate`, `play`, `playback_info`, `scrub`, `rate`, `stop`, `photo` |
| `ts`, `hls`, `screen` | MPEG-TS writer, in-memory segment store and HLS server, capture and encode thread, `start_cast` |
| `mirror` | `MirroringProvider` (no implementation) and `Unsupported` |
| `creds` | Credential type (secrets hidden in `Debug`) and the 0600 JSON store |

Crates used (all permissive, licences read from `cargo metadata`, texts not checked): `mdns-sd`, `plist`, `num-bigint`, `sha1`, `sha2`, `hkdf`, `ed25519-dalek`, `x25519-dalek` (BSD-3-Clause), `chacha20poly1305`, `aes`, `ctr`, `aes-gcm`, `zeroize` (secret session keys and seeds are wiped when dropped where the code holds them in `Zeroizing`; `num-bigint` values and copies inside the crypto crates are not covered). The SRP crate `srp` was **not** used: its fit with Apple's variant was unchecked in the plan, and the exact hashing conventions (below) are easier to control in our own 250 lines.

## Where the protocol knowledge comes from

* **pyatv 0.18.0** (MIT, sdist from pypi.org, **read, not copied**): `protocols/airplay/auth/legacy.py`, `.../srp.py`, `.../hap.py`, `.../hap_transient.py`, `auth/hap_srp.py`, `auth/hap_tlv8.py`, `auth/hap_session.py`, `protocols/raop/protocols/airplayv1.py`, `airplayv2.py`, `protocols/airplay/player.py`, and its helper library srptools 2.0.0 (BSD-3-Clause) for the SRP byte conventions. The constants (the two prime groups) are the public values of RFC 5054 and RFC 3526.
* The planning notes on doubletake (HKP 5, `com.apple.ScreenCapture`) were used only as a pointer; no doubletake code was read for this work.
* UxPlay: **only the installed program's behaviour** was observed. Its README and source were not read or used.
* GitHub, crates.io's website API and the like were not reachable (403 from the sandbox proxy); cargo's registry downloads worked.

## What was checked against the real UxPlay 1.68

Setup: `uxplay 1.68.2` from Ubuntu, `-vs fakesink -as 0 -pin 4711`, Xvfb, Avahi, in the Linux container of 2026-11-06. (`-pin 4711` gives a fixed PIN; `-p` was also tried.)

| Step | Result |
| --- | --- |
| mDNS discovery (`airplay-send list`) | **Worked.** `_airplay._tcp` found: name, model `AppleTV3,2`, `deviceid`, features `0x5A7FFEE6,0x0`, `pw=true`, `pk` |
| Port | **UxPlay quirk:** it advertises `_airplay._tcp` with a port one higher than the one it listens on (listens 44545, advertises 44546; with `-p` listens 7000, advertises 7001), while `_raop._tcp` has the listening port. The sender therefore also browses `_raop._tcp` (matched by MAC address) and falls back to that port when the advertised one refuses. UNCERTAIN whether other receivers behave this way |
| Dialect | UxPlay answers **only RTSP/1.0 with CSeq** on this port. A plain `HTTP/1.1` request gets no answer (`httpd didn't get response`), the connection closes. `Session::connect` therefore probes (`GET /info` over HTTP/1.1, 2 s) and falls back to RTSP/1.0. This costs two seconds on UxPlay. In RTSP answers a missing `Content-Length` means "no body" (UxPlay's `pair-pin-start` answer has none and keeps the connection open) |
| `pair-pin-start` | **Worked.** UxPlay prints the PIN line |
| Legacy PIN pairing (`/pair-setup-pin`, SRP-6a SHA-1 2048, AES-GCM step) | **Worked** with the right PIN, our SRP maths and byte conventions are accepted by UxPlay; a wrong PIN is **rejected with 470** (`Client Authentication Failure (client proof not validated)`). UxPlay **does send** its proof (`proof` in the second answer); we now **require** and check it, and the interop test passes with that (2026-11-06) |
| `pair-verify` with the stored credentials (X25519, Ed25519, AES-CTR) | **Worked** (UxPlay answered 200 to the second message; UxPlay's own log shows no error) |
| `/play` | **Acknowledged only.** UxPlay 1.68 answers `200 OK` with an empty body to `/play`, `/playback-info`, `/scrub`, `/rate` and `/stop` and logs nothing; no media is played and `/playback-info` has no `duration`. It has no HLS/URL player in this version (no `-hls` option). So **playback of a link was NOT observed on any real receiver** |
| `/photo` | **Failed.** UxPlay's parser rejects the `PUT` method in RTSP mode (`HPE_INVALID_CONSTANT`) and closes the connection |
| `screen` | The capture, encoder, TS muxer and HLS server ran for 14 s against UxPlay (which ignores `/play`). **ffmpeg and ffprobe, acting as the HLS client, played the playlist**: H.264 Constrained Baseline 1280x720, 1.001 s segments, about 10 fps. No receiver fetched it |
| HomeKit-style pairing | **Not tested against UxPlay**: UxPlay 1.68 answers `/pair-setup` only with its 32-byte legacy exchange (observed), not TLV8 |

The integration test `crates/cc-airplay-sender/tests/uxplay_interop.rs` repeats the pairing and `/play` steps against a UxPlay it starts itself (skips with a loud `NOTE: SKIPPING ...` line if `uxplay` or `Xvfb` is missing or uxplay does not open its port). In the 2026-11-06 container `uxplay -p N` listened on **N+1** (not N), so the test tries both. UNCERTAIN: the HomeKit M4 proof is required because the HAP description says the accessory sends it; no Apple receiver or UxPlay (which does not speak TLV8) confirmed that for AirPlay.

## What the test fake covers (not a real receiver)

`src/testfake.rs` is labelled as a test fake. It implements the protocol subset from the **same sources** as the sender: legacy PIN pairing and pair-verify, HomeKit transient and PIN pairing and pair-verify with the encrypted channel, `server-info`, `play`, `playback-info`, `scrub`, `rate`, `stop`, `photo`, an RTSP-only mode like UxPlay, and optionally a small HLS client. Because both were written from the same reading, passing tests show internal consistency (crypto round trips, framing, state handling, wrong-PIN and forged-key rejection, hostile answers) but **not** that Apple receivers agree. The screen test (Xvfb with a red window, fake receiver fetching the playlist and three segments) also decodes the fetched segments with ffmpeg and checks the picture colour.

## The screen as an HLS stream

* H.264 Baseline from the OpenH264 encoder in `cc-media` (screen-content mode), 10 fps and 2 Mbit/s by default, constant frame rate (an unchanged screen repeats the last picture, so the stream never stalls).
* Every segment starts with an IDR frame (a key frame is forced when a segment is due), MPEG-TS with PAT, PMT, PCR on the video PID, one PES per access unit with PTS and an access unit delimiter. Segment length 1 s, the playlist keeps the newest 6, version 3, no `EXT-X-ENDLIST` until the stream stops.
* Low-latency choices: short segments, aligned key frames, no encoder look-ahead, no B-frames. **Not** used: LL-HLS partial segments or blocking playlist reload (plain HLS receivers do not all support them).
* Expected delay: a receiver normally starts about three segments behind the live edge and buffers more, so **several seconds** (estimate 3 to 6 s; **not measured on a real receiver**). On the test machine the first two segments were ready about 2 s after the start.
* The HLS server listens on our address towards the receiver only, answers only the receiver's IP address, GET and HEAD only, at most 16 connections, 10 s timeouts, 16 MiB per segment, segments only in memory. Plain HTTP, no encryption: anyone on the path can see the screen. One request per connection (`Connection: close`), single `Range` requests on segments answer 206, no CORS header, segments are served from the shared store without an extra copy. UNCERTAIN: whether a given receiver accepts a plain `http://` LAN URL.
* No audio, no mouse pointer, no Wayland (limits of `cc-capture`, see [capture.md](capture.md)).

## Hostile input

The receiver's answers are untrusted. Size caps: HTTP head 16 KiB, body 4 MiB, plist 1 MiB, TLV8 64 KiB, store file 1 MiB, TXT entries 64 of 512 bytes, 256 devices, 16 addresses per device. Deadlines: connect 5 s, socket read/write 10 s, one HTTP message 20 s, probe 2 s. Invalid SRP values, bad lengths, truncated TLVs, wrong frame tags, unknown start lines, endless bodies and header floods end in errors (unit tests and the `hostile_receivers...` test); the code has no `unwrap` on peer data. The `plist` crate parses recursively and has **no depth limit**: a hostile receiver could abort the whole process with about 60,000 nested arrays (400 KB; XML plists at about 900 KB), a stack overflow that nothing can catch. `plistx::decode_dict` therefore checks the input first without recursion: for `bplist00` it validates the trailer, every offset and declared length against the data, rejects a container that is reached twice (cycles, exponential sharing) and nesting deeper than 32; for XML it scans the tags for nesting deeper than 32 (comments, CDATA and processing instructions skipped). Only then the crate runs, on a thread with a 16 MiB stack, and a panic inside it becomes an error. Regression tests: nested binary and XML probes, a 2^40 array length, a cycle, a huge object count. Other crates (`mdns-sd`, `num-bigint`) were not audited for hostile input. "Nothing panics on bad input" is the aim and is tested for the paths above, not proven.

## UNCERTAIN list

* Everything about **Apple receivers** (Apple TV, HomePod, AirPlay TVs): pairing details, `X-Apple-HKP` values (3 PIN and 4 transient are from pyatv; 5 for tvOS with the access-control entry `com.apple.ScreenCapture` from the planning notes is **not implemented** because the encoding of that entry is not documented in a source we may use), whether a tvOS build that answers 470 to transient pairing (reported in the plan) can be paired at all, acceptance of plain HTTP/RTSP dialects, `/photo` and `/play` behaviour.
* **AirPlay 2 media sessions**: AirPlay 2 receivers want an RTSP session (SETUP with plist, event channel, timing server, feedback, RECORD, then `/play`, see pyatv `airplayv2.py`). This is **not implemented**. Whether those receivers accept our plain `/play` after pairing is unknown. The AirPlay-1 style requests (`/play` with `Content-Location` and `Start-Position`, `/scrub`, `/rate`, `/stop`, `PUT /photo`) follow pyatv's `airplayv1.py`; the `/photo` headers (`X-Apple-AssetKey`, `X-Apple-Transition`) come from public descriptions of the older protocol, not from a source read in this work.
* SRP conventions: minimal-length integers in `M1`/`M2`, padded in `u`/`k`, raw salt (pyatv drops leading zero bytes of the salt, about 1 in 256 salts differ), our `A` is sent padded to the prime length (pyatv sends it unpadded). Verified for the legacy variant against UxPlay only; the 3072-bit variant only against our fake.
* HomeKit M6: the receiver's signature is checked like the HAP description says (`AccessoryX | id | key`); pyatv does not check it. A mismatch is reported as a warning only. The receiver's SRP proof (M4 in HomeKit pairing, transient and PIN; answer 2 in legacy pairing) is **required and checked**: a missing proof is an error (downgrade protection). After pairing, plaintext bytes that follow the last pairing answer make the switch to the encrypted channel fail (they could otherwise be forged answers).
* `pair-verify` (legacy): the receiver's signature is not checked (its key is unknown to us, pyatv does not check it either).
* The meaning of feature bits and `flags` comes from pyatv's table, which says itself that two public sources are partly inconsistent.
* Whether a missing `Content-Length` in HTTP/1.1 answers should mean "read to the end" (RFC) or "empty" (our choice, needed for UxPlay's RTSP answers).
* Credential storage: the file is written to a fresh, randomly named temporary file in the same directory (created new, mode 0600, fsynced), renamed over the store, and the directory is fsynced; a directory that has to be created gets mode 0700. The store key is the `deviceid` from the mDNS announcement, which **anyone on the network can fake**. For HomeKit credentials this does not let a rogue device pass pair-verify (the receiver's stored key signs the exchange), but a rogue device that announces another device's `deviceid` and is then paired by the user **overwrites that entry** (the CLI prints a warning when `pair` replaces an entry). Legacy credentials give no way to authenticate the receiver at all. A key bound to the receiver's long-term key would need that key before pairing; not done. Rejected: any `B` that is 0, 1, N-1 or not below N in SRP. A session is dropped after any I/O or framing error (a half-written encrypted frame would desynchronise the counters). File mode 0600 on Unix; Windows ACLs and the system key store (planned) are not done. The credentials hold our long-term Ed25519 seed.
* IPv6 receivers with link-local addresses (zone ids) were not handled or tested; IPv4 addresses are preferred.
* H.264 licensing of the OpenH264 build: see [capture.md](capture.md).
* The latency of the HLS path on a real receiver and the playlist's acceptance by Apple's own validator (`mediastreamvalidator`) were not checked.

## Not done

* Real AirPlay screen mirroring (forbidden: FairPlay).
* AirPlay 2 RTSP media session, `X-Apple-HKP: 5` with access control, audio streaming (ALAC/RTP), remote control, volume.
* Windows and macOS capture backends (`cc-capture` has none yet), Wayland.
* Conversion of PNG or other photo formats (only JPEG is accepted; no re-encoding).
* Storing credentials in the system key store.
