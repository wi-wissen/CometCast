// SPDX-License-Identifier: GPL-3.0-or-later
// The Miracast receiver (Windows only): models, the interface the screens use, and the
// dart:ffi implementation of the C interface of crates/cc-miracast-ffi (cc_miracast_ffi.dll).
//
// Everything is polled. Text crosses the boundary as JSON, pictures as raw BGRA bytes.
// The state machine above this file (miracast_controller.dart) is tested with a fake
// [MiracastCore]; the FFI class below is NOT tested on Windows.
//
// UNCERTAIN: never run on Windows (docs/miracast.md, docs/miracast-device-test-plan.md).

import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:typed_data';

/// Version of the C interface this file speaks (cc_miracast_abi_version).
const miracastAbiVersion = 1;

/// Environment variable with the path of cc_miracast_ffi.dll (development and tests).
const miracastLibraryEnvVar = 'COMETCAST_MIRACAST_LIB';

/// Status of the receiver. Names match the `status` strings of the Rust side.
enum MiracastStatus {
  off,
  starting,
  stopping,
  listening,
  connectionPending,
  connected,
  unavailable,
  disabledByPolicy,
  unsupported,
  failed,
}

/// What is wrong, when something is. The screen maps each to a translated hint.
enum MiracastIssue {
  notWindows,
  featureMissing,
  adapterUnsupported,
  adapterNotOptimized,
  disabledByPolicy,
  temporarilyUnavailable,
  accessDenied,
  invalidSettings,
  libraryMissing,
  other,
}

/// How a sender must be authorised. Every foreign device is confirmed by a person in all
/// three modes; the PIN modes add a PIN on top.
enum MiracastPairing {
  confirm('confirm'),
  pinIfRequested('pin_if_requested'),
  pinRequired('pin_required');

  const MiracastPairing(this.wire);

  /// The name in the start JSON.
  final String wire;
}

MiracastStatus parseMiracastStatus(Object? raw) {
  switch (raw) {
    case 'off':
      return MiracastStatus.off;
    case 'starting':
      return MiracastStatus.starting;
    case 'stopping':
      return MiracastStatus.stopping;
    case 'listening':
      return MiracastStatus.listening;
    case 'connection_pending':
      return MiracastStatus.connectionPending;
    case 'connected':
      return MiracastStatus.connected;
    case 'unavailable':
      return MiracastStatus.unavailable;
    case 'disabled_by_policy':
      return MiracastStatus.disabledByPolicy;
    case 'unsupported':
      return MiracastStatus.unsupported;
    default:
      // An unknown word is treated as a failure, never as something that works.
      return MiracastStatus.failed;
  }
}

MiracastIssue? parseMiracastIssue(Object? raw) {
  switch (raw) {
    case null:
      return null;
    case 'not_windows':
      return MiracastIssue.notWindows;
    case 'feature_missing':
      return MiracastIssue.featureMissing;
    case 'adapter_unsupported':
      return MiracastIssue.adapterUnsupported;
    case 'adapter_not_optimized':
      return MiracastIssue.adapterNotOptimized;
    case 'disabled_by_policy':
      return MiracastIssue.disabledByPolicy;
    case 'temporarily_unavailable':
      return MiracastIssue.temporarilyUnavailable;
    case 'access_denied':
      return MiracastIssue.accessDenied;
    case 'invalid_settings':
      return MiracastIssue.invalidSettings;
    default:
      return MiracastIssue.other;
  }
}

/// A sender that waits for the person's decision.
class MiracastRequest {
  const MiracastRequest({required this.id, required this.name, this.pin});

  final int id;

  /// The sender's name, already cleaned by the Rust side (no control characters).
  final String name;

  /// PIN to show, if Windows gave one.
  final String? pin;

  @override
  bool operator ==(Object other) =>
      other is MiracastRequest &&
      other.id == id &&
      other.name == name &&
      other.pin == pin;

