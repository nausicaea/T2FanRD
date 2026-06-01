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
COPY manifest.yaml /
COPY rootfs/ /rootfs/
COPY --from=build --chown=root:root --chmod=0755 /workdir/target/x86_64-unknown-linux-musl/release/t2fanrd /rootfs/usr/local/sbin/t2fanrd
COPY --from=build --chown=root:root --chmod=0755 /workdir/target/x86_64-unknown-linux-musl/release/t2fanrd /rootfs/usr/local/lib/containers/t2fanrd/t2fanrd
