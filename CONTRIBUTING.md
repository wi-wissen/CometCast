# Contributing to Comet Cast

Thank you for helping. Please read this page before opening a pull request.

## Language

- **Everything in the repository is English**: source code, comments, file names, README and docs, commit messages, issues and pull requests.
- **The user interface is bilingual (German and English).** UI text goes through translation keys, never hard-coded strings. The default language follows the system language. See [docs/i18n.md](docs/i18n.md).

## Commit messages

English, short, imperative. A Conventional-Commits-like prefix is suggested:

```
feat(cc-proto): add room-code message
fix(web): handle missing getDisplayMedia
docs: clarify server discovery order
```

Common prefixes: `feat`, `fix`, `docs`, `refactor`, `test`, `build`, `chore`.

## Marking unsure places

Never state an assumption as a fact. If you are not sure, mark it and link it to [docs/measurement-points.md](docs/measurement-points.md):

- In code: `// UNCERTAIN: <what is unsure> (see docs/measurement-points.md#a3)`
- In docs: `**Uncertain:** <what is unsure>` with a link to the matching row.

If the assumption has no row yet, add one (rules at the top of that file). Use the words "sourced", "assumption" and "unchecked" for the status.

## Hard rules: what we do not do

- Do not extract keys or certificates from devices, apps or firmware.
- Do not add FairPlay, Google Cast, AirServer or AirParrot material (leaked code, keys, extracted binaries or certificates).
- Do not circumvent DRM.
- Do not copy code from AGPL projects such as RustDesk. Ideas, described in your own words, are fine.
- UxPlay (GPL) is used only as a separate process, never linked or copied into our code.

A pull request that breaks these rules will be closed.

## Running checks

Rust:

```sh
cargo build --workspace
cargo test --workspace
```

Web app:

```sh
cd apps/web && npm ci && npm run dev
```

Translations: run the check script `node tools/check-i18n.mjs` (it checks that the English and German key sets match, see [docs/i18n.md](docs/i18n.md)).

Run the checks that apply to the part you changed before you open a pull request.

## Licence headers

Every source file gets an SPDX header with the licence and a copyright line. Use the comment style of the language:

```rust
// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (c) Comet Cast contributors
```

The exact copyright line is to be confirmed with the steward; keep it as shown until then.

## Contributor License Agreement (CLA)

Contributions from third parties are accepted under the [CLA](CLA.md). The CLA lets Schulverwalter GmbH (the steward) distribute the apps, for example in app stores, while the public project stays GPL-3.0-or-later.

For now, you accept it by ticking the checkbox in the pull request template ("I have read and agree to CLA.md"). Whether to add an automated CLA bot is an open decision; there is none at the moment.

The CLA text is currently a draft and needs legal review before the first external contribution is accepted.

## Pull requests

1. Keep a pull request to one topic.
2. Fill in the pull request template.
3. Say how you tested it, or that you could not.
4. Be kind: see the [Code of Conduct](CODE_OF_CONDUCT.md).

Security problems: do not open a public issue, see [SECURITY.md](SECURITY.md).
