// SPDX-License-Identifier: GPL-3.0-or-later
// The rules of the display that need no widgets: the setup wizard, what is remembered, the
// connection view, name cleaning and the timings. Ported from apps/web/src/core
// (displaySetup.ts, connectionView.ts, idleDim.ts, controlsReveal.ts, joinPrefs.ts).

import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'package:shared_preferences/shared_preferences.dart';

import '../design/tokens.dart';

// --- timings (spec 3.6, 4.2, 4.4, 8) -----------------------------------------------------

/// Name tag and End stay this long after the last input.
const controlsHide = Duration(seconds: 5);

/// Mouse movement shows the controls at most once per this time.
const mouseThrottle = Duration(milliseconds: 400);

/// Input is ignored this long after a join request (or the next one) appears.
const requestGuard = Duration(seconds: 1);

/// Idle dimming after this long without activity (spec 8.6; one constant to change).
const idleDimAfter = Duration(minutes: 7);

/// A toast stays this long.
const toastFor = Duration(milliseconds: 3200);

/// Back on the idle screen of an Android TV leaves the app only on a second press within this time.
const doubleBackWindow = Duration(seconds: 2);

/// The offline box appears when the attempt after this much silence has failed (spec 4.2).
const offlineAfterFailure = Duration(seconds: 3);

/// Connecting becomes offline after this much silence without any failure (spec 4).
const offlineAfterSilence = Duration(seconds: 8);

// --- names (spec 4.4) --------------------------------------------------------------------

const roomNameMax = 32;

final _controlChars = RegExp(
  '[\\u0000-\\u001f\\u007f-\\u009f\\u061c\\u200b-\\u200f\\u2028-\\u202e'
  '\\u2060-\\u206f\\ufeff\\ue000-\\uf8ff]',
);

/// Strips control and invisible characters and collapses white space.
String stripName(String raw) => raw
    .replaceAll(RegExp(r'[\t\n\r]'), ' ')
    .replaceAll(_controlChars, '')
    .replaceAll(RegExp(r'\s+'), ' ')
    .trim();

/// A room name: cleaned and cut to [roomNameMax] characters.
String cleanRoomName(String raw) =>
    String.fromCharCodes(stripName(raw).runes.take(roomNameMax));

/// The name in a request: cut to [max] characters plus an ellipsis (spec 4.4).
String shortName(String name, [int max = 18]) {
  final r = name.runes.toList();
  return r.length <= max ? name : '${String.fromCharCodes(r.take(max))}…';
}

/// The join code as two groups: `K7Q M2W`.
String formatCode(String code) {
  final c = code.replaceAll(RegExp(r'\s+'), '');
  if (c.length < 4) return c;
  final half = (c.length + 1) ~/ 2;
  return '${c.substring(0, half)} ${c.substring(half)}';
}

/// The server address as shown and encoded: `host[:port]` without the default port 443
/// (decision N12; the glossary `{dom}`).
String displayHost(String server) {
  var v = server.trim().replaceFirst(
    RegExp(r'^[a-zA-Z][a-zA-Z0-9+.-]*://'),
    '',
  );
  v = v.replaceFirst(RegExp(r'/+$'), '');
  return v.endsWith(':443') ? v.substring(0, v.length - 4) : v;
}

/// Is [host] (from [displayHost]) an IP address, not a name? IPv4 with an optional port, or an
/// IPv6 literal in brackets.
bool isIpHost(String host) =>
    RegExp(r'^\d{1,3}(\.\d{1,3}){3}(:\d+)?$').hasMatch(host) ||
    host.startsWith('[');

/// What the QR code encodes: `https://{dom}/{CODE}` (spec 4.2), `{dom}` without port 443.
String joinUrl(String dom, String code) =>
    'https://${displayHost(dom)}/${code.replaceAll(RegExp(r'\s+'), '')}';

// --- setup (spec 4.1) --------------------------------------------------------------------

/// The five room name suggestions (keys `setup.name.s1` to `s5`) are given by the screen from
/// the translations; the wizard only keeps the chosen text.
enum DefaultMode { open, confirm }

class WizardState {
  const WizardState({
    this.step = 0,
    required this.room,
    this.custom = '',
    this.mode = DefaultMode.open,
    this.previousChip,
  });

  /// 0 welcome, 1 room name, 2 who may connect, 3 address, 4 done.
  final int step;

  /// The name that is chosen: the chip or the cleaned custom text, whichever is set.
  final String room;

