ARG RUST_VERSION = 1.9
ARG DEBIAN_SUITE = trixie
FROM --platform=$BUILDPLATFORM rust:${rust_version}-${DEBIAN_SUITE} AS build

RUN apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-reccomends \
    gcc-aarch64-linux-gnu \
    libc6-dev-arm64-cross && \
    rm -rf /var/lib/apt/lists/*

RUN rustup target add aarch64-linux-gnu    

ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc \
    AR_aarch64_unknown_linux_gnu=aarch64-linux-gnu-ar

WORKDIR /build
COPY ..

RUN --mount=type=cache, target=/usr/local/cargo/registry, sharing=locked \
    --mount=type=cache, target=/usr/local/cargo/git, sharing=locked \ 
    --mount=type=cache, target=/build/target, sharing=locked \
    cargo build --release --locked \
    --target aarch64-unknown-linux-gnu \
    -p tor-taxi-checker &&
    cp target/aarch64-unknown-linux-gnu/release/tor-taxi-checker /tor-taxi-checker

FROM debian:${DEBIAN_SUITE}-slim


RUN apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
      tor && \
    rm -rf /var/lib/apt/lists/*

COPY --from=build /tor-taxi-checker  /usr/local/bin/tor-taxi-checker
COPY deploy/pi/entrypoint.sh         /usr/local/bin/entrypoint.sh
COPY deploy/pi/healthcheck.sh        /usr/local/bin/healthcheck.sh
COPY deploy/pi/torrc                 /etc/tor/torrc   


COPY crates/checker/links.toml      /etc/tor-taxi/links.toml
RUN chmod +x /usr/local/bin/entrypoint.sh   /usr/local/bin/healthcheck.sh

USER debian-tor

HEALTHCHECK --interval=60s --timeout=15s --start-period=300s --retries=3 \
  CMD ["/usr/local/bin/healthcheck.sh"]

ENTRYPOINT ["/usr/local/bin/entrypoint.sh", "--links", "/etc/tor-taxi/links.toml"]



