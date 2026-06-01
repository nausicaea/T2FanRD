FROM docker.io/library/rust:1.96.0-alpine3.23 AS build
COPY Cargo.toml Cargo.lock /workdir/
COPY <<-EOF /workdir/src/main.rs
fn main() {}
EOF
WORKDIR /workdir
RUN cargo fetch --locked
COPY src/*.rs ./src/
RUN cargo build --frozen --release

#FROM docker.io/library/alpine:3.23
#VOLUME ["/sys/devices/platform/coretemp.0", "/sys/class/drm/card0", "/sys/devices/pci*"]
#COPY --from=build --chown=root:root --chmod=0755 /workdir/target/release/t2fanrd /usr/local/bin/t2fanrd
#USER 0:0
#ENTRYPOINT ["/usr/local/bin/t2fanrd"]
