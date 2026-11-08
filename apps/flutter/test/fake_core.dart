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

  /// The room name of the last startReceiver.
  String? receiverRoomName;

  @override
  Stream<ReceiverUiEvent> startReceiver({
    required String server,
    String? udpBind,
    bool content = true,
    UiJoinMode joinMode = UiJoinMode.open,
    String? roomName,
  }) {
    receiverRoomName = roomName;
    receiverServer = server;
    receiverContent = content;
    receiverJoinMode = joinMode;
    return receiverEvents.stream;
  }

  // --- join approval ----------------------------------------------------------------------

  /// The join mode of the last startReceiver.
  UiJoinMode? receiverJoinMode;

  /// What the UI asked for (the fake does not answer with `joinModeChanged` by itself; tests
  /// emit it, like the server would).
  final joinModeRequests = <UiJoinMode>[];
  final joinDecisions = <({BigInt? id, bool allow})>[];
  int removePeerCalls = 0;
  final removePeerBlock = <bool>[];
  final blockedRequests = <BigInt>[];
  final unblocked = <BigInt>[];
  int endRoomCalls = 0;

  @override
  bool setJoinMode(UiJoinMode mode) {
    joinModeRequests.add(mode);
    return true;
  }

  @override
  bool decideJoin({BigInt? id, required bool allow}) {
    joinDecisions.add((id: id, allow: allow));
    return true;
  }

  @override
  bool removePeer({bool block = false}) {
    removePeerCalls++;
    removePeerBlock.add(block);
    return true;
  }

  @override
  bool denyAndBlockJoin(BigInt id) {
    blockedRequests.add(id);
    return true;
  }

  @override
  bool unblock(BigInt id) {
    unblocked.add(id);
    return true;
  }

  @override
  bool endRoom() {
    endRoomCalls++;
    return true;
  }

  /// The name given to the last sender start.
  String? senderName;

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
    String? name,
  }) {
    senderName = name;
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

  // --- presenter session (join once) ------------------------------------------------------

  final presenterEvents = StreamController<PresenterUiEvent>.broadcast();
  int startPresenterCalls = 0;
  String? presenterCode;
  String? presenterServer;
  String? presenterName;
  String? presenterHandover;
  final handoverRequests = <String>[];
  int stopPresenterCalls = 0;
  int stopPresentingCalls = 0;
  int presentEncodedCalls = 0;
  final presentedScreens = <ScreenRequest?>[];
  final presentedContent = <ContentSource>[];

  /// What the present* and stopPresenting calls answer (false = "no session").
  bool presenterAccepts = true;

  @override
  Stream<PresenterUiEvent> startPresenter({
    required String code,
    required String server,
    String? name,
    String? handover,
    String? udpBind,
  }) {
    startPresenterCalls++;
    presenterCode = code;
    presenterServer = server;
    presenterName = name;
    presenterHandover = handover;
    return presenterEvents.stream;
  }

  /// What `requestHandover` queues as the answer event (null = the display does not answer
  /// and no event comes; true/false = `HandoverResult`).
  bool? handoverAnswer = true;

  @override
  bool requestHandover(String token) {
    if (!presenterAccepts) return false;
    handoverRequests.add(token);
    final a = handoverAnswer;
    if (a != null) {
      scheduleMicrotask(
        () => presenterEvents.add(PresenterUiEvent.handoverResult(ok: a)),
      );
    }
    return true;
  }

  @override
  Future<void> stopPresenter() async => stopPresenterCalls++;

  @override
  bool presentScreen(ScreenRequest? screen) {
    if (presenterAccepts) presentedScreens.add(screen);
    return presenterAccepts;
  }

  @override
  bool presentEncoded() {
    if (presenterAccepts) presentEncodedCalls++;
    return presenterAccepts;
  }

  @override
  bool presentContent(ContentSource source) {
    if (presenterAccepts) presentedContent.add(source);
    return presenterAccepts;
  }

  @override
  bool stopPresenting() {
    if (presenterAccepts) stopPresentingCalls++;
    return presenterAccepts;
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

  /// What setVideoPassthrough was last called with.
  bool passthrough = false;
  int videoKeyframeRequests = 0;

  /// Units that the next takeEncodedVideo hands out (then cleared, like the real queue).
  final encodedUnits = <EncodedVideoUnit>[];

  @override
  void setVideoPassthrough(bool on) => passthrough = on;

  @override
  void requestVideoKeyframe() => videoKeyframeRequests++;

  @override
  Future<List<EncodedVideoUnit>> takeEncodedVideo() async {
    final out = List<EncodedVideoUnit>.of(encodedUnits);
    encodedUnits.clear();
    return out;
  }

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

  // The display announcement (spec 14.8).
  final announceCalls =
      <({bool active, String id, Uint8List key, int seqFloor})>[];
  AnnounceStatus announceStatus = AnnounceStatus.announcing;
  int seqToPersist = 1000;
  int _secrets = 0;

  /// Every secret made by [newDisplaySecret], in order.
  final madeSecrets = <Uint8List>[];

  @override
  Uint8List newDisplaySecret() {
    // Deterministic, different per call; the 32 bytes are all `n`.
    final s = Uint8List.fromList(List.filled(32, ++_secrets));
    madeSecrets.add(s);
    return s;
  }

  /// A fake identity: the id is the key bytes in hex (32 digits from the first 16 bytes).
  @override
  DisplayIdentity? displayIdentity(Uint8List secretKey) {
    if (secretKey.length != 32) return null;
    final id = secretKey
        .take(16)
        .map((b) => b.toRadixString(16).padLeft(2, '0'))
        .join();
    return DisplayIdentity(
      id: id,
      tag: id.substring(0, 4).toUpperCase(),
      publicKey: 'pk$id',
    );
  }

  @override
  int displaySeqToPersist() => seqToPersist;

  @override
  Future<AnnounceStatus> setDisplayAnnouncing({
    required bool active,
    required Uint8List secretKey,
    required int seqFloor,
  }) async {
    announceCalls.add((
      active: active,
      id: displayIdentity(secretKey)?.id ?? '',
      key: secretKey,
      seqFloor: seqFloor,
    ));
    return announceStatus;
  }

  /// What the app last passed to [setRememberedScreens].
  List<RememberedScreen> remembered = [];

  @override
  void setRememberedScreens(List<RememberedScreen> screens) =>
      remembered = List.of(screens);

  // Screens on this network (spec 14): one stream per search, driven by the test.
  final nearbySearches = <StreamController<NearbyUiEvent>>[];
  final nearbyNativeFlags = <bool>[];
  int nearbyStopCalls = 0;
  bool hasLocalNetwork = true;
  final nativeSeen = <({String instance, List<String> addresses})>[];
  final nativeGone = <String>[];
  int nativeResets = 0;
  final nativePermissions = <NearbyPermission>[];

  /// The controller of the newest search.
  StreamController<NearbyUiEvent> get search => nearbySearches.last;

  // --- round 5 ---

  String version = '0.1.0';

  @override
  String appVersion() => version;

  /// What probeServer answers.
  ProbeResult probeResult = ProbeResult.ok;
  final probed = <String>[];

  @override
  String joinAddress(String server) {
    var v = server.trim().replaceFirst(
      RegExp(r'^[a-zA-Z][a-zA-Z0-9+.-]*://'),
      '',
    );
    v = v.replaceFirst(RegExp(r'/+$'), '');
    if (v == 'relay.cometcast.de:443' || v == 'relay.cometcast.de') {
      return 'cometcast.de/app';
    }
    return v.endsWith(':443') ? v.substring(0, v.length - 4) : v;
  }

  @override
  AddressParse serverForAddress(String input) {
    var v = input.trim().replaceFirst(
      RegExp(r'^[a-zA-Z][a-zA-Z0-9+.-]*://'),
      '',
    );
    v = v
        .replaceFirst(RegExp(r'/(app/?)?$'), '')
        .split('/')
        .first
        .toLowerCase();
    if (v.isEmpty || v.contains(' ') || v.contains('@')) {
      return const AddressParse.invalid();
    }
    if (v == 'cometcast.de' || v == 'relay.cometcast.de') {
      return const AddressParse.ok(server: 'relay.cometcast.de:443');
    }
    return AddressParse.ok(server: v.contains(':') ? v : '$v:443');
  }

  @override
  Future<ProbeResult> probeServer(String server) async {
    probed.add(server);
    return probeResult;
  }

  bool airplayAvailable = false;
  final airplayStarts = <({String name, bool askFirst})>[];
  final airplayControllers = <StreamController<AirplayUiEvent>>[];
  final airplayCalls = <String>[];

  @override
  bool airplayReceiveAvailable() => airplayAvailable;

  @override
  Stream<AirplayUiEvent> startAirplayReceive({
    required String name,
    required bool askFirst,
  }) {
    airplayStarts.add((name: name, askFirst: askFirst));
    final c = StreamController<AirplayUiEvent>();
    airplayControllers.add(c);
    return c.stream;
  }

  @override
  bool airplaySetAskFirst(bool ask) {
    airplayCalls.add('askFirst:$ask');
    return true;
  }

  @override
  bool airplaySetLocked(bool locked) {
    airplayCalls.add('locked:$locked');
    return true;
  }

  @override
  bool airplaySetRefusing(bool refusing) {
    airplayCalls.add('refusing:$refusing');
    return true;
  }

  @override
  bool airplayAnswer(bool allow) {
    airplayCalls.add('answer:$allow');
    return true;
  }

  @override
  bool airplayEndSession() {
    airplayCalls.add('end');
    return true;
  }

  @override
  void stopAirplayReceive() => airplayCalls.add('stop');

  bool officeConvert = true;

  @override
  bool officeConvertAvailable() => officeConvert;

  final tvWatchers = <StreamController<TvUiEvent>>[];
  int tvStops = 0;

  @override
  Stream<TvUiEvent> watchTvs() {
    final c = StreamController<TvUiEvent>();
    tvWatchers.add(c);
    return c.stream;
  }

  @override
  void stopWatchingTvs() => tvStops++;

  final tvStarts = <String>[];
  final tvSessions = <StreamController<PresenterUiEvent>>[];
  final tvPins = <String>[];
  bool tvPairResult = true;

  @override
  Stream<PresenterUiEvent> startTvPresenter({required String id}) {
    tvStarts.add(id);
    final c = StreamController<PresenterUiEvent>();
    tvSessions.add(c);
    return c.stream;
  }

  @override
  Future<bool> tvPair(String pin) async {
    tvPins.add(pin);
    return tvPairResult;
  }

  String? tvDataDir;

  @override
  void setTvDataDir(String? path) => tvDataDir = path;

  @override
  bool setCastTrustAnchors(String? path) => path != null;

  @override
  Stream<NearbyUiEvent> watchNearbyScreens({required bool native}) {
    nearbyNativeFlags.add(native);
    final c = StreamController<NearbyUiEvent>();
    nearbySearches.add(c);
    return c.stream;
  }

  @override
  Future<void> stopNearbyScreens() async => nearbyStopCalls++;

  final nativeAnnounce = <StreamController<NativeAnnounceEvent>>[];
  int nativeAnnounceStops = 0;

  @override
  Stream<NativeAnnounceEvent> watchNativeAnnouncements() {
    final c = StreamController<NativeAnnounceEvent>();
    nativeAnnounce.add(c);
    return c.stream;
  }

  @override
  void stopNativeAnnouncements() => nativeAnnounceStops++;

  @override
  void nearbyNativeSeen({
    required String instance,
    required List<String> addresses,
    required List<(String, String)> txt,
  }) => nativeSeen.add((instance: instance, addresses: addresses));

  @override
  void nearbyNativeGone(String instance) => nativeGone.add(instance);

  @override
  void nearbyNativeReset() => nativeResets++;

  @override
  void nearbyNativePermission(NearbyPermission state) =>
      nativePermissions.add(state);

  @override
  bool localNetworkAvailable() => hasLocalNetwork;

  @override
  Stream<SenderUiEvent> startSender({
    required String code,
    required String server,
    int? seconds,
    String? udpBind,
    ScreenRequest? screen,
    String? name,
  }) {
    senderName = name;
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
    String? name,
  }) {
    senderName = name;
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
