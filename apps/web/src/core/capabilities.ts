// SPDX-License-Identifier: GPL-3.0-or-later

export interface Capabilities {
  secureContext: boolean
  getDisplayMedia: boolean
  /** Screen sharing needs both. */
  screenShare: boolean
}

export function detectCapabilities(): Capabilities {
  const secureContext = typeof window !== 'undefined' && window.isSecureContext === true
  const getDisplayMedia =
    typeof navigator !== 'undefined' && typeof navigator.mediaDevices?.getDisplayMedia === 'function'
  return { secureContext, getDisplayMedia, screenShare: secureContext && getDisplayMedia }
}
