// SPDX-License-Identifier: GPL-3.0-or-later
// All page text of the website, English. text.de.mjs has exactly the same keys (tools/build-site.mjs fails the build when
// a key or a block of HTML is missing in one language). Legal pages (privacy, imprint) and self-hosting are in site-content/*.md.
// Written from the user's side: what you get and what you do. German: informal "du".
export default function text({ GH, DOMAIN }) {
  return {
    ui: {
      skip: 'Skip to content',
      mainNav: 'Main navigation',
      footerNav: 'Footer',
      brandAlt: 'Comet Cast logo: a comet shaped like a C',
      whatYouNeed: 'What you need',
      download: 'Download',
      openApp: 'Open web app',
      switchText: 'DE',
      switchLabel: 'Auf Deutsch lesen',
      footProduct: 'Product',
      footTrust: 'Trust',
      footProject: 'Project',
      footApp: 'Web app',
      footPrivacy: 'Privacy',
      footSelfhost: 'Host it yourself',
      footOpen: 'Open source',
      footHow: 'How it works (technical)',
      footContact: 'Contact',
      footImprint: 'Imprint',
      footFine: 'Free software · no cookies · nothing loaded from third parties',
    },
    // Shown on the German page when the browser prefers English (so: written in English).
    hint: { text: 'This page is also available in English.', link: 'Read in English', close: 'Close' },
    home: {
      title: 'Comet Cast: show your screen on any screen in the room',
      desc: 'Send your laptop, phone or tablet screen, a PDF, a photo or a link to the TV or projector in the room. Often without installing anything.',
      h1: 'Show your screen on <em>any</em> screen in the room.',
      lead: 'From a laptop, phone or tablet to the TV or projector. Often without installing anything.',
      ctaDownload: 'Download',
      ctaApp: 'Open web app',
      beta: 'Beta',
      tryIt: 'Try it out. Free, no account.',
      tv: {
        label: `A screen showing Comet Cast: open ${DOMAIN} and enter the code DXZ PNJ`,
        room: 'Meeting room Elbe',
        pill: 'Approval on',
        step1: 'Open in your browser',
        step2: 'Enter this code',
        scan: 'Or scan',
        foot: 'No download needed.',
        settings: 'Settings',
      },
      finderH2: 'What do you need?',
      finderIntro: 'Choose your device and how the screen is connected. You see right away whether it works and what to install.',
      stepsH2: 'Then it is three steps',
      steps: [
        { h: 'The screen shows a code', p: 'And a QR code, and the address to open.' },
        { h: 'You enter it or scan it', p: "Or tap the screen in the app's list of screens nearby." },
        { h: 'You present', p: 'Your screen, a PDF, a photo or a link. Need a moment? Press pause and the room keeps the last picture.' },
      ],
    },
    finder: {
      qHave: 'I have',
      qNet: 'How are you connected?',
      qScreen: 'The screen is connected to',
      nets: {
        same: ['Same Wi-Fi', 'e.g. at home'],
        office: ['Office or school', 'own device, guest Wi-Fi'],
        diff: ['Different networks', 'e.g. mobile data'],
      },
      onDevice: 'On your device',
      onScreen: 'On the screen',
      chipNone: 'Nothing to install',
      chipWeb: 'Open {domain}',
      chipApp: 'Comet Cast app',
      chipAppOnScreen: 'Comet Cast app on it',
      netBuiltinStatic: 'Only in the same Wi-Fi',
      netBuiltinOffice: 'Often blocked in school and office networks',
      netComet: 'Also works across networks',
      verdictOne: 'Works. Here is how:',
      verdictMany: 'Works. {n} ways, the easiest first:',
      verdictNone: 'Not possible with Comet Cast yet.',
      verdictNoNet: 'Nothing works for this pair across different networks. Put both devices in the same Wi-Fi to use the built-in way.',
      tableCaption: 'All answers: device, screen and the ways, easiest first',
      tableTo: 'Show on',
      tableWays: 'Ways, easiest first',
    },
    download: {
      title: 'Download Comet Cast',
      desc: 'Get Comet Cast for Windows, Mac, Linux, Android, iPhone and iPad, Apple TV and Google TV, or open the web app and install nothing.',
      eyebrow: 'Download',
      h1: 'Get Comet Cast',
      lead: 'Often you do not need to download anything: open the web app in your browser. The apps are there for when you want them.',
      beta: 'Beta',
      tryIt: 'Try it out',
      soon: 'Coming soon',
      version: 'Version {v}',
      yours: { windows: 'For your Windows PC', macos: 'For your Mac', linux: 'For your Linux computer', android: 'For your Android device', ios: 'For your iPhone or iPad' },
      cards: {
        web: { h: 'Web app', p: 'In your browser, on computers, phones and tablets. Nothing to install.', btn: 'Open web app' },
        windows: { h: 'Windows', p: 'For Windows laptops and PCs.', btn: 'Download for Windows' },
        macos: { h: 'Mac', p: 'For MacBooks and iMacs.', btn: 'Download for Mac' },
        linux: { h: 'Linux', p: 'For Linux computers. Choose the .deb for Debian and Ubuntu.', btn: 'Download for Linux' },
        android: { h: 'Android phone or tablet', p: 'Send from your phone, or turn a tablet into a screen.', btn: 'Download for Android', store: 'Get it on Google Play' },
        ios: { h: 'iPhone or iPad', p: 'Send photos, PDFs, links and your screen.', store: 'Get it on the App Store' },
        appletv: { h: 'Apple TV', p: 'Turns your TV into a Comet Cast screen.', store: 'Get it on the App Store' },
        googletv: { h: 'Google TV or Android TV', p: 'Turns your TV into a Comet Cast screen.', store: 'Get it on Google Play' },
      },
      notSure: 'Not sure what you need?',
      notSureLink: 'Check what you need',
      allReleases: 'All versions on GitHub',
    },
    how: {
      title: 'How it works: where your data goes',
      desc: 'The technical side of Comet Cast: the data path, what the relay server sees and stores, your own server, signed announcements and the building blocks.',
      body: () => `
<span class="eyebrow">For IT and the curious</span>
<h1>Where your data goes</h1>
<p class="lead">The picture goes straight from your device to the screen. A small server only helps the two find each other.</p>
<div class="path">
<svg viewBox="0 0 900 190" role="img" aria-labelledby="path-t">
<title id="path-t">Data path: your device and the screen talk directly; the relay only helps them find each other.</title>
<rect x="10" y="60" width="170" height="70" rx="12" class="box"/>
<text x="95" y="100" text-anchor="middle" font-weight="700">Your device</text>
<rect x="720" y="60" width="170" height="70" rx="12" class="box"/>
<text x="805" y="100" text-anchor="middle" font-weight="700">The screen</text>
<rect x="350" y="4" width="200" height="44" rx="10" class="box"/>
<text x="450" y="31" text-anchor="middle" font-size="14" font-weight="700">Relay: code and handshake</text>
<path d="M180 82 C 260 30, 290 26, 350 26" class="dash"/>
<path d="M550 26 C 610 26, 640 30, 720 82" class="dash"/>
<path d="M180 108 L 720 108" class="direct"/>
<text x="450" y="98" text-anchor="middle" font-weight="700">Picture, files, links: directly (WebRTC, encrypted)</text>
<text x="450" y="160" text-anchor="middle" class="mut" font-size="14">If the direct way is blocked: your organisation's own TURN server. The public relay never carries the picture.</text>
</svg>
</div>
<h2>What the relay sees and stores</h2>
<p>The relay is a small server that matches a device and a screen by their room code and passes along the messages they need to set up the connection. It sees the room code, the name of the screen and your network address (to slow down abuse). It keeps rooms in memory only and stores nothing on disk. Pictures, files and links never go through the public relay.</p>
<h2>If the direct way is blocked</h2>
<p>Some networks do not let two devices talk directly. An organisation can then run its own server with a TURN relay: the picture travels through that machine, inside the organisation, and not through the public relay. <a href="/self-hosting/">Host it yourself</a> explains how.</p>
<h2>Screens nearby and signed announcements</h2>
<p>Screens in your network announce themselves so that the app can list them. Every announcement is signed with a key that belongs to the screen, so another device on the network cannot pretend to be it, remove it from your list or change what it says. When a device joins with a name that another device used before, the screen shows a neutral note.</p>
<h2>The connection check code</h2>
<p>After two devices connect, both show the same short code. If the codes match, nobody sat in between. You do not have to compare them, but you can.</p>
<h2>Building blocks</h2>
<div class="libs"><table>
<thead><tr><th>Part</th><th>Built on (open source)</th><th>Where the data goes</th></tr></thead>
<tbody>
<tr><td>Picture and sound</td><td><code>webrtc-rs</code>, <code>OpenH264</code></td><td><span class="stays">Stays in your network</span></td></tr>
<tr><td>Finding each other</td><td><code>cometcast-server</code> (<code>axum</code>), <code>ALTCHA</code></td><td><span class="leaves">Room code to the relay, nothing stored</span></td></tr>
<tr><td>Screens nearby</td><td><code>mdns-sd</code>, <code>ed25519-dalek</code></td><td><span class="stays">Stays in your network, signed</span></td></tr>
<tr><td>PDF pages</td><td><code>hayro</code></td><td><span class="stays">Opened on the screen device</span></td></tr>
<tr><td>iPhone via AirPlay</td><td><code>UxPlay</code></td><td><span class="stays">Stays in your network</span></td></tr>
<tr><td>Web app</td><td><code>Vue</code>, <code>qrcode-generator</code></td><td><span class="stays">Runs in your browser</span></td></tr>
</tbody></table></div>
<h2>More detail</h2>
<p>Architecture: <a href="${GH}/blob/main/docs/architecture.md">docs/architecture.md</a>. Protocol: <a href="${GH}/blob/main/docs/protocol.md">docs/protocol.md</a>. All source code: <a href="${GH}">${GH.replace('https://', '')}</a>. These documents are in English.</p>`,
    },
    open: {
      title: 'Open source and licence',
      desc: 'Comet Cast is free software under GPL-3.0-or-later. Source code, licence, contributions and the third-party components it uses.',
      body: ({ pub }) => `
<h1>Open source</h1>
<p class="lead">Comet Cast is free software under <strong>GPL-3.0-or-later</strong>. You may use it, study it, change it and share it. Source: <a href="${GH}">${GH.replace('https://', '')}</a>.</p>
<h2>Licence</h2>
<p>Licence text: <a href="${GH}/blob/main/LICENSE">LICENSE</a>. The GPL covers the code. It does not by itself give rights to the name "Comet Cast" or the logo.</p>
<h2>Take part</h2>
<p>Contributions are welcome and are accepted under the <a href="${GH}/blob/main/CLA.md">CLA</a>. It allows the steward${pub ? `, ${pub},` : ''} to distribute the apps in app stores; the public version stays GPL-3.0-or-later. See <a href="${GH}/blob/main/CONTRIBUTING.md">CONTRIBUTING.md</a> and the <a href="${GH}/blob/main/CODE_OF_CONDUCT.md">Code of Conduct</a>.</p>
<h2>Third-party components</h2>
<p>Comet Cast builds on many open-source libraries. The full list with their licences is in <a href="${GH}/blob/main/THIRD_PARTY_NOTICES.md">THIRD_PARTY_NOTICES.md</a>. The fonts of this website are served from this site itself:</p>
<ul>
<li>Bricolage Grotesque, © 2022 The Bricolage Grotesque Project Authors</li>
<li>Atkinson Hyperlegible, © 2020 Braille Institute of America, Inc.</li>
<li>JetBrains Mono, © 2020 The JetBrains Mono Project Authors</li>
</ul>
<p>All three are licensed under the <a href="https://openfontlicense.org/open-font-license-official-text/">SIL Open Font License 1.1</a> and are used unchanged.</p>`,
    },
    contact: {
      title: 'Contact and security',
      desc: 'Report a problem, share an idea, or report a security issue privately.',
      body: ({ contactHtml }) => `
<h1>Contact</h1>
<h2>Problems and ideas</h2>
<p>Please open an issue on GitHub: <a href="${GH}/issues">${GH.replace('https://', '')}/issues</a>.</p>
<h2>Security issues</h2>
<p>Please do <strong>not</strong> open a public issue. Use GitHub's private reporting: open the <em>Security</em> tab of the repository and choose <em>Report a vulnerability</em>. If that is not shown, open an issue that only says you need a private channel, without details. The full policy: <a href="${GH}/blob/main/SECURITY.md">SECURITY.md</a>.</p>
${contactHtml ? `<h2>Email</h2>\n<p>${contactHtml}</p>` : ''}`,
    },
    notFound: {
      title: '404 - Page not found',
      h1: '404',
      p: 'This page does not exist.',
      link: 'Back to the home page',
    },
    redirect: { title: 'This page has moved', p: 'This page has moved to' },
  };
}
