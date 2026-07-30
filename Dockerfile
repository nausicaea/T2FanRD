ARG BUILDPLATFORM=linux/amd64
FROM --platform=${BUILDPLATFORM} docker.io/library/rust:1.97.1-alpine3.23@sha256:c4a364ddbf684fe038e6fa6a4f25b30c8dc85247423e0e660676ece0d17be4a2 AS build
ARG FEATURES=""
ARG RUSTFLAGS="-C target-feature=+crt-static"
ARG TARGET=x86_64-unknown-linux-musl
RUN apk add --no-cache openssl-dev openssl-libs-static
WORKDIR /workdir
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs
RUN cargo build --locked --release --target ${TARGET}
COPY src/ ./src/
RUN cargo build --frozen --release --target ${TARGET} ${FEATURES}

FROM scratch
LABEL \
    org.opencontainers.image.title="t2fanrd" \
    org.opencontainers.image.description="Simple Fan Daemon for T2 Macs" \
    org.opencontainers.image.authors="GnomedDev,nausicaea" \
    org.opencontainers.image.source="https://github.com/nausicaea/t2fanrd" \
    org.opencontainers.image.version="0.4.6" \
    org.opencontainers.image.licenses="GPL-3.0-only"
COPY manifest.yaml /
COPY rootfs/ /rootfs/
COPY --from=build --chown=root:root --chmod=0755 /workdir/target/x86_64-unknown-linux-musl/release/t2fanrd /rootfs/usr/local/lib/containers/t2fanrd/t2fanrd
