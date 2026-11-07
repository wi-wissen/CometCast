#!/usr/bin/env node
// SPDX-License-Identifier: GPL-3.0-or-later
// Generates the static pages in site/ (German at /, English at /en/) from the text below.
// Run: node tools/build-site.mjs   (no dependencies). The generated HTML is committed.
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { IMPRINT_REQUIRED, REPO_ROOT, loadBranding, missingImprint } from './branding.mjs';

// Publisher identity and imprint data come from the branding file (COMETCAST_BRANDING, default branding/default.json;
// branding/README.md); the text of the privacy and imprint pages is in site-content/*.md.
//   COMETCAST_PUBLIC_BUILD=1   fail while required imprint fields are empty (the guard "fill the imprint before enabling Pages")
//   --out DIR                  write there instead of site/ (used by the tests)
const oi = process.argv.indexOf('--out');
const root = oi >= 0 ? resolve(process.argv[oi + 1]) : join(REPO_ROOT, 'site');
const { file: brandingFile, branding } = loadBranding();
const PUBLIC_BUILD = process.env.COMETCAST_PUBLIC_BUILD === '1';
const missing = missingImprint(branding);
if (PUBLIC_BUILD && missing.length) {
  console.error(`build-site: PUBLIC BUILD REFUSED. The imprint is not filled in: ${missing.map((k) => `imprint.${k}`).join(', ')} (required: ${IMPRINT_REQUIRED.map((k) => `imprint.${k}`).join(', ')}).`);
  console.error(`Fill these in ${brandingFile} (see branding/README.md and docs/website.md), or select your own file with COMETCAST_BRANDING. Never publish the site without an imprint.`);
  process.exit(1);
}
const incomplete = missing.length > 0;
// The imprint page exists when the branding names a publisher or any imprint detail; the default has neither.
const hasImprint = Boolean(branding.publisher_name) || Object.values(branding.imprint).some(Boolean);
const ORIGIN = 'https://cometcast.de';
const GH = branding.source_url.replace(/\/$/, '');
const RELAY = 'https://github.com/wi-wissen/CometCast-Relay';
const pub = branding.publisher_name;
const esc = (v) => String(v).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
const todo = (s) => `<span class="todo">TODO: ${s}</span>`;

// slug per page and language ('' = start page)
const pages = ['start', 'how', 'downloads', 'privacy', 'open', 'contact', ...(hasImprint ? ['imprint'] : [])];
const slug = {
  de: { start: '', how: 'funktionsweise/', downloads: 'downloads/', privacy: 'datenschutz/', open: 'open-source/', contact: 'kontakt/', imprint: 'impressum/' },
  en: { start: '', how: 'how-it-works/', downloads: 'downloads/', privacy: 'privacy/', open: 'open-source/', contact: 'contact/', imprint: 'imprint/' },
};
const url = (lang, p) => (lang === 'de' ? '/' : '/en/') + slug[lang][p];

const ui = {
  de: {
    nav: { start: 'Start', how: 'Funktionsweise', downloads: 'Downloads', privacy: 'Datenschutz & Selbst hosten', open: 'Open Source', contact: 'Kontakt' },
    imprint: 'Impressum', skip: 'Zum Inhalt', other: 'English', otherLabel: 'Switch to English',
    foot: 'Comet Cast ist freie Software (GPL-3.0-or-later). Diese Seite setzt keine Cookies und lädt nichts von Dritten.',
    logoAlt: 'Comet Cast Logo: ein Komet in Form eines C',
  },
  en: {
    nav: { start: 'Home', how: 'How it works', downloads: 'Downloads', privacy: 'Privacy & self-hosting', open: 'Open source', contact: 'Contact' },
    imprint: 'Imprint', skip: 'Skip to content', other: 'Deutsch', otherLabel: 'Auf Deutsch wechseln',
    foot: 'Comet Cast is free software (GPL-3.0-or-later). This site sets no cookies and loads nothing from third parties.',
    logoAlt: 'Comet Cast logo: a comet shaped like a C',
  },
};

const content = {};

