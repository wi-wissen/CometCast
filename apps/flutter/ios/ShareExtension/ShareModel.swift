// SPDX-License-Identifier: GPL-3.0-or-later
// The state of the share sheet (native-apps.md 21.3, spec 15.3): the item, the page, and the
// session of libcc_ios.a (`cc_ios_share_*`, polled a few times a second). No Flutter engine: the
// extension has far less memory than the app.

import Foundation
import SwiftUI

/// What the person shares.
struct SharedItem: Equatable {
    enum Kind: Equatable { case image, pdf, link }
    var kind: Kind
    /// A copy of the file in the App Group container (`share/items/`), or nil for a link.
    var fileURL: URL?
    /// The file name, or the link text.
    var title: String
    var size: Int
    /// The link, for `.link`.
    var link: String?
}

enum SharePage: Equatable {
    case share
    case relayConfirm
    case waiting
    case presenting
    case error(ShareErrorPage)
}

@MainActor
final class ShareModel: ObservableObject {
    @Published var page: SharePage = .share
    @Published var item: SharedItem?
    /// The reason an item cannot be shown (`sendReject*` key), shown on the item card.
    @Published var refusal: String?
    @Published var name: String
    @Published var code: String = ""
    @Published var codeError = false
    @Published var server: String
    @Published var roomName: String?
    @Published var check = 0
    @Published var timeoutSecs = 0
    @Published var sas: String?
    @Published var pageNumber = 0
    @Published var pageCount = 0
    @Published var progress: Double = 0
    @Published var connecting = false

    private let settings = ShareSettings()
    private let store: ShareSettingsStore?
    private var confirmed: [String]
    private var timer: Timer?
    private var shown = false
    /// Called when the sheet must close (`share.stopClose`, Cancel).
    var close: (() -> Void)?

    init(group: String = BroadcastIdentity.appGroup) {
        let store = ShareSettingsStore.shared(group: group)
        let s = store?.read() ?? ShareSettings()
        self.store = store
        name = s.name
        server = s.lastServer ?? "relay.cometcast.de:443"
        confirmed = s.confirmedServers
    }

    var via: String { joinAddress(server) }

    // MARK: item

    func setItem(_ item: SharedItem?, refusal: String? = nil) {
        self.item = item
        self.refusal = refusal
    }

    // MARK: join and show

    /// `share.show`: validate like Connect, then (for an untrusted server) the relay page, then join.
    func show() {
        guard item != nil, refusal == nil else { return }
        let normalized = code.uppercased().filter { !$0.isWhitespace && $0 != "-" }
        guard normalized.count == 6, normalized.allSatisfy({ "ABCDEFGHJKLMNPQRSTUVWXYZ23456789".contains($0) }) else {
            codeError = true
            return
        }
        codeError = false
        code = normalized
        if !isTrustedServer(server, confirmed: confirmed) {
            page = .relayConfirm
            return
        }
        join()
    }

    /// `relay.confirm.use`: remember the server as confirmed and go on.
    func useRelay() {
        confirmed.append(server)
        var s = store?.read() ?? settings
        s.confirmedServers = confirmed
        store?.write(s)
        join()
    }

    /// `relay.confirm.cancel`: the default relay.
    func useDefaultRelay() {
        server = "relay.cometcast.de:443"
        join()
    }

    private func join() {
        store?.writeName(name)
        connecting = true
        let cfg: [String: Any] = [
            "code": code,
            "server": server,
            "name": name,
            "log_dir": store.map { _ in logDirectory() } ?? "",
        ]
        guard let json = try? JSONSerialization.data(withJSONObject: cfg),
              let text = String(data: json, encoding: .utf8)
        else { return fail(.other) }
        let r = text.withCString { cc_ios_share_start($0) }
        if r != 0 {
            connecting = false
            return fail(r == 3 ? .code : .other)
        }
        page = .share
        startPolling()
    }

    private func logDirectory() -> String {
        guard let c = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: BroadcastIdentity.appGroup)
        else { return "" }
        return c.appendingPathComponent("share").path
    }

    private func presentItem() {
        guard let item, !shown else { return }
        shown = true
        let payload: [String: String]
        switch item.kind {
        case .link: payload = ["link": item.link ?? ""]
        case .image, .pdf: payload = ["file": item.fileURL?.path ?? ""]
        }
        if let d = try? JSONSerialization.data(withJSONObject: payload),
           let t = String(data: d, encoding: .utf8) {
            _ = t.withCString { cc_ios_share_present($0) }
        }
    }

    // MARK: remote

    func previousPage() { sendCmd("prev") }
    func nextPage() { sendCmd("next") }

    private func sendCmd(_ c: String) {
        _ = "{\"cmd\":\"\(c)\"}".withCString { cc_ios_share_cmd($0) }
    }

    /// `share.stopClose`, Cancel while waiting, and a sheet that was swiped away: stop and leave.
    func stopAndClose() {
        timer?.invalidate()
        timer = nil
        cc_ios_share_stop()
        removeItemCopy()
        close?()
    }

    private func removeItemCopy() {
        if let u = item?.fileURL { try? FileManager.default.removeItem(at: u) }
    }

    // MARK: polling

    private func startPolling() {
        timer?.invalidate()
        timer = Timer.scheduledTimer(withTimeInterval: 0.2, repeats: true) { [weak self] _ in
            Task { @MainActor in self?.drain() }
        }
    }

    private func drain() {
        var buf = [UInt8](repeating: 0, count: 4096)
        while true {
            var n = cc_ios_share_poll(&buf, buf.count)
            if n < 0 {
                buf = [UInt8](repeating: 0, count: Int(-n))
                n = cc_ios_share_poll(&buf, buf.count)
            }
            if n <= 0 { return }
            let line = String(decoding: buf[0..<Int(n)], as: UTF8.self)
            handle(ShareEvent.parse(line))
        }
    }

    func handle(_ e: ShareEvent) {
        switch e {
        case let .waiting(check, timeout, room):
            page = .waiting
            self.check = check
            timeoutSecs = timeout
            roomName = room ?? roomName
        case let .joined(room):
            roomName = room ?? roomName
            connecting = false
            page = .presenting
            presentItem()
        case let .connectionCode(s): sas = s
        case let .page(p, of: n): pageNumber = p; pageCount = n
        case let .progress(sent, total): progress = total > 0 ? Double(sent) / Double(total) : 0
        case let .contentRejected(reason):
            refusal = ShareModel.rejectKey(reason)
            shown = false
            page = .share
            cc_ios_share_stop()
        case .contentPrepared, .contentSent, .presenting, .unknown: break
        case .stopped: break
        case let .ended(code): fail(ShareErrorPage.forCode(code))
        case let .error(code): fail(ShareErrorPage.forCode(code))
        }
    }

    private func fail(_ p: ShareErrorPage) {
        timer?.invalidate()
        timer = nil
        connecting = false
        shown = false
        cc_ios_share_stop()
        page = .error(p)
    }

    /// Back from an error page: the share page with the item kept.
    func backToShare() { page = .share }

    /// The `sendReject*` key of a refusal reason of the core.
    static func rejectKey(_ reason: String) -> String {
        switch reason {
        case "not_a_regular_file": return "sendRejectNotFile"
        case "empty": return "sendRejectEmpty"
        case "too_large": return "sendRejectTooLarge"
        case "unsupported_type": return "sendRejectType"
        case "not_pdf": return "sendRejectNotPdf"
        case "bad_link": return "shareNoLink"
        default: return "sendRejectUnreadable"
        }
    }
}
