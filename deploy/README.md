# Deploying cometcast-server

Files in this folder: `Dockerfile` (server plus web app), `docker-compose.yml` (server plus
coturn), `config/cometcast-server.example.toml`, `coturn/` (template and `render.sh`), `.env.example`.
Reference for the program and every setting: [bins/cometcast-server/README.md](../bins/cometcast-server/README.md).
Deep reference and test notes: [docs/server.md](../docs/server.md).

> **Verified only once, on one Linux machine (2026-11-07, Docker Engine 29.9.2, Compose 5.6.1, coturn
> 4.18.0, amd64).** Everything marked **UNCERTAIN** was not tried. All limits are unmeasured placeholders.

## Which setup?

| You want | Use |
| --- | --- |
| One container, no internet needed (office, meeting room) | Docker image alone: it bundles the web app. `mode = "internal"`. |
| Calls across networks (media through TURN) | Docker compose with coturn. TURN credentials work in **internal** mode only. |
| Your own public server | `mode = "open"` (ALTCHA always, no TURN). Behind a reverse proxy. |

## Quick start with Docker Compose

Run from this folder. The UIDs matter: the server image runs as 65532, the coturn image as `nobody` (65534);
neither runs as root. The same steps are in the header of `docker-compose.yml`.

```sh
# 1. configuration
cp config/cometcast-server.example.toml config/cometcast-server.toml     # edit: mode, TLS paths, turn urls
cp .env.example .env                                                     # edit: realm, IPs, relay ports

# 2. secrets (one line each)
mkdir -p secrets certs
openssl rand -hex 32 > secrets/turn_secret
openssl rand -hex 32 > secrets/altcha_secret
sudo chown root:65532 secrets/*; sudo chmod 640 secrets/*

# 3. certificate and key (never commit them); both programs read the key
cp /path/fullchain.pem certs/fullchain.pem; cp /path/privkey.pem certs/privkey.pem
sudo chown 65532:65534 certs/privkey.pem; sudo chmod 640 certs/privkey.pem

# 4. coturn configuration (holds the TURN secret)
sudo TURN_CONF_GROUP=65534 ./coturn/render.sh

# 5. build and start (repository root is the build context)
docker build -f Dockerfile -t cometcast-server:local ..
docker compose up -d
docker compose ps          # server should be "healthy"
curl --cacert certs/fullchain.pem https://your.host:8443/healthz
```

Without TURN (internal server, one network, or open mode): delete the `[turn]` section from the
config and the `coturn` service from the compose file, and skip the TURN secret and `render.sh`.
(UNCERTAIN: this reduced setup was not run as such.)

Building behind a TLS-intercepting proxy: add `--network=host --secret id=extra_ca,src=/path/ca.pem`
to `docker build`, then `docker compose up -d` **without** `--build` (the secret is not passed by
compose; UNCERTAIN whether `build.secrets` works). The image does not contain the WASM PDF renderer,
so PDF display is missing in the web app it serves.

Ports: the server publishes `8443/tcp` on all host addresses. Change the left side to
`127.0.0.1:8443:8443` if a reverse proxy on the same host is the only client.

## Bare binary

