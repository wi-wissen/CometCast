// SPDX-License-Identifier: GPL-3.0-or-later
// The text field of the display wizard: it gets the focus ring like a button and opens the
// keyboard only on OK / Enter / tap, not when it gets focus; Up and Down leave it, Left and
// Right move the caret (spec 4.1, native-apps.md 8). Same ring as a button but no scale.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'kit.dart';

class DField extends StatefulWidget {
  const DField({
    super.key,
    required this.row,
    required this.col,
    required this.label,
    required this.controller,
    required this.onChanged,
    required this.onDone,
    this.maxLength,
    this.autofocus = false,
    this.fieldKey,
  });

  final int row;
  final int col;
  final String label;
  final TextEditingController controller;
  final ValueChanged<String> onChanged;

  /// The keyboard was closed with Done / Enter: the screen moves focus to the row's primary
  /// button (spec: wizard step 1 Next, step 3 Looks right).
  final VoidCallback onDone;
  final int? maxLength;
  final bool autofocus;
  final Key? fieldKey;

  @override
  State<DField> createState() => _DFieldState();
}

class _DFieldState extends State<DField> {
  late final RowNode _node = RowNode(
    row: widget.row,
    col: widget.col,
    debugLabel: 'field:${widget.row}',
    onKeyEvent: _onKey,
  );
  bool _editing = false;

  @override
  void dispose() {
    _node.dispose();
    super.dispose();
  }

  KeyEventResult _onKey(FocusNode node, KeyEvent e) {
    if (e is KeyUpEvent) return KeyEventResult.ignored;
    if (e is KeyDownEvent &&
        (DisplayScope.maybeOf(context)?.keyGate?.call(e) ?? false)) {
      return KeyEventResult.handled;
    }
    final k = e.logicalKey;
    // Up and Down never move the caret: they leave the field.
    final dir = k == LogicalKeyboardKey.arrowUp
        ? TraversalDirection.up
        : k == LogicalKeyboardKey.arrowDown
        ? TraversalDirection.down
        : null;
    if (dir != null) {
      _editing = false;
      if (mounted) setState(() {});
      node.focusInDirection(dir);
      return KeyEventResult.handled;
    }
    if (!_editing && isActivateKey(k) && k != LogicalKeyboardKey.space) {
      setState(() => _editing = true);
      return KeyEventResult.handled;
    }
    if (!_editing &&
        (k == LogicalKeyboardKey.arrowLeft ||
            k == LogicalKeyboardKey.arrowRight)) {
      // Not editing yet: the arrows are the screen's, the caret is not moved.
      return node.focusInDirection(
            k == LogicalKeyboardKey.arrowLeft
                ? TraversalDirection.left
                : TraversalDirection.right,
          )
          ? KeyEventResult.handled
          : KeyEventResult.handled;
    }
    return KeyEventResult.ignored;
  }

  @override
  Widget build(BuildContext context) {
    final s = DisplayScope.of(context);
    final t = s.tokens;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(widget.label, style: s.type.dots),
        const SizedBox(height: 6),
        DisplayControl(
          row: widget.row,
          col: widget.col,
          radius: 10,
          scale: false,
          externalFocus: true,
          focusNode: _node,
          autofocus: widget.autofocus,
          // A tap or click opens the keyboard at once.
          onPressed: () => setState(() => _editing = true),
          label: widget.label,
          child: Container(
            height: 50,
            padding: const EdgeInsets.symmetric(horizontal: 14),
            alignment: Alignment.centerLeft,
            decoration: BoxDecoration(
              color: t.bg,
              borderRadius: BorderRadius.circular(10),
              border: Border.all(width: 2, color: t.line),
            ),
            child: EditableText(
              key: widget.fieldKey,
              controller: widget.controller,
              focusNode: _node,
              readOnly: !_editing,
              showCursor: _editing,
              style: s.type.field,
              cursorColor: t.acc,
              backgroundCursorColor: t.line,
              maxLines: 1,
              inputFormatters: widget.maxLength == null
                  ? null
                  : [LengthLimitingTextInputFormatter(widget.maxLength)],
              keyboardType: TextInputType.text,
              textInputAction: TextInputAction.done,
              onChanged: widget.onChanged,
              onSubmitted: (_) {
                setState(() => _editing = false);
                widget.onDone();
              },
            ),
          ),
        ),
      ],
    );
  }
}
