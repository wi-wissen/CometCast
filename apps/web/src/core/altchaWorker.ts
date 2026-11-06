// SPDX-License-Identifier: GPL-3.0-or-later
//
// Web Worker that solves one ALTCHA puzzle off the main thread (so the page stays responsive).
// Protocol: main -> worker {challenge, timeoutMs}; worker -> main {ok:true, solution} or
// {ok:false, reason}. Used through solveInWorker() in altchaClient.ts.
import { AltchaError, solveChallenge, type AltchaChallenge, type AltchaSolution } from './altcha'

export type WorkerRequest = { challenge: AltchaChallenge; timeoutMs?: number }
export type WorkerReply = { ok: true; solution: AltchaSolution } | { ok: false; reason: string }

self.onmessage = async (ev: MessageEvent<WorkerRequest>) => {
  let reply: WorkerReply
  try {
    reply = { ok: true, solution: await solveChallenge(ev.data.challenge, { timeoutMs: ev.data.timeoutMs }) }
  } catch (e) {
    reply = { ok: false, reason: e instanceof AltchaError ? e.reason : 'bad_challenge' }
  }
  ;(self as unknown as { postMessage(m: WorkerReply): void }).postMessage(reply)
}
