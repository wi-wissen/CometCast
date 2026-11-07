// SPDX-License-Identifier: GPL-3.0-or-later
//! A tiny ZIP writer for test fixtures (also `#[path]`-included by the unit tests of the
//! crate). It can write correct archives and, on purpose, wrong ones.
#![allow(dead_code, missing_docs)]

use std::io::Write;

#[derive(Clone)]
pub struct Entry {
    pub name: Vec<u8>,
    pub data: Vec<u8>,
    pub deflate: bool,
    /// Written in the central directory instead of the real uncompressed size.
    pub lie_usize: Option<u32>,
    pub flags: u16,
}

impl Entry {
    pub fn stored(name: &str, data: &[u8]) -> Self {
        Self {
            name: name.as_bytes().to_vec(),
            data: data.to_vec(),
            deflate: false,
            lie_usize: None,
            flags: 0,
        }
    }
    pub fn deflated(name: &str, data: &[u8]) -> Self {
        Self {
            deflate: true,
            ..Self::stored(name, data)
        }
    }
}

fn le16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn le32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

/// Write an archive from entries.
pub fn build(entries: &[Entry]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for e in entries {
        let mut crc = flate2::Crc::new();
        crc.update(&e.data);
        let packed = if e.deflate {
            let mut enc =
                flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
            enc.write_all(&e.data).unwrap();
            enc.finish().unwrap()
        } else {
            e.data.clone()
        };
        let offset = out.len() as u32;
        let method: u16 = if e.deflate { 8 } else { 0 };
        let usize_real = e.lie_usize.unwrap_or(e.data.len() as u32);
        // local header
        le32(&mut out, 0x0403_4b50);
        le16(&mut out, 20);
        le16(&mut out, e.flags);
        le16(&mut out, method);
        le16(&mut out, 0);
        le16(&mut out, 0x21);
        le32(&mut out, crc.sum());
        le32(&mut out, packed.len() as u32);
        le32(&mut out, usize_real);
        le16(&mut out, e.name.len() as u16);
        le16(&mut out, 0);
        out.extend_from_slice(&e.name);
        out.extend_from_slice(&packed);
        // central header
        le32(&mut central, 0x0201_4b50);
        le16(&mut central, 20);
        le16(&mut central, 20);
        le16(&mut central, e.flags);
        le16(&mut central, method);
        le16(&mut central, 0);
        le16(&mut central, 0x21);
        le32(&mut central, crc.sum());
        le32(&mut central, packed.len() as u32);
        le32(&mut central, usize_real);
        le16(&mut central, e.name.len() as u16);
        le16(&mut central, 0);
        le16(&mut central, 0);
        le16(&mut central, 0);
        le16(&mut central, 0);
        le32(&mut central, 0);
        le32(&mut central, offset);
        central.extend_from_slice(&e.name);
    }
    let cd_offset = out.len() as u32;
    out.extend_from_slice(&central);
    le32(&mut out, 0x0605_4b50);
    le16(&mut out, 0);
    le16(&mut out, 0);
    le16(&mut out, entries.len() as u16);
    le16(&mut out, entries.len() as u16);
    le32(&mut out, central.len() as u32);
    le32(&mut out, cd_offset);
    le16(&mut out, 0);
    out
}

const CT: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>OVERRIDES</Types>"#;

fn content_types(overrides: &str) -> String {
    CT.replace("OVERRIDES", overrides)
}

fn rels(target_type: &str, target: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/{target_type}" Target="{target}"/></Relationships>"#
    )
}

/// A minimal .docx with one paragraph.
pub fn docx(text: &str) -> Vec<u8> {
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>{text}</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1417" w:right="1417" w:bottom="1417" w:left="1417" w:header="708" w:footer="708" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    );
    build(&[
        Entry::deflated(
            "[Content_Types].xml",
            content_types(r#"<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>"#).as_bytes(),
        ),
        Entry::deflated("_rels/.rels", rels("officeDocument", "word/document.xml").as_bytes()),
        Entry::deflated("word/document.xml", doc.as_bytes()),
    ])
}

/// A minimal .xlsx with one sheet and one text cell plus one number.
pub fn xlsx(text: &str) -> Vec<u8> {
    let wb = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets></workbook>"#;
    let sheet = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>{text}</t></is></c><c r="B1"><v>42</v></c></row></sheetData></worksheet>"#
    );
    build(&[
        Entry::deflated(
            "[Content_Types].xml",
            content_types(r#"<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#).as_bytes(),
        ),
        Entry::deflated("_rels/.rels", rels("officeDocument", "xl/workbook.xml").as_bytes()),
        Entry::deflated("xl/workbook.xml", wb.as_bytes()),
        Entry::deflated("xl/_rels/workbook.xml.rels", rels("worksheet", "worksheets/sheet1.xml").as_bytes()),
        Entry::deflated("xl/worksheets/sheet1.xml", sheet.as_bytes()),
    ])
}

/// A minimal .pptx with one slide that has one text box.
pub fn pptx(text: &str) -> Vec<u8> {
    const NS: &str = r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#;
    let head = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;
    let pres = format!(
        r#"{head}<p:presentation {NS}><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst><p:sldIdLst><p:sldId id="256" r:id="rId2"/></p:sldIdLst><p:sldSz cx="9144000" cy="5143500"/><p:notesSz cx="6858000" cy="9144000"/></p:presentation>"#
    );
    let pres_rels = format!(
        r#"{head}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"/></Relationships>"#
    );
    let tree = r#"<p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>"#;
    let master = format!(
        r#"{head}<p:sldMaster {NS}>{tree}</p:spTree></p:cSld><p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/><p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/></p:sldLayoutIdLst></p:sldMaster>"#
    );
    let master_rels = format!(
        r#"{head}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/></Relationships>"#
    );
    let layout =
        format!(r#"{head}<p:sldLayout {NS} type="blank">{tree}</p:spTree></p:cSld></p:sldLayout>"#);
    let layout_rels = format!(
        r#"{head}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/></Relationships>"#
    );
    let slide = format!(
        r#"{head}<p:sld {NS}>{tree}<p:sp><p:nvSpPr><p:cNvPr id="2" name="Text"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="457200" y="457200"/><a:ext cx="8229600" cy="1143000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="en-US" sz="4000"/><a:t>{text}</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>"#
    );
    let slide_rels = format!(
        r#"{head}<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/></Relationships>"#
    );
    build(&[
        Entry::deflated(
            "[Content_Types].xml",
            content_types(r#"<Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/><Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/><Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/><Override PartName="/ppt/slides/slide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#).as_bytes(),
        ),
        Entry::deflated("_rels/.rels", rels("officeDocument", "ppt/presentation.xml").as_bytes()),
        Entry::deflated("ppt/presentation.xml", pres.as_bytes()),
        Entry::deflated("ppt/_rels/presentation.xml.rels", pres_rels.as_bytes()),
        Entry::deflated("ppt/slideMasters/slideMaster1.xml", master.as_bytes()),
        Entry::deflated("ppt/slideMasters/_rels/slideMaster1.xml.rels", master_rels.as_bytes()),
        Entry::deflated("ppt/slideLayouts/slideLayout1.xml", layout.as_bytes()),
        Entry::deflated("ppt/slideLayouts/_rels/slideLayout1.xml.rels", layout_rels.as_bytes()),
        Entry::deflated("ppt/slides/slide1.xml", slide.as_bytes()),
        Entry::deflated("ppt/slides/_rels/slide1.xml.rels", slide_rels.as_bytes()),
    ])
}