content.start = {
  de: {
    title: 'Comet Cast: eine App zum Senden und Empfangen',
    desc: 'Comet Cast schickt Bildschirm, Dokumente, Links und Medien auf Apple TV, Google TV, Mini-PCs und Browser. Eine App ist Sender und Empfänger.',
    body: `
<div class="hero"><picture><source srcset="/assets/cometcast-logo-dark.svg" media="(prefers-color-scheme: dark)"><img src="/assets/cometcast-logo-light.svg" alt="" width="112" height="112"></picture><div>
<h1>Eine App. Sender und Empfänger.</h1>
<p class="lead">Comet Cast schickt Bildschirm, Dokumente, Links und Medien an Apple TV, Google TV / Android TV, Mini-PCs und Browser. Gedacht für Büros, Besprechungen, Vereine, Workshops und Schulen, in denen viele verschiedene Geräte in einem Raum zusammenkommen.</p>
<div class="btns"><a class="btn primary" href="/app/">Web-App öffnen</a><a class="btn" href="/downloads/">Downloads &amp; Status</a><a class="btn" href="/funktionsweise/">Wie es funktioniert</a></div>
<p><small>Die Web-App liegt unter <code>/app/</code>. Sie braucht einen Relay-Server; der Stand steht unter <a href="/downloads/">Downloads</a>.</small></p>
</div></div>
<div class="note"><p><strong>Stand: Beta 0.1, in Entwicklung.</strong> Vieles funktioniert bisher nur auf einem Rechner oder in einem lokalen Netz. Was geprüft ist und was nicht, steht ehrlich auf der Seite <a href="/downloads/">Downloads</a>.</p></div>
<h2>Was Comet Cast ist</h2>
<div class="cards">
<div class="card"><h3>Eine App für beide Seiten</h3><p>Dieselbe App sendet und empfängt. Die Idee ähnelt LocalSend, aber zum Präsentieren auf einem Bildschirm.</p></div>
<div class="card"><h3>So wenige Installationen wie möglich</h3><p>Die Seite mit den weniger Geräten bekommt die App. Die andere Seite nimmt einen Browser oder eine Funktion, die das Gerät schon hat.</p></div>
<div class="card"><h3>Viele Inhalte</h3><p>Bildschirm, PDF-Dokumente, Bilder, Links und Medien. Auf dem Empfänger gibt es eine Werkzeugleiste (Stift, Marker, Laserpointer, Standbild, Schwarzbild).</p></div>
</div>
<h2>Wohin man senden kann</h2>
<ul><li>Browser (Sender und Empfänger, ohne Installation)</li><li>Mini-PCs und Desktops (Linux, Windows, macOS)</li><li>Android TV / Google TV</li><li>Apple TV (nativer Empfänger)</li></ul>
<p>Nicht alles davon läuft schon. Der Status je Plattform steht in der <a href="/downloads/#status">Tabelle</a>.</p>
<h2>Verbindung per Raumcode</h2>
<p>Der Empfänger zeigt einen Raumcode (oder QR-Code), der Sender gibt ihn ein. Danach wird direkt verbunden, wenn das Netz es zulässt. <a href="/funktionsweise/">Mehr dazu</a>.</p>`,
  },
  en: {
    title: 'Comet Cast: one app to send and receive',
    desc: 'Comet Cast sends your screen, documents, links and media to Apple TV, Google TV, mini PCs and browsers. One app is sender and receiver.',
    body: `
<div class="hero"><picture><source srcset="/assets/cometcast-logo-dark.svg" media="(prefers-color-scheme: dark)"><img src="/assets/cometcast-logo-light.svg" alt="" width="112" height="112"></picture><div>
<h1>One app. Sender and receiver.</h1>
<p class="lead">Comet Cast sends your screen, documents, links and media to Apple TV, Google TV / Android TV, mini PCs and browsers. It is meant for offices, meetings, clubs, workshops and schools, where many different devices meet in one room.</p>
<div class="btns"><a class="btn primary" href="/app/">Open web app</a><a class="btn" href="/en/downloads/">Downloads &amp; status</a><a class="btn" href="/en/how-it-works/">How it works</a></div>
<p><small>The web app is at <code>/app/</code>. It needs a relay server; the current status is on <a href="/en/downloads/">Downloads</a>.</small></p>
</div></div>
<div class="note"><p><strong>Status: v0.1 beta, in development.</strong> Much of it has only been tried on one machine or one local network. The page <a href="/en/downloads/">Downloads</a> says honestly what is checked and what is not.</p></div>
<h2>What Comet Cast is</h2>
<div class="cards">
<div class="card"><h3>One app for both sides</h3><p>The same app sends and receives. The idea is similar to LocalSend, but for presenting on a display.</p></div>
<div class="card"><h3>As few installs as possible</h3><p>The side with fewer devices gets the app. The other side uses a browser or a system feature it already has.</p></div>
<div class="card"><h3>Many kinds of content</h3><p>Screen, PDF documents, images, links and media. The receiver has a toolbar (pen, marker, laser, freeze, black screen).</p></div>
</div>
<h2>Where you can send to</h2>
<ul><li>Browsers (sender and receiver, no install)</li><li>Mini PCs and desktops (Linux, Windows, macOS)</li><li>Android TV / Google TV</li><li>Apple TV (native receiver)</li></ul>
<p>Not all of this runs yet. The status per platform is in the <a href="/en/downloads/#status">table</a>.</p>
<h2>Connecting with a room code</h2>
<p>The receiver shows a room code (or QR code) and the sender enters it. Then the devices connect directly if the network allows it. <a href="/en/how-it-works/">More</a>.</p>`,
  },
};

