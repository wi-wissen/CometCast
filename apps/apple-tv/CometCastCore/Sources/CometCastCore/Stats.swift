// SPDX-License-Identifier: GPL-3.0-or-later
//
// Turns WebRTC statistics (the W3C `RTCStatsReport` entries, as the app copies them out of the library) into the
// few numbers the on-screen statistics and the device test plan need: picture size, frame rate, frames decoded and
// dropped, codec, decoder implementation, bit rate, and the route (direct on the same network, direct through NAT,
// or relayed through TURN). Pure: the app only converts the library's objects into `RtcStat`.

import Foundation

public struct RtcStat: Equatable, Sendable {
    public var id: String
    public var type: String
    public var values: [String: JSONValue]
    public init(id: String, type: String, values: [String: JSONValue]) { self.id = id; self.type = type; self.values = values }
}

public enum StatsRoute: String, Equatable, Sendable {
    /// Both candidates are `host`: same network.
    case host
    /// Direct, with at least one server reflexive or peer reflexive candidate (through NAT).
    case srflx
    /// A TURN relay is in the path.
    case relay
    case unknown
}

public struct StatsSummary: Equatable, Sendable {
    public var width = 0, height = 0
    public var framesPerSecond: Double?
    public var framesDecoded = 0, framesDropped = 0, keyFramesDecoded = 0
    public var bytesReceived: Double = 0
    public var packetsLost = 0
    public var jitterMs: Double?
    public var codec: String?
    public var decoder: String?
    public var route: StatsRoute = .unknown
    public var transport: String?
    public var roundTripMs: Double?
    public init() {}
}

/// The summary of the first video `inbound-rtp` entry and the nominated (or succeeded) candidate pair.
public func summarizeStats(_ stats: [RtcStat]) -> StatsSummary {
    var s = StatsSummary()
    let byId = Dictionary(stats.map { ($0.id, $0) }, uniquingKeysWith: { a, _ in a })
    func num(_ v: [String: JSONValue], _ k: String) -> Double? { v[k]?.numberValue }
    func str(_ v: [String: JSONValue], _ k: String) -> String? { v[k]?.stringValue }
    if let rtp = stats.first(where: { $0.type == "inbound-rtp" && ($0.values["kind"]?.stringValue ?? $0.values["mediaType"]?.stringValue) == "video" }) {
        let v = rtp.values
        s.width = Int(num(v, "frameWidth") ?? 0); s.height = Int(num(v, "frameHeight") ?? 0)
        s.framesPerSecond = num(v, "framesPerSecond")
        s.framesDecoded = Int(num(v, "framesDecoded") ?? 0); s.framesDropped = Int(num(v, "framesDropped") ?? 0)
        s.keyFramesDecoded = Int(num(v, "keyFramesDecoded") ?? 0)
        s.bytesReceived = num(v, "bytesReceived") ?? 0
        s.packetsLost = Int(num(v, "packetsLost") ?? 0)
        s.jitterMs = num(v, "jitter").map { $0 * 1000 }
        s.decoder = str(v, "decoderImplementation")
        if let cid = str(v, "codecId"), let c = byId[cid] { s.codec = str(c.values, "mimeType") }
    }
    let pairs = stats.filter { $0.type == "candidate-pair" }
    let pair = pairs.first(where: { $0.values["nominated"]?.boolValue == true && str($0.values, "state") == "succeeded" })
        ?? pairs.first(where: { str($0.values, "state") == "succeeded" })
    if let p = pair {
        s.roundTripMs = num(p.values, "currentRoundTripTime").map { $0 * 1000 }
        let l = str(p.values, "localCandidateId").flatMap { byId[$0] }
        let r = str(p.values, "remoteCandidateId").flatMap { byId[$0] }
        let lt = l.flatMap { str($0.values, "candidateType") }, rt = r.flatMap { str($0.values, "candidateType") }
        s.transport = l.flatMap { str($0.values, "protocol") }
        if lt == "relay" || rt == "relay" { s.route = .relay }
        else if let lt, let rt { s.route = (lt == "host" && rt == "host") ? .host : .srflx }
    }
    return s
}

/// Bit rate in kbit/s from two byte counters `seconds` apart.
public func bitrateKbps(previousBytes: Double, currentBytes: Double, seconds: Double) -> Double? {
    guard seconds > 0, currentBytes >= previousBytes else { return nil }
    return (currentBytes - previousBytes) * 8 / 1000 / seconds
}
