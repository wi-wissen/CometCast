// SPDX-License-Identifier: GPL-3.0-or-later
// The line under the code field, "via {dom} . Change", and the inline address field it opens
// (spec 15.7, fix list F5). The checking and the choice of the server are done by the caller
// ([onUse]); this widget has only the states: line, field, checking, error.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'package:cometcast/l10n/app_localizations.dart';

import 'host_text.dart';
import 'kit.dart';
import 'nearby.dart' show PCompactButton;

/// What the caller says about a typed address.
enum AddressVerdict { ok, notCometCast }

typedef UseAddress = Future<AddressVerdict> Function(String input);

class AddressLine extends StatefulWidget {
  const AddressLine({
    super.key,
    required this.dom,
    required this.policy,
    required this.onUse,
    this.locked = false,
  });

  /// The join address the app will use (`{dom}`).
  final String dom;

  /// The administrator's policy fixes the server: no link (`start.viaPolicy`).
  final bool policy;

  /// A nearby row is connecting: the link does nothing (N54).
  final bool locked;

  final UseAddress onUse;

  @override
  State<AddressLine> createState() => _AddressLineState();
}

enum _Mode { line, field, checking }

class _AddressLineState extends State<AddressLine> {
  _Mode _mode = _Mode.line;
  bool _error = false;
  final _controller = TextEditingController();

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  void _cancel() => setState(() {
    _mode = _Mode.line;
    _error = false;
    _controller.clear();
  });

  Future<void> _use() async {
    if (_mode == _Mode.checking) return;
    final input = _controller.text.trim();
    setState(() {
      _mode = _Mode.checking;
      _error = false;
    });
    AddressVerdict v;
    try {
      v = await widget.onUse(input);
    } catch (_) {
      v = AddressVerdict.notCometCast;
    }
    if (!mounted) return;
    if (v == AddressVerdict.ok) {
      setState(() {
        _mode = _Mode.line;
        _controller.clear();
      });
    } else {
      setState(() {
        _mode = _Mode.field;
        _error = true;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final s = PresenterScope.of(context);
    final t = AppLocalizations.of(context);
    final via = t.startVia(widget.dom);
    if (_mode == _Mode.line) {
      // A box 44 high that starts at the code field's bottom edge, the text centred in it, in
      // the link and in the policy variant alike (spec 15.7, N99).
      return Container(
        key: const Key('address-line-box'),
        constraints: const BoxConstraints(minHeight: 44),
        alignment: Alignment.centerLeft,
        child: Wrap(
          key: const Key('address-line'),
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            HostText(via, host: widget.dom, style: s.type.small),
            Text(' · ', style: s.type.small),
            if (widget.policy)
              Text(
                t.startViaPolicy,
                key: const Key('address-policy'),
                style: s.type.small,
              )
            else
              PTextLink(
                key: const Key('address-change'),
                label: t.startViaChange,
                bold: true,
                size: 13,
                left: true,
                onPressed: widget.locked
                    ? () {}
                    : () => setState(() {
                        _mode = _Mode.field;
                        _error = false;
                      }),
              ),
          ],
        ),
      );
    }
    final checking = _mode == _Mode.checking;
    final buttons = <Widget>[
      PCompactButton(
        key: const Key('address-use'),
        label: checking ? t.startAddrChecking : t.startAddrUse,
        centered: true,
        onPressed: checking ? null : _use,
      ),
      PCompactButton(
        key: const Key('address-cancel'),
        label: t.commonCancel,
        centered: true,
        onPressed: _cancel,
      ),
    ];
    final field = PField(
      fieldKey: const Key('address-field'),
      label: t.startAddrLabel,
      controller: _controller,
      hint: widget.dom,
      keyboardType: TextInputType.url,
      error: _error,
      errorText: _error ? t.startAddrError : null,
      textInputAction: TextInputAction.go,
      readOnly: checking,
      autofocus: true,
      onSubmitted: (_) => _use(),
    );
    Widget block(bool sideBySide) => Column(
      key: const Key('address-field-block'),
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (sideBySide)
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(child: field),
              const SizedBox(width: 8),
              Padding(
                padding: const EdgeInsets.only(top: 21),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [buttons[0], const SizedBox(width: 8), buttons[1]],
                ),
              ),
            ],
          )
        else ...[
          field,
          const SizedBox(height: 8),
          Wrap(spacing: 8, runSpacing: 8, children: buttons),
        ],
      ],
    );
    // Laptop: on the field's right in one row if it fits, otherwise below (spec 15.7). The phone
    // page measures its height (intrinsics), so a layout builder is only used on the laptop.
    // The opened state replaces the box; its label stands 14 below the code field as for any
    // field (N99).
    return Padding(
      padding: const EdgeInsets.only(top: 14),
      child: CallbackShortcuts(
        bindings: {const SingleActivator(LogicalKeyboardKey.escape): _cancel},
        child: s.laptop
            ? LayoutBuilder(
                builder: (context, box) => block(box.maxWidth >= 480),
              )
            : block(false),
      ),
    );
  }
}