content.how = {
  de: {
    title: 'Funktionsweise: Raumcode und Verbindungswege',
    desc: 'Wie Comet Cast Geräte verbindet: Raumcode, direkte Verbindung, Server mit TURN oder das eigene WLAN des Empfängers.',
    body: `
<h1>Funktionsweise</h1>
<p class="lead">Geräte finden sich über einen Raumcode. Danach versucht Comet Cast die Verbindungswege der Reihe nach.</p>
<h2>1. Raumcode</h2>
<p>Der Empfänger erzeugt einen Raumcode, auch als QR-Code. Der Sender gibt ihn ein. Der Code ist kurz; der Server bremst Raten durch Limits und zeitweise Sperren.</p>
<h2>2. Verbindungswege, in dieser Reihenfolge</h2>
<ol class="steps">
<li><strong>Direkt.</strong> WebRTC zwischen den beiden Geräten.</li>
<li><strong>Über einen Server.</strong> Wenn direkt nicht geht: der Server einer Organisation mit TURN-Relay (und optional einem SFU).</li>
<li><strong>Eigenes WLAN.</strong> Wenn auch das nicht geht: das WLAN des Empfängers.</li>
</ol>
<div class="note"><p>Die Reihenfolge ist der Plan. Gemessen ist bisher nur wenig: Verbindungen im lokalen Netz und ein TURN-Test auf einem einzigen Rechner. Über echte Netze, NAT oder zwischen zwei Geräten über TURN ist noch nicht getestet, ob und wie gut es funktioniert.</p></div>
<h2>Wie die App den Server findet</h2>
<ol class="steps"><li>MDM-Richtlinie</li><li>DNS-SRV-Eintrag <code>_cometcast._tcp</code></li><li>Ankündigung des Anzeigegeräts</li><li>Eingebaute Standardliste</li></ol>
<p>Parallel läuft mDNS für die lokale Suche. Die Server-Suche in den Apps ist zum Teil noch nicht gebaut.</p>
<h2>Was der Server tut</h2>
<p>Der Server vermittelt nur: Räume und Signalisierung. Der offene Server speichert und leitet keine Bilder, Dateien oder Medien weiter. Der Bildschirm läuft direkt zwischen den Geräten oder, wenn nötig, über TURN. Mehr dazu auf der Seite <a href="/datenschutz/">Datenschutz &amp; Selbst hosten</a>.</p>
<h2>Technische Details</h2>
<p>Architektur: <a href="${GH}/blob/main/docs/architecture.md">docs/architecture.md</a>, Protokoll: <a href="${GH}/blob/main/docs/protocol.md">docs/protocol.md</a>, offene Annahmen: <a href="${GH}/blob/main/docs/measurement-points.md">docs/measurement-points.md</a> (englisch).</p>`,
  },
  en: {
    title: 'How it works: room code and connection routes',
    desc: 'How Comet Cast connects devices: room code, direct connection, a server with TURN, or the receiver\'s own Wi-Fi.',
    body: `
<h1>How it works</h1>
<p class="lead">Devices find each other with a room code. Then Comet Cast tries the connection routes in order.</p>
<h2>1. Room code</h2>
<p>The receiver creates a room code, also shown as a QR code. The sender enters it. The code is short; the server slows guessing with limits and temporary bans.</p>
<h2>2. Connection routes, in this order</h2>
<ol class="steps">
<li><strong>Direct.</strong> WebRTC between the two devices.</li>
<li><strong>Through a server.</strong> If direct fails: an organisation's server with a TURN relay (and optionally an SFU).</li>
<li><strong>Own Wi-Fi.</strong> If that fails too: the receiver's own Wi-Fi.</li>
</ol>
<div class="note"><p>The order is the plan. Little is measured so far: connections on a local network and a TURN test on a single machine. Across real networks, through NAT, or between two devices over TURN it has not been tested yet whether and how well it works.</p></div>
<h2>How the app finds the server</h2>
<ol class="steps"><li>MDM policy</li><li>DNS-SRV record <code>_cometcast._tcp</code></li><li>Announcement from the display device</li><li>Built-in default list</li></ol>
<p>mDNS runs in parallel for local discovery. Parts of the server discovery in the apps are not built yet.</p>
<h2>What the server does</h2>
<p>The server only mediates: rooms and signalling. The open server stores and relays no images, files or media. The screen travels directly between the devices or, if needed, through TURN. More on <a href="/en/privacy/">Privacy &amp; self-hosting</a>.</p>
<h2>Technical details</h2>
<p>Architecture: <a href="${GH}/blob/main/docs/architecture.md">docs/architecture.md</a>, protocol: <a href="${GH}/blob/main/docs/protocol.md">docs/protocol.md</a>, open assumptions: <a href="${GH}/blob/main/docs/measurement-points.md">docs/measurement-points.md</a>.</p>`,
  },
};

