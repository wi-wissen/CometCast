# Security policy

Comet Cast is in an early beta (v0.1) and nothing works end to end yet. Reports are still welcome.

## Reporting a vulnerability

Please do **not** open a public issue for a security problem.

Use GitHub's private vulnerability reporting: open the **Security** tab of this repository and choose **Report a vulnerability**. If that option is not shown, open an issue that says only that you need a private channel, without any details, and a maintainer will respond.

Please include:

- What is affected (component, version or commit).
- Steps to reproduce.
- What an attacker could do.

We will acknowledge a report as soon as we can and keep you informed. We have no fixed response times yet.

## Scope

Examples that are in scope: the room-code and server brokering logic, the open server's overload protection, handling of received files and documents, and the transport between devices.

Out of scope: weaknesses of third-party products that we only use or point to (report them to their authors), and platform limits listed in [docs/measurement-points.md](docs/measurement-points.md).

## Supported versions

Only the latest commit on the default branch is supported while the project is in beta.
