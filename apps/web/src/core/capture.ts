// SPDX-License-Identifier: GPL-3.0-or-later

/**
 * TEST-ONLY: when the page URL has `?testVideo=1`, the sender sends an animated canvas instead of
 * the screen. This exists so automated tests can exercise signalling + WebRTC where
 * getDisplayMedia cannot be granted headlessly. Not a user feature: the flag only exists in the dev
 * server and in builds made with `VITE_E2E=1` (the end-to-end tests). A release build ignores it, so
 * a crafted link cannot make "Share screen" skip the browser's own capture prompt.
 */
const TEST_HOOKS: boolean = import.meta.env.DEV || import.meta.env.VITE_E2E === '1'

export function isTestVideo(search: string = typeof location !== 'undefined' ? location.search : ''): boolean {
  return TEST_HOOKS && new URLSearchParams(search).get('testVideo') === '1'
}

export function createTestStream(): MediaStream {
  const canvas = document.createElement('canvas')
  canvas.width = 640
  canvas.height = 360
  const ctx = canvas.getContext('2d')
  let n = 0
  // Redraw continuously so the encoder has new frames; the timer stops when the track ends.
  const timer = setInterval(() => {
    if (!ctx) return
    ctx.fillStyle = `hsl(${(n * 5) % 360} 60% 40%)`
    ctx.fillRect(0, 0, canvas.width, canvas.height)
    ctx.fillStyle = '#fff'
    ctx.font = '48px sans-serif'
    ctx.fillText(`TEST ${n++}`, 40, 200)
  }, 50)
  const stream = canvas.captureStream(20)
  for (const t of stream.getVideoTracks()) t.addEventListener('ended', () => clearInterval(timer))
  const stop = stream.getVideoTracks()[0]?.stop.bind(stream.getVideoTracks()[0])
  const track = stream.getVideoTracks()[0]
  if (track && stop) {
    track.stop = () => {
      clearInterval(timer)
      stop()
    }
  }
  return stream
}

/** Asks the browser for the screen (must be called from a user gesture). */
export async function getScreenStream(): Promise<MediaStream> {
  if (isTestVideo()) return createTestStream()
  return navigator.mediaDevices.getDisplayMedia({ video: true, audio: false })
}
