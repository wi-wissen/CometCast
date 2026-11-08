---
title: Privacy
description: What this website collects (nothing) and what the Comet Cast relay server sees and stores.
---
# Privacy

## This website

No cookies, no tracking, no fonts or scripts from third parties. The only thing your browser may remember is a small note on your own device that you closed the language hint. The site is delivered by GitHub Pages; GitHub technically processes connection data such as the IP address (see GitHub's privacy statement). {{todo:have the controller review and add a full privacy notice (GDPR Art. 13) here}}

## The relay server

The relay server (<code>cometcast-server</code>) helps a device and a screen find each other: room codes and connection offers. It stores no pictures, files or media and does not forward them. Rooms live in memory only. To protect itself it sees and counts the address of the connecting device (for limits and temporary bans, also in memory only).

By default the apps use the public relay <code>relay.cometcast.de</code>. Its operations files are in the repository <a href="{{RELAY}}">CometCast-Relay</a>. If your organisation runs its own server with a TURN relay, the picture travels through that machine inside the organisation when a direct connection is not possible. See <a href="/self-hosting/">Host it yourself</a>.

## What stays on your devices

Picture, sound, files and links go directly from your device to the screen, encrypted. Screens nearby are found in your own network and never leave it.
