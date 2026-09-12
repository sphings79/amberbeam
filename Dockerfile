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
COPY src-tauri/Cargo.toml src-tauri/
RUN mkdir -p crates/amberbeam-core/src crates/amberbeam-commands/src \
      crates/amberbeam-serve/src src-tauri/src \
    && echo "" > crates/amberbeam-core/src/lib.rs \
    && echo "" > crates/amberbeam-commands/src/lib.rs \
    && echo "fn main() {}" > crates/amberbeam-serve/src/main.rs \
    && echo "" > src-tauri/src/lib.rs \
    && echo "fn main() {}" > src-tauri/src/main.rs \
    # Only the one crate. The desktop shell wants a webview and a windowing
    # system, neither of which belongs in a container, and building it here
    # would pull both in to throw both away.
    && cargo build --release -p amberbeam-serve || true

COPY crates crates
COPY Cargo.toml Cargo.lock ./
# Touched so cargo does not believe the placeholder build is still current.
RUN touch crates/*/src/lib.rs crates/amberbeam-serve/src/main.rs \
    && cargo build --release -p amberbeam-serve

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
