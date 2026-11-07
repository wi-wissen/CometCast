# App link files of the relay

`assetlinks.json` (Android App Links) and `apple-app-site-association` (iOS Universal Links) belong at
`https://{relay}/.well-known/` of the **built-in relay's host** (`relay.cometcast.de`, the name planned in
`crates/cc-discovery`); other hosts stay browser links (native-apps.md 20.4). Serve both with
`Content-Type: application/json`, over https, without redirects.

Both files hold a placeholder on purpose, so that the verification fails safely until the real values are
put in. Nothing here is a secret:

- `sha256_cert_fingerprints`: the SHA-256 fingerprint of the certificate that signs the release app
  (`keytool -list -v -keystore ...`; with Play App Signing the fingerprint of the app signing key shown in
  the Play Console).
- `appIDs`: `{TEAM ID}.de.cometcast.cometcast` (bundle id of `apps/flutter/ios/Config/CometCast.xcconfig`).

The paths match exactly six characters (`/......` in `AndroidManifest.xml`, `/??????` here): the room code.
The questions this left open are in `docs/design/implementation-questions-flutter.md`, section E.
