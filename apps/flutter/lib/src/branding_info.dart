// SPDX-License-Identifier: GPL-3.0-or-later
// The publisher identity and legal links the About pages show. The values of the build come from
// the generated `branding.g.dart`; [BrandingInfo.current] can be replaced by tests and by the
// screenshot test (to show the official branding file) and is never changed by the app itself.

import 'branding.g.dart';

class BrandingInfo {
  const BrandingInfo({
    this.publisherName = '',
    this.privacyUrlDe = '',
    this.privacyUrlEn = '',
    this.imprintUrlDe = '',
    this.imprintUrlEn = '',
    this.sourceUrl = '',
  });

  /// What the build was generated with (`tools/gen-branding.mjs`).
  const BrandingInfo.generated()
    : publisherName = Branding.publisherName,
      privacyUrlDe = Branding.privacyUrlDe,
      privacyUrlEn = Branding.privacyUrlEn,
      imprintUrlDe = Branding.imprintUrlDe,
      imprintUrlEn = Branding.imprintUrlEn,
      sourceUrl = Branding.sourceUrl;

  /// The values of `branding/cometcast.json` (the official build), for screenshots and tests.
  static const BrandingInfo official = BrandingInfo(
    publisherName: 'Schulverwalter GmbH',
    privacyUrlDe: 'https://cometcast.de/datenschutz/',
    privacyUrlEn: 'https://cometcast.de/en/privacy/',
    imprintUrlDe: 'https://cometcast.de/impressum/',
    imprintUrlEn: 'https://cometcast.de/en/imprint/',
    sourceUrl: 'https://github.com/wi-wissen/CometCast',
  );

  static BrandingInfo current = const BrandingInfo.generated();

  final String publisherName;
  final String privacyUrlDe;
  final String privacyUrlEn;
  final String imprintUrlDe;
  final String imprintUrlEn;
  final String sourceUrl;
}
