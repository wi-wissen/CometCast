// SPDX-License-Identifier: GPL-3.0-or-later
import 'dart:typed_data';

import 'package:cometcast/core/core_api.dart';

/// A [w] x [h] picture of one colour, as the core would hand it out.
VideoFrame solidFrame(int w, int h, List<int> rgb, {int seq = 1}) {
  final px = Uint8List(w * h * 4);
  for (var i = 0; i < w * h; i++) {
    px.setRange(i * 4, i * 4 + 4, [...rgb, 255]);
  }
  return VideoFrame(seq: BigInt.from(seq), width: w, height: h, rgba: px);
}
