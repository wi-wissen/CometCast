// SPDX-License-Identifier: GPL-3.0-or-later
import { onBeforeUnmount, shallowRef } from 'vue'
import { ReceiverSession, type ReceiverState } from '../core/receiverSession'
import { useRelay } from './useRelay'
import { fetchIceServers, obtainAltchaPayload } from '../core/altchaClient'

export function useReceiver() {
  // shallowRef: MediaStream must not be wrapped in a reactive proxy.
  const state = shallowRef<ReceiverState>({
    phase: 'idle',
    code: null,
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
    nextPage: () => session.nextPage(),
    prevPage: () => session.prevPage(),
    renderPage: (page: number, width: number) => session.renderPage(page, width),
  }
}
