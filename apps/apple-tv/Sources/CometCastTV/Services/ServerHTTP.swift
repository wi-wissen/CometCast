// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here).
//
// The two HTTP endpoints next to the WebSocket (docs/server.md): GET /altcha/challenge (solved here) and
// GET /ice-servers. Bodies are capped (256 KiB), redirects are not followed, no cookies, no cache.
// Errors use the vocabulary of the signalling errors (rate_limited, banned, forbidden, server_full, altcha_*).

import CometCastCore
import Foundation

struct ServerError: Error { let code: String }

private final class NoRedirect: NSObject, URLSessionTaskDelegate {
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
                    newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) {
        completionHandler(nil)
    }
}

enum ServerHTTP {
    private static let maxBody = 256 * 1024
    /// UNCERTAIN: placeholders as in the web client (8 s challenge, 5 s ICE list).
    static let challengeTimeout: TimeInterval = 8
    static let iceTimeout: TimeInterval = 5

    private static let session: URLSession = {
        let cfg = URLSessionConfiguration.ephemeral
        cfg.httpCookieStorage = nil
        cfg.httpShouldSetCookies = false
        cfg.urlCache = nil
        cfg.requestCachePolicy = .reloadIgnoringLocalAndRemoteCacheData
        return URLSession(configuration: cfg, delegate: NoRedirect(), delegateQueue: nil)
    }()

    /// (status, Retry-After, body). Throws on network errors and on a body over the cap.
    private static func get(_ url: URL, timeout: TimeInterval) async throws -> (Int, String?, Data) {
        var req = URLRequest(url: url)
        req.timeoutInterval = timeout
        let (bytes, resp) = try await session.bytes(for: req)
        guard let http = resp as? HTTPURLResponse else { throw ServerError(code: "altcha_failed") }
        var data = Data()
        for try await b in bytes {
            data.append(b)
            if data.count > maxBody { throw ServerError(code: "altcha_failed") }
        }
        return (http.statusCode, http.value(forHTTPHeaderField: "Retry-After"), data)
    }

    /// Fetches a puzzle, solves it (off the main thread) and returns the text for create_room.altcha.
    /// Throws ServerError with a code that has a translation under `errors.<code>`.
    static func altchaPayload(base: String, isCancelled: @escaping @Sendable () -> Bool) async throws -> String {
        guard let url = URL(string: base + "/altcha/challenge") else { throw ServerError(code: "altcha_failed") }
        let status: Int, retry: String?, body: Data
        do { (status, retry, body) = try await get(url, timeout: challengeTimeout) } catch let e as ServerError { throw e } catch { throw ServerError(code: "altcha_failed") }
        guard status == 200 else { throw ServerError(code: httpErrorCode(status: status, retryAfter: retry)) }
        guard let text = String(data: body, encoding: .utf8), let challenge = parseAltchaChallenge(text) else { throw ServerError(code: "altcha_failed") }
        do { try checkChallengeLimits(challenge) } catch let e as AltchaError { throw ServerError(code: altchaErrorCode(e)) }
        return try await Task.detached(priority: .userInitiated) {
            do {
                let sol = try solveAltcha(challenge, isCancelled: isCancelled)
                return encodeAltchaPayload(challenge: challenge, solution: sol)
            } catch let e as AltchaError {
                throw ServerError(code: e.reason == "aborted" ? "aborted" : altchaErrorCode(e))
            }
        }.value
    }

    /// GET /ice-servers. 404 (no TURN configured) and every failure mean "no list".
    static func iceServers(base: String) async -> [IceServerConfig] {
        guard let url = URL(string: base + "/ice-servers"), let (status, _, body) = try? await get(url, timeout: iceTimeout),
              status == 200, let text = String(data: body, encoding: .utf8) else { return [] }
        return parseIceServers(text)
    }
}
