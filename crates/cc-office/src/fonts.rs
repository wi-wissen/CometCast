// SPDX-License-Identifier: GPL-3.0-or-later
//! The metric-compatible Office fonts that ship with the converter (see `fonts/FONTS.md`).
//!
//! Liberation Sans, Serif and Mono (Arial, Times New Roman, Courier New), Carlito (Calibri) and
//! Caladea (Cambria), all SIL OFL 1.1. They are compiled in as gzip files (about 3.7 MB for the
//! 20 faces instead of 7.4 MB) and inflated by the conversion worker; nothing is fetched at run
//! time and no system font is needed for these families. `office2pdf` already maps the Office names
//! to these families (Calibri to Carlito, Cambria to Caladea, Arial to Liberation Sans, Times New
//! Roman to Liberation Serif, Courier New to Liberation Mono), so handing the bytes over as
//! `ConvertOptions::font_bytes` (which take priority over system and built-in fonts) is enough.

use std::io::Read;

use flate2::read::GzDecoder;

macro_rules! fonts {
    ($($name:literal),* $(,)?) => {
        /// `(file name without .gz, gzip bytes)`
        pub(crate) const FONTS_GZ: &[(&str, &[u8])] = &[
            $(($name, include_bytes!(concat!("../fonts/", $name, ".gz")))),*
        ];
    };
}

fonts!(
    "LiberationSans-Regular.ttf",
    "LiberationSans-Bold.ttf",
    "LiberationSans-Italic.ttf",
    "LiberationSans-BoldItalic.ttf",
    "LiberationSerif-Regular.ttf",
    "LiberationSerif-Bold.ttf",
    "LiberationSerif-Italic.ttf",
    "LiberationSerif-BoldItalic.ttf",
    "LiberationMono-Regular.ttf",
    "LiberationMono-Bold.ttf",
    "LiberationMono-Italic.ttf",
    "LiberationMono-BoldItalic.ttf",
    "Carlito-Regular.ttf",
    "Carlito-Bold.ttf",
    "Carlito-Italic.ttf",
    "Carlito-BoldItalic.ttf",
    "Caladea-Regular.ttf",
    "Caladea-Bold.ttf",
    "Caladea-Italic.ttf",
    "Caladea-BoldItalic.ttf",
);

/// Inflates one bundled font. The data is part of the program, so a failure is a build defect.
pub(crate) fn inflate(gz: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(gz.len() * 2);
    GzDecoder::new(gz)
        .read_to_end(&mut out)
        .map_err(|e| format!("bundled font is damaged: {e}"))?;
    Ok(out)
}

/// All bundled faces as plain TrueType bytes.
pub(crate) fn bundled_fonts() -> Result<Vec<Vec<u8>>, String> {
    FONTS_GZ.iter().map(|(_, gz)| inflate(gz)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundled_font_inflates_to_truetype() {
        assert_eq!(FONTS_GZ.len(), 20);
        for (name, gz) in FONTS_GZ {
            let ttf = inflate(gz).unwrap();
            // TrueType scaler type 0x00010000.
            assert_eq!(&ttf[..4], &[0, 1, 0, 0], "{name} is not a TrueType file");
            assert!(ttf.len() > 50_000, "{name} is suspiciously small");
        }
    }
}
