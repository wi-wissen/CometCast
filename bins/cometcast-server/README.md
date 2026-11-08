# cometcast-server

The Comet Cast signalling server. It lets a receiver and a sender find each other with a room
code and relays their small signalling messages (WebRTC offers, answers, ICE candidates). It
never stores or forwards media, images or files. Rooms live in memory only.

Code: this folder (the program) and [`crates/cc-server`](../../crates/cc-server) (the library).
Deep reference: [docs/server.md](../../docs/server.md). Deployment (Docker, reverse proxy,
coturn, firewall): [deploy/README.md](../../deploy/README.md). Protocol: [docs/protocol.md](../../docs/protocol.md).

> **Status: v0.1 beta.** Verified on one Linux machine only (first real run 2026-11-07). Every
> number in the settings is an unmeasured placeholder. Statements marked **UNCERTAIN** were not
> verified. See [What is not verified](#what-is-not-verified-uncertain).

## Quick start

Without any setting the server listens on `127.0.0.1:8443` (this computer only), in internal
mode, over plain HTTP.

```sh
# from the repository root (needs the Rust toolchain)
cargo run --release -p cometcast-server -- --version
cargo run --release -p cometcast-server -- --print-default-config > server.toml   # every setting, with defaults
cargo run --release -p cometcast-server -- --config server.toml
cargo run --release -p cometcast-server -- --web-dir apps/web/dist                # also serve the web app
```

For a quick local test with HTTPS (browsers need a secure context for screen sharing, except on
`localhost`): `--tls-self-signed` makes a throw-away certificate. **Development only.**

For production use the Docker setup in [deploy/README.md](../../deploy/README.md).

## Bare binary

```sh
cargo build --release --locked -p cometcast-server
install -m 0755 target/release/cometcast-server /usr/local/bin/
```

The workspace needs Rust 1.92 or newer (`rust-version` in `Cargo.toml`). Run it as an unprivileged
user. Example systemd unit (**UNCERTAIN:** not run here, adapt paths):

```ini
[Unit]
Description=Comet Cast server
After=network-online.target

[Service]
User=cometcast
ExecStart=/usr/local/bin/cometcast-server --config /etc/cometcast/server.toml
Restart=on-failure
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
# Secrets: use secret_file = "/etc/cometcast/altcha_secret" in the config (owner cometcast, mode 0600)

[Install]
WantedBy=multi-user.target
```

The server stops on SIGTERM or Ctrl-C: every open WebSocket gets a Close frame (reason
`server_shutdown`) and the program returns at the latest 5 s later.

## Command line

```
cometcast-server [options]
  --config <file>        TOML configuration file (or env COMETCAST_CONFIG)
  --mode <open|internal> operating mode (default internal)
  --listen <addr:port>   address to listen on (default 127.0.0.1:8443)
  --web-dir <path>       serve this directory (the web app) from the same address
  --allowed-origins <a,b> web origins that may use this relay from another site (CORS + /ws check)
  --tls-cert <file>      certificate chain (PEM); use with --tls-key to serve HTTPS
  --tls-key <file>       private key (PEM)
  --tls-self-signed      DEVELOPMENT ONLY: make a throw-away self-signed certificate
  --set <key=value>      set any setting, e.g. --set limits.max_rooms=500 (repeatable)
  --print-default-config print the default configuration file and exit
  --healthcheck          ask the server on this machine for /healthz; exit 0 if it answers 200
  --version, --help
```

Exit code 2 means an invalid configuration (the message says which setting).

## Configuration

Three sources, later ones win: config file (TOML), then environment variables, then command-line
options. Unknown keys in the file are errors. The file and every key in it are optional.

### Environment variables

| Variable | Meaning |
| --- | --- |
| `COMETCAST_CONFIG` | Path of the config file |
| `COMETCAST_LISTEN` | Same as `listen` |
| `COMETCAST_MODE` | Same as `mode` |
| `COMETCAST_WEB_DIR` | Same as `web_dir` (set it to an empty value to switch off the directory the Docker image presets) |
| `COMETCAST_ALLOWED_ORIGINS` | Same as `allowed_origins` (comma-separated) |
| `COMETCAST__SECTION__KEY` | Any setting; double underscore is a dot, e.g. `COMETCAST__LIMITS__MAX_ROOMS=500` sets `limits.max_rooms` |

Values given outside the file are read as TOML where possible (numbers, `true`, arrays such as
`["10.0.0.0/8"]`). Lists of networks may also be comma-separated text.

### Secrets

Use `secret_file = "/path"` for `altcha` and `turn` (for example a Docker secret). Do not pass
secrets with `--set` or environment variables: command lines and environments are visible to other
users of the machine. Secrets need at least 16 characters (UNCERTAIN: placeholder minimum), are
never printed, and are removed from error messages. Generate one with `openssl rand -hex 32`.

### Settings reference

Defaults are those of `--print-default-config` (internal mode), which equal the file
[`crates/cc-server/src/default-config.toml`](../../crates/cc-server/src/default-config.toml); a test
checks that. The "open" column shows the default when `mode = "open"` where it differs
(from [docs/server.md](../../docs/server.md#settings)). Times are seconds. **All numbers are untested placeholders.**

| Key | Default (internal) | Open | Meaning |
| --- | --- | --- | --- |
| `mode` | `internal` | | `open` or `internal` |
| `listen` | `127.0.0.1:8443` | | Address and port |
| `web_dir` | not set | | Serve the web app from this directory. Empty (`""`, or `COMETCAST_WEB_DIR=`) means not set: no files, unknown paths are `404` |
| `allowed_origins` | `[]` | | Exact web origins (`https://host[:port]`) that may use this relay from another site: CORS on `/ice-servers`, `/altcha/challenge`, `/healthz` and an `Origin` check on `/ws`; empty = same-origin only and any `/ws` origin; `*` internal mode only. [Details](../../docs/server.md#web-app-on-another-origin) |
| `idle_room_secs` | 1800 | | A room without a sender is removed after this (1 to 86400) |
| `hello_secs` | 5 | | A connection must say hello within this (1 to 86400) |
| `no_room_secs` | 60 | | A connection must be in a room within this (1 to 86400) |
| `max_message_bytes` | 65536 | | Largest WebSocket message |
| `outgoing_queue_len` | 256 | | Queued messages per slow client |
| `tls.cert_file`, `tls.key_file` | not set | | PEM files; HTTPS/WSS is on when set |
| `tls.self_signed` | `false` | | Development only |
| `tls.self_signed_names` | `["localhost","127.0.0.1","::1"]` | | Names in that certificate |
| `tls.self_signed_cert_out` | not set | | Write the public certificate here |
| `network.trusted_proxies` | `[]` | | CIDRs whose `X-Forwarded-For` is believed |
| `network.allow` | `[]` | | Only these networks may connect (empty = everybody) |
| `network.deny` | `[]` | | Refused networks (wins over `allow`) |
| `limits.connections_per_min` | 300 | 30 | New WebSocket connections per client address |
| `limits.create_room_per_min` | 60 | 6 | `create_room` per client address |
| `limits.join_attempts_per_min` | 120 | 20 | `join_room` per client address |
| `limits.failed_join_weight` | 2 | 5 | A failed join counts as this many attempts |
| `limits.http_per_min` | 300 | 30 | `/altcha/challenge` per address |
| `limits.ice_per_min` | 300 | 30 | `/ice-servers` per address (its own bucket, not shared with `/altcha/challenge`) |
| `limits.max_rooms` | 100000 | 10000 | Rooms, whole server |
| `limits.max_connections` | 20000 | 5000 | Open WebSockets, whole server |
| `limits.ban_after_failed_joins` | 30 | 10 | Failed joins within the window cause a ban |
| `limits.failed_join_window_secs` | 600 | | The window |
| `limits.ban_secs` | `[60, 300, 1800]` | | 1st, 2nd, 3rd+ ban |
| `limits.ban_memory_secs` | 3600 | | A ban history is forgotten after this |
| `limits.max_connections_per_ip` | 500 | 20 | Open connections per address (IPv6: /64) |
| `limits.max_rooms_per_ip` | 100 | 5 | Rooms owned by one address |
| `default_join_mode` | `open` | `open` | Join mode of rooms whose creator names none (`open` or `confirm`) |
| `allow_open_rooms` | `true` | `true` | `false` refuses rooms that anybody with the code can join (needs `default_join_mode = "confirm"`) |
| `join_approval_secs` | 60 | 60 | How long a join request waits for the display device (1 to 86400) |
| `limits.max_pending_per_room` / `max_pending_per_ip` | 8 / 8 | 8 / 3 | Senders waiting for approval per room / per address |
| `limits.denied_join_weight` | 2 | 2 | Failures counted toward a ban for a denied or timed-out join request |
| `limits.ping_interval_secs` | 20 | | Server pings this often |
| `limits.pong_timeout_secs` | 20 | | No sign of life for interval + timeout closes the connection |
| `limits.paired_idle_secs` | 14400 | 3600 | Idle and not answering pings: closed |
| `limits.max_room_lifetime_secs` | 86400 | 43200 | Hard cap on any room's life |
| `limits.msg_rate_per_sec` | 50 | 20 | Messages per second per connection |
| `limits.msg_burst` | 200 | 100 | Burst for the above |
| `limits.bytes_per_sec` | 262144 | 65536 | Bytes per second per connection |
| `limits.bytes_burst` | 1048576 | 262144 | Burst; must be at least `max_message_bytes` |
| `limits.max_room_relay_bytes` | 67108864 | 8388608 | Bytes relayed through one room, whole life |
| `limits.max_bad_messages` | 50 | 20 | Garbage or wrong-role messages before closing |
| `limits.global_failed_joins_per_min` | 1000 | 300 | Failed joins per minute (whole server) above which all joins are slowed (at most 10000) |
| `limits.wide_failure_factor` | 10 | | An IPv6 /48 is banned after `ban_after_failed_joins` times this |
| `altcha.mode` | `optional` | `always` (forced) | `always` or `optional` |
| `altcha.secret` / `secret_file` | random at start | | HMAC secret for puzzles, at least 16 characters |
| `altcha.cost` | 1000 | | PBKDF2 iterations per attempt |
| `altcha.base_counter` | 1000 | | Search size; doubles per pressure level |
| `altcha.max_difficulty_factor` | 8 | | Cap on the multiplier |
| `altcha.ttl_secs` | 300 | | Puzzle lifetime (1 to 86400) |
| `stun.urls` | `[]` | `[]` | Public STUN URLs listed at `/ice-servers`; allowed in **both** modes; max 4, only `stun:`/`stuns:` `host[:port]`, no credentials |
| `turn.urls`, `turn.stun_urls` | not set | not allowed | URLs given to clients (`turn.stun_urls` is checked like `stun.urls`) |
| `turn.secret` / `secret_file` | not set | not allowed | Shared secret with coturn |
| `turn.ttl_secs` | 3600 | | Credential lifetime |

Notes that the table cannot show: `ban_secs` repeats its last value; the failure table is capped at
200 000 entries; bans and counters are in memory and lost at restart.

## Modes: internal and open

| | Internal | Open |
| --- | --- | --- |
| For | An organisation's own devices | A public server, mediation only |
| ALTCHA for `create_room` | Optional (`altcha.mode`), can be `always` | Always; the server refuses to start otherwise |
| Limits | Large defaults | Strict defaults |
| `/ice-servers` with `[stun]` | STUN list, no credentials | same |
| `/ice-servers` with `[turn]` | TURN credentials (plus the STUN list) | **Not available; the server refuses to start with a `[turn]` section** |
| Image/file storage | Not built | Never |

Consequence: a TURN relay with credentials from this server only works in internal mode. An open
server can still help direct connections across NATs by listing a public STUN server (`[stun]`;
no `ALTCHA` for that call, per-address `ice_per_min` applies, CORS as for the other endpoints).
The STUN service is a separate program, see [docs/server.md](../../docs/server.md#stun-list-and-ice-servers).

## HTTP endpoints

| Path | Meaning |
| --- | --- |
| `GET /ws` | WebSocket signalling |
| `GET /healthz` | `200 ok`; always allowed from loopback, so health checks keep working |
| `GET /altcha/challenge` | A new ALTCHA puzzle (JSON, `no-store`) |
| `GET /ice-servers` | STUN list and, with `[turn]`, TURN credentials (JSON); `404` with neither `[stun]` nor `[turn]` |
| anything else | Static files from `web_dir`, if set |

Refusals: `403` (not allowed or banned, with `Retry-After` when banned), `429` (rate limit),
`503` (server full), `400` (broken `X-Forwarded-For` from a trusted proxy). No CORS headers are
sent unless `allowed_origins` is set (then only for the listed origins, with `Retry-After` exposed; an empty value is a start error, see
[docs/server.md](../../docs/server.md#web-app-on-another-origin)).

## ALTCHA

Creating a room needs a solved proof-of-work puzzle (always in open mode). Difficulty rises with
server load and recent bad events. Puzzles are single-use. Without a configured secret a random one
is made at start, so puzzles from before a restart (or from another instance) are refused: behind
several instances use the same `secret_file`. Joining a room is not protected by ALTCHA, only by
limits and bans. Solving time on weak devices is not measured.

## Limits and bans

Counted per client address (IPv6 reduced to /64, plus /48 with a higher threshold). Too many
failed joins ban the address for 1, 5, then 30 minutes (placeholders). Details and the exact rules:
[docs/server.md](../../docs/server.md#rate-limits-and-bans). Room codes have 32^6 (about 1.07
billion) values; the limits are what slows guessing.

## Client address and reverse proxy

The client address is the TCP peer, unless the peer is in `network.trusted_proxies`; then
`X-Forwarded-For` is read from the right (first address that is not a trusted proxy). A broken
header from a trusted proxy gives `400`. Set `trusted_proxies` to the proxy's address and nothing
wider. Without it, all clients behind a proxy look like one address and share one set of limits and
one TURN quota (looks like an outage). Examples for Caddy and nginx: [deploy/README.md](../../deploy/README.md#reverse-proxy).

## Logs and metrics

The server prints a start banner (version, listen address, mode, ALTCHA, STUN, TURN, web dir) and warnings
to standard output and standard error. There is no log level setting, no access log and **no metrics
endpoint** (the count of untracked failures is kept internally only). Check a running server with
`/healthz`. In Docker: `docker compose logs cometcast-server`.

## Upgrade and backup

The server keeps no state on disk (rooms, bans and counters are in memory), so there is nothing to
back up except your config and secrets. Upgrade = deploy the new binary or image and restart; all
rooms end and clients see the connection drop. Certificates are read once at start: restart after a
renewal (no reload yet).

## Security checklist

- Run as a non-root user; in Docker use the provided read-only, `cap_drop: ALL` setup.
- Serve HTTPS (certificate files, or a reverse proxy that does TLS). Do not use `--tls-self-signed` outside development.
- Secrets only via `secret_file` with mode 0600/0640; never in the config file, `--set` or the environment.
- Set `network.allow` to your networks for an internal server.
- Set `network.trusted_proxies` only to your proxy's address; make the proxy overwrite `X-Forwarded-For`.
- Open only the ports you use; the TURN ports only with TURN, UDP 3478 only if you run a STUN service.
- Pin the Docker base images by digest before a production build.
- Keep `open` mode TURN-free (the server enforces it) and do not lower its limits without measuring.
- Restart after certificate renewal.

## Troubleshooting

| Symptom | Cause and fix |
| --- | --- |
| Exits with code 2 | Invalid setting; the message names the key. Unknown keys are errors. |
| Server restarts in a loop, "cannot read key" | The TLS key is unreadable for the server user (certbot makes 0600 root-only keys; the Docker image runs as UID 65532). Give the user (or a group) read access to the key and the folders above it. The banner `listening on https://...` is printed before this check, which is misleading. |
| Container `unhealthy` after changing `command:` | The health check reads the config file and `COMETCAST_*` variables but not compose `command:` flags. Move the setting to `environment:` or override `healthcheck:` with the same flags. |
| Everyone gets `403` | `network.allow` is set and does not contain the client's address. Behind a proxy, check `trusted_proxies`. A banned address gets `403` with `Retry-After`. |
| `400` on every request | The trusted proxy sends a broken `X-Forwarded-For`. |
| `429` / `rate_limited` | Limits per address; many users behind one NAT hit the open-mode defaults. Use internal mode or raise the limits. |
| `altcha_invalid` after a restart | Random secret changed. Set `altcha.secret_file`. |
| `/ice-servers` gives `404` | Neither a `[stun]` nor a `[turn]` section (`[turn]` is refused in open mode; use `[stun]` there). |
| TURN error 486 for everybody | A proxy without `trusted_proxies`: the whole world shares one TURN user name and its quota. |
| coturn starts but TURN does not authenticate | coturn could not read `turnserver.conf` and started with defaults. See [deploy/README.md](../../deploy/README.md#troubleshooting). |
| Screen sharing does not start in the browser | Needs HTTPS (or `localhost`) and a desktop browser (UNCERTAIN: mobile browsers not checked). |
| PDF does not show in the served web app | The Docker image does not contain the WASM PDF renderer. |

## What is not verified (UNCERTAIN)

Two real devices relaying through coturn (also in one private network), NAT, TURN over TLS (5349),
IPv6 peers, other coturn versions than 4.18.0, arm64, a real certificate and renewal, load, what
source address the server sees for remote clients behind Docker port publishing, CORS behind a CDN or
rewriting proxy and with a real GitHub Pages origin, `docker stop` with live clients timed, and all limit values. The relay host
`relay.cometcast.de` does not exist yet.

## Licence

GPL-3.0-or-later, see [LICENSE](../../LICENSE).
