// SPDX-License-Identifier: GPL-3.0-or-later
//! The messages of the `control` data channel and their validation.
//!
//! The format is the one of the web client (docs/web-m2.md, `apps/web/src/core/control.ts`):
//! one JSON object per text message with a `kind`, and binary chunks of 16 KiB. This module
//! adds the `document` kinds (PDF), which are marked UNCERTAIN in the docs.
//!
//! Everything here is pure: parsing never panics and never allocates more than the message
//! itself.

use cc_proto::ControlCmd;

/// Size of every binary chunk except the last one of a transfer (16 KiB).
pub const CHUNK_SIZE: usize = 16 * 1024;
/// Longest text message in characters (docs/web-m2.md).
pub const MAX_TEXT_CHARS: usize = 8192;
/// Longest link in characters.
pub const MAX_LINK_LENGTH: usize = 2048;
/// Longest file name in characters.
pub const MAX_NAME_LENGTH: usize = 255;
/// Largest image the format allows (10 MiB, as the web client).
/// UNCERTAIN: a guess, no measurements behind it.
pub const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;
/// Largest PDF a sender sends and a receiver accepts by default (64 MiB). The renderer of
/// `cc-docs` accepts 100 MiB by default; a receiver can set its own limit.
/// UNCERTAIN: a guess for typical PDFs; scans of whole books can be larger.
pub const MAX_DOCUMENT_BYTES: u64 = 64 * 1024 * 1024;
/// Largest page number a `go_to_page` command may carry (as the web client).
pub const MAX_PAGE_NUMBER: u32 = 1_000_000;
/// The only MIME type of a document.
pub const PDF_MIME: &str = "application/pdf";

/// The accepted image types. No SVG: it can carry script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageMime {
    /// `image/png`
    Png,
    /// `image/jpeg`
    Jpeg,
    /// `image/webp`
    Webp,
    /// `image/gif`
    Gif,
}

impl ImageMime {
    /// Parse the exact MIME string (no parameters, case sensitive, like the web client).
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "image/png" => Some(Self::Png),
            "image/jpeg" => Some(Self::Jpeg),
            "image/webp" => Some(Self::Webp),
            "image/gif" => Some(Self::Gif),
            _ => None,
        }
    }

    /// The MIME string.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Webp => "image/webp",
            Self::Gif => "image/gif",
        }
    }

    /// Guess from a file name extension (for senders).
    pub fn from_extension(name: &str) -> Option<Self> {
        let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
        match ext.as_str() {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "webp" => Some(Self::Webp),
            "gif" => Some(Self::Gif),
            _ => None,
        }
    }
}

/// A parsed text message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// Remote-control command.
    Cmd(ControlCmd),
    /// Starts an image transfer.
    Image {
        /// Display name (control and bidirectional characters removed).
        name: String,
        /// Declared type.
        mime: ImageMime,
        /// Bytes that follow in chunks.
        size: u64,
    },
    /// Ends an image transfer.
    ImageEnd,
    /// Starts a PDF transfer (new in the native clients, UNCERTAIN).
    Document {
        /// Display name.
        name: String,
        /// Bytes that follow in chunks.
        size: u64,
    },
    /// Ends a PDF transfer.
    DocumentEnd,
    /// Asks the receiver to show a link (already validated, normalised).
    Link {
        /// The normalised `http` or `https` URL.
        url: String,
    },
    /// Display to presenter: the shown PDF page and the page count (decision 13.31). A
    /// receiver ignores it; a presenter keeps the last one for the current document.
    Page {
        /// The shown page, from 1.
        page: u32,
        /// The page count, from 1, at least `page`.
        pages: u32,
    },
    /// Presenter to display (iOS, native-apps.md 20.5): "hold this single-use token, the
    /// broadcast extension will join with it". The token is 1 to 64 characters of the
    /// base64url alphabet. A display without hand-over support does not answer.
    Handover {
        /// The token (checked like `join_room.handover`).
        token: String,
    },
    /// Display to presenter: the token of a `handover` message is stored. A presenter that
    /// does not get this within a few seconds joins the ordinary way (the display then asks
    /// the host as for any join).
    HandoverOk,
}

