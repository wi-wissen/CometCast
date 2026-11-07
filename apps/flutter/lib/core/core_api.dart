// SPDX-License-Identifier: GPL-3.0-or-later
// The part of the Rust core that the UI uses, as a small Dart interface.
//
// Why an interface: widget tests plug in a fake (no native library needed), the real app
// plugs in [RustCoreApi], which calls the generated flutter_rust_bridge code.

import 'dart:typed_data';

import 'package:cometcast/src/rust/api.dart' as rust;
import 'package:cometcast/src/rust/api.dart'
    show
        AdvertiseOutcome,
        CaptureInfo,
        ContentFrame,
        ContentSenderUiEvent,
        ContentSource,
        EncodedVideoUnit,
        LinkCheck,
        NearbyReceiverEvent,
        PushResult,
        ReceiverUiEvent,
        RoomCodeCheck,
        ScreenRequest,
        SenderUiEvent,
        ServerDefaults,
        ServerDiscovery,
        UiCodecInfo,
        UiCodecState,
        UiContentCmd,
        UiJoinMode,
        VideoFrame;

// The event and result classes (generated) are re-exported for the screens and tests.
export 'package:cometcast/src/rust/api.dart'
    hide
        startReceiver,
        stopReceiver,
        setJoinMode,
        decideJoin,
        removePeer,
        startSender,
        startEncodedSender,
        pushEncodedFrame,
        pushCodecConfig,
        keyframeRequested,
        stopSender,
        checkRoomCode,
        roomCodeAlphabet,
        roomCodeLength,
        takeFrame,
        setVideoMaxSize,
        setVideoPassthrough,
        requestVideoKeyframe,
        takeEncodedVideo,
        takeContentFrame,
        setContentTargetWidth,
        validateLink,
        setDocsWorker,
        docsWorkerPath,
        setOfficeWorker,
        officeWorkerPath,
        startContentSender,
        sendContentCmd,
        captureInfo,
        serverDefaults,
        discoverServers,
        startAdvertising,
        stopAdvertising,
        watchReceivers,
        stopWatchingReceivers,
        codecState,
        codecInfo,
        ensureCodec,
        setCodecDir;

abstract class CoreApi {
  /// Connect to [server], create a room and report events. The first event is the room code.
  /// A start failure arrives as an `error` event, after which the stream ends.
  ///
  /// With [content] the receiver also accepts images, PDFs and links on the control channel
  /// and reports them as the `content...` events (see `take_content_frame` in api.rs).
  ///
  /// [joinMode] says who may join the room from the start (see [setJoinMode]).
  Stream<ReceiverUiEvent> startReceiver({
    required String server,
    String? udpBind,
    bool content = true,
    UiJoinMode joinMode = UiJoinMode.open,
  });

  Future<void> stopReceiver();

  /// Change who may join the running room, at any time (the server confirms with a
  /// `joinModeChanged` event). False if no receiver runs.
  bool setJoinMode(UiJoinMode mode);

  /// Allow or deny the waiting request [id]. With [id] null and [allow] false: deny everybody who
  /// waits. False if no receiver runs.
  bool decideJoin({BigInt? id, required bool allow});

  /// Remove the connected sender (the receiver then ends with `peerLeft`).
  bool removePeer();

  /// The received image or PDF page that waits, or null. Call it after a `contentFrameReady`
  /// event; fetching re-arms that event. Only the latest picture is kept.
  Future<ContentFrame?> takeContentFrame();

  /// Width in physical pixels that PDF pages are rendered at (the view width times the device
  /// pixel ratio). The page that is shown is rendered again.
  void setContentTargetWidth(int width);

  /// The link rules, in Rust (`cc-files`): valid http(s) links come back normalised with their
  /// punycode host. The only place where link rules live; call before sending and before opening.
  LinkCheck validateLink(String input);

  /// Use this render worker program for PDFs, or null for the default search. Tests and hosts
  /// with another layout; the app normally finds the worker next to its executable.
  void setDocsWorker(String? path);

  /// The PDF render worker that would be used now, or null if none was found.
  String? docsWorkerPath();

  /// Use this program to convert Office files, or null for the default search (next to the
  /// executable). Tests and hosts with another layout.
  void setOfficeWorker(String? path);

  /// The Office conversion worker that would be used now, or null if none was found (Office
  /// files are then refused with [UiSendRejection.officeUnavailable]).
  String? officeWorkerPath();

  /// Check [source] (an image, PDF or Office file, or a link), join the room [code] and send it with
  /// progress; commands follow with [sendContentCmd]. Errors arrive as events.
  Stream<ContentSenderUiEvent> startContentSender({
    required String code,
    required String server,
    String? udpBind,
    required ContentSource source,
    String? name,
  });

