// SPDX-License-Identifier: GPL-3.0-or-later
// A stand-in for the Rust core in widget tests (no native library needed).

import 'dart:async';
import 'dart:typed_data';

import 'package:cometcast/core/core_api.dart';

class FakeCoreApi implements CoreApi {
  final receiverEvents = StreamController<ReceiverUiEvent>.broadcast();
  final senderEvents = StreamController<SenderUiEvent>.broadcast();

  String? receiverServer;
  String? senderCode;
  String? senderServer;
  int? senderSeconds;
  int stopReceiverCalls = 0;
  int stopSenderCalls = 0;
  ScreenRequest? senderScreen;

  // The encoded (Android capture) path.
  String? encodedCode;
  String? encodedServer;
  int startEncodedCalls = 0;
  final pushedFrames = <({Uint8List data, bool isKey, int ptsUs})>[];
  final pushedConfigs = <Uint8List>[];

  /// What pushCodecConfig answers (false = "no encoded sender yet").
  bool acceptConfig = true;
  PushResult pushResult = PushResult.queued;

  /// Answer of the next keyframeRequested call (cleared by it, like the real flag).
  bool keyframeRequest = false;

  /// Pictures waiting for takeFrame (the newest is handed out, the others are dropped, like
  /// the real mailbox).
  final frames = <VideoFrame>[];
  int takeFrameCalls = 0;

  /// If set, the next takeFrame picks its picture at once but answers only after this.
  Completer<void>? takeFrameGate;

  /// Number of upcoming takeFrame calls that throw.
  int takeFrameFailures = 0;
  final maxSizes = <(int, int)>[];

  /// What captureInfo answers (default: X11 works, whole desktop plus one monitor).
  CaptureInfo captureInfoResult = const CaptureInfo(
    support: CaptureSupport.supported(),
    display: ':0',
    monitors: [
      CaptureMonitor(
        id: 'root',
        name: 'Whole desktop',
        x: 0,
        y: 0,
        width: 640,
        height: 480,
      ),
      CaptureMonitor(
        id: 'out:HDMI-1',
        name: 'HDMI-1',
        x: 0,
        y: 0,
        width: 640,
        height: 480,
      ),
    ],
  );
  String? captureInfoDisplay;

  /// The H.264 codec (default: built in, nothing to do).
  UiCodecState codecStateResult = const UiCodecState.ready();
  UiCodecInfo? codecInfoResult;
  int ensureCodecCalls = 0;

  /// If set, ensureCodec answers only when this completes (then with codecStateAfter).
  Completer<void>? ensureCodecGate;
  UiCodecState codecStateAfter = const UiCodecState.ready();
  String? codecDir;

  @override
  UiCodecState codecState() => codecStateResult;

  @override
  UiCodecInfo? codecInfo() => codecInfoResult;

  @override
  Future<UiCodecState> ensureCodec() async {
    ensureCodecCalls++;
    await ensureCodecGate?.future;
    codecStateResult = codecStateAfter;
    return codecStateAfter;
  }

  @override
  void setCodecDir(String? path) => codecDir = path;

  /// Whether the last startReceiver asked for content.
  bool? receiverContent;

  @override
  Stream<ReceiverUiEvent> startReceiver({
    required String server,
    String? udpBind,
    bool content = true,
  }) {
    receiverServer = server;
    receiverContent = content;
    return receiverEvents.stream;
  }

  // --- received content -----------------------------------------------------------------

  /// Pictures waiting for takeContentFrame (the newest is handed out, like the real mailbox).
  final contentFrames = <ContentFrame>[];
  int takeContentFrameCalls = 0;
  final contentWidths = <int>[];

  @override
  Future<ContentFrame?> takeContentFrame() async {
    takeContentFrameCalls++;
    final newest = contentFrames.isEmpty ? null : contentFrames.last;
    contentFrames.clear();
    return newest;
  }

  @override
  void setContentTargetWidth(int width) => contentWidths.add(width);

  /// The real rules live in Rust (cc-files); this stand-in accepts absolute http(s) URLs
  /// with a host and no credentials. test/rust_core_test.dart checks the real thing.
  final linkChecks = <String>[];

  @override
  LinkCheck validateLink(String input) {
    linkChecks.add(input);
    final u = Uri.tryParse(input.trim());
    if (u == null ||
        RegExp(r'\s').hasMatch(input.trim()) ||
        (u.scheme != 'http' && u.scheme != 'https') ||
        u.host.isEmpty ||
        u.userInfo.isNotEmpty) {
      return const LinkCheck.invalid();
    }
    return LinkCheck.valid(url: u.toString(), host: u.host);
  }

  String? docsWorker;

  @override
  void setDocsWorker(String? path) => docsWorker = path;

  @override
  String? docsWorkerPath() => docsWorker;

  String? officeWorker;

  @override
  void setOfficeWorker(String? path) => officeWorker = path;

  @override
  String? officeWorkerPath() => officeWorker;

  // --- content sender -------------------------------------------------------------------

  final contentSenderEvents =
      StreamController<ContentSenderUiEvent>.broadcast();
  String? contentCode;
  String? contentServer;
  ContentSource? contentSource;
  final contentCommands = <UiContentCmd>[];

  /// What sendContentCmd answers.
  bool acceptCommands = true;

  @override
  Stream<ContentSenderUiEvent> startContentSender({
    required String code,
    required String server,
    String? udpBind,
    required ContentSource source,
  }) {
    contentCode = code;
    contentServer = server;
    contentSource = source;
    return contentSenderEvents.stream;
  }

