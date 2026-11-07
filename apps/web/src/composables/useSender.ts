// SPDX-License-Identifier: GPL-3.0-or-later
import { onBeforeUnmount, shallowRef } from 'vue'
import type { ControlCmd, ImageSource } from '../core/control'
import { getScreenStream } from '../core/capture'
import { SenderSession, type SenderState } from '../core/senderSession'
import { useRelay } from './useRelay'
import { fetchIceServers } from '../core/altchaClient'

/** `contentOnly`: no screen capture, only the control channel (images, links). */
export function useSender(opts: { contentOnly?: boolean } = {}) {
  const state = shallowRef<SenderState>({ phase: 'idle', errorCode: null, channelOpen: false })
  // The relay is fixed while a session exists; it is changed on the start page.
  // The url is read when the session starts, after the relay was confirmed (see start below).
  const { current, ensureTrusted } = useRelay()
  const session = new SenderSession({
    get url() {
      return current.value.wsUrl
    },
    iceServers: () => fetchIceServers(current.value.httpBase),
    onChange: (s) => (state.value = s),
    getStream: opts.contentOnly ? undefined : getScreenStream,
  })
  onBeforeUnmount(() => session.stop())
  return {
    state,
    // A relay that only a link asked for must be confirmed before anything is captured or connected.
    start: async (code: string) => {
      if (await ensureTrusted()) await session.start(code)
    },
    stop: () => session.stop(),
    sendImage: (f: ImageSource, onProgress?: (sent: number, total: number) => void) => session.sendImage(f, onProgress),
    sendDocument: (f: ImageSource, onProgress?: (sent: number, total: number) => void) => session.sendDocument(f, onProgress),
    sendLink: (url: string) => session.sendLink(url),
    sendCmd: (cmd: ControlCmd) => session.sendCmd(cmd),
  }
}
