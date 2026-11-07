// SPDX-License-Identifier: GPL-3.0-or-later
//
// The licence list of About (spec 15.9 "Content sources", native-apps.md 21.7): entries `{name, licence, text,
// featured}` built from the bundled copies of THIRD_PARTY_NOTICES.md and THIRD_PARTY_LICENSES.md. Parsing is
// done once, off the UI thread (the app starts a background task); the list is lazy in the view.
//
// Rules (spec 15.9):
//   * featured, in this order: the notice sections of the parts that have their own notice (Cisco OpenH264,
//     Atkinson Hyperlegible, then the others in file order); parts that this app does not ship are left out
//     (Open Screen Library: "on desktop builds"; UxPlay: "where UxPlay is shipped"; the Office fonts: "where the
//     Office worker is shipped");
//   * all: every `####` heading under "Rust licence texts" and under "Flutter licence texts" (its text is the
//     fenced block below it) and every row of the npm table as name + SPDX identifier, whose text is the SPDX line;
//   * the licence (subtitle) of a heading entry is the first non-empty line of its text, at most 60 characters.
// Lines inside a fenced block are text, never headings (the licence texts contain lines starting with `## `).

import Foundation

public struct LicenceEntry: Equatable, Sendable {
    public var name: String
    public var licence: String
    public var text: String
    public var featured: Bool
    public init(name: String, licence: String, text: String, featured: Bool) {
        self.name = name; self.licence = licence; self.text = text; self.featured = featured
    }
}

/// A notice section that is featured: its heading in THIRD_PARTY_NOTICES.md (prefix) and the name shown.
public struct FeaturedNotice: Equatable, Sendable {
    public var heading: String
    public var name: String?
    public init(_ heading: String, name: String? = nil) { self.heading = heading; self.name = name }
}

/// The Apple TV build: OpenH264 and Atkinson first (the order of the spec), then the other notices of the spec's
/// list in file order. Open Screen Library (desktop builds), UxPlay and the Office fonts are not shipped here.
public let appleTvFeaturedNotices: [FeaturedNotice] = [
    FeaturedNotice("OpenH264 notice", name: "Cisco OpenH264"),
    FeaturedNotice("Atkinson Hyperlegible notice", name: "Atkinson Hyperlegible"),
    FeaturedNotice("PDFium Foxit font notice"),
    FeaturedNotice("Adobe CMap notice"),
]

private struct Section {
    var level: Int
    var heading: String
    var lines: [Substring]
}

/// Splits markdown into sections by `#` headings (levels 2 to 4) outside fenced blocks. Text before the first
/// heading is dropped.
private func sections(_ md: String) -> [Section] {
    var out: [Section] = []
    var inFence = false
    for line in md.split(separator: "\n", omittingEmptySubsequences: false) {
        if line.hasPrefix("```") { inFence.toggle(); if !out.isEmpty { out[out.count - 1].lines.append(line) }; continue }
        if !inFence, line.hasPrefix("##") {
            let hashes = line.prefix(while: { $0 == "#" }).count
            if (2...4).contains(hashes), line.dropFirst(hashes).first == " " {
                out.append(Section(level: hashes, heading: String(line.dropFirst(hashes + 1)), lines: []))
                continue
            }
        }
        if !out.isEmpty { out[out.count - 1].lines.append(line) }
    }
    return out
}

/// The text of a section: the content of its fenced block when `fencedOnly` and it has one, else its body, trimmed.
private func bodyText(_ s: Section, fencedOnly: Bool) -> String {
    var fenced: [Substring] = []
    var inFence = false
    var sawFence = false
    for l in s.lines {
        if l.hasPrefix("```") { inFence.toggle(); sawFence = true; continue }
        if inFence { fenced.append(l) }
    }
    let lines = fencedOnly && sawFence ? fenced : s.lines
    return lines.joined(separator: "\n").trimmingCharacters(in: .whitespacesAndNewlines)
}

private func firstLine(_ text: String, limit: Int = 60) -> String {
    let l = text.split(separator: "\n", omittingEmptySubsequences: true)
        .map { $0.trimmingCharacters(in: .whitespaces) }.first { !$0.isEmpty } ?? ""
    return String(l.prefix(limit))
}

/// Builds the list: featured entries first, then all. `licences` is THIRD_PARTY_LICENSES.md, `notices` is
/// THIRD_PARTY_NOTICES.md.
public func parseLicences(notices: String, licences: String, featured: [FeaturedNotice] = appleTvFeaturedNotices) -> [LicenceEntry] {
    var out: [LicenceEntry] = []
    let noticeSections = sections(notices).filter { $0.level == 2 }
    for f in featured {
        guard let s = noticeSections.first(where: { $0.heading.hasPrefix(f.heading) }) else { continue }
        let text = bodyText(s, fencedOnly: false)
        out.append(LicenceEntry(name: f.name ?? s.heading, licence: firstLine(text), text: text, featured: true))
    }
    var rustOrFlutter = false
    var npm = false
    for s in sections(licences) {
        if s.level <= 3 {
            rustOrFlutter = s.heading.hasPrefix("Rust licence texts") || s.heading.hasPrefix("Flutter licence texts")
            npm = s.level == 2 && s.heading.hasPrefix("npm packages")
            if npm { out.append(contentsOf: npmEntries(s)) }
            continue
        }
        if rustOrFlutter {
            let text = bodyText(s, fencedOnly: true)
            out.append(LicenceEntry(name: s.heading, licence: firstLine(text), text: text, featured: false))
        }
    }
    return out
}

/// `| Package | Version | Licence |` rows: name + SPDX identifier, the text is the SPDX line.
private func npmEntries(_ s: Section) -> [LicenceEntry] {
    var out: [LicenceEntry] = []
    for l in s.lines where l.hasPrefix("|") {
        let cells = l.split(separator: "|", omittingEmptySubsequences: false).dropFirst().dropLast()
            .map { $0.trimmingCharacters(in: .whitespaces) }
        guard cells.count >= 3, cells[0] != "Package", !cells[0].hasPrefix("---"), !cells[0].isEmpty else { continue }
        out.append(LicenceEntry(name: cells[0], licence: cells[2], text: cells[2], featured: false))
    }
    return out
}
