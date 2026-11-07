# SPDX-License-Identifier: GPL-3.0-or-later
#
# Comet Cast server image: cometcast-server plus the web app (static files).
# Build from the REPOSITORY ROOT:
#   docker build -f deploy/Dockerfile -t cometcast-server:local .
#
# First real build and run: 2026-11-07, Docker Engine 29.9.2 (BuildKit), linux/amd64 only.
# Built and run OK after the fixes listed in docs/server.md ("First real run"): image size
# about 43 MB (11 MB compressed content), runs as UID 65532, health check works, read-only root
# file system works. Not run: other architectures (arm64), a build without a proxy.
# The web stage does NOT build the WASM PDF renderer (apps/web/public/pdf): PDF display in the
# web app is therefore missing in this image (see apps/web/README.md).

# ---- Stage 1: web app (Vue, built to plain files) ----
# Tag node:22-bookworm-slim exists and `npm ci` + `npm run build` worked (run 2026-11-07).
FROM node:22-bookworm-slim AS web
WORKDIR /src/apps/web
COPY apps/web/package.json apps/web/package-lock.json ./
# Optional extra CA bundle (only for builds behind a TLS-intercepting proxy):
#   docker build --secret id=extra_ca,src=/path/ca.pem ...   (never stored in the image)
RUN --mount=type=secret,id=extra_ca,required=false \
    if [ -s /run/secrets/extra_ca ]; then export NODE_EXTRA_CA_CERTS=/run/secrets/extra_ca; fi; \
    npm ci
COPY apps/web/ ./
# Branding (publisher and legal links, branding/README.md): vite.config.ts reads ../../tools/branding.mjs and the file
# named by COMETCAST_BRANDING (a path below the repository root, e.g. branding/cometcast.json); empty = branding/default.json.
#   docker build --build-arg COMETCAST_BRANDING=branding/cometcast.json ...
COPY tools/branding.mjs /src/tools/branding.mjs
COPY branding/ /src/branding/
ARG COMETCAST_BRANDING=
RUN COMETCAST_BRANDING="$COMETCAST_BRANDING" npm run build

# ---- Stage 2: server binary ----
# The workspace needs Rust >= 1.92 (rust-version in Cargo.toml). First run: built with 1.99.1
# (rust:1-bookworm on 2026-11-07). The image tag is pinned to a minor release for repeatable builds.
FROM rust:1.99-bookworm AS build
WORKDIR /src
COPY . .
# rust-toolchain.toml says channel = "stable". rustup would then try to DOWNLOAD a toolchain called
# "stable" (the image only has the numbered one) and the build fails. Use the image's toolchain.
RUN rm -f rust-toolchain.toml
# --locked: use exactly the versions in Cargo.lock.
# The optional extra_ca secret is only for builds behind a TLS-intercepting proxy (see above).
RUN --mount=type=secret,id=extra_ca,required=false \
    if [ -s /run/secrets/extra_ca ]; then export CARGO_HTTP_CAINFO=/run/secrets/extra_ca SSL_CERT_FILE=/run/secrets/extra_ca; fi; \
    cargo build --release --locked -p cometcast-server

# ---- Stage 3: small runtime image ----
# distroless "cc" has glibc and nothing else (no shell, no package manager). It runs as
# a non-root user (UID 65532) with the ":nonroot" tag. The tag exists and the binary runs on it.
FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /src/target/release/cometcast-server /usr/local/bin/cometcast-server
COPY --from=web /src/apps/web/dist /srv/cometcast/web
# Inside a container the server must listen on all interfaces. The config file can
# still override this.
ENV COMETCAST_LISTEN=0.0.0.0:8443 \
    COMETCAST_WEB_DIR=/srv/cometcast/web
EXPOSE 8443
USER nonroot
# No shell and no curl in this image: the server binary checks itself (GET /healthz on loopback,
# TLS certificate not verified for that local check). Uses the same config file as the server.
HEALTHCHECK --interval=30s --timeout=6s --start-period=10s --retries=3 \
  CMD ["/usr/local/bin/cometcast-server", "--healthcheck", "--config", "/etc/cometcast/server.toml"]
ENTRYPOINT ["/usr/local/bin/cometcast-server"]
# Mount your config at /etc/cometcast/server.toml and run with:
#   --config /etc/cometcast/server.toml
CMD ["--config", "/etc/cometcast/server.toml"]
