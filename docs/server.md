# Running cometcast-server

`cometcast-server` is the signalling server (code: `crates/cc-server`, program:
`bins/cometcast-server`). It mediates between a receiver and a sender and never stores
or forwards media, images or files (protocol: [protocol.md](protocol.md)). This page is
for people who run it. Milestone M8 (server side) added configuration files, HTTPS,
protection against abuse, ALTCHA and TURN credentials.

Statements that are not verified are marked **UNCERTAIN**. All numbers in the settings are
placeholders that have not been measured ([measurement-points.md](measurement-points.md),
assumption A12).

## Quick start

```sh
cargo run -p cometcast-server -- --print-default-config > server.toml   # every setting, with defaults
cargo run -p cometcast-server -- --config server.toml
cargo run -p cometcast-server -- --web-dir apps/web/dist                # no config file needed
```

Without any setting the server listens on `127.0.0.1:8443` (only this computer), in
internal mode, over plain HTTP.

## Configuration

Three sources, later ones win: **config file** (TOML), then **environment variables**,
then **command-line options**. Unknown keys in the file are errors (a typo is not silently
ignored). The file is optional; every key is optional.

- File: `--config <file>` or `COMETCAST_CONFIG=<file>`. `--print-default-config` prints a
  commented file with all defaults of internal mode (a test checks that the printed file
  equals the built-in defaults).
- Environment: `COMETCAST__SECTION__KEY=value` sets `section.key` (double underscore = dot),
  for example `COMETCAST__LIMITS__MAX_ROOMS=500`. Short forms: `COMETCAST_LISTEN`,
  `COMETCAST_MODE`, `COMETCAST_WEB_DIR`, `COMETCAST_ALLOWED_ORIGINS` (comma-separated).
- Command line: `--mode`, `--listen`, `--web-dir`, `--allowed-origins a,b`, `--tls-cert`, `--tls-key`,
  `--tls-self-signed`, `--healthcheck` (see below) and `--set section.key=value` for anything else.
  `--healthcheck` reads the same settings (config file and `COMETCAST_*` environment, plus any
  flags given to the `--healthcheck` call itself), asks the server on this machine (loopback, same
  port, HTTP or HTTPS with the certificate not checked) for `GET /healthz`, and exits 0 only for
  `200`. Addresses tried: for `0.0.0.0` or `127.0.0.1` only `127.0.0.1`; for `[::]` first
  `127.0.0.1`, then `::1` (a refused connection moves on, any answer other than 200 is final); for
  `[::1]` only `::1`; any other listen address is asked directly. It exists for the container health
  check (the image has no shell and no curl). **Pitfall:** Docker's `HEALTHCHECK` in the image
  uses only the config file path; flags you add in compose `command:` (`--listen`, `--tls-*`,
  `--set`) are not seen by it. Give such settings through `environment:` instead, or override
  `healthcheck:` with the same flags, or the container turns `unhealthy` (port/scheme mismatch).
  The server stops on SIGTERM as well as Ctrl-C. On stop every open WebSocket gets a Close frame
  (reason `server_shutdown`), and the server returns at the latest 5 s after the signal, with or
  without TLS (open HTTP connections that do not end are dropped then). An integration test
  (`shutdown_closes_open_websockets_and_run_returns_in_time`) checks this with two live clients over
  plain HTTP; the 5 s bound with TLS uses `axum_server`'s graceful shutdown as before and has no
  new test.
- Values given outside the file are read as TOML values where possible: numbers, `true`,
  and arrays like `["10.0.0.0/8"]`. Lists of networks may also be written as plain
  comma-separated text (`10.0.0.0/8, 192.168.0.0/16`).
- Secrets (`turn.secret`, `altcha.secret`) should be given as `secret_file = "/path"`
  (for example a Docker secret). Do not use `--set` or environment variables for them: command
  lines and environments are visible to other users of the machine. Values given anyway are
  always read as text (never as numbers), must have at least 16 characters (UNCERTAIN:
  placeholder minimum), are never printed, and are removed from error messages. The debug
  output shows `<hidden>`.
  In the TOML file every key containing `secret` must be text in quotes; a number or list
  there is refused with a message that names the key and never shows the value. Syntax errors
  of the file are reported with line and column only, without quoting the line.
- The settings are checked at start; an invalid combination stops the server with an
  error message (exit code 2).

### Settings

All time values are seconds. "Open / internal" columns show the default in that mode.
UNCERTAIN: every number is a placeholder.