  @override
  int get hashCode => Object.hash(id, name, pin);
}

/// Counters of the picture path.
class MiracastFrameStats {
  const MiracastFrameStats({
    this.published = 0,
    this.taken = 0,
    this.dropped = 0,
    this.failed = 0,
    this.width = 0,
    this.height = 0,
  });

  final int published;
  final int taken;
  final int dropped;
  final int failed;
  final int width;
  final int height;
}

/// The state the Rust side reports (see crates/cc-miracast-ffi/src/controller.rs).
class MiracastState {
  const MiracastState({
    required this.status,
    this.issue,
    this.generation = 0,
    this.pending,
    this.connected,
    this.lastError,
    this.rejected = 0,
    this.failed = 0,
    this.frames = const MiracastFrameStats(),
    this.feature = 'unknown',
    this.adapter = 'undetermined',
  });

  static const off = MiracastState(status: MiracastStatus.off);

  final MiracastStatus status;
  final MiracastIssue? issue;

  /// Grows with every start; lets the UI tell sessions apart.
  final int generation;
  final MiracastRequest? pending;

  /// Name of the connected sender, if any.
  final String? connected;
  final String? lastError;

  /// How many foreign senders were turned away without asking.
  final int rejected;

  /// How many connections failed.
  final int failed;
  final MiracastFrameStats frames;

  /// `installed`, `not_installed` or `unknown` (the optional feature "Wireless Display").
  final String feature;

  /// `supported`, `not_optimized`, `not_supported` or `undetermined`.
  final String adapter;

  /// The same state with the waiting request removed (answered locally, not confirmed by the
  /// core yet). A pending status falls back to listening.
  MiracastState withoutPending() => MiracastState(
    status: status == MiracastStatus.connectionPending ? MiracastStatus.listening : status,
    issue: issue,
    generation: generation,
    pending: null,
    connected: connected,
    lastError: lastError,
    rejected: rejected,
    failed: failed,
    frames: frames,
    feature: feature,
    adapter: adapter,
  );

  factory MiracastState.fromJson(Map<String, Object?> j) {
    int i(Object? v) => v is num ? v.toInt() : 0;
    String? s(Object? v) => v is String ? v : null;
    final p = j['pending'];
    MiracastRequest? pending;
    if (p is Map && p['id'] is num && p['name'] is String) {
      pending = MiracastRequest(
        id: i(p['id']),
        name: p['name'] as String,
        pin: s(p['pin']),
      );
    }
    final f = j['frames'];
    var frames = const MiracastFrameStats();
    if (f is Map) {
      frames = MiracastFrameStats(
        published: i(f['published']),
        taken: i(f['taken']),
        dropped: i(f['dropped']),
        failed: i(f['failed']),
        width: i(f['width']),
        height: i(f['height']),
      );
    }
    return MiracastState(
      status: parseMiracastStatus(j['status']),
      issue: parseMiracastIssue(j['issue']),
      generation: i(j['generation']),
      pending: pending,
      connected: s(j['connected']),
      lastError: s(j['last_error']),
      rejected: i(j['rejected']),
      failed: i(j['failed']),
      frames: frames,
      feature: s(j['feature']) ?? 'unknown',
      adapter: s(j['adapter']) ?? 'undetermined',
    );
  }
}

/// What a look at the system found, before anything is started.
class MiracastProbe {
  const MiracastProbe({
    this.issue,
    this.feature = 'unknown',
    this.adapter = 'undetermined',
    this.receiverAvailable = false,
  });

  /// A problem that is clear before starting, or null.
  final MiracastIssue? issue;
  final String feature;
  final String adapter;
  final bool receiverAvailable;

  factory MiracastProbe.fromJson(Map<String, Object?> j) => MiracastProbe(
    issue: parseMiracastIssue(j['issue']),
    feature: j['feature'] is String ? j['feature'] as String : 'unknown',
    adapter: j['adapter'] is String ? j['adapter'] as String : 'undetermined',
    receiverAvailable: j['receiver_available'] == true,
  );
}

