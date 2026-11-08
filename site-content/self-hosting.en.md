---
title: Host it yourself
description: Run your own Comet Cast server, with the web app included, for your school, company or club.
---
# Host it yourself

Your own server keeps everything inside your organisation: room codes, the web app and, if you want, the picture itself.

## One container is enough

The server is a Docker image with the web app inside, so one container works even without internet. It is also a single program you can run directly. There are two modes: <em>open</em> (public, only helps devices find each other, with a small puzzle to create a room) and <em>internal</em> (for the devices of an organisation, with TURN access so the picture stays inside your network).

## Where to start

- <a href="{{GH}}/blob/main/deploy/README.md">Quick start with Docker Compose</a>
- <a href="{{GH}}/blob/main/bins/cometcast-server/README.md">Server README</a>
- <a href="{{GH}}/blob/main/docs/server.md">Reference: docs/server.md</a>

These documents are in English. How the pieces fit together: <a href="/how-it-works/">How it works</a>.
