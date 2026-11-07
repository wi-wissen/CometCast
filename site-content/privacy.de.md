---
title: Datenschutz und Selbst hosten
description: Was der Comet-Cast-Relay-Server sieht, wie man ihn selbst betreibt, und was diese Webseite sammelt (nichts).
---
# Datenschutz und Selbst hosten

## Diese Webseite

Keine Cookies, kein Tracking, keine Schriften oder Skripte von Dritten. Die Seite wird über GitHub Pages ausgeliefert; GitHub verarbeitet dabei technisch bedingt Verbindungsdaten wie die IP-Adresse (siehe die Datenschutzhinweise von GitHub). {{todo:Datenschutzerklärung nach Art. 13 DSGVO von der Verantwortlichen prüfen lassen und hier ergänzen}}

## Der Relay-Server

Der Relay-Server (<code>cometcast-server</code>) vermittelt zwischen Sender und Empfänger: Raumcodes und Verbindungsangebote (Signalisierung). Er speichert keine Bilder, Dateien oder Medien und leitet sie nicht weiter. Räume liegen nur im Arbeitsspeicher. Zum Schutz vor Missbrauch sieht und zählt er die Adresse des Clients (für Limits und zeitweise Sperren, ebenfalls nur im Speicher). Mit einem TURN-Relay (coturn, nur auf einem internen Server) läuft der Datenstrom bei Bedarf über diesen Rechner.

Ein öffentlicher Server <code>relay.cometcast.de</code> ist geplant, <strong>existiert aber noch nicht</strong>. Betriebsdateien und Anleitung für diesen Host liegen im Repository <a href="{{RELAY}}">CometCast-Relay</a>.

## Selbst hosten

Ein eigener Server ist ein Docker-Image (mit der Web-App darin, ein Container reicht auch ohne Internet) oder ein einzelnes Programm. Es gibt zwei Modi: <em>offen</em> (öffentlich, nur Vermittlung, ALTCHA-Rechenaufgabe zum Anlegen eines Raums) und <em>intern</em> (für die Geräte einer Organisation, mit TURN-Zugangsdaten).

- <a href="{{GH}}/blob/main/deploy/README.md">Schnellstart mit Docker Compose</a> (englisch)
- <a href="{{GH}}/blob/main/bins/cometcast-server/README.md">Server-README</a>
- <a href="{{GH}}/blob/main/docs/server.md">Ausführliche Referenz docs/server.md</a>

> Der Server ist nur auf einer Maschine geprüft. Nicht geprüft: zwei echte Geräte über TURN, NAT, echtes Zertifikat, Last. Alle Grenzwerte sind ungemessene Platzhalter.
