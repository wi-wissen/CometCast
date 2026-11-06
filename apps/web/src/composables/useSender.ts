// SPDX-License-Identifier: GPL-3.0-or-later
import { onBeforeUnmount, shallowRef } from 'vue'
import { getScreenStream } from '../core/capture'
import { SenderSession, type SenderState } from '../core/senderSession'
import { signalingUrl } from '../core/signaling'

export function useSender() {
  const state = shallowRef<SenderState>({ phase: 'idle', errorCode: null })
  const session = new SenderSession({
    url: signalingUrl(location),
    onChange: (s) => (state.value = s),
    getStream: getScreenStream,
  })
  onBeforeUnmount(() => session.stop())
  return { state, start: (code: string) => session.start(code), stop: () => session.stop() }
}
