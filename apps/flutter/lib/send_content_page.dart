// SPDX-License-Identifier: GPL-3.0-or-later
// "Send file or link": pick an image (PNG, JPEG, WebP, GIF), a PDF or an Office file (.docx,
// .pptx, .xlsx; converted to a PDF on this computer first), or type a web link;
// connect with the receiver's room code, send with a progress bar, and for a PDF turn pages
// from here (previous / next / go to page / stop; arrow keys, Page Up, Page Down).
//
// All checks are done by Rust (file type and size, PDF, link rules); this screen only shows
// the result. The sender does not draw the page: the receiver does.

import 'package:file_selector/file_selector.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'package:cometcast/l10n/app_localizations.dart';

import 'content_sender_controller.dart';
import 'core/core_api.dart';
import 'send_page.dart' show UpperCaseFormatter;
import 'settings.dart';
import 'widgets.dart';

/// Lets the person choose a file and returns its path, or null if they cancelled. [typesLabel]
/// is the translated name of the type filter. Replaced in tests.
typedef PickFile = Future<String?> Function(String typesLabel);

/// The real file dialog (`file_selector`: GTK on Linux, native dialogs elsewhere).
/// UNCERTAIN: only the Linux plugin was built here; on Android `file_selector` hands out a
/// copy of the chosen file whose path was not checked.
Future<String?> pickImageOrPdf(String typesLabel) async {
  final file = await openFile(
    acceptedTypeGroups: [contentTypeGroup(typesLabel)],
  );
  return file?.path;
}

/// The file types of the dialog: images, PDF and the three Office types the Rust core
/// converts (`.docx .pptx .xlsx`). UNCERTAIN: the Office types are listed by extension, MIME
/// type and UTI; the GTK dialog filters by extension or MIME type, other platforms were not
/// tried.
XTypeGroup contentTypeGroup(String typesLabel) => XTypeGroup(
  label: typesLabel,
  extensions: const [
    'png',
    'jpg',
    'jpeg',
    'webp',
    'gif',
    'pdf',
    'docx',
    'pptx',
    'xlsx',
  ],
  mimeTypes: const [
    'image/png',
    'image/jpeg',
    'image/webp',
    'image/gif',
    'application/pdf',
    'application/vnd.openxmlformats-officedocument.wordprocessingml.document',
    'application/vnd.openxmlformats-officedocument.presentationml.presentation',
    'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
  ],
  uniformTypeIdentifiers: const [
    'public.image',
    'com.adobe.pdf',
    'org.openxmlformats.wordprocessingml.document',
    'org.openxmlformats.presentationml.presentation',
    'org.openxmlformats.spreadsheetml.sheet',
  ],
);

enum _Mode { file, link }

/// The translated text for a refusal found by the Rust core.
String sendRejectionText(AppLocalizations t, UiSendRejection r) => switch (r) {
  UiSendRejection.unreadable => t.sendRejectUnreadable,
  UiSendRejection.notARegularFile => t.sendRejectNotFile,
  UiSendRejection.empty => t.sendRejectEmpty,
  UiSendRejection.tooLarge => t.sendRejectTooLarge,
  UiSendRejection.unsupportedType => t.sendRejectType,
  UiSendRejection.notPdf => t.sendRejectNotPdf,
  UiSendRejection.documentUnusable => t.sendRejectDocument,
  UiSendRejection.badLink => t.sendContentLinkInvalid,
  UiSendRejection.officeUnavailable => t.sendRejectOfficeUnavailable,
  UiSendRejection.officeEncrypted => t.sendRejectOfficeEncrypted,
  UiSendRejection.officeFailed => t.sendRejectOfficeFailed,
};

class SendContentPage extends StatefulWidget {
  const SendContentPage({
    super.key,
    required this.core,
    required this.settings,
    this.pickFile = pickImageOrPdf,
  });

  final CoreApi core;
  final AppSettings settings;

  /// How a file is chosen (replaced in tests).
  final PickFile pickFile;

  @override
  State<SendContentPage> createState() => _SendContentPageState();
}

class _SendContentPageState extends State<SendContentPage> {
  late final ContentSenderController _controller = ContentSenderController(
    core: widget.core,
    udpBind: widget.settings.udpBind,
  );
  final _code = TextEditingController();
  final _link = TextEditingController();
  final _goto = TextEditingController();
  final _codeFocus = textFieldFocusNode(debugLabel: 'codeField');
  final _linkFocus = textFieldFocusNode(debugLabel: 'linkField');
  final _gotoFocus = textFieldFocusNode(debugLabel: 'gotoField');
  _Mode _mode = _Mode.file;
  String? _path;
  bool _triedToStart = false;
  bool _serverEmpty = false;
  bool _gotoInvalid = false;

