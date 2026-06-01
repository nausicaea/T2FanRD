ARG BUILDPLATFORM=linux/amd64
FROM --platform=${BUILDPLATFORM} docker.io/library/rust:1.96.0-alpine3.23 AS build
ARG RUSTFLAGS="-C target-feature=+crt-static"
ARG TARGET=x86_64-unknown-linux-musl
WORKDIR /workdir
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs
RUN cargo fetch --locked --target ${TARGET}
COPY src/ ./src/
RUN cargo build --frozen --release --target ${TARGET}

FROM scratch
LABEL org.opencontainers.image.title="t2fanrd"
LABEL org.opencontainers.image.description="Simple Fan Daemon for T2 Macs"
LABEL org.opencontainers.image.authors="GnomedDev,nausicaea"
LABEL org.opencontainers.image.source="https://github.com/nausicaea/t2fanrd"
LABEL org.opencontainers.image.version="0.1.0"
LABEL org.opencontainers.image.licenses="GPL-3.0-only"
COPY manifest.yaml /
COPY rootfs/ /rootfs/
COPY --from=build --chown=root:root --chmod=0755 /workdir/target/x86_64-unknown-linux-musl/release/t2fanrd /rootfs/usr/local/sbin/t2fanrd
COPY --from=build --chown=root:root --chmod=0755 /workdir/target/x86_64-unknown-linux-musl/release/t2fanrd /rootfs/usr/local/lib/containers/t2fanrd/t2fanrd