/// One picture: BGRA, rows without padding.
class MiracastFrame {
  const MiracastFrame({
    required this.width,
    required this.height,
    required this.seq,
    required this.bgra,
  });

  final int width;
  final int height;
  final int seq;
  final Uint8List bgra;
}

/// The start call failed before a receiver existed (bad arguments, already running, library).
class MiracastCallException implements Exception {
  const MiracastCallException(this.code, [this.message]);

  /// Return code of the C function (negative), or 0 for "library not available".
  final int code;
  final String? message;

  /// The library could not be loaded or has another version.
  bool get isLibraryProblem => code == 0;

  @override
  String toString() => 'MiracastCallException($code${message == null ? '' : ', $message'})';
}

/// What the screens use. The real one talks to the DLL; tests use a fake.
abstract class MiracastCore {
  /// Look at the system (optional feature, adapter). Never throws; a missing library is
  /// reported as [MiracastIssue.libraryMissing].
  Future<MiracastProbe> probe();

  /// Start the receiver (returns at once; poll [state]). Throws [MiracastCallException].
  Future<void> start({required String name, required MiracastPairing pairing});

  /// Stop the receiver. Safe when nothing runs.
  Future<void> stop();

  /// Current state. Throws [MiracastCallException] if the library is unusable.
  Future<MiracastState> state();

  /// Answer the waiting request.
  Future<void> respond(int id, bool accept);

  /// End the active connection, keep listening.
  Future<void> disconnect();

  /// The newest picture, or null if there is no new one.
  Future<MiracastFrame?> pollFrame();

  /// Largest picture size the Rust side hands out.
  Future<void> setMaxSize(int width, int height);

  /// Mute or unmute the audio.
  Future<void> setMuted(bool muted);
}

/// Candidate places of cc_miracast_ffi.dll, in search order (pure, for tests): next to the
/// executable (where the CMake install step puts it), then in `lib/`.
List<String> miracastLibraryCandidates({
  required String exeDir,
  String separator = '\\',
}) {
  const name = 'cc_miracast_ffi.dll';
  return [
    [exeDir, name].join(separator),
    [exeDir, 'lib', name].join(separator),
  ];
}

/// Path to load: the environment override, else the first candidate that exists, else the
/// plain file name (system loader path).
String resolveMiracastLibraryPath({
  Map<String, String>? environment,
  String? executableDir,
  String? separator,
  bool Function(String path)? fileExists,
}) {
  final override = (environment ?? Platform.environment)[miracastLibraryEnvVar];
  if (override != null && override.isNotEmpty) return override;
  final exeDir = executableDir ?? File(Platform.resolvedExecutable).parent.path;
  final exists = fileExists ?? (String p) => File(p).existsSync();
  for (final c in miracastLibraryCandidates(
    exeDir: exeDir,
    separator: separator ?? Platform.pathSeparator,
  )) {
    if (exists(c)) return c;
  }
  return 'cc_miracast_ffi.dll';
}

/// Mirror of `CcFrameInfo` in crates/cc-miracast-ffi/src/abi.rs (repr(C): u32, u32, u64).
final class CcFrameInfo extends Struct {
  @Uint32()
  external int width;

  @Uint32()
  external int height;

  @Uint64()
  external int seq;
}

