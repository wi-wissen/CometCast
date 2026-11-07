// SPDX-License-Identifier: GPL-3.0-or-later
// The few Node functions that tests use to read source files (the project has no @types/node).
declare module 'node:fs' {
  export function readFileSync(path: string, encoding: 'utf8'): string
}
declare module 'node:url' {
  export function fileURLToPath(url: string | URL): string
}
