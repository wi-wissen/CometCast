// SPDX-License-Identifier: GPL-3.0-or-later
import { onBeforeUnmount, shallowRef } from 'vue'
import { ReceiverSession, type ReceiverEvent, type ReceiverState } from '../core/receiverSession'
import { useRelay } from './useRelay'
import { fetchIceServers, obtainAltchaPayload } from '../core/altchaClient'
import { saveDefaultMode } from '../core/displaySetup'
import type { JoinMode } from '../core/signaling'

export interface UseReceiverOptions {
  /** The room's starting mode (the default chosen in the setup wizard; first run: open). */
  joinMode?: JoinMode
  onEvent?: (e: ReceiverEvent) => void
}

export function useReceiver(opts: UseReceiverOptions = {}) {
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
    joinMode: opts.joinMode ?? 'open',
    onEvent: opts.onEvent,
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
    // Join mode. Open and Approval on become the display's default (kept in the setup); Locked lasts
    // only until the session ends or the display restarts (spec 4.3), so it is never remembered.
    setJoinMode: (mode: JoinMode) => {
      if (mode !== 'locked') saveDefaultMode(mode)
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