| Key | Open | Internal | Meaning |
| --- | --- | --- | --- |
| `mode` | | `internal` | `open` or `internal` (see below). |
| `listen` | | `127.0.0.1:8443` | Address and port. UNCERTAIN: default port not decided. |
| `web_dir` | | none | Serve the web app from the same address. An **empty value** (`web_dir = ""`, `--web-dir ""`, `COMETCAST_WEB_DIR=`) means none: no files are served and every unknown path is `404` from the server itself. This is how to switch off the directory that the Docker image presets. |
| `allowed_origins` | | `[]` | Web origins (`https://host[:port]`, exact) whose pages may use this relay from another site: CORS for `/ice-servers`, `/altcha/challenge`, `/healthz` and an `Origin` check on `/ws`. Empty = same-origin only for HTTP, any browser origin on `/ws`. `*` = any origin, internal mode only. See [Web app on another origin](#web-app-on-another-origin). Env `COMETCAST_ALLOWED_ORIGINS`, CLI `--allowed-origins`. |
| `default_join_mode` | `open` | `open` | Join mode of rooms whose creator names none (`open` or `confirm`; never `locked`). Web, Flutter and CLI clients send `open` themselves unless the person chose another mode. |
| `allow_open_rooms` | `true` | `true` | `false` = rooms in join mode `open` are refused (`open_rooms_disabled`), also when a receiver switches an existing room to `open`. Needs `default_join_mode = "confirm"`. For operators who want approval everywhere. |
| `join_approval_secs` | 60 | 60 | A join request waits this long for the display device (`approval_timeout`). 1 to 86400. |
| `idle_room_secs` | 1800 | 1800 | Room without sender expires. 1 to 86400. |
| `hello_secs` / `no_room_secs` | 5 / 60 | 5 / 60 | Handshake and "must be in a room" deadlines. Each 1 to 86400. |
| `max_message_bytes` | 65536 | 65536 | Largest WebSocket message. |
| `outgoing_queue_len` | 256 | 256 | Queue per slow client. |
| `tls.cert_file`, `tls.key_file` | | none | PEM files; HTTPS/WSS is on when set. |
| `tls.self_signed` | | `false` | Development only: throw-away certificate. |
| `tls.self_signed_names` | | `localhost`, `127.0.0.1`, `::1` | Names in that certificate. |
| `tls.self_signed_cert_out` | | none | Write the public certificate here (for `curl --cacert`). |
| `network.trusted_proxies` | | empty | CIDR list of reverse proxies whose `X-Forwarded-For` is believed. |
| `network.allow` / `network.deny` | | empty | CIDR lists. Empty `allow` = everybody. `deny` wins. |
| `limits.connections_per_min` | 30 | 300 | New WebSocket connections per client address. |
| `limits.create_room_per_min` | 6 | 60 | `create_room` per client address. |
| `limits.join_attempts_per_min` | 20 | 120 | `join_room` per client address. |
| `limits.failed_join_weight` | 5 | 2 | A failed join counts as this many attempts. |
| `limits.http_per_min` | 30 | 300 | `/altcha/challenge` per client address. |
| `limits.ice_per_min` | 30 | 300 | `/ice-servers` per client address. Its own bucket: it is no longer shared with `/altcha/challenge`, so a client that fetches the ICE list on every connect cannot use up the puzzle budget (and the other way round). |
| `limits.max_rooms` | 10000 | 100000 | Rooms at the same time, whole server. |
| `limits.max_connections` | 5000 | 20000 | Open WebSocket connections, whole server. |
| `limits.ban_after_failed_joins` | 10 | 30 | Failed joins within the window cause a ban. |
| `limits.failed_join_window_secs` | 600 | 600 | The window. |
| `limits.ban_secs` | `[60, 300, 1800]` | same | 1st, 2nd, 3rd+ ban. The plan says 1, 5, 30 minutes. |
| `limits.ban_memory_secs` | 3600 | 3600 | A ban history is forgotten after this long without a new ban. 1 to 31536000 (one year); each `ban_secs` entry at most one year too. |
| `limits.max_connections_per_ip` | 20 | 500 | Open connections from one address (IPv6: /64). Internal is high because an office may sit behind one NAT address. |
| `limits.max_rooms_per_ip` | 5 | 100 | Rooms owned by one address. |
| `limits.ping_interval_secs` / `pong_timeout_secs` | 20 / 20 | 20 / 20 | The server pings; a connection that sends no data and no pong for the sum is closed (close reason `ping_timeout`). A ping sent by the client does not count as a sign of life. Each 1 to 86400. |
| `limits.paired_idle_secs` | 3600 | 14400 | A connection in a room is closed (`idle_timeout`) only if nothing was relayed for this long **and** it has not answered a ping for `ping_interval + pong_timeout`. A connection that answers pings is never closed for being idle. See "Liveness and lifetime". 1 to 31536000. |
| `limits.max_room_lifetime_secs` | 43200 | 86400 | Hard cap: longest life of any room, however busy and alive (`room_lifetime`). UNCERTAIN: 12 h / 24 h are placeholders; clients see `peer_left` when it is reached even if the video still runs. 1 to 31536000. |
| `limits.msg_rate_per_sec` / `msg_burst` | 20 / 100 | 50 / 200 | Per connection: messages per second and burst. |
| `limits.bytes_per_sec` / `bytes_burst` | 65536 / 262144 | 262144 / 1048576 | Per connection: bytes per second and burst (burst must be at least `max_message_bytes`). |
| `limits.max_room_relay_bytes` | 8388608 | 67108864 | Total bytes relayed through one room. |
| `limits.max_bad_messages` | 20 | 50 | Garbage or wrong-role messages before the connection is closed. |
| `limits.global_failed_joins_per_min` | 300 | 1000 | Emergency brake: more failed joins than this in a minute (whole server) slow down every `join_room`. Only failed joins count (unknown, malformed or full room code); bad messages, floods and denied or timed-out join requests count toward the address ban only. At most 10000 (the server remembers only that many events; a higher value is refused at start because the brake could never trigger). |
| `limits.max_pending_per_room` | 8 | 8 | Senders that may wait for approval in one room at the same time; more get `rate_limited`. At most 90 (each waiting sender gets its own two-digit check number). |
| `limits.max_pending_per_ip` | 3 | 8 | Senders waiting for approval from one client address (all rooms); more get `rate_limited`. |
| `limits.denied_join_weight` | 2 | 2 | A denied or timed-out join request counts as this many failures toward the address ban (0 = never). UNCERTAIN: placeholder; everybody behind one NAT address shares the budget. |
| `limits.wide_failure_factor` | 10 | 10 | An IPv6 /48 is banned after `ban_after_failed_joins` times this many failures. UNCERTAIN: placeholder (the first version used 4, which is probably too low for a large company or campus network that owns a /48). |
| `altcha.mode` | `always` (forced) | `optional` | See ALTCHA. |
| `altcha.secret` / `secret_file` | | random at start | HMAC secret for puzzles, at least 16 characters. |
| `altcha.cost` | 1000 | 1000 | PBKDF2 iterations per attempt. |
| `altcha.base_counter` | 1000 | 1000 | Size of the search; doubles per pressure level. |
| `altcha.max_difficulty_factor` | 8 | 8 | Upper limit of the multiplier. |
| `altcha.ttl_secs` | 300 | 300 | Puzzle lifetime. 1 to 86400. |
| `stun.urls` | `[]` | `[]` | Public STUN servers listed at `/ice-servers`, **allowed in both modes**. At most 4, only `stun:` / `stuns:` URLs (lowercase) as `host[:port]` (port 1-65535, IPv6 as `[addr]`), no credentials (`@`), path or query; checked at start. Env `COMETCAST__STUN__URLS=stun:a:3478,stun:b:3478` (comma-separated), CLI `--set stun.urls=...`. |
| `turn.urls`, `turn.stun_urls` | not allowed | none | URLs given to clients. `turn.stun_urls` follows the same rules as `stun.urls`; both are listed (without duplicates) in one STUN entry. |
| `turn.secret` / `secret_file` | not allowed | none | Shared secret with coturn, at least 16 characters (`openssl rand -hex 32`). |
| `turn.ttl_secs` | | 3600 | Credential lifetime. 1 to 31536000. |

## Join approval (who may join a room)

The display device (the receiver that created the room) chooses a join mode: `open` (anybody who knows the code joins at once), `confirm` (every join waits for an explicit allow on the display device) or `locked` (nobody can join), and may change it at any time. The server enforces it: a waiting sender can relay nothing, only the room's creator can decide, and the owner sees only a cleaned display name and a check number of a waiting sender. All message shapes, error codes and limits are in [protocol.md](protocol.md#join-approval); this section is about operating it.

- Settings: `default_join_mode`, `allow_open_rooms`, `join_approval_secs`, `limits.max_pending_per_room`, `limits.max_pending_per_ip`, `limits.denied_join_weight` (table above; all overridable by environment variable or `--set`, for example `--set allow_open_rooms=false --set default_join_mode=confirm`).
- The server default stays `open`, like every client's default, so nothing changes for existing set-ups. An operator who wants approval everywhere sets `allow_open_rooms = false` and `default_join_mode = "confirm"`: clients that ask for `open` are told `open_rooms_disabled`; the web app and the Flutter app then create a room that asks first and say so.
- The host can also block a sender for the rest of a room session ("No and block", or "End and block"), end the connected sender's presentation, and end the whole room session; a blocked sender hears the same `join_denied` as any denied sender. The block list is per room, held in memory, capped at 32 entries and keyed by network address (IPv4 address or IPv6 /64), so it also blocks other people behind the same address. See [protocol.md](protocol.md#join-approval).
- Waiting senders count against the per-room and per-address limits above and against the connection limits (a waiting sender is an open connection). A denied or timed-out request counts toward the usual temporary ban (`limits.denied_join_weight`), so a client that keeps asking is slowed down by the same machinery as failed joins. When a denial or timeout starts the ban, the joiner's connection gets `banned` and is closed (as in protocol.md). The ban then refuses new connections, joins and HTTP requests of that address; its other connections that are already open stay.
- Not covered: AirPlay, Google Cast and Miracast receivers have their own mechanisms (UxPlay PIN, Miracast consent); join approval concerns Comet Cast protocol connections only.
- Tests: `crates/cc-server/tests/join_approval.rs` (all modes, spoofing attempts, limits, timeouts, name cleaning, check numbers, operator options).

## Open and internal mode

The two modes follow the planning page ("Offen und intern"). What is built so far:

| | Open | Internal |
| --- | --- | --- |
| Purpose | Public server, mediation only | A server inside an organisation, for its own devices |
| ALTCHA for `create_room` | Always required; cannot be switched off (the server refuses to start otherwise) | Optional: checked when a client sends it; can be required with `altcha.mode = "always"` |
| Rate limits and caps | Strict defaults | Larger defaults (6 to 10 times for most numbers) |
| STUN list (`/ice-servers`, `[stun]`) | Available when `[stun]` is configured; no credentials | same |
| TURN credentials (`/ice-servers`) | Not available; the server refuses to start with a `[turn]` section | Available when `[turn]` is configured |
| IP allowlist | Empty (everybody) | Normally the organisation's networks (you set it) |
| Images and files through the server | Never | **Not built yet** (planned: checked images with expiry) |
| Approval at the display device | Client feature, not part of the server | same |

Both modes: rooms live in memory only; nothing is written to disk.

## HTTP endpoints

| Path | Meaning |
| --- | --- |
| `GET /ws` | WebSocket signalling ([protocol.md](protocol.md)). With `allowed_origins` set, a browser `Origin` that is not listed gets `403`. |
| `GET /healthz` | `200 ok`. Always allowed for a request that comes from the same computer (loopback) and is not forwarded for somebody else, even if `allow` would refuse it, so health checks keep working. A request that a trusted proxy forwards with `X-Forwarded-For` is checked like any other request against its forwarded client address (a denied client gets `403` on `/healthz` too). |
| `GET /altcha/challenge` | A new puzzle (JSON), `Cache-Control: no-store`. |
| `GET /ice-servers` | STUN list and, with `[turn]`, TURN credentials (JSON array); `404` when neither `[stun]` nor `[turn]` is configured. |
| anything else | Static files from `web_dir`, if set. A path that is no file and has no file extension (`/display`, a join code such as `/K7QM2W`, `/app`) answers `200` with `index.html` of `web_dir`, because the web app chooses its page from the path; a missing file with an extension is a plain `404`. A static host that is not this server needs the same behaviour (GitHub Pages: a copy of `index.html` as `404.html`). |

Refusals: `403` (not allowed or banned; banned answers carry `Retry-After`), `429`
(rate limit, `Retry-After`), `503` (server full).

By default no CORS headers are sent: the endpoints work for a web app served from the same address.
A web app on another origin needs `allowed_origins`, next section.

### Web app on another origin

The web app is plain static files. It can be served from any static host (GitHub Pages at
`https://cometcast.de/app/`, nginx, an existing web server) while the relay runs elsewhere (for example
`relay.cometcast.de`, or your own Docker container). The Docker image still bundles the web app for a
one-container setup; there the page and the relay share one origin and nothing below is needed.
How the web app picks its relay: [apps/web/README.md](../apps/web/README.md#static-hosting-and-relay-selection).

`allowed_origins` (config file, `COMETCAST_ALLOWED_ORIGINS=https://cometcast.de,http://localhost:5173`,
or `--allowed-origins`) lists the web origins that may use the relay:

| Topic | Behaviour |
| --- | --- |
| Entries | `scheme://host[:port]`, `http` or `https`, **exact match** (scheme, host, port; case-insensitive; a default port such as `:443` is the same origin as none). No path, no trailing slash, no `*.` wildcard. A bad entry stops the server at start (exit 2). |
| Default (not set) | No CORS headers at all (same-origin use only, as before); `/ws` accepts any `Origin`. "Not set" means absent (or a TOML `[]`). A value that is given but names no origin (`COMETCAST_ALLOWED_ORIGINS=`, `--allowed-origins ""`, `","`, `allowed_origins = ""` in the file) is a **start error** that names the source (exit 2) in every mode, because it would silently switch the check off. |
| Which endpoints | CORS only on `/ice-servers`, `/altcha/challenge` and `/healthz` (the calls a browser makes cross-origin). Never on static files. `/ws` is not subject to CORS in browsers, so it gets the `Origin` check below instead. |
| Allowed origin | `Access-Control-Allow-Origin: <that origin>` (the origin is echoed, never `*`) and `Vary: Origin`; also on refusals (`429`, `403`, `404`) so the page can read the status and show the right message. `Access-Control-Expose-Headers: Retry-After` is sent with it, so the page can read how long a `429` or a ban lasts (without it the browser hides that header from cross-origin pages). |
| Other origin | No `Access-Control-Allow-Origin`, so the browser blocks reading the answer. The `GET` itself is still processed (the server cannot tell such a page from `curl`); `Vary: Origin` is set. |
| Preflight | `OPTIONS` with `Origin` and `Access-Control-Request-Method` on those paths: allowed origin -> `204` with `Allow-Methods: GET, HEAD, OPTIONS`, the requested plain header names echoed back, `Max-Age: 600`; other origin -> `403` without CORS headers. **Private Network Access:** a preflight with `Access-Control-Request-Private-Network: true` is answered with `Access-Control-Allow-Private-Network: true` only for an allowed origin and only when the relay is in internal mode or its configured listen address is loopback/private (a public relay never says yes). UNCERTAIN: tested with raw preflight requests in the integration tests, not in a real Chrome PNA flow (Chrome's rollout and exact preflight rules were not checked); behind a reverse proxy the listen address says nothing about where the relay really sits. The web app's own requests are simple `GET`s and need no preflight. |
| Credentials | Never: `Access-Control-Allow-Credentials` is not sent, and the web app sends no cookies. |
| `/ws` | With a non-empty list, an upgrade whose `Origin` header is not listed (also `null`) gets `403` before the handshake. **No `Origin` header (native clients, curl, the CLI) is always accepted.** |
| `*` | Means any origin (`Access-Control-Allow-Origin: *`, `/ws` accepts everything). Accepted in **internal** mode only (a LAN relay); **refused in open mode** with a start error, because a public relay should name the web apps it serves. |
| The relay's own origin | If the relay also serves the web app (`web_dir`) **and** a list is set, put the relay's own origin in the list too, otherwise its own page is refused on `/ws`. Same-origin is deliberately not detected from the `Host` header: an attacker can point their own domain at your relay and make `Host` equal `Origin`. |

Trade-off for the default (empty list). It keeps today's behaviour and a LAN relay needs no setup, but any web
site can then make a visitor's browser open a WebSocket to your relay and use up that visitor's address quotas
(rate limits, room limits, bans are per client address), or try room codes from it. **Recommendation: on a
public (open) relay always set `allowed_origins`** (for example `["https://cometcast.de"]`). `Origin` is a browser
header: it protects visitors' browsers from being used by other sites; it is **not authentication** (any
non-browser client can send any `Origin`), and ALTCHA plus the rate limits remain the protection against abuse.

Checked: unit and integration tests in `crates/cc-server` (headers, preflight, refused origin, `/ws` accept and
reject, wildcard rules, config sources) and a browser test with the web app on a second static origin
(`apps/web/e2e/static-origin.spec.mjs`, Chrome). UNCERTAIN: a real GitHub Pages site, a real
`relay.cometcast.de`, behaviour behind a CDN or a reverse proxy that rewrites `Origin` or adds its own CORS
headers (do not add CORS headers in the proxy as well).

## TLS (HTTPS and WSS)

Browsers are said to need a secure context for screen sharing (HTTPS, except
`localhost`). UNCERTAIN: to be verified in the target browsers. TLS uses rustls with the
`ring` crypto backend. Only HTTP/1.1 is offered (WebSocket upgrades are HTTP/1.1).

Ways to get a certificate (the plan: "HTTPS im lokalen Netz"). Built: certificate files
and the development self-signed certificate. The rest is documentation only:

| Way | Status |
| --- | --- |
| **Certificate and key files** (`tls.cert_file`, `tls.key_file`, PEM, readable by the server user) | Built and tested with a certificate generated at test time. Works with any certificate, for example from Let's Encrypt made by another tool. |
| **Public domain plus split DNS** (plan: the default for a server in a house): the server has a name such as `cast.example.org`; the local DNS answers with the LAN address; the certificate is a normal public one (the plan says via DNS-01). | Document only. Getting the certificate is up to the operator for now. |
| **Own CA, distributed by MDM** (plan: option for houses without internet, managed devices only) | Document only. Use `tls.cert_file` with a certificate from that CA. |
| **ACME built into the server** (plan: the `instant-acme` crate) | **Out of scope for now, future work.** Nothing is built. |
| **Self-signed at start** (`tls.self_signed = true` or `--tls-self-signed`) | Built, **development only**. A loud warning is printed at every start. Browsers do not trust it. `tls.self_signed_cert_out` writes the public certificate (never the key). |
| **Reverse proxy does TLS** (see below) | Possible; the server then runs plain HTTP behind it. |

UNCERTAIN: certificates are read once at start; after a renewal the server must be
restarted (no reload yet).

## Client address, reverse proxy, allow and deny lists

All limits, bans and lists work on the **client address**:

1. By default it is the address of the direct TCP peer.
2. Only if that peer is in `network.trusted_proxies`, the `X-Forwarded-For` header is read
   **from the right**: the first address that is not itself a trusted proxy is the client.
   Everything to its left is ignored (clients can write anything there). Accepted forms per
   hop: `1.2.3.4`, `1.2.3.4:5555`, `2001:db8::1`, `[2001:db8::1]`, `[2001:db8::1]:5555`.
   **If the peer is a trusted proxy and the header is present but broken** (not valid text,
   empty, an empty hop, anything that is not an address) the request is **refused with
   `400`**. The server never falls back to the proxy's address then. Only when the header is
   missing altogether is the proxy itself the client. If every hop is a trusted proxy, the
   left-most hop counts. From a peer that is not trusted the header is ignored completely.
3. IPv4-in-IPv6 (`::ffff:1.2.3.4`) is treated as IPv4. Networks in `allow`, `deny` and
   `trusted_proxies` written that way (`::ffff:192.0.2.0/120`) are converted to IPv4
   (`192.0.2.0/24`) when the configuration is read, so they match; mapped networks shorter
   than `/96` are kept as they are. For counting, an IPv6 address is reduced to its /64
   network.

`network.deny` wins over `network.allow`; an empty `allow` lets everybody in. Refused
clients get `403` for every path.

Example nginx in front (UNCERTAIN: not run here):

```nginx
location / {
    proxy_pass http://127.0.0.1:8443;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    # Overwrite, do not append, so a client cannot prepend a fake address:
    proxy_set_header X-Forwarded-For $remote_addr;
    proxy_read_timeout 1h;
}
```

with `network.trusted_proxies = ["127.0.0.1/32"]` in the server. Caddy's `reverse_proxy`
handles the WebSocket upgrade by itself and sets `X-Forwarded-For` (UNCERTAIN: not run
here). If you use a proxy, set `trusted_proxies` to the proxy's address and nothing wider.

## Rate limits and bans

Counted per client address, with [governor](https://crates.io/crates/governor) (one
bucket per address; a quota of N per minute also allows a burst of N):

- New WebSocket connections (`connections_per_min`): refused with `429` before the upgrade.
- `create_room` (`create_room_per_min`): the client gets `rate_limited`; the connection stays open.
- `join_room` (`join_attempts_per_min`): same. A **failed** join (unknown or malformed
  code, full room) is counted harder: it costs `failed_join_weight` attempts.
- `/altcha/challenge` (`http_per_min`) and `/ice-servers` (`ice_per_min`): two separate buckets per address.
- Whole server: `max_connections` (`503`) and `max_rooms` (`create_room` gets `rate_limited`).

**Per connection (after the upgrade):** a token bucket limits messages and bytes per
second. A connection that exceeds it gets `rate_limited` and is closed, and that counts as a
failure toward a ban. Every `bad_message` / `wrong_role` also counts as a failure, and
`max_bad_messages` of them close the connection. A room may relay at most
`max_room_relay_bytes` in total. Signalling messages are small, so the defaults should be far
above normal use; UNCERTAIN: they are not measured.

**Liveness and lifetime:** the server sends a WebSocket ping every `ping_interval_secs`. A
connection that sends neither data nor a pong for `ping_interval_secs + pong_timeout_secs` is
closed (`ping_timeout`); a ping sent by the client does not count, anybody can send those
without reading. Browsers and normal WebSocket libraries answer pings automatically.

A room stays open as long as **both** of its connections keep answering pings, however
quiet the signalling channel is: media flows peer to peer, so a long presentation without any
relayed message is normal, and closing such a room would make the clients treat `peer_left`
as the end of a healthy presentation. The idle rule therefore applies only to a connection
that has stopped answering pings: if nothing was relayed for `paired_idle_secs` **and** the
connection has not answered a ping for `ping_interval_secs + pong_timeout_secs`, it is closed
(`idle_timeout`) and its peer is told `peer_left`. Both sides check themselves, so a dead side
goes and the other side learns about it. `max_room_lifetime_secs` stays a hard cap for every
room (`room_lifetime`); it should be well above the longest session you expect (defaults 12 h
open, 24 h internal; UNCERTAIN: placeholders). When it is reached, clients see the room end
although the video may still be running. When a receiver leaves, the sender's connection
stays open; it can create or join another room.

Control frames are not free: every ping and pong frame a client sends uses the same
per-connection message budget as a text message, so a flood of them is cut off like any other
flood (`rate_limited`, closed, counted as a failure).

**Slow HTTP clients:** a new connection must send a complete request head within 15 s
(before that, nothing is answered and the connection is closed), and on a kept-alive
connection each further request head must arrive within 15 s too. Plain HTTP and HTTPS
behave the same; with HTTPS the TLS handshake has its own 10 s limit first. An upgraded
WebSocket is not affected (liveness above applies). UNCERTAIN: 15 s is a placeholder and not
configurable.

**Per-address caps:** open connections (`max_connections_per_ip`) and rooms
(`max_rooms_per_ip`) per client address.

**Temporary bans:** `ban_after_failed_joins` failed joins within `failed_join_window_secs`
ban the address. The sender gets `banned`, its connection is closed, and new connections,
joins and HTTP requests from that address are refused. The first ban lasts `ban_secs[0]`,
the next one `ban_secs[1]`, and so on (the last value repeats); the history is forgotten
after `ban_memory_secs`. Failures are counted per address (IPv6: /64) and also per IPv6 /48, where the threshold is
`wide_failure_factor` times higher (default 10, UNCERTAIN), so moving to another /64 does not escape a ban.
`create_room` is refused for banned addresses too. The failure table has a hard size cap
(200 000 entries); when it is full, entries that no longer matter are dropped (at most one sweep per second, because a sweep locks the table); if nothing can be dropped, new addresses are not tracked
(a counter records this; no metrics endpoint exists yet). Bans and counters are in memory and
are lost at a restart.

This matters because room codes are short: a guesser is slowed down by these limits.
A code has 32^6, about 1.07 billion, possible values (6 characters; the plan wants 8 characters
for public servers, which is **not changed here**: it is a protocol decision).

## ALTCHA proof-of-work

In open mode a room can only be created after the client solved a small puzzle (about
750 000 PBKDF2 iterations on average with the default numbers, designed to be well under a second on a normal device, UNCERTAIN:
not measured on the oldest target device). This makes mass creation expensive without
cookies or accounts.

Which implementation: the official **`altcha` crate, version 1.1.0, licence MIT**
(checked in the crate's `Cargo.toml`; it implements ALTCHA "Proof-of-Work v2"). Its API
fits and is used directly. Because we use its "deterministic mode", checking a solution is one HMAC.

Flow:

1. The client calls `GET /altcha/challenge` and receives JSON `{"parameters": {...}, "signature": "..."}`.
2. The client finds the counter that makes `PBKDF2-SHA256(nonce || counter, salt, cost)` start with
   `keyPrefix` (the ALTCHA widget does this in the browser; the `altcha` crate's
   `solve_challenge` does it natively). The result is `{"counter": n, "derivedKey": "<hex>"}`.
3. The client sends `{"type":"create_room","altcha":"<base64>"}` where the text is the
   standard base64 of the JSON `{"challenge": <the challenge>, "solution": <the solution>}`.
4. The server checks signature, expiry and solution, and remembers the puzzle so it can be
   used **once** (a [moka](https://crates.io/crates/moka) cache with a time-to-live, so replays fail).
   The list of used puzzles holds at most 200 000 entries; when it is full, new solutions are
   refused (`altcha_invalid`) rather than forgetting a used puzzle early. Limits: the list is
   in the memory of one process. After a restart it is empty, so with a fixed `altcha.secret` a
   solution from before the restart can be used once more until it expires (`altcha.ttl_secs`).
   Several instances that share the secret each keep their own list, so one solution can create
   one room on each instance (there is no shared store).

Errors: `altcha_required` (missing), `altcha_invalid` (wrong, expired, replayed, signed by
another server, or the list of used puzzles is full). Without a configured `altcha.secret` the server uses a random secret made at
start, so puzzles from before a restart, or from another server instance, are refused.
Behind several instances, set the same `altcha.secret` on all of them.

**Adaptive difficulty:** the search size is `base_counter * factor`, where factor is
`2^level` (at most `max_difficulty_factor`) and `level` is the largest of
- server load: rooms / `max_rooms` below 50 percent = 0, below 75 = 1, below 90 = 2, else 3,
- bad events in the last minute (rate-limit hits, failed joins, wrong solutions): fewer than 10 = 0, 10 to 49 = 1, 50 to 199 = 2, 200 or more = 3,

plus 1 for a client that has recent failed joins. UNCERTAIN: all thresholds are placeholders.

Not done: ALTCHA for **joining** a room (the plan wants it at creation and joining; here only
creation, joining is protected by the limits and bans above). The web app (WebCrypto in a
Web Worker) and the native Rust clients solve the puzzle and use `/ice-servers`; the Flutter apps
do so through the native core. Solving time on weak devices is not measured (measurement point 8).
Independent check: the puzzle was solved by a
small separate Node.js script that implements the PBKDF2 step from the description above,
against a running server over WSS.

## STUN list and ICE servers

The intended connection order is: direct WebRTC first, then an internal server (TURN/SFU), then
the receiver's own Wi-Fi. For the first step across NATs the browser needs a STUN server, so a
public open relay can hand out STUN **without TURN**: STUN only tells a client its own public
address and relays no media (open TURN would be an abuse risk and stays internal-only).

`[stun] urls = ["stun:relay.example.org:3478"]` is allowed in both modes. `GET /ice-servers` then
returns, without any username or credential (the shape is `RTCIceServer`, a bare JSON array, as the
clients already parse it; there is no `ttl` field because nothing expires):

```json
[{"urls": ["stun:relay.example.org:3478"]}]
```

- No `[turn]`: the answer is just that STUN entry (open or internal mode).
- With `[turn]` (internal mode only): STUN entry first (`[stun] urls` plus `[turn] stun_urls`, no
  duplicates), then the TURN entry with REST credentials, as below.
- Neither configured: `404`. Clients (web, Rust cores, Swift core) read `404`, and any other failure,
  as "no ICE servers" and connect without them; a `429` or `403` is told apart from that in the
  web client's state and console log (`apps/web/src/core/relay.ts`), see
  [implementation-questions](design/implementation-questions.md) (D1).
- Open mode keeps its rules: `[turn]` is refused, ALTCHA is required to create rooms. The
  `/ice-servers` call itself needs **no ALTCHA** (a public STUN list is no secret and costs nothing
  to hand out), but it obeys the allow/deny lists, bans and the per-address `limits.ice_per_min`
  (its own bucket, not shared with `/altcha/challenge`), and CORS applies as for the other browser-called endpoints
  (`allowed_origins`, see [Web app on another origin](#web-app-on-another-origin)).
- The server only lists the URLs. The STUN service itself is a separate program (for example a
  STUN-only coturn; see the relay repository). STUN responses are small, but UDP source addresses can
  be spoofed, so a public STUN server is a (small) reflection/amplification helper. Operators should
  rate-limit it at the firewall and keep the response size small. UNCERTAIN: no measurements here.

## TURN credentials and coturn

TURN relays media when a direct connection is impossible (network separates devices).
coturn runs as a **separate process** and is not built here.

`GET /ice-servers` with `[turn]` configured (internal mode only) returns a JSON
array shaped like the browser's `RTCIceServer` list:

```json
[
  {"urls": ["stun:relay.example.org:3478"]},
  {"urls": ["turn:relay.example.org:3478?transport=udp"],
   "username": "1793985394:deec920ccf0cda4a",
   "credential": "SLw5VvvBZMywXQoq1rfKEBSlcwE=",
   "ttl": 3600}
]
```

The scheme is coturn's `use-auth-secret` ("TURN REST"): `username = "<expiry unix time>:<id>"`
(`credential = base64(HMAC-SHA1(secret, username))`). The HMAC is tested against values computed
independently with Python's `hmac`/`hashlib`. The id is **stable per client and time bucket**: it
is a keyed hash (HMAC with the TURN secret) of the client address (IPv4, or IPv6 /64) and the
bucket `now / ttl_secs`, and the expiry is the end of the NEXT bucket, so a credential lives
between `ttl_secs` and `2 * ttl_secs` and `ttl` in the answer is the real remaining time. Why: a
fresh random user name per call would give every call a fresh coturn `user-quota`, so the quota
would limit nothing. Now repeated calls of one client return the same user name (tested). Residual:
(a) clients behind one NAT share one user name and its quota; (b) an attacker with many addresses
(a botnet, or many IPv6 /64 networks) gets many user names; `/ice-servers` is also rate limited
per address by `limits.ice_per_min` (its own bucket), which does not help
against many addresses. `ttl` is an extra field that browsers ignore. UNCERTAIN: the plan says
"about 1 h"; the keyed hash instead of the room code is our choice (the endpoint knows no room).

**DoS risk, honestly:** whoever can call `/ice-servers` (any address allowed by the access lists;
in internal mode that is the internal network) can obtain valid credentials and use relay
allocations. Defences here are the coturn quotas set in the template (see below), nothing else; the
open points are a firewall rate limit for 3478 and a global limiter. The quota and bandwidth values
are **untuned** (UNCERTAIN).

The coturn template is `deploy/coturn/turnserver.conf.template` (render it with
`deploy/coturn/render.sh`; the script checks every value, refuses a `turn_secret` file with more than one line, validates ports (1-65535, min <= max) and addresses, and writes the result with mode 600; `deploy/coturn/test-render.sh` tests it with hostile inputs and is run by `cargo test -p cc-server`). It follows the plan's hardening list: narrow relay port range,
`use-auth-secret`, `no-multicast-peers`, `no-tcp-relay` (only RFC 6062 TCP *allocations*: TCP and TLS
as the connection to coturn on 3478/5349 stay open, and UDP relaying is unaffected), quotas, no CLI (off by default), TLS port 5349, and
`denied-peer-ip` for private and special ranges (the CLI is off by default in coturn 4.18, so there is no `no-cli` line: that option is rejected there). Pitfall from the plan: if you deny all private
ranges, relaying between two devices in the same private network breaks, because the TURN
server's own relay address is private; therefore `allowed-peer-ip=<relay IP>` is set.
Further denied ranges were added after security reviews: documentation ranges (IPv4 and
`2001:db8::/32`, `3fff::/20`), IPv4-mapped `::ffff:0:0/96`, NAT64 `64:ff9b::/96` and
`64:ff9b:1::/48`, 6to4 `2002::/16` and `192.88.99.0/24`, Teredo `2001::/32`, ORCHID
`2001:10::/28` and `2001:20::/28`, `5f00::/16`, discard-only `100::/64`, `fec0::/10`. coturn 4.18.0
accepted and listed all of them at start. **Not added: `::/96`**: `denied-peer-ip=::-::ffff:ffff`
made coturn 4.18.0 refuse every public IPv4 peer (403, tested), as `::-::` did. The IPv6 lines
were never exercised against a real IPv6 peer (the test relay was IPv4 only, an IPv6 peer is
answered with "address family mismatch" before the deny list matters, and the test host had no
IPv6), and the list does not cover your own routable IPv6 networks. IPv4 lines are unchanged.

**Quotas (tested against coturn 4.18.0, values UNCERTAIN/untuned):** `user-quota=24` (was 6; raised because clients behind one NAT or one IPv6 /64 share one user name);
`total-quota` is computed by `render.sh` as 80 % of the relay ports (400 for the default 501
ports, so it is always below the port count; a quota above the port count never triggers);
`max-bps=750000` and `bps-capacity=125000000` (bytes/s). Measured with a raw TURN client: one user
name got 6 allocations and then error 486 (quota); with many user names the total stopped at
exactly 400 (`total-quota`, with a large `bps-capacity`) and 167 with the shipped
`bps-capacity` / `max-bps` = 166, because **each allocation reserves `max-bps` out of
`bps-capacity`** (found by testing: the first guess of 1 MB/s and 60 MB/s capped the server at 60
allocations). So the effective concurrent cap is `min(total-quota, bps-capacity / max-bps)` = 166
allocations (about 80 calls) with the shipped values: raise `bps-capacity` for a bigger server.
Not tested: the byte limit per allocation itself, per-user bandwidth (needs a coturn user database,
not used), behaviour under real media load. Residual DoS: many addresses can still fill the total
cap (then everybody is refused); no firewall rate limit for 3478 is provided.

**Host networking:** the compose file runs coturn with `network_mode: host`, so coturn
listens on **all host addresses** unless `TURN_LISTENING_IP` is set in `.env` (written as
`listening-ip` by `render.sh`; tested: coturn then listens only there, and its health check must
ask that address too), and the `allowed-peer-ip` address is the host itself. A client with valid
credentials can then send **UDP** packets to UDP services that listen on that address (for example
DNS, or coturn itself). Because UDP is the only relay type (`no-tcp-relay`), TCP services such as
the Comet Cast server on 8443 or SSH are **not** reachable this way. Use a relay address without
other UDP services, or firewall it. **Run only against coturn 4.18.0** (see [First real run](#first-real-run)); other versions
are untested. Options still marked UNCERTAIN in the template were not checked by that run, and the
same-network case still needs a real test with two devices.

Where relaying may go (what the run showed, with the `allowed-peer-ip` of the relay address
192.0.2.2 and everything else as in the template): private, loopback, link-local, CGNAT
(100.64/10), benchmarking, multicast, reserved, documentation and `0.0.0.0` peers are refused
(403 on CreatePermission); public addresses are allowed; the relay address itself is allowed.
**Configured exception:** `allowed-peer-ip=<relay IP>`: with host networking that is the host.
If you relay between devices inside a private network through this server, the relay IP must
be the address those devices reach; nothing else private is reachable.

## Deployment

### Ports and firewall

| Port | Protocol | Program | Purpose |
| --- | --- | --- | --- |
| 8443 (placeholder) or 443 behind a proxy | TCP | cometcast-server | HTTPS/WSS (or HTTP behind a proxy) |
| 3478 | UDP and TCP | coturn | STUN/TURN |
| 5349 | TCP | coturn | TURN over TLS (UNCERTAIN: also UDP/DTLS if wanted) |
| 49160-49660 (placeholder) | UDP | coturn | Relay ports (`min-port`/`max-port`) |

Open only what you use. The TURN ports are only needed with TURN. Do not expose the
management side of anything (coturn's CLI is switched off).

### Names

- Planned default public server: **`relay.cometcast.de`. This name does NOT exist yet**; nothing
  answers there. All examples use `example.org` names.
- Split DNS for a server inside a house: the public name resolves to the LAN address inside.
- DNS-SRV record so devices can find the server (record name from the plan; **client-side
  discovery is not built yet**, so nothing reads this record today):

  ```
  _cometcast._tcp.example.org.  3600 IN SRV 10 0 443 cast.example.org.
  ```

### Key file permissions

The server stops at start with a clear message if it cannot read the certificate or key
file. Common traps: the Docker image runs as UID 65532, and certbot makes private keys
readable by root only (mode 0600) in folders only root can enter. Give the server user read
access to the key and the folders above it (a copy owned by that user, or a group). Do not
make keys world-readable. Secrets for compose (`secrets/turn_secret`, `secrets/altcha_secret`)
need the same care.

### Docker

`deploy/` has a `Dockerfile` (multi-stage; Node builds the web app, Rust builds the server,
`gcr.io/distroless/cc-debian12:nonroot` runs it as UID 65532), `docker-compose.yml` (server plus
coturn), an example config and an `.env.example`. **The steps for the first start are in the
comment at the top of `docker-compose.yml`; follow them in order** (secret and key file owners
matter, see below).

What the image and compose file give you (all checked in the first run, see below):

| Item | State |
| --- | --- |
| Server user | non-root (UID 65532); coturn runs as `COTURN_UID:COTURN_GID` from `.env` (default 3478:3478, compose `user:`), not the image's `nobody:nogroup` (65534): GID 65534 is the host's shared `nogroup` on Debian/Ubuntu, so other daemons could read what is given to it ([deploy/README.md](../deploy/README.md#coturn-user-and-group)). UNCERTAIN: the first run used `nobody`; the new user was not run yet |
| Root file system | read-only for both containers (coturn gets tmpfs `/tmp`, `/var/tmp`) |
| Capabilities | `cap_drop: ALL` and `no-new-privileges` for both; coturn needs `NET_BIND_SERVICE` added back, otherwise its binary cannot start |
| Health check | server: in the Dockerfile, `cometcast-server --healthcheck`; `docker compose ps` shows `healthy` (reads config file and `COMETCAST_*` env, not compose `command:` flags, see above). coturn: compose runs `turnutils_stunclient -p 3478 <TURN_LISTENING_IP or 127.0.0.1>` (a STUN request over UDP; exit 0 when answered, 1 when not; checked by hand with `docker exec`, not by waiting for Docker to report `healthy`); it does not test TLS on 5349 or relaying |
| Resource limits | `mem_limit` 512m / `pids_limit` 512 (server), 1g / 512 (coturn): UNCERTAIN guesses, never load-tested; accepted by `docker compose config`, not exercised under load. The server needs no tmpfs (read-only root worked in the first run) |
| Base images | pinned by tag **and** digest (`image:tag@sha256:...`): `node:22-bookworm-slim`, `rust:1.99-bookworm`, `gcr.io/distroless/cc-debian12:nonroot` in the Dockerfile and `coturn/coturn:4.18.0` in compose (the multi-arch index digests read from Docker Hub and gcr.io on 2026-11-08; coturn's equals the relay repository's pin of 2026-11-07). Dependabot (`.github/dependabot.yml`) proposes digest updates. UNCERTAIN: no image was built with these digests yet |
| Secrets | `turn_secret`, `altcha_secret` as compose file secrets (`/run/secrets/...`), read by the server from `secret_file`; never in the image, `Dockerfile.dockerignore` excludes `deploy/secrets`, `deploy/certs`, `deploy/.env`, `**/.env`, `**/.env.*` (except `.env.example`), `**/*.pem`, `**/*.key` (nested too; checked with a test build of a scratch context) |
| coturn secret | has to be in `turnserver.conf` (coturn has no secret-file option; UNCERTAIN: not looked for other ways), written by `render.sh` with mode 600, or 640 for coturn's group with `TURN_CONF_GROUP=3478` (must equal `COTURN_GID`; 65534 is refused) |
| TLS files | mounted read-only from `deploy/certs/`; the key must be readable by 65532 (server) and `COTURN_GID` (coturn): `chown 65532:3478 certs/privkey.pem; chmod 640` (`render.sh` with `TURN_CONF_GROUP` sets group and mode of the key too) |
| Ports | server `8443/tcp` published on all host addresses; coturn on the host network (3478 udp/tcp, 5349 tcp, relay range) |
| Trusted proxy | none by default; set `network.trusted_proxies` only for your proxy's address |

Silent failure modes found in the run, and what to look for:

- coturn cannot read `turnserver.conf` (mode 600 root file, or a group other than `COTURN_GID`): it does **not**
  stop. It logs `Cannot find config file ... Default and command-line settings will be used` and
  starts with defaults, so there is no TURN REST authentication and no hardening from the file. Check
  `docker compose logs coturn` for that line and for `Bad configuration format` or `ERROR`.
- coturn cannot read the TLS key: it logs `cannot start TLS and DTLS listeners` and runs without 5349.
- The server cannot read the TLS key: it stops with a clear message and Docker restarts it in a loop
  (the start banner `listening on https://...` is printed before that check, which is misleading).

**Build behind a TLS-intercepting proxy** (as in the test environment): pass the proxy CA with
`docker build --secret id=extra_ca,src=<ca.pem> ...`; the secret is only mounted during the `npm ci`
and `cargo build` steps and is not part of the image. With `docker compose up --build` this secret
is not passed (UNCERTAIN: `build.secrets` in the compose file was not tried), so build with
`docker build` first and run `docker compose up -d` without `--build`.

The image does **not** contain the WASM PDF renderer, so the web app served from it cannot show PDFs
([web README](../apps/web/README.md)). The bundled web app uses its own relay (same origin). If you set `allowed_origins` in the image's config, include the origin your users open (the relay's own address), see [Web app on another origin](#web-app-on-another-origin).

### Where these files may live later

The operational files in `deploy/` (Docker, compose, coturn, example configuration) may later move
to a separate repository, planned name **`CometCast-Relay`**. The server code
(`crates/cc-server`, `bins/cometcast-server`) stays in this repository. Nothing about that
repository exists yet.

## First real run

Date 2026-11-07. Machine: one Linux container with root, Docker Engine 29.9.2 (the daemon was not
running at first: `dockerd` was started by hand and worked, storage driver overlayfs), Compose 5.6.1,
linux/amd64, all network through a TLS-intercepting proxy. Everything ran on that single machine
(client and server on the same host; the host address 192.0.2.2 was used as the relay address).
Test secrets and a throw-away self-signed certificate were generated for the run and deleted.

**Ran and passed**

- `docker build -f deploy/Dockerfile` (with the fixes below): web stage (`npm ci`, `vite build`),
  Rust stage (`cargo build --release --locked -p cometcast-server`, about 1 minute), distroless
  runtime. Image about 43 MB on disk.
- `docker compose up -d`: server `Up (healthy)`; `docker inspect`: user `nonroot`, read-only root
  file system, `cap_drop ALL`, `no-new-privileges`; coturn user `nobody:nogroup`, read-only, cap
  `NET_BIND_SERVICE` only, host network. `docker stop` of the (idle) server took 0.2 s. That was
  with no client connected; at that time open WebSockets were not closed by the server and the
  plain-HTTP path had no time bound (both fixed since, see [Review fixes](#review-fixes-after-the-first-run)).
- HTTPS with the mounted certificate: `/healthz` returned `ok`, `/` served the web app,
  `/altcha/challenge` and `/ice-servers` answered. `curl --cacert` validated the certificate for `localhost`.
- WebSocket over WSS (a small Node.js script): receiver `hello`, `create_room`, `room_created`;
  sender `hello`, `join_room`, `joined`; receiver `peer_joined`; `offer` and `answer` relayed;
  `peer_left` on sender disconnect; a wrong code gave `room_not_found`. (`cometcast-cli` was not built:
  only the server crate was built, as agreed.)
- Trusted proxy and allow list (`trusted_proxies = ["172.16.0.0/12"]`, `allow = ["203.0.113.0/24"]`):
  `X-Forwarded-For: 203.0.113.9` gave 200, `8.8.8.8` gave 403, `8.8.8.8, 203.0.113.9` gave 200 (read
  from the right), no header gave 403.
- coturn 4.18.0 with the rendered template: starts without `Bad configuration format` or `ERROR`
  lines (after the fixes), listens on 3478 and 5349 tcp, **nothing on 5766 (no CLI)**.
- TURN REST credentials from `GET /ice-servers` are **accepted by coturn**: a small Python TURN client
  (raw STUN messages, long-term credential with `username` and `credential` from the endpoint)
  got a UDP allocation (`ALLOCATE OK`, relayed address 192.0.2.2:port); `turnutils_uclient -y` (from
  the coturn image) relayed 20 of 20 packets between two allocations, 0 lost.
- Refused as intended: a wrong password and a missing credential (no allocation: no open relay),
  an expired username (401); relaying to peers 127.0.0.1, 172.16/12 (docker bridges), 10/8,
  192.168/16, 100.64/10, 169.254/16, 198.18/15, 224/4, 240/4, 0.0.0.0, 192.0.2.x (except the relay
  address), 198.51.100.x, 203.0.113.x, 192.88.99.x: all `403` on CreatePermission. Allowed: 8.8.8.8
  and 1.1.1.1 (permission only; no packets were sent to the internet) and the relay address.
- `cargo test -p cc-server -p cometcast-server`: passed (including the new health check test);
  `cargo clippy` clean for both crates; `deploy/coturn/test-render.sh`: 125 checks passed.

**Bugs found and fixed**

1. Dockerfile: `rust-toolchain.toml` says `channel = "stable"`; rustup tried to download a toolchain
   called `stable` inside `rust:1-bookworm` and the build failed. Fixed: the file is removed in the
   build stage; the tag is pinned to `rust:1.99-bookworm` (the workspace needs 1.92+).
2. Dockerfile/health check: there was none ("needs a flag in the server"). Added
   `cometcast-server --healthcheck` and a `HEALTHCHECK` (also works with TLS).
3. Server ignored SIGTERM (only Ctrl-C), so `docker stop` would wait 10 s and kill it. Fixed.
4. compose: coturn container could not start (`/usr/bin/turnserver: Operation not permitted`) because
   `cap_drop: ALL` removes the capability the binary's file capability asks for. Fixed with
   `cap_add: NET_BIND_SERVICE`. coturn also got `read_only` and tmpfs.
5. coturn config: `no-cli` is an ERROR in 4.18 (the CLI is off by default; `cli` turns it on),
   `no-tlsv1`/`no-tlsv1_1` are not valid options, `no-rfc5780` is deprecated (default). Removed.
6. coturn config: **`denied-peer-ip=::-::` made coturn refuse every IPv4 peer, public ones too**, so no
   relay could work at all. Removed (bisected with a second coturn instance).
7. File permissions: the 600 `turnserver.conf` is unreadable for the coturn user and coturn silently
   falls back to defaults; the TLS key (owner root, 600) is unreadable for both. Added
   `TURN_CONF_GROUP` to `render.sh` and exact `chown`/`chmod` steps to the compose header.
8. `.env.example` had an empty `COTURN_VERSION`; set to `4.18.0` (the tag exists).

**Not run / not verified (UNCERTAIN)**

- Real network: two devices, NAT, the same-network relay case, TURN over TLS (5349 was only checked
  to be listening in this run; the review run did TLS handshakes with `openssl s_client`, but no TURN
  traffic over TLS), TURN with TCP as the client transport (3478/tcp; `no-tcp-relay` does NOT switch
  that off, it only switches off RFC 6062 TCP allocations), IPv6 relay and the
  IPv6 `denied-peer-ip` lines (the relay was IPv4 only; those lines were accepted by coturn but no
  IPv6 peer was tried), a browser WebRTC connection through this TURN server.
- coturn quotas (`user-quota`, `total-quota`) and relay port exhaustion were not exercised in this
  first run (the shipped values were also ineffective: see Review fixes).
- Certificate from a real CA, renewal, split DNS, any public name. `relay.cometcast.de` was not set up.
- Other CPU architectures, other tags of the base images, a build without the intercepting proxy
  (the build used `--network=host` and the proxy CA as a build secret).
- `docker compose up --build` (the extra CA secret is not passed there).
- `denied-peer-ip` protects only the relayed side. Whether a client with valid credentials can reach
  host services on the relay address (host networking, the `allowed-peer-ip` exception) was not
  tested; the warning in the template stands.

## Review fixes after the first run

Review of the deploy commit, same machine type (Docker Engine 29.9.2 started by hand, coturn
4.18.0 from `coturn/coturn:4.18.0`, host address 192.0.2.2, no IPv6 on the host).

**Done**

1. TURN relay exhaustion: coturn quotas were ineffective (`user-quota=50`, `total-quota=5000` above
   the ~501 relay ports) and `/ice-servers` minted a fresh user name per call. Now: stable user
   name per client and time bucket (see [TURN credentials](#turn-credentials-and-coturn)),
   `user-quota=6`, `total-quota` = 80 % of the ports (`render.sh`), `max-bps`, `bps-capacity`.
   **Tested against coturn 4.18.0 with a raw TURN client:** 10 allocations with one user name gave
   6 OK and 4 x error 486; many user names stopped at exactly 400 (`total-quota`) when the
   bandwidth capacity was raised, and at 167 (about 166) with the shipped capacity (each
   allocation reserves `max-bps`). Unit test: same client and bucket give the same user, other
   client or bucket another one; integration test: repeated `/ice-servers` calls return one user.
   Values remain untuned (UNCERTAIN); many-address attackers and a firewall limit are open.
2. Graceful shutdown: the shutdown signal now reaches the WebSocket handlers (Close frame, reason
   `server_shutdown`, task ends, peer and room cleanup run) and the plain-HTTP path has the same
   5 s bound as TLS. Integration test with two live WebSocket clients over plain HTTP. Not tested:
   the same with TLS, and `docker stop` timing with clients connected.
3. Health check: tries `127.0.0.1` then `::1` for a `[::]` listener, reports every failed address,
   treats any non-200 answer as unhealthy. Unit tests for the target lists (including `[::]`),
   for non-200/garbage/empty answers, and for the fallback logic (with two IPv4 ports, since the
   test host has no IPv6; a real `[::1]` listener test exists but was skipped here). The compose
   `command:` pitfall is documented instead of solved.
4. Template and docs corrections: `no-tcp-relay` only disables RFC 6062 TCP allocations; the
   relay-address exposure is UDP services only. TLS: coturn 4.18.0 refused TLS 1.0 and 1.1 and
   accepted 1.2 and 1.3 in a handshake test with OpenSSL 3.0.13 `s_client` (one image, one client:
   UNCERTAIN for others); `no-tlsv1`/`no-tlsv1_1` stay invalid; `cipher-list=ECDHE+AESGCM:ECDHE+CHACHA20`
   was verified to start and to take effect (a client offering only `AES128-SHA` was refused) and
   was added for TLS 1.2/DTLS.
5. IPv6 `denied-peer-ip`: added `2001:db8::/32`, `2001:10::/28`, `2001:20::/28`, `5f00::/16`,
   `3fff::/20` (accepted by coturn). `::/96` NOT added (breaks all IPv4 peers, tested). Optional
   `TURN_LISTENING_IP` (`listening-ip`) in `.env`/`render.sh`; `TURN_EXTERNAL_IP` may be
   `public/private` (each half validated; coturn 4.18.0 accepted it and an allocation worked on a
   single machine, **never run behind a real NAT**). Render tests: 164 checks.
6. Compose: coturn health check (`turnutils_stunclient`), `mem_limit` and `pids_limit` for both
   services, `Dockerfile.dockerignore` patterns for nested `.env*`, `*.pem`, `*.key` (verified with
   a scratch build context). Base images remained **not digest-pinned** then (pinned since, see the table above).

**Not verified**: TLS shutdown with clients; IPv6 behaviour of any kind; the NAT form behind a real
NAT; the memory and pid limits under load; a full `docker compose up` of the changed files (only
`docker compose config`, the coturn container with the rendered config, and `docker exec` of the
health command were run; the server image was not rebuilt).

## Not done / open

- Not verified by the first run: two real devices relaying through coturn (also in one private
  network), NAT, TURN over TLS on 5349, IPv6 peers, other coturn versions, arm64, a real certificate,
  load, and what source address the server sees for remote clients behind Docker port publishing.
- ACME (`instant-acme`) is out of scope; certificate reload without restart is not built.
- A metrics endpoint (for example the number of untracked failures); sharing bans between several server instances;
- ALTCHA for joining.
- Image and file storage on internal servers, the SFU, authentication, a ban list that survives
  restarts or is editable at run time (the plan mentions run-time block lists), bandwidth limits.
- Limits per IPv6 /64 and per IPv4 use the same quota; the plan's separate global limiter per
  action is only partly built (global caps for rooms and connections).
- All numbers: placeholders, unmeasured.

### Known costs of the stable TURN user name (from the second review, UNCERTAIN)

- **Shared NAT:** all clients behind one IPv4 address or one IPv6 /64 share one TURN user name and
  therefore one `user-quota` (24 after the review; 6 was too low for an office or a large organisation with
  several users, because a browser can use 2-3 allocations per PeerConnection). Whether 24 suits
  real use is not measured.
- **Bucket rollover:** the previous user name stays valid until the end of its bucket, so one client
  can briefly hold up to twice the quota.
- **Behind a reverse proxy without `trusted_proxies`:** the server sees only the proxy address, so
  the whole world shares one user name and one quota, and the next caller gets error 486. This fails
  safe (the header cannot be forged to mint user names) but looks like an outage. Set
  `trusted_proxies` whenever a proxy is in front.
- **`TURN_EXTERNAL_IP`:** `render.sh` checks the syntax only, not whether the "public" half is really
  public. A private address there renders without complaint and hands clients an unreachable relay.
- **Healthcheck and `TURN_LISTENING_IP`:** compose reads it from its own `.env` handling and
  `render.sh` from `deploy/.env`; with `--env-file` or another working directory the two can
  disagree and coturn stays "unhealthy".
- **Shutdown:** cleanup queues two Close frames (`server_shutdown`, then `bye`); harmless as long as
  the writer stops at the first. Clients that reconnect automatically on `server_shutdown` could
  cause a reconnect storm (not addressed).
