// SPDX-License-Identifier: GPL-3.0-or-later
// Mirrors RoomCode::ALPHABET in crates/cc-proto (32 characters; no 0, O, 1, I) and apps/web/src/core/roomCode.ts.

public let roomCodeAlphabet = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789"
public let roomCodeLength = 6

/// Upper case, drop characters outside the alphabet, cut to length.
public func normalizeRoomCode(_ input: String) -> String {
    String(input.uppercased().filter { roomCodeAlphabet.contains($0) }.prefix(roomCodeLength))
}

public func isValidRoomCode(_ code: String) -> Bool {
    code.count == roomCodeLength && code.allSatisfy { roomCodeAlphabet.contains($0) }
}

/// "ABC234" -> "ABC 234" for the big display.
public func displayRoomCode(_ code: String) -> String {
    guard code.count == roomCodeLength else { return code }
    return "\(code.prefix(3)) \(code.suffix(3))"
}