// Status rows taken from docs/architecture.md and README.md (state of 2026-11-07).
const rows = [
  ['ok', 'Browser: Sender und Empfänger (Bildschirm über WebRTC, Raumcode, Werkzeugleiste, Bilder, Links, PDF)', 'Browser: sender and receiver (screen over WebRTC, room code, toolbar, images, links, PDF)', 'Funktioniert, aber nur auf einem Rechner / im lokalen Netz geprüft (PDF und Medienlinks nur mit Chromium auf Linux). Bildschirmfreigabe braucht HTTPS und einen Desktop-Browser; Mobil ist ungeprüft.', 'Works, but checked only on one machine / a local network (PDF and media links only with Chromium on Linux). Screen sharing needs HTTPS and a desktop browser; mobile is unchecked.'],
  ['part', 'Signalisierungsserver (<code>cometcast-server</code>)', 'Signalling server (<code>cometcast-server</code>)', 'Gebaut und getestet. Erster echter Lauf auf einer Linux-Maschine: Docker-Image, Compose, WSS, coturn mit TURN-Zugangsdaten. Nicht geprüft: zwei echte Geräte, NAT, TURN über TLS, IPv6, echtes Zertifikat, Last, arm64.', 'Built and tested. First real run on one Linux machine: Docker image, Compose, WSS, coturn with TURN credentials. Not verified: two real devices, NAT, TURN over TLS, IPv6, a real certificate, load, arm64.'],
  ['no', 'Öffentlicher Relay-Server <code>relay.cometcast.de</code>', 'Public relay server <code>relay.cometcast.de</code>', 'Existiert noch nicht.', 'Does not exist yet.'],
  ['part', 'Native Rust-Peers und Kommandozeile (<code>cometcast-cli</code>)', 'Native Rust peers and command line tool (<code>cometcast-cli</code>)', 'Reden mit der Browser-App; Entwicklerwerkzeug. Linux-Bildschirmaufnahme (X11) mit H.264 vorhanden; Wayland, Windows, macOS nicht.', 'Talk to the browser app; a developer tool. X11 screen capture with H.264 exists on Linux; Wayland, Windows and macOS do not.'],
  ['part', 'Flutter-Desktop-App (Linux)', 'Flutter desktop app (Linux)', 'Nutzt den Rust-Kern, hat die Werkzeugleiste des Web-Empfängers. Nur Linux getestet. Bildschirmaufnahme und Videoanzeige in den nativen Apps sind noch nicht fertig.', 'Uses the Rust core and has the web receiver\'s toolbar. Only Linux tested. Screen capture and video display in the native apps are not finished.'],
  ['no', 'Windows und macOS App', 'Windows and macOS app', 'Nicht getestet. Aufnahme dort nicht gebaut.', 'Untested. Capture not built there.'],
  ['part', 'AirPlay empfangen (über UxPlay)', 'AirPlay receiving (through UxPlay)', 'Der Steuerprozess läuft mit dem echten UxPlay unter Linux. Kein Test mit iPhone oder iPad; keine App-Integration; Windows fehlt.', 'The supervisor runs with the real UxPlay on Linux. No test with an iPhone or iPad; not integrated in the app; Windows missing.'],
  ['no', 'Android und Android TV / Google TV', 'Android and Android TV / Google TV', 'Eine Debug-APK baut. Auf keinem Gerät ausgeführt.', 'A debug APK builds. Nothing ran on a device.'],
  ['no', 'Apple TV (Empfänger)', 'Apple TV (receiver)', 'Der Quellcode liegt im Repository, funktioniert aber noch nicht.', 'The source is in the repository, but it does not work yet.'],
  ['no', 'Miracast empfangen (Windows 11)', 'Miracast receiving (Windows 11)', 'Nur kompiliert, nie auf Windows ausgeführt.', 'Compile-checked only, never run on Windows.'],
  ['no', 'Google Cast als Sender', 'Google Cast sender', 'Nur gegen einen eigenen Test-Empfänger auf demselben Rechner geprüft, nie gegen einen echten Chromecast.', 'Tested only against our own fake receiver on one machine, never against a real Chromecast.'],
  ['part', 'AirPlay als Sender (ohne FairPlay)', 'AirPlay sender (without FairPlay)', 'Suche und Kopplung gegen UxPlay geprüft; alles andere nur gegen eigenen Test. Nichts auf einem Apple TV. Echte Bildschirmspiegelung ist wegen FairPlay bewusst nicht gebaut.', 'Discovery and pairing checked against UxPlay; everything else only against our own fake. Nothing on an Apple TV. Real mirroring is deliberately not built (FairPlay).'],
  ['no', 'iOS-Broadcast-Erweiterung', 'iOS broadcast extension', 'Nicht begonnen.', 'Not started.'],
  ['no', 'STUN/TURN über echte Netze', 'STUN/TURN across real networks', 'Nicht geprüft.', 'Not verified.'],
];
const stLabel = { de: { ok: 'läuft (eingeschränkt)', part: 'teilweise', no: 'läuft noch nicht' }, en: { ok: 'works (limited)', part: 'partly', no: 'not working yet' } };
const statusTable = (lang) => {
  const i = lang === 'de' ? 0 : 1;
  const h = lang === 'de' ? ['Bereich', 'Status', 'Was geprüft ist'] : ['Area', 'Status', 'What is checked'];
  return `<div class="tablewrap"><table><thead><tr><th>${h[0]}</th><th>${h[1]}</th><th>${h[2]}</th></tr></thead><tbody>${rows
    .map((r) => `<tr><td>${r[1 + i]}</td><td class="st ${r[0]}">${stLabel[lang][r[0]]}</td><td>${r[3 + i]}</td></tr>`)
    .join('')}</tbody></table></div>`;
};

