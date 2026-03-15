FROM rust:1.88-bookworm AS builder

RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY .cargo .cargo
COPY crates crates
COPY proto proto

RUN cargo build --release --bin bws-csi-provider

FROM gcr.io/distroless/cc-debian12

COPY --from=builder /build/target/release/bws-csi-provider /usr/local/bin/bws-csi-provider

ENTRYPOINT ["bws-csi-provider"]
