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
  { id: 'androidtv', icon: 'googletv', en: ['Android device', 'Google TV, Android TV, board'], de: ['Android-Gerät', 'Google TV, Android TV, Tafel'] },
  { id: 'chromecast', icon: 'chromecast', en: ['Chromecast', ''], de: ['Chromecast', ''] },
  { id: 'winpc', icon: 'desktop', en: ['Windows PC', ''], de: ['Windows-PC', ''] },
  { id: 'macscreen', icon: 'desktop', en: ['Mac', 'iMac, Mac mini'], de: ['Mac', 'iMac, Mac mini'] },
  { id: 'linuxpc', icon: 'desktop', en: ['Linux PC', 'mini PC'], de: ['Linux-PC', 'Mini-PC'] },
];
/**
 * Where the presenter's device is in the network, seen from the screen:
 * same = the same Wi-Fi (direct; built-in ways work),
 * org = another Wi-Fi of the same place, e.g. guest Wi-Fi while the screen is in the internal network (kept apart on
 *       purpose; built-in ways never work; Comet Cast is reliable only with the organisation's own server with TURN),
 * away = somewhere else entirely, mobile data or another site (over the internet; usually works with STUN, strict
 *        networks need the organisation's own server).
 */
export const NETS = ['same', 'org', 'away'];
export const DEFAULT_NET = 'same';
/** Where "for your IT" points: the self-hosting page, at its "when do you need it" section. */
const IT_LINK = { en: '/self-hosting/#when', de: '/de/selbst-hosten/#wann' };

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
const BROWSER_NEEDLESS = N('Your browser can do this on its own. You do not need Comet Cast for it.', 'Dein Browser kann das schon selbst. Dafür brauchst du Comet Cast nicht.');
const ASK = N('With Comet Cast the screen can ask before it shows anything, and you can send PDFs and links.', 'Mit Comet Cast kann der Bildschirm vorher um Erlaubnis fragen, und du kannst PDFs und Links schicken.');
const PHOTOS = N('Photos, PDFs and links only, not your screen.', 'Nur Fotos, PDFs und Links, nicht dein Bildschirm.');
const NOTHING = N('Nothing to install on either side.', 'Auf keiner Seite etwas installieren.');
const WINONLY = N('Works when the computer at the screen runs Windows.', 'Geht, wenn der Computer am Bildschirm Windows hat.');
const DELAY = N('Photos and videos right away; your screen with a short delay.', 'Fotos und Videos sofort, dein Bildschirm mit kurzer Verzögerung.');
const MAC_ALONE = N('A Mac can do this on its own.', 'Ein Mac kann das auch ganz allein.');
const MAC_SCREEN = N('A Mac with macOS 12 or newer can show this on its own. You do not need Comet Cast for it.', 'Ein Mac ab macOS 12 kann das schon selbst zeigen. Dafür brauchst du Comet Cast nicht.');