/// Why a text message was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    /// Longer than [`MAX_TEXT_CHARS`].
    #[error("message too long")]
    TooLong,
    /// Not a JSON object with a `kind`.
    #[error("not a control message")]
    Malformed,
    /// A `kind` this version does not know.
    #[error("unknown message kind")]
    UnknownKind,
    /// A known `kind` with missing, wrongly typed or out-of-range fields.
    #[error("invalid fields")]
    InvalidFields,
    /// The MIME type is not on the allow list.
    #[error("type not allowed")]
    BadMime,
    /// The link is not an acceptable http(s) URL.
    #[error("link not allowed")]
    BadLink,
}

/// Parse one text message. `max_size` is the largest `size` accepted for the transfer kinds.
/// Never panics.
pub fn parse_message(text: &str, max_image: u64, max_document: u64) -> Result<Message, ParseError> {
    // Bytes first (cheap), then characters: a message cannot be longer in characters
    // than in bytes, so the byte test never refuses a message that is short enough.
    if text.len() > MAX_TEXT_CHARS * 4 || text.chars().count() > MAX_TEXT_CHARS {
        return Err(ParseError::TooLong);
    }
    let v: serde_json::Value = serde_json::from_str(text).map_err(|_| ParseError::Malformed)?;
    let o = v.as_object().ok_or(ParseError::Malformed)?;
    let kind = o
        .get("kind")
        .and_then(|k| k.as_str())
        .ok_or(ParseError::Malformed)?;
    match kind {
        "cmd" => {
            let c = o.get("cmd").ok_or(ParseError::InvalidFields)?;
            let cmd: ControlCmd =
                serde_json::from_value(c.clone()).map_err(|_| ParseError::InvalidFields)?;
            if let ControlCmd::GoToPage { page } = cmd
                && !(1..=MAX_PAGE_NUMBER).contains(&page)
            {
                return Err(ParseError::InvalidFields);
            }
            Ok(Message::Cmd(cmd))
        }
        "image" => {
            let name = parse_name(o.get("name"))?;
            let mime = o
                .get("mime")
                .and_then(|m| m.as_str())
                .ok_or(ParseError::InvalidFields)?;
            let mime = ImageMime::parse(mime).ok_or(ParseError::BadMime)?;
            let size = parse_size(o.get("size"), max_image)?;
            Ok(Message::Image { name, mime, size })
        }
        "image_end" => Ok(Message::ImageEnd),
        "document" => {
            let name = parse_name(o.get("name"))?;
            let mime = o
                .get("mime")
                .and_then(|m| m.as_str())
                .ok_or(ParseError::InvalidFields)?;
            if mime != PDF_MIME {
                return Err(ParseError::BadMime);
            }
            let size = parse_size(o.get("size"), max_document)?;
            Ok(Message::Document { name, size })
        }
        "document_end" => Ok(Message::DocumentEnd),
        "link" => {
            let url = o
                .get("url")
                .and_then(|u| u.as_str())
                .ok_or(ParseError::InvalidFields)?;
            let url = validate_link(url).ok_or(ParseError::BadLink)?;
            Ok(Message::Link { url })
        }
        "page" => {
            let n = |k: &str| {
                o.get(k)
                    .and_then(|v| v.as_u64().or_else(|| v.as_f64().and_then(whole_number)))
                    .and_then(|v| u32::try_from(v).ok())
                    .filter(|v| (1..=MAX_PAGE_NUMBER).contains(v))
            };
            match (n("page"), n("pages")) {
                (Some(page), Some(pages)) if page <= pages => Ok(Message::Page { page, pages }),
                _ => Err(ParseError::InvalidFields),
            }
        }
        "handover" => {
            let token = o
                .get("token")
                .and_then(|t| t.as_str())
                .and_then(|t| cc_proto::sanitize_handover(Some(t)))
                .ok_or(ParseError::InvalidFields)?;
            Ok(Message::Handover { token })
        }
        "handover_ok" => Ok(Message::HandoverOk),
        _ => Err(ParseError::UnknownKind),
    }
}

