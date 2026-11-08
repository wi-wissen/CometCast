---
title: Host it yourself
description: Run your own Comet Cast server, with the web app included, for your school, company or club.
---
# Host it yourself

Your own server keeps everything inside your organisation: room codes, the web app and, if you want, the picture itself.

<h2 id="when">When do you need your own server?</h2>

Without your own server, Comet Cast uses the public relay. It only helps devices find each other; the picture goes directly between them. That is enough when devices can reach each other: in the same Wi-Fi, and often across the internet.

You need your own server when people present from a **different network** than the screen, the classic case being **guest Wi-Fi versus the internal network**. These networks are usually kept apart on purpose, and many firewalls do not let the direct connection through. Your own server with a TURN relay, reachable from both networks (for example in the DMZ), carries the picture between them. It always works, the picture stays inside your organisation, and nobody has to set up their device: the screens show your server's address and QR code.

Without IT, people can join the Wi-Fi of the screen instead, if they are allowed to.

## One container is enough

The server is a Docker image with the web app inside, so one container works even without internet. It is also a single program you can run directly. There are two modes: <em>open</em> (public, only helps devices find each other, with a small puzzle to create a room) and <em>internal</em> (for the devices of an organisation, with TURN access so the picture stays inside your network).

## Where to start

- <a href="{{GH}}/blob/main/deploy/README.md">Quick start with Docker Compose</a>
- <a href="{{GH}}/blob/main/bins/cometcast-server/README.md">Server README</a>
- <a href="{{GH}}/blob/main/docs/server.md">Reference: docs/server.md</a>

These documents are in English. How the pieces fit together: <a href="/how-it-works/">How it works</a>.
