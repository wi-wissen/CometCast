// SPDX-License-Identifier: GPL-3.0-or-later
// The file dialog of the presenter's file page and the texts for files the core refuses.

import 'package:file_selector/file_selector.dart';

import 'package:cometcast/l10n/app_localizations.dart';

import '../../core/core_api.dart';

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