/// An integral float (`2.0`, `1e1`) as an integer, as JavaScript's `Number.isInteger` sees it.
fn whole_number(f: f64) -> Option<u64> {
    (f.is_finite() && f >= 0.0 && f.fract() == 0.0 && f <= 4_294_967_295.0).then_some(f as u64)
}

fn parse_name(v: Option<&serde_json::Value>) -> Result<String, ParseError> {
    let s = v
        .and_then(|n| n.as_str())
        .ok_or(ParseError::InvalidFields)?;
    let n = s.chars().count();
    if n == 0 || n > MAX_NAME_LENGTH {
        return Err(ParseError::InvalidFields);
    }
    let clean = sanitize_name(s);
    if clean.is_empty() {
        return Err(ParseError::InvalidFields);
    }
    Ok(clean)
}

fn parse_size(v: Option<&serde_json::Value>, max: u64) -> Result<u64, ParseError> {
    // `as_u64` is `None` for fractions, negative numbers and numbers written as floats.
    let size = v
        .and_then(|s| s.as_u64())
        .ok_or(ParseError::InvalidFields)?;
    if size == 0 || size > max {
        return Err(ParseError::InvalidFields);
    }
    Ok(size)
}

fn is_bidi(c: char) -> bool {
    matches!(c, '\u{200e}' | '\u{200f}' | '\u{061c}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}

/// Characters that are invisible or change how text is laid out, so a displayed name can
/// read differently from what it is: the bidirectional controls, the Unicode "format"
/// characters (category Cf: zero-width space and joiners, BOM, soft hyphen, invisible
/// operators, tag characters, ...), the line and paragraph separators (Zl, Zp) and a few
/// letters and symbols that render as blank (braille blank, Hangul fillers).
///
/// UNCERTAIN: a hand-written list of the Cf ranges of Unicode 15, not generated from the
/// Unicode data; characters added to Cf later are not covered.
fn is_invisible(c: char) -> bool {
    is_bidi(c)
        || matches!(c,
            '\u{00ad}'
            | '\u{0600}'..='\u{0605}'
            | '\u{06dd}'
            | '\u{070f}'
            | '\u{0890}'..='\u{0891}'
            | '\u{08e2}'
            | '\u{180e}'
            | '\u{200b}'..='\u{200f}'
            | '\u{2028}'..='\u{202e}'
            | '\u{2060}'..='\u{206f}'
            | '\u{feff}'
            | '\u{fff9}'..='\u{fffb}'
            | '\u{110bd}'
            | '\u{110cd}'
            | '\u{13430}'..='\u{1343f}'
            | '\u{1bca0}'..='\u{1bca3}'
            | '\u{1d173}'..='\u{1d17a}'
            // The whole tag block (E0001 and E0020..E007F are Cf).
            | '\u{e0000}'..='\u{e007f}'
            // Blank-looking letters and symbols.
            | '\u{115f}' | '\u{1160}' | '\u{17b4}' | '\u{17b5}' | '\u{2800}' | '\u{3164}' | '\u{ffa0}')
}

/// Removes control characters, bidirectional controls and other invisible characters (see
/// `is_invisible`), which can make a displayed name read differently from what it is.
pub fn sanitize_name(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control() && !is_invisible(*c))
        .collect()
}

/// What a transfer carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferKind {
    /// An image.
    Image,
    /// A PDF document.
    Document,
}

