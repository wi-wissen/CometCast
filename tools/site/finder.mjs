// SPDX-License-Identifier: GPL-3.0-or-later
// The finder "What do you need?": devices, pictograms and the answer matrix (from docs/design/website-pitch.html, extended
// in round 3 of docs/design/website.md). The matrix has both languages side by side so they cannot drift apart.
// renderFinder() returns the static HTML: a full answer table (works without JavaScript) and a hidden picker that
// site.js turns on (it then hides the table).
const esc = (v) => String(v).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

export const ICON = {
  laptop: '<rect class="dev-line" x="12" y="6" width="32" height="22" rx="2"/><path class="dev-line" d="M6 34h44l-3-6H9z"/><circle class="dev-fill" cx="28" cy="17" r="3"/>',
  iphone: '<rect class="dev-line" x="18" y="3" width="20" height="38" rx="4"/><path class="dev-line" d="M25 8h6"/><circle class="dev-fill" cx="28" cy="22" r="3"/>',
  android: '<rect class="dev-line" x="18" y="3" width="20" height="38" rx="3"/><path class="dev-line" d="M24 36h8"/><circle class="dev-fill" cx="28" cy="20" r="3"/>',
  appletv: '<rect class="dev-line" x="4" y="4" width="48" height="28" rx="2"/><rect class="dev-line" x="20" y="35" width="16" height="6" rx="2"/><circle class="dev-fill" cx="28" cy="18" r="3"/>',
  googletv: '<rect class="dev-line" x="4" y="4" width="48" height="28" rx="2"/><path class="dev-line" d="M20 40h16M28 32v8"/><circle class="dev-fill" cx="28" cy="18" r="3"/>',
  chromecast: '<rect class="dev-line" x="4" y="4" width="48" height="28" rx="2"/><circle class="dev-line" cx="44" cy="38" r="4"/><path class="dev-line" d="M10 26a5 5 0 0 1 5 5M10 20a11 11 0 0 1 11 11"/>',
  desktop: '<rect class="dev-line" x="4" y="6" width="34" height="24" rx="2"/><path class="dev-line" d="M16 38h10M21 30v8"/><rect class="dev-line" x="42" y="10" width="10" height="28" rx="2"/><circle class="dev-fill" cx="21" cy="18" r="3"/>',
  board: '<rect class="dev-line" x="4" y="4" width="48" height="30" rx="2"/><path class="dev-line" d="M14 42l4-8M42 42l-4-8M33 12l-8 13"/><circle class="dev-fill" cx="36" cy="18" r="3"/>',
};

export const SENDERS = [
  { id: 'win', icon: 'laptop', en: ['Windows laptop', ''], de: ['Windows-Laptop', ''] },
  { id: 'mac', icon: 'laptop', en: ['Mac', 'MacBook, iMac'], de: ['Mac', 'MacBook, iMac'] },
  { id: 'linux', icon: 'laptop', en: ['Linux laptop', ''], de: ['Linux-Laptop', ''] },
  { id: 'iphone', icon: 'iphone', en: ['iPhone or iPad', ''], de: ['iPhone oder iPad', ''] },
  { id: 'android', icon: 'android', en: ['Android phone', 'or tablet'], de: ['Android-Handy', 'oder Tablet'] },
];
export const SCREENS = [
  { id: 'appletv', icon: 'appletv', en: ['Apple TV', ''], de: ['Apple TV', ''] },
  { id: 'googletv', icon: 'googletv', en: ['Google TV', 'or Android TV'], de: ['Google TV', 'oder Android TV'] },
  { id: 'chromecast', icon: 'chromecast', en: ['Chromecast', ''], de: ['Chromecast', ''] },
  { id: 'winpc', icon: 'desktop', en: ['Windows PC', ''], de: ['Windows-PC', ''] },
  { id: 'macpc', icon: 'desktop', en: ['Mac or Linux computer', ''], de: ['Mac oder Linux-Computer', ''] },
  { id: 'board', icon: 'board', en: ['Interactive board', 'Android'], de: ['Interaktive Tafel', 'Android'] },
];
/** How the two are connected; the default is the middle one. */
export const NETS = ['same', 'office', 'diff'];
export const DEFAULT_NET = 'office';

