// SPDX-License-Identifier: GPL-3.0-or-later
// Text with one styled value inside a translated sentence (the security code in `present.sas` and
// `presenting.sas`): the value is a placeholder in the string, so its place follows the language.

import 'package:flutter/painting.dart';

const _marker = '\u0001';

/// Splits [template] (made by calling the localized getter with [spanMarker]) around the
/// marker and puts [value] in [valueStyle]; the rest keeps the base style of the Text.rich.
List<InlineSpan> valueSpans(
  String template,
  String value,
  TextStyle valueStyle,
) {
  final i = template.indexOf(_marker);
  if (i < 0) return [TextSpan(text: template)];
  return [
    if (i > 0) TextSpan(text: template.substring(0, i)),
    TextSpan(text: value, style: valueStyle),
    if (i + 1 < template.length) TextSpan(text: template.substring(i + 1)),
  ];
}

/// The placeholder value to pass to the localized getter.
const spanMarker = _marker;
