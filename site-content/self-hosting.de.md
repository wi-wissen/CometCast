---
title: Selbst hosten
description: Betreibe deinen eigenen Comet-Cast-Server, mit der Web-App darin, für deine Schule, Firma oder deinen Verein.
---
# Selbst hosten

Ein eigener Server hält alles in deiner Organisation: Raumcodes, die Web-App und, wenn du willst, auch das Bild selbst.

<h2 id="wann">Wann braucht ihr einen eigenen Server?</h2>

Ohne eigenen Server nutzt Comet Cast das öffentliche Relay. Es hilft Geräten nur, sich zu finden; das Bild geht direkt zwischen ihnen. Das reicht, wenn die Geräte sich erreichen: im selben WLAN und oft auch über das Internet.

Einen eigenen Server braucht ihr, wenn Leute aus einem **anderen Netz** präsentieren als dem des Bildschirms. Der klassische Fall ist **Gäste-WLAN gegenüber internem Netz**. Diese Netze sind meist absichtlich getrennt, und viele Firewalls lassen die direkte Verbindung nicht durch. Euer eigener Server mit TURN-Relay, den beide Netze erreichen (zum Beispiel in der DMZ), trägt das Bild zwischen ihnen. Das klappt immer, das Bild bleibt in eurer Organisation, und niemand muss sein Gerät einrichten: Die Bildschirme zeigen die Adresse und den QR-Code eures Servers.

Ohne IT können sich die Leute stattdessen mit dem WLAN des Bildschirms verbinden, wenn sie dürfen.

## Ein Container reicht

Der Server ist ein Docker-Image mit der Web-App darin, ein Container reicht also auch ohne Internet. Er ist auch ein einzelnes Programm, das du direkt starten kannst. Es gibt zwei Modi: <em>offen</em> (öffentlich, hilft Geräten nur sich zu finden, mit einer kleinen Rechenaufgabe zum Anlegen eines Raums) und <em>intern</em> (für die Geräte einer Organisation, mit TURN-Zugang, damit das Bild in deinem Netz bleibt).

## Wo du anfängst

- <a href="{{GH}}/blob/main/deploy/README.md">Schnellstart mit Docker Compose</a> (englisch)
- <a href="{{GH}}/blob/main/bins/cometcast-server/README.md">Server-README</a>
- <a href="{{GH}}/blob/main/docs/server.md">Referenz: docs/server.md</a>

Wie die Teile zusammenspielen: <a href="/de/so-funktionierts/">Wie es funktioniert</a>.
