// SPDX-License-Identifier: GPL-3.0-or-later
import { onBeforeUnmount, shallowRef } from 'vue'
import type { ControlCmd, ImageSource } from '../core/control'
import { getScreenStream } from '../core/capture'
import { SenderSession, type SenderState } from '../core/senderSession'
import { signalingUrl } from '../core/signaling'

/** `contentOnly`: no screen capture, only the control channel (images, links). */
export function useSender(opts: { contentOnly?: boolean } = {}) {
  const state = shallowRef<SenderState>({ phase: 'idle', errorCode: null, channelOpen: false })
  const session = new SenderSession({
    url: signalingUrl(location),
    onChange: (s) => (state.value = s),
    getStream: opts.contentOnly ? undefined : getScreenStream,
  })
  onBeforeUnmount(() => session.stop())
  return {
    state,
    start: (code: string) => session.start(code), stop: () => session.stop(),
    sendImage: (f: ImageSource, onProgress?: (sent: number, total: number) => void) => session.sendImage(f, onProgress),
    sendLink: (url: string) => session.sendLink(url),
    sendCmd: (cmd: ControlCmd) => session.sendCmd(cmd),
  }
}
