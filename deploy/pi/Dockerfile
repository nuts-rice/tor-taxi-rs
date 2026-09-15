# Cross-compiled, never emulated: the builder is pinned to $BUILDPLATFORM so
# rustc runs natively on x86 and only the linker targets aarch64. Only the small
# runtime stage below is emulated.
ARG RUST_VERSION=1.91
# Builder and runtime share a suite on purpose: the binary is dynamically linked
# against glibc, so a newer builder ships symbols the Pi cannot resolve.
ARG DEBIAN_SUITE=trixie

FROM --platform=$BUILDPLATFORM rust:${RUST_VERSION}-${DEBIAN_SUITE} AS build

# One apt package because rustls resolves to ring here, not aws-lc-rs.
RUN apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
      gcc-aarch64-linux-gnu \
      libc6-dev-arm64-cross && \
    rm -rf /var/lib/apt/lists/*

RUN rustup target add aarch64-unknown-linux-gnu

# cargo reads the first; the CC_/AR_ pair is for ring's build script, which
# shells out to a C toolchain directly and ignores cargo's config.
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc \
    AR_aarch64_unknown_linux_gnu=aarch64-linux-gnu-ar

WORKDIR /build
COPY . .

# The cp runs inside this same RUN because target/ is a cache mount: it is gone
# the instant the layer commits.
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/build/target,sharing=locked \
    cargo build --release --locked \
      --target aarch64-unknown-linux-gnu \
      -p tor-taxi-checker && \
    cp target/aarch64-unknown-linux-gnu/release/tor-taxi-checker /tor-taxi-checker


FROM debian:${DEBIAN_SUITE}-slim

# No ca-certificates: hyper-rustls compiles webpki-roots into the binary, so the
# D1 call carries its own trust store, and probes set danger_accept_invalid_certs.
RUN apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
      tor && \
    rm -rf /var/lib/apt/lists/*

COPY --from=build /tor-taxi-checker  /usr/local/bin/tor-taxi-checker
COPY deploy/pi/entrypoint.sh         /usr/local/bin/entrypoint.sh
COPY deploy/pi/healthcheck.sh        /usr/local/bin/healthcheck.sh
COPY deploy/pi/torrc                 /etc/tor/torrc
# The fallback copy; the systemd unit bind-mounts the host's over it, because
# sweep() re-reads this every pass so a new link needs no rebuild.
COPY crates/checker/links.toml       /etc/tor-taxi/links.toml

RUN chmod +x /usr/local/bin/entrypoint.sh /usr/local/bin/healthcheck.sh

# debian-tor arrives with the tor package and already owns /var/lib/tor, so the
# named volume inherits the right ownership with no chown at startup.
USER debian-tor

HEALTHCHECK --interval=60s --timeout=15s --start-period=300s --retries=3 \
  CMD ["/usr/local/bin/healthcheck.sh"]

# --links belongs in ENTRYPOINT, not CMD: as CMD it would vanish the moment
# anyone passed --once. clap's Set action keeps the last value, so a later
# --links still overrides it.
ENTRYPOINT ["/usr/local/bin/entrypoint.sh", "--links", "/etc/tor-taxi/links.toml"]