  @override
  bool sendContentCmd(UiContentCmd cmd) {
    if (acceptCommands) contentCommands.add(cmd);
    return acceptCommands;
  }

  @override
  Future<void> stopReceiver() async => stopReceiverCalls++;

  @override
  Future<VideoFrame?> takeFrame() async {
    takeFrameCalls++;
    if (takeFrameFailures > 0) {
      takeFrameFailures--;
      throw StateError('takeFrame failed');
    }
    final newest = frames.isEmpty ? null : frames.last;
    frames.clear();
    final gate = takeFrameGate;
    takeFrameGate = null;
    if (gate != null) await gate.future;
    return newest;
  }

  @override
  void setVideoMaxSize(int width, int height) => maxSizes.add((width, height));

  @override
  Future<CaptureInfo> captureInfo({String? display}) async {
    captureInfoDisplay = display;
    return captureInfoResult;
  }

  /// What serverDefaults answers (the offline answer of the core). Default: no policy,
  /// the built-in list; the real values are checked in rust_core_test.dart.
  ServerDefaults defaults = const ServerDefaults(
    servers: [
      ServerCandidate(
        address: 'relay.cometcast.de:443',
        source: 'default-list',
      ),
    ],
    locked: false,
    announcementsAllowed: true,
    notices: [],
    devFallback: '127.0.0.1:8443',
  );

  @override
  ServerDefaults serverDefaults() => defaults;

  /// What discoverServers answers; null = the same as [defaults]. Set [discoveryGate] to keep
  /// the search "running" until the test completes it.
  ServerDiscovery? discovery;
  Completer<void>? discoveryGate;
  bool discoveryThrows = false;
  int discoverCalls = 0;

  @override
  Future<ServerDiscovery> discoverServers() async {
    discoverCalls++;
    final gate = discoveryGate;
    if (gate != null) await gate.future;
    if (discoveryThrows) throw StateError('discovery failed');
    return discovery ??
        ServerDiscovery(
          servers: defaults.servers,
          locked: defaults.locked,
          announcementsAllowed: defaults.announcementsAllowed,
          notices: defaults.notices,
          warnings: const [],
        );
  }

  // Announcing the receiver.
  final advertiseCalls = <({String name, String? server})>[];
  int stopAdvertisingCalls = 0;

  /// Answers of the next startAdvertising calls (the last one repeats); default: started.
  List<AdvertiseOutcome> advertiseOutcomes = [AdvertiseOutcome.started];

  @override
  Future<AdvertiseOutcome> startAdvertising({
    required String name,
    String? preferredServer,
  }) async {
    advertiseCalls.add((name: name, server: preferredServer));
    final i = advertiseCalls.length - 1;
    return advertiseOutcomes[i < advertiseOutcomes.length
        ? i
        : advertiseOutcomes.length - 1];
  }

  @override
  Future<void> stopAdvertising() async => stopAdvertisingCalls++;

  // Receivers nearby.
  StreamController<NearbyReceiverEvent>? nearbyEvents;
  int watchCalls = 0;
  int stopWatchingCalls = 0;
  bool nearbyCancelled = false;

  @override
  Stream<NearbyReceiverEvent> watchReceivers() {
    watchCalls++;
    final c = nearbyEvents = StreamController<NearbyReceiverEvent>(
      onCancel: () => nearbyCancelled = true,
    );
    return c.stream;
  }

  @override
  Future<void> stopWatchingReceivers() async => stopWatchingCalls++;

  @override
  Stream<SenderUiEvent> startSender({
    required String code,
    required String server,
    int? seconds,
    String? udpBind,
    ScreenRequest? screen,
  }) {
    senderScreen = screen;
    senderCode = code;
    senderServer = server;
    senderSeconds = seconds;
    return senderEvents.stream;
  }

  @override
  Stream<SenderUiEvent> startEncodedSender({
    required String code,
    required String server,
    String? udpBind,
  }) {
    startEncodedCalls++;
    encodedCode = code;
    encodedServer = server;
    return senderEvents.stream;
  }

  @override
  PushResult pushEncodedFrame(
    Uint8List data, {
    required bool isKey,
    required int ptsUs,
  }) {
    pushedFrames.add((data: data, isKey: isKey, ptsUs: ptsUs));
    return pushResult;
  }

  @override
  bool pushCodecConfig(Uint8List data) {
    pushedConfigs.add(data);
    return acceptConfig;
  }

  @override
  bool keyframeRequested() {
    final r = keyframeRequest;
    keyframeRequest = false;
    return r;
  }

  @override
  Future<void> stopSender() async => stopSenderCalls++;

  // The real rules live in Rust (cc-proto). This copy only exists for tests without the
  // native library; rust_core_test.dart checks that both agree.
  static const _alphabet = 'ABCDEFGHJKLMNPQRSTUVWXYZ23456789';

  @override
  String get roomCodeAlphabet => _alphabet;

  @override
  int get roomCodeLength => 6;

  @override
  RoomCodeCheck checkRoomCode(String input) {
    // ASCII-only uppercase, like Rust's to_ascii_uppercase ('ſ' must stay invalid).
    final upper = input.trim().replaceAllMapped(
      RegExp('[a-z]'),
      (m) => m[0]!.toUpperCase(),
    );
    for (final c in upper.split('')) {
      if (!_alphabet.contains(c)) return RoomCodeCheck.invalidChar(ch: c);
    }
    if (upper.length != roomCodeLength) {
      return const RoomCodeCheck.wrongLength();
    }
    return RoomCodeCheck.valid(code: upper);
  }
}
