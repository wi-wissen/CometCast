# Documentation

| Page | Content |
| --- | --- |
| [measurement-points.md](measurement-points.md) | The 17 beta assumptions, the ten measurement points and the measurement spikes. Most important page: it says what is unverified |
| [architecture.md](architecture.md) | Layers, Rust core crates, platform apps, server, route ladder, decisions, milestones |
| [protocol.md](protocol.md) | The signalling protocol between sender, receiver and server |
| [server.md](server.md) | Operating `cometcast-server`: configuration, open and internal mode, TLS, limits, ALTCHA, TURN, deployment |
| [transport.md](transport.md) | The native WebRTC path (`cc-transport`, receiver and sender cores, CLI): what works and what does not |
| [web-m2.md](web-m2.md) | Web app toolbar, image and link transfer over the control DataChannel (message format, limits) |
| [flutter-bridge.md](flutter-bridge.md) | How the Flutter app talks to the Rust core (flutter_rust_bridge), codegen, build integration |
| [airplay.md](airplay.md) | AirPlay receiving through the separate UxPlay process (`cc-airplay`): supervisor, policy, licence boundary, what is unverified |
| [i18n.md](i18n.md) | How the bilingual (de/en) UI works |

Other project files: [README](../README.md), [CONTRIBUTING](../CONTRIBUTING.md), [CLA](../CLA.md), [SECURITY](../SECURITY.md), [THIRD_PARTY_NOTICES](../THIRD_PARTY_NOTICES.md).

Docs are English. Unsure statements are written as "**Uncertain:** ..." and linked to the measurement points.
