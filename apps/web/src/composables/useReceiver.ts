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
  const { current } = useRelay()
  const url = current.value.wsUrl
  const base = current.value.httpBase
  const session = new ReceiverSession({
    url,
    onChange: (s) => (state.value = s),
    altcha: (signal) => obtainAltchaPayload(base, { signal }),
    iceServers: () => fetchIceServers(base),
  })
  onBeforeUnmount(() => session.stop())
  return {
    state,
    start: () => session.start(),
    stop: () => session.stop(),
    clearContent: () => session.clearContent(),
    nextPage: () => session.nextPage(),
    prevPage: () => session.prevPage(),
    renderPage: (page: number, width: number) => session.renderPage(page, width),
  }
}
