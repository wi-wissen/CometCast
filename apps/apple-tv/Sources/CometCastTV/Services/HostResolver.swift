// SPDX-License-Identifier: GPL-3.0-or-later
// UNCERTAIN: never compiled (no Xcode here).
//
// The IPv4 address of an in-house box (spec 4.2, decision N40): the idle screen shows it in `idle.altAddress` and puts
// it into the QR code, when the display's server came from a box announcement and `{dom}` is a host name.

import Foundation

enum HostResolver {
    /// The first IPv4 address of `host`, or nil. Runs off the main actor (getaddrinfo blocks).
    static func ipv4(_ host: String) async -> String? {
        await Task.detached(priority: .utility) { () -> String? in
            var hints = addrinfo()
            hints.ai_family = AF_INET
            hints.ai_socktype = SOCK_STREAM
            var res: UnsafeMutablePointer<addrinfo>?
            guard getaddrinfo(host, nil, &hints, &res) == 0, let first = res else { return nil }
            defer { freeaddrinfo(res) }
            guard let sa = first.pointee.ai_addr else { return nil }
            var addr = sa.withMemoryRebound(to: sockaddr_in.self, capacity: 1) { $0.pointee.sin_addr }
            var buf = [CChar](repeating: 0, count: Int(INET_ADDRSTRLEN))
            guard inet_ntop(AF_INET, &addr, &buf, socklen_t(INET_ADDRSTRLEN)) != nil else { return nil }
            return String(cString: buf)
        }.value
    }
}
