// SPDX-License-Identifier: GPL-3.0-or-later
// The principal class of the share extension (native-apps.md 21.3, decision N82): it takes the
// one shared item, copies it into the App Group container, hosts the SwiftUI pages and stops and
// leaves when the sheet closes (also when it is swiped away). No Flutter engine.
//
// UNCERTAIN: never compiled or run (no Xcode here, docs/ios.md).

import SwiftUI
import UIKit
import UniformTypeIdentifiers

final class ShareViewController: UIViewController {
    private let model = ShareModel()
    private var finished = false

    override func viewDidLoad() {
        super.viewDidLoad()
        model.close = { [weak self] in self?.finish() }
        let host = UIHostingController(rootView: ShareRootView(model: model))
        addChild(host)
        host.view.frame = view.bounds
        host.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        view.addSubview(host.view)
        host.didMove(toParent: self)
        loadItem()
    }

    /// Swiping the sheet away is the same as "Stop and close".
    override func viewDidDisappear(_ animated: Bool) {
        super.viewDidDisappear(animated)
        if !finished {
            finished = true
            cc_ios_share_stop()
        }
    }

    private func finish() {
        finished = true
        extensionContext?.completeRequest(returningItems: nil)
    }

    // MARK: the item

    private func loadItem() {
        let providers = (extensionContext?.inputItems as? [NSExtensionItem])?
            .flatMap { $0.attachments ?? [] } ?? []
        guard providers.count == 1, let p = providers.first else {
            model.setItem(nil)
            return
        }
        if p.hasItemConformingToTypeIdentifier(UTType.pdf.identifier) {
            copyFile(p, type: UTType.pdf, kind: .pdf, limit: 64 * 1024 * 1024)
        } else if p.hasItemConformingToTypeIdentifier(UTType.image.identifier) {
            copyFile(p, type: UTType.image, kind: .image, limit: 10 * 1024 * 1024)
        } else if p.hasItemConformingToTypeIdentifier(UTType.url.identifier) {
            p.loadItem(forTypeIdentifier: UTType.url.identifier, options: nil) { [weak self] value, _ in
                let url = value as? URL
                DispatchQueue.main.async {
                    guard let url, let scheme = url.scheme?.lowercased(), scheme == "http" || scheme == "https" else {
                        self?.model.setItem(nil)
                        return
                    }
                    self?.model.setItem(SharedItem(kind: .link, fileURL: nil, title: url.absoluteString, size: 0, link: url.absoluteString))
                }
            }
        } else {
            model.setItem(nil)
        }
    }

    /// Copies the file into `<container>/share/items/<random>/<name>`, refusing beyond the limit
    /// (the "too large" text is the existing one; nothing is left on the disk).
    private func copyFile(_ p: NSItemProvider, type: UTType, kind: SharedItem.Kind, limit: Int) {
        p.loadFileRepresentation(forTypeIdentifier: type.identifier) { [weak self] url, _ in
            guard let self else { return }
            var item: SharedItem?
            var refusal: String?
            if let url,
               let c = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: BroadcastIdentity.appGroup) {
                let size = (try? url.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0
                if size > limit {
                    item = SharedItem(kind: kind, fileURL: nil, title: url.lastPathComponent, size: size, link: nil)
                    refusal = "sendRejectTooLarge"
                } else {
                    let dir = c.appendingPathComponent("share/items/\(UUID().uuidString)", isDirectory: true)
                    let target = dir.appendingPathComponent(url.lastPathComponent)
                    do {
                        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
                        try FileManager.default.copyItem(at: url, to: target)
                        item = SharedItem(kind: kind, fileURL: target, title: url.lastPathComponent, size: size, link: nil)
                    } catch {
                        item = SharedItem(kind: kind, fileURL: nil, title: url.lastPathComponent, size: size, link: nil)
                        refusal = "sendRejectUnreadable"
                    }
                }
            }
            DispatchQueue.main.async { self.model.setItem(item, refusal: refusal) }
        }
    }
}