  /// What is typed in the custom field, as typed. Starts empty.
  final String custom;
  final DefaultMode mode;

  /// The chip that was selected before typing (selected again when the field is emptied).
  final String? previousChip;

  WizardState copyWith({
    int? step,
    String? room,
    String? custom,
    DefaultMode? mode,
    String? previousChip,
    bool clearPrevious = false,
  }) => WizardState(
    step: step ?? this.step,
    room: room ?? this.room,
    custom: custom ?? this.custom,
    mode: mode ?? this.mode,
    previousChip: clearPrevious ? null : (previousChip ?? this.previousChip),
  );

  /// Is this suggestion the selected one? Never together with a non-empty custom name.
  bool chipSelected(String suggestion) =>
      custom.trim().isEmpty && room == cleanRoomName(suggestion);
}

WizardState initialWizard(String firstSuggestion) =>
    WizardState(room: cleanRoomName(firstSuggestion));

WizardState wizardNext(WizardState s) =>
    s.step >= 4 ? s : s.copyWith(step: s.step + 1);

WizardState wizardBack(WizardState s) =>
    s.step <= 0 ? s : s.copyWith(step: s.step - 1);

/// A chip was chosen: it clears the custom field.
WizardState wizardChip(WizardState s, String suggestion) => s.copyWith(
  room: cleanRoomName(suggestion),
  custom: '',
  clearPrevious: true,
);

/// The custom field changed. A non-empty text deselects all chips; emptying it again selects
/// the chip that was selected before typing (decision 13.39; at first the first suggestion).
WizardState wizardCustom(WizardState s, String value, String firstSuggestion) {
  final custom = String.fromCharCodes(value.runes.take(roomNameMax));
  if (custom.trim().isEmpty) {
    final back = s.previousChip ?? firstSuggestion;
    return s.copyWith(
      custom: custom,
      room: cleanRoomName(back),
      clearPrevious: true,
    );
  }
  // Remember the chip that was selected, once, when typing starts.
  final wasChip = s.custom.trim().isEmpty ? s.room : s.previousChip;
  return s.copyWith(
    custom: custom,
    room: cleanRoomName(custom),
    previousChip: wasChip,
  );
}

WizardState wizardMode(WizardState s, DefaultMode m) => s.copyWith(mode: m);

/// What the wizard produced and the display remembers (spec 4.1: Finish stores room name,
/// default join mode and server).
class DisplaySetup {
  const DisplaySetup({
    required this.room,
    required this.mode,
    required this.server,
  });
  final String room;
  final DefaultMode mode;

  /// `host[:port]` chosen in step 3, or null for "whatever the core resolves".
  final String? server;

  DisplaySetup copyWith({String? room, DefaultMode? mode, String? server}) =>
      DisplaySetup(
        room: room ?? this.room,
        mode: mode ?? this.mode,
        server: server ?? this.server,
      );
}

// --- what is remembered (spec 4.3, 10) ---------------------------------------------------

/// Where the display keeps its setup and appearance. Failures are ignored: the wizard shows
/// again, the appearance is Dark.
abstract class DisplayStore {
  Future<DisplaySetup?> loadSetup();
  Future<void> saveSetup(DisplaySetup s);
  Future<Appearance> loadAppearance();
  Future<void> saveAppearance(Appearance a);

  /// The key of this installation (see the signed announcements design): 32 secret bytes of an
  /// Ed25519 key made on first use with [generate] (the core's operating system randomness) and
  /// kept with the settings; the display's id derives from it. A new key after [reset]. Not the
  /// host name, not a hardware id. UNCERTAIN: it lives in the app's preferences, not in a
  /// hardware-backed keystore (see docs/measurement-points.md).
  Future<DisplayKeyData> key(Uint8List Function() generate);

  /// Remember the highest `seq` the core may have used with the key (from
  /// `displaySeqToPersist`), so a clock that goes back after a restart cannot reuse values.
  Future<void> saveSeq(int seq);

  /// "Reset the app's data" (spec 10): forget setup, appearance and the key (so the id changes).
  Future<void> reset();
}

/// The display key and the persisted `seq` floor.
class DisplayKeyData {
  const DisplayKeyData(this.secret, this.seqFloor);
  final Uint8List secret;
  final int seqFloor;
}

class MemoryDisplayStore implements DisplayStore {
  MemoryDisplayStore({this.setup, this.appearance = Appearance.dark});
  DisplaySetup? setup;
  Appearance appearance;