See [bins/cometcast-server/README.md](../bins/cometcast-server/README.md#bare-binary). coturn then is your own installation
(use `coturn/turnserver.conf.template` as a starting point; `render.sh` does the filling).

## Reverse proxy

Terminate TLS in the proxy and run the server on plain HTTP behind it (no `[tls]` section), or keep
TLS on the server and proxy at the TCP level. Needed either way: WebSocket upgrade, and a
trustworthy `X-Forwarded-For` with `network.trusted_proxies` set to the proxy's address.
Both examples are **UNCERTAIN: not run here**.

Caddy (handles the WebSocket upgrade by itself and sets `X-Forwarded-For`):

```caddyfile
relay.example.org {
    reverse_proxy 127.0.0.1:8443
}
```

nginx:

```nginx
server {
    listen 443 ssl;
    http2 on;
    server_name relay.example.org;
    ssl_certificate     /etc/letsencrypt/live/relay.example.org/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/relay.example.org/privkey.pem;

    location / {
        proxy_pass http://127.0.0.1:8443;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        # Overwrite, do not append, so a client cannot prepend a fake address:
        proxy_set_header X-Forwarded-For $remote_addr;
        proxy_read_timeout 1h;
    }
}
```

Server side, for a proxy on the same host (bare binary):

```toml
listen = "127.0.0.1:8443"
[network]
trusted_proxies = ["127.0.0.1/32"]
```

With the server in Docker and the proxy on the host, the proxy's address as the server sees it is the
Docker bridge address; the example config suggests `172.16.0.0/12` and says to trust nothing wider
than your proxy. UNCERTAIN: which source address the server sees for a remote client through Docker's port publishing was
not tested. Without `trusted_proxies` everyone shares one address (limits, bans, one TURN user name).

## TLS options

| Way | State |
| --- | --- |
| Certificate and key files (`tls.cert_file`, `tls.key_file`), for example from Let's Encrypt made by another tool | Built, tested with a generated certificate |
| Reverse proxy does TLS | Possible; server runs plain HTTP behind it |
| Public name with split DNS inside the organisation | Documentation only; you get the certificate |
| Own CA distributed by MDM | Documentation only; use files from that CA |
| ACME built into the server | Not built |
| `tls.self_signed` | Development only; browsers do not trust it |

Certificates are read once at start: **restart after renewal**. certbot keys are 0600 root-only; copy
them with the owners shown above (for example in a certbot deploy hook).

## Configuration

The server reads `/etc/cometcast/server.toml` in the image (`CMD ["--config", ...]`); inside a container
`COMETCAST_LISTEN=0.0.0.0:8443` and `COMETCAST_WEB_DIR=/srv/cometcast/web` are preset. Full settings
table and environment variables: [bins/cometcast-server/README.md](../bins/cometcast-server/README.md#configuration).
Secrets: `secret_file` (`/run/secrets/turn_secret`, `/run/secrets/altcha_secret` through compose secrets); never `--set`.

`.env` (used by compose and `coturn/render.sh`):

| Variable | Meaning |
| --- | --- |
| `COTURN_VERSION` | coturn image tag; `4.18.0` is the one that was run |
| `TURN_REALM` | TURN realm, usually the host name |
| `TURN_EXTERNAL_IP` | Public IP; behind NAT `public/private` (never run behind a real NAT) |
| `TURN_LISTENING_IP` | Optional: listen only on this address (else all host addresses) |
| `TURN_RELAY_IP` | Address coturn relays from; the only own address peers may reach |
| `TURN_MIN_PORT`, `TURN_MAX_PORT` | Relay UDP range, default 49160-49660 (placeholder) |

## TURN / coturn

coturn runs as a separate container with host networking (Linux only). `render.sh` fills the template from `.env`
and `secrets/turn_secret` and writes `coturn/turnserver.conf` (not committed; it contains the secret). The server's
`turn.secret_file` must hold the same value. The template sets `use-auth-secret`, a narrow relay range, quotas
(`user-quota=24`, `total-quota` = 80 % of the ports, `max-bps`, `bps-capacity`), `no-tcp-relay`, no CLI,
and `denied-peer-ip` for private and special ranges with `allowed-peer-ip` for the relay address.
Run only coturn 4.18.0. Clients get credentials from `GET /ice-servers` (internal mode).

Important caveats (details in [docs/server.md](../docs/server.md#turn-credentials-and-coturn)):
with host networking a client with credentials can send UDP to UDP services on the relay address (DNS, coturn itself);
use a relay address without other UDP services or firewall it. Effective concurrent allocations are
`min(total-quota, bps-capacity / max-bps)`, about 166 with the shipped values. No firewall rate limit for 3478 is provided.

## Firewall: ports to open

| Port | Protocol | Program | Purpose |
| --- | --- | --- | --- |
| 443 (behind a proxy) or 8443 | TCP | server / proxy | HTTPS and WSS |
| 3478 | UDP and TCP | coturn | STUN/TURN |
| 5349 | TCP | coturn | TURN over TLS |
| 49160-49660 (placeholder) | UDP | coturn | Relay ports; must match `TURN_MIN_PORT`/`TURN_MAX_PORT` |

Open only what you use; the TURN ports only with TURN. coturn's CLI port (5766) must stay closed (it is off).

## Open and internal in practice

- **Internal:** `network.allow` = your networks, ALTCHA optional or `always`, TURN allowed, large limits.
- **Open:** ALTCHA always, strict limits, no `[turn]` (the server refuses to start with one), no images or files.
- ALTCHA: set `altcha.secret_file` so puzzles survive restarts and work on several instances.

## Limits and bans

All per client address; see the table in the server README. First ban 1 min, then 5, then 30. Bans live in memory and are lost at restart.
To unban an address now: restart the server (there is no admin interface; UNCERTAIN: no run-time ban list exists).

## Health check and monitoring

- Server: `docker compose ps` (health from `cometcast-server --healthcheck`, reads the config file and `COMETCAST_*`, not compose `command:` flags). From outside: `GET /healthz`.
- coturn: a STUN request over UDP (`turnutils_stunclient`). It does not test TLS on 5349 or relaying.
- Logs: `docker compose logs cometcast-server coturn`. The server has no log levels, no access log and no metrics endpoint.

## Upgrade and backup

No state on disk. Backup: `config/`, `.env`, `secrets/`, `certs/` (keep them private). Upgrade:
`git pull`, rebuild the image, `docker compose up -d`; all rooms end during the restart. Pin base images by digest before a production build (they are plain tags now).

## Security hardening checklist

- [ ] HTTPS on, certificate valid, key mode 640 with the right owner
- [ ] `secrets/` and `certs/` not in git (`.gitignore` and `Dockerfile.dockerignore` already exclude them)
- [ ] `network.allow` set for an internal server
- [ ] `trusted_proxies` only the proxy's address; proxy overwrites `X-Forwarded-For`
- [ ] Only needed ports open; relay range narrow; 5766 closed
- [ ] coturn relay address without other UDP services, or firewalled
- [ ] Base images pinned by digest; image rebuilt for security updates
- [ ] Containers keep `read_only`, `cap_drop: ALL`, `no-new-privileges`, memory and pid limits
- [ ] Restart planned after certificate renewal

## Troubleshooting

Real pitfalls found in the first run and review ([docs/server.md](../docs/server.md#first-real-run)):

| Symptom | Cause and fix |
| --- | --- |
| coturn log `Cannot find config file ... Default and command-line settings will be used` | `turnserver.conf` is mode 600 root; coturn runs as `nobody`. Re-run `TURN_CONF_GROUP=65534 ./coturn/render.sh`. coturn does **not** stop; no TURN auth and no hardening until fixed. |
| coturn log `cannot start TLS and DTLS listeners` | coturn cannot read the key. `chown 65532:65534 certs/privkey.pem; chmod 640`. Runs without 5349. |
| Server restarts in a loop | Cannot read the TLS key (clear message in the log). Same fix. |
| coturn: `/usr/bin/turnserver: Operation not permitted` | `cap_drop: ALL` without `cap_add: NET_BIND_SERVICE`. Keep the compose file's setting. |
| `Bad configuration format` in coturn | A template option not valid in your coturn version (`no-cli`, `no-tlsv1` are invalid in 4.18); use 4.18.0. |
| Nobody can relay, public peers get 403 | Do not add `denied-peer-ip=::-::` (or `::/96`): it blocks every IPv4 peer in 4.18.0. |
| Relay does not work between two devices in one private network | The relay IP must be the address those devices reach; other private ranges are denied. Needs a real two-device test (UNCERTAIN). |
| Container `unhealthy` | `command:` flags are not seen by the health check; use `environment:`. coturn: `TURN_LISTENING_IP` differs between compose and `render.sh` (`--env-file`, other working directory). |
| TURN error 486 for everyone | Proxy without `trusted_proxies` (one shared TURN user name), or quota reached. |
| `TURN_EXTERNAL_IP` private address | `render.sh` checks syntax only; clients get an unreachable relay. |
| Docker build fails at `rustup` | `rust-toolchain.toml` says `stable`; the Dockerfile already removes it. Do not undo that. |
| `docker compose up --build` fails behind an intercepting proxy | Build with `docker build --secret ...` first (see above). |

## What is NOT verified (UNCERTAIN)

Two real devices through TURN (also in one private network), NAT and the `public/private` form of
`TURN_EXTERNAL_IP`, TURN over TLS traffic, TURN over TCP as client transport, IPv6 relay and IPv6
deny lines, a browser WebRTC connection through this coturn, quotas under real load, other coturn
versions, arm64, a real certificate and renewal, `docker compose up --build`, a full `compose up` of the
latest files after the review fixes (only `compose config` and the coturn container were run), memory
and pid limits under load, the Caddy and nginx examples, and the setup without coturn.
`relay.cometcast.de` does not exist yet.
