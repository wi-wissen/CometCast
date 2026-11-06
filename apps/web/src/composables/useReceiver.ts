// SPDX-License-Identifier: GPL-3.0-or-later
import { onBeforeUnmount, shallowRef } from 'vue'
import { ReceiverSession, type ReceiverState } from '../core/receiverSession'
import { signalingUrl } from '../core/signaling'

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
  const session = new ReceiverSession({ url: signalingUrl(location), onChange: (s) => (state.value = s) })
  onBeforeUnmount(() => session.stop())
  return { state, start: () => session.start(), stop: () => session.stop(), clearContent: () => session.clearContent() }
}
