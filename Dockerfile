ARG BUILDPLATFORM=linux/amd64
FROM --platform=${BUILDPLATFORM} docker.io/library/rust:1.99.0-alpine3.23@sha256:42c2519fdf75d9e34cc61a1aaf34b1a684da9bfd65fe5daefef98d92a801b001 AS build
ARG FEATURES=""
ARG RUSTFLAGS="-C target-feature=+crt-static"
ARG TARGET=x86_64-unknown-linux-musl
ENV RUSTFLAGS=${RUSTFLAGS}
RUN apk add --no-cache openssl-dev openssl-libs-static
WORKDIR /workdir
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs
RUN cargo build --locked --release --target ${TARGET} ${FEATURES}
COPY src/ ./src/
RUN find src -exec touch {} +; \
    cargo build --frozen --release --target ${TARGET} ${FEATURES}

FROM scratch
LABEL \
    org.opencontainers.image.title="t2fanrd" \
    org.opencontainers.image.description="Simple Fan Daemon for T2 Macs" \
    org.opencontainers.image.authors="GnomedDev,nausicaea" \
    org.opencontainers.image.source="https://github.com/nausicaea/t2fanrd" \
    org.opencontainers.image.version="0.4.9" \
    org.opencontainers.image.licenses="GPL-3.0-only"
COPY manifest.yaml /
COPY rootfs/ /rootfs/
COPY --from=build --chown=root:root --chmod=0755 /workdir/target/x86_64-unknown-linux-musl/release/t2fanrd /rootfs/usr/local/lib/containers/t2fanrd/t2fanrd
