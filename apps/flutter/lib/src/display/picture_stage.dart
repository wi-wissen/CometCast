// SPDX-License-Identifier: GPL-3.0-or-later
// The picture while presenting: full bleed on black, letterboxed in its own aspect ratio, no
// chrome (spec 4.5). Live video (software image or hardware texture), a received image or a PDF
// page. Everything else (name tag, End, toasts) is drawn on the canvas by the display root.

import 'dart:math' as math;

import 'package:flutter/material.dart';

import '../../receiver_controller.dart';
import '../design/tokens.dart';

class PictureStage extends StatelessWidget {
  const PictureStage({super.key, required this.controller});

  final ReceiverController controller;

  @override
  Widget build(BuildContext context) {
    final c = controller;
    final shown = c.shown;
    final Widget child;
    if (shown is ReceivedImage) {
      child = _Image(image: shown);
    } else if (shown is ReceivedDocument) {
      child = _Page(doc: shown, controller: c);
    } else if (c.textureId != null && c.hasLiveVideo) {
      final (w, h) = c.pictureSize;
      child = Center(
        child: AspectRatio(
          aspectRatio: w / h,
          child: Texture(key: const Key('videoTexture'), textureId: c.textureId!),
        ),
      );
    } else if (c.image != null) {
      child = LayoutBuilder(
        builder: (context, box) {
          final dpr = MediaQuery.devicePixelRatioOf(context);
          final w = (box.maxWidth * dpr).round();
          final h = (box.maxHeight * dpr).round();
          WidgetsBinding.instance.addPostFrameCallback(
            (_) => c.reportViewSize(w, h),
          );
          final image = c.image!;
          return Center(
            child: AspectRatio(
              aspectRatio: image.width / image.height,
              child: RepaintBoundary(
                key: const Key('videoView'),
                child: RawImage(
                  image: image,
                  fit: BoxFit.fill,
                  filterQuality: FilterQuality.low,
                ),
              ),
            ),
          );
        },
      );
    } else {
      child = const SizedBox.shrink();
    }
    return ColoredBox(color: DisplayTokens.stage, child: child);
  }
}

class _Image extends StatelessWidget {
  const _Image({required this.image});
  final ReceivedImage image;

  /// Larger pictures are decoded at this size at most (memory).
  static const _maxDecodeSide = 4096;

  @override
  Widget build(BuildContext context) {
    final big = math.max(image.width, image.height) > _maxDecodeSide;
    final wide = image.width >= image.height;
    return Center(
      child: Image.memory(
        image.bytes,
        key: const Key('contentImage'),
        fit: BoxFit.contain,
        cacheWidth: big && wide ? _maxDecodeSide : null,
        cacheHeight: big && !wide ? _maxDecodeSide : null,
        filterQuality: FilterQuality.medium,
        errorBuilder: (context, error, stack) => const SizedBox.shrink(),
      ),
    );
  }
}

/// A PDF page drawn by the core at the width of this view, on black (decision 13.41).
class _Page extends StatelessWidget {
  const _Page({required this.doc, required this.controller});
  final ReceivedDocument doc;
  final ReceiverController controller;

  @override
  Widget build(BuildContext context) {
    final img = doc.image;
    return LayoutBuilder(
      builder: (context, box) {
        final dpr = MediaQuery.devicePixelRatioOf(context);
        final w = (box.maxWidth * dpr).round();
        WidgetsBinding.instance.addPostFrameCallback(
          (_) => controller.reportContentWidth(w),
        );
        if (img == null) return const SizedBox.expand();
        return Center(
          child: AspectRatio(
            aspectRatio: img.width / img.height,
            child: RepaintBoundary(
              key: const Key('pageView'),
              child: RawImage(
                image: img,
                fit: BoxFit.fill,
                filterQuality: FilterQuality.medium,
              ),
            ),
          ),
        );
      },
    );
  }
}
