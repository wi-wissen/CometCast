// SPDX-License-Identifier: GPL-3.0-or-later
// The Dart side of the iOS screen broadcast (Swift: ios/Runner/Broadcast/BroadcastBridge.swift).
//
// On iOS the screen is not captured by the app but by a ReplayKit Broadcast Upload Extension that
// makes the whole network connection itself (docs/ios.md). The app only
//   1. hands over the room code and the server through the App Group ([IosBroadcast.prepare]),
//   2. opens the system broadcast picker ([IosBroadcast.showPicker]),
//   3. shows the status that the extension writes back ([IosBroadcast.events]),
//   4. can ask the extension to stop ([IosBroadcast.stop]).
// Nothing is pushed through Dart and the Rust sender of the app is not used.
//
//   MethodChannel 'de.cometcast/ios_broadcast'         prepare / showPicker / stop / status / isCaptured / info
//   EventChannel  'de.cometcast/ios_broadcast/status'  maps, see [parseBroadcastStatus]
//
// [IosBroadcast] is an interface so that widget tests can plug in a fake.
//
// UNCERTAIN: never run on a device (docs/ios.md).

import 'dart:async';

import 'package:flutter/services.dart';

/// Id of the single entry that `captureInfo` lists on iOS (`IOS_BROADCAST_ID` in
/// crates/cc-ffi/src/api.rs; a Rust test pins the value).
const iosBroadcastMonitorId = 'ios-broadcast';

/// Where the extension says the broadcast is. The names are the raw values of
/// `BroadcastStateName` in ios/Shared/BroadcastShared.swift, plus [lost] (made by the app when
/// the extension stopped writing: it was killed, for example by the memory limit).
enum IosBroadcastState {
  starting,
  connecting,
  waiting,
  streaming,
  reconnecting,
  paused,
  ended,
  failed,
  lost,
}

/// Why it ended or failed. Raw values of `BroadcastErrorName` in the Swift code; each has a
/// translated sentence in the ARB files (`iosError*`).
enum IosBroadcastError {
  none,
  noConfig,
  configExpired,
  invalidRoomCode,
  invalidRelay,
  roomNotFound,
  serverUnreachable,
  connectionLost,
  peerLeft,
  encoderFailed,
  memory,
  stoppedByApp,
  internalError,
}

/// One report of the extension.
class IosBroadcastStatus {
  const IosBroadcastStatus({
    required this.state,
    this.error = IosBroadcastError.none,
    this.sent = 0,
    this.dropped = 0,
    this.bytes = 0,
    this.keyframes = 0,
    this.width = 0,
    this.height = 0,
    this.freeMemoryMB = -1,
    this.seq = 0,
    this.updatedAtMs,
  });

  final IosBroadcastState state;
  final IosBroadcastError error;

  /// Access units sent, dropped, bytes sent, key frames sent.
  final int sent;
  final int dropped;
  final int bytes;
  final int keyframes;

  /// Size of the encoded picture; 0 until known.
  final int width;
  final int height;

  /// What iOS says the extension may still allocate, in MB (-1 = unknown). For the memory
  /// soak test in docs/ios-device-test-plan.md.
  final int freeMemoryMB;
  final int seq;

  /// When the extension wrote this report (Unix time in milliseconds), if it said. The app
  /// ignores reports older than its own start request: they belong to an earlier broadcast.
  final int? updatedAtMs;

  bool get isTerminal =>
      state == IosBroadcastState.ended ||
      state == IosBroadcastState.failed ||
      state == IosBroadcastState.lost;

  @override
  String toString() => 'IosBroadcastStatus($state, $error, sent $sent)';
}

/// What `prepare` needs.
class IosBroadcastRequest {
  const IosBroadcastRequest({
    required this.code,
    required this.relay,
    this.maxDimension = 1280,
    this.fps = 30,
    this.bitrate = 2500000,
  });

  final String code;
  final String relay;
  final int maxDimension;
  final int fps;
  final int bitrate;
}

abstract class IosBroadcast {
  /// Status reports of the extension (and [IosBroadcastState.lost] when it goes silent).
  Stream<IosBroadcastStatus> get events;

  /// Write the room code and server into the App Group so that the extension can read them
  /// when the person starts the broadcast. Throws [PlatformException] if the App Group is not
  /// available (a signing problem).
  Future<void> prepare(IosBroadcastRequest request);

  /// Open the system "Screen Broadcast" sheet with Comet Cast preselected. Returns false if
  /// the sheet could not be opened from the app (then the person starts the broadcast from
  /// Control Center; the hand-over is the same).
  Future<bool> showPicker();

  /// Ask the extension to end the broadcast and remove the hand-over files. Safe when nothing
  /// runs.
  Future<void> stop();

  /// The last status the extension wrote, if any.
  Future<IosBroadcastStatus?> currentStatus();
}

/// The real implementation, on top of the platform channels.
class ChannelIosBroadcast implements IosBroadcast {
  const ChannelIosBroadcast();

  static const _method = MethodChannel('de.cometcast/ios_broadcast');
  static const _events = EventChannel('de.cometcast/ios_broadcast/status');

  @override
  Stream<IosBroadcastStatus> get events => _events
      .receiveBroadcastStream()
      .map(parseBroadcastStatus)
      .where((e) => e != null)
      .cast<IosBroadcastStatus>();

  @override
  Future<void> prepare(IosBroadcastRequest r) async {
    await _method.invokeMethod<bool>('prepare', {
      'code': r.code,
      'relay': r.relay,
      'maxDimension': r.maxDimension,
      'fps': r.fps,
      'bitrate': r.bitrate,
    });
  }

  @override
  Future<bool> showPicker() async =>
      await _method.invokeMethod<bool>('showPicker') ?? false;

  @override
  Future<void> stop() => _method.invokeMethod<void>('stop');

  @override
  Future<IosBroadcastStatus?> currentStatus() async =>
      parseBroadcastStatus(await _method.invokeMethod<Object?>('status'));
}

/// Turns a map from the platform into a status; null for something unknown or malformed
/// (ignored, not fatal). Unknown state or error names are not guessed.
IosBroadcastStatus? parseBroadcastStatus(dynamic raw) {
  if (raw is! Map) return null;
  final state = _byName(IosBroadcastState.values, raw['state']);
  if (state == null) return null;
  return IosBroadcastStatus(
    state: state,
    error:
        _byName(IosBroadcastError.values, raw['error']) ??
        (state == IosBroadcastState.failed
            ? IosBroadcastError.internalError
            : IosBroadcastError.none),
    sent: _int(raw['sent']),
    dropped: _int(raw['dropped']),
    bytes: _int(raw['bytes']),
    keyframes: _int(raw['keyframes']),
    width: _int(raw['width']),
    height: _int(raw['height']),
    freeMemoryMB: raw['freeMemoryMB'] is num ? _int(raw['freeMemoryMB']) : -1,
    seq: _int(raw['seq']),
    updatedAtMs: raw['updatedAt'] is num
        ? ((raw['updatedAt'] as num) * 1000).round()
        : null,
  );
}

T? _byName<T extends Enum>(List<T> values, dynamic name) {
  if (name is! String) return null;
  for (final v in values) {
    if (v.name == name) return v;
  }
  return null;
}

int _int(dynamic v) => v is num ? v.toInt() : 0;
