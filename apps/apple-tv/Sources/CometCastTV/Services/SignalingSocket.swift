// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here).
//
// WebSocket to the signalling server (docs/protocol.md): one JSON text frame per message, binary frames are
// ignored. hello is sent when the socket opens; the server's hello is checked for the protocol version (2, with join approval;
// the join approval messages are plain `ClientMessage`/`ServerMessage` values, so nothing else is needed here).

import CometCastCore
import Foundation

@MainActor
final class SignalingSocket {
    enum Event {
        case ready
        case message(ServerMessage)
        case closed
    }

    private let task: URLSessionWebSocketTask
    private let session: URLSession
    private var closedByUs = false
    private var receiveTask: Task<Void, Never>?
    private let onEvent: (Event) -> Void

    /// UNCERTAIN: 256 KiB; the server's own limit is 64 KiB (docs/protocol.md).
    private static let maxMessageBytes = 256 * 1024

    init(url: URL, onEvent: @escaping (Event) -> Void) {
        let cfg = URLSessionConfiguration.ephemeral
        cfg.httpCookieStorage = nil
        cfg.urlCache = nil
        cfg.timeoutIntervalForRequest = 15
        session = URLSession(configuration: cfg)
        var req = URLRequest(url: url)
        req.timeoutInterval = 15
        task = session.webSocketTask(with: req)
        task.maximumMessageSize = Self.maxMessageBytes
        self.onEvent = onEvent
    }

    func open() {
        task.resume()
        send(ClientMessage.hello(role: .receiver))
        receiveTask = Task { [weak self] in await self?.receiveLoop() }
    }

    func send(_ m: ClientMessage) {
        let text = m.encode()
        task.send(.string(text)) { _ in } // a failing send ends the receive loop with an error as well
    }

    func close() {
        closedByUs = true
        receiveTask?.cancel()
        task.cancel(with: .normalClosure, reason: nil)
        session.invalidateAndCancel()
    }

    private func receiveLoop() async {
        while !Task.isCancelled {
            do {
                let msg = try await task.receive()
                guard case .string(let text) = msg, let m = ServerMessage.decode(text) else { continue } // binary or invalid: ignored
                if case .hello(let v) = m {
                    if v != Double(protoVersion) {
                        onEvent(.message(.error(code: "proto_mismatch", message: "protocol version differs")))
                        close()
                        return
                    }
                    onEvent(.ready)
                } else {
                    onEvent(.message(m))
                }
            } catch {
                if !closedByUs { onEvent(.closed) }
                return
            }
        }
    }
}
