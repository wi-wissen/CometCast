// SPDX-License-Identifier: GPL-3.0-or-later
import { onBeforeUnmount, shallowRef } from 'vue'
import type { ControlCmd, ImageSource } from '../core/control'
import { getScreenStream } from '../core/capture'
import { SenderSession, type SenderState } from '../core/senderSession'
import { useRelay } from './useRelay'
import { fetchIceServers, type IceFetchState } from '../core/altchaClient'
import { loadDisplayName } from '../core/joinPrefs'

/** `contentOnly`: no screen capture, only the control channel (images, links). */
export function useSender(opts: { contentOnly?: boolean } = {}) {
  const state = shallowRef<SenderState>({ phase: 'idle', errorCode: null, channelOpen: false })
  // The relay is fixed while a session exists; it is changed on the start page.
  // The url is read when the session starts, after the relay was confirmed (see start below).
  const { current, ensureTrusted } = useRelay()
  let iceState: IceFetchState | undefined
  const session = new SenderSession({
    get url() {
      return current.value.wsUrl
    },
    iceServers: () => fetchIceServers(current.value.httpBase, { onState: (s) => (iceState = s) }),
    iceState: () => iceState,
    onChange: (s) => (state.value = s),
    // Shown to the host when it has to approve the join (read when the join is sent).
    name: () => loadDisplayName() || undefined,
    getStream: opts.contentOnly ? undefined : getScreenStream,
  })
  onBeforeUnmount(() => session.stop())
  return {
    state,
    // A relay that only a link asked for must be confirmed before anything is captured or connected.
    // `capture: false` joins without asking for the screen (the screen is added later with shareScreen).
    start: async (code: string, opts: { capture?: boolean } = {}) => {
      if (await ensureTrusted()) await session.start(code, opts)
    },
    stop: () => session.stop(),
    shareScreen: () => session.shareScreen(),
    stopScreen: () => session.stopScreen(),
    sendImage: (f: ImageSource, onProgress?: (sent: number, total: number) => void) => session.sendImage(f, onProgress),
    sendDocument: (f: ImageSource, onProgress?: (sent: number, total: number) => void) => session.sendDocument(f, onProgress),
    sendLink: (url: string) => session.sendLink(url),
    sendCmd: (cmd: ControlCmd) => session.sendCmd(cmd),
  }
}
