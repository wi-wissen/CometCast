// SPDX-License-Identifier: GPL-3.0-or-later
//
// Usage: node check-wasm-memory.mjs <file.wasm> <max pages>
// Parses the memory section of a .wasm file and exits with an error unless the first memory has a
// maximum of at most <max pages> (64 KiB each). Used by build-wasm.sh to prove that the cap really
// reached the module that ships (a linker flag that is silently dropped would not be noticed).
import { readFileSync } from 'node:fs'

const [file, limitArg] = process.argv.slice(2)
const limit = Number(limitArg)
if (!file || !Number.isInteger(limit) || limit <= 0) {
  console.error('usage: check-wasm-memory.mjs <file.wasm> <max pages>')
  process.exit(2)
}
const b = readFileSync(file)
let p = 0
const leb = () => {
  let r = 0n
  let shift = 0n
  for (;;) {
    if (p >= b.length) throw new Error('truncated module')
    const x = b[p++]
    r |= BigInt(x & 0x7f) << shift
    if ((x & 0x80) === 0) return Number(r)
    shift += 7n
  }
}
if (b.length < 8 || b.readUInt32BE(0) !== 0x0061736d) {
  console.error(`${file}: not a wasm module`)
  process.exit(1)
}
p = 8
let result = null
while (p < b.length && result === null) {
  const id = b[p++]
  const size = leb()
  const end = p + size
  if (id === 5) {
    const count = leb()
    if (count < 1) break
    const flags = leb()
    const min = leb()
    const max = flags & 1 ? leb() : null
    result = { min, max }
  }
  p = end
}
if (!result) {
  console.error(`${file}: no memory section found`)
  process.exit(1)
}
if (result.max === null || result.max > limit) {
  const got = result.max === null ? 'unbounded' : `${result.max} pages`
  console.error(`${file}: memory max is ${got}, expected at most ${limit} pages (the memory cap was not applied)`)
  process.exit(1)
}
console.log(`memory cap ok: min ${result.min} pages, max ${result.max} pages (${(result.max * 64) / 1024} MiB)`)
