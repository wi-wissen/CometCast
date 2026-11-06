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

About PDF rendering: a received PDF is hostile input and is rendered by hayro, an experimental renderer, in a **resource-limited child process** (memory, CPU, time, process-group kill, empty environment). That is **not a sandbox by itself**: the child runs as the same user. On Linux the child additionally restricts itself with Landlock (no file system, no TCP) and a seccomp allowlist (no `fork`, `exec`, `socket`) where the kernel allows it; on other systems, and on Linux kernels or containers without them, a code-execution bug in the renderer would equal a compromise of the user's session. Which protections were active is reported by `Document::protections()`. Details and limits: [docs/documents.md](docs/documents.md). Reports about escapes from this worker are in scope.

Out of scope: weaknesses of third-party products that we only use or point to (report them to their authors), and platform limits listed in [docs/measurement-points.md](docs/measurement-points.md).

## Supported versions

Only the latest commit on the default branch is supported while the project is in beta.
