// SPDX-License-Identifier: GPL-3.0-or-later
import { onBeforeUnmount, shallowRef } from 'vue'
import { ReceiverSession, type ReceiverState } from '../core/receiverSession'
import { signalingUrl } from '../core/signaling'
import { fetchIceServers, httpBaseUrl, obtainAltchaPayload } from '../core/altchaClient'

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
  const url = signalingUrl(location)
  const base = httpBaseUrl(url)
  const session = new ReceiverSession({
    url,
    onChange: (s) => (state.value = s),
    altcha: (signal) => obtainAltchaPayload(base, { signal }),
    iceServers: () => fetchIceServers(base),
  })
  onBeforeUnmount(() => session.stop())
  return { state, start: () => session.start(), stop: () => session.stop(), clearContent: () => session.clearContent() }
}