  /// Queue a presenter command (next/previous/go to page/stop) for the running content sender.
  /// False if it was not accepted (no sender, queue full, page 0).
  bool sendContentCmd(UiContentCmd cmd);

  /// The newest decoded picture (RGBA) that was not fetched yet, or null. Call it after a
  /// `frameReady` event; fetching re-arms that event.
  Future<VideoFrame?> takeFrame();

  /// Size of the drawing area in physical pixels; the core scales pictures down to fit.
  void setVideoMaxSize(int width, int height);

  /// Hand the received H.264 undecoded to the host (Android `MediaCodec`) instead of decoding
  /// it in Rust. Call before [startReceiver]; the choice is kept. See `set_video_passthrough`.
  void setVideoPassthrough(bool on);

  /// Ask the sender for a key frame (rate limited by the core).
  void requestVideoKeyframe();

  /// The encoded access units that wait in pass-through mode (after an `EncodedReady` event).
  Future<List<EncodedVideoUnit>> takeEncodedVideo();

  /// Join the room [code] and send the built-in test video for [seconds] seconds, or with
  /// [screen] the screen of this computer (until stopped when [seconds] is null).
  Stream<SenderUiEvent> startSender({
    required String code,
    required String server,
    int? seconds,
    String? udpBind,
    ScreenRequest? screen,
    String? name,
  });

  /// Join the room [code] and send the H.264 access units that the host pushes with
  /// [pushEncodedFrame] (Android: the output of the native screen encoder) until stopped.
  /// Same event rules as [startSender].
  Stream<SenderUiEvent> startEncodedSender({
    required String code,
    required String server,
    String? udpBind,
    String? name,
  });

  /// Hand one H.264 access unit (Annex B or AVCC) to the encoded sender. The bytes are copied.
  /// Never blocks; a unit that cannot be taken is dropped and the result says why.
  PushResult pushEncodedFrame(
    Uint8List data, {
    required bool isKey,
    required int ptsUs,
  });

  /// Hand the codec configuration (SPS/PPS, Annex B or avcC) to the encoded sender. False if
  /// no encoded sender runs or the data has no parameter sets.
  bool pushCodecConfig(Uint8List data);

  /// True once per key frame request of the Rust core: ask the encoder for a sync frame.
  bool keyframeRequested();

  /// Can this system share its screen, and which monitors exist? [display] forces an X11
  /// display such as `:99` (development and tests).
  Future<CaptureInfo> captureInfo({String? display});

  /// Server addresses to pre-fill (from cc-discovery) and the development fallback.
  ServerDefaults serverDefaults();

  /// Ask all sources (policy, DNS-SRV, announcements) for servers. Never throws in the real
  /// core; the time limits are set in Rust. A Rust future cannot be cancelled from Dart, so a
  /// screen that closes in the meantime simply ignores the answer (see `AppSettings.discover`).
  Future<ServerDiscovery> discoverServers();

  /// Announce this receiver (and its preferred server, never the room code) on the local
  /// network until [stopAdvertising]. Replaces a running announcement.
  Future<AdvertiseOutcome> startAdvertising({
    required String name,
    String? preferredServer,
  });

  Future<void> stopAdvertising();

  /// Receivers announced on the local network, until [stopWatchingReceivers] or until the
  /// listener cancels.
  Stream<NearbyReceiverEvent> watchReceivers();

  Future<void> stopWatchingReceivers();

  Future<void> stopSender();

  /// State of the H.264 codec (Cisco OpenH264, downloaded on first use in release builds).
  /// `ready` in builds that contain the codec.
  UiCodecState codecState();

  /// Facts for the consent text (host, file name, size, administrator settings); null if this
  /// build never downloads the codec.
  UiCodecInfo? codecInfo();

  /// Import the administrator's file or download the codec. Only after the person agreed (or
  /// when [UiCodecInfo.autoDownload] is true). While it runs, [codecState] reports progress.
  Future<UiCodecState> ensureCodec();

  /// Android: the app-private directory in which the codec is stored.
  void setCodecDir(String? path);

  /// Same rules as `cc_proto::RoomCode::parse` (the Rust code is the single source of truth).
  RoomCodeCheck checkRoomCode(String input);

  /// Number of characters in a room code.
  int get roomCodeLength;

  /// The characters a room code may contain (uppercase).
  String get roomCodeAlphabet;
}

/// The real implementation: calls the Rust library.
class RustCoreApi implements CoreApi {
  const RustCoreApi();

