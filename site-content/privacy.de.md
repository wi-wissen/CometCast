---
title: Datenschutz
description: Was diese Website sammelt (nichts) und was der Comet-Cast-Relay-Server sieht und speichert.
---
# Datenschutz

## Diese Website

Keine Cookies, kein Tracking, keine Schriften oder Skripte von Dritten. Dein Browser merkt sich höchstens einen kleinen Hinweis auf deinem eigenen Gerät, dass du den Sprachhinweis geschlossen hast. Die Seite wird über GitHub Pages ausgeliefert; GitHub verarbeitet dabei technisch bedingt Verbindungsdaten wie die IP-Adresse (siehe die Datenschutzhinweise von GitHub). {{todo:Datenschutzerklärung nach Art. 13 DSGVO von der Verantwortlichen prüfen lassen und hier ergänzen}}

## Der Relay-Server

Der Relay-Server (<code>cometcast-server</code>) hilft einem Gerät und einem Bildschirm, sich zu finden: Raumcodes und Verbindungsangebote. Er speichert keine Bilder, Dateien oder Medien und leitet sie nicht weiter. Räume liegen nur im Arbeitsspeicher. Zum Schutz vor Missbrauch sieht und zählt er die Adresse des verbindenden Geräts (für Limits und zeitweise Sperren, ebenfalls nur im Speicher).

Standardmäßig benutzen die Apps das öffentliche Relay <code>relay.cometcast.de</code>. Seine Betriebsdateien liegen im Repository <a href="{{RELAY}}">CometCast-Relay</a>. Betreibt deine Organisation einen eigenen Server mit TURN-Relay, läuft das Bild über diesen Rechner innerhalb der Organisation, wenn keine direkte Verbindung möglich ist. Siehe <a href="/de/selbst-hosten/">Selbst hosten</a>.

## Was auf deinen Geräten bleibt

Bild, Ton, Dateien und Links gehen direkt von deinem Gerät zum Bildschirm, verschlüsselt. Bildschirme in der Nähe werden in deinem eigenen Netz gefunden und verlassen es nie.
