---
title: Privacy and self-hosting
description: What the Comet Cast relay server sees, how to run your own, and what this website collects (nothing).
---
# Privacy and self-hosting

## This website

No cookies, no tracking, no fonts or scripts from third parties. The site is delivered by GitHub Pages; GitHub technically processes connection data such as the IP address (see GitHub's privacy statement). {{todo:have the controller review and add a full privacy notice (GDPR Art. 13) here}}

## The relay server

The relay server (<code>cometcast-server</code>) mediates between sender and receiver: room codes and connection offers (signalling). It stores no images, files or media and does not forward them. Rooms live in memory only. To protect itself it sees and counts the client address (for limits and temporary bans, also in memory only). With a TURN relay (coturn, internal server only) the media stream passes through that machine when needed.

A public server <code>relay.cometcast.de</code> is planned but <strong>does not exist yet</strong>. Operations files and the guide for that host live in the repository <a href="{{RELAY}}">CometCast-Relay</a>.

## Self-hosting

Your own server is a Docker image (the web app is bundled, so one container is enough, even without internet) or a single program. There are two modes: <em>open</em> (public, mediation only, ALTCHA puzzle to create a room) and <em>internal</em> (for an organisation's own devices, with TURN credentials).

- <a href="{{GH}}/blob/main/deploy/README.md">Quick start with Docker Compose</a>
- <a href="{{GH}}/blob/main/bins/cometcast-server/README.md">Server README</a>
- <a href="{{GH}}/blob/main/docs/server.md">Deep reference docs/server.md</a>

> The server has been checked on one machine only. Not verified: two real devices over TURN, NAT, a real certificate, load. All limits are unmeasured placeholders.
