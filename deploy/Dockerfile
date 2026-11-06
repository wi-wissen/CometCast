# SPDX-License-Identifier: GPL-3.0-or-later
#
# Comet Cast server image: cometcast-server plus the web app (static files).
# Build from the REPOSITORY ROOT:
#   docker build -f deploy/Dockerfile -t cometcast-server:local .
#
# UNCERTAIN: this file has NOT been built or run. The machine it was written on has the
# Docker command line but no Docker daemon. Image tags, the build steps and the
# distroless runtime (glibc, no shell) are untested assumptions. Check them first.

# ---- Stage 1: web app (Vue, built to plain files) ----
# UNCERTAIN: tag is an assumption (apps/web is built with Node 22 in this repository's tests).
FROM node:22-bookworm-slim AS web
WORKDIR /src/apps/web
COPY apps/web/package.json apps/web/package-lock.json ./
RUN npm ci
COPY apps/web/ ./
RUN npm run build

# ---- Stage 2: server binary ----
# The newest stable Rust is used by the project (rust-toolchain.toml says "stable").
# UNCERTAIN: "rust:1-bookworm" is an assumed tag; pin an exact version for real releases.
FROM rust:1-bookworm AS build
WORKDIR /src
COPY . .
# --locked: use exactly the versions in Cargo.lock.
RUN cargo build --release --locked -p cometcast-server

# ---- Stage 3: small runtime image ----
# distroless "cc" has glibc and nothing else (no shell, no package manager). It runs as
# a non-root user (UID 65532) with the ":nonroot" tag.
# UNCERTAIN: tag is an assumption.
FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /src/target/release/cometcast-server /usr/local/bin/cometcast-server
COPY --from=web /src/apps/web/dist /srv/cometcast/web
# Inside a container the server must listen on all interfaces. The config file can
# still override this.
ENV COMETCAST_LISTEN=0.0.0.0:8443 \
    COMETCAST_WEB_DIR=/srv/cometcast/web
EXPOSE 8443
USER nonroot
ENTRYPOINT ["/usr/local/bin/cometcast-server"]
# Mount your config at /etc/cometcast/server.toml and run with:
#   --config /etc/cometcast/server.toml
CMD ["--config", "/etc/cometcast/server.toml"]
