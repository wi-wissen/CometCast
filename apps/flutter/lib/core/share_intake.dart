// SPDX-License-Identifier: GPL-3.0-or-later
// The item of the Android share sheet (native-apps.md 21.2, spec 15.3), from the platform
// channel `cometcast/share` (android/.../share/ShareBridge.kt). No widgets and no strings: the
// share page (fix list F7) decides what to show for each kind.

import 'dart:async';

import 'package:flutter/services.dart';

/// What the share sheet handed over.
enum SharedKind {
  /// A file (image, PDF, Office), already copied into the app's cache folder.
  file,

  /// A web address found in the shared text.
  link,

  /// Text without a web address, or a type that is not registered (page text `share.noLink`).
  noLink,

  /// Larger than the limit of its kind (10 MiB image, 64 MiB PDF, 50 MiB Office); nothing was
  /// copied.
  tooLarge,
}

/// One shared item.
class SharedItem {
  const SharedItem({
    required this.kind,
    this.path,
    this.name,
    this.size,
    this.mime,
    this.url,
  });

  final SharedKind kind;

  /// The copy in the cache folder (`file` only).
  final String? path;

  /// The file's display name (`file` only).
  final String? name;

  /// Size in bytes (`file` only).
  final int? size;

  final String? mime;

  /// The address as found; the core's `validate_link` has the last word (`link` only).
  final String? url;

  /// Parses what the channel returns; null for null or a map that is not an item.
  static SharedItem? fromMap(Object? m) {
    if (m is! Map) return null;
    final kind = switch (m['kind']) {
      'file' => SharedKind.file,
      'link' => SharedKind.link,
      'noLink' => SharedKind.noLink,
      'tooLarge' => SharedKind.tooLarge,
      _ => null,
    };
    if (kind == null) return null;
    final path = m['path'];
    if (kind == SharedKind.file && (path is! String || path.isEmpty)) {
      return null;
    }
    final size = m['size'];
    return SharedItem(
      kind: kind,
      path: path is String ? path : null,
      name: m['name'] is String ? m['name'] as String : null,
      size: size is int ? size : null,
      mime: m['mime'] is String ? m['mime'] as String : null,
      url: m['url'] is String ? m['url'] as String : null,
    );
  }

  @override
  String toString() => 'SharedItem($kind, $name$url)';
}

abstract class ShareIntake {
  /// The item that opened or re-opened the app, once; null if there is none (the normal
  /// launch, or any platform other than Android).
  Future<SharedItem?> take();

  /// Fires when a new item arrives while the app runs; then call [take].
  Stream<void> get arrivals;

  /// Deletes the cache copy of [path] (when the presentation of the item ends).
  Future<void> discard(String path);
}

/// The platform channel implementation. A missing plugin (every platform but Android) is not
/// an error: there is no shared item then.
class ChannelShareIntake implements ShareIntake {
  ChannelShareIntake([MethodChannel? channel])
    : _channel = channel ?? const MethodChannel('cometcast/share') {
    _channel.setMethodCallHandler((call) async {
      if (call.method == 'shared' && !_controller.isClosed) {
        _controller.add(null);
      }
      return null;
    });
  }

  final MethodChannel _channel;
  final _controller = StreamController<void>.broadcast();

  @override
  Stream<void> get arrivals => _controller.stream;

  @override
  Future<SharedItem?> take() async {
    try {
      return SharedItem.fromMap(
        await _channel.invokeMethod<Object?>('takeSharedItem'),
      );
    } on PlatformException {
      return null;
    } on MissingPluginException {
      return null;
    }
  }

  @override
  Future<void> discard(String path) async {
    try {
      await _channel.invokeMethod<void>('discardSharedItem', path);
    } on PlatformException {
      // Nothing to do: the folder is emptied when the app starts.
    } on MissingPluginException {
      // Not Android.
    }
  }
}

/// No share intake (tests, other platforms).
class NoShareIntake implements ShareIntake {
  const NoShareIntake();

  @override
  Stream<void> get arrivals => const Stream.empty();

  @override
  Future<void> discard(String path) async {}

  @override
  Future<SharedItem?> take() async => null;
}
