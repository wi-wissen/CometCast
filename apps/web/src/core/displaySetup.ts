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
  /** The room name: the chosen suggestion, or the cleaned custom text, whichever is set. */
  room: string
  /** What is typed into the custom field (as typed). It starts empty (spec 4.1). */
  custom: string
  /** The suggestion that was selected before anything was typed: it is selected again when the field is emptied (decision 13.39). */
  chip: string
  mode: DefaultMode
}
export type WizardAction =
  | { type: 'next' }
  | { type: 'back' }
  /** A suggestion chip was chosen: it clears the custom field and becomes the one to return to. */
  | { type: 'room'; value: string }
  /** The custom field changed: a non-empty text deselects all chips. */
  | { type: 'custom'; value: string }
  | { type: 'mode'; value: DefaultMode }

export function initialWizard(room: string): WizardState {
  // The first suggestion is preselected, the custom field starts empty; Open is preselected (D1).
  return { step: 0, room: cleanRoomName(room), custom: '', chip: cleanRoomName(room), mode: 'open' }
}

/** Is this suggestion the selected one? Never together with a non-empty custom name (a name of only spaces counts as empty). */
export function chipSelected(s: WizardState, suggestion: string): boolean {
  return cleanRoomName(s.custom) === '' && s.room === cleanRoomName(suggestion)
}

export function wizardReduce(s: WizardState, a: WizardAction): WizardState {
  switch (a.type) {
    case 'next':
      return s.step >= 4 ? s : { ...s, step: (s.step + 1) as SetupStep }
    case 'back':
      // Back on the first screen does nothing.
      return s.step <= 0 ? s : { ...s, step: (s.step - 1) as SetupStep }
    case 'room': {
      const room = cleanRoomName(a.value)
      return { ...s, room, custom: '', chip: room }
    }
    case 'custom': {
      const custom = Array.from(a.value).slice(0, ROOM_NAME_MAX).join('')
      const name = cleanRoomName(custom)
      // Emptied again (or only spaces): the chip that was selected before typing is selected again, so exactly one name
      // is always visible as chosen (spec 4.1, decision 13.39).
      return { ...s, custom, room: name === '' ? s.chip : name }
    }
    case 'mode':
      return { ...s, mode: a.value }
  }
}

// --- the address step (web) --------------------------------------------------------------------------------

/**
 * Which line explains where the address comes from (spec 4.1, web): `sourceDefault` for the default relay (the one built into this
 * page), `sourcePage` for a relay that comes from the page's own server, its `config.json` or its link, `sourceTyped` for an
 * address typed into the field. `source` is the relay's own source (core/relay.ts); `inChoices`: it is one of the relays the page offers.
 */
export function addressSourceKey(o: { typed: boolean; source: 'user' | 'query' | 'config' | 'build' | 'origin'; inChoices: boolean }): 'setup.address.sourceTyped' | 'setup.address.sourceDefault' | 'setup.address.sourcePage' {
  // A relay that was chosen in this tab and is not one the page offers was typed (also after a reload of the tab).
  if (o.typed || (o.source === 'user' && !o.inChoices)) return 'setup.address.sourceTyped'
  return o.source === 'build' ? 'setup.address.sourceDefault' : 'setup.address.sourcePage'
}

/**
 * "Other address": it goes through the relays of config.json (`choices`, default first), then opens the text field.
 * With a single relay (or when the current address is the last one, or typed) it opens the field at once.
 */
export function nextAddressStep(choices: string[], current: string, typed: boolean): { kind: 'choice'; ws: string } | { kind: 'field' } {
  const i = typed ? -1 : choices.indexOf(current)
  return i >= 0 && i < choices.length - 1 ? { kind: 'choice', ws: choices[i + 1] } : { kind: 'field' }
}