content.downloads = {
  de: {
    title: 'Downloads und Status',
    desc: 'Downloads von Comet Cast über GitHub Releases und eine ehrliche Statustabelle: was funktioniert und was ungeprüft ist.',
    body: `
<h1>Downloads und Status</h1>
<p class="lead">Comet Cast ist eine Beta (0.1). Diese Seite sagt, was funktioniert und was nicht.</p>
<h2>Downloads</h2>
<div class="btns"><a class="btn primary" href="${GH}/releases/latest">Neueste Version auf GitHub Releases</a><a class="btn" href="/app/">Web-App öffnen</a></div>
<div class="note"><p>Zum Zeitpunkt dieser Seite (2026-11-07) ist auf GitHub <strong>noch keine Version veröffentlicht</strong>. Der Link führt dann zu einer leeren oder fehlenden Seite. Dateien gibt es erst, wenn eine Version dort erscheint. Wer jetzt testen will, baut aus dem Quellcode (siehe <a href="${GH}#build-and-run">README</a>).</p>
<p>Die Web-App liegt unter <code>/app/</code>. Sie besteht aus statischen Dateien und wird von dieser Seite ausgeliefert; sie spricht per <code>wss</code> mit einem Relay-Server. Die Anbindung an <code>relay.cometcast.de</code> ist noch nicht fertig, und dieser Server existiert noch nicht.</p></div>
<h2 id="status">Statustabelle (Stand 2026-11-07)</h2>
<p>Quelle: <a href="${GH}/blob/main/docs/architecture.md#milestones">docs/architecture.md</a> und README. Nichts darüber hinaus ist behauptet. Was nicht in der Tabelle steht, ist ein Plan.</p>
${statusTable('de')}
<p>Die vollständige Liste ungeprüfter Annahmen: <a href="${GH}/blob/main/docs/measurement-points.md">docs/measurement-points.md</a> (englisch).</p>`,
  },
  en: {
    title: 'Downloads and status',
    desc: 'Download Comet Cast through GitHub Releases, with an honest status table of what works and what is untested.',
    body: `
<h1>Downloads and status</h1>
<p class="lead">Comet Cast is a beta (0.1). This page says what works and what does not.</p>
<h2>Downloads</h2>
<div class="btns"><a class="btn primary" href="${GH}/releases/latest">Latest release on GitHub Releases</a><a class="btn" href="/app/">Open web app</a></div>
<div class="note"><p>When this page was written (2026-11-07) <strong>no release is published</strong> on GitHub yet. The link then leads to an empty or missing page. There are files only once a release appears there. To try it now, build from source (see the <a href="${GH}#build-and-run">README</a>).</p>
<p>The web app is at <code>/app/</code>. It consists of static files and is served by this site; it talks to a relay server over <code>wss</code>. The connection to <code>relay.cometcast.de</code> is not finished, and that server does not exist yet.</p></div>
<h2 id="status">Status table (as of 2026-11-07)</h2>
<p>Source: <a href="${GH}/blob/main/docs/architecture.md#milestones">docs/architecture.md</a> and the README. Nothing beyond that is claimed. What is not in the table is a plan.</p>
${statusTable('en')}
<p>The complete list of unverified assumptions: <a href="${GH}/blob/main/docs/measurement-points.md">docs/measurement-points.md</a>.</p>`,
  },
};