  @override
  Future<DisplaySetup?> loadSetup() async => setup;
  @override
  Future<void> saveSetup(DisplaySetup s) async => setup = s;
  @override
  Future<Appearance> loadAppearance() async => appearance;
  @override
  Future<void> saveAppearance(Appearance a) async => appearance = a;
  Uint8List? _key;
  int _seq = 0;

  @override
  Future<DisplayKeyData> key(Uint8List Function() generate) async =>
      DisplayKeyData(_key ??= generate(), _seq);

  @override
  Future<void> saveSeq(int seq) async => _seq = seq;

  @override
  Future<void> reset() async {
    setup = null;
    appearance = Appearance.dark;
    _key = null;
    _seq = 0;
  }
}

class PrefsDisplayStore implements DisplayStore {
  const PrefsDisplayStore();
  static const _room = 'display.room';
  static const _mode = 'display.mode';
  static const _server = 'display.server';
  static const _look = 'display.appearance';
  static const _key = 'display.key';
  static const _seq = 'display.seq';

  @override
  Future<DisplaySetup?> loadSetup() async {
    try {
      final p = await SharedPreferences.getInstance();
      final room = cleanRoomName(p.getString(_room) ?? '');
      if (room.isEmpty) return null;
      final server = p.getString(_server);
      return DisplaySetup(
        room: room,
        mode: p.getString(_mode) == 'confirm'
            ? DefaultMode.confirm
            : DefaultMode.open,
        server: (server == null || server.trim().isEmpty) ? null : server,
      );
    } catch (_) {
      return null;
    }
  }

  @override
  Future<void> saveSetup(DisplaySetup s) async {
    try {
      final p = await SharedPreferences.getInstance();
      await p.setString(_room, s.room);
      await p.setString(_mode, s.mode.name);
      if (s.server == null) {
        await p.remove(_server);
      } else {
        await p.setString(_server, s.server!);
      }
    } catch (_) {
      // Not remembered: the wizard shows again.
    }
  }

  @override
  Future<Appearance> loadAppearance() async {
    try {
      final p = await SharedPreferences.getInstance();
      final v = p.getString(_look);
      return Appearance.values.firstWhere(
        (a) => a.name == v,
        orElse: () => Appearance.dark,
      );
    } catch (_) {
      return Appearance.dark;
    }
  }

  @override
  Future<void> saveAppearance(Appearance a) async {
    try {
      final p = await SharedPreferences.getInstance();
      await p.setString(_look, a.name);
    } catch (_) {
      // Not remembered.
    }
  }

  static DisplayKeyData? _cached;

  static Uint8List? _decode(String? text) {
    if (text == null) return null;
    try {
      final b = base64Url.decode(base64Url.normalize(text));
      return b.length == 32 ? Uint8List.fromList(b) : null;
    } catch (_) {
      return null;
    }
  }

  @override
  Future<DisplayKeyData> key(Uint8List Function() generate) async {
    final cached = _cached;
    if (cached != null) return cached;
    try {
      final p = await SharedPreferences.getInstance();
      final saved = _decode(p.getString(_key));
      if (saved != null) {
        return _cached = DisplayKeyData(
          saved,
          int.tryParse(p.getString(_seq) ?? '') ?? 0,
        );
      }
      final fresh = generate();
      await p.setString(_key, base64Url.encode(fresh));
      await p.remove(_seq);
      return _cached = DisplayKeyData(fresh, 0);
    } catch (_) {
      // Not kept: a new key per run (the screen then shows up as a new one; nothing breaks).
      return _cached = DisplayKeyData(generate(), 0);
    }
  }

  @override
  Future<void> saveSeq(int seq) async {
    final c = _cached;
    if (c != null) _cached = DisplayKeyData(c.secret, seq);
    try {
      final p = await SharedPreferences.getInstance();
      await p.setString(_seq, seq.toString());
    } catch (_) {
      // Not remembered: the next start uses the clock.
    }
  }

  @override
  Future<void> reset() async {
    _cached = null;
    try {
      final p = await SharedPreferences.getInstance();
      for (final k in [_room, _mode, _server, _look, _key, _seq]) {
        await p.remove(k);
      }
    } catch (_) {
      // Nothing to reset.
    }
  }
}

// --- connection view (spec 4, 4.2) -------------------------------------------------------

enum ConnectionView { online, connecting, offline }

enum OfflineCause { network, busy, outdated }

