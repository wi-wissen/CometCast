// SPDX-License-Identifier: GPL-3.0-or-later
// State of the "Send file or link" screen: check and send an image, a PDF or a link, then (for
// a PDF) the presenter commands. Like SenderController, but for the content channel.

import 'dart:async';

import 'package:flutter/foundation.dart';

import 'core/core_api.dart';

enum ContentSendPhase {
  /// Nothing started.
  idle,

  /// The file or link is being read and checked (by Rust).
  preparing,

  /// Joined the room, waiting for the connection to the receiver.
  connecting,

  /// The transfer runs (progress).
  sending,

  /// Delivered. The receiver shows it while this stays connected; page commands are possible.
  ready,

  /// The stop command was delivered and the sender ended.
  stopped,

  /// Ended: the receiver left, the content was refused, or something failed.
  ended,
}

class ContentSenderController extends ChangeNotifier {
  ContentSenderController({required this.core, this.udpBind});

  final CoreApi core;
  final String? udpBind;

  StreamSubscription<ContentSenderUiEvent>? _sub;
  bool _disposed = false;

  ContentSendPhase phase = ContentSendPhase.idle;
  UiConnectionState? connectionState;

  /// What is sent (known after the check).
  UiContentKind? kind;
  String? name;
  int size = 0;

  /// Pages of a PDF if the Rust core could count them; null = unknown.
  int? pages;

  /// Host of a link (punycode form).
  String? host;

  int sent = 0;
  int total = 0;

  /// Why the content was refused before connecting (translated by the screen).
  UiSendRejection? rejection;

  /// Technical error text in English (shown after a translated "Error:" prefix).
  String? error;
  bool receiverLeft = false;

  /// The last page command was not accepted by the core.
  bool commandFailed = false;

  /// The page that was asked for last (starts at 1). The sender does not draw the page and
  /// does not hear back from the receiver, so this is the requested page, not a report.
  int page = 1;

  bool get isActive =>
      phase == ContentSendPhase.preparing ||
      phase == ContentSendPhase.connecting ||
      phase == ContentSendPhase.sending ||
      phase == ContentSendPhase.ready;

  /// Page controls are available.
  bool get canPresent =>
      phase == ContentSendPhase.ready && kind == UiContentKind.pdf;

  /// Percent of the transfer, 0 to 100.
  int get percent => total <= 0 ? 0 : (sent * 100 ~/ total).clamp(0, 100);

  void start({
    required String code,
    required String server,
    required ContentSource source,
  }) {
    _cancel();
    phase = ContentSendPhase.preparing;
    connectionState = null;
    kind = null;
    name = null;
    size = 0;
    pages = null;
    host = null;
    sent = 0;
    total = 0;
    rejection = null;
    error = null;
    receiverLeft = false;
    commandFailed = false;
    page = 1;
    notifyListeners();
    _sub = core
        .startContentSender(
          code: code,
          server: server.trim(),
          udpBind: udpBind,
          source: source,
        )
        .listen(
          _onEvent,
          onError: (Object e) {
            error = e.toString();
            phase = ContentSendPhase.ended;
            notifyListeners();
          },
          onDone: () {
            if (isActive) {
              phase = ContentSendPhase.ended;
              notifyListeners();
            }
          },
        );
  }

  void _onEvent(ContentSenderUiEvent e) {
    switch (e) {
      case ContentSenderUiEvent_Prepared(
        :final name,
        :final kind,
        :final size,
        :final pages,
        :final host,
      ):
        this.name = displayName(name);
        this.kind = kind;
        this.size = size.toInt();
        this.pages = pages;
        this.host = host;
        phase = ContentSendPhase.connecting;
      case ContentSenderUiEvent_Joined():
        if (phase == ContentSendPhase.preparing) {
          phase = ContentSendPhase.connecting;
        }
      case ContentSenderUiEvent_ConnectionState(:final state):
        connectionState = state;
      case ContentSenderUiEvent_Connected():
        phase = ContentSendPhase.sending;
        sent = 0;
        total = size;
      case ContentSenderUiEvent_Progress(:final sent, :final total):
        this.sent = sent.toInt();
        this.total = total.toInt();
      case ContentSenderUiEvent_Sent():
        sent = total;
        phase = ContentSendPhase.ready;
      case ContentSenderUiEvent_Stopped():
        phase = ContentSendPhase.stopped;
      case ContentSenderUiEvent_PeerLeft():
        receiverLeft = true;
        phase = ContentSendPhase.ended;
      case ContentSenderUiEvent_Rejected(:final reason):
        rejection = reason;
        phase = ContentSendPhase.ended;
      case ContentSenderUiEvent_Error(:final message):
        error = message;
        phase = ContentSendPhase.ended;
    }
    notifyListeners();
  }

  // --- presenter commands ------------------------------------------------------------------

  bool _send(UiContentCmd cmd) {
    if (phase != ContentSendPhase.ready) return false;
    final ok = core.sendContentCmd(cmd);
    commandFailed = !ok;
    return ok;
  }

  /// Next page. At the last page (when the count is known) nothing is sent.
  bool nextPage() {
    if (!canPresent) return false;
    final last = pages;
    if (last != null && page >= last) return false;
    final ok = _send(const UiContentCmd.nextPage());
    if (ok) page++;
    notifyListeners();
    return ok;
  }

  bool prevPage() {
    if (!canPresent || page <= 1) return false;
    final ok = _send(const UiContentCmd.prevPage());
    if (ok) page--;
    notifyListeners();
    return ok;
  }

  /// Go to [target] (1-based). A page past the end is clamped when the count is known; the
  /// receiver clamps in any case.
  bool goToPage(int target) {
    if (!canPresent || target < 1) return false;
    final last = pages;
    final p = (last != null && target > last) ? last : target;
    final ok = _send(UiContentCmd.goToPage(page: p));
    if (ok) page = p;
    notifyListeners();
    return ok;
  }

  /// Tell the receiver to close the content. The sender ends after the command left (a
  /// `stopped` event arrives).
  bool stopShowing() {
    if (phase != ContentSendPhase.ready) return false;
    final ok = _send(const UiContentCmd.stop());
    notifyListeners();
    return ok;
  }

  /// Stop sending and disconnect (the receiver then drops what it shows).
  Future<void> stop() async {
    _cancel();
    phase = ContentSendPhase.idle;
    notifyListeners();
    await core.stopSender();
  }

  void _cancel() {
    _sub?.cancel();
    _sub = null;
  }

  @override
  void dispose() {
    _disposed = true;
    _cancel();
    unawaited(core.stopSender());
    super.dispose();
  }

  @override
  void notifyListeners() {
    if (_disposed) return;
    super.notifyListeners();
  }
}

/// A file name made safe to show in the status line: control and bidirectional-override
/// characters removed, at most [maxLength] characters (then "..."). Applies to the name in
/// `Prepared`, which comes from the file system, not from the receiver.
String displayName(String name, {int maxLength = 80}) {
  final kept = name.runes.where(
    (r) =>
        r >= 0x20 &&
        !(r >= 0x7f && r <= 0x9f) &&
        r != 0x200e &&
        r != 0x200f &&
        !(r >= 0x202a && r <= 0x202e) &&
        !(r >= 0x2066 && r <= 0x2069),
  );
  final list = kept.toList();
  if (list.length <= maxLength) return String.fromCharCodes(list);
  return '${String.fromCharCodes(list.take(maxLength - 3))}...';
}