content.open = {
  de: {
    title: 'Open Source und Lizenz',
    desc: 'Comet Cast steht unter GPL-3.0-or-later. Beiträge laufen über eine CLA.',
    body: `
<h1>Open Source</h1>
<p class="lead">Comet Cast ist freie Software unter <strong>GPL-3.0-or-later</strong>. Quellcode: <a href="${GH}">github.com/wi-wissen/CometCast</a>.</p>
<h2>Lizenz</h2>
<p>Text der Lizenz: <a href="${GH}/blob/main/LICENSE">LICENSE</a>. Komponenten Dritter: <a href="${GH}/blob/main/THIRD_PARTY_NOTICES.md">THIRD_PARTY_NOTICES.md</a>.</p>
<p>Die GPL deckt den Code. Sie gibt für sich keine Rechte am Namen „Comet Cast“ oder am Logo. Eine Namens- und Logo-Richtlinie gibt es noch nicht. Das ist ein Überblick, keine Rechtsberatung.</p>
<h2>Mitmachen und CLA</h2>
<p>Beiträge Dritter werden unter der <a href="${GH}/blob/main/CLA.md">CLA</a> angenommen. Sie erlaubt der Betreuerin${pub ? `, der ${pub},` : ''} die Apps in App-Stores zu verteilen; die öffentliche Version bleibt GPL-3.0-or-later. Der CLA-Text ist ein Entwurf und braucht noch eine juristische Prüfung. Siehe <a href="${GH}/blob/main/CONTRIBUTING.md">CONTRIBUTING.md</a> und den <a href="${GH}/blob/main/CODE_OF_CONDUCT.md">Verhaltenskodex</a>.</p>
<h2>Was wir bewusst nicht tun</h2>
<ul><li>Keine Schlüssel oder Zertifikate extrahieren, kein FairPlay-, AirServer- oder AirParrot-Material</li><li>Keine DRM-Umgehung</li><li>Kein Cast-Empfang in Version 1, kein AirDrop, kein DLNA/UPnP, kein Miracast-Sender</li></ul>`,
  },
  en: {
    title: 'Open source and licence',
    desc: 'Comet Cast is licensed under GPL-3.0-or-later. Contributions go through a CLA.',
    body: `
<h1>Open source</h1>
<p class="lead">Comet Cast is free software under <strong>GPL-3.0-or-later</strong>. Source: <a href="${GH}">github.com/wi-wissen/CometCast</a>.</p>
<h2>Licence</h2>
<p>Licence text: <a href="${GH}/blob/main/LICENSE">LICENSE</a>. Third-party components: <a href="${GH}/blob/main/THIRD_PARTY_NOTICES.md">THIRD_PARTY_NOTICES.md</a>.</p>
<p>The GPL covers the code. It does not by itself grant rights to the name "Comet Cast" or the logo. A naming and logo policy is not written yet. This is an overview, not legal advice.</p>
<h2>Contributing and the CLA</h2>
<p>Contributions from third parties are accepted under the <a href="${GH}/blob/main/CLA.md">CLA</a>. It allows the steward${pub ? `, ${pub},` : ''} to distribute the apps in app stores; the public version stays GPL-3.0-or-later. The CLA text is a draft that still needs a lawyer's review. See <a href="${GH}/blob/main/CONTRIBUTING.md">CONTRIBUTING.md</a> and the <a href="${GH}/blob/main/CODE_OF_CONDUCT.md">Code of Conduct</a>.</p>
<h2>What we deliberately do not do</h2>
<ul><li>No extraction of keys or certificates, no FairPlay, AirServer or AirParrot material</li><li>No DRM circumvention</li><li>No Cast receiving in v1, no AirDrop, no DLNA/UPnP, no Miracast sender</li></ul>`,
  },
};

content.contact = {
  de: {
    title: 'Kontakt und Sicherheit',
    desc: 'Fehler melden, Sicherheitslücken privat melden, mitmachen.',
    body: `
<h1>Kontakt und Sicherheit</h1>
<h2>Fehler und Ideen</h2>
<p>Bitte als Issue auf GitHub: <a href="${GH}/issues">github.com/wi-wissen/CometCast/issues</a>.</p>
<h2>Sicherheitslücken</h2>
<p>Bitte <strong>kein</strong> öffentliches Issue. Nutze die private Meldung von GitHub: im Repository den Reiter <em>Security</em> und dann <em>Report a vulnerability</em>. Wird das nicht angezeigt, eröffne ein Issue, das nur sagt, dass du einen privaten Weg brauchst, ohne Einzelheiten. Es gibt noch keine festen Antwortzeiten. Die ganze Richtlinie: <a href="${GH}/blob/main/SECURITY.md">SECURITY.md</a> (englisch).</p>
<h2>E-Mail</h2>
<p>${branding.contact ? esc(branding.contact) : todo('Kontakt-E-Mail eintragen, falls gewünscht')}</p>`,
  },
  en: {
    title: 'Contact and security',
    desc: 'Report bugs, report vulnerabilities privately, get involved.',
    body: `
<h1>Contact and security</h1>
<h2>Bugs and ideas</h2>
<p>Please open an issue on GitHub: <a href="${GH}/issues">github.com/wi-wissen/CometCast/issues</a>.</p>
<h2>Vulnerabilities</h2>
<p>Please do <strong>not</strong> open a public issue. Use GitHub's private reporting: open the <em>Security</em> tab of the repository and choose <em>Report a vulnerability</em>. If that is not shown, open an issue that says only that you need a private channel, without details. There are no fixed response times yet. The full policy: <a href="${GH}/blob/main/SECURITY.md">SECURITY.md</a>.</p>
<h2>Email</h2>
<p>${branding.contact ? esc(branding.contact) : todo('add a contact email address if wanted')}</p>`,
  },
};