// The matrix follows one rule, so no combination is forgotten:
// 1. Built-in ways of the devices (AirPlay, Cast, Miracast): only where they exist, only in the same Wi-Fi.
// 2. One Comet Cast way per pair. Each side lists every option it has:
//    - your device: a computer can use the browser or the app; a phone the app (its browser sends photos, PDFs and
//      links only, no screen);
//    - the screen: a computer or an interactive board can use the browser or the app; Apple TV and Google TV the app;
//      a Chromecast cannot run Comet Cast.
// 3. The Comet Cast app on a computer can also send to a TV that has no Comet Cast at all (Cast, AirPlay).
// Order: ways where nothing has to be installed come first.
const DESKTOP = ['win', 'mac', 'linux'];
const SCREEN_SIDE = { appletv: ['app'], androidtv: ['app'], chromecast: null, winpc: ['web', 'app'], macscreen: ['web', 'app'], linuxpc: ['web', 'app'] };
// Google TV and Android TV have a Cast receiver built in; Android boards usually do not, but they have a browser.
const TV_ONLY = N('Works with Google TV and Android TV; most interactive boards have no Cast built in.', 'Geht bei Google TV und Android TV; die meisten interaktiven Tafeln haben kein Cast eingebaut.');
const BOARD_WEB = N('An interactive board can also use cometcast.de in its browser instead of the app.', 'Eine interaktive Tafel kann statt der App auch cometcast.de im Browser öffnen.');
const PHONE_WEB = N('In the browser only photos, PDFs and links; with the app also your screen.', 'Im Browser nur Fotos, PDFs und Links, mit der App auch dein Bildschirm.');
const BUILTIN = {
  win: { androidtv: [W(castW, 'none', TV_ONLY)], chromecast: [W(castW, 'none', BROWSER_NEEDLESS)], winpc: [W(winK, 'app', WINONLY)] },
  mac: { androidtv: [W(castW, 'none', TV_ONLY)], chromecast: [W(castW, 'none', BROWSER_NEEDLESS)], appletv: [W(airMac, 'none', MAC_ALONE)], macscreen: [W(airMac, 'none', MAC_SCREEN)], linuxpc: [W(airMac, 'app')] },
  linux: { androidtv: [W(castL, 'none', TV_ONLY)], chromecast: [W(castL, 'none', BROWSER_NEEDLESS)] },
  iphone: { appletv: [W(airIos, 'none', N('Your iPhone can do this on its own. You do not need Comet Cast for it.', 'Das kann dein iPhone schon selbst. Dafür brauchst du Comet Cast nicht.'))], macscreen: [W(airIos, 'none', MAC_SCREEN)], linuxpc: [W(airIos, 'app')] },
  android: { androidtv: [W(castBtn, 'none', TV_ONLY)], chromecast: [W(castBtn, 'none', N('Your phone can do this on its own. You do not need Comet Cast for it.', 'Das kann dein Handy schon selbst. Dafür brauchst du Comet Cast nicht.'))], winpc: [W(smart, 'app', WINONLY)] },
};
// The app on a computer sends to TVs without Comet Cast (Cast-compatible and AirPlay receivers).
const APP_TO_TV = { androidtv: [W('app', 'none', TV_ONLY)], chromecast: [W('app', 'none')], appletv: [W('app', 'none', DELAY)] };
const COMET_NOTE = { appletv: ASK, androidtv: BOARD_WEB };

function cometWay(s, r) {
  const screen = SCREEN_SIDE[r];
  if (!screen) return [];
  const desktop = DESKTOP.includes(s);
  const you = desktop ? ['web', 'app'] : ['app', 'web'];
  const note = desktop ? COMET_NOTE[r] : r === 'androidtv' ? N(PHONE_WEB.en + ' ' + BOARD_WEB.en, PHONE_WEB.de + ' ' + BOARD_WEB.de) : PHONE_WEB;
  return [W(you, screen, note)];
}
const noInstall = (w) => (w.screen === 'none' || (Array.isArray(w.screen) && w.screen.includes('web'))) && (typeof w.you === 'object' && !Array.isArray(w.you) || (Array.isArray(w.you) && w.you.includes('web')));

export const M = {};
for (const { id: s } of SENDERS) {
  M[s] = {};
  for (const { id: r } of SCREENS) {
    const ways = [...((BUILTIN[s] || {})[r] || []), ...cometWay(s, r), ...(DESKTOP.includes(s) ? APP_TO_TV[r] || [] : [])];
    M[s][r] = ways.filter(noInstall).concat(ways.filter((w) => !noInstall(w)));
  }
}

const isOwn = (v) => typeof v === 'object' && !Array.isArray(v);

function oneChip(v, side, t, domain) {
  const text = v === 'none' ? t.chipNone : v === 'web' ? t.chipWeb.replace('{domain}', domain) : side === 'screen' ? t.chipAppOnScreen : t.chipApp;
  return `<span class="need ${v}">${esc(text)}</span>`;
}