// What goes on each side: none, web (the site's address in the browser), app (Comet Cast app), or an object: built into the device.
const own = (en, de) => ({ en, de });
const N = (en, de) => ({ en, de });
const W = (you, screen, note) => ({ you, screen, note });
const castW = own('Chrome or Edge: Cast, built in', 'Chrome oder Edge: Streamen, schon eingebaut');
const castL = own('Chrome or Chromium: Cast, built in', 'Chrome oder Chromium: Streamen, schon eingebaut');
const airMac = own('Mac: AirPlay, built in', 'Mac: AirPlay, schon eingebaut');
const winK = own('Windows: Win + K, built in', 'Windows: Win + K, schon eingebaut');
const airIos = own('AirPlay, built in', 'AirPlay, schon eingebaut');
const castBtn = own('Cast button, built in', 'Übertragen-Knopf, schon eingebaut');
const smart = own('Smart View or Cast, built in', 'Smart View oder Übertragen, schon eingebaut');
const BROWSER_ALONE = N('Your browser can do this on its own.', 'Dein Browser kann das schon selbst.');
const BROWSER_NEEDLESS = N('Your browser can do this on its own. You do not need Comet Cast for it.', 'Dein Browser kann das schon selbst. Dafür brauchst du Comet Cast nicht.');
const ASK = N('With Comet Cast the screen can ask before it shows anything, and you can send PDFs and links.', 'Mit Comet Cast kann der Bildschirm vorher um Erlaubnis fragen, und du kannst PDFs und Links schicken.');
const PHOTOS = N('Photos, PDFs and links only, not your screen.', 'Nur Fotos, PDFs und Links, nicht dein Bildschirm.');
const NOTHING = N('Nothing to install on either side.', 'Auf keiner Seite etwas installieren.');
const WINONLY = N('Works when the computer at the screen runs Windows.', 'Geht, wenn der Computer am Bildschirm Windows hat.');
const DELAY = N('Photos and videos right away; your screen with a short delay.', 'Fotos und Videos sofort, dein Bildschirm mit kurzer Verzögerung.');
const MAC_ALONE = N('A Mac can do this on its own.', 'Ein Mac kann das auch ganz allein.');

const laptopTo = (cast, extra = []) => ({
  googletv: [W(cast, 'none', BROWSER_ALONE), W('web', 'app', ASK), W('app', 'none')],
  chromecast: [W(cast, 'none', BROWSER_NEEDLESS), W('app', 'none')],
  appletv: [W('web', 'app'), W('app', 'none', DELAY)],
  winpc: [W('web', 'web', NOTHING), ...extra],
  macpc: [W('web', 'web', NOTHING)],
  board: [W('web', 'web', NOTHING), W('web', 'app')],
});
const win = laptopTo(castW, [W(winK, 'app', WINONLY)]);
const mac = laptopTo(castW);
mac.appletv = [W(airMac, 'none', MAC_ALONE), ...mac.appletv];
mac.macpc = [W('web', 'web', NOTHING), W(airMac, 'app')];
const linux = laptopTo(castL);

export const M = {
  win,
  mac,
  linux,
  iphone: {
    appletv: [W(airIos, 'none', N('Your iPhone can do this on its own. You do not need Comet Cast for it.', 'Das kann dein iPhone schon selbst. Dafür brauchst du Comet Cast nicht.')), W('app', 'app', N('Also sends PDFs and links, and the screen can ask before it shows anything.', 'Schickt auch PDFs und Links, und der Bildschirm kann vorher um Erlaubnis fragen.'))],
    googletv: [W('app', 'app'), W('web', 'app', PHOTOS)],
    chromecast: [],
    winpc: [W('app', 'web'), W('web', 'web', PHOTOS)],
    macpc: [W(airIos, 'app'), W('app', 'web')],
    board: [W('app', 'web'), W('app', 'app'), W('web', 'web', PHOTOS)],
  },
  android: {
    appletv: [W('app', 'app'), W('web', 'app', PHOTOS)],
    googletv: [W(castBtn, 'none', N('Most Android phones can do this on their own.', 'Die meisten Android-Handys können das schon selbst.')), W('app', 'app', ASK)],
    chromecast: [W(castBtn, 'none', N('Your phone can do this on its own. You do not need Comet Cast for it.', 'Das kann dein Handy schon selbst. Dafür brauchst du Comet Cast nicht.'))],
    winpc: [W(smart, 'app', WINONLY), W('app', 'web')],
    macpc: [W('app', 'web')],
    board: [W('app', 'web'), W('app', 'app')],
  },
};

const isOwn = (v) => typeof v === 'object';

function chip(w, side, lang, t, domain) {
  const v = side === 'you' ? w.you : w.screen;
  if (isOwn(v)) return `<span class="need own">${esc(v[lang])}</span>`;
  const text = v === 'none' ? t.chipNone : v === 'web' ? t.chipWeb.replace('{domain}', domain) : side === 'screen' ? t.chipAppOnScreen : t.chipApp;
  return `<span class="need ${v}">${esc(text)}</span>`;
}

