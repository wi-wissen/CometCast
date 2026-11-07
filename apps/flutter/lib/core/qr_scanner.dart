// SPDX-License-Identifier: GPL-3.0-or-later
// The system QR scanner (native-apps.md section 8, spec 5.3): Android Google code scanner
// (`GmsBarcodeScanning`, no camera permission), iOS and iPadOS VisionKit
// `DataScannerViewController` (camera permission with the text `ios.cameraUsage`). Computers
// have none: there is no scan button there. The platform code is
// android/.../scanner/QrBridge.kt and ios/Runner/Scanner/QrBridge.swift.

import 'package:flutter/services.dart';

abstract class QrScanner {
  /// Whether this device can scan (a system scanner exists and works): the first-run page shows
  /// the scan button and "Scan QR code again" on the error pages only then.
  Future<bool> isAvailable();

  /// Open the system scanner and return the text of the first QR code, or null when the person
  /// cancelled (nothing changes) or the scanner could not run.
  Future<String?> scan();
}

class ChannelQrScanner implements QrScanner {
  const ChannelQrScanner([MethodChannel? channel])
    : _channel = channel ?? const MethodChannel('de.cometcast/qr');

  final MethodChannel _channel;

  @override
  Future<bool> isAvailable() async {
    try {
      return await _channel.invokeMethod<bool>('isAvailable') ?? false;
    } on PlatformException {
      return false;
    } on MissingPluginException {
      return false;
    }
  }

  @override
  Future<String?> scan() async {
    try {
      final v = await _channel.invokeMethod<String>('scan');
      return (v == null || v.isEmpty) ? null : v;
    } on PlatformException {
      return null;
    } on MissingPluginException {
      return null;
    }
  }
}