  @override
  Stream<ReceiverUiEvent> startReceiver({
    required String server,
    String? udpBind,
    bool content = true,
    UiJoinMode joinMode = UiJoinMode.open,
  }) => rust.startReceiver(
    server: server,
    udpBind: udpBind,
    content: content,
    joinMode: joinMode,
  );

  @override
  Future<void> stopReceiver() => rust.stopReceiver();

  @override
  bool setJoinMode(UiJoinMode mode) => rust.setJoinMode(mode: mode);

  @override
  bool decideJoin({BigInt? id, required bool allow}) =>
      rust.decideJoin(id: id, allow: allow);

  @override
  bool removePeer() => rust.removePeer();

  @override
  Future<ContentFrame?> takeContentFrame() => rust.takeContentFrame();

  @override
  void setContentTargetWidth(int width) =>
      rust.setContentTargetWidth(width: width);

  @override
  LinkCheck validateLink(String input) => rust.validateLink(input: input);

  @override
  void setDocsWorker(String? path) => rust.setDocsWorker(path: path);

  @override
  String? docsWorkerPath() => rust.docsWorkerPath();

  @override
  void setOfficeWorker(String? path) => rust.setOfficeWorker(path: path);

  @override
  String? officeWorkerPath() => rust.officeWorkerPath();

  @override
  Stream<ContentSenderUiEvent> startContentSender({
    required String code,
    required String server,
    String? udpBind,
    required ContentSource source,
    String? name,
  }) => rust.startContentSender(
    code: code,
    server: server,
    udpBind: udpBind,
    source: source,
    name: name,
  );

  @override
  bool sendContentCmd(UiContentCmd cmd) => rust.sendContentCmd(cmd: cmd);

  @override
  Future<VideoFrame?> takeFrame() => rust.takeFrame();

  @override
  void setVideoMaxSize(int width, int height) =>
      rust.setVideoMaxSize(width: width, height: height);

  @override
  void setVideoPassthrough(bool on) => rust.setVideoPassthrough(on_: on);

  @override
  void requestVideoKeyframe() => rust.requestVideoKeyframe();

  @override
  Future<List<EncodedVideoUnit>> takeEncodedVideo() => rust.takeEncodedVideo();

  @override
  Future<CaptureInfo> captureInfo({String? display}) =>
      rust.captureInfo(display: display);

  @override
  ServerDefaults serverDefaults() => rust.serverDefaults();

  @override
  Future<ServerDiscovery> discoverServers() => rust.discoverServers();

  @override
  Future<AdvertiseOutcome> startAdvertising({
    required String name,
    String? preferredServer,
  }) => rust.startAdvertising(name: name, preferredServer: preferredServer);

  @override
  Future<void> stopAdvertising() => rust.stopAdvertising();

  @override
  Stream<NearbyReceiverEvent> watchReceivers() => rust.watchReceivers();

  @override
  Future<void> stopWatchingReceivers() => rust.stopWatchingReceivers();

  @override
  Stream<SenderUiEvent> startSender({
    required String code,
    required String server,
    int? seconds,
    String? udpBind,
    ScreenRequest? screen,
    String? name,
  }) => rust.startSender(
    code: code,
    server: server,
    seconds: seconds,
    udpBind: udpBind,
    screen: screen,
    name: name,
  );

  @override
  Stream<SenderUiEvent> startEncodedSender({
    required String code,
    required String server,
    String? udpBind,
    String? name,
  }) => rust.startEncodedSender(
    code: code,
    server: server,
    udpBind: udpBind,
    name: name,
  );

  @override
  PushResult pushEncodedFrame(
    Uint8List data, {
    required bool isKey,
    required int ptsUs,
  }) => rust.pushEncodedFrame(data: data, isKey: isKey, ptsUs: ptsUs);

  @override
  bool pushCodecConfig(Uint8List data) => rust.pushCodecConfig(data: data);

  @override
  bool keyframeRequested() => rust.keyframeRequested();

  @override
  Future<void> stopSender() => rust.stopSender();

  @override
  UiCodecState codecState() => rust.codecState();

  @override
  UiCodecInfo? codecInfo() => rust.codecInfo();

  @override
  Future<UiCodecState> ensureCodec() => rust.ensureCodec();

  @override
  void setCodecDir(String? path) => rust.setCodecDir(path: path);

  @override
  RoomCodeCheck checkRoomCode(String input) => rust.checkRoomCode(input: input);

  @override
  int get roomCodeLength => rust.roomCodeLength();

  @override
  String get roomCodeAlphabet => rust.roomCodeAlphabet();
}
