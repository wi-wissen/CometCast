// SPDX-License-Identifier: GPL-3.0-or-later
//
// First-run setup of the display (docs/design/first-run-and-display.md, 4.1). Finish stores the
// room name, the default join mode and the server; the wizard is shown only until then.
// Storage is the display device's own localStorage (every access wrapped).
import type { JoinMode } from './signaling'

export const SETUP_KEY = 'cometcast.display.setup'
export const ROOM_NAME_MAX = 32
export const NAME_SUGGESTION_KEYS = ['setup.name.s1', 'setup.name.s2', 'setup.name.s3', 'setup.name.s4', 'setup.name.s5'] as const

/** Locked is not offered as a default; it only exists during a session. */
export type DefaultMode = Exclude<JoinMode, 'locked'>

export interface DisplaySetup {
  room: string
  mode: DefaultMode
  /** wss:// URL of the relay chosen in step 3, or null for "whatever the page resolves". */
  server: string | null
}

export function cleanRoomName(raw: string): string {
  return Array.from(raw.replace(/[\u0000-\u001f\u007f-\u009f]/g, '').replace(/\s+/g, ' ').trim())
    .slice(0, ROOM_NAME_MAX)
    .join('')
}

export function loadSetup(): DisplaySetup | null {
  try {
    const raw = localStorage.getItem(SETUP_KEY)
    if (!raw) return null
    const v = JSON.parse(raw) as Partial<DisplaySetup> | null
    if (!v || typeof v !== 'object') return null
    const room = typeof v.room === 'string' ? cleanRoomName(v.room) : ''
    if (!room) return null
    const mode: DefaultMode = v.mode === 'confirm' ? 'confirm' : 'open'
    const server = typeof v.server === 'string' && v.server.length < 300 ? v.server : null
    return { room, mode, server }
  } catch {
    return null
  }
}

export function saveSetup(s: DisplaySetup): void {
  try {
    localStorage.setItem(SETUP_KEY, JSON.stringify(s))
  } catch {
    /* not remembered: the wizard shows again after a reload */
  }
}

/** Updates only the default mode of an existing setup (Settings changes Open or Approval on). */
export function saveDefaultMode(mode: DefaultMode): void {
  const s = loadSetup()
  if (s) saveSetup({ ...s, mode })
}

// --- the wizard as a pure state machine -------------------------------------------------------

export type SetupStep = 0 | 1 | 2 | 3 | 4
export interface WizardState {
  step: SetupStep
  room: string
  mode: DefaultMode
}
export type WizardAction =
  | { type: 'next' }
  | { type: 'back' }
  | { type: 'room'; value: string }
  | { type: 'mode'; value: DefaultMode }

export function initialWizard(room: string): WizardState {
  // The first suggestion is preselected; Open is preselected (D1).
  return { step: 0, room: cleanRoomName(room), mode: 'open' }
}

export function wizardReduce(s: WizardState, a: WizardAction): WizardState {
  switch (a.type) {
    case 'next':
      return s.step >= 4 ? s : { ...s, step: (s.step + 1) as SetupStep }
    case 'back':
      // Back on the first screen does nothing.
      return s.step <= 0 ? s : { ...s, step: (s.step - 1) as SetupStep }
    case 'room':
      return { ...s, room: cleanRoomName(a.value) }
    case 'mode':
      return { ...s, mode: a.value }
  }
}
