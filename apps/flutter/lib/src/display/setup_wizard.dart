// SPDX-License-Identifier: GPL-3.0-or-later
// First-run setup of the display (spec 4.1): welcome, room name, who may connect, address,
// done. Logic: display_logic.dart; this file draws it and moves the focus.

import 'package:flutter/material.dart';

import 'package:cometcast/l10n/app_localizations.dart';

import '../design/icons.dart';
import 'display_logic.dart';
import 'fields.dart';
import 'kit.dart';

/// Where an address in step 3 comes from (the source lines of spec 11.3).
enum AddressSource { defaultServer, own, policy, box, typed }

class AddressOption {
  const AddressOption(this.address, this.source);
  final String address;
  final AddressSource source;
}

/// The error lines under the address field (copy `relay.errors.*`).
enum AddressError { empty, invalid, insecure, credentials, path }

/// Checks an address typed into the field. Returns null when it is acceptable.
AddressError? checkTypedAddress(String raw) {
  final v = raw.trim();
  if (v.isEmpty) return AddressError.empty;
  if (RegExp(r'\s').hasMatch(v)) return AddressError.invalid;
  if (v.toLowerCase().startsWith('ws://')) return AddressError.insecure;
  final rest = v.replaceFirst(RegExp(r'^[a-zA-Z]+://'), '');
  if (rest.contains('@')) return AddressError.credentials;
  if (rest.contains('/')) return AddressError.path;
  return null;
}

class SetupController extends ChangeNotifier {
  SetupController({
    required this.suggestions,
    required this.options,
    required this.onFinish,
    required this.changeAllowed,
  }) : _wizard = initialWizard(suggestions.first),
       _current = options.isEmpty ? null : 0 {
    _name.text = '';
  }

  /// The five room name suggestions in the current language (`setup.name.s1` to `s5`).
  final List<String> suggestions;

  /// The addresses that were found (the first is the one shown at first). May be empty.
  List<AddressOption> options;

  /// Discovery finished later: replace the list (the shown address changes only if it was
  /// not typed and the person did not move on yet).
  void updateOptions(List<AddressOption> o) {
    options = o;
    if (_typed == null && o.isNotEmpty) {
      _current = (_current ?? 0).clamp(0, o.length - 1);
    }
    notifyListeners();
  }

  /// A policy sets the address: "Other address" is not offered.
  final bool changeAllowed;
  final void Function(DisplaySetup setup) onFinish;

  WizardState _wizard;
  WizardState get wizard => _wizard;

  final TextEditingController _name = TextEditingController();
  TextEditingController get nameController => _name;
  final TextEditingController _address = TextEditingController();
  TextEditingController get addressController => _address;

  int? _current;
  AddressOption? _typed;
  bool showField = false;
  AddressError? fieldError;

  AddressOption? get address =>
      _typed ?? (_current == null ? null : options[_current!]);

  void _set(WizardState s) {
    _wizard = s;
    notifyListeners();
  }

  void next() => _set(wizardNext(_wizard));
  void back() {
    if (showField) {
      closeField();
    } else {
      _set(wizardBack(_wizard));
    }
  }

  void chip(String s) {
    _name.text = '';
    _set(wizardChip(_wizard, s));
  }

  void custom(String v) => _set(wizardCustom(_wizard, v, suggestions.first));

  void mode(DefaultMode m) => _set(wizardMode(_wizard, m));

  /// "Other address": cycles the found ones, then opens the field. With one option (or none)
  /// it opens the field at once.
  void otherAddress() {
    final i = _typed != null ? -1 : (_current ?? -1);
    if (i >= 0 && i < options.length - 1) {
      _current = i + 1;
      notifyListeners();
    } else {
      showField = true;
      fieldError = null;
      notifyListeners();
    }
  }

  /// Confirm the typed address (Looks right or Enter in the field).
  bool applyField() {
    final err = checkTypedAddress(_address.text);
    fieldError = err;
    if (err != null) {
      notifyListeners();
      return false;
    }
    _typed = AddressOption(_address.text.trim(), AddressSource.typed);
    showField = false;
    _address.text = '';
    notifyListeners();
    return true;
  }

  void closeField() {
    showField = false;
    fieldError = null;
    notifyListeners();
  }

