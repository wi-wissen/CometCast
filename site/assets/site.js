// SPDX-License-Identifier: GPL-3.0-or-later
// The only script of the site (no frameworks, no requests, no cookies). Three small jobs:
//  1. the language hint (only when the browser prefers the other offered language; closing it is remembered)
//  2. the finder: turns the static answer table into a picker (the table stays for visitors without JavaScript)
//  3. the download page: moves the card that fits the visitor's device to the top
(() => {
  'use strict'
  const KEY = 'cometcast.langhint'
  const remember = () => { try { localStorage.setItem(KEY, '1') } catch (e) { /* private mode: it just shows again */ } }
  const remembered = () => { try { return localStorage.getItem(KEY) === '1' } catch (e) { return false } }

  function langHint() {
    const el = document.querySelector('.langhint')
    if (!el || remembered()) return
    const list = navigator.languages && navigator.languages.length ? navigator.languages : [navigator.language || '']
    let pref = ''
    for (const l of list) {
      const m = /^(en|de)(?:[-_]|$)/i.exec(l || '')
      if (m) { pref = m[1].toLowerCase(); break }
    }
    if (!pref || pref !== el.dataset.target) return
    el.hidden = false
    el.querySelector('[data-hint-close]').addEventListener('click', () => { remember(); el.hidden = true })
    el.querySelector('[data-hint-link]').addEventListener('click', remember)
  }

  function device() {
    const ua = (navigator.userAgent || '').toLowerCase()
    const plat = ((navigator.userAgentData && navigator.userAgentData.platform) || navigator.platform || '').toLowerCase()
    if (ua.indexOf('android') >= 0) return 'android'
    if (/iphone|ipad|ipod/.test(ua) || (plat.indexOf('mac') >= 0 && navigator.maxTouchPoints > 1)) return 'ios'
    if (plat.indexOf('win') >= 0 || ua.indexOf('windows') >= 0) return 'windows'
    if (plat.indexOf('mac') >= 0) return 'macos'
    if (plat.indexOf('linux') >= 0 || ua.indexOf('x11') >= 0) return 'linux'
    return ''
  }

  function finder() {
    const root = document.getElementById('finder-app')
    if (!root) return
    const picker = root.querySelector('.picker')
    const matrix = root.querySelector('.matrix')
    const answer = root.querySelector('#answer')
    const groups = picker.querySelectorAll('.picks')
    const sBtns = [].slice.call(groups[0].querySelectorAll('.pick'))
    const rBtns = [].slice.call(groups[1].querySelectorAll('.pick'))
    const nBtns = [].slice.call(picker.querySelectorAll('.net'))
    const ids = (btns) => btns.map((b) => b.dataset.id)
    const dflt = { s: { windows: 'win', macos: 'mac', linux: 'linux', ios: 'iphone', android: 'android' }[device()] || 'win', r: 'appletv', n: root.dataset.defaultNet }
    const st = { s: dflt.s, r: dflt.r, n: dflt.n }

    function fromHash() {
      const m = /^#([a-z]+)-([a-z]+)(?:-([a-z]+))?$/.exec(location.hash)
      if (!m || ids(sBtns).indexOf(m[1]) < 0 || ids(rBtns).indexOf(m[2]) < 0) return false
      st.s = m[1]; st.r = m[2]
      st.n = m[3] && ids(nBtns).indexOf(m[3]) >= 0 ? m[3] : dflt.n
      return true
    }
    const cap = (x) => x.charAt(0).toUpperCase() + x.slice(1)

    function render() {
      sBtns.forEach((b) => b.setAttribute('aria-pressed', String(b.dataset.id === st.s)))
      rBtns.forEach((b) => b.setAttribute('aria-pressed', String(b.dataset.id === st.r)))
      nBtns.forEach((b) => b.setAttribute('aria-pressed', String(b.dataset.id === st.n)))
      const row = matrix.querySelector('tr[data-s="' + st.s + '"][data-r="' + st.r + '"]')
      const box = row.querySelector('td').cloneNode(true)
      const verdict = box.querySelector('.verdict')
      const list = box.querySelector('.ways')
      if (list) {
        let ways = [].slice.call(list.children)
        if (st.n === 'diff') ways = ways.filter((w) => w.dataset.kind !== 'builtin')
        // away from home, the way that also works across networks comes first, then the built-in one (stable order inside each)
        if (st.n === 'office') ways = ways.filter((w) => w.dataset.kind === 'comet').concat(ways.filter((w) => w.dataset.kind === 'builtin'))
        ways.forEach((w) => {
          const chip = w.querySelector('.netchip')
          const text = chip.dataset['text' + cap(st.n)] || ''
          chip.textContent = text
          chip.hidden = !text
        })
        list.textContent = ''
        ways.forEach((w) => list.appendChild(w))
        if (!ways.length) {
          list.remove()
          verdict.className = 'verdict no'
          verdict.textContent = root.dataset.vNonet
        } else {
          verdict.textContent = ways.length === 1 ? root.dataset.vOne : root.dataset.vMany.replace('{n}', String(ways.length))
        }
      }
      answer.textContent = ''
      while (box.firstChild) answer.appendChild(box.firstChild)
    }
    function choose(key, id) {
      st[key] = id
      render()
      try { history.replaceState(null, '', '#' + st.s + '-' + st.r + (st.n === dflt.n ? '' : '-' + st.n)) } catch (e) { /* ignore */ }
    }
    sBtns.forEach((b) => b.addEventListener('click', () => choose('s', b.dataset.id)))
    rBtns.forEach((b) => b.addEventListener('click', () => choose('r', b.dataset.id)))
    nBtns.forEach((b) => b.addEventListener('click', () => choose('n', b.dataset.id)))
    window.addEventListener('hashchange', () => { if (fromHash()) render() })

    const linked = fromHash()
    picker.hidden = false
    matrix.hidden = true
    render()
    if (linked) { const sec = document.getElementById('finder'); if (sec) sec.scrollIntoView() }
  }

  function downloads() {
    const box = document.getElementById('downloads')
    const card = box && box.querySelector('[data-platform="' + device() + '"]')
    if (!card || !card.dataset.yours) return
    const tag = document.createElement('p')
    tag.className = 'yours'
    tag.textContent = card.dataset.yours
    card.classList.add('is-yours')
    card.querySelector('.dl-main').insertBefore(tag, card.querySelector('h3'))
    box.insertBefore(card, box.firstChild)
  }

  langHint()
  finder()
  downloads()
})()
