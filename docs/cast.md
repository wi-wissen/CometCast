# Chromecast / Google Cast compatible sender (`cc-cast`, BETA)

**Status: written from the public Open Screen source, tested only against a fake receiver on loopback. It has never talked to a real Chromecast, Google TV or TV with Cast. Measurement point A5 (see [measurement-points.md](measurement-points.md)) is open.**

The crate `crates/cc-cast` lets Comet Cast **send** its screen to a Cast receiver that runs Google's mirroring receiver app. Receiving Cast (acting as a Cast receiver) is out of scope. Descriptively, the feature is called "Chromecast / Google Cast compatible sender"; see "Legal notes".

```
cometcast-cli cast list [--seconds N]
cometcast-cli cast send <device-name|ip[:port]> [--screen] [--display :99] [--fps N] [--bitrate-kbps N]
                        [--seconds N] [--trust-anchors PEM] [--insecure-skip-device-auth]
```

## What it does, step by step

| Step | Module | Source in Open Screen | Tested how |
| --- | --- | --- | --- |
| mDNS browse `_googlecast._tcp.local.`, TXT keys `id`, `fn`, `md`, `ve`, `ca`, `st` | `discovery` | `cast/common/public/receiver_info.h`, `receiver_info.cc` | TXT parsing unit tests only. The mDNS browsing itself (crate `mdns-sd`) was run on a network without Cast devices and found none |
| `CastMessage` protobuf, 4-byte big-endian length prefix, 64 KiB limit | `proto`, `framing` | `cast/common/channel/proto/cast_channel.proto`, `message_framer.cc` | unit tests (split/back-to-back frames, oversize, garbage) |
| TLS to port 8009 with the device's self-signed certificate | `tls` | (`cast/sender/channel/sender_socket_factory.cc` was fetched but not studied) | loopback against the fake receiver |
| Device authentication (challenge with SHA-256, signature check, chain check) | `device_auth` | `cast/sender/channel/cast_auth_util.cc`, `message_util.cc` | loopback: success, no anchors, foreign root, bad signature, bad nonce |
| Heartbeat `PING`/`PONG`, answer to the receiver's `PING` | `channel` | **not in the Open Screen sender code that was read** (see UNCERTAIN list) | unit tests and loopback (PINGs counted) |
| `CONNECT` to `receiver-0`, `GET_STATUS`, `LAUNCH` (app `0F5096E8`), `RECEIVER_STATUS` (sessionId, transportId), `LAUNCH_ERROR`, `INVALID_REQUEST`, second `CONNECT` to the transport id | `session` | `cast/standalone_sender/looping_file_cast_agent.cc`, `cast/common/channel/message_util.cc`, `cast/sender/channel/message_util.cc`, `connection_namespace_handler.cc`, `cast/common/public/cast_streaming_app_ids.h` | loopback state machine incl. `LAUNCH_ERROR` |
| `OFFER` / `ANSWER` JSON on `urn:x-cast:com.google.cast.webrtc` (H.264, `aesKey`, `aesIvMask`, `ssrc`, `timeBase` 1/90000, ...) | `offer` | `cast/streaming/public/offer_messages.cc`, `answer_messages.cc`, `sender_message.cc`, `receiver_message.cc`, `message_fields.h`, `public/sender_session.cc`, `impl/rtp_defines.h`, `public/constants.h` | unit tests and loopback |
| AES-128-CTR of each frame, nonce = frame id XOR IV mask | `rtp` | `cast/streaming/impl/frame_crypto.cc` | unit tests (independent block computation, 64-bit carry) and decrypt on the receiver side of the loopback test |
| RTP packetisation with the cast header, sender report every 500 ms, parsing of picture-loss and cast feedback, NACK retransmission | `rtp`, `streamer` | `impl/rtp_packetizer.cc`, `rtp_defines.h`, `sender_report_builder.cc`, `ntp_time.cc`, `compound_rtcp_parser.cc`, `rtcp_common.cc`, `sender_impl.cc` | unit tests; loopback with reassembly, PLI and NACK |
| Capture (X11) -> OpenH264 -> RTP | `media` | reuses `cc-capture` and `cc-media` | loopback with a scripted screen and with the real encoder + decoder; **the X11 path with this crate was not run** |

Open Screen's own C++ code is not copied. The Rust code follows its field names, constants and layouts; the module headers say which file each part comes from.

