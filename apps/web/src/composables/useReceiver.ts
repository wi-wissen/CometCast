// SPDX-License-Identifier: GPL-3.0-or-later
import { onBeforeUnmount, shallowRef } from 'vue'
import { ReceiverSession, type ReceiverState } from '../core/receiverSession'
import { useRelay } from './useRelay'
import { fetchIceServers, obtainAltchaPayload } from '../core/altchaClient'
import { loadJoinMode, saveJoinMode } from '../core/joinPrefs'
import type { JoinMode } from '../core/signaling'

export function useReceiver() {
  // shallowRef: MediaStream must not be wrapped in a reactive proxy.
  const state = shallowRef<ReceiverState>({
    phase: 'idle',
    code: null,
    joinMode: 'open',
    requests: [],
    peerName: null,
    blocks: [],
    notice: null,
    errorCode: null,
    stream: null,
    content: null,
    transfer: null,
    contentError: null,
  })
  // The relay is fixed while a session exists; it is changed on the start page.
  // The url is read when the session starts, after the relay was confirmed (see start below).
  const { current, ensureTrusted } = useRelay()
  const session = new ReceiverSession({
    get url() {
      return current.value.wsUrl
    },
    onChange: (s) => (state.value = s),
    altcha: (signal) => obtainAltchaPayload(current.value.httpBase, { signal }),
    iceServers: () => fetchIceServers(current.value.httpBase),
    // The mode the person chose last (first run: open).
    joinMode: loadJoinMode(),
  })
  onBeforeUnmount(() => session.stop())
  return {
    state,
    // A relay that only a link asked for must be confirmed before anything is connected.
    start: async () => {
      if (await ensureTrusted()) session.start()
    },
    stop: () => session.stop(),
    clearContent: () => session.clearContent(),
    // Join approval. The chosen mode is remembered for the next room (a convenience only).
    setJoinMode: (mode: JoinMode) => {
      saveJoinMode(mode)
      session.setJoinMode(mode)
    },
    decide: (id: number, allow: boolean, block = false) => session.decide(id, allow, block),
    denyAll: () => session.denyAll(),
    removePeer: (block = false) => session.removePeer(block),
    unblock: (id: number) => session.unblock(id),
    endRoom: () => session.endRoom(),
    dismissNotice: () => session.dismissNotice(),
    nextPage: () => session.nextPage(),
    prevPage: () => session.prevPage(),
    renderPage: (page: number, width: number) => session.renderPage(page, width),
  }
}