/** A side may list several options ("open cometcast.de or Comet Cast app"). */
function chip(w, side, lang, t, domain) {
  const v = side === 'you' ? w.you : w.screen;
  if (isOwn(v)) return `<span class="need own">${esc(v[lang])}</span>`;
  if (Array.isArray(v)) return v.map((x) => oneChip(x, side, t, domain)).join(`<span class="or">${esc(t.or)}</span>`);
  return oneChip(v, side, t, domain);
}

function wayHtml(w, s, r, lang, t, domain) {
  const builtin = isOwn(w.you);
  const rName = r[lang][0];
  return (
    `<li class="way" data-kind="${builtin ? 'builtin' : 'comet'}">` +
    `<div class="side"><span class="who">${esc(t.onDevice)}</span>${chip(w, 'you', lang, t, domain)}</div>` +
    `<svg class="arrow" viewBox="0 0 70 20" aria-hidden="true"><path d="M4 10h58M54 3l8 7-8 7"/></svg>` +
    `<div class="side right"><span class="who">${esc(t.onScreen)} · ${esc(rName)}</span>${chip(w, 'screen', lang, t, domain)}</div>` +
    (builtin ? `<p class="netchip builtin">${esc(t.netBuiltinStatic)}</p>` : '') +
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

/** The box above the ways for one connection answer (texts may contain <b>, written by us). */
function netBoxHtml(n, lang, t) {
  const b = t.netBox[n];
  return (
    `<div class="netbox netbox-${n}" data-net="${n}" hidden><p class="netbox-h">${esc(b.h)}</p>` +
    b.p.map((x) => `<p>${esc(x)}</p>`).join('') +
    (b.li ? `<ul>${b.li.map((x) => `<li>${x}</li>`).join('')}</ul>` : '') +
    (b.more ? `<p class="netbox-more"><a href="${IT_LINK[lang]}">${esc(b.more)}</a></p>` : '') +
    `</div>`
  );
}

/** t = the finder texts of the language (strings in tools/site/text.*.mjs). */
export function renderFinder(lang, t, domain) {
  const netBtns = NETS.map((n) => `<button type="button" class="net" data-id="${n}" aria-pressed="false"><span>${esc(t.nets[n][0])}</span><small>${esc(t.nets[n][1])}</small></button>`).join('');
  const picker =
    `<div class="picker" hidden>` +
    `<div class="ask"><h3>${esc(t.qHave)}</h3>${picks(SENDERS, lang, t.qHave, ' five')}</div>` +
    `<div class="ask"><h3>${esc(t.qNet)}</h3><div class="nets" role="group" aria-label="${esc(t.qNet)}">${netBtns}</div></div>` +
    `<div class="ask"><h3>${esc(t.qScreen)}</h3>${picks(SCREENS, lang, t.qScreen, ' six')}</div>` +
    `<div class="answer-wrap" aria-live="polite">${NETS.map((n) => netBoxHtml(n, lang, t)).join('')}<div class="answer" id="answer"></div></div>` +
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
    `<div class="matrix"><p class="mnote">${esc(t.tableNote)}</p><table class="mtable"><caption class="sr">${esc(t.tableCaption)}</caption>` +
    `<thead><tr><th scope="col">${esc(t.tableTo)}</th><th scope="col">${esc(t.tableWays)}</th></tr></thead>${rows}</table></div>`;
  const data = ` data-default-net="${DEFAULT_NET}" data-v-one="${esc(t.verdictOne)}" data-v-many="${esc(t.verdictMany)}" data-v-none="${esc(t.verdictNone)}" data-v-nonet="${esc(t.verdictNoNet)}" data-v-across-one="${esc(t.verdictAcrossOne)}" data-v-across-many="${esc(t.verdictAcrossMany)}"`;
  return `<div class="finder" id="finder-app"${data}>${picker}${table}</div>`;
}