/// Port of `connectionView` of the web app.
ConnectionView connectionView({
  required bool hasCode,
  required bool errored,
  required bool failed,
  required Duration withoutCode,
}) {
  if (hasCode && !errored) return ConnectionView.online;
  if (errored || failed) {
    return withoutCode >= offlineAfterFailure
        ? ConnectionView.offline
        : ConnectionView.connecting;
  }
  return withoutCode >= offlineAfterSilence
      ? ConnectionView.offline
      : ConnectionView.connecting;
}

/// The body variant for an error message of the core (spec 4.2). The core reports server
/// codes inside its technical English message.
OfflineCause offlineCauseOf(String? message) {
  final m = message ?? '';
  if (_has(m, const ['rate_limited', 'banned', 'server_full', 'forbidden'])) {
    return OfflineCause.busy;
  }
  if (_has(m, const ['proto_mismatch', 'bad_hello'])) {
    return OfflineCause.outdated;
  }
  return OfflineCause.network;
}

bool _has(String m, List<String> codes) => codes.any(m.contains);

/// Seconds after the loss at which the first attempts are made: 1 s, 2 s, 3 s, then every
/// 4 s (decision 13.30).
Duration _attemptAt(int attempt) => attempt < 3
    ? Duration(seconds: attempt + 1)
    : Duration(seconds: 3 + 4 * (attempt - 2));

/// How long to wait before the next attempt. [attempt] is the number of attempts made since the
/// loss (0 = none yet), [sinceLoss] the time since the loss. After a busy refusal: 30 s after
/// the refusal; null when only a newer app helps (spec 4.2).
Duration? retryDelay({
  required int attempt,
  required Duration sinceLoss,
  required OfflineCause cause,
}) {
  if (cause == OfflineCause.outdated) return null;
  if (cause == OfflineCause.busy) return const Duration(seconds: 30);
  final left = _attemptAt(attempt) - sinceLoss;
  return left.isNegative ? Duration.zero : left;
}

// --- idle dimming and the controls of a presentation ------------------------------------

/// Dims after [delay] without activity while enabled (spec 8.6). [onChange] reports the state.
class IdleDim {
  IdleDim({required this.onChange, this.delay = idleDimAfter});
  final void Function(bool dimmed) onChange;
  final Duration delay;
  Timer? _timer;
  bool _enabled = false;
  bool _dimmed = false;

  bool get dimmed => _dimmed;

  /// Idle and nothing else going on = enabled. Leaving idle restores the brightness.
  void setEnabled(bool v) {
    if (v == _enabled) return;
    _enabled = v;
    if (v) {
      _arm();
    } else {
      _timer?.cancel();
      _set(false);
    }
  }

  /// Any input, join, request or mode change. Returns true when the screen was dimmed (the
  /// input then only restores the brightness, spec 8.6).
  bool activity() {
    if (!_enabled) return false;
    final was = _dimmed;
    _set(false);
    _arm();
    return was;
  }

  void _arm() {
    _timer?.cancel();
    _timer = Timer(delay, () => _set(true));
  }

  void _set(bool d) {
    if (d == _dimmed) return;
    _dimmed = d;
    onChange(d);
  }

  void dispose() => _timer?.cancel();
}

/// Name tag and End: shown by input, hidden [controlsHide] after the last one (spec 4.5).
class ControlsReveal {
  ControlsReveal({
    required this.onChange,
    this.now = DateTime.now,
    this.hideAfter = controlsHide,
  });
  final void Function(bool shown) onChange;
  final DateTime Function() now;
  final Duration hideAfter;
  Timer? _timer;
  DateTime _lastMouse = DateTime.fromMillisecondsSinceEpoch(0);
  bool _shown = false;

  bool get visible => _shown;

  /// Key, click, tap or touch. True when the controls were hidden before: the input only
  /// reveals (spec 8.2).
  bool poke() {
    final wasHidden = !_shown;
    _set(true);
    _timer?.cancel();
    _timer = Timer(hideAfter, () => _set(false));
    return wasHidden;
  }

  /// Mouse movement, throttled to one trigger per [mouseThrottle]. True when it revealed.
  bool mouseMove() {
    final t = now();
    if (t.difference(_lastMouse) < mouseThrottle) return false;
    _lastMouse = t;
    return poke();
  }

  void hideNow() {
    _timer?.cancel();
    _set(false);
  }

  void _set(bool v) {
    if (v == _shown) return;
    _shown = v;
    onChange(v);
  }

  void dispose() => _timer?.cancel();
}
