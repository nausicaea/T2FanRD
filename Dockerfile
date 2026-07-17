ARG BUILDPLATFORM=linux/amd64
FROM --platform=${BUILDPLATFORM} docker.io/library/rust:1.96.0-alpine3.23@sha256:5dc2af9dd547c33f64d5fc1d299ab93b51f39eaa16c426c476b990ce6caf5b3e AS build
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
    org.opencontainers.image.version="0.4.5" \
    org.opencontainers.image.licenses="GPL-3.0-only"
COPY manifest.yaml /
COPY rootfs/ /rootfs/
COPY --from=build --chown=root:root --chmod=0755 /workdir/target/x86_64-unknown-linux-musl/release/t2fanrd /rootfs/usr/local/lib/containers/t2fanrd/t2fanrd
