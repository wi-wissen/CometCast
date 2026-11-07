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
}

impl OfficeFormat {
    /// All accepted formats.
    pub const ALL: [OfficeFormat; 3] = [Self::Docx, Self::Pptx, Self::Xlsx];

    /// Format from a file name (case-insensitive, with or without a directory part). The name
    /// must have a dot: a bare word such as "docx" is a file called docx, not a format.
    /// OpenDocument files (`.odt .odp .ods`) and legacy binary formats (`.doc`, `.ppt`,
    /// `.xls`) are not accepted.
    pub fn from_name(name: &str) -> Option<Self> {
        let (_, ext) = name.rsplit_once('.')?;
        Self::from_extension(ext)
    }

    /// Format from an extension without a dot (case-insensitive), as the parent passes it to
    /// the worker.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "docx" => Some(Self::Docx),
            "pptx" => Some(Self::Pptx),
            "xlsx" => Some(Self::Xlsx),
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
        }
    }

    /// The part that must exist in the container for the content to match the format.
    pub(crate) fn main_part(self) -> &'static str {
        match self {
            Self::Docx => "word/document.xml",
            Self::Pptx => "ppt/presentation.xml",
            Self::Xlsx => "xl/workbook.xml",
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
        // OpenDocument is not accepted.
        for odf in ["t.odp", "t.odt", "t.ods"] {
            assert_eq!(OfficeFormat::from_name(odf), None);
        }
        assert_eq!(OfficeFormat::from_name("old.doc"), None);
        assert_eq!(OfficeFormat::from_name("old.xls"), None);
        assert_eq!(OfficeFormat::from_name("noext"), None);
        // A dotless name is never a format, even when it spells one.
        assert_eq!(OfficeFormat::from_name("docx"), None);
        assert_eq!(OfficeFormat::from_name("/tmp/xlsx"), None);
        assert_eq!(
            OfficeFormat::from_extension("DOCX"),
            Some(OfficeFormat::Docx)
        );
        assert_eq!(OfficeFormat::from_extension(".docx"), None);
        assert_eq!(OfficeFormat::from_name("a.pdf"), None);
    }
}
