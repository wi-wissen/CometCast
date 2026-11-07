// SPDX-License-Identifier: GPL-3.0-or-later
// The system QR scanner for Dart (native-apps.md section 8), MethodChannel `de.cometcast/qr`:
//   isAvailable -> Bool     iOS 16 or newer, a scanner that works (VisionKit) and camera access
//                           not refused
//   scan        -> String?  VisionKit `DataScannerViewController`, QR codes only; the text of the
//                           first code, or nil when the person cancelled or the scanner failed
//
// The camera permission text is `NSCameraUsageDescription` (InfoPlist.strings, the copy of
// spec 11: `ios.cameraUsage`). Dart checks the text (`parseLinkUrl`) and shows `app.scanWrong`
// for anything that is not a link of ours. The Cancel button is the system's (`.cancel`), so
// the code carries no text of its own.
//
// UNCERTAIN: never compiled or run on a device. The deployment target is 15.0; the scanner
// needs 16.0 and `isAvailable` says false before that.

import AVFoundation
import Flutter
import UIKit
import VisionKit

final class QrBridge: NSObject {
    private let channel: FlutterMethodChannel
    private var pending: FlutterResult?
    private var scannerHolder: AnyObject?

    init(messenger: FlutterBinaryMessenger) {
        channel = FlutterMethodChannel(name: "de.cometcast/qr", binaryMessenger: messenger)
        super.init()
        channel.setMethodCallHandler { [weak self] call, result in
            self?.handle(call, result: result)
        }
    }

    private func handle(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
        switch call.method {
        case "isAvailable":
            result(QrBridge.supported())
        case "scan":
            scan(result: result)
        default:
            result(FlutterMethodNotImplemented)
        }
    }

    static func supported() -> Bool {
        guard #available(iOS 16.0, *) else { return false }
        guard AVCaptureDevice.authorizationStatus(for: .video) != .denied,
            AVCaptureDevice.authorizationStatus(for: .video) != .restricted
        else { return false }
        return DataScannerViewController.isSupported
    }

    private func finish(_ value: String?) {
        guard let result = pending else { return }
        pending = nil
        scannerHolder = nil
        result(value)
    }

    private func scan(result: @escaping FlutterResult) {
        guard #available(iOS 16.0, *), QrBridge.supported(), pending == nil,
            let presenter = QrBridge.topViewController()
        else {
            result(nil)
            return
        }
        pending = result
        let scanner = DataScannerViewController(
            recognizedDataTypes: [.barcode(symbologies: [.qr])],
            qualityLevel: .balanced,
            recognizesMultipleItems: false,
            isHighFrameRateTrackingEnabled: false,
            isPinchToZoomEnabled: true,
            isGuidanceEnabled: true,
            isHighlightingEnabled: true)
        let delegate = ScannerDelegate { [weak self] text in
            presenter.dismiss(animated: true) { self?.finish(text) }
        }
        scanner.delegate = delegate
        scannerHolder = delegate
        let cancel = UIBarButtonItem(
            systemItem: .cancel,
            primaryAction: UIAction { _ in delegate.finish(nil) })
        scanner.navigationItem.leftBarButtonItem = cancel
        let nav = UINavigationController(rootViewController: scanner)
        presenter.present(nav, animated: true) {
            do {
                try scanner.startScanning()
            } catch {
                delegate.finish(nil)
            }
        }
    }

    private static func topViewController() -> UIViewController? {
        let scene = UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .first { $0.activationState == .foregroundActive }
        var top = scene?.windows.first { $0.isKeyWindow }?.rootViewController
        while let next = top?.presentedViewController { top = next }
        return top
    }
}

@available(iOS 16.0, *)
private final class ScannerDelegate: NSObject, DataScannerViewControllerDelegate {
    private var done: ((String?) -> Void)?

    init(done: @escaping (String?) -> Void) {
        self.done = done
    }

    func finish(_ text: String?) {
        guard let d = done else { return }
        done = nil
        d(text)
    }

    func dataScanner(
        _ dataScanner: DataScannerViewController, didAdd addedItems: [RecognizedItem],
        allItems: [RecognizedItem]
    ) {
        for item in addedItems {
            if case .barcode(let code) = item, let text = code.payloadStringValue {
                finish(text)
                return
            }
        }
    }

    func dataScanner(
        _ dataScanner: DataScannerViewController, becameUnavailableWithError error: DataScannerViewController.ScanningUnavailable
    ) {
        finish(nil)
    }
}