  @override
  void initState() {
    super.initState();
    widget.settings.discover(widget.core);
  }

  @override
  void dispose() {
    widget.settings.cancelDiscovery();
    _controller.dispose();
    _codeFocus.dispose();
    _linkFocus.dispose();
    _gotoFocus.dispose();
    _code.dispose();
    _link.dispose();
    _goto.dispose();
    super.dispose();
  }

  String _fileName(String path) {
    final i = path.lastIndexOf(RegExp(r'[/\\]'));
    return i < 0 ? path : path.substring(i + 1);
  }

  Future<void> _pick() async {
    final label = AppLocalizations.of(context).sendContentPickerTypes;
    final path = await widget.pickFile(label);
    if (!mounted || path == null) return;
    setState(() => _path = path);
  }

  String? _codeError(AppLocalizations t) {
    final text = _code.text;
    if (text.trim().isEmpty && !_triedToStart) return null;
    final check = widget.core.checkRoomCode(text);
    return switch (check) {
      RoomCodeCheck_Valid() => null,
      RoomCodeCheck_InvalidChar(:final ch) => t.roomCodeInvalidChar(ch),
      RoomCodeCheck_WrongLength() =>
        (_triedToStart || text.trim().length > widget.core.roomCodeLength)
            ? t.roomCodeWrongLength(widget.core.roomCodeLength)
            : null,
    };
  }

  /// The Rust verdict on the typed link (never decided in Dart).
  LinkCheck get _linkCheck => widget.core.validateLink(_link.text);

  void _start() {
    setState(() {
      _triedToStart = true;
      _serverEmpty = widget.settings.serverAddress.trim().isEmpty;
    });
    final check = widget.core.checkRoomCode(_code.text);
    if (check is! RoomCodeCheck_Valid || _serverEmpty) return;
    final ContentSource source;
    if (_mode == _Mode.file) {
      final p = _path;
      if (p == null) return;
      source = ContentSource.file(path: p);
    } else {
      // The core validates again before it sends; this check only avoids a round trip.
      if (_linkCheck is! LinkCheck_Valid) return;
      source = ContentSource.link(url: _link.text);
    }
    widget.settings.rememberServer();
    _controller.start(
      code: check.code,
      server: widget.settings.serverAddress.trim(),
      source: source,
    );
  }

  void _gotoPage() {
    final n = int.tryParse(_goto.text.trim());
    setState(() => _gotoInvalid = n == null || n < 1);
    if (n == null || n < 1) return;
    _controller.goToPage(n);
  }