  void finish() {
    final a = address;
    onFinish(
      DisplaySetup(
        room: _wizard.room.isEmpty ? suggestions.first : _wizard.room,
        mode: _wizard.mode,
        // Only an address the wizard produced is kept.
        server: a?.address,
      ),
    );
  }

  @override
  void dispose() {
    _name.dispose();
    _address.dispose();
    super.dispose();
  }
}

class SetupWizardView extends StatefulWidget {
  const SetupWizardView({
    super.key,
    required this.controller,
    required this.remote,
  });

  final SetupController controller;

  /// A TV remote: `setup.done.bodyRemote` (else `bodyPointer`).
  final bool remote;

  @override
  State<SetupWizardView> createState() => _SetupWizardViewState();
}

class _SetupWizardViewState extends State<SetupWizardView> {
  SetupController get c => widget.controller;
  final _nextNode = RowNode(row: 2, col: 1, debugLabel: 'wizard:next');
  final _okNode = RowNode(row: 1, col: 2, debugLabel: 'wizard:ok');

  @override
  void dispose() {
    _nextNode.dispose();
    _okNode.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: c,
      builder: (context, _) {
        final s = DisplayScope.of(context);
        final t = AppLocalizations.of(context);
        final w = c.wizard;
        final card = DCard(
          width: 780,
          children: [_top(s, t, w.step), ..._body(context, s, t, w)],
        );
        // Step 0 is centred vertically; steps 1 to 4 have their top edge at 40 so that the logo
        // and the dots do not jump (decision 13.46).
        return Align(
          alignment: w.step == 0 ? Alignment.center : Alignment.topCenter,
          child: Padding(
            padding: EdgeInsets.only(top: w.step == 0 ? 0 : 40),
            // Keyed by step: initial focus follows the step.
            child: KeyedSubtree(key: ValueKey('step${w.step}'), child: card),
          ),
        );
      },
    );
  }

  Widget _top(DisplayScope s, AppLocalizations t, int step) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.spaceBetween,
      children: [
        const DLogo(size: 40),
        if (step > 0)
          Row(
            children: [
              for (var n = 1; n <= 4; n++) ...[
                Container(
                  width: 10,
                  height: 10,
                  decoration: BoxDecoration(
                    shape: BoxShape.circle,
                    color: n <= step ? s.tokens.acc : s.tokens.line,
                  ),
                ),
                const SizedBox(width: 8),
              ],
              Text(t.commonStepOf('$step'), style: s.type.dots),
            ],
          ),
      ],
    );
  }

  Text _h(DisplayScope s, String text) => Text(text, style: s.type.heading);
  Text _p(DisplayScope s, String text) => Text(text, style: s.type.body);

  List<Widget> _body(
    BuildContext context,
    DisplayScope s,
    AppLocalizations t,
    WizardState w,
  ) {
    switch (w.step) {
      case 0:
        return [
          _h(s, t.setupWelcomeTitle),
          _p(s, t.setupWelcomeBody),
          Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: [
              DButton(
                key: const Key('wz-start'),
                label: t.setupWelcomeStart,
                kind: DButtonKind.primary,
                row: 0,
                col: 0,
                autofocus: true,
                onPressed: c.next,
              ),
            ],
          ),
        ];
      case 1:
        return [
          _h(s, t.setupNameTitle),
          _p(s, t.setupNameBody),
          Wrap(
            spacing: 10,
            runSpacing: 10,
            children: [
              for (var i = 0; i < c.suggestions.length; i++)
                _Chip(
                  key: Key('wz-chip-$i'),
                  label: c.suggestions[i],
                  selected: w.chipSelected(c.suggestions[i]),
                  row: 0,
                  col: i,
                  onPressed: () => c.chip(c.suggestions[i]),
                ),
            ],
          ),
          DField(
            fieldKey: const Key('wz-room'),
            row: 1,
            col: 0,
            label: t.setupNameCustom,
            controller: c.nameController,
            maxLength: roomNameMax,
            onChanged: c.custom,
            onDone: () => _nextNode.requestFocus(),
          ),
          _actions([
            DButton(
              key: const Key('wz-back'),
              label: t.commonBack,
              row: 2,
              col: 0,
              onPressed: c.back,
            ),
            DButton(
              key: const Key('wz-next'),
              label: t.commonNext,
              kind: DButtonKind.primary,
              row: 2,
              col: 1,
              autofocus: true,
              focusNode: _nextNode,
              onPressed: c.next,
            ),
          ]),
        ];
      case 2:
        return [
          _h(s, t.setupModeTitle),
          IntrinsicHeight(
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Expanded(
                  child: _ModeCard(
                    key: const Key('wz-mode-open'),
                    eyebrow: t.setupModeFewPeople,
                    icon: CcIconName.open,
                    title: t.modeOpen,
                    text: t.setupModeOpenText,
                    selected: w.mode == DefaultMode.open,
                    col: 0,
                    onPressed: () => c.mode(DefaultMode.open),
                  ),
                ),
                const SizedBox(width: 14),
                Expanded(
                  child: _ModeCard(
                    key: const Key('wz-mode-confirm'),
                    eyebrow: t.setupModeManyPeople,
                    icon: CcIconName.approval,
                    title: t.modeApproval,
                    text: t.setupModeApprovalText,
                    selected: w.mode == DefaultMode.confirm,
                    col: 1,
                    onPressed: () => c.mode(DefaultMode.confirm),
                  ),
                ),
              ],
            ),
          ),
          _p(s, t.setupModeLater),
          _actions([
            DButton(
              key: const Key('wz-back'),
              label: t.commonBack,
              row: 1,
              col: 0,
              onPressed: c.back,
            ),
            DButton(
              key: const Key('wz-next'),
              label: t.commonNext,
              kind: DButtonKind.primary,
              row: 1,
              col: 1,
              onPressed: c.next,
            ),
          ]),
        ];
      case 3:
        final a = c.address;
        return [
          _h(s, t.setupAddressTitle),
          if (c.showField) ...[
            DField(
              fieldKey: const Key('wz-addr'),
              row: 0,
              col: 0,
              autofocus: true,
              label: t.setupAddressFieldLabel,
              controller: c.addressController,
              onChanged: (_) {},
              onDone: () {
                if (c.applyField()) {
                  _otherNode.requestFocus();
                } else {
                  _okNode.requestFocus();
                }
              },
            ),
            if (c.fieldError != null)
              Row(
                children: [
                  CcIcon(CcIconName.info, color: s.tokens.acc, size: 20),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      _errorText(t, c.fieldError!),
                      key: const Key('wz-addr-error'),
                      style: s.type.cardText.copyWith(color: s.tokens.fg),
                    ),
                  ),
                ],
              ),
          ] else
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 18, vertical: 16),
              decoration: BoxDecoration(
                color: s.tokens.panel2,
                borderRadius: BorderRadius.circular(16),
              ),
              child: Row(
                children: [
                  CcIcon(CcIconName.globe, color: s.tokens.acc, size: 34),
                  const SizedBox(width: 16),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          a == null ? '' : displayHost(a.address),
                          key: const Key('wz-domain'),
                          style: s.type.cardAddress,
                        ),
                        Text(
                          a == null ? '' : _sourceText(t, a.source),
                          key: const Key('wz-source'),
                          style: s.type.choiceText.copyWith(height: null),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
          _p(s, t.setupAddressBody),
          _actions([
            DButton(
              key: const Key('wz-back'),
              label: t.commonBack,
              row: 1,
              col: 0,
              onPressed: c.back,
            ),
            if (!c.showField && c.changeAllowed)
              DButton(
                key: const Key('wz-change'),
                label: t.setupAddressChange,
                row: 1,
                col: 1,
                focusNode: _otherNode,
                onPressed: c.otherAddress,
              ),
            DButton(
              key: const Key('wz-ok'),
              label: t.setupAddressOk,
              kind: DButtonKind.primary,
              row: 1,
              col: c.showField || !c.changeAllowed ? 1 : 2,
              autofocus: !c.showField,
              focusNode: _okNode,
              onPressed: c.showField ? () => c.applyField() : c.next,
            ),
          ]),
        ];
      default:
        return [
          _h(s, t.setupDoneTitle),
          _p(s, widget.remote ? t.setupDoneBodyRemote : t.setupDoneBodyPointer),
          Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: [
              DButton(
                key: const Key('wz-finish'),
                label: t.setupDoneFinish,
                kind: DButtonKind.primary,
                row: 0,
                col: 0,
                autofocus: true,
                onPressed: c.finish,
              ),
            ],
          ),
        ];
    }
  }

  final _otherNode = RowNode(row: 1, col: 1, debugLabel: 'wizard:other');

  Widget _actions(List<Widget> buttons, {bool left = false}) => Row(
    mainAxisAlignment: left ? MainAxisAlignment.start : MainAxisAlignment.end,
    children: [
      for (var i = 0; i < buttons.length; i++) ...[
        if (i > 0) const SizedBox(width: 12),
        buttons[i],
      ],
    ],
  );

  String _sourceText(AppLocalizations t, AddressSource s) => switch (s) {
    AddressSource.defaultServer => t.setupAddressSourceDefault,
    AddressSource.own => t.setupAddressSourceOwn,
    AddressSource.policy => t.setupAddressSourcePolicy,
    AddressSource.box => t.setupAddressSourceBox,
    AddressSource.typed => t.setupAddressSourceTyped,
  };

  String _errorText(AppLocalizations t, AddressError e) => switch (e) {
    AddressError.empty => t.relayErrorsEmpty,
    AddressError.invalid => t.relayErrorsInvalid,
    AddressError.insecure => t.relayErrorsInsecure,
    AddressError.credentials => t.relayErrorsCredentials,
    AddressError.path => t.relayErrorsPath,
  };
}