/// The kind and announced size of an `image` / `document` start message, read leniently: used
/// when the message was **refused**, so that the receiver can skip the data that follows
/// instead of counting every chunk as a violation. `None` if the text is not such a message
/// or carries no usable size. Never panics.
pub fn announced_transfer(text: &str) -> Option<(TransferKind, u64)> {
    if text.len() > MAX_TEXT_CHARS * 4 {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let o = v.as_object()?;
    let kind = match o.get("kind")?.as_str()? {
        "image" => TransferKind::Image,
        "document" => TransferKind::Document,
        _ => return None,
    };
    let size = o.get("size")?.as_u64().filter(|s| *s > 0)?;
    Some((kind, size))
}

/// Accepts only absolute `http`/`https` URLs without credentials. Returns the normalised
/// URL, or `None`. The check follows `validateLinkUrl` of the web client: length at most 2048,
/// no whitespace or control characters (the parser would silently drop tabs and newlines), a
/// non-empty host. A link is never opened by code; the receiver's person has to decide.
///
/// The length limit applies to the input **and** to the normalised URL (percent-encoding and
/// punycode can make it several times longer), so a link that passes always fits into a
/// `link` message and passes the receiver's check. The result is a fixed point:
/// `validate_link(validate_link(x))` equals `validate_link(x)`.
pub fn validate_link(input: &str) -> Option<String> {
    let normalised = check_link(input)?;
    // Idempotent by construction: anything that does not survive a second pass is refused.
    (check_link(&normalised).as_deref() == Some(normalised.as_str())).then_some(normalised)
}

fn check_link(input: &str) -> Option<String> {
    // Only plain spaces are trimmed; a tab, newline or U+0085 at the edge is refused below.
    let s = input.trim_matches(' ');
    if s.is_empty() || s.chars().count() > MAX_LINK_LENGTH {
        return None;
    }
    if s.chars()
        .any(|c| c.is_control() || c.is_whitespace() || is_bidi(c))
    {
        return None;
    }
    let u = url::Url::parse(s).ok()?;
    if u.scheme() != "http" && u.scheme() != "https" {
        return None;
    }
    if !u.username().is_empty() || u.password().is_some() {
        return None;
    }
    // The web client lets `http://@host` through (empty user name); the raw text of the
    // authority is stricter here: any `@` before the path is refused.
    let after_scheme = s.split_once("://")?.1;
    let authority_end = after_scheme
        .find(['/', '\\', '?', '#'])
        .unwrap_or(after_scheme.len());
    if after_scheme[..authority_end].contains('@') {
        return None;
    }
    if u.host_str().is_none_or(str::is_empty) {
        return None;
    }
    let out: String = u.into();
    // The normalised form is ASCII, so bytes and characters agree.
    if out.len() > MAX_LINK_LENGTH {
        return None;
    }
    Some(out)
}

impl Message {
    /// The JSON text of the message, in the format of docs/web-m2.md.
    pub fn to_json(&self) -> String {
        use serde_json::json;
        let v = match self {
            Message::Cmd(c) => json!({"kind": "cmd", "cmd": c}),
            Message::Image { name, mime, size } => {
                json!({"kind": "image", "name": name, "mime": mime.as_str(), "size": size})
            }
            Message::ImageEnd => json!({"kind": "image_end"}),
            Message::Document { name, size } => {
                json!({"kind": "document", "name": name, "mime": PDF_MIME, "size": size})
            }
            Message::DocumentEnd => json!({"kind": "document_end"}),
            Message::Link { url } => json!({"kind": "link", "url": url}),
            Message::Page { page, pages } => json!({"kind": "page", "page": page, "pages": pages}),
            Message::Handover { token } => json!({"kind": "handover", "token": token}),
            Message::HandoverOk => json!({"kind": "handover_ok"}),
        };
        v.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<Message, ParseError> {
        parse_message(s, MAX_IMAGE_BYTES, MAX_DOCUMENT_BYTES)
    }

    #[test]
    fn web_messages_parse() {
        assert_eq!(
            parse(r#"{"kind":"image","name":"a.png","mime":"image/png","size":40000}"#),
            Ok(Message::Image {
                name: "a.png".into(),
                mime: ImageMime::Png,
                size: 40000
            })
        );
        assert_eq!(parse(r#"{"kind":"image_end"}"#), Ok(Message::ImageEnd));
        assert_eq!(
            parse(r#"{"kind":"cmd","cmd":"next_page"}"#),
            Ok(Message::Cmd(ControlCmd::NextPage))
        );
        assert_eq!(
            parse(r#"{"kind":"cmd","cmd":{"go_to_page":{"page":3}}}"#),
            Ok(Message::Cmd(ControlCmd::GoToPage { page: 3 }))
        );
        assert_eq!(
            parse(r#"{"kind":"cmd","cmd":"stop"}"#),
            Ok(Message::Cmd(ControlCmd::Stop))
        );
    }

    #[test]
    fn handover_messages_are_parsed_and_bounded() {
        assert_eq!(
            parse(r#"{"kind":"handover","token":"AbC-_123"}"#),
            Ok(Message::Handover {
                token: "AbC-_123".into()
            })
        );
        assert_eq!(parse(r#"{"kind":"handover_ok"}"#), Ok(Message::HandoverOk));
        let long = format!(r#"{{"kind":"handover","token":"{}"}}"#, "a".repeat(65));
        for bad in [
            r#"{"kind":"handover"}"#,
            r#"{"kind":"handover","token":""}"#,
            r#"{"kind":"handover","token":5}"#,
            r#"{"kind":"handover","token":"a b"}"#,
            r#"{"kind":"handover","token":"a+b"}"#,
            long.as_str(),
        ] {
            assert_eq!(parse(bad), Err(ParseError::InvalidFields), "{bad}");
        }
        assert_eq!(
            Message::Handover { token: "x1".into() }.to_json(),
            r#"{"kind":"handover","token":"x1"}"#
        );
        assert_eq!(Message::HandoverOk.to_json(), r#"{"kind":"handover_ok"}"#);
    }

    #[test]
    fn page_messages_follow_the_web_rules() {
        let page = |p, n| Ok(Message::Page { page: p, pages: n });
        assert_eq!(parse(r#"{"kind":"page","page":1,"pages":3}"#), page(1, 3));
        assert_eq!(parse(r#"{"kind":"page","page":3,"pages":3}"#), page(3, 3));
        assert_eq!(
            parse(r#"{"kind":"page","page":2.0,"pages":1e1}"#),
            page(2, 10)
        );
        assert_eq!(
            parse(r#"{"kind":"page","page":1000000,"pages":1000000}"#),
            page(1_000_000, 1_000_000)
        );
        for bad in [
            r#"{"kind":"page","page":4,"pages":3}"#,
            r#"{"kind":"page","page":0,"pages":3}"#,
            r#"{"kind":"page","page":1,"pages":0}"#,
            r#"{"kind":"page","page":-1,"pages":3}"#,
            r#"{"kind":"page","page":1.5,"pages":3}"#,
            r#"{"kind":"page","page":1,"pages":1000001}"#,
            r#"{"kind":"page","page":"1","pages":3}"#,
            r#"{"kind":"page","pages":3}"#,
        ] {
            assert_eq!(parse(bad), Err(ParseError::InvalidFields), "{bad}");
        }
        let m = Message::Page { page: 2, pages: 7 };
        assert_eq!(parse(&m.to_json()), Ok(m));
    }

    #[test]
    fn documents_parse_and_round_trip() {
        let m = Message::Document {
            name: "a.pdf".into(),
            size: 5,
        };
        assert_eq!(
            m.to_json(),
            r#"{"kind":"document","mime":"application/pdf","name":"a.pdf","size":5}"#
        );
        assert_eq!(parse(&m.to_json()), Ok(m));
        assert_eq!(
            parse(r#"{"kind":"document_end"}"#),
            Ok(Message::DocumentEnd)
        );
    }

    #[test]
    fn every_message_round_trips() {
        for m in [
            Message::Cmd(ControlCmd::PrevPage),
            Message::Cmd(ControlCmd::GoToPage { page: 9 }),
            Message::Cmd(ControlCmd::Stop),
            Message::Image {
                name: "x.webp".into(),
                mime: ImageMime::Webp,
                size: 1,
            },
            Message::ImageEnd,
            Message::DocumentEnd,
            Message::Link {
                url: "https://example.org/a?b=1".into(),
            },
        ] {
            assert_eq!(parse(&m.to_json()), Ok(m));
        }
    }

    #[test]
    fn bad_mimes_are_refused() {
        for mime in [
            "image/svg+xml",
            "text/html",
            "IMAGE/PNG",
            "image/png; x=1",
            "",
        ] {
            let t = format!(r#"{{"kind":"image","name":"a","mime":"{mime}","size":3}}"#);
            assert_eq!(parse(&t), Err(ParseError::BadMime), "{mime}");
        }
        for mime in ["image/png", "application/octet-stream", "application/PDF"] {
            let t = format!(r#"{{"kind":"document","name":"a","mime":"{mime}","size":3}}"#);
            assert_eq!(parse(&t), Err(ParseError::BadMime), "{mime}");
        }
    }

    #[test]
    fn sizes_are_checked() {
        let img = |size: &str| {
            format!(r#"{{"kind":"image","name":"a","mime":"image/png","size":{size}}}"#)
        };
        assert!(parse(&img("1")).is_ok());
        assert!(parse(&img("10485760")).is_ok());
        for bad in [
            "0",
            "-1",
            "10485761",
            "1.5",
            "\"7\"",
            "null",
            "1e3",
            "99999999999999999999",
        ] {
            assert_eq!(parse(&img(bad)), Err(ParseError::InvalidFields), "{bad}");
        }
        let doc = |size: u64| {
            format!(r#"{{"kind":"document","name":"a","mime":"application/pdf","size":{size}}}"#)
        };
        assert!(parse(&doc(MAX_DOCUMENT_BYTES)).is_ok());
        assert!(parse(&doc(MAX_DOCUMENT_BYTES + 1)).is_err());
        // A receiver with a smaller limit refuses earlier.
        assert!(parse_message(&doc(11), 10, 10).is_err());
    }

    #[test]
    fn names_are_cleaned_or_refused() {
        let t = |n: &str| format!(r#"{{"kind":"image","name":{n},"mime":"image/png","size":3}}"#);
        assert_eq!(
            parse(&t("\"a\\u202eb.png\"")),
            Ok(Message::Image {
                name: "ab.png".into(),
                mime: ImageMime::Png,
                size: 3
            })
        );
        assert!(
            parse(&t("\"\\u202e\\u0000\"")).is_err(),
            "nothing left after cleaning"
        );
        assert!(parse(&t(r#""""#)).is_err());
        assert!(parse(&t(&format!("\"{}\"", "a".repeat(256)))).is_err());
        assert!(parse(&t(&format!("\"{}\"", "a".repeat(255)))).is_ok());
        assert!(parse(&t("5")).is_err());
    }

    #[test]
    fn cmd_edge_cases() {
        for bad in [
            r#"{"kind":"cmd"}"#,
            r#"{"kind":"cmd","cmd":"explode"}"#,
            r#"{"kind":"cmd","cmd":{"go_to_page":{"page":0}}}"#,
            r#"{"kind":"cmd","cmd":{"go_to_page":{"page":1000001}}}"#,
            r#"{"kind":"cmd","cmd":{"go_to_page":{"page":-1}}}"#,
            r#"{"kind":"cmd","cmd":{"go_to_page":{}}}"#,
            r#"{"kind":"cmd","cmd":7}"#,
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
        assert!(parse(r#"{"kind":"cmd","cmd":{"go_to_page":{"page":1000000}}}"#).is_ok());
    }

    #[test]
    fn junk_is_refused_without_panic() {
        assert_eq!(parse(""), Err(ParseError::Malformed));
        assert_eq!(parse("null"), Err(ParseError::Malformed));
        assert_eq!(parse("[]"), Err(ParseError::Malformed));
        assert_eq!(parse(r#"{"kind":5}"#), Err(ParseError::Malformed));
        assert_eq!(parse(r#"{"kind":"zip"}"#), Err(ParseError::UnknownKind));
        assert_eq!(parse(&"[".repeat(100_000)), Err(ParseError::TooLong));
        // Deeply nested JSON: serde_json has a recursion limit, so this is an error, not a crash.
        assert!(parse(&format!("{}1{}", "[".repeat(5000), "]".repeat(5000))).is_err());
        let long = format!(
            r#"{{"kind":"link","url":"https://a.org/{}"}}"#,
            "x".repeat(9000)
        );
        assert_eq!(parse(&long), Err(ParseError::TooLong));
    }

    #[test]
    fn links_follow_the_web_rules() {
        assert_eq!(
            validate_link("https://example.org/path?x=1").as_deref(),
            Some("https://example.org/path?x=1")
        );
        assert_eq!(
            validate_link("http://Example.ORG").as_deref(),
            Some("http://example.org/")
        );
        assert_eq!(
            validate_link("  https://example.org  ").as_deref(),
            Some("https://example.org/")
        );
        for bad in [
            "",
            "javascript:alert(1)",
            "data:text/html,hi",
            "file:///etc/passwd",
            "ftp://example.org",
            "//example.org",
            "example.org",
            "https://user@example.org",
            "https://user:pw@example.org",
            "https://@example.org",
            "https://exa mple.org",
            "https://example.org/a\tb",
            "https://example.org/a\nb",
            "https://example.org/\u{0}",
            "https://example.org/\u{7f}",
            "https://example.org/\u{85}",
            "https://example.org/\u{202e}",
            "https://",
            "https://[::1",
        ] {
            assert_eq!(validate_link(bad), None, "{bad:?}");
        }
        // A backslash ends the host like a slash does (WHATWG); the host is example.org.
        assert_eq!(
            validate_link("https://example.org\\@evil.example").as_deref(),
            Some("https://example.org/@evil.example")
        );
        let at_limit = format!(
            "https://a.org/{}",
            "x".repeat(MAX_LINK_LENGTH - "https://a.org/".len())
        );
        assert!(validate_link(&at_limit).is_some());
        assert!(validate_link(&format!("{at_limit}x")).is_none());
        // Internationalised host names are normalised to punycode like in a browser.
        assert_eq!(
            validate_link("https://bücher.example/").as_deref(),
            Some("https://xn--bcher-kva.example/")
        );
    }

    #[test]
    fn invisible_characters_are_stripped_from_names() {
        for c in [
            '\u{2028}',
            '\u{2029}',
            '\u{200b}',
            '\u{200d}',
            '\u{feff}',
            '\u{00ad}',
            '\u{2060}',
            '\u{2062}',
            '\u{e0041}',
            '\u{e0001}',
            '\u{e007f}',
            '\u{2800}',
            '\u{3164}',
            '\u{202e}',
            '\u{0085}',
            '\u{0007}',
            '\u{180e}',
        ] {
            assert_eq!(sanitize_name(&format!("a{c}b")), "ab", "U+{:04X}", c as u32);
        }
        // Ordinary text, including emoji and combining marks, is kept.
        assert_eq!(
            sanitize_name("Zeugnis \u{1f4c4} é.pdf"),
            "Zeugnis \u{1f4c4} é.pdf"
        );
        // A name that is only invisible characters is refused.
        let only = r#"{"kind":"image","name":"\u200b\u2028\ufeff","mime":"image/png","size":3}"#;
        assert_eq!(parse(only), Err(ParseError::InvalidFields));
    }

    #[test]
    fn name_length_counts_characters() {
        // 255 characters pass, also when they take 4 bytes (or 2 UTF-16 units) each.
        let t = |n: &str| format!(r#"{{"kind":"image","name":"{n}","mime":"image/png","size":3}}"#);
        assert!(parse(&t(&"\u{1f4c4}".repeat(255))).is_ok());
        assert!(parse(&t(&"\u{1f4c4}".repeat(256))).is_err());
        assert!(parse(&t(&"é".repeat(255))).is_ok());
        assert!(parse(&t(&"é".repeat(256))).is_err());
    }

    #[test]
    fn a_link_that_grows_when_normalised_is_refused_or_fits() {
        // 1500 x 'é' is 1500 characters long but 6000 bytes after percent-encoding.
        let long = format!("https://a.org/{}", "é".repeat(1500));
        assert_eq!(validate_link(&long), None);
        // Short enough after normalising: accepted, and the result is accepted unchanged.
        let ok = format!("https://a.org/{}", "é".repeat(300));
        let n = validate_link(&ok).expect("1800 bytes + prefix fit");
        assert!(n.len() <= MAX_LINK_LENGTH);
        assert_eq!(validate_link(&n).as_deref(), Some(n.as_str()));
        // Exactly at the limit after normalising.
        let fill = MAX_LINK_LENGTH - "https://a.org/".len();
        let at = format!(
            "https://a.org/{}",
            "é".repeat(fill / 6) + &"x".repeat(fill % 6)
        );
        assert_eq!(validate_link(&at).map(|u| u.len()), Some(MAX_LINK_LENGTH));
        let over = format!("{at}x");
        assert_eq!(validate_link(&over), None);
        // Whatever passes is idempotent and fits a message the receiver accepts.
        for s in [
            "https://example.org/a b",
            "http://Example.ORG:80/%7Efoo?x=\u{e9}#\u{e9}",
            "https://example.org\\@evil.example",
            "https://bücher.example/ü?ü#ü",
            "https://a.org/#\\\\\\\\",
        ] {
            if let Some(n) = validate_link(s) {
                assert_eq!(validate_link(&n).as_deref(), Some(n.as_str()), "{s}");
                let json = Message::Link { url: n }.to_json();
                assert!(parse(&json).is_ok(), "{s}");
            }
        }
        // A fragment full of backslashes doubles in JSON but still fits 8192 characters.
        let bs = format!("https://a.org/#{}", "\\".repeat(2000));
        let n = validate_link(&bs).expect("backslashes are kept in a fragment");
        assert!(Message::Link { url: n }.to_json().chars().count() <= MAX_TEXT_CHARS);
    }

    #[test]
    fn refused_starts_report_their_announced_size() {
        let a = announced_transfer(r#"{"kind":"document","name":"a","mime":"x","size":9000000}"#);
        assert_eq!(a, Some((TransferKind::Document, 9_000_000)));
        let a = announced_transfer(r#"{"kind":"image","size":5}"#);
        assert_eq!(a, Some((TransferKind::Image, 5)));
        for bad in [
            "",
            "nope",
            r#"{"kind":"link","size":5}"#,
            r#"{"kind":"image","size":0}"#,
            r#"{"kind":"image","size":-3}"#,
            r#"{"kind":"image","size":1.5}"#,
            r#"{"kind":"image"}"#,
        ] {
            assert_eq!(announced_transfer(bad), None, "{bad}");
        }
    }

    #[test]
    fn extension_guess() {
        assert_eq!(ImageMime::from_extension("a.PNG"), Some(ImageMime::Png));
        assert_eq!(ImageMime::from_extension("a.jpeg"), Some(ImageMime::Jpeg));
        assert_eq!(ImageMime::from_extension("a.svg"), None);
        assert_eq!(ImageMime::from_extension("noext"), None);
    }
}
