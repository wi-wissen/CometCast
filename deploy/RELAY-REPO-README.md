# CometCast-Relay

> DRAFT for `README.md` of the separate repository `wi-wissen/CometCast-Relay`. Not pushed. Replaces the
> current skeleton text there. Items in **bold UNCERTAIN** need a decision or a check.

Operations repository for the public Comet Cast relay at `relay.cometcast.de`. **The host does not exist yet.**

## How this repository relates to the main one

| Repository | Holds |
| --- | --- |
| [wi-wissen/CometCast](https://github.com/wi-wissen/CometCast) | The code (`cc-server`, `cometcast-server`) **and the generic deployment files** in `deploy/` (Dockerfile, Compose, coturn template and `render.sh`, example config), tested together with the code. Operator guide: [deploy/README.md](https://github.com/wi-wissen/CometCast/blob/main/deploy/README.md). Reference: [docs/server.md](https://github.com/wi-wissen/CometCast/blob/main/docs/server.md). |
| This repository | What is specific to the live host `relay.cometcast.de`: the actual configuration choices, DNS/firewall/host notes, runbook, and the setup checklist [docs/SETUP.md](docs/SETUP.md). **No secrets or private keys, ever.** |

The generic files stay in the main repository for now (the earlier plan to move them here is not done). This repository should
reference a tested commit or tag of the main repository instead of copying files, so the two cannot drift apart.

## What the relay does

- Mediates connections: room codes and WebRTC offers/answers (signalling). It stores and relays no images, files or media.
- Optional TURN relay (coturn) for networks that block direct connections.
- Protects itself with rate limits, temporary bans and ALTCHA proof-of-work.

## Decision needed before setup (UNCERTAIN)

The server has two modes. In **open** mode ALTCHA is always on and the server **refuses to start with a `[turn]` section**
(no `/ice-servers`). TURN credentials exist only in **internal** mode. The current `docs/SETUP.md` says the default relay is
`open` and also lists coturn ports; those two do not fit together as the server is built today. Choose:
(a) public relay in `open` mode without TURN (direct connections and the own-Wi-Fi fallback only), or (b) a different arrangement for
TURN on the public host (not designed). Also note the main repository's example config uses `8443`; the SETUP text says `443`
(use a reverse proxy on 443 or change `listen`). Also, the web app on `cometcast.de` calls the relay cross-origin: set `allowed_origins = ["https://cometcast.de"]`
(CORS for `/ice-servers` and `/altcha/challenge`, and an `Origin` check on `/ws`; see `docs/server.md`, "Web app on another origin").

## Status

The statement "Docker and coturn have never been run" in the old skeleton is out of date: on 2026-11-07 the image built and ran, the Compose
stack started, WebSocket room create/join worked over WSS, and coturn 4.18.0 accepted TURN REST credentials and allocated a relay,
all on one Linux machine. **Not verified:** two real devices, NAT, TURN over TLS, IPv6, a real certificate, arm64, load.
Details: [docs/server.md "First real run"](https://github.com/wi-wissen/CometCast/blob/main/docs/server.md#first-real-run).

## Contents

| Path | Content |
| --- | --- |
| [docs/SETUP.md](docs/SETUP.md) | Checklist for setting up `relay.cometcast.de` (update it to match [deploy/README.md](https://github.com/wi-wissen/CometCast/blob/main/deploy/README.md), see below) |
| `LICENSE` | GPL-3.0, same as the main repository |

### Changes to make in `docs/SETUP.md` to keep both consistent

1. Replace "Docker and coturn have never been run" with the status above and link to deploy/README.md for the exact steps (UIDs, `TURN_CONF_GROUP=65534 render.sh`, key owner `65532:65534`, mode 640).
2. Resolve the open/TURN decision above and state the chosen mode.
3. State the real ports: server 8443 behind a proxy on 443 (or direct), coturn 3478 udp+tcp, 5349 tcp, relay range from `.env`.
4. Add: set `network.trusted_proxies` when a proxy is in front; restart after certificate renewal; the health check is `/healthz`.
5. The "Not done yet" list: remove "Docker/coturn never run"; keep "no metrics endpoint" and "no certificate reload".

## Licence

GPL-3.0-or-later. Contributions follow the CLA of the main repository
([CLA.md](https://github.com/wi-wissen/CometCast/blob/main/CLA.md)).
