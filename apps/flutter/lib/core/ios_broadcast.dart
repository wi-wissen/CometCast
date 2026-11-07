// SPDX-License-Identifier: GPL-3.0-or-later
// The Dart side of the iOS screen broadcast (Swift: ios/Runner/Broadcast/BroadcastBridge.swift).
//
// On iOS the screen is not captured by the app but by a ReplayKit Broadcast Upload Extension that
// makes the whole network connection itself (docs/ios.md). The app only
//   1. hands over the room code, the server, the person's name and the single-use hand-over
//      token of the display through the App Group ([IosBroadcast.prepare]; native-apps.md 20.5),
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
import 'dart:math';

import 'package:flutter/foundation.dart' show kReleaseMode;
import 'package:flutter/services.dart';

/// Test builds only (native-apps.md 20.5, step 7): option (a) of the F2 answer, the extension
/// joins on its own connection without the hand-over token (the host is asked as for any join;
/// the extension sends the name, the app shows the waiting page with the extension's check number
/// and joins again with the same code afterwards). Switched on with
/// `--dart-define=COMETCAST_IOS_OPTION_A=true`; **a release build ignores it** (see
/// [iosOptionAAllowed]), so the hand-over cannot be turned off in a shipped app.
const _iosOptionADefine = bool.fromEnvironment('COMETCAST_IOS_OPTION_A');

/// Whether option (a) is on: only when [defined] and the build is not a release build.
bool iosOptionAAllowed({required bool release, required bool defined}) =>
    defined && !release;

/// Option (a) for this build (never true in a release build).
bool get iosOptionA =>
    iosOptionAAllowed(release: kReleaseMode, defined: _iosOptionADefine);

/// Id of the single entry that `captureInfo` lists on iOS (`IOS_BROADCAST_ID` in
/// crates/cc-ffi/src/api.rs; a Rust test pins the value).
const iosBroadcastMonitorId = 'ios-broadcast';

/// Where the extension says the broadcast is. The names are the raw values of
/// `BroadcastStateName` in ios/Shared/BroadcastShared.swift, plus [lost] (made by the app when
/// the extension stopped writing: it was killed, for example by the memory limit).
enum IosBroadcastState {
  starting,
  connecting,

  /// The join waits for the host's decision: [IosBroadcastStatus.check] and
  /// [IosBroadcastStatus.timeoutSecs] say what the waiting page shows (20.5, step 6).
  pending,
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

  /// The host denied the join, nobody decided in time, the room is locked (20.5, step 6).
  joinDenied,
  approvalTimeout,
  roomLocked,
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
    this.check,
    this.timeoutSecs,
    this.roomName,
    this.sas,
    this.nextHandover,
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

  /// While [IosBroadcastState.pending]: the check number the display shows on its request, and
  /// the seconds the host has to decide.
  final int? check;
  final int? timeoutSecs;

  /// The display's room name and the connection check code (`123 456`) of the broadcast, for
  /// the Presenting page (`presenting.where`, `presenting.sas`).
  final String? roomName;
  final String? sas;

  /// The token of the way back: the display holds it, the app joins with it after the
  /// broadcast and needs no new approval (20.5, step 5). Null: join the ordinary way.
  final String? nextHandover;

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
    this.name,
    this.handover,
  });

  final String code;
  final String relay;

  /// The person's name for the display's request, and the single-use token the display holds
  /// for this person (null = an ordinary join, the display asks the host).
  final String? name;
  final String? handover;
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
      'name': r.name,
      'handover': r.handover,
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
    check: raw['check'] is num ? _int(raw['check']) : null,
    timeoutSecs: raw['timeoutSecs'] is num ? _int(raw['timeoutSecs']) : null,
    roomName: _text(raw['roomName']),
    sas: _text(raw['sas']),
    nextHandover: _text(raw['nextHandover']),
  );
}

String? _text(dynamic v) => v is String && v.isNotEmpty ? v : null;

/// A new single-use hand-over token: 128 random bits as base64url without padding (22
/// characters of `A-Z a-z 0-9 - _`), the form the display accepts (native-apps.md 20.5). Pass a
/// [Random] only in tests.
String newHandoverToken([Random? random]) {
  final r = random ?? Random.secure();
  const alphabet =
      'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_';
  final bytes = List<int>.generate(16, (_) => r.nextInt(256));
  final out = StringBuffer();
  var bits = 0;
  var acc = 0;
  for (final b in bytes) {
    acc = (acc << 8) | b;
    bits += 8;
    while (bits >= 6) {
      bits -= 6;
      out.write(alphabet[(acc >> bits) & 63]);
    }
    acc &= (1 << bits) - 1;
  }
  if (bits > 0) out.write(alphabet[(acc << (6 - bits)) & 63]);
  return out.toString();
}

T? _byName<T extends Enum>(List<T> values, dynamic name) {
  if (name is! String) return null;
  for (final v in values) {
    if (v.name == name) return v;
  }
  return null;
}

int _int(dynamic v) => v is num ? v.toInt() : 0;
