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
  `COMETCAST_MODE`, `COMETCAST_WEB_DIR`.
- Command line: `--mode`, `--listen`, `--web-dir`, `--tls-cert`, `--tls-key`,
  `--tls-self-signed`, and `--set section.key=value` for anything else.
- Values given outside the file are read as TOML values where possible: numbers, `true`,
  and arrays like `["10.0.0.0/8"]`. Lists of networks may also be written as plain
  comma-separated text (`10.0.0.0/8, 192.168.0.0/16`).
- Secrets (`turn.secret`, `altcha.secret`) can be given as `secret_file = "/path"`
  instead (Docker secrets). They are never printed; the debug output shows `<hidden>`.
- The settings are checked at start; an invalid combination stops the server with an
  error message (exit code 2).

### Settings

All time values are seconds. "Open / internal" columns show the default in that mode.
UNCERTAIN: every number is a placeholder.

| Key | Open | Internal | Meaning |
| --- | --- | --- | --- |
| `mode` | | `internal` | `open` or `internal` (see below). |
| `listen` | | `127.0.0.1:8443` | Address and port. UNCERTAIN: default port not decided. |
| `web_dir` | | none | Serve the web app from the same address. |
| `idle_room_secs` | 1800 | 1800 | Room without sender expires. |
| `hello_secs` / `no_room_secs` | 5 / 60 | 5 / 60 | Handshake and "must be in a room" deadlines. |
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
| `limits.http_per_min` | 30 | 300 | `/altcha/challenge` and `/ice-servers` per client address. |
| `limits.max_rooms` | 10000 | 100000 | Rooms at the same time, whole server. |
| `limits.max_connections` | 5000 | 20000 | Open WebSocket connections, whole server. |
| `limits.ban_after_failed_joins` | 10 | 30 | Failed joins within the window cause a ban. |
| `limits.failed_join_window_secs` | 600 | 600 | The window. |
| `limits.ban_secs` | `[60, 300, 1800]` | same | 1st, 2nd, 3rd+ ban. The plan says 1, 5, 30 minutes. |
| `limits.ban_memory_secs` | 3600 | 3600 | A ban history is forgotten after this long without a new ban. |
| `altcha.mode` | `always` (forced) | `optional` | See ALTCHA. |
| `altcha.secret` / `secret_file` | | random at start | HMAC secret for puzzles. |
| `altcha.cost` | 1000 | 1000 | PBKDF2 iterations per attempt. |
| `altcha.base_counter` | 1000 | 1000 | Size of the search; doubles per pressure level. |
| `altcha.max_difficulty_factor` | 8 | 8 | Upper limit of the multiplier. |
| `altcha.ttl_secs` | 300 | 300 | Puzzle lifetime. |
| `turn.urls`, `turn.stun_urls` | not allowed | none | URLs given to clients. |
| `turn.secret` / `secret_file` | not allowed | none | Shared secret with coturn. |
| `turn.ttl_secs` | | 3600 | Credential lifetime. |

## Open and internal mode

The two modes follow the planning page ("Offen und intern"). What is built so far:

| | Open | Internal |
| --- | --- | --- |
| Purpose | Public server, mediation only | A server inside an organisation, for its own devices |
| ALTCHA for `create_room` | Always required; cannot be switched off (the server refuses to start otherwise) | Optional: checked when a client sends it; can be required with `altcha.mode = "always"` |
| Rate limits and caps | Strict defaults | Larger defaults (6 to 10 times for most numbers) |
| TURN credentials (`/ice-servers`) | Not available; the server refuses to start with a `[turn]` section | Available when `[turn]` is configured |
| IP allowlist | Empty (everybody) | Normally the organisation's networks (you set it) |
| Images and files through the server | Never | **Not built yet** (planned: checked images with expiry) |
| Approval at the display device | Client feature, not part of the server | same |

Both modes: rooms live in memory only; nothing is written to disk.

## HTTP endpoints

| Path | Meaning |
| --- | --- |
| `GET /ws` | WebSocket signalling ([protocol.md](protocol.md)). |
| `GET /healthz` | `200 ok`. Always allowed from the same computer (loopback), even if `allow` would refuse it, so health checks keep working. |
| `GET /altcha/challenge` | A new puzzle (JSON), `Cache-Control: no-store`. |
| `GET /ice-servers` | TURN credentials (JSON); `404` when TURN is not configured. |
| anything else | Static files from `web_dir`, if set. |

Refusals: `403` (not allowed or banned; banned answers carry `Retry-After`), `429`
(rate limit, `Retry-After`), `503` (server full).

UNCERTAIN: no CORS headers are sent. The endpoints work for a web app served from the same
address. A web app on another origin (for example a page that talks to a public server)
would be blocked by the browser; whether and how to allow that is not decided.

## TLS (HTTPS and WSS)

Browsers are said to need a secure context for screen sharing (HTTPS, except
`localhost`). UNCERTAIN: to be verified in the target browsers. TLS uses rustls with the
`ring` crypto backend. Only HTTP/1.1 is offered (WebSocket upgrades are HTTP/1.1).

Ways to get a certificate (the plan: "HTTPS im lokalen Netz"). Built: certificate files
and the development self-signed certificate. The rest is documentation only:

| Way | Status |
| --- | --- |
| **Certificate and key files** (`tls.cert_file`, `tls.key_file`, PEM, readable by the server user) | Built and tested with a certificate generated at test time. Works with any certificate, for example from Let's Encrypt made by another tool. |
| **Public domain plus split DNS** (plan: the default for a server in a house): the server has a name such as `cast.school.example`; the school's DNS answers with the LAN address; the certificate is a normal public one (the plan says via DNS-01). | Document only. Getting the certificate is up to the operator for now. |
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
   Everything to its left is ignored (clients can write anything there). A header that
   contains something that is not an address is not trusted at all.
3. IPv4-in-IPv6 (`::ffff:1.2.3.4`) is treated as IPv4. For counting, an IPv6 address is
   reduced to its /64 network.

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
- `/altcha/challenge` and `/ice-servers` (`http_per_min`).
- Whole server: `max_connections` (`503`) and `max_rooms` (`create_room` gets `rate_limited`).

**Temporary bans:** `ban_after_failed_joins` failed joins within `failed_join_window_secs`
ban the address. The sender gets `banned`, its connection is closed, and new connections,
joins and HTTP requests from that address are refused. The first ban lasts `ban_secs[0]`,
the next one `ban_secs[1]`, and so on (the last value repeats); the history is forgotten
after `ban_memory_secs`. Bans and counters are in memory and are lost at a restart.

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

Errors: `altcha_required` (missing), `altcha_invalid` (wrong, expired, replayed, or signed by
another server). Without a configured `altcha.secret` the server uses a random secret made at
start, so puzzles from before a restart, or from another server instance, are refused.
Behind several instances, set the same `altcha.secret` on all of them.

**Adaptive difficulty:** the search size is `base_counter * factor`, where factor is
`2^level` (at most `max_difficulty_factor`) and `level` is the largest of
- server load: rooms / `max_rooms` below 50 percent = 0, below 75 = 1, below 90 = 2, else 3,
- bad events in the last minute (rate-limit hits, failed joins, wrong solutions): fewer than 10 = 0, 10 to 49 = 1, 50 to 199 = 2, 200 or more = 3,

plus 1 for a client that has recent failed joins. UNCERTAIN: all thresholds are placeholders.

Not done: ALTCHA for **joining** a room (the plan wants it at creation and joining; here only
creation, joining is protected by the limits and bans above); the web app does not
use ALTCHA yet (`apps/web` was not changed, and neither were the native clients; today
they work against internal servers only). Independent check: the puzzle was solved by a
small separate Node.js script that implements the PBKDF2 step from the description above,
against a running server over WSS.

## TURN credentials and coturn

TURN relays media when a direct connection is impossible (network separates devices).
coturn runs as a **separate process** and is not built here.

`GET /ice-servers` (only if `[turn]` is configured, internal mode only) returns a JSON
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
(expiry = now + `ttl_secs`, id random per request), `credential = base64(HMAC-SHA1(secret, username))`.
The HMAC is tested against values computed independently with Python's `hmac`/`hashlib`.
`ttl` is an extra field that browsers ignore. UNCERTAIN: the plan says "about 1 h"; and that the
id is a random value, not the room code, is our choice (the endpoint knows no room).

The coturn template is `deploy/coturn/turnserver.conf.template` (render it with
`deploy/coturn/render.sh`). It follows the plan's hardening list: narrow relay port range,
`use-auth-secret`, `no-multicast-peers`, `no-tcp-relay`, quotas, `no-cli`, TLS port 5349, and
`denied-peer-ip` for private and special ranges. Pitfall from the plan: if you deny all private
ranges, relaying between two devices in the same private network breaks, because the TURN
server's own relay address is private; therefore `allowed-peer-ip=<relay IP>` is set.
**UNCERTAIN: coturn was not run and the template was not tested against any coturn
version.** Every option marked UNCERTAIN in it needs checking, and the same-network case
needs a real test with two devices.

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
  _cometcast._tcp.school.example.  3600 IN SRV 10 0 443 cast.school.example.
  ```

### Docker

`deploy/` has a `Dockerfile` (multi-stage; Node builds the web app, Rust builds the server,
`gcr.io/distroless/cc-debian12:nonroot` runs it as a non-root user), `docker-compose.yml`
(server plus coturn), an example config and an `.env.example`.
**UNCERTAIN: none of it was built or started.** The machine used had the Docker CLI but no
Docker daemon; at most `docker compose config` (a syntax check) was run. Image tags (Node,
Rust, distroless, coturn 4.18 from the plan) are unchecked assumptions. Read the comments at the top of
each file before the first use.

### Where these files may live later

The operational files in `deploy/` (Docker, compose, coturn, example configuration) may later move
to a separate repository, planned name **`cometcast-relay`**. The server code
(`crates/cc-server`, `bins/cometcast-server`) stays in this repository. Nothing about that
repository exists yet.

## Not done / open

- Docker images and compose were not built or run; coturn was not run (see above).
- ACME (`instant-acme`) is out of scope; certificate reload without restart is not built.
- ALTCHA for joining; ALTCHA in the web and native clients; CORS for other origins.
- Image and file storage on internal servers, the SFU, authentication, a ban list that survives
  restarts or is editable at run time (the plan mentions run-time block lists), bandwidth limits.
- Limits per IPv6 /64 and per IPv4 use the same quota; the plan's separate global limiter per
  action is only partly built (global caps for rooms and connections).
- All numbers: placeholders, unmeasured.