/// Room name chip: min height 46, padding 0 18, pill, border 2, fill `--panel2`; selected:
/// border `--acc` and bold.
class _Chip extends StatelessWidget {
  const _Chip({
    super.key,
    required this.label,
    required this.selected,
    required this.row,
    required this.col,
    required this.onPressed,
  });
  final String label;
  final bool selected;
  final int row;
  final int col;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final s = DisplayScope.of(context);
    final t = s.tokens;
    return DisplayControl(
      row: row,
      col: col,
      radius: 999,
      onPressed: onPressed,
      selected: selected,
      label: label,
      child: Container(
        constraints: const BoxConstraints(minHeight: 46),
        padding: const EdgeInsets.symmetric(horizontal: 18),
        decoration: BoxDecoration(
          color: t.panel2,
          borderRadius: BorderRadius.circular(999),
          border: Border.all(width: 2, color: selected ? t.acc : t.line),
        ),
        child: Center(
          widthFactor: 1,
          heightFactor: 1,
          child: Text(
            label,
            style: s.type.chip.copyWith(
              fontWeight: selected ? FontWeight.w700 : FontWeight.w400,
            ),
          ),
        ),
      ),
    );
  }
}

/// Choice card of step 2: border 2, radius 16, padding 16, min height 120.
class _ModeCard extends StatelessWidget {
  const _ModeCard({
    super.key,
    required this.eyebrow,
    required this.icon,
    required this.title,
    required this.text,
    required this.selected,
    required this.col,
    required this.onPressed,
  });
  final String eyebrow;
  final CcIconName icon;
  final String title;
  final String text;
  final bool selected;
  final int col;
  final VoidCallback onPressed;

  @override
  Widget build(BuildContext context) {
    final s = DisplayScope.of(context);
    final t = s.tokens;
    return DisplayControl(
      row: 0,
      col: col,
      radius: 16,
      autofocus: selected,
      selected: selected,
      onPressed: onPressed,
      child: Container(
        constraints: const BoxConstraints(minHeight: 120),
        padding: const EdgeInsets.all(16),
        decoration: BoxDecoration(
          color: selected ? t.selectedTileFill : t.panel2,
          borderRadius: BorderRadius.circular(16),
          border: Border.all(width: 2, color: selected ? t.acc : t.line),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(eyebrow.toUpperCase(), style: s.type.eyebrow),
            const SizedBox(height: 6),
            Row(
              children: [
                CcIcon(icon, color: t.fg, size: 24),
                const SizedBox(width: 10),
                Text(title, style: s.type.choiceTitle),
              ],
            ),
            const SizedBox(height: 6),
            Text(text, style: s.type.choiceText),
          ],
        ),
      ),
    );
  }
}