// Native and Dart signatures of the C functions.
typedef _AbiNative = Uint32 Function();
typedef _AbiDart = int Function();
typedef _AllocNative = Pointer<Uint8> Function(Size);
typedef _AllocDart = Pointer<Uint8> Function(int);
typedef _FreeNative = Void Function(Pointer<Uint8>, Size);
typedef _FreeDart = void Function(Pointer<Uint8>, int);
typedef _BufNative = Int32 Function(Pointer<Uint8>, Size);
typedef _BufDart = int Function(Pointer<Uint8>, int);
typedef _VoidNative = Int32 Function();
typedef _VoidDart = int Function();
typedef _RespondNative = Int32 Function(Uint64, Int32);
typedef _RespondDart = int Function(int, int);
typedef _SizeNative = Int32 Function(Uint32, Uint32);
typedef _SizeDart = int Function(int, int);
typedef _IntNative = Int32 Function(Int32);
typedef _IntDart = int Function(int);
typedef _FrameNative = Int32 Function(Pointer<Uint8>, Size, Pointer<CcFrameInfo>);
typedef _FrameDart = int Function(Pointer<Uint8>, int, Pointer<CcFrameInfo>);

/// Return codes of the C interface (crates/cc-miracast-ffi/src/abi.rs).
const _errBuffer = -3;

/// The real implementation over dart:ffi. The library is loaded on first use; a missing file
/// or a wrong interface version shows up as a [MiracastCallException] with code 0.
///
/// UNCERTAIN: never run. The calls are synchronous and short; the Rust side does the slow
/// parts (start, stop) on threads of its own.
class FfiMiracastCore implements MiracastCore {
  FfiMiracastCore({DynamicLibrary Function()? opener})
    : _opener = opener ?? (() => DynamicLibrary.open(resolveMiracastLibraryPath()));

  final DynamicLibrary Function() _opener;
  _Api? _api;

  _Api get _a {
    final existing = _api;
    if (existing != null) return existing;
    final DynamicLibrary lib;
    try {
      lib = _opener();
    } catch (e) {
      throw MiracastCallException(0, 'cannot load cc_miracast_ffi.dll: $e');
    }
    final api = _Api(lib);
    final v = api.abiVersion();
    if (v != miracastAbiVersion) {
      throw MiracastCallException(0, 'interface version $v, expected $miracastAbiVersion');
    }
    return _api = api;
  }

  @override
  Future<MiracastProbe> probe() async {
    try {
      final j = _a.readJson(_a.probe);
      return MiracastProbe.fromJson(j);
    } on MiracastCallException {
      return const MiracastProbe(issue: MiracastIssue.libraryMissing);
    }
  }

  @override
  Future<void> start({required String name, required MiracastPairing pairing}) async {
    final a = _a;
    final bytes = utf8.encode(jsonEncode({'name': name, 'pairing': pairing.wire}));
    final p = a.alloc(bytes.length);
    if (p == nullptr) throw const MiracastCallException(-2, 'allocation failed');
    try {
      p.asTypedList(bytes.length).setAll(0, bytes);
      final rc = a.start(p, bytes.length);
      if (rc != 0) throw MiracastCallException(rc);
    } finally {
      a.free(p, bytes.length);
    }
  }

  @override
  Future<void> stop() async {
    _a.stop();
  }

  @override
  Future<MiracastState> state() async => MiracastState.fromJson(_a.readJson(_a.state));

  @override
  Future<void> respond(int id, bool accept) async {
    final rc = _a.respond(id, accept ? 1 : 0);
    if (rc != 0) throw MiracastCallException(rc);
  }

  @override
  Future<void> disconnect() async {
    final rc = _a.disconnect();
    if (rc != 0) throw MiracastCallException(rc);
  }

  @override
  Future<MiracastFrame?> pollFrame() async => _a.pollFrame();

  @override
  Future<void> setMaxSize(int width, int height) async {
    _a.setMaxSize(width, height);
  }

  @override
  Future<void> setMuted(bool muted) async {
    _a.setMuted(muted ? 1 : 0);
  }
}

