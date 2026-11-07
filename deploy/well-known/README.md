# App link files

`assetlinks.json` (Android App Links) and `apple-app-site-association` (iOS Universal Links) belong at
`https://cometcast.de/.well-known/`: the website serves them (`.github/workflows/pages.yml` stages this
folder as `/.well-known/`). The apps open `https://cometcast.de/app/{CODE}` (native-apps.md 20.4, 21.6);
the relay host `relay.cometcast.de` serves no web app and no verification file (the old Android filter
for `relay.cometcast.de/{CODE}` may stay in `AndroidManifest.xml` until that host is retired and needs
`assetlinks.json` there too). Serve both with `Content-Type: application/json`, over https, without
redirects.

Both files hold a placeholder on purpose, so that the verification fails safely until the real values are
put in. Nothing here is a secret:

- `sha256_cert_fingerprints`: the SHA-256 fingerprint of the certificate that signs the release app
  (`keytool -list -v -keystore ...`; with Play App Signing the fingerprint of the app signing key shown in
  the Play Console).
- `appIDs`: `{TEAM ID}.de.cometcast.cometcast` (bundle id of `apps/flutter/ios/Config/CometCast.xcconfig`).

The paths match `/app/` plus exactly six characters (`/app/......` in `AndroidManifest.xml`,
`/app/??????` here): the room code. `/app/display` and every other path stay in the browser. The
questions this left open are in `docs/design/implementation-questions-flutter.md`, section E.
