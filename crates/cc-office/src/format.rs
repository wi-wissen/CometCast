// SPDX-License-Identifier: GPL-3.0-or-later
//! Which Office formats are accepted, and how a file is recognised.

/// An accepted input format. The extension of the file name only *proposes* the format; the
/// content (a ZIP container with the expected main part) must agree, see `zipcheck`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OfficeFormat {
    /// Word, Office Open XML.
    Docx,
    /// PowerPoint, Office Open XML.
    Pptx,
    /// Excel, Office Open XML.
    Xlsx,
    /// OpenDocument text (LibreOffice engine only).
    Odt,
    /// OpenDocument presentation (LibreOffice engine only).
    Odp,
    /// OpenDocument spreadsheet (LibreOffice engine only).
    Ods,
}

impl OfficeFormat {
    /// All accepted formats.
    pub const ALL: [OfficeFormat; 6] = [
        Self::Docx,
        Self::Pptx,
        Self::Xlsx,
        Self::Odt,
        Self::Odp,
        Self::Ods,
    ];

    /// Format from a file name or extension (case-insensitive, with or without a dot or a
    /// directory part). Legacy binary formats (`.doc`, `.ppt`, `.xls`) are not accepted.
    pub fn from_name(name: &str) -> Option<Self> {
        let ext = name.rsplit('.').next()?;
        // Without a dot the whole text is taken as the extension ("docx").
        match ext.to_ascii_lowercase().as_str() {
            "docx" => Some(Self::Docx),
            "pptx" => Some(Self::Pptx),
            "xlsx" => Some(Self::Xlsx),
            "odt" => Some(Self::Odt),
            "odp" => Some(Self::Odp),
            "ods" => Some(Self::Ods),
            _ => None,
        }
    }

    /// The canonical extension, without a dot. The conversion always uses this for the
    /// private copy of the input, never the name the user gave.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Docx => "docx",
            Self::Pptx => "pptx",
            Self::Xlsx => "xlsx",
            Self::Odt => "odt",
            Self::Odp => "odp",
            Self::Ods => "ods",
        }
    }

    /// Whether the pure-Rust engine can convert this format.
    pub fn rust_engine_supports(self) -> bool {
        matches!(self, Self::Docx | Self::Pptx | Self::Xlsx)
    }

    /// The part that must exist in the container for the content to match the format
    /// (OOXML), or `None` for OpenDocument (checked through the `mimetype` entry instead).
    pub(crate) fn main_part(self) -> Option<&'static str> {
        match self {
            Self::Docx => Some("word/document.xml"),
            Self::Pptx => Some("ppt/presentation.xml"),
            Self::Xlsx => Some("xl/workbook.xml"),
            _ => None,
        }
    }

    /// The expected content of the first `mimetype` entry of an OpenDocument file.
    pub(crate) fn odf_mimetype(self) -> Option<&'static str> {
        match self {
            Self::Odt => Some("application/vnd.oasis.opendocument.text"),
            Self::Odp => Some("application/vnd.oasis.opendocument.presentation"),
            Self::Ods => Some("application/vnd.oasis.opendocument.spreadsheet"),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_map_to_formats() {
        assert_eq!(OfficeFormat::from_name("a.DOCX"), Some(OfficeFormat::Docx));
        assert_eq!(
            OfficeFormat::from_name("/x/y.z/slides.pptx"),
            Some(OfficeFormat::Pptx)
        );
        assert_eq!(OfficeFormat::from_name("t.xlsx"), Some(OfficeFormat::Xlsx));
        assert_eq!(OfficeFormat::from_name("t.odp"), Some(OfficeFormat::Odp));
        assert_eq!(OfficeFormat::from_name("old.doc"), None);
        assert_eq!(OfficeFormat::from_name("old.xls"), None);
        assert_eq!(OfficeFormat::from_name("noext"), None);
        assert_eq!(OfficeFormat::from_name("a.pdf"), None);
    }

    #[test]
    fn only_ooxml_goes_to_the_rust_engine() {
        for f in OfficeFormat::ALL {
            let ooxml = matches!(
                f,
                OfficeFormat::Docx | OfficeFormat::Pptx | OfficeFormat::Xlsx
            );
            assert_eq!(f.rust_engine_supports(), ooxml);
            assert_eq!(f.main_part().is_some(), ooxml);
            assert_eq!(f.odf_mimetype().is_some(), !ooxml);
        }
    }
}