// ---- pages from site-content/*.md ----------------------------------------------------------------
// A small block format (no dependencies): front matter (title, description, optional title_incomplete /
// description_incomplete for the placeholder state), `# ` / `## ` headings, paragraphs (lines joined with <br>),
// `- ` lists, `> ` notes; inline HTML passes through. `{{GH}}` `{{RELAY}}` `{{publisher_name}}` `{{contact}}`
// `{{imprint.x|hint}}` (the value; empty: a TODO marker with the hint while the imprint is incomplete, else nothing)
// `{{todo:text}}`; `{{#if a|b}}` ... `{{/if}}` on lines of their own (true when one of the names is set; `incomplete`
// is true while required imprint fields are empty).
const lookup = (name) => (name === 'incomplete' ? incomplete : name.startsWith('imprint.') ? branding.imprint[name.slice(8)] : name === 'publisher_name' || name === 'contact' ? branding[name] : undefined);

function renderPage(name, lang) {
  const f = join(REPO_ROOT, 'site-content', `${name}.${lang}.md`);
  const m = /^---\n([\s\S]*?)\n---\n([\s\S]*)$/.exec(readFileSync(f, 'utf8'));
  if (!m) throw new Error(`${f}: front matter missing`);
  const fm = Object.fromEntries(m[1].split('\n').map((l) => [l.slice(0, l.indexOf(':')), l.slice(l.indexOf(':') + 1).trim()]));
  const lines = [];
  const stack = [];
  for (const raw of m[2].split('\n')) {
    const c = /^\{\{#if ([^}]+)\}\}$/.exec(raw.trim());
    if (c) { stack.push(c[1].split('|').some((n) => Boolean(lookup(n.trim())))); continue; }
    if (raw.trim() === '{{/if}}') { stack.pop(); continue; }
    if (stack.includes(false)) continue;
    let hadToken = false;
    const text = raw.replace(/\{\{([^}]+)\}\}/g, (_, t) => {
      hadToken = true;
      if (t === 'GH') return GH;
      if (t === 'RELAY') return RELAY;
      if (t.startsWith('todo:')) return todo(t.slice(5));
      const [key, hint] = t.split('|');
      const v = lookup(key.trim());
      if (v === undefined) throw new Error(`${f}: unknown placeholder {{${t}}}`);
      return v ? esc(v) : hint !== undefined && incomplete ? todo(hint) : '';
    });
    if (hadToken && !text.trim()) continue;
    lines.push(text);
  }
  const blocks = [];
  let cur = null;
  const flush = () => {
    if (!cur) return;
    if (cur.t === 'ul') blocks.push(`<ul>\n${cur.l.map((x) => `<li>${x}</li>`).join('\n')}\n</ul>`);
    else if (cur.t === 'note') blocks.push(`<div class="note"><p>${cur.l.join(' ')}</p></div>`);
    else blocks.push(`<p>${cur.l.join('<br>')}</p>`);
    cur = null;
  };
  for (const l of lines) {
    const h = /^(#{1,2}) (.*)$/.exec(l);
    if (!l.trim()) flush();
    else if (h) { flush(); blocks.push(`<h${h[1].length}>${h[2]}</h${h[1].length}>`); }
    else {
      const t = l.startsWith('- ') ? 'ul' : l.startsWith('> ') ? 'note' : 'p';
      if (!cur || cur.t !== t) { flush(); cur = { t, l: [] }; }
      cur.l.push(t === 'p' ? l : l.slice(2));
    }
  }
  flush();
  const pick = (k) => (incomplete && fm[`${k}_incomplete`]) || fm[k];
  return { title: pick('title'), desc: pick('description'), body: `\n${blocks.join('\n')}` };
}
for (const p of ['privacy', 'imprint']) {
  if (p === 'imprint' && !hasImprint) continue;
  content[p] = { de: renderPage(p, 'de'), en: renderPage(p, 'en') };
}


function layout(lang, p, c) {
  const t = ui[lang];
  const other = lang === 'de' ? 'en' : 'de';
  const self = url(lang, p);
  const nav = ['start', 'how', 'downloads', 'privacy', 'open', 'contact']
    .map((k) => `<a href="${url(lang, k)}"${k === p ? ' aria-current="page"' : ''}>${t.nav[k]}</a>`)
    .join('');
  const noindex = p === 'imprint' ? '\n<meta name="robots" content="noindex">' : '';
  const alt = (l) => `<link rel="alternate" hreflang="${l}" href="${ORIGIN}${url(l, p)}">`;
  return `<!doctype html>
<html lang="${lang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${c.title}</title>
<meta name="description" content="${c.desc.replace(/"/g, '&quot;')}">${noindex}
<link rel="canonical" href="${ORIGIN}${self}">
${alt('de')}
${alt('en')}
<link rel="alternate" hreflang="x-default" href="${ORIGIN}${url('de', p)}">
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<link rel="apple-touch-icon" href="/apple-touch-icon.png">
<meta name="theme-color" content="#eceef2" media="(prefers-color-scheme: light)">
<meta name="theme-color" content="#111318" media="(prefers-color-scheme: dark)">
<link rel="stylesheet" href="/assets/style.css">
</head>
<body>
<a class="skip" href="#main">${t.skip}</a>
<header class="site"><div class="wrap">
<a class="brand" href="${url(lang, 'start')}"><picture><source srcset="/assets/cometcast-logo-dark.svg" media="(prefers-color-scheme: dark)"><img src="/assets/cometcast-logo-light.svg" alt="${t.logoAlt}" width="36" height="36"></picture><span>Comet Cast</span></a>
<nav class="main" aria-label="${lang === 'de' ? 'Hauptnavigation' : 'Main navigation'}">${nav}</nav>
<div class="lang"><a href="${url(other, p)}" hreflang="${other}" lang="${other}" title="${t.otherLabel}">${t.other}</a></div>
</div></header>
<main id="main"><div class="wrap">${c.body}
</div></main>
<footer class="site"><div class="wrap"><span>${t.foot}</span>${hasImprint ? `<a href="${url(lang, 'imprint')}">${t.imprint}</a>` : ''}<a href="${GH}">GitHub</a></div></footer>
</body>
</html>
`;
}

function write(rel, text) {
  const f = join(root, rel);
  mkdirSync(dirname(f), { recursive: true });
  writeFileSync(f, text);
}

const urls = [];
for (const lang of ['de', 'en']) {
  for (const p of pages) {
    const rel = (lang === 'de' ? '' : 'en/') + slug[lang][p] + 'index.html';
    write(rel, layout(lang, p, content[p][lang]));
    if (p !== 'imprint') urls.push(url(lang, p));
  }
}

// 404 (bilingual, root-relative links)
write('404.html', `<!doctype html>
<html lang="de">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>404 - Seite nicht gefunden / Page not found</title>
<meta name="robots" content="noindex">
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<link rel="stylesheet" href="/assets/style.css">
<script>
// GitHub Pages answers every unknown path with this page. Under /app/ that means a join link (/app/K7QM2W) or
// /app/display of the web app, which chooses its page by the path: show the app's index.html here, the address stays.
// Every other path keeps this 404 page. (docs/website.md)
(function () {
  if (!/^\\/app(\\/|$)/.test(location.pathname)) return;
  document.documentElement.style.visibility = 'hidden';
  fetch('/app/index.html', { credentials: 'omit' })
    .then(function (r) { if (!r.ok) throw new Error('no app'); return r.text(); })
    .then(function (html) { document.open(); document.write(html); document.close(); })
    .catch(function () { document.documentElement.style.visibility = ''; });
})();
</script>
</head>
<body>
<main id="main"><div class="wrap">
<div class="hero"><picture><source srcset="/assets/cometcast-logo-dark.svg" media="(prefers-color-scheme: dark)"><img src="/assets/cometcast-logo-light.svg" alt="" width="112" height="112"></picture><div>
<h1>404</h1>
<p class="lead">Diese Seite gibt es nicht. <a href="/">Zur Startseite</a>.</p>
<p lang="en" class="lead">This page does not exist. <a href="/en/">Back to the home page</a>.</p>
</div></div>
</div></main>
</body>
</html>
`);

// sitemap with hreflang alternates, robots
const pagesForMap = pages.filter((p) => p !== 'imprint');
const entry = (lang, p) =>
  `<url><loc>${ORIGIN}${url(lang, p)}</loc>` +
  ['de', 'en'].map((l) => `<xhtml:link rel="alternate" hreflang="${l}" href="${ORIGIN}${url(l, p)}"/>`).join('') +
  `</url>`;
write('sitemap.xml', `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">
${['de', 'en'].flatMap((l) => pagesForMap.map((p) => entry(l, p))).join('\n')}
</urlset>
`);
write('robots.txt', `User-agent: *\nAllow: /\n\nSitemap: ${ORIGIN}/sitemap.xml\n`);
write('CNAME', 'cometcast.de\n');
write('.nojekyll', '');
console.log(`wrote ${pages.length * 2} pages + 404 under site/`);
