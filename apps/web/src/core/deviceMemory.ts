// SPDX-License-Identifier: GPL-3.0-or-later
//
// The display's memory of who used a name (docs/design/presenter-identity.md); the same rules as `DeviceMemory` in
// cc-receiver.
//
// Pairs (cleaned name, device id) of every request the display ALLOWED and of every presenter that joined in open mode with
// a valid proof, at most 200, the least recently used dropped first. A request or open-mode join with a valid proof whose
// name (compared without regard to case) is stored ONLY with other device ids gets a note. A name stored with this id, or
// never seen, gets none. The names the server makes up (`Guest N`) are never stored or compared.
//
// Stored in localStorage like the other display settings; every access is wrapped (private windows, blocked site data).

export const DEVICE_MEMORY_KEY = 'cometcast.deviceMemory'
export const MAX_PAIRS = 200
const MAX_NAME_CHARS = 40

/** The label that the server gives somebody who sent no name: `Guest ` and digits. */
export function isGeneratedName(name: string): boolean {
  return /^Guest [0-9]+$/.test(name)
}

const isDeviceId = (id: unknown): id is string => typeof id === 'string' && /^[0-9a-f]{32}$/.test(id)
const keyOf = (name: string): string => name.trim().toLowerCase()

export class DeviceMemory {
  /** Least recently used first. */
  private pairs: Array<[string, string]> = []

  get size(): number {
    return this.pairs.length
  }

  /** Reads what {@link toJSON} wrote; anything else (garbage, bad ids, generated names) is skipped. */
  static fromJSON(text: string | null | undefined): DeviceMemory {
    const m = new DeviceMemory()
    try {
      const v: unknown = JSON.parse(text ?? '')
      if (Array.isArray(v)) {
        for (const p of v) {
          if (Array.isArray(p) && p.length === 2 && typeof p[0] === 'string') m.remember(p[0], p[1])
        }
      }
    } catch {
      /* an empty memory */
    }
    return m
  }

  toJSON(): string {
    return JSON.stringify(this.pairs)
  }

  /** The note: the name is stored, but only with device ids other than `id`. Never for a generated name. */
  usedByOtherDevice(name: string, id: string): boolean {
    if (isGeneratedName(name)) return false
    const key = keyOf(name)
    let seen = false
    for (const [n, i] of this.pairs) {
      if (keyOf(n) !== key) continue
      if (i === id) return false
      seen = true
    }
    return seen
  }

  /** Remembers that `id` was allowed under `name`. Returns true when something changed. */
  remember(name: string, id: unknown): boolean {
    const clean = Array.from(name.trim()).slice(0, MAX_NAME_CHARS).join('')
    if (clean === '' || isGeneratedName(clean) || !isDeviceId(id)) return false
    const key = keyOf(clean)
    const before = this.toJSON()
    this.pairs = this.pairs.filter(([n, i]) => !(i === id && keyOf(n) === key))
    this.pairs.push([clean, id])
    if (this.pairs.length > MAX_PAIRS) this.pairs.splice(0, this.pairs.length - MAX_PAIRS)
    return this.toJSON() !== before
  }
}

/** What the receiver session needs: the note, and the memory of an allowed pair. */
export interface DeviceMemoryStore {
  usedByOtherDevice(name: string, id: string): boolean
  remember(name: string, id: string): void
}

/** The memory in localStorage (loaded at every use, so several tabs share it; failures leave it empty). */
export function localStorageDeviceMemory(): DeviceMemoryStore {
  const load = (): DeviceMemory => {
    try {
      return DeviceMemory.fromJSON(localStorage.getItem(DEVICE_MEMORY_KEY))
    } catch {
      return new DeviceMemory()
    }
  }
  return {
    usedByOtherDevice: (name, id) => load().usedByOtherDevice(name, id),
    remember(name, id) {
      const m = load()
      if (!m.remember(name, id)) return
      try {
        localStorage.setItem(DEVICE_MEMORY_KEY, m.toJSON())
      } catch {
        /* not remembered */
      }
    },
  }
}
