ARG BUILDPLATFORM=linux/amd64
FROM --platform=${BUILDPLATFORM} docker.io/library/rust:1.96.0-alpine3.23@sha256:66f48b19d6e88519e2e58bebe0d945779a6a4ca41c2db17db78c9569655b50ac AS build
ARG FEATURES="-F observability"
ARG RUSTFLAGS="-C target-feature=+crt-static"
ARG TARGET=x86_64-unknown-linux-musl
RUN apk add --no-cache openssl-dev openssl-libs-static
WORKDIR /workdir
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs
RUN cargo fetch --locked --target ${TARGET}
COPY src/ ./src/
RUN cargo build --frozen --release --target ${TARGET} ${FEATURES}

FROM scratch
LABEL \
    org.opencontainers.image.title="t2fanrd" \
    org.opencontainers.image.description="Simple Fan Daemon for T2 Macs" \
    org.opencontainers.image.authors="GnomedDev,nausicaea" \
    org.opencontainers.image.source="https://github.com/nausicaea/t2fanrd" \
    org.opencontainers.image.version="0.4.3" \
    org.opencontainers.image.licenses="GPL-3.0-only"
COPY manifest.yaml /
COPY rootfs/ /rootfs/
COPY --from=build --chown=root:root --chmod=0755 /workdir/target/x86_64-unknown-linux-musl/release/t2fanrd /rootfs/usr/local/lib/containers/t2fanrd/t2fanrd