## Open Screen source that was available and read

The source **was** reachable over HTTPS (`chromium.googlesource.com`, `?format=TEXT`, base64). Commit: `8b62b21479f1adb7663f182d97962b960c5c64ef` (the head of `main` on 2026-11-06; commit message "Roll Perfetto ..."). Files fetched at that commit and read (some only in the relevant parts):

- `cast/common/channel/proto/cast_channel.proto`, `message_framer.cc`, `message_util.h`, `message_util.cc`, `virtual_connection.h`, `virtual_connection_router.cc`, `connection_namespace_handler.cc`
- `cast/common/public/cast_streaming_app_ids.h`, `cast_streaming_app_ids.cc`, `receiver_info.h`, `receiver_info.cc`
- `cast/sender/channel/cast_auth_util.cc`, `cast/sender/channel/message_util.cc`
- `cast/standalone_sender/looping_file_cast_agent.cc`
- `cast/streaming/message_fields.h`, `sender_message.h`, `sender_message.cc`, `ssrc.cc`, `resolution.cc`
- `cast/streaming/public/sender_session.h`, `sender_session.cc`, `offer_messages.cc`, `answer_messages.cc`, `receiver_message.cc`, `session_messenger.cc`, `constants.h`, `constants.cc`, `encoded_frame.h`, `frame_id.h`
- `cast/streaming/impl/rtp_packetizer.cc`, `rtp_packetizer.h`, `rtp_defines.h`, `frame_crypto.cc`, `sender_impl.cc`, `sender_report_builder.cc`, `ntp_time.cc`, `rtcp_common.cc`, `compound_rtcp_parser.cc`
- `util/simple_fraction.cc`, `LICENSE`

Fetched but **not** studied: `cast/sender/channel/sender_socket_factory.cc`, `cast/sender/cast_platform_client.cc`. Looked up by name only (directory listings): the certificate directory `cast/common/certificate`, including `cast_root_ca_cert_der-inc.h` (the trust anchors, **not downloaded**, see below). Not read at all: BoringSSL (so the exact behaviour of `AES_ctr128_encrypt` is assumed), Chromium's own mirroring sender (not part of Open Screen), anything about how Google's receiver app decides to accept or refuse a sender.

Facts confirmed in that source (not assumed): app id `0F5096E8` is `GetCastStreamingAudioVideoAppId()` (audio-only is `85CDB22F`); namespaces `urn:x-cast:com.google.cast.webrtc` (OFFER/ANSWER), `...tp.connection`, `...tp.heartbeat`, `...tp.deviceauth`, `...receiver`; platform ids `sender-0` / `receiver-0`; the launch message (`LAUNCH`, `requestId`, `appId`, `language`, `supportedAppTypes: ["WEB"]`); the app answers with `RECEIVER_STATUS` carrying `sessionId` and `transportId`; the mDNS service id `_googlecast._tcp`; payload type 101 for H.264 (96 for the Android TV "hack"); the cast RTP header; the nonce construction; the SSRC ranges; default target delay 400 ms; RTCP report interval 500 ms; the RTP/RTCP of Open Screen share one UDP port, whose number is `udpPort` in the ANSWER and whose host is the receiver's address.

## Device authentication: exactly what verification needs

Open Screen (`cast_auth_util.cc`) checks, in this order: the TLS certificate of the connection is valid now and expires within 4 days; the sender nonce is echoed; the chain `client_auth_certificate` + `intermediate_certificate` leads to a **trusted Cast root**; (optionally) the CRL; and the signature over `nonce || DER(TLS certificate)` verifies with the device certificate's key (RSA, PKCS#1 v1.5 or PSS; SHA-1 is tolerated, SHA-256 is requested).