  @override
  Widget build(BuildContext context) {
    final t = AppLocalizations.of(context);
    return EscapeToGoBack(
      child: Scaffold(
        appBar: AppBar(title: Text(t.sendContentTitle)),
        body: ListenableBuilder(
          listenable: _controller,
          builder: (context, _) {
            final active = _controller.isActive;
            return PageBody(
              children: [
                ServerField(
                  settings: widget.settings,
                  enabled: !active,
                  textInputAction: TextInputAction.next,
                ),
                if (_serverEmpty)
                  Padding(
                    padding: const EdgeInsets.only(top: 8),
                    child: Text(
                      t.serverEmpty,
                      key: const Key('serverError'),
                      style: TextStyle(
                        color: Theme.of(context).colorScheme.error,
                      ),
                    ),
                  ),
                const SizedBox(height: 16),
                SegmentedButton<_Mode>(
                  key: const Key('modeSwitch'),
                  segments: [
                    ButtonSegment(
                      value: _Mode.file,
                      icon: const Icon(Icons.insert_drive_file_outlined),
                      label: Text(t.sendContentModeFile),
                    ),
                    ButtonSegment(
                      value: _Mode.link,
                      icon: const Icon(Icons.link),
                      label: Text(t.sendContentModeLink),
                    ),
                  ],
                  selected: {_mode},
                  onSelectionChanged: active
                      ? null
                      : (s) => setState(() => _mode = s.first),
                ),
                const SizedBox(height: 16),
                if (_mode == _Mode.file)
                  _buildFilePicker(t, active)
                else
                  _buildLinkField(t, active),
                const SizedBox(height: 16),
                TextField(
                  key: const Key('codeField'),
                  controller: _code,
                  focusNode: _codeFocus,
                  enabled: !active,
                  textCapitalization: TextCapitalization.characters,
                  textInputAction: TextInputAction.done,
                  autocorrect: false,
                  enableSuggestions: false,
                  inputFormatters: [
                    UpperCaseFormatter(),
                    LengthLimitingTextInputFormatter(32),
                  ],
                  style: Theme.of(context).textTheme.headlineSmall
                      ?.copyWith(fontFamily: 'monospace', letterSpacing: 4),
                  decoration: InputDecoration(
                    labelText: t.roomCodeInputLabel,
                    helperText: t.roomCodeHelper(
                      widget.core.roomCodeLength,
                      widget.core.roomCodeAlphabet,
                    ),
                    errorText: _codeError(t),
                    border: const OutlineInputBorder(),
                  ),
                  onChanged: (_) => setState(() {}),
                  onSubmitted: (_) => active ? null : _start(),
                ),
                const SizedBox(height: 16),
                Align(
                  alignment: Alignment.centerLeft,
                  child: active
                      ? OutlinedButton.icon(
                          key: const Key('stopButton'),
                          onPressed: _controller.stop,
                          icon: const Icon(Icons.stop),
                          label: Text(t.stop),
                        )
                      : FilledButton.icon(
                          key: const Key('startButton'),
                          onPressed: _start,
                          icon: const Icon(Icons.send),
                          label: Text(t.sendContentStart),
                        ),
                ),
                const SizedBox(height: 24),
                Semantics(
                  liveRegion: true,
                  child: Text(
                    _statusText(t),
                    key: const Key('statusText'),
                    style: Theme.of(context).textTheme.bodyLarge,
                  ),
                ),
                if (_controller.phase == ContentSendPhase.converting) ...[
                  const SizedBox(height: 12),
                  Semantics(
                    label: t.sendContentConvertingLabel,
                    child: const LinearProgressIndicator(
                      key: Key('convertProgress'),
                    ),
                  ),
                ],
                if (_controller.phase == ContentSendPhase.sending) ...[
                  const SizedBox(height: 12),
                  Semantics(
                    label: t.sendContentProgress,
                    value: '${_controller.percent} %',
                    child: LinearProgressIndicator(
                      key: const Key('sendProgress'),
                      value: _controller.total <= 0
                          ? null
                          : _controller.sent / _controller.total,
                    ),
                  ),
                ],
                if (_controller.phase == ContentSendPhase.ready) ...[
                  const SizedBox(height: 16),
                  _buildReady(t),
                ],
              ],
            );
          },
        ),
      ),
    );
  }

