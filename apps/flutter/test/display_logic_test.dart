// SPDX-License-Identifier: GPL-3.0-or-later
import 'package:flutter_test/flutter_test.dart';

import 'package:cometcast/src/display/display_logic.dart';
import 'package:cometcast/src/display/setup_wizard.dart';

void main() {
  const chips = ['Meeting room Elbe', 'Living room'];

  test('wizard: a chip clears the field, typing deselects chips, emptying reselects', () {
    var w = initialWizard(chips.first);
    expect(w.chipSelected(chips.first), isTrue);
    w = wizardChip(w, chips[1]);
    expect(w.room, 'Living room');
    w = wizardCustom(w, 'Studio', chips.first);
    expect(w.room, 'Studio');
    expect(w.chipSelected(chips[1]), isFalse);
    w = wizardCustom(w, '   ', chips.first);
    expect(w.room, 'Living room'); // the chip chosen before typing
    expect(w.chipSelected(chips[1]), isTrue);
  });

  test('wizard: steps stay between 0 and 4, mode defaults to open', () {
    var w = initialWizard('x');
    expect(w.mode, DefaultMode.open);
    w = wizardBack(w);
    expect(w.step, 0);
    for (var i = 0; i < 6; i++) {
      w = wizardNext(w);
    }
    expect(w.step, 4);
  });

  test('names and codes', () {
    expect(cleanRoomName('  A​B \n C '), 'AB C');
    expect(cleanRoomName('x' * 40).length, 32);
    expect(shortName('Alexandria Montgomery'), 'Alexandria Montgom…');
    expect(shortName('Lena'), 'Lena');
    expect(formatCode('K7QM2W'), 'K7Q M2W');
    expect(
      joinUrl('cast.example.org', 'K7Q M2W'),
      'https://cast.example.org/K7QM2W',
    );
  });

  test('connection view: connecting, offline after 3 s of failure or 8 s of silence', () {
    ConnectionView v(bool code, bool err, bool failed, int s) => connectionView(
      hasCode: code,
      errored: err,
      failed: failed,
      withoutCode: Duration(seconds: s),
    );
    expect(v(true, false, false, 0), ConnectionView.online);
    expect(v(false, false, false, 7), ConnectionView.connecting);
    expect(v(false, false, false, 8), ConnectionView.offline);
    expect(v(false, true, true, 2), ConnectionView.connecting);
    expect(v(false, true, true, 3), ConnectionView.offline);
  });

  test('retry schedule: 1 s, 2 s, 3 s after the loss, then every 4 s; busy 30 s; outdated never', () {
    Duration? d(
      int attempt,
      int since, [
      OfflineCause c = OfflineCause.network,
    ]) => retryDelay(
      attempt: attempt,
      sinceLoss: Duration(seconds: since),
      cause: c,
    );
    expect(d(0, 0), const Duration(seconds: 1));
    expect(d(1, 1), const Duration(seconds: 1));
    expect(d(2, 2), const Duration(seconds: 1));
    expect(d(3, 3), const Duration(seconds: 4));
    expect(d(4, 7), const Duration(seconds: 4));
    expect(d(0, 0, OfflineCause.busy), const Duration(seconds: 30));
    expect(d(0, 0, OfflineCause.outdated), isNull);
    expect(offlineCauseOf('server error rate_limited: x'), OfflineCause.busy);
    expect(
      offlineCauseOf('server error proto_mismatch'),
      OfflineCause.outdated,
    );
    expect(
      offlineCauseOf('signalling connection closed'),
      OfflineCause.network,
    );
  });

  test('address check', () {
    expect(checkTypedAddress(''), AddressError.empty);
    expect(checkTypedAddress('a b'), AddressError.invalid);
    expect(checkTypedAddress('ws://x'), AddressError.insecure);
    expect(checkTypedAddress('u@x'), AddressError.credentials);
    expect(checkTypedAddress('x.org/path'), AddressError.path);
    expect(checkTypedAddress('cast.example.org:8443'), isNull);
  });
}
