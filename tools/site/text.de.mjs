// SPDX-License-Identifier: GPL-3.0-or-later
// Alle Seitentexte der Website, Deutsch (informelles "du"). Dieselben Schlüssel wie text.en.mjs; tools/build-site.mjs bricht ab,
// wenn in einer Sprache ein Schlüssel oder ein HTML-Block fehlt. Rechtstexte und Selbst hosten: site-content/*.md.
export default function text({ GH, DOMAIN }) {
  return {
    ui: {
      skip: 'Zum Inhalt',
      mainNav: 'Hauptnavigation',
      footerNav: 'Fußbereich',
      brandAlt: 'Comet-Cast-Logo: ein Komet in Form eines C',
      whatYouNeed: 'Was du brauchst',
      download: 'Download',
      openApp: 'Web-App öffnen',
      switchText: 'EN',
      switchLabel: 'Read in English',
      footProduct: 'Produkt',
      footTrust: 'Vertrauen',
      footProject: 'Projekt',
      footApp: 'Web-App',
      footPrivacy: 'Datenschutz',
      footSelfhost: 'Selbst hosten',
      footOpen: 'Open Source',
      footHow: 'Wie es technisch funktioniert',
      footContact: 'Kontakt',
      footImprint: 'Impressum',
      footFine: 'Freie Software · keine Cookies · nichts von Dritten geladen',
    },
    // Shown on the English page when the browser prefers German (so: written in German).
    hint: { text: 'Diese Seite gibt es auch auf Deutsch.', link: 'Auf Deutsch lesen', close: 'Schließen' },
    home: {
      title: 'Comet Cast: zeig deinen Bildschirm auf jedem Bildschirm im Raum',
      desc: 'Schick den Bildschirm von Laptop, Handy oder Tablet, ein PDF, ein Foto oder einen Link auf den Fernseher oder Beamer im Raum. Oft ganz ohne etwas zu installieren.',
      h1: 'Zeig deinen Bildschirm auf <em>jedem</em> Bildschirm im Raum.',
      lead: 'Vom Laptop, Handy oder Tablet auf Fernseher oder Beamer. Oft ganz ohne etwas zu installieren.',
      ctaDownload: 'Download',
      ctaApp: 'Web-App öffnen',
      beta: 'Beta',
      tryIt: 'Einfach ausprobieren. Kostenlos, ohne Konto.',
      tv: {
        label: `Ein Bildschirm mit Comet Cast: ${DOMAIN} öffnen und den Code DXZ PNJ eingeben`,
        room: 'Besprechungsraum Elbe',
        pill: 'Mit Freigabe',
        step1: 'Im Browser öffnen',
        step2: 'Diesen Code eingeben',
        scan: 'Oder scannen',
        foot: 'Kein Download nötig.',
        settings: 'Einstellungen',
      },
      finderH2: 'Was brauchst du?',
      finderIntro: 'Wähle dein Gerät und wie der Bildschirm angeschlossen ist. Du siehst sofort, ob es geht und was du installieren musst.',
      stepsH2: 'Dann sind es drei Schritte',
      steps: [
        { h: 'Der Bildschirm zeigt einen Code', p: 'Dazu einen QR-Code und die Adresse zum Öffnen.' },
        { h: 'Du gibst ihn ein oder scannst ihn', p: 'Oder du tippst in der App auf den Bildschirm in der Liste in der Nähe.' },
        { h: 'Du präsentierst', p: 'Deinen Bildschirm, ein PDF, ein Foto oder einen Link. Brauchst du kurz Zeit? Drück Pause, und der Raum sieht das letzte Bild.' },
      ],
    },
    finder: {
      qHave: 'Ich habe',
      qNet: 'Wie seid ihr verbunden?',
      qScreen: 'Der Bildschirm hängt an',
      nets: {
        same: ['Im selben WLAN wie der Bildschirm', 'z. B. zu Hause'],
        org: ['In einem anderen WLAN vor Ort', 'z. B. Gäste-WLAN, Bildschirm im internen Netz'],
        away: ['Ganz woanders', 'mobile Daten oder anderer Standort'],
      },
      // Der Kasten über den Wegen: was diese Verbindung bedeutet, in Alltagssprache.
      netBox: {
        same: {
          h: 'Gleiches WLAN: Dein Gerät und der Bildschirm reden direkt miteinander.',
          p: ['Alle Wege unten gehen.', 'Eine Ausnahme: Manche Firmen- und Schulnetze trennen Geräte auch im selben WLAN. Klappt gar nichts, lies bei „anderes WLAN vor Ort“ weiter.'],
        },
        org: {
          h: 'Anderes WLAN: Die beiden Netze sind meist absichtlich getrennt.',
          p: ['Eingebaute Wege wie AirPlay, Cast oder Miracast gehen hier nicht. Comet Cast findet den Bildschirm trotzdem über seinen Code. Ob das Bild durchkommt, hängt von eurem Netz ab:'],
          li: [
            '<b>Zuverlässig:</b> Eure Organisation betreibt einen eigenen Comet-Cast-Server. Dann läuft das Bild über diesen Server und bleibt im Haus. Eure IT richtet ihn einmal ein, du musst nichts einstellen.',
            '<b>Ohne eigenen Server</b> klappt es nur, wenn euer Netz die Verbindung durchlässt. Kommt keine zustande, verbinde dich mit dem WLAN des Bildschirms, wenn du darfst.',
          ],
          more: 'Für eure IT: wann ihr einen eigenen Server braucht',
        },
        away: {
          h: 'Ganz woanders: über das Internet.',
          p: ['Eingebaute Wege gehen hier nicht. Comet Cast verbindet die beiden über das Internet. Das klappt meistens. Strenge Netze, etwa bei manchen Firmen oder Mobilfunkanbietern, brauchen den eigenen Server der Organisation.'],
          more: 'Für eure IT: wann ihr einen eigenen Server braucht',
        },
      },
      onDevice: 'Auf deinem Gerät',
      onScreen: 'Auf dem Bildschirm',
      chipNone: 'Nichts installieren',
      chipWeb: '{domain} öffnen',
      chipApp: 'Comet-Cast-App',
      chipAppOnScreen: 'Comet-Cast-App darauf',
      netBuiltinStatic: 'Nur im selben WLAN',
      tableNote: 'Eingebaute Wege (AirPlay, Cast, Miracast) gehen nur, wenn dein Gerät und der Bildschirm im selben WLAN sind. Comet Cast verbindet auch über Netze hinweg; zwischen einem Gäste-WLAN und einem internen Netz klappt das zuverlässig nur mit dem eigenen Server eurer Organisation.',
      verdictOne: 'Geht. So:',
      verdictMany: 'Geht. {n} Wege, der einfachste zuerst:',
      verdictNone: 'Mit Comet Cast geht das noch nicht.',
      verdictAcrossOne: 'Dieser Weg geht über Netze hinweg:',
      verdictAcrossMany: 'Diese {n} Wege gehen über Netze hinweg:',
      verdictNoNet: 'Für dieses Paar gibt es nur einen eingebauten Weg, und der braucht dasselbe WLAN. Verbinde dich mit dem WLAN des Bildschirms, wenn du darfst.',
      tableCaption: 'Alle Antworten: Gerät, Bildschirm und die Wege, der einfachste zuerst',
      tableTo: 'Zeigen auf',
      tableWays: 'Wege, der einfachste zuerst',
    },
    download: {
      title: 'Comet Cast herunterladen',
      desc: 'Comet Cast für Windows, Mac, Linux, Android, iPhone und iPad, Apple TV und Google TV, oder gleich die Web-App öffnen und nichts installieren.',
      eyebrow: 'Download',
      h1: 'Comet Cast holen',
      lead: 'Oft brauchst du gar nichts herunterzuladen: Öffne die Web-App einfach im Browser. Die Apps gibt es für den Fall, dass du sie lieber hast.',
      beta: 'Beta',
      tryIt: 'Einfach ausprobieren',
      soon: 'Bald verfügbar',
      version: 'Version {v}',
      yours: { windows: 'Für deinen Windows-PC', macos: 'Für deinen Mac', linux: 'Für deinen Linux-Computer', android: 'Für dein Android-Gerät', ios: 'Für dein iPhone oder iPad' },
      cards: {
        web: { h: 'Web-App', p: 'Im Browser, auf Computern, Handys und Tablets. Nichts zu installieren.', btn: 'Web-App öffnen' },
        windows: { h: 'Windows', p: 'Für Windows-Laptops und -PCs.', btn: 'Für Windows herunterladen' },
        macos: { h: 'Mac', p: 'Für MacBooks und iMacs.', btn: 'Für Mac herunterladen' },
        linux: { h: 'Linux', p: 'Für Linux-Computer. Für Debian und Ubuntu nimmst du die .deb-Datei.', btn: 'Für Linux herunterladen' },
        android: { h: 'Android-Handy oder Tablet', p: 'Vom Handy senden oder ein Tablet zum Bildschirm machen.', btn: 'Für Android herunterladen', store: 'Bei Google Play holen' },
        ios: { h: 'iPhone oder iPad', p: 'Fotos, PDFs, Links und deinen Bildschirm senden.', store: 'Im App Store holen' },
        appletv: { h: 'Apple TV', p: 'Macht deinen Fernseher zum Comet-Cast-Bildschirm.', store: 'Im App Store holen' },
        googletv: { h: 'Google TV oder Android TV', p: 'Macht deinen Fernseher zum Comet-Cast-Bildschirm.', store: 'Bei Google Play holen' },
      },
      notSure: 'Du weißt nicht, was du brauchst?',
      notSureLink: 'Schau nach, was du brauchst',
      allReleases: 'Alle Versionen auf GitHub',
    },
    how: {
      title: 'Wie es funktioniert: wohin deine Daten gehen',
      desc: 'Die technische Seite von Comet Cast: der Datenweg, was der Relay-Server sieht und speichert, der eigene Server, signierte Ankündigungen und die Bausteine.',
      body: () => `
<span class="eyebrow">Für IT und Neugierige</span>
<h1>Wohin deine Daten gehen</h1>
<p class="lead">Das Bild geht direkt von deinem Gerät zum Bildschirm. Ein kleiner Server hilft nur dabei, dass sich die beiden finden.</p>
<div class="path">
<svg viewBox="0 0 900 190" role="img" aria-labelledby="path-t">
<title id="path-t">Datenweg: Dein Gerät und der Bildschirm reden direkt miteinander; das Relay hilft ihnen nur, sich zu finden.</title>
<rect x="10" y="60" width="170" height="70" rx="12" class="box"/>
<text x="95" y="100" text-anchor="middle" font-weight="700">Dein Gerät</text>
<rect x="720" y="60" width="170" height="70" rx="12" class="box"/>
<text x="805" y="100" text-anchor="middle" font-weight="700">Der Bildschirm</text>
<rect x="350" y="4" width="200" height="44" rx="10" class="box"/>
<text x="450" y="31" text-anchor="middle" font-size="14" font-weight="700">Relay: Code und Verbindungsaufbau</text>
<path d="M180 82 C 260 30, 290 26, 350 26" class="dash"/>
<path d="M550 26 C 610 26, 640 30, 720 82" class="dash"/>
<path d="M180 108 L 720 108" class="direct"/>
<text x="450" y="98" text-anchor="middle" font-weight="700">Bild, Dateien, Links: direkt (WebRTC, verschlüsselt)</text>
<text x="450" y="160" text-anchor="middle" class="mut" font-size="14">Ist der direkte Weg gesperrt: der eigene TURN-Server der Organisation. Das öffentliche Relay trägt nie das Bild.</text>
</svg>
</div>
<h2>Was das Relay sieht und speichert</h2>
<p>Das Relay ist ein kleiner Server. Er bringt ein Gerät und einen Bildschirm über ihren Raumcode zusammen und reicht die Nachrichten weiter, die beide für den Verbindungsaufbau brauchen. Er sieht den Raumcode, den Namen des Bildschirms und deine Netzwerkadresse (um Missbrauch zu bremsen). Räume liegen nur im Arbeitsspeicher, auf der Platte wird nichts gespeichert. Bilder, Dateien und Links laufen nie über das öffentliche Relay.</p>
<h2>Gäste-WLAN und internes Netz</h2>
<p>Der klassische Fall: Du bist im Gäste-WLAN, der Bildschirm hängt im internen Netz der Organisation. Sich finden klappt immer, weil beide den Server über das Internet erreichen. Das Bild braucht einen Weg zwischen den beiden Netzen, und die sind meist absichtlich getrennt.</p>
<div class="path nets"><svg viewBox="0 0 900 236" role="img" aria-labelledby="nets-t"><title id="nets-t">Drei Fälle: gleiches WLAN, Gäste-WLAN ohne eigenen Server, Gäste-WLAN mit eigenem Server</title><rect x="0" y="0" width="290" height="236" rx="12" class="panel"/><text x="145" y="28" text-anchor="middle" font-weight="700" font-size="15">Gleiches WLAN</text><text x="145" y="50" text-anchor="middle" font-size="13" font-weight="700" class="cap-direct">Direkt</text><rect x="12" y="150" width="96" height="44" rx="10" class="box"/><text x="60" y="177" text-anchor="middle" font-size="13" font-weight="700">Dein Gerät</text><rect x="182" y="150" width="96" height="44" rx="10" class="box"/><text x="230" y="177" text-anchor="middle" font-size="13" font-weight="700">Bildschirm</text><text x="60" y="214" text-anchor="middle" class="mut" font-size="12">gleiches WLAN</text><text x="230" y="214" text-anchor="middle" class="mut" font-size="12">gleiches WLAN</text><path d="M108 172 L182 172" class="direct"/><rect x="305" y="0" width="290" height="236" rx="12" class="panel"/><text x="450" y="28" text-anchor="middle" font-weight="700" font-size="15">Gäste-WLAN, ohne eigenen Server</text><text x="450" y="50" text-anchor="middle" font-size="13" font-weight="700" class="cap-maybe">Nur wenn er dich zurücklässt</text><rect x="317" y="150" width="96" height="44" rx="10" class="box"/><text x="365" y="177" text-anchor="middle" font-size="13" font-weight="700">Dein Gerät</text><rect x="487" y="150" width="96" height="44" rx="10" class="box"/><text x="535" y="177" text-anchor="middle" font-size="13" font-weight="700">Bildschirm</text><text x="365" y="214" text-anchor="middle" class="mut" font-size="12">Gäste-WLAN</text><text x="535" y="214" text-anchor="middle" class="mut" font-size="12">internes Netz</text><path d="M450 112 L450 224" class="wall"/><rect x="395" y="68" width="110" height="38" rx="10" class="box"/><text x="450" y="92" text-anchor="middle" font-size="12" font-weight="700">Router, Internet</text><path d="M365 150 C 365 110, 373 87, 395 87" class="maybe"/><path d="M505 87 C 527 87, 535 110, 535 150" class="maybe"/><rect x="610" y="0" width="290" height="236" rx="12" class="panel"/><text x="755" y="28" text-anchor="middle" font-weight="700" font-size="15">Gäste-WLAN, mit eigenem Server</text><text x="755" y="50" text-anchor="middle" font-size="13" font-weight="700" class="cap-server">Zuverlässig, bleibt im Haus</text><rect x="622" y="150" width="96" height="44" rx="10" class="box"/><text x="670" y="177" text-anchor="middle" font-size="13" font-weight="700">Dein Gerät</text><rect x="792" y="150" width="96" height="44" rx="10" class="box"/><text x="840" y="177" text-anchor="middle" font-size="13" font-weight="700">Bildschirm</text><text x="670" y="214" text-anchor="middle" class="mut" font-size="12">Gäste-WLAN</text><text x="840" y="214" text-anchor="middle" class="mut" font-size="12">internes Netz</text><path d="M755 112 L755 224" class="wall"/><rect x="700" y="68" width="110" height="38" rx="10" class="box"/><text x="755" y="92" text-anchor="middle" font-size="12" font-weight="700">Euer Server</text><path d="M670 150 C 670 110, 678 87, 700 87" class="direct"/><path d="M810 87 C 832 87, 840 110, 840 150" class="direct"/></svg></div>
<ul class="cases">
<li><b>Gleiches WLAN.</b> Dein Gerät und der Bildschirm reden direkt miteinander. Eingebaute Wege wie AirPlay und Cast gehen auch. Manche Netze trennen Geräte sogar im selben WLAN („Client-Isolation“); dann gilt der nächste Fall.</li>
<li><b>Verschiedene Netze, kein eigener Server.</b> Comet Cast erfährt die öffentliche Adresse beider Seiten und versucht, ins Internet hinaus und wieder hinein zu gehen. Das klappt nur, wenn der Router eurer Organisation solchen Verkehr wieder nach innen leitet („Hairpinning“). Viele Firmen- und Schul-Firewalls tun das nicht, oft mit Absicht. Dann kommt keine Verbindung zustande, und die App sagt das. Eingebaute Wege gehen über Netze hinweg nie.</li>
<li><b>Verschiedene Netze, eigener Server.</b> Eure Organisation betreibt einen Comet-Cast-Server mit TURN-Relay, den beide Netze erreichen, zum Beispiel in der DMZ. Das Bild läuft von deinem Gerät zu diesem Server und weiter zum Bildschirm. Das klappt immer, und das Bild bleibt im Haus. Der Bildschirm zeigt die Adresse und den QR-Code eures Servers, wer scannt, landet dort; niemand stellt am eigenen Gerät etwas ein.</li>
</ul>
<p>Aus mobilen Daten oder von einem anderen Standort ist es dieselbe Idee: Comet Cast verbindet über das Internet, das klappt meistens; strenge Netze brauchen euren eigenen Server. Das öffentliche Relay trägt nie das Bild, es hat absichtlich kein TURN-Relay. <a href="/de/selbst-hosten/#wann">Wann ihr einen eigenen Server braucht</a>.</p>
<h2>Bildschirme in der Nähe und signierte Ankündigungen</h2>
<p>Bildschirme in deinem Netz kündigen sich an, damit die App sie auflisten kann. Jede Ankündigung ist mit einem Schlüssel signiert, der zum Bildschirm gehört. Ein anderes Gerät im Netz kann sich also nicht als er ausgeben, ihn aus deiner Liste werfen oder ändern, was er sagt. Kommt ein Gerät mit einem Namen, den schon ein anderes Gerät benutzt hat, zeigt der Bildschirm einen neutralen Hinweis.</p>
<h2>Der Verbindungs-Prüfcode</h2>
<p>Sobald zwei Geräte verbunden sind, zeigen beide denselben kurzen Code. Stimmen die Codes überein, hat sich niemand dazwischengeschoben. Du musst sie nicht vergleichen, kannst es aber.</p>
<h2>Bausteine</h2>
<div class="libs"><table>
<thead><tr><th>Teil</th><th>Aufgebaut auf (Open Source)</th><th>Wohin die Daten gehen</th></tr></thead>
<tbody>
<tr><td>Bild und Ton</td><td><code>webrtc-rs</code>, <code>OpenH264</code></td><td><span class="stays">Bleibt in deinem Netz</span></td></tr>
<tr><td>Sich finden</td><td><code>cometcast-server</code> (<code>axum</code>), <code>ALTCHA</code></td><td><span class="leaves">Raumcode zum Relay, nichts gespeichert</span></td></tr>
<tr><td>Bildschirme in der Nähe</td><td><code>mdns-sd</code>, <code>ed25519-dalek</code></td><td><span class="stays">Bleibt in deinem Netz, signiert</span></td></tr>
<tr><td>PDF-Seiten</td><td><code>hayro</code></td><td><span class="stays">Wird auf dem Anzeigegerät geöffnet</span></td></tr>
<tr><td>iPhone per AirPlay</td><td><code>UxPlay</code></td><td><span class="stays">Bleibt in deinem Netz</span></td></tr>
<tr><td>Web-App</td><td><code>Vue</code>, <code>qrcode-generator</code></td><td><span class="stays">Läuft in deinem Browser</span></td></tr>
</tbody></table></div>
<h2>Mehr Details</h2>
<p>Architektur: <a href="${GH}/blob/main/docs/architecture.md">docs/architecture.md</a>. Protokoll: <a href="${GH}/blob/main/docs/protocol.md">docs/protocol.md</a>. Der gesamte Quellcode: <a href="${GH}">${GH.replace('https://', '')}</a>. Diese Dokumente sind auf Englisch.</p>`,
    },
    open: {
      title: 'Open Source und Lizenz',
      desc: 'Comet Cast ist freie Software unter GPL-3.0-or-later. Quellcode, Lizenz, Mitmachen und die Komponenten Dritter.',
      body: ({ pub }) => `
<h1>Open Source</h1>
<p class="lead">Comet Cast ist freie Software unter <strong>GPL-3.0-or-later</strong>. Du darfst sie benutzen, ansehen, ändern und weitergeben. Quellcode: <a href="${GH}">${GH.replace('https://', '')}</a>.</p>
<h2>Lizenz</h2>
<p>Text der Lizenz: <a href="${GH}/blob/main/LICENSE">LICENSE</a>. Die GPL deckt den Code. Sie gibt für sich keine Rechte am Namen „Comet Cast“ oder am Logo.</p>
<h2>Mitmachen</h2>
<p>Beiträge sind willkommen und werden unter der <a href="${GH}/blob/main/CLA.md">CLA</a> angenommen. Sie erlaubt der Betreuerin${pub ? `, der ${pub},` : ''} die Apps in App-Stores zu verteilen; die öffentliche Version bleibt GPL-3.0-or-later. Siehe <a href="${GH}/blob/main/CONTRIBUTING.md">CONTRIBUTING.md</a> und den <a href="${GH}/blob/main/CODE_OF_CONDUCT.md">Verhaltenskodex</a>.</p>
<h2>Komponenten Dritter</h2>
<p>Comet Cast baut auf vielen Open-Source-Bibliotheken auf. Die vollständige Liste mit ihren Lizenzen steht in <a href="${GH}/blob/main/THIRD_PARTY_NOTICES.md">THIRD_PARTY_NOTICES.md</a>. Die Schriften dieser Website kommen von dieser Seite selbst:</p>
<ul>
<li>Bricolage Grotesque, © 2022 The Bricolage Grotesque Project Authors</li>
<li>Atkinson Hyperlegible, © 2020 Braille Institute of America, Inc.</li>
<li>JetBrains Mono, © 2020 The JetBrains Mono Project Authors</li>
</ul>
<p>Alle drei stehen unter der <a href="https://openfontlicense.org/open-font-license-official-text/">SIL Open Font License 1.1</a> und werden unverändert benutzt.</p>`,
    },
    contact: {
      title: 'Kontakt und Sicherheit',
      desc: 'Ein Problem melden, eine Idee teilen oder eine Sicherheitslücke privat melden.',
      body: ({ contactHtml }) => `
<h1>Kontakt</h1>
<h2>Probleme und Ideen</h2>
<p>Bitte als Issue auf GitHub: <a href="${GH}/issues">${GH.replace('https://', '')}/issues</a>.</p>
<h2>Sicherheitslücken</h2>
<p>Bitte <strong>kein</strong> öffentliches Issue. Nutze die private Meldung von GitHub: im Repository den Reiter <em>Security</em> und dann <em>Report a vulnerability</em>. Wird das nicht angezeigt, eröffne ein Issue, das nur sagt, dass du einen privaten Weg brauchst, ohne Einzelheiten. Die ganze Richtlinie: <a href="${GH}/blob/main/SECURITY.md">SECURITY.md</a> (englisch).</p>
${contactHtml ? `<h2>E-Mail</h2>\n<p>${contactHtml}</p>` : ''}`,
    },
    notFound: {
      title: '404 - Seite nicht gefunden',
      h1: '404',
      p: 'Diese Seite gibt es nicht.',
      link: 'Zur Startseite',
    },
    redirect: { title: 'Diese Seite ist umgezogen', p: 'Diese Seite ist umgezogen nach' },
  };
}