  Widget _buildFilePicker(AppLocalizations t, bool active) {
    final path = _path;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Align(
          alignment: Alignment.centerLeft,
          child: OutlinedButton.icon(
            key: const Key('pickFileButton'),
            onPressed: active ? null : _pick,
            icon: const Icon(Icons.folder_open),
            label: Text(t.sendContentPick),
          ),
        ),
        const SizedBox(height: 8),
        Text(
          path == null
              ? (_triedToStart ? t.sendContentNeedFile : t.sendContentNoFile)
              : t.sendContentChosen(_fileName(path)),
          key: const Key('chosenFile'),
          style: TextStyle(
            color: (path == null && _triedToStart)
                ? Theme.of(context).colorScheme.error
                : null,
          ),
        ),
      ],
    );
  }

  Widget _buildLinkField(AppLocalizations t, bool active) {
    final text = _link.text.trim();
    final check = text.isEmpty ? null : _linkCheck;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        TextField(
          key: const Key('linkField'),
          controller: _link,
          focusNode: _linkFocus,
          enabled: !active,
          keyboardType: TextInputType.url,
          textInputAction: TextInputAction.next,
          autocorrect: false,
          enableSuggestions: false,
          decoration: InputDecoration(
            labelText: t.sendContentLinkLabel,
            errorText: (_triedToStart && check is! LinkCheck_Valid)
                ? t.sendContentLinkInvalid
                : null,
            border: const OutlineInputBorder(),
          ),
          onChanged: (_) => setState(() {}),
        ),
        if (check is LinkCheck_Valid)
          Padding(
            padding: const EdgeInsets.only(top: 8),
            child: Text(
              t.sendContentLinkPreview(check.host),
              key: const Key('linkPreview'),
            ),
          ),
      ],
    );
  }

  /// After delivery: the presenter controls of a PDF, and "stop showing".
  Widget _buildReady(AppLocalizations t) {
    final c = _controller;
    final theme = Theme.of(context);
    final stopButton = OutlinedButton.icon(
      key: const Key('stopShowingButton'),
      onPressed: c.stopShowing,
      icon: const Icon(Icons.cancel_outlined),
      label: Text(t.sendContentStopShowing),
    );
    if (!c.canPresent) {
      return Align(alignment: Alignment.centerLeft, child: stopButton);
    }
    final pages = c.pages;
    return CallbackShortcuts(
      bindings: {
        const SingleActivator(LogicalKeyboardKey.arrowRight): c.nextPage,
        const SingleActivator(LogicalKeyboardKey.arrowDown): c.nextPage,
        const SingleActivator(LogicalKeyboardKey.pageDown): c.nextPage,
        const SingleActivator(LogicalKeyboardKey.arrowLeft): c.prevPage,
        const SingleActivator(LogicalKeyboardKey.arrowUp): c.prevPage,
        const SingleActivator(LogicalKeyboardKey.pageUp): c.prevPage,
      },
      child: Focus(
        // The keys work as soon as the PDF is delivered, without a click first.
        autofocus: true,
        child: Card(
          key: const Key('presenter'),
          child: Padding(
            padding: const EdgeInsets.all(16),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(t.sendPresenterTitle, style: theme.textTheme.titleMedium),
                const SizedBox(height: 8),
                Row(
                  children: [
                    IconButton.filledTonal(
                      key: const Key('prevPageButton'),
                      tooltip: t.sendPresenterPrev,
                      icon: const Icon(Icons.chevron_left),
                      onPressed: c.page > 1 ? c.prevPage : null,
                    ),
                    Expanded(
                      child: Semantics(
                        liveRegion: true,
                        child: Text(
                          pages == null
                              ? t.sendPresenterPageUnknown(c.page)
                              : t.contentPageIndicator(c.page, pages),
                          key: const Key('presenterPage'),
                          textAlign: TextAlign.center,
                          style: theme.textTheme.titleLarge,
                        ),
                      ),
                    ),
                    IconButton.filledTonal(
                      key: const Key('nextPageButton'),
                      tooltip: t.sendPresenterNext,
                      icon: const Icon(Icons.chevron_right),
                      onPressed: (pages == null || c.page < pages)
                          ? c.nextPage
                          : null,
                    ),
                  ],
                ),
                const SizedBox(height: 12),
                Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    SizedBox(
                      width: 160,
                      child: TextField(
                        key: const Key('gotoField'),
                        controller: _goto,
                        focusNode: _gotoFocus,
                        keyboardType: TextInputType.number,
                        textInputAction: TextInputAction.go,
                        inputFormatters: [
                          FilteringTextInputFormatter.digitsOnly,
                        ],
                        decoration: InputDecoration(
                          labelText: t.sendPresenterGoto,
                          errorText: _gotoInvalid
                              ? t.sendPresenterGotoInvalid
                              : null,
                          border: const OutlineInputBorder(),
                        ),
                        onSubmitted: (_) => _gotoPage(),
                      ),
                    ),
                    const SizedBox(width: 12),
                    Padding(
                      padding: const EdgeInsets.only(top: 8),
                      child: FilledButton(
                        key: const Key('gotoButton'),
                        onPressed: _gotoPage,
                        child: Text(t.sendPresenterGo),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 12),
                stopButton,
                if (c.commandFailed) ...[
                  const SizedBox(height: 8),
                  Text(
                    t.sendPresenterCmdFailed,
                    key: const Key('commandFailed'),
                    style: TextStyle(color: theme.colorScheme.error),
                  ),
                ],
                const SizedBox(height: 12),
                Text(t.sendPresenterKeys, style: theme.textTheme.bodySmall),
                Text(t.sendPresenterNote, style: theme.textTheme.bodySmall),
              ],
            ),
          ),
        ),
      ),
    );
  }

  String _statusText(AppLocalizations t) {
    final c = _controller;
    if (c.rejection != null) return sendRejectionText(t, c.rejection!);
    if (c.error != null) return t.statusError(c.error!);
    if (c.receiverLeft) return t.statusPeerLeftReceiver;
    final name = c.name ?? '';
    switch (c.phase) {
      case ContentSendPhase.idle:
        return t.statusIdle;
      case ContentSendPhase.preparing:
        return t.sendContentPreparing;
      case ContentSendPhase.converting:
        return t.sendContentConverting(name);
      case ContentSendPhase.connecting:
        if (c.connectionState != null) {
          return connectionStateText(t, c.connectionState);
        }
        return t.statusConnecting;
      case ContentSendPhase.sending:
        return t.sendContentSending(name, c.percent);
      case ContentSendPhase.ready:
        return c.kind == UiContentKind.link
            ? t.sendContentLinkDone(c.host ?? name)
            : t.sendContentDone(name);
      case ContentSendPhase.stopped:
        return t.sendContentStopped;
      case ContentSendPhase.ended:
        return t.statusEnded;
    }
  }
}