function wayHtml(w, s, r, lang, t, domain) {
  const builtin = isOwn(w.you);
  const rName = r[lang][0];
  // The text of the connection chip per answer; the static text (no JavaScript) says both things.
  const attrs = builtin
    ? `data-text-same="" data-text-office="${esc(t.netBuiltinOffice)}" data-text-diff=""`
    : `data-text-same="" data-text-office="${esc(t.netComet)}" data-text-diff="${esc(t.netComet)}"`;
  const staticText = builtin ? t.netBuiltinStatic : t.netComet;
  return (
    `<li class="way" data-kind="${builtin ? 'builtin' : 'comet'}">` +
    `<div class="side"><span class="who">${esc(t.onDevice)}</span>${chip(w, 'you', lang, t, domain)}</div>` +
    `<svg class="arrow" viewBox="0 0 70 20" aria-hidden="true"><path d="M4 10h58M54 3l8 7-8 7"/></svg>` +
    `<div class="side right"><span class="who">${esc(t.onScreen)} · ${esc(rName)}</span>${chip(w, 'screen', lang, t, domain)}</div>` +
    `<p class="netchip ${builtin ? 'builtin' : 'comet'}" ${attrs}>${esc(staticText)}</p>` +
    (w.note ? `<p class="note">${esc(w.note[lang])}</p>` : '') +
    `</li>`
  );
}

export function verdictText(t, n) {
  return n === 1 ? t.verdictOne : t.verdictMany.replace('{n}', String(n));
}

function answerHtml(s, r, lang, t, domain) {
  const ways = M[s.id][r.id];
  if (!ways.length) return `<p class="verdict no">${esc(t.verdictNone)}</p>`;
  return `<p class="verdict yes">${esc(verdictText(t, ways.length))}</p><ol class="ways">${ways.map((w) => wayHtml(w, s, r, lang, t, domain)).join('')}</ol>`;
}

const picks = (items, lang, group, extra = '') =>
  `<div class="picks${extra}" role="group" aria-label="${esc(group)}">${items
    .map(
      (it) =>
        `<button type="button" class="pick" data-id="${it.id}" aria-pressed="false"><svg viewBox="0 0 56 44" aria-hidden="true">${ICON[it.icon]}</svg><span>${esc(it[lang][0])}</span><small>${esc(it[lang][1])}</small></button>`,
    )
    .join('')}</div>`;

/** t = the finder texts of the language (strings in tools/site/text.*.mjs). */
export function renderFinder(lang, t, domain) {
  const netBtns = NETS.map((n) => `<button type="button" class="net" data-id="${n}" aria-pressed="false"><span>${esc(t.nets[n][0])}</span><small>${esc(t.nets[n][1])}</small></button>`).join('');
  const picker =
    `<div class="picker" hidden>` +
    `<div class="ask"><h3>${esc(t.qHave)}</h3>${picks(SENDERS, lang, t.qHave, ' five')}</div>` +
    `<div class="ask"><h3>${esc(t.qNet)}</h3><div class="nets" role="group" aria-label="${esc(t.qNet)}">${netBtns}</div></div>` +
    `<div class="ask"><h3>${esc(t.qScreen)}</h3>${picks(SCREENS, lang, t.qScreen, ' six')}</div>` +
    `<div class="answer" id="answer" aria-live="polite"></div>` +
    `</div>`;
  const rows = SENDERS.map(
    (s) =>
      `<tbody><tr class="grp"><th colspan="2" scope="rowgroup"><svg viewBox="0 0 56 44" aria-hidden="true">${ICON[s.icon]}</svg>${esc(t.qHave)}: ${esc(s[lang][0])}</th></tr>` +
      SCREENS.map(
        (r) =>
          `<tr id="${s.id}-${r.id}" data-s="${s.id}" data-r="${r.id}"><th scope="row">${esc(t.tableTo)} ${esc(r[lang][0])}${r[lang][1] ? ` <small>${esc(r[lang][1])}</small>` : ''}</th><td>${answerHtml(s, r, lang, t, domain)}</td></tr>`,
      ).join('') +
      `</tbody>`,
  ).join('');
  const table =
    `<div class="matrix"><table class="mtable"><caption class="sr">${esc(t.tableCaption)}</caption>` +
    `<thead><tr><th scope="col">${esc(t.tableTo)}</th><th scope="col">${esc(t.tableWays)}</th></tr></thead>${rows}</table></div>`;
  const data = ` data-default-net="${DEFAULT_NET}" data-v-one="${esc(t.verdictOne)}" data-v-many="${esc(t.verdictMany)}" data-v-none="${esc(t.verdictNone)}" data-v-nonet="${esc(t.verdictNoNet)}"`;
  return `<div class="finder" id="finder-app"${data}>${picker}${table}</div>`;
}