- Implemented: TLS-certificate validity and lifetime, nonce, chain signatures and validity dates up to a caller-supplied anchor, **certificate roles** (the device certificate must be a non-CA with KeyUsage `digitalSignature`; every issuer must be a CA with `BasicConstraints` CA=true, intermediates need KeyUsage `keyCertSign`, `pathLenConstraint` is honoured, so a non-CA certificate can never sign another one; a trust anchor without any KeyUsage is tolerated, one with a KeyUsage must allow `keyCertSign`), RSA signature (PKCS#1 v1.5 with SHA-256 or SHA-1, PSS with SHA-256).
- SHA-1 responses are accepted like Open Screen does (`enforce_sha256_checking` is false there), but only after the chain has been validated up to an anchor. The sender always asks for SHA-256.
- **Not implemented** (UNCERTAIN: Open Screen's validator does these): the Cast policy OIDs of the certificates, the device policy, CRL evaluation (the CRL in the response is ignored), name constraints, unknown critical extensions. UNCERTAIN: real device certificates were never seen, so the KeyUsage requirement on the device leaf is an assumption; a leaf without KeyUsage is rejected.
- **The trust anchors.** They are public data in the Open Screen tree (`cast/common/certificate/cast_root_ca_cert_der-inc.h`), but under the project rule "no key or certificate material taken from a product" **nothing is downloaded or embedded**. `DeviceAuth::Verify(TrustAnchors)` therefore **fails closed** when it has no anchors: `cast send` without `--trust-anchors` stops with an explanation. A person who decides to supply anchors themselves can pass them as a PEM file with `--trust-anchors`.
- `--insecure-skip-device-auth` (`DeviceAuth::InsecureSkip`) is OFF by default. It skips the device-auth exchange entirely (no challenge is sent), so any host that answers on that address is accepted as the receiver and gets the screen. The CLI prints a warning when it is used, and the library returns the same warning in `Negotiated::warnings`. Combining it with `--trust-anchors` is an error.
- TLS: the self-signed device certificate is accepted by a custom rustls verifier that records the certificate; the handshake signature is still checked against its key. This is only meaningful together with device auth (the signed blob contains this certificate).

## What is NOT verified (no Chromecast here)

Everything in this crate is unverified against a device. Specific open points, in the order of the measurement-point list:

1. **A5: `LAUNCH_ERROR`.** The crate reports it (`CastError::LaunchError { reason }`, e.g. `NOT_FOUND`, `SYSTEM_ERROR`), but whether Google's mirroring receiver app launches for a third-party sender at all is unknown. Google may restrict the app (for example by requiring signed app/sender identity); nothing in Open Screen says either way.
2. **A5: no `ANSWER`.** An `OFFER` the receiver cannot use is answered with an error or not at all (`CastError::OfferRejected`, or a timeout after 10 s per step). The exact fields Google's receiver requires or tolerates (empty `protection`, `profile`, `level`, `errorRecoveryMode`, the `rtpProfile: "cast"` field, the single resolution) were taken from Open Screen's own sender, not tried.
3. **A5: latency over 400 ms.** The default target delay of 400 ms is Open Screen's default; no latency was measured. The encoder starts with 2 Mbit/s at 15 fps (cc-media defaults), also unmeasured.
4. **H.264 packaging:** Annex-B (start codes) is sent, as `cc-media` produces it. Whether the receiver wants Annex-B or AVCC is open (plan, spike 10). Profile is Constrained-Baseline-style from OpenH264; resolution is the capture size rounded down to even numbers, with **no scaling** (a 4K screen is offered as 4K).
5. **AES-CTR counter carry:** assumed to be a 128-bit big-endian increment (BoringSSL source not read). A test covers the carry across the low 64 bits against an independent computation.
6. **Frame references:** delta frames reference the previous frame id; the first frame references itself. Not checked against a device.
7. **Retransmission:** only the NACK list of the cast feedback is used (re-sends from a cache of the last 32 frames). The CST2 frame ACK vector, receiver reports, receiver frame logs and RTT are ignored. Resending is bounded because NACKs come from the network: at most 64 NACK entries and 16 events per RTCP datagram, a packet is not resent twice within 20 ms, all resends share a budget of about 1 MB/s and 1000 packets/s (burst 250 ms), and a resend never waits for a full socket (it is dropped). RTCP is accepted only from the exact address and port the RTP is sent to (UNCERTAIN: a receiver answering from another port would be ignored). No congestion control, no `kickstart` packet. Static screens produce no frames; a key frame is repeated after 2 s of silence (a guess).
8. **Heartbeat:** the 5 s `PING` interval is from the project plan. The `{"type":"PING"}` / `{"type":"PONG"}` bodies and the 15 s silence timeout come from general knowledge of the public Cast v2 protocol, **not from the Open Screen files that were read** (a search found no heartbeat handling in `cast/common/channel` or `cast/standalone_sender`). Whether real devices require the heartbeat from the sender, and with which timing, is open.
9. **`GET_STATUS`:** sent before `LAUNCH`; the reply is optional for this sender. `STOP` on exit uses the `sessionId` (format from `CreateStopRequest` in Open Screen).
10. **Discovery on real networks:** not tried. Link-local IPv6 addresses are listed after IPv4 and not used for connecting unless they are the only ones (scope ids are dropped by this code: `IpAddr` has no scope, so a link-local IPv6 address will usually not connect; not worked around).
11. **`CONNECT` details:** `connType 0` (strong), `sdkType 2` (sent as `skdType`, spelled like Open Screen's key constant), `platform 6` (Linux, sent on every OS). Receivers may behave differently for unknown user agents.
12. **Windows/macOS:** capture exists only for X11 on Linux (see [capture.md](capture.md)); the crate itself is portable Rust.

## Fallbacks from the plan (not implemented)

If A5 fails (no `ANSWER`, `LAUNCH_ERROR`, latency over 400 ms):

1. **Default Media Receiver with HLS** (a few seconds of delay): launch the Default Media Receiver app and have it play an HLS stream served by Comet Cast.
2. **Custom Web Receiver** (a one-time developer registration fee of US$ 5, as stated in the plan; UNCERTAIN, check the current price): our own receiver page that plays a WebRTC stream with low delay.

Both are separate work; none of it is started. The `session` and `channel` modules would be reused (the same Cast v2 channel launches those apps).

## Legal notes (all UNCERTAIN, not legal advice)

- **Open Screen** is BSD-3-Clause. The notice is kept in [THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md) and the module headers say which file each part follows. No Open Screen source file is copied and none of its data (trust anchors, test certificates) is embedded.
- **No proprietary material:** no keys or certificates from any product, no AirServer/AirParrot/FairPlay material, no circumvention of DRM, no impersonation of a Cast receiver. There is no private key in the repository: the fake receiver's RSA "device key" in `crates/cc-cast/tests/e2e.rs` is generated at test time and never stored.
- **The fake receiver** in `crates/cc-cast/tests/e2e.rs` is a test double: its certificates are generated at test time (rcgen), it speaks only the subset the tests need, it does not advertise itself on the network, and real senders reject it.
- **Trademarks:** "Google Cast" and "Chromecast" are trademarks of Google. Use them only descriptively ("Chromecast/Google Cast compatible sender"), never as the product name, and not as a claim of certification. Whether Google's terms for the Cast SDK and the receiver apps allow an unregistered third-party sender to use the mirroring app (`0F5096E8`) was not checked.
- The H.264 patent question is the same as for the rest of the screen sender, see [capture.md](capture.md), "H.264 licensing".

## How to try it on a real device (checklist for the person testing)

1. Same network/VLAN as the device, multicast allowed. `cometcast-cli cast list` should show it (friendly name, model, address, id).
2. `cometcast-cli cast send <name> --display :0 --insecure-skip-device-auth` (or with `--trust-anchors roots.pem` if you obtained the roots yourself).
3. Record, for measurement point A5: the stage that failed (TLS, device auth, LAUNCH, ANSWER, no picture), the exact `reason` or `error` text, the device model and firmware, the picture delay, and whether the picture is correct (green/garbled picture means Annex-B vs AVCC or profile problems).
4. Compare with Open Screen's own `cast_sender` (build it from the same commit) against the same device: if that works and this does not, the difference is in this crate.

## Tests

Text that comes from the network (friendly name, model, id, `LAUNCH_ERROR` reason, OFFER error text) is escaped and length-capped before it is printed. The app-stopped detection (`RECEIVER_STATUS`) only reacts to a status with an explicit `applications` array (UNCERTAIN: a receiver that omits it when idle ends the stream by connection close or heartbeat timeout instead). The heartbeat's first PING is sent one interval after the start, so it cannot overtake the first `CONNECT`.

`cargo test -p cc-cast`: 43 unit tests plus 14 loopback tests (`tests/e2e.rs`) with the fake receiver. The loopback tests cover: full negotiation with verified device auth, `LAUNCH_ERROR`, rejected `OFFER`, device auth without anchors / with a foreign root / with a bad signature / with a bad nonce / with a chain signed by a non-CA device leaf (forged chain), a NACK flood and a foreign RTCP source, the explicit insecure option and its warning, heartbeat, encrypted multi-packet frames decrypted on the other side, PLI and NACK, `run_stream`, the capture loop with a scripted screen, and real OpenH264 output through the whole path to a successful decode.
