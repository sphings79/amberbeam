# AmberBeam as a container.
#
# Three stages: the interface, the service, and an image holding only what is
# needed to run. The build stages carry a Rust toolchain and a Node install;
# neither has any business being on a machine that is only meant to serve
# files, and shipping them would mean shipping every hole either of them ever
# has.
#
# What comes out is one binary, the built interface, and the few libraries the
# binary actually links against.

# --- The interface ---------------------------------------------------------
#
# Built with AMBERBEAM_SHELL=web, which is the whole of what makes it the
# container build: the second seam picks the HTTP bridge instead of Tauri's,
# and nothing else in the interface changes or knows.
FROM node:22-bookworm-slim AS web
WORKDIR /build
COPY package.json package-lock.json ./
RUN npm ci
COPY . .
ENV AMBERBEAM_SHELL=web
RUN npm run build

# --- The service -----------------------------------------------------------
FROM rust:1-bookworm AS service
WORKDIR /build
# The manifests first, so a change to the source does not throw away the
# dependency build. Cargo wants the crates to exist before it will read a
# workspace, hence the empty sources.
COPY Cargo.toml Cargo.lock ./
COPY crates/amberbeam-core/Cargo.toml crates/amberbeam-core/
COPY crates/amberbeam-commands/Cargo.toml crates/amberbeam-commands/
COPY crates/amberbeam-serve/Cargo.toml crates/amberbeam-serve/
COPY crates/amberbeam-mcp/Cargo.toml crates/amberbeam-mcp/
COPY src-tauri/Cargo.toml src-tauri/
RUN mkdir -p crates/amberbeam-core/src crates/amberbeam-commands/src \
      crates/amberbeam-serve/src crates/amberbeam-mcp/src src-tauri/src \
    && echo "" > crates/amberbeam-core/src/lib.rs \
    && echo "" > crates/amberbeam-commands/src/lib.rs \
    && echo "fn main() {}" > crates/amberbeam-serve/src/main.rs \
    && echo "" > crates/amberbeam-mcp/src/lib.rs \
    && echo "fn main() {}" > crates/amberbeam-mcp/src/main.rs \
    && echo "" > src-tauri/src/lib.rs \
    && echo "fn main() {}" > src-tauri/src/main.rs \
    # Only these two. The desktop shell wants a webview and a windowing
    # system, neither of which belongs in a container, and building it here
    # would pull both in to throw both away.
    && (cargo build --release -p amberbeam-serve; cargo build --release -p amberbeam-mcp; true)

COPY crates crates
COPY Cargo.toml Cargo.lock ./
# Touched so cargo does not believe the placeholder build is still current.
#
# One after the other, not both in one command: each binary is linked with
# LTO in a single codegen unit, and cargo runs those two links at the same
# time. Two of them at once needs more memory than a small Docker machine has
# -- here, 1.9 GB, where the linker was killed outright. Sequentially each one
# fits, and the build only takes longer.
RUN touch crates/*/src/lib.rs crates/amberbeam-serve/src/main.rs \
      crates/amberbeam-mcp/src/main.rs \
    && cargo build --release -p amberbeam-serve \
    && cargo build --release -p amberbeam-mcp

# --- What actually runs ----------------------------------------------------
FROM debian:bookworm-slim
# ca-certificates for FTPS and for asking GitHub about updates. Nothing else:
# every package here is a package somebody has to keep patched.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    # Not root. A program whose entire job is reading and writing files should
    # not be the user that may write all of them.
    && useradd --create-home --uid 10001 --shell /usr/sbin/nologin amberbeam \
    && mkdir -p /data /config /web \
    && chown -R amberbeam:amberbeam /data /config

COPY --from=service /build/target/release/amberbeam-serve /usr/local/bin/amberbeam-serve
# The same core again, spoken to by a program rather than by a person. It is
# not started by the image: a client on somebody's own machine runs it with
# `docker exec -i`, and it inherits this container's environment -- which is
# how the passphrase reaches it and the saved passwords open.
COPY --from=service /build/target/release/amberbeam-mcp /usr/local/bin/amberbeam-mcp
COPY --from=web /build/dist /web

USER amberbeam
# The local side starts wherever HOME points. A starting point, not a fence —
# the container page in the documentation says so plainly.
ENV HOME=/data \
    AMBERBEAM_CONFIG=/config \
    AMBERBEAM_WEB=/web \
    AMBERBEAM_ADDRESS=0.0.0.0:2122
WORKDIR /data
EXPOSE 2122
VOLUME ["/data", "/config"]

ENTRYPOINT ["/usr/local/bin/amberbeam-serve"]
