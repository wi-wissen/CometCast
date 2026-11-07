// SPDX-License-Identifier: GPL-3.0-or-later
// The received Miracast picture, drawn by the display in the Presenting state (spec 15.4). It
// lives in the display code since the presenter's Miracast page is to be retired (fix list round
// 5 F9, N117); `miracast_page.dart` still uses it until then.

import 'dart:async';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';

import 'package:cometcast/l10n/app_localizations.dart';

import '../../core/miracast.dart';
import '../../miracast_controller.dart';

/// Turns a picture into a [ui.Image] (replaced in tests).
typedef FrameDecoder = Future<ui.Image> Function(MiracastFrame frame);

/// The real [FrameDecoder]: BGRA bytes to an image.
Future<ui.Image> decodeMiracastFrame(MiracastFrame f) {
  final c = Completer<ui.Image>();
  ui.decodeImageFromPixels(
    f.bgra,
    f.width,
    f.height,
    ui.PixelFormat.bgra8888,
    c.complete,
  );
  return c.future;
}

/// The received picture. Decodes each new picture of the controller and draws the newest.
class MiracastVideoView extends StatefulWidget {
  const MiracastVideoView({
    super.key,
    required this.controller,
    this.decoder = decodeMiracastFrame,
  });

  final MiracastController controller;
  final FrameDecoder decoder;

  @override
  State<MiracastVideoView> createState() => _MiracastVideoViewState();
}

class _MiracastVideoViewState extends State<MiracastVideoView> {
  ui.Image? _image;
  int _lastSeq = -1;
  bool _decoding = false;

  @override
  void initState() {
    super.initState();
    widget.controller.addListener(_onChange);
    _onChange();
  }

  @override
  void didUpdateWidget(MiracastVideoView old) {
    super.didUpdateWidget(old);
    if (old.controller != widget.controller) {
      old.controller.removeListener(_onChange);
      widget.controller.addListener(_onChange);
    }
  }

  void _onChange() {
    final f = widget.controller.frame;
    if (f == null) {
      // Not connected any more: never keep showing the last picture.
      if (_image != null || _lastSeq != -1) {
        _lastSeq = -1;
        if (mounted) {
          setState(() {
            _image?.dispose();
            _image = null;
          });
        }
      }
      return;
    }
    if (_decoding || f.seq == _lastSeq) return;
    _decoding = true;
    _lastSeq = f.seq;
    widget
        .decoder(f)
        .then(
          (img) {
            _decoding = false;
            if (!mounted) {
              img.dispose();
              return;
            }
            setState(() {
              final old = _image;
              _image = img;
              old?.dispose();
            });
            // A newer picture may have arrived while decoding.
            _onChange();
          },
          onError: (Object _) {
            _decoding = false;
          },
        );
  }

  @override
  void dispose() {
    widget.controller.removeListener(_onChange);
    _image?.dispose();
    _image = null;
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final t = AppLocalizations.of(context);
    final img = _image;
    final ratio = img == null || img.height == 0
        ? 16 / 9
        : img.width / img.height;
    return Semantics(
      label: t.miracastVideoLabel(widget.controller.connectedName ?? ''),
      image: true,
      child: AspectRatio(
        aspectRatio: ratio,
        child: ColoredBox(
          color: Colors.black,
          child: img == null
              ? const SizedBox.expand()
              : RawImage(image: img, fit: BoxFit.contain),
        ),
      ),
    );
  }
}
