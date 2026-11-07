// SPDX-License-Identifier: GPL-3.0-or-later
// The one table of Beta features (spec 15.1 N89): which ways carry the Beta chip and the
// "· Beta" in their rows. When a way has passed its device test plan, the engineers remove it
// from [betaFeatures]; the chip and the suffix go with it. No other feature may use the chip.

enum BetaFeature {
  /// Receiving AirPlay on a desktop display (15.4).
  airplayReceive,

  /// Receiving Miracast on a Windows display (15.4).
  miracastReceive,

  /// Sending to Google Cast and AirPlay TVs from the desktop presenter (15.5).
  otherTvs,
}

/// The features that are still Beta.
const Set<BetaFeature> betaFeatures = {
  BetaFeature.airplayReceive,
  BetaFeature.miracastReceive,
  BetaFeature.otherTvs,
};

bool isBeta(BetaFeature f) => betaFeatures.contains(f);