/// Looked-up functions and the buffers that are reused between calls.
class _Api {
  _Api(DynamicLibrary lib)
    : abiVersion = lib.lookupFunction<_AbiNative, _AbiDart>('cc_miracast_abi_version'),
      alloc = lib.lookupFunction<_AllocNative, _AllocDart>('cc_miracast_alloc'),
      free = lib.lookupFunction<_FreeNative, _FreeDart>('cc_miracast_free'),
      probe = lib.lookupFunction<_BufNative, _BufDart>('cc_miracast_probe'),
      start = lib.lookupFunction<_BufNative, _BufDart>('cc_miracast_start'),
      stop = lib.lookupFunction<_VoidNative, _VoidDart>('cc_miracast_stop'),
      state = lib.lookupFunction<_BufNative, _BufDart>('cc_miracast_state'),
      respond = lib.lookupFunction<_RespondNative, _RespondDart>('cc_miracast_respond'),
      disconnect = lib.lookupFunction<_VoidNative, _VoidDart>('cc_miracast_disconnect'),
      setMaxSize = lib.lookupFunction<_SizeNative, _SizeDart>('cc_miracast_set_max_size'),
      setMuted = lib.lookupFunction<_IntNative, _IntDart>('cc_miracast_set_muted'),
      _pollFrame = lib.lookupFunction<_FrameNative, _FrameDart>('cc_miracast_poll_frame');

  final _AbiDart abiVersion;
  final _AllocDart alloc;
  final _FreeDart free;
  final _BufDart probe;
  final _BufDart start;
  final _VoidDart stop;
  final _BufDart state;
  final _RespondDart respond;
  final _VoidDart disconnect;
  final _SizeDart setMaxSize;
  final _IntDart setMuted;
  final _FrameDart _pollFrame;

  static const _jsonCap = 8192;
  static const _defaultFrameBytes = 1920 * 1080 * 4;

  Pointer<Uint8>? _json;
  Pointer<Uint8>? _frame;
  int _frameCap = 0;
  Pointer<CcFrameInfo>? _info;

  /// Call a function that writes JSON into a buffer and decode it.
  Map<String, Object?> readJson(_BufDart fn) {
    final buf = _json ??= alloc(_jsonCap);
    if (buf == nullptr) throw const MiracastCallException(-2, 'allocation failed');
    final n = fn(buf, _jsonCap);
    if (n < 0) throw MiracastCallException(n);
    final decoded = jsonDecode(utf8.decode(buf.asTypedList(n)));
    if (decoded is! Map) throw const MiracastCallException(-2, 'bad JSON');
    return decoded.cast<String, Object?>();
  }

  MiracastFrame? pollFrame() {
    final info = _info ??= alloc(sizeOf<CcFrameInfo>()).cast<CcFrameInfo>();
    if (info == nullptr) throw const MiracastCallException(-2, 'allocation failed');
    if (_frame == null) {
      _frame = alloc(_defaultFrameBytes);
      _frameCap = _defaultFrameBytes;
      if (_frame == nullptr) {
        _frame = null;
        throw const MiracastCallException(-2, 'allocation failed');
      }
    }
    var rc = _pollFrame(_frame!, _frameCap, info);
    if (rc == _errBuffer) {
      // The picture is bigger than the buffer: grow it (it stays in the mailbox) and retry.
      final need = info.ref.width * info.ref.height * 4;
      if (need <= 0 || need > (1 << 28)) return null;
      free(_frame!, _frameCap);
      _frame = alloc(need);
      _frameCap = need;
      if (_frame == nullptr) {
        _frame = null;
        _frameCap = 0;
        throw const MiracastCallException(-2, 'allocation failed');
      }
      rc = _pollFrame(_frame!, _frameCap, info);
    }
    if (rc == 0) return null;
    if (rc < 0) throw MiracastCallException(rc);
    final w = info.ref.width;
    final h = info.ref.height;
    final len = w * h * 4;
    // Copy: the native buffer is reused by the next call.
    return MiracastFrame(
      width: w,
      height: h,
      seq: info.ref.seq,
      bgra: Uint8List.fromList(_frame!.asTypedList(len)),
    );
  }
}
